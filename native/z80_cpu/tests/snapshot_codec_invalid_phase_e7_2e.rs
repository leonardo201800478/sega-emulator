use sega_z80_cpu::{
    Z80MicroSequencer, Z80MicroSequencerState, Z80MicroSequencerStateCodecError,
};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[test]
fn e7_2e_codec_rejects_unknown_phase_tag_after_valid_checksum() {
    const HEADER_LEN: usize = 14;
    const PHASE_TAG_OFFSET_IN_BODY: usize = 27;

    let cpu = Z80MicroSequencer::default();
    let mut bytes = cpu.export_state().encode().unwrap();
    let body_len = u32::from_be_bytes(bytes[10..14].try_into().unwrap()) as usize;
    let checksum_offset = HEADER_LEN + body_len;
    let phase_tag_offset = HEADER_LEN + PHASE_TAG_OFFSET_IN_BODY;

    assert_eq!(bytes[phase_tag_offset], 0, "default Ready phase uses tag zero");
    bytes[phase_tag_offset] = 0xFF;
    let checksum = crc32(&bytes[HEADER_LEN..checksum_offset]);
    bytes[checksum_offset..checksum_offset + 4].copy_from_slice(&checksum.to_be_bytes());

    assert_eq!(
        Z80MicroSequencerState::decode(&bytes),
        Err(Z80MicroSequencerStateCodecError::InvalidTag(0xFF))
    );
}
