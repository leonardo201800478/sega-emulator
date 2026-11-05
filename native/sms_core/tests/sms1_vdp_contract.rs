//! Independent public-contract tests for the SMS1/315-5124 VDP port surface.
//!
//! Expected values are fixed from the documented SMS control-port protocol,
//! VRAM read-ahead, 14-bit address wrap and 32-entry six-bit CRAM format.
//! These tests deliberately do not call private renderer/port helpers.

use sega_sms_core::vdp_sms1::{Sms1Vdp, VideoStandard};

fn select(vdp: &mut Sms1Vdp, address: u16, code: u8) {
    vdp.write_control(address as u8);
    vdp.write_control(((code & 0x03) << 6) | ((address >> 8) as u8 & 0x3F));
}

#[test]
fn control_port_requires_two_bytes_and_status_read_cancels_partial_word() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);

    vdp.write_control(0x12);
    vdp.set_status_bits(0x20);
    assert_eq!(vdp.read_status(), 0x20);
    assert_eq!(vdp.read_status(), 0x00);

    // The read-status operation abandons the earlier 0x12 byte. These two
    // writes therefore form a new word with address 0x1640 and code 1 (VRAM write).
    vdp.write_control(0x40);
    vdp.write_control(0x56);
    assert_eq!(vdp.address(), 0x1640);
    vdp.write_data(0xA5);
    assert_eq!(vdp.state.memory.vram_byte(0x1640), Some(0xA5));
}

#[test]
fn data_port_access_cancels_an_incomplete_control_word() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);

    vdp.write_control(0x12);
    vdp.write_data(0xA5);

    // The previous first control byte must not combine with this second byte.
    vdp.write_control(0x34);
    vdp.write_control(0x40);
    assert_eq!(vdp.address(), 0x0034);
}

#[test]
fn vram_read_ahead_returns_prefetched_byte_and_wraps_at_0x3fff() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
    assert!(vdp.state.memory.set_vram_byte(0x3FFF, 0xA5));
    assert!(vdp.state.memory.set_vram_byte(0x0000, 0x5A));

    select(&mut vdp, 0x3FFF, 0);

    // Programming the read address preloads the read-ahead buffer and wraps
    // the 14-bit address before the first data-port read.
    assert_eq!(vdp.read_buffer(), 0xA5);
    assert_eq!(vdp.address(), 0x0000);
    assert_eq!(vdp.read_data(), 0xA5);
    assert_eq!(vdp.read_data(), 0x5A);
    assert_eq!(vdp.address(), 0x0002);
}

#[test]
fn cram_write_uses_32_entry_alias_and_masks_to_six_bits() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);

    // CRAM write code 3 with address 0x20 aliases to entry zero.
    select(&mut vdp, 0x0020, 3);
    vdp.write_data(0xFF);

    assert_eq!(vdp.state.memory.cram_byte(0), Some(0x3F));
    assert_eq!(vdp.state.memory.cram_byte(0x1F), Some(0x00));
}

#[test]
fn register_write_accepts_r0_through_r10_and_ignores_r11() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);

    vdp.write_control(0x5A);
    vdp.write_control(0x8A); // Register write, R10.
    assert_eq!(vdp.state.registers.get(10), Some(0x5A));

    vdp.write_control(0xC3);
    vdp.write_control(0x8B); // Register write, R11 is outside SMS1 R0-R10.
    assert_eq!(vdp.state.registers.get(11), None);
}

#[test]
fn mirrored_data_and_control_ports_share_the_same_protocol() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);

    // $01BF mirrors $BF; $01BE mirrors $BE.
    vdp.write_port(0x01BF, 0x34);
    vdp.write_port(0x01BF, 0x40);
    vdp.write_port(0x01BE, 0xC7);

    assert_eq!(vdp.state.memory.vram_byte(0x0034), Some(0xC7));
}

#[test]
fn status_read_returns_latched_bits_once_and_clears_them() {
    let mut vdp = Sms1Vdp::new(VideoStandard::Ntsc);
    vdp.set_status_bits(0x60);

    assert_eq!(vdp.read_port(0x00BF), 0x60);
    assert_eq!(vdp.read_port(0x01BF), 0x00);
}
