//! Deterministic SMS VBlank IRQ wiring reproducer.
//!
//! Run from the repository root:
//! cargo run --release -p sega-sms-core --example irq_wiring_repro

use sega_sms_core::{SmsTemporalMachine, SmsVdpProfile, Territory};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rom = vec![0; 0x8000];
    let main: [u8; 18] = [
        0xF3, // DI
        0x31, 0xF0, 0xDF, // LD SP,$DFF0
        0xED, 0x56, // IM 1
        0x3E, 0xE0, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF, // R1: 16KB, display, VBlank IRQ
        0xFB, // EI
        0x76, // HALT
        0x18, 0xFD, // JR -3
    ];
    rom[..main.len()].copy_from_slice(&main);

    let isr: [u8; 14] = [
        0xF5, // PUSH AF
        0xDB, 0xBF, // IN A,($BF): acknowledge VDP IRQ
        0x3A, 0x00, 0xC0, // LD A,($C000)
        0x3C, // INC A
        0x32, 0x00, 0xC0, // LD ($C000),A
        0xF1, // POP AF
        0xFB, // EI
        0xED, 0x4D, // RETI
    ];
    rom[0x38..0x38 + isr.len()].copy_from_slice(&isr);

    let mut machine = SmsTemporalMachine::new_with_hardware_profile(
        rom,
        1,
        3_579_545,
        44_100,
        SmsVdpProfile::sms1_t0_ntsc(),
        Territory::Export,
    )?;

    const FRAMES: u8 = 20;
    for frame in 1..=FRAMES {
        let instructions = machine.step_frame(100_000)?;
        println!(
            "frame={frame:02}/{FRAMES} isr_count={} instructions={instructions}",
            machine.memory().read(0xC000)
        );
    }

    let count = machine.memory().read(0xC000);
    println!("ISR executed {count} times in {FRAMES} frames");
    if count != FRAMES {
        return Err(format!("expected {FRAMES} VBlank IRQs, got {count}").into());
    }

    Ok(())
}
