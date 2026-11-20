//! CPU-visible SMS1 VCounter and raster IRQ integration through the MachineScheduler.
//!
//! These fixtures deliberately use the real Z80 port instructions and the
//! temporal bus. They are discriminating integration tests, not a claim that
//! the complete SmsVdpTest ROM has been reproduced.

use sega_sms_core::{SmsTemporalMachine, SmsTemporalTraceEvent, SmsVdpProfile, Territory};
use sega_z80_cpu::Z80BusOperation;

const ROM_SIZE: usize = 0x8000;
const PSG_CLOCK_HZ: u64 = 3_579_545;
const SAMPLE_RATE_HZ: u64 = 44_100;

fn machine_with_program(program: &[u8], interrupt_handler: Option<&[u8]>) -> SmsTemporalMachine {
    let mut rom = vec![0; ROM_SIZE];
    rom[..program.len()].copy_from_slice(program);
    if let Some(handler) = interrupt_handler {
        rom[0x38..0x38 + handler.len()].copy_from_slice(handler);
    }

    SmsTemporalMachine::new_with_hardware_profile(
        rom,
        1,
        PSG_CLOCK_HZ,
        SAMPLE_RATE_HZ,
        SmsVdpProfile::sms1_t0_ntsc(),
        Territory::Export,
    )
    .expect("SMS1 temporal machine should initialize")
}

fn run_instructions(machine: &mut SmsTemporalMachine, count: usize) {
    for _ in 0..count {
        machine
            .step_complete_instruction()
            .expect("Z80 instruction should complete through the Scheduler");
    }
}

fn run_until_memory(machine: &mut SmsTemporalMachine, address: u16, value: u8, limit: usize) {
    for _ in 0..limit {
        if machine.inspect_memory(address) == value {
            return;
        }
        machine
            .step_complete_instruction()
            .expect("Z80 instruction should complete through the Scheduler");
    }
    panic!(
        "timed out waiting for address {address:#06X}={value:#04X}; got {:#04X}",
        machine.inspect_memory(address)
    );
}

#[test]
fn smsvdptest_run_test25_vcounter_transition_has_one_increment_and_stable_followup() {
    fn read_after_nops(nops: usize) -> (u8, u64) {
        let mut program = vec![0x00; nops]; // NOPs align the CPU against the raster.
        program.extend_from_slice(&[
            0xDB, 0x40, // IN A,($40)
            0x32, 0x00, 0xC0, // LD ($C000),A
            0x76, // HALT
        ]);
        let mut machine = machine_with_program(&program, None);
        machine.set_trace_enabled(true);
        run_instructions(&mut machine, nops + 2);

        let value = machine.inspect_memory(0xC000);
        let read_timestamp = machine
            .take_trace()
            .into_iter()
            .find_map(|event| match event {
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation: Z80BusOperation::PortRead,
                    address: 0x0040,
                    data: Some(data),
                    ..
                } => Some((data, timestamp.ticks() as u64)),
                _ => None,
            })
            .expect("the Z80 IN instruction must complete a scheduled port read");

        assert_eq!(value, read_timestamp.0);
        read_timestamp
    }

    // 57 NOPs consume 228 T-states. IN completes at T11, before the T15 latch.
    let before = read_after_nops(57);
    assert_eq!(before, (0x00, 239));

    // One more NOP moves the same IN completion to T15 exactly, still
    // before the HPos23 threshold (first representable Z80 boundary is T16).
    let at_boundary = read_after_nops(58);
    assert_eq!(at_boundary, (0x00, 243));

    // The next sample remains on the new counter value, matching the
    // third sample in SmsVdpTest RunTest25's before/after transition check.
    let after = read_after_nops(59);
    assert_eq!(after, (0x01, 247));
}

#[test]
fn smsvdptest_run_test31_frame_irq_isr_reads_vcounter_c1() {
    // DI; LD SP,$DFF0; IM 1; enable VINT in R1; EI; HALT; JR HALT.
    let program = [
        0xF3, 0x31, 0xF0, 0xDF, 0xED, 0x56, 0x3E, 0x20, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF, 0xFB,
        0x76, 0x18, 0xFD,
    ];
    // Read/acknowledge VDP status, then read VCounter and store both values.
    let handler = [
        0xDB, 0xBF, 0x32, 0x01, 0xC0, 0xDB, 0x40, 0x32, 0x00, 0xC0, 0xFB, 0xED, 0x4D,
    ];
    let mut machine = machine_with_program(&program, Some(&handler));

    run_until_memory(&mut machine, 0xC000, 0xC1, 60_000);

    assert_eq!(
        machine.inspect_memory(0xC001) & 0x80,
        0x80,
        "the VINT ISR must observe the VBlank status bit"
    );
    assert_eq!(
        machine.vdp().read_v_counter(),
        0xC1,
        "the VCounter latch must remain on the first VBlank line after ISR service"
    );
}

