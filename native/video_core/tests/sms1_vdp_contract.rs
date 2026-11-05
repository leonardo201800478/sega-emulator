//! Public-contract tests for the shared SMS1/315-5124 VDP.
//!
//! These tests exercise the public port surface and documented register/memory
//! protocol. They do not assert the legacy scanline duration as physical timing.

use sega_video::sms::{SmsVdp, SmsVdpProfile};

fn new_sms1() -> SmsVdp {
    SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc())
}

fn select(vdp: &mut SmsVdp, address: u16, code: u8) {
    vdp.write_port(0xBF, address as u8);
    vdp.write_port(0xBF, ((code & 0x03) << 6) | ((address >> 8) as u8 & 0x3F));
}

#[test]
fn control_port_requires_two_bytes_and_status_read_cancels_partial_word() {
    let mut vdp = new_sms1();

    vdp.write_port(0xBF, 0x12);
    assert_eq!(vdp.address(), 0x0000);
    assert_eq!(vdp.read_port(0xBF), 0x00);

    // These bytes now form a fresh VRAM-write command for address 0x1640.
    select(&mut vdp, 0x1640, 1);
    assert_eq!(vdp.address(), 0x1640);
    vdp.write_port(0xBE, 0xA5);
    assert_eq!(vdp.vram()[0x1640], 0xA5);
}

#[test]
fn data_port_access_cancels_an_incomplete_control_word() {
    let mut vdp = new_sms1();

    vdp.write_port(0xBF, 0x12);
    vdp.write_port(0xBE, 0xA5);

    // The second control byte cannot pair with the abandoned 0x12 byte.
    select(&mut vdp, 0x0034, 1);
    assert_eq!(vdp.address(), 0x0034);
    vdp.write_port(0xBE, 0x5A);

    select(&mut vdp, 0x0034, 0);
    assert_eq!(vdp.read_port(0xBE), 0x5A);
}

#[test]
fn vram_read_ahead_returns_prefetched_byte_and_wraps_at_0x3fff() {
    let mut vdp = new_sms1();

    select(&mut vdp, 0x3FFF, 1);
    vdp.write_port(0xBE, 0xA5);
    select(&mut vdp, 0x0000, 1);
    vdp.write_port(0xBE, 0x5A);

    select(&mut vdp, 0x3FFF, 0);
    assert_eq!(vdp.address(), 0x0000);
    assert_eq!(vdp.read_port(0xBE), 0xA5);
    assert_eq!(vdp.read_port(0xBE), 0x5A);
    assert_eq!(vdp.address(), 0x0002);
}

#[test]
fn cram_write_uses_32_entry_alias_and_masks_to_six_bits() {
    let mut vdp = new_sms1();

    select(&mut vdp, 0x0020, 3);
    vdp.write_port(0xBE, 0xFF);

    assert_eq!(vdp.cram()[0], 0x3F);
    assert_eq!(vdp.cram()[0x1F], 0x00);
}

#[test]
fn register_write_accepts_r0_through_r10_and_ignores_r11() {
    let mut vdp = new_sms1();

    vdp.write_port(0xBF, 0x5A);
    vdp.write_port(0xBF, 0x8A);
    assert_eq!(vdp.registers()[10], 0x5A);

    vdp.write_port(0xBF, 0xC3);
    vdp.write_port(0xBF, 0x8B);
    assert_eq!(vdp.registers()[11], 0x00);
}

#[test]
fn mirrored_data_and_control_ports_share_the_same_protocol() {
    let mut vdp = new_sms1();

    select(&mut vdp, 0x0034, 1);
    vdp.write_port(0x01BE, 0xC7);
    select(&mut vdp, 0x0034, 0);

    assert_eq!(vdp.read_port(0x01BE), 0xC7);
}

#[test]
fn status_read_returns_vblank_once_and_clears_it() {
    let mut vdp = new_sms1();

    // The current baseline uses 228 T-states/line. This test verifies only the
    // public status/acknowledge contract, not that line duration as hardware truth.
    vdp.advance(228 * 192);
    assert_ne!(vdp.status() & 0x80, 0);

    assert_ne!(vdp.read_port(0x00BF) & 0x80, 0);
    assert_eq!(vdp.read_port(0x01BF) & 0x80, 0);
}
