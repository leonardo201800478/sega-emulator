use sega_sms_core::SmsTemporalMachine;

fn read_h_counter_after_control_writes(values: &[u8]) -> u8 {
    let mut rom = vec![0; 0x8000];
    let mut cursor = 0;

    for value in values {
        rom[cursor..cursor + 2].copy_from_slice(&[0x3E, *value]); // LD A,n
        cursor += 2;
        rom[cursor..cursor + 2].copy_from_slice(&[0xD3, 0x3F]); // OUT (3F),A
        cursor += 2;
    }

    rom[cursor..cursor + 2].copy_from_slice(&[0xDB, 0x7F]); // IN A,(7F)
    cursor += 2;
    rom[cursor..cursor + 3].copy_from_slice(&[0x32, 0x00, 0xC0]); // LD (C000),A

    let mut machine = SmsTemporalMachine::new(rom, 3).expect("SMS temporal machine");
    for _ in 0..(values.len() * 2 + 2) {
        machine
            .step_instruction(0)
            .expect("synthetic Z80 instruction completes");
    }

    machine.export_state().bus.memory.read(0xC000)
}

#[test]
fn th1_output_low_to_input_high_latches_hcounter_at_bus_completion() {
    // TH1: output-low (0xDD) -> input-high (0xDF).
    // The second OUT completes at Z80 T-state 36, where the SMS H-counter is 0x03.
    assert_eq!(read_h_counter_after_control_writes(&[0xDD, 0xDF]), 0x03);
}

#[test]
fn th2_output_low_to_input_high_latches_hcounter_at_bus_completion() {
    // TH2: output-low (0x77) -> input-high (0x7F).
    assert_eq!(read_h_counter_after_control_writes(&[0x77, 0x7F]), 0x03);
}

#[test]
fn output_low_to_output_high_does_not_latch_hcounter() {
    // Both TH pins remain outputs. A low-to-high output-level change is not
    // the input TH rising edge that clocks the SMS H-counter latch.
    assert_eq!(read_h_counter_after_control_writes(&[0x55, 0x75]), 0x00);
}

#[test]
fn falling_th_transition_does_not_replace_the_latched_hcounter() {
    // The first pair latches 0x03. Returning both TH pins to output-low is a
    // falling transition and must leave the captured value unchanged.
    assert_eq!(
        read_h_counter_after_control_writes(&[0x55, 0xFF, 0x55]),
        0x03
    );
}
