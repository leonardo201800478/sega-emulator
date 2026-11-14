//! Public-contract tests for the shared SMS1/315-5124 VDP.
//!
//! These tests exercise the public port surface and documented register/memory
//! protocol. They do not assert the legacy scanline duration as physical timing.

use sega_video::sms::{SmsVdp, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard};

fn new_sms1() -> SmsVdp {
    SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc())
}

fn select(vdp: &mut SmsVdp, address: u16, code: u8) {
    vdp.write_port(0xBF, address as u8);
    vdp.write_port(0xBF, ((code & 0x03) << 6) | ((address >> 8) as u8 & 0x3F));
}

fn write_vram(vdp: &mut SmsVdp, address: u16, value: u8) {
    select(vdp, address, 1);
    vdp.write_port(0xBE, value);
}

fn write_cram(vdp: &mut SmsVdp, address: u16, value: u8) {
    select(vdp, address, 3);
    vdp.write_port(0xBE, value);
}

fn write_register(vdp: &mut SmsVdp, register: u8, value: u8) {
    vdp.write_port(0xBF, value);
    vdp.write_port(0xBF, 0x80 | register);
}

fn frame_pixel(frame: &[u8], x: usize, y: usize) -> [u8; 3] {
    let offset = (y * 256 + x) * 3;
    [frame[offset], frame[offset + 1], frame[offset + 2]]
}

fn configure_sms1_background(vdp: &mut SmsVdp, r2: u8, r3: u8, r4: u8) {
    for (register, value) in [(0, 0x04), (1, 0x40), (2, r2), (3, r3), (4, r4), (5, 0x7F)] {
        write_register(vdp, register, value);
    }
    write_vram(vdp, 0x3F00, 0xD0);
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
fn cram_write_ignores_all_address_bits_above_the_five_bit_index() {
    let mut vdp = new_sms1();
    let cases = [
        (0x0020, 0xFF),
        (0x0040, 0xC1),
        (0x0100, 0x7E),
        (0x3FE0, 0xAA),
    ];

    for (address, value) in cases {
        select(&mut vdp, address, 3);
        vdp.write_port(0xBE, value);

        assert_eq!(
            vdp.cram()[0],
            value & 0x3F,
            "CRAM address {address:#06X} must alias index zero"
        );
    }

    assert_eq!(vdp.vram()[0], 0x00, "CRAM writes must not modify VRAM");
}

#[test]
fn sms1_profile_rejects_sms2_extended_height_modes_through_public_api() {
    let mut vdp = new_sms1();

    for mode_bits in [0x08, 0x10] {
        vdp.write_port(0xBF, 0x40 | mode_bits);
        vdp.write_port(0xBF, 0x81);

        assert_eq!(vdp.raster_position().visible_height, 192);
    }
}

#[test]
fn mode4_background_pixels_are_published_on_vblank_through_public_ports() {
    let mut vdp = new_sms1();

    select(&mut vdp, 0x0000, 1);
    vdp.write_port(0xBE, 0x80);
    select(&mut vdp, 0x3800, 1);
    vdp.write_port(0xBE, 0x00);
    vdp.write_port(0xBE, 0x00);
    select(&mut vdp, 0x3F00, 1);
    vdp.write_port(0xBE, 0xD0);
    select(&mut vdp, 0x0001, 3);
    vdp.write_port(0xBE, 0x03);

    for (register, value) in [
        (0, 0x04),
        (1, 0x40),
        (2, 0x0E),
        (3, 0xFF),
        (4, 0x07),
        (5, 0x7F),
    ] {
        vdp.write_port(0xBF, value);
        vdp.write_port(0xBF, 0x80 | register);
    }

    // The frame is published at HPos 24 of the first VBlank line (T16).
    // 228 T-states/line remains the current deterministic integration model.
    vdp.advance(228 * 192 + 16);

    let frame = vdp.framebuffer();
    assert_eq!(frame.len(), 256 * 192 * 3);
    assert_eq!(&frame[0..3], &[238, 0, 0]);
    assert_eq!(&frame[3..6], &[0, 0, 0]);
}

#[test]
fn sms1_r2_mask_bit_controls_the_high_name_table_y_address_bit() {
    let mut masked = new_sms1();
    configure_sms1_background(&mut masked, 0x0E, 0xFF, 0x07);
    write_vram(&mut masked, 0x3800, 0);
    write_vram(&mut masked, 0x3801, 0);
    write_vram(&mut masked, 0x3C00, 2);
    write_vram(&mut masked, 0x3C01, 0);
    write_vram(&mut masked, 0x0040, 0x80);
    write_cram(&mut masked, 1, 0x03);
    masked.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(masked.framebuffer(), 0, 128), [0, 0, 0]);

    let mut unmasked = new_sms1();
    configure_sms1_background(&mut unmasked, 0x0F, 0xFF, 0x07);
    write_vram(&mut unmasked, 0x3800, 0);
    write_vram(&mut unmasked, 0x3801, 0);
    write_vram(&mut unmasked, 0x3C00, 2);
    write_vram(&mut unmasked, 0x3C01, 0);
    write_vram(&mut unmasked, 0x0040, 0x80);
    write_cram(&mut unmasked, 1, 0x03);
    unmasked.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(unmasked.framebuffer(), 0, 128), [238, 0, 0]);
}

