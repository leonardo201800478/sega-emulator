//! Public API tests for the Rust-only SMS runtime boundary.

use sega_sms_core::ControllerButtons;
use sega_sms_runtime::{DiagnosticsConsent, SmsTemporalWorker};

#[test]
fn public_worker_api_loads_steps_resets_and_accepts_controller_levels() {
    let rom = vec![0; 0x4000];
    let worker = SmsTemporalWorker::new(rom.clone(), "usa").unwrap();

    worker
        .set_controller(
            1,
            ControllerButtons {
                up: true,
                down: false,
                left: false,
                right: false,
                button_1: true,
                button_2: false,
            },
        )
        .unwrap();
    assert_eq!(worker.frame_number(), 0);

    assert!(worker.step_frame(100_000).unwrap() > 0);
    assert_eq!(worker.frame_number(), 1);

    worker.reset().unwrap();
    assert_eq!(worker.frame_number(), 0);

    worker.load_rom(rom).unwrap();
    assert_eq!(worker.frame_number(), 0);
}

#[test]
fn public_worker_api_saves_and_restores_a_complete_paused_machine_state() {
    let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
    worker.enable_diagnostics(&DiagnosticsConsent::from_explicit_user_action());

    worker.step_frame(100_000).unwrap();
    let saved = worker.save_state().unwrap();
    let saved_frame = worker.frame_number();
    let saved_debug = worker.debug_snapshot().unwrap();

    worker.step_frame(100_000).unwrap();
    assert!(worker.frame_number() > saved_frame);

    worker.load_state(saved).unwrap();
    assert_eq!(worker.frame_number(), saved_frame);

    let restored_debug = worker.debug_snapshot().unwrap();
    assert_eq!(restored_debug.pc, saved_debug.pc);
    assert_eq!(
        restored_debug.machine_time_ticks,
        saved_debug.machine_time_ticks
    );
    assert_eq!(restored_debug.vdp_scanline, saved_debug.vdp_scanline);
    assert_eq!(
        restored_debug.vdp_cycles_into_line,
        saved_debug.vdp_cycles_into_line
    );
}

#[test]
fn invalid_save_state_is_rejected_without_mutating_the_running_machine() {
    let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
    worker.enable_diagnostics(&DiagnosticsConsent::from_explicit_user_action());
    worker.step_frame(100_000).unwrap();

    let before = worker.debug_snapshot().unwrap();
    let mut invalid = worker.save_state().unwrap();
    invalid.machine_state.format_version = 0;

    assert!(worker.load_state(invalid).is_err());
    let after = worker.debug_snapshot().unwrap();
    assert_eq!(after.pc, before.pc);
    assert_eq!(after.machine_time_ticks, before.machine_time_ticks);
}

#[test]
fn public_worker_api_accepts_sms1_presentation_crop_setting() {
    use sega_sms_runtime::SmsOverscanMode;

    let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
    worker
        .set_overscan_mode(SmsOverscanMode::CropLeft8)
        .unwrap();
    worker.set_overscan_mode(SmsOverscanMode::Off).unwrap();
}

#[test]
fn save_state_rejects_a_different_rom_without_mutating_the_target_machine() {
    let source = SmsTemporalWorker::new(vec![0x00; 0x4000], "usa").unwrap();
    source.step_frame(100_000).unwrap();
    let saved = source.save_state().unwrap();

    let target = SmsTemporalWorker::new(vec![0xFF; 0x4000], "usa").unwrap();
    target.enable_diagnostics(&DiagnosticsConsent::from_explicit_user_action());
    target.step_frame(100_000).unwrap();
    let before = target.debug_snapshot().unwrap();

    assert!(target.load_state(saved).is_err());
    let after = target.debug_snapshot().unwrap();
    assert_eq!(after.pc, before.pc);
    assert_eq!(after.machine_time_ticks, before.machine_time_ticks);
}

#[test]
fn save_state_rejects_a_different_region_profile() {
    let source = SmsTemporalWorker::new(vec![0x00; 0x4000], "usa").unwrap();
    source.step_frame(100_000).unwrap();
    let saved = source.save_state().unwrap();

    let target = SmsTemporalWorker::new(vec![0x00; 0x4000], "europe").unwrap();
    target.step_frame(100_000).unwrap();
    assert!(target.load_state(saved).is_err());
}
