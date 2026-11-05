use sega_sms_core::SmsMachine;

#[test]
fn cpu_reset_preserves_machine_clock_and_psg_phase() {
    let mut machine = SmsMachine::new(vec![0; 0x4000]).unwrap();

    assert_eq!(machine.step(), Ok(4));
    let tstates_before_reset = machine.cpu().total_tstates;
    let psg_phase_before_reset = machine.psg().clock_divider_phase();
    let raster_before_reset = machine.vdp().raster_position();

    machine.reset_cpu();

    assert_eq!(machine.cpu().registers.pc, 0);
    assert_eq!(machine.cpu().total_tstates, tstates_before_reset);
    assert_eq!(machine.psg().clock_divider_phase(), psg_phase_before_reset);
    assert_eq!(
        machine.vdp().raster_position().line,
        raster_before_reset.line
    );
    assert_eq!(
        machine.vdp().raster_position().tstates_into_line,
        raster_before_reset.tstates_into_line
    );

    assert_eq!(machine.step(), Ok(4));
    assert_eq!(machine.cpu().total_tstates, tstates_before_reset + 4);
    assert_eq!(
        machine.psg().clock_divider_phase(),
        psg_phase_before_reset + 4
    );
    assert_eq!(
        machine.vdp().raster_position().tstates_into_line,
        raster_before_reset.tstates_into_line + 4
    );
}
