//! Full-machine ROM-runner smoke fixture generated in memory.
//!
//! This is a synthetic homebrew loop, not an external diagnostic suite and not
//! a PASS claim for SMSmemtest/SMSTestSuite/SmsVdpTest.

use sega_sms_core::{SmsTemporalMachine, SmsVdpProfile, Territory};

#[test]
fn synthetic_cartridge_completes_frames_through_the_machine_scheduler() {
    let mut rom = vec![0x00; 0x8000];
    // JP $0000: keep the CPU active while the Scheduler advances the SMS VDP.
    rom[..3].copy_from_slice(&[0xC3, 0x00, 0x00]);

    let mut machine = SmsTemporalMachine::new_with_hardware_profile(
        rom,
        1,
        3_579_545,
        48_000,
        SmsVdpProfile::sms1_t0_ntsc(),
        Territory::Export,
    )
    .expect("synthetic cartridge should construct a complete SMS temporal machine");

    let initial_frame = machine.frame_number();
    for completed_frame in 1..=2 {
        let instructions = machine
            .step_frame(100_000)
            .expect("synthetic CPU loop should reach the next VDP frame boundary");

        assert!(
            instructions > 0,
            "frame stepping must execute CPU instructions"
        );
        assert_eq!(
            machine.frame_number(),
            initial_frame + completed_frame,
            "one step_frame call must complete exactly one VDP frame"
        );

        // Keep the PSG output queue bounded just as the offline runner does.
        let _drained_samples = machine.drain_audio_samples(None);
    }
}
