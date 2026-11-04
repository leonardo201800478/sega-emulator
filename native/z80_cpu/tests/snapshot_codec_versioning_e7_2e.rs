use sega_z80_cpu::{
    Z80_MICRO_SEQUENCER_STATE_VERSION, Z80MicroSequencer, Z80MicroSequencerStateCodecError,
};

#[test]
fn e7_2e_codec_rejects_a_snapshot_from_an_unsupported_schema_version() {
    let cpu = Z80MicroSequencer::default();
    let mut bytes = cpu.export_state().encode().unwrap();
    let unsupported = Z80_MICRO_SEQUENCER_STATE_VERSION - 1;
    bytes[8..10].copy_from_slice(&unsupported.to_be_bytes());

    assert_eq!(
        sega_z80_cpu::Z80MicroSequencerState::decode(&bytes),
        Err(Z80MicroSequencerStateCodecError::UnsupportedVersion {
            expected: Z80_MICRO_SEQUENCER_STATE_VERSION,
            actual: unsupported,
        })
    );
}

#[test]
fn e7_2e_codec_refuses_to_encode_a_dto_with_an_old_schema_version() {
    let cpu = Z80MicroSequencer::default();
    let mut state = cpu.export_state();
    state.format_version = Z80_MICRO_SEQUENCER_STATE_VERSION - 1;

    assert_eq!(
        state.encode(),
        Err(Z80MicroSequencerStateCodecError::InvalidState)
    );
}