#[test]
fn sms1_r3_mask_gates_background_plane_zero_and_one_pattern_addresses() {
    let mut masked = new_sms1();
    configure_sms1_background(&mut masked, 0x0F, 0x00, 0x07);
    write_vram(&mut masked, 0x3800, 2);
    write_vram(&mut masked, 0x3801, 0);
    write_vram(&mut masked, 0x0040, 0x80);
    write_cram(&mut masked, 1, 0x03);
    masked.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(masked.framebuffer(), 0, 0), [0, 0, 0]);

    let mut unmasked = new_sms1();
    configure_sms1_background(&mut unmasked, 0x0F, 0x01, 0x07);
    write_vram(&mut unmasked, 0x3800, 2);
    write_vram(&mut unmasked, 0x3801, 0);
    write_vram(&mut unmasked, 0x0040, 0x80);
    write_cram(&mut unmasked, 1, 0x03);
    unmasked.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(unmasked.framebuffer(), 0, 0), [238, 0, 0]);
}

#[test]
fn sms1_r4_mask_gates_background_plane_two_and_three_pattern_addresses() {
    let mut masked = new_sms1();
    configure_sms1_background(&mut masked, 0x0F, 0xFF, 0x00);
    write_vram(&mut masked, 0x3800, 64);
    write_vram(&mut masked, 0x3801, 0);
    write_vram(&mut masked, 0x0802, 0x80);
    write_cram(&mut masked, 4, 0x03);
    masked.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(masked.framebuffer(), 0, 0), [0, 0, 0]);

    let mut unmasked = new_sms1();
    configure_sms1_background(&mut unmasked, 0x0F, 0xFF, 0x01);
    write_vram(&mut unmasked, 0x3800, 64);
    write_vram(&mut unmasked, 0x3801, 0);
    write_vram(&mut unmasked, 0x0802, 0x80);
    write_cram(&mut unmasked, 4, 0x03);
    unmasked.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(unmasked.framebuffer(), 0, 0), [238, 0, 0]);
}

#[test]
fn sms2_ignores_sms1_r2_r3_r4_background_address_masks() {
    let profile = SmsVdpProfile::new(
        SmsVdpRevision::Sega3155246,
        VideoStandard::Ntsc,
        SpriteLimitMode::HardwareLimit,
    );
    let mut vdp = SmsVdp::new_with_profile(profile);
    for (register, value) in [
        (0, 0x04),
        (1, 0x40),
        (2, 0x0E),
        (3, 0x00),
        (4, 0x00),
        (5, 0x7F),
    ] {
        write_register(&mut vdp, register, value);
    }
    write_vram(&mut vdp, 0x3F00, 0xD0);
    write_vram(&mut vdp, 0x3C00, 2);
    write_vram(&mut vdp, 0x3C01, 0);
    write_vram(&mut vdp, 0x3C02, 64);
    write_vram(&mut vdp, 0x3C03, 0);
    write_vram(&mut vdp, 0x0040, 0x80);
    write_vram(&mut vdp, 0x0802, 0x80);
    write_cram(&mut vdp, 1, 0x03);
    write_cram(&mut vdp, 4, 0x03);

    vdp.advance(228 * 192 + 16);
    assert_eq!(frame_pixel(vdp.framebuffer(), 0, 128), [255, 0, 0]);
    assert_eq!(frame_pixel(vdp.framebuffer(), 8, 128), [255, 0, 0]);
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

    // VBlank status is asserted at HPos 24 of the first VBlank line (T16).
    vdp.advance(228 * 192 + 16);
    assert_ne!(vdp.status() & 0x80, 0);

    assert_ne!(vdp.read_port(0x00BF) & 0x80, 0);
    assert_eq!(vdp.read_port(0x01BF) & 0x80, 0);
}
