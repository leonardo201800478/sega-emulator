//! Public API tests for the Rust-only SMS runtime boundary.

use sega_sms_core::ControllerButtons;
use sega_sms_runtime::SmsTemporalWorker;

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