#[test]
fn line_irq_r10_c0_is_observed_before_vint_as_vcounter_c0() {
    // Configure R10=$BE and enable HINT. The first HINT at line 1 reloads
    // the countdown; the next underflow is the first non-visible line C0.
    // VINT is deliberately disabled so the C0 sample is unambiguously HINT.
    let program = [
        0xF3, 0x31, 0xF0, 0xDF, 0xED, 0x56, 0x3E, 0xBE, 0xD3, 0xBF, 0x3E, 0x8A, 0xD3, 0xBF, 0xDB,
        0xBF, // Clear the initial line-counter underflow before enabling HINT.
        0x3E, 0x14, 0xD3, 0xBF, 0x3E, 0x80, 0xD3, 0xBF, 0xFB, 0x76, 0x18, 0xFD,
    ];
    // The status read clears the line IRQ before VCounter is sampled.
    // C010=status, C011=VCounter, C012=number of serviced HINTs.
    let handler = [
        0xDB, 0xBF, 0x32, 0x10, 0xC0, 0xDB, 0x40, 0x32, 0x11, 0xC0, 0x3A, 0x12, 0xC0, 0x3C, 0x32,
        0x12, 0xC0, 0xFB, 0xED, 0x4D,
    ];
    let mut machine = machine_with_program(&program, Some(&handler));

    run_until_memory(&mut machine, 0xC012, 0x02, 60_000);

    assert_eq!(
        machine.vdp().registers()[10],
        0xBE,
        "R10 register write must survive the temporal bus"
    );
    assert_eq!(
        machine.inspect_memory(0xC010) & 0x80,
        0,
        "the HINT at VCounter C0 must be serviced before VBlank status is set"
    );
    assert_eq!(
        machine.inspect_memory(0xC011),
        0xC0,
        "R10=$BE countdown must produce the second HINT on the first non-visible line"
    );
}

fn run_smsvdptest_run_test33_hint_sample(r10: u8) -> (u8, u8) {
    // Match RunTest33's sequence: wait for VBlank via IM1, then program
    // R10 and enable HINT. The ISR distinguishes VINT (status bit 7) from
    // HINT and stores the CPU-visible VCounter sample.
    let program = [
        0xF3, 0x31, 0xF0, 0xDF, 0xED, 0x56, 0x3E, 0x20, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF, 0xFB,
        0x76, 0x3E, r10, 0xD3, 0xBF, 0x3E, 0x8A, 0xD3, 0xBF, 0x3E, 0x14, 0xD3, 0xBF, 0x3E, 0x80,
        0xD3, 0xBF, 0xFB, 0x76, 0x18, 0xFD,
    ];
    let handler = [
        0xF5, // PUSH AF
        0xDB, 0xBF, // IN A,($BF) - acknowledge and classify IRQ
        0xCB, 0x7F, // BIT 7,A
        0x20, 0x0C, // JR NZ,vint
        0xDB, 0x40, // IN A,($40) - HINT VCounter sample
        0x32, 0x01, 0xC0, // LD ($C001),A
        0x3E, 0x01, 0x32, 0x00, 0xC0, // LD ($C000),A - HINT completed
        0x18, 0x05, // JR done
        0xDB, 0x40, // vint: IN A,($40)
        0x32, 0x02, 0xC0, // LD ($C002),A - first VINT sample
        0xF1, 0xFB, 0xED, 0x4D, // POP AF; EI; RETI
    ];
    let mut machine = machine_with_program(&program, Some(&handler));
    run_until_memory(&mut machine, 0xC000, 0x01, 120_000);
    (
        machine.inspect_memory(0xC001),
        machine.inspect_memory(0xC002),
    )
}

#[test]
fn smsvdptest_run_test33_hint_vcounter_matches_r10_samples() {
    for r10 in [0x00, 0x61, 0x80, 0xAF, 0xC0] {
        let (hint_sample, vblank_sample) = run_smsvdptest_run_test33_hint_sample(r10);
        assert_eq!(
            vblank_sample, 0xC1,
            "RunTest33 precondition: VINT ISR must sample C1 before R10={r10:#04X}"
        );
        assert_eq!(
            hint_sample, r10,
            "RunTest33 HINT VCounter sample must equal R10={r10:#04X}"
        );
    }
}
