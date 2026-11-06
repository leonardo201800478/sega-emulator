//! SMS VDP (315-5124/315-5246 family), implemented independently from SG-1000.
//! Deterministic scanline model: Mode 4, VRAM/CRAM, scroll, sprites, IRQ, light-gun
//! click injection and standard-monitor SegaScope output. Not dot-cycle exact.

use super::profile::{SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard};

pub const SMS_VDP_WIDTH: usize = 256;
pub const SMS_VDP_MAX_HEIGHT: usize = 240;
const VRAM_SIZE: usize = 0x4000;
const CRAM_SIZE: usize = 0x20;
const LINE_TSTATES: u64 = 228;
const SMS_HCOUNTER_POSITIONS_PER_LINE: u64 = 342;
const SMS_R8_LATCH_HPOS: u16 = 21;
const STRIDE: usize = SMS_VDP_WIDTH * 3;
const FRAME_SIZE: usize = STRIDE * SMS_VDP_MAX_HEIGHT;
const VBLANK: u8 = 0x80;
const OVERFLOW: u8 = 0x40;
const COLLISION: u8 = 0x20;
const TMS_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 0],
    [33, 200, 66],
    [94, 220, 120],
    [84, 85, 237],
    [125, 118, 252],
    [212, 82, 77],
    [66, 235, 245],
    [252, 85, 84],
    [255, 121, 120],
    [212, 193, 84],
    [230, 206, 128],
    [33, 176, 59],
    [201, 91, 186],
    [204, 204, 204],
    [255, 255, 255],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmsVdpEye {
    #[default]
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StereoPresentation {
    #[default]
    Alternating,
    AnaglyphRedCyan,
    SideBySide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsRasterPosition {
    pub line: u16,
    pub tstates_into_line: u16,
    pub visible_height: u16,
    pub total_lines: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsVdp {
    vram: Vec<u8>,
    cram: [u8; CRAM_SIZE],
    registers: [u8; 16],
    latched_scroll_x: u8,
    latched_scroll_y: u8,
    pending_scroll_y: Option<u8>,
    status: u8,
    address: u16,
    read_buffer: u8,
    control_first: Option<u8>,
    cram_write: bool,
    scanline: u16,
    tstates_into_line: u16,
    frame_number: u64,
    framebuffer: Vec<u8>,
    working_framebuffer: Vec<u8>,
    frame_ready: bool,
    profile: SmsVdpProfile,
    line_counter: u8,
    line_irq_pending: bool,
    light_x: u16,
    light_y: u16,
    light_trigger: bool,
    light_pulse: bool,
    h_counter_latch: u8,
    stereo_eye: SmsVdpEye,
    stereo_presentation: StereoPresentation,
    left_frame: Vec<u8>,
    right_frame: Vec<u8>,
}

impl SmsVdp {
    /// Preserve the Legacy SMS2 constructor and its existing behavior.
    pub fn new(video_standard: VideoStandard, sprite_limit: SpriteLimitMode) -> Self {
        Self::new_with_profile(SmsVdpProfile::new(
            SmsVdpRevision::Sega3155246,
            video_standard,
            sprite_limit,
        ))
    }

    /// Construct the VDP with an explicit ASIC and regional profile.
    pub fn new_with_profile(profile: SmsVdpProfile) -> Self {
        Self {
            vram: vec![0; VRAM_SIZE],
            cram: [0; CRAM_SIZE],
            registers: [0; 16],
            latched_scroll_x: 0,
            latched_scroll_y: 0,
            pending_scroll_y: None,
            status: 0,
            address: 0,
            read_buffer: 0,
            control_first: None,
            cram_write: false,
            scanline: 0,
            tstates_into_line: 0,
            frame_number: 0,
            framebuffer: vec![0; FRAME_SIZE],
            working_framebuffer: vec![0; FRAME_SIZE],
            frame_ready: false,
            profile,
            line_counter: 0,
            line_irq_pending: false,
            light_x: 0,
            light_y: 0,
            light_trigger: false,
            light_pulse: false,
            h_counter_latch: 0,
            stereo_eye: SmsVdpEye::Right,
            stereo_presentation: StereoPresentation::Alternating,
            left_frame: vec![0; FRAME_SIZE],
            right_frame: vec![0; FRAME_SIZE],
        }
    }

    /// Return the immutable hardware profile currently selected.
    pub const fn profile(&self) -> SmsVdpProfile {
        self.profile
    }

    pub fn video_standard(&self) -> VideoStandard {
        self.profile.video_standard
    }
    pub fn system_clock_hz(&self) -> u64 {
        match self.profile.video_standard {
            VideoStandard::Pal => 3_546_895,
            VideoStandard::Ntsc | VideoStandard::PalM | VideoStandard::Unknown => 3_579_545,
        }
    }
    pub fn total_lines(&self) -> u16 {
        match self.profile.video_standard {
            VideoStandard::Pal => 313,
            _ => 262,
        }
    }
    pub fn visible_height(&self) -> u16 {
        let requested_height = if self.registers[1] & 0x08 != 0 {
            240
        } else if self.registers[1] & 0x10 != 0 {
            224
        } else {
            192
        };

        if self
            .profile
            .capabilities()
            .active_height_modes
            .contains(&requested_height)
        {
            requested_height
        } else {
            192
        }
    }
    pub fn raster_position(&self) -> SmsRasterPosition {
        SmsRasterPosition {
            line: self.scanline,
            tstates_into_line: self.tstates_into_line,
            visible_height: self.visible_height(),
            total_lines: self.total_lines(),
        }
    }
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }
    pub fn vram(&self) -> &[u8] {
        &self.vram
    }
    pub fn cram(&self) -> &[u8; CRAM_SIZE] {
        &self.cram
    }
    pub fn registers(&self) -> &[u8; 16] {
        &self.registers
    }
    pub fn status(&self) -> u8 {
        self.status
    }
    pub fn address(&self) -> u16 {
        self.address
    }
    pub fn scanline(&self) -> u16 {
        self.scanline
    }
    pub fn cycles_into_line(&self) -> u16 {
        self.tstates_into_line
    }
    pub fn sprite_limit_mode(&self) -> SpriteLimitMode {
        self.profile.sprite_limit
    }
    pub fn set_sprite_limit_mode(&mut self, mode: SpriteLimitMode) {
        self.profile.sprite_limit = mode;
    }
    pub fn set_stereo_eye(&mut self, eye: SmsVdpEye) {
        self.stereo_eye = eye;
    }
    pub fn set_stereo_presentation(&mut self, mode: StereoPresentation) {
        self.stereo_presentation = mode;
    }
    pub fn light_pulse_active(&self) -> bool {
        self.light_pulse
    }

    /// The host supplies logical SMS coordinates. A click creates a synthetic
    /// raster event; it does not claim to simulate the physical photodiode.
    pub fn set_light_phaser_input(&mut self, x: u16, y: u16, trigger_pressed: bool) {
        self.light_x = x.min(255);
        self.light_y = y.min(239);
        self.light_trigger = trigger_pressed;
        if !trigger_pressed {
            self.light_pulse = false;
        }
    }

    pub fn interrupt_pending(&self) -> bool {
        (self.status & VBLANK != 0 && self.registers[1] & 0x20 != 0)
            || (self.line_irq_pending && self.registers[0] & 0x10 != 0)
    }
    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer[..STRIDE * usize::from(self.visible_height())]
    }
    pub fn consume_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    pub fn presented_framebuffer(&self) -> (usize, usize, Vec<u8>) {
        let height = usize::from(self.visible_height());
        let row_bytes = SMS_VDP_WIDTH * 3;
        let size = row_bytes * height;
        match self.stereo_presentation {
            StereoPresentation::Alternating => (SMS_VDP_WIDTH, height, self.framebuffer().to_vec()),
            StereoPresentation::AnaglyphRedCyan => {
                let mut out = vec![0; size];
                for p in 0..SMS_VDP_WIDTH * height {
                    let i = p * 3;
                    out[i] = self.left_frame[i];
                    out[i + 1] = self.right_frame[i + 1];
                    out[i + 2] = self.right_frame[i + 2];
                }
                (SMS_VDP_WIDTH, height, out)
            }
            StereoPresentation::SideBySide => {
                let mut out = vec![0; size * 2];
                for y in 0..height {
                    let src = y * row_bytes;
                    let dst = y * row_bytes * 2;
                    out[dst..dst + row_bytes]
                        .copy_from_slice(&self.left_frame[src..src + row_bytes]);
                    out[dst + row_bytes..dst + row_bytes * 2]
                        .copy_from_slice(&self.right_frame[src..src + row_bytes]);
                }
                (SMS_VDP_WIDTH * 2, height, out)
            }
        }
    }

    pub fn reset(&mut self, clear_vram: bool) {
        self.cram.fill(0);
        self.registers.fill(0);
        self.latched_scroll_x = 0;
        self.latched_scroll_y = 0;
        self.pending_scroll_y = None;
        self.status = 0;
        self.address = 0;
        self.read_buffer = 0;
        self.control_first = None;
        self.cram_write = false;
        self.scanline = 0;
        self.tstates_into_line = 0;
        self.frame_number = 0;
        self.frame_ready = false;
        self.line_counter = 0;
        self.line_irq_pending = false;
        self.light_pulse = false;
        self.h_counter_latch = 0;
        self.framebuffer.fill(0);
        self.working_framebuffer.fill(0);
        self.left_frame.fill(0);
        self.right_frame.fill(0);
        if clear_vram {
            self.vram.fill(0);
        }
    }

    /// SMS VDP data ($BE) and control/status ($BF) ports.
    pub fn read_port(&mut self, port: u16) -> u8 {
        match (port as u8) & 0xC1 {
            0x80 => {
                let value = self.read_buffer;
                self.read_buffer = self.vram[usize::from(self.address) & 0x3FFF];
                self.address = self.address.wrapping_add(1) & 0x3FFF;
                self.control_first = None;
                self.cram_write = false;
                value
            }
            0x81 => {
                let value = self.status;
                self.status = 0;
                self.line_irq_pending = false;
                self.control_first = None;
                value
            }
            _ => 0xFF,
        }
    }
    pub fn write_port(&mut self, port: u16, value: u8) {
        match (port as u8) & 0xC1 {
            0x80 => self.write_data(value),
            0x81 => self.write_control(value),
            _ => {}
        }
    }

    pub fn read_v_counter(&self) -> u8 {
        let line = self.scanline;
        match (self.profile.video_standard, self.visible_height()) {
            (VideoStandard::Ntsc | VideoStandard::PalM | VideoStandard::Unknown, 192)
                if line >= 219 =>
            {
                0xD5_u8.wrapping_add((line - 219) as u8)
            }
            (VideoStandard::Ntsc | VideoStandard::PalM | VideoStandard::Unknown, 224)
                if line >= 235 =>
            {
                0xE5_u8.wrapping_add((line - 235) as u8)
            }
            (VideoStandard::Pal, 192) if line >= 243 => 0xBA_u8.wrapping_add((line - 243) as u8),
            (VideoStandard::Pal, 224) if line >= 259 => 0xCA_u8.wrapping_add((line - 259) as u8),
            (VideoStandard::Pal, 240) if line >= 267 => 0xD2_u8.wrapping_add((line - 267) as u8),
            _ => line as u8,
        }
    }
    /// Return the last H-counter value captured by a TH rising edge or Light Phaser hit.
    pub fn read_h_counter(&self) -> u8 {
        self.h_counter_latch
    }

    /// Capture the current SMS H-counter value at a TH 0-to-1 transition.
    ///
    /// The conversion follows the 315-5124 timing model: 342 horizontal
    /// positions per 228 Z80 T-states. The counter samples the previous HPos,
    /// wraps at the line boundary, subtracts 46, and divides by two.
    pub fn latch_h_counter(&mut self) {
        let hpos = (u64::from(self.tstates_into_line) * 3 / 2)
            % SMS_HCOUNTER_POSITIONS_PER_LINE;
        let previous_hpos = if hpos == 0 {
            SMS_HCOUNTER_POSITIONS_PER_LINE - 1
        } else {
            hpos - 1
        };
        let hclock = previous_hpos as i64 - 46;
        self.h_counter_latch = (hclock >> 1) as u8;
    }

    fn current_hpos(&self) -> u16 {
        ((u64::from(self.tstates_into_line) * 3) / 2) as u16
    }

    pub fn advance(&mut self, tstates: u64) {
        let mut remaining = tstates;
        while remaining > 0 {
            let step = remaining.min(LINE_TSTATES - u64::from(self.tstates_into_line));
            self.tstates_into_line += step as u16;
            remaining -= step;
            if u64::from(self.tstates_into_line) == LINE_TSTATES {
                self.tstates_into_line = 0;
                self.complete_scanline();
            }
        }
    }

    fn complete_scanline(&mut self) {
        let line = self.scanline;
        self.light_pulse = false;
        let height = self.visible_height();
        if line < height {
            let mut render_registers = self.registers;
            render_registers[8] = self.latched_scroll_x;
            render_registers[9] = self.latched_scroll_y;
            render_scanline_for_revision(
                &self.vram,
                &self.cram,
                &render_registers,
                &mut self.status,
                &mut self.working_framebuffer,
                usize::from(line),
                self.profile.sprite_limit == SpriteLimitMode::HardwareLimit,
                self.profile.revision,
            );
            if self.light_trigger && self.light_y == line {
                self.light_pulse = true;
                self.h_counter_latch = 0x08 + (self.light_x / 2) as u8;
            }
            if self.line_counter == 0 {
                self.line_irq_pending = true;
                self.line_counter = self.registers[10];
            } else {
                self.line_counter = self.line_counter.wrapping_sub(1);
            }
        }
        self.scanline += 1;
        if self.scanline == height {
            if let Some(scroll_y) = self.pending_scroll_y.take() {
                self.latched_scroll_y = scroll_y;
            }
            self.status |= VBLANK;
            self.frame_ready = true;
            self.framebuffer.clone_from(&self.working_framebuffer);
            match self.stereo_eye {
                SmsVdpEye::Left => self.left_frame.clone_from(&self.working_framebuffer),
                SmsVdpEye::Right => self.right_frame.clone_from(&self.working_framebuffer),
            }
        }
        if self.scanline >= self.total_lines() {
            self.scanline = 0;
            self.frame_number = self.frame_number.wrapping_add(1);
            self.working_framebuffer.fill(0);
            self.line_counter = self.registers[10];
        }
        // R8 is copied for the next scanline. Writes during the current line
        // may replace this latch only through HPos 21 (inclusive).
        self.latched_scroll_x = self.registers[8];
    }

    fn write_data(&mut self, value: u8) {
        self.control_first = None;
        if self.cram_write {
            self.cram[usize::from(self.address & 0x1F)] = value & 0x3F;
            self.read_buffer = value;
            self.address = self.address.wrapping_add(1) & 0x3FFF;
        } else {
            let address = usize::from(self.address) & 0x3FFF;
            self.vram[address] = value;
            self.read_buffer = value;
            self.address = self.address.wrapping_add(1) & 0x3FFF;
        }
    }
    fn write_control(&mut self, value: u8) {
        let Some(first) = self.control_first.take() else {
            self.control_first = Some(value);
            return;
        };
        if value & 0xC0 == 0x80 {
            self.address = ((u16::from(value & 0x3F) << 8) | u16::from(first)) & 0x3FFF;
            let register = usize::from(value & 0x0F);
            if register <= 10 {
                self.registers[register] = first;
                if register == 8
                    && (self.scanline >= self.visible_height()
                        || self.current_hpos() <= SMS_R8_LATCH_HPOS)
                {
                    self.latched_scroll_x = first;
                }
                if register == 9 {
                    if self.scanline < self.visible_height() {
                        self.pending_scroll_y = Some(first);
                    } else {
                        self.latched_scroll_y = first;
                        self.pending_scroll_y = None;
                    }
                }
            }
            self.cram_write = false;
            return;
        }
        self.address = ((u16::from(value & 0x3F) << 8) | u16::from(first)) & 0x3FFF;
        match value & 0xC0 {
            0x00 => {
                self.cram_write = false;
                self.read_buffer = self.vram[usize::from(self.address)];
                self.address = self.address.wrapping_add(1) & 0x3FFF;
            }
            0x40 => self.cram_write = false,
            0xC0 => {
                self.cram_write = true;
            }
            _ => self.cram_write = false,
        }
    }
}
impl Default for SmsVdp {
    fn default() -> Self {
        Self::new(VideoStandard::Ntsc, SpriteLimitMode::HardwareLimit)
    }
}

fn palette_rgb(cram: &[u8; CRAM_SIZE], index: u8) -> [u8; 3] {
    let v = cram[usize::from(index & 0x1F)] & 0x3F;
    [(v & 3) * 85, ((v >> 2) & 3) * 85, ((v >> 4) & 3) * 85]
}
fn put_pixel(frame: &mut [u8], x: usize, y: usize, rgb: [u8; 3]) {
    if x < SMS_VDP_WIDTH && y < SMS_VDP_MAX_HEIGHT {
        let i = (y * SMS_VDP_WIDTH + x) * 3;
        frame[i..i + 3].copy_from_slice(&rgb);
    }
}

fn tms_rgb(color: u8) -> [u8; 3] {
    TMS_PALETTE[usize::from(color & 0x0F)]
}
fn tms_pixel(frame: &mut [u8], x: usize, y: usize, color: u8) {
    if x < SMS_VDP_WIDTH && y < SMS_VDP_MAX_HEIGHT {
        let i = (y * SMS_VDP_WIDTH + x) * 3;
        frame[i..i + 3].copy_from_slice(&tms_rgb(color));
    }
}
fn tms_span(frame: &mut [u8], x: usize, y: usize, bits: u8, fg: u8, bg: u8, width: usize) {
    for bit in 0..width {
        tms_pixel(
            frame,
            x + bit,
            y,
            if bits & (0x80 >> bit) != 0 { fg } else { bg },
        );
    }
}
fn tms_quarter(frame: &mut [u8], x: usize, y: usize, color: u8) {
    for dx in 0..4 {
        tms_pixel(frame, x + dx, y, color);
    }
}

/// SMS1 R5 bit 0 gates address bit 7 across the sprite attribute table.
fn sms1_sprite_attribute_address(sat_base: usize, offset: usize, register5: u8) -> usize {
    let address = (sat_base + offset) & 0x3FFF;
    if register5 & 0x01 == 0 {
        address & !0x0080
    } else {
        address
    }
}

/// SMS1 R6 bits 1-0 gate sprite pattern address bits 12-11.
fn sms1_sprite_pattern_address(address: usize, register6: u8) -> usize {
    let mask = usize::from(register6 & 0x03) << 11;
    (address & !0x1800) | (address & mask)
}

fn render_legacy_scanline(
    vram: &[u8],
    regs: &[u8; 16],
    status: &mut u8,
    frame: &mut [u8],
    y: usize,
    unlocked: bool,
) {
    let backdrop = regs[7] & 0x0F;
    for x in 0..SMS_VDP_WIDTH {
        tms_pixel(frame, x, y, backdrop);
    }
    if regs[1] & 0x40 == 0 {
        return;
    }
    let mode = ((regs[1] >> 4) & 1) | ((regs[1] >> 2) & 2) | (((regs[0] >> 1) & 1) << 2);
    let names = usize::from(regs[2] & 0x0F) << 10;
    let patterns = usize::from(regs[4] & 0x07) << 11;

    if mode == 1 {
        let fg = if regs[7] >> 4 != 0 {
            regs[7] >> 4
        } else {
            backdrop
        };
        for col in 0..40 {
            let ch = vram[(names + (y / 8) * 40 + col) & 0x3FFF];
            let bits = vram[(patterns + usize::from(ch) * 8 + (y & 7)) & 0x3FFF];
            tms_span(frame, 8 + col * 6, y, bits, fg, backdrop, 6);
        }
        return;
    }
    if mode == 2 {
        for col in 0..32 {
            let ch = vram[(names + (y / 8) * 32 + col) & 0x3FFF];
            let bits = vram[(patterns + usize::from(ch) * 8 + ((y & 0x1C) >> 2)) & 0x3FFF];
            tms_quarter(
                frame,
                col * 8,
                y,
                if bits >> 4 != 0 { bits >> 4 } else { backdrop },
            );
            tms_quarter(
                frame,
                col * 8 + 4,
                y,
                if bits & 0x0F != 0 {
                    bits & 0x0F
                } else {
                    backdrop
                },
            );
        }
    } else if mode == 4 {
        let pat_base = usize::from(regs[4] & 0x04) << 11;
        let color_base = usize::from(regs[3] & 0x80) << 6;
        for col in 0..32 {
            let ch = vram[(names + (y / 8) * 32 + col) & 0x3FFF];
            let offset = ((y & 0xC0) << 5) + usize::from(ch) * 8 + (y & 7);
            let bits = vram[(pat_base + offset) & 0x3FFF];
            let colors = vram[(color_base + offset) & 0x3FFF];
            let fg = if colors >> 4 != 0 {
                colors >> 4
            } else {
                backdrop
            };
            let bg = if colors & 0x0F != 0 {
                colors & 0x0F
            } else {
                backdrop
            };
            tms_span(frame, col * 8, y, bits, fg, bg, 8);
        }
    } else {
        let color_base = (usize::from(regs[3]) << 6) & 0x3FC0;
        for col in 0..32 {
            let ch = vram[(names + (y / 8) * 32 + col) & 0x3FFF];
            let bits = vram[(patterns + usize::from(ch) * 8 + (y & 7)) & 0x3FFF];
            let colors = vram[(color_base + usize::from(ch) / 8) & 0x3FFF];
            let fg = if colors >> 4 != 0 {
                colors >> 4
            } else {
                backdrop
            };
            let bg = if colors & 0x0F != 0 {
                colors & 0x0F
            } else {
                backdrop
            };
            tms_span(frame, col * 8, y, bits, fg, bg, 8);
        }
    }
    render_legacy_sprites(vram, regs, status, frame, y, unlocked);
}

fn render_legacy_sprites(
    vram: &[u8],
    regs: &[u8; 16],
    status: &mut u8,
    frame: &mut [u8],
    y: usize,
    unlocked: bool,
) {
    let table = (usize::from(regs[5] & 0x7F) << 7) & 0x3F80;
    let pattern_base = usize::from(regs[6] & 0x07) << 11;
    let size = if regs[1] & 0x02 != 0 { 16 } else { 8 };
    let zoom = if regs[1] & 0x01 != 0 { 2 } else { 1 };
    let mut active: Vec<(usize, i32, usize, u8, usize)> = Vec::with_capacity(32);
    for index in 0..32 {
        let at = (table + index * 4) & 0x3FFF;
        let raw_y = vram[at];
        if raw_y == 0xD0 {
            break;
        }
        let mut sy = i32::from(raw_y.wrapping_add(1));
        if sy >= 0xE0 {
            sy -= 0x100;
        }
        if (y as i32) < sy || (y as i32) >= sy + (size * zoom) as i32 {
            continue;
        }
        if active.len() == 4 && *status & OVERFLOW == 0 {
            *status = (*status & !0x1F) | OVERFLOW | (index as u8 & 0x1F);
        }
        if active.len() >= 4 && !unlocked {
            break;
        }
        let mut sx = i32::from(vram[at + 1]);
        let attr = vram[at + 3];
        if attr & 0x80 != 0 {
            sx -= 32;
        }
        let pattern = usize::from(vram[at + 2]);
        let row = ((y as i32 - sy) / zoom as i32) as usize;
        active.push((index, sx, pattern, attr & 0x0F, row));
    }

    let mut occupied = [false; SMS_VDP_WIDTH];
    let mut drawn = [false; SMS_VDP_WIDTH];
    for (_index, sx, pattern, color, row) in active {
        if color == 0 {
            continue;
        }
        for source_x in 0..size {
            let tile = if size == 16 {
                (pattern & 0xFC) + (source_x / 8) * 2 + row / 8
            } else {
                pattern
            };
            let at = (pattern_base + tile * 8 + (row & 7)) & 0x3FFF;
            if vram[at] & (0x80 >> (source_x & 7)) == 0 {
                continue;
            }
            for dx in 0..zoom {
                let x = sx + (source_x * zoom + dx) as i32;
                if !(0..SMS_VDP_WIDTH as i32).contains(&x) {
                    continue;
                }
                let x = x as usize;
                if occupied[x] {
                    *status |= COLLISION;
                }
                occupied[x] = true;
                if !drawn[x] {
                    tms_pixel(frame, x, y, color);
                    drawn[x] = true;
                }
            }
        }
    }
}

#[cfg(test)]
fn render_scanline(
    vram: &[u8],
    cram: &[u8; CRAM_SIZE],
    regs: &[u8; 16],
    status: &mut u8,
    frame: &mut [u8],
    y: usize,
    hardware_sprite_limit: bool,
) {
    render_scanline_for_revision(
        vram,
        cram,
        regs,
        status,
        frame,
        y,
        hardware_sprite_limit,
        SmsVdpRevision::Sega3155246,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "explicit render inputs keep hardware state ownership at the VDP boundary"
)]
fn render_scanline_for_revision(
    vram: &[u8],
    cram: &[u8; CRAM_SIZE],
    regs: &[u8; 16],
    status: &mut u8,
    frame: &mut [u8],
    y: usize,
    hardware_sprite_limit: bool,
    revision: SmsVdpRevision,
) {
    if regs[0] & 0x04 == 0 {
        render_legacy_scanline(vram, regs, status, frame, y, !hardware_sprite_limit);
        return;
    }
    let backdrop = palette_rgb(cram, 16 + (regs[7] & 0x0F));
    for x in 0..SMS_VDP_WIDTH {
        put_pixel(frame, x, y, backdrop);
    }
    if regs[1] & 0x40 == 0 {
        return;
    }
    let height = if regs[1] & 0x08 != 0 {
        240
    } else if regs[1] & 0x10 != 0 {
        224
    } else {
        192
    };
    let name_base = if height == 192 {
        usize::from(regs[2] & 0x0E) << 10
    } else {
        (usize::from(regs[2] & 0x0C) << 10) | 0x0700
    };
    // In SMS Mode 4, background tile indices address the 512-tile pattern
    // space directly. Register 4's base bits apply to legacy TMS9918 modes,
    // not this renderer.
    let pattern_base = 0;
    let mut bg_color = [0_u8; SMS_VDP_WIDTH];
    let mut bg_priority = [false; SMS_VDP_WIDTH];
    let backdrop = 16_u8 + (regs[7] & 0x0F);
    for x in 0..SMS_VDP_WIDTH {
        if regs[0] & 0x20 != 0 && x < 8 {
            bg_color[x] = backdrop;
            put_pixel(frame, x, y, palette_rgb(cram, backdrop));
            continue;
        }
        let hs = if regs[0] & 0x40 != 0 && y < 16 {
            0
        } else {
            usize::from(regs[8])
        };
        // The SMS VDP blanks the leading fine-scroll pixels with the
        // backdrop instead of wrapping the last name-table column into view.
        if x < (hs & 7) {
            bg_color[x] = backdrop;
            put_pixel(frame, x, y, palette_rgb(cram, backdrop));
            continue;
        }
        let vs = if regs[0] & 0x80 != 0 && x >= 192 {
            0
        } else {
            usize::from(regs[9])
        };
        let sx = (x + 0x100 - hs) & 0xFF;
        // The standard 192-line mode scrolls through a 224-line name table;
        // extended 224/240-line modes use the full 256-line address range.
        let raw_sy = y + vs;
        let sy = if height == 192 && raw_sy >= 224 {
            raw_sy - 224
        } else {
            raw_sy & 0xFF
        };
        let entry = (name_base + (sy / 8) * 64 + (sx / 8) * 2) & 0x3FFF;
        let name = u16::from(vram[entry]) | (u16::from(vram[(entry + 1) & 0x3FFF]) << 8);
        let tile = usize::from(name & 0x01FF);
        let mut px = sx & 7;
        let mut py = sy & 7;
        if name & (1 << 9) != 0 {
            px = 7 - px;
        }
        if name & (1 << 10) != 0 {
            py = 7 - py;
        }
        let p = (pattern_base + tile * 32 + py * 4) & 0x3FFF;
        let mut color = 0_u8;
        for plane in 0..4 {
            color |= ((vram[(p + plane) & 0x3FFF] >> (7 - px)) & 1) << plane;
        }
        let palette = if name & (1 << 11) != 0 { 16 } else { 0 };
        bg_color[x] = palette + color;
        bg_priority[x] = name & (1 << 12) != 0 && color != 0;
        put_pixel(frame, x, y, palette_rgb(cram, bg_color[x]));
    }

    let sat = (usize::from(regs[5] & 0x7E) << 7) & 0x3F00;
    let sprite_pattern = usize::from(regs[6] & 0x04) << 11;
    let sprite_height = if regs[1] & 0x02 != 0 { 16 } else { 8 };
    let zoom = if regs[1] & 0x01 != 0 { 2 } else { 1 };
    let mut active: Vec<(usize, i32, usize, usize)> = Vec::with_capacity(64);
    for index in 0..64 {
        let y_address = if revision == SmsVdpRevision::Sega3155124 {
            sms1_sprite_attribute_address(sat, index, regs[5])
        } else {
            (sat + index) & 0x3FFF
        };
        let raw_y = vram[y_address];
        if raw_y == 0xD0 && height == 192 {
            break;
        }
        let sy = if raw_y >= 0xF0 {
            i32::from(raw_y) - 255
        } else {
            i32::from(raw_y) + 1
        };
        if (y as i32) < sy || (y as i32) >= sy + (sprite_height * zoom) {
            continue;
        }
        let x_offset = 0x80 + index * 2;
        let x_address = if revision == SmsVdpRevision::Sega3155124 {
            sms1_sprite_attribute_address(sat, x_offset, regs[5])
        } else {
            (sat + x_offset) & 0x3FFF
        };
        let tile_address = if revision == SmsVdpRevision::Sega3155124 {
            sms1_sprite_attribute_address(sat, x_offset + 1, regs[5])
        } else {
            (sat + x_offset + 1) & 0x3FFF
        };
        let mut sx = i32::from(vram[x_address]);
        if regs[0] & 0x08 != 0 {
            sx -= 8;
        }
        let mut tile = usize::from(vram[tile_address]);
        if sprite_height == 16 {
            tile &= !1;
        }
        let row = ((y as i32 - sy) / zoom) as usize;
        active.push((index, sx, tile, row));
    }
    if active.len() > 8 && *status & OVERFLOW == 0 {
        *status = (*status & !0x1F) | OVERFLOW | ((active[8].0 as u8) & 0x1F);
    }
    let count = if hardware_sprite_limit {
        active.len().min(8)
    } else {
        active.len()
    };
    let mut occupied = [false; SMS_VDP_WIDTH];
    let mut sprite_rgb: [Option<[u8; 3]>; SMS_VDP_WIDTH] = [None; SMS_VDP_WIDTH];
    // Lower sprite index wins visually; all drawn opaque sprite pixels affect collision.
    for (rank, &(_index, sx, tile, row)) in active.iter().take(count).enumerate() {
        let tile_row = if sprite_height == 16 && row >= 8 {
            tile + 1
        } else {
            tile
        };
        let py = if sprite_height == 16 { row & 7 } else { row };
        let raw_pattern = (sprite_pattern + tile_row * 32 + py * 4) & 0x3FFF;
        let p = if revision == SmsVdpRevision::Sega3155124 {
            sms1_sprite_pattern_address(raw_pattern, regs[6])
        } else {
            raw_pattern
        };
        let width_scale = if revision == SmsVdpRevision::Sega3155124 && zoom == 2 {
            if rank < count.saturating_sub(4) { 2 } else { 1 }
        } else {
            zoom
        };
        for px in 0..8 {
            let mut color = 0_u8;
            for plane in 0..4 {
                color |= ((vram[(p + plane) & 0x3FFF] >> (7 - px)) & 1) << plane;
            }
            if color == 0 {
                continue;
            }
            for zx in 0..width_scale {
                let x = sx + (px * width_scale + zx);
                if !(0..SMS_VDP_WIDTH as i32).contains(&x) {
                    continue;
                }
                let x = x as usize;
                if regs[0] & 0x20 != 0 && x < 8 {
                    continue;
                }
                if occupied[x] {
                    *status |= COLLISION;
                }
                occupied[x] = true;
                if sprite_rgb[x].is_none() {
                    sprite_rgb[x] = Some(palette_rgb(cram, 16 + color));
                }
            }
        }
    }
    for x in 0..SMS_VDP_WIDTH {
        if let Some(rgb) = sprite_rgb[x]
            && (!bg_priority[x] || bg_color[x] & 0x0F == 0)
        {
            put_pixel(frame, x, y, rgb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::profile::{SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, VideoStandard};
    use super::{
        COLLISION, FRAME_SIZE, LINE_TSTATES, OVERFLOW, SMS_VDP_WIDTH, SmsVdp, StereoPresentation,
        TMS_PALETTE, VBLANK, render_legacy_scanline, render_legacy_sprites, render_scanline,
        render_scanline_for_revision,
    };
    fn reg(v: &mut SmsVdp, i: u8, value: u8) {
        v.write_port(0xBF, value);
        v.write_port(0xBF, 0x80 | i);
    }
    fn addr(v: &mut SmsVdp, a: u16, mode: u8) {
        v.write_port(0xBF, a as u8);
        v.write_port(0xBF, ((a >> 8) as u8 & 0x3F) | mode);
    }

    fn set_sprite(v: &mut SmsVdp, index: usize, y: u8, x: u8, tile: u8) {
        v.vram[0x3F00 + index] = y;
        v.vram[0x3F80 + index * 2] = x;
        v.vram[0x3F81 + index * 2] = tile;
    }

    fn solid_sprite_tile(v: &mut SmsVdp, tile: u8, color: u8) {
        let base = usize::from(tile) * 32;
        for row in 0..8 {
            for plane in 0..4 {
                v.vram[base + row * 4 + plane] = if color & (1 << plane) != 0 { 0xFF } else { 0 };
            }
        }
    }
    #[test]
    fn owns_16k_vram_and_32_byte_cram() {
        let v = SmsVdp::default();
        assert_eq!(v.vram().len(), 0x4000);
        assert_eq!(v.cram().len(), 0x20);
    }
    #[test]
    fn control_register_vram_write_and_wrap() {
        let mut v = SmsVdp::default();
        reg(&mut v, 1, 0x40);
        assert_eq!(v.registers()[1], 0x40);
        addr(&mut v, 0x3FFF, 0x40);
        v.write_port(0xBE, 0xA5);
        v.write_port(0xBE, 0x5A);
        assert_eq!(v.vram()[0x3FFF], 0xA5);
        assert_eq!(v.vram()[0], 0x5A);
        assert_eq!(v.address(), 1);
    }
    #[test]
    fn cram_is_six_bit_and_separate_from_vram() {
        let mut v = SmsVdp::default();
        addr(&mut v, 3, 0xC0);
        v.write_port(0xBE, 0xFF);
        assert_eq!(v.cram()[3], 0x3F);
        assert_eq!(v.vram()[3], 0);
    }
    #[test]
    fn read_ahead_and_vblank_irq_clear_on_status_read() {
        let mut v = SmsVdp::default();
        addr(&mut v, 0x1234, 0x40);
        v.write_port(0xBE, 0x71);
        addr(&mut v, 0x1234, 0);
        assert_eq!(v.read_port(0xBE), 0x71);
        reg(&mut v, 1, 0x20);
        v.advance(228 * 192);
        assert!(v.interrupt_pending());
        assert_ne!(v.read_port(0xBF) & 0x80, 0);
        assert!(!v.interrupt_pending());
    }
    #[test]
    fn regional_clocks_and_raster_lengths_are_distinct() {
        let n = SmsVdp::new(VideoStandard::Ntsc, SpriteLimitMode::HardwareLimit);
        let pm = SmsVdp::new(VideoStandard::PalM, SpriteLimitMode::HardwareLimit);
        let p = SmsVdp::new(VideoStandard::Pal, SpriteLimitMode::HardwareLimit);
        assert_eq!((n.total_lines(), n.system_clock_hz()), (262, 3_579_545));
        assert_eq!((pm.total_lines(), pm.system_clock_hz()), (262, 3_579_545));
        assert_eq!((p.total_lines(), p.system_clock_hz()), (313, 3_546_895));
    }
    #[test]
    fn explicit_sms1_profile_keeps_only_192_line_mode() {
        let mut vdp = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());

        assert_eq!(vdp.profile().revision, SmsVdpRevision::Sega3155124);
        assert_eq!(vdp.visible_height(), 192);

        vdp.registers[1] = 0x08;
        assert_eq!(vdp.visible_height(), 192);

        vdp.registers[1] = 0x10;
        assert_eq!(vdp.visible_height(), 192);
    }

    #[test]
    fn legacy_constructor_preserves_sms2_profile_and_extended_modes() {
        let mut vdp = SmsVdp::new(VideoStandard::Ntsc, SpriteLimitMode::HardwareLimit);

        assert_eq!(vdp.profile().revision, SmsVdpRevision::Sega3155246);
        vdp.registers[1] = 0x10;
        assert_eq!(vdp.visible_height(), 224);
        vdp.registers[1] = 0x08;
        assert_eq!(vdp.visible_height(), 240);
    }

    #[test]
    fn extended_modes_expose_224_and_240_line_framebuffers() {
        let mut v = SmsVdp::default();
        reg(&mut v, 1, 0x10);
        assert_eq!(v.visible_height(), 224);
        reg(&mut v, 1, 0x08);
        assert_eq!(v.visible_height(), 240);
        assert_eq!(v.framebuffer().len(), 256 * 240 * 3);
    }
    #[test]
    fn unlocked_sprite_policy_is_configurable() {
        let mut v = SmsVdp::default();
        assert_eq!(v.sprite_limit_mode(), SpriteLimitMode::HardwareLimit);
        v.set_sprite_limit_mode(SpriteLimitMode::UnlockedCompatibility);
        assert_eq!(
            v.sprite_limit_mode(),
            SpriteLimitMode::UnlockedCompatibility
        );
    }
    #[test]
    fn mouse_click_is_qualified_by_raster_line() {
        let mut v = SmsVdp::default();
        v.set_light_phaser_input(120, 45, true);
        v.advance(228 * 46);
        assert!(v.light_pulse_active());
        assert_eq!(v.read_h_counter(), 0x08 + 60);
        v.set_light_phaser_input(120, 45, false);
        assert!(!v.light_pulse_active());
        assert_eq!(v.read_h_counter(), 0x08 + 60);
    }
    #[test]
    fn stereo_output_modes_have_expected_dimensions() {
        let mut v = SmsVdp::default();
        v.set_stereo_presentation(StereoPresentation::SideBySide);
        let (w, h, frame) = v.presented_framebuffer();
        assert_eq!((w, h, frame.len()), (512, 192, 512 * 192 * 3));
        v.set_stereo_presentation(StereoPresentation::AnaglyphRedCyan);
        assert_eq!(v.presented_framebuffer().0, SMS_VDP_WIDTH);
    }
    #[test]
    fn advance_is_chunking_invariant() {
        let mut a = SmsVdp::default();
        let mut b = SmsVdp::default();
        a.advance(228 * 262 * 3 + 17);
        b.advance(228 * 100);
        b.advance(228 * 162 + 17);
        b.advance(228 * 262 * 2);
        assert_eq!(a.raster_position(), b.raster_position());
        assert_eq!(a.frame_number(), b.frame_number());
    }
    #[test]
    fn line_counter_reloads_from_register_ten() {
        let mut v = SmsVdp::default();
        reg(&mut v, 0, 0x14);
        reg(&mut v, 10, 1);
        v.advance(228 * 2);
        assert!(v.interrupt_pending());
        let _ = v.read_port(0xBF);
        assert!(!v.interrupt_pending());
    }

    #[test]
    fn mid_irq_r10_write_changes_next_reload_not_active_countdown() {
        let mut v = SmsVdp::default();
        reg(&mut v, 0, 0x14);
        reg(&mut v, 10, 63);
        // Model the counter after frame-start initialization. The first
        // line IRQ occurs after the 64th visible scanline.
        v.line_counter = 63;
        v.advance(228 * 64);
        assert!(v.interrupt_pending());

        // The interrupt handler changes the reload value. The countdown
        // already reloaded at underflow and must remain at 63 for this period.
        let _ = v.read_port(0xBF);
        reg(&mut v, 10, 47);
        assert_eq!(v.line_counter, 63);

        v.advance(228 * 63);
        assert!(!v.interrupt_pending());
        v.advance(228);
        assert!(v.interrupt_pending());

        // The new reload value applies at the next underflow: 48 lines.
        let _ = v.read_port(0xBF);
        v.advance(228 * 47);
        assert!(!v.interrupt_pending());
        v.advance(228);
        assert!(v.interrupt_pending());
    }

    #[test]
    fn legacy_graphics_i_uses_the_tms_palette_and_pattern_color_table() {
        let mut v = SmsVdp::default();
        v.registers[1] = 0x40;
        v.registers[2] = 0x01;
        v.registers[3] = 0x01;
        v.registers[4] = 0x01;
        v.registers[7] = 0x01;
        v.vram[0x400] = 1;
        v.vram[0x808] = 0x80;
        v.vram[0x40] = 0x42;
        let mut status = 0x1F;
        let mut frame = vec![0; FRAME_SIZE];
        render_legacy_scanline(&v.vram, &v.registers, &mut status, &mut frame, 0, false);
        assert_eq!(&frame[0..3], &TMS_PALETTE[4]);
        assert_eq!(&frame[3..6], &TMS_PALETTE[2]);
    }

    #[test]
    fn sms1_r5_bit_zero_aliases_sprite_x_and_tile_addresses_into_y_table() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[17] = 0x03;
        v.vram[0x20] = 0x80;
        v.vram[0x3F00] = 0x80;
        v.vram[0x3F01] = 0xFF;
        v.vram[0x3F02] = 20;
        v.vram[0x3F03] = 1;
        v.vram[0x3F82] = 0;
        v.vram[0x3F83] = 0;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );
        assert_eq!(&frame[20 * 3..20 * 3 + 3], &[255, 0, 0]);

        v.registers[5] = 0x7F;
        frame.fill(0);
        status = 0;
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );
        assert_eq!(&frame[20 * 3..20 * 3 + 3], &[0, 0, 0]);
    }

    #[test]
    fn sms1_r6_bit_one_gates_sprite_pattern_address_bit_twelve() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.registers[6] = 0x05;
        v.cram[17] = 0x03;
        set_sprite(&mut v, 0, 0xFF, 10, 0x80);
        v.vram[0x2000] = 0x80;
        v.vram[0x3000] = 0;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_eq!(&frame[10 * 3..10 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms1_r6_bit_zero_gates_sprite_pattern_address_bit_eleven() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.registers[6] = 0x06;
        v.cram[17] = 0x03;
        set_sprite(&mut v, 0, 0xFF, 10, 0x40);
        v.vram[0x2000] = 0x80;
        v.vram[0x2800] = 0;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_eq!(&frame[10 * 3..10 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms1_first_sprite_uses_sprite_palette_entries() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.cram[17] = 0x03;
        set_sprite(&mut v, 0, 0xFF, 10, 1);
        v.vram[0x3F01] = 0xD0;
        solid_sprite_tile(&mut v, 1, 1);

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_eq!(&frame[10 * 3..10 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms1_hardware_sprite_buffer_selects_eight_and_overflows_on_ninth() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.cram[17] = 0x03;
        solid_sprite_tile(&mut v, 1, 1);

        for index in 0..9 {
            set_sprite(&mut v, index, 0xFF, (index * 28) as u8, 1);
        }
        v.vram[0x3F09] = 0xD0;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_ne!(status & OVERFLOW, 0);
        assert_eq!(status & 0x1F, 8);
        assert_eq!(&frame[196 * 3..196 * 3 + 3], &[255, 0, 0]);
        assert_eq!(&frame[224 * 3..224 * 3 + 3], &[0, 0, 0]);
    }

    #[test]
    fn sms1_tall_magnified_sprite_uses_even_tile_pair_and_vertical_duplication() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x43;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.cram[17] = 0x03;
        set_sprite(&mut v, 0, 0xFF, 10, 3);
        v.vram[0x3F01] = 0xD0;
        v.vram[0x40] = 0x80;
        v.vram[0x60] = 0x80;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        for y in [0usize, 1, 16, 17] {
            render_scanline_for_revision(
                &v.vram,
                &v.cram,
                &v.registers,
                &mut status,
                &mut frame,
                y,
                true,
                SmsVdpRevision::Sega3155124,
            );
            assert_eq!(
                &frame[(y * SMS_VDP_WIDTH + 10) * 3..(y * SMS_VDP_WIDTH + 10) * 3 + 3],
                &[255, 0, 0],
                "sprite row {y} must use the even tile pair and duplicate vertically"
            );
        }
    }

    #[test]
    fn sms1_horizontal_zoom_skips_last_four_selected_sprites() {
        for sprite_count in [2usize, 4, 6, 8] {
            let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
            v.registers[0] = 0x04;
            v.registers[1] = 0x41;
            v.registers[2] = 0x0E;
            v.registers[5] = 0x7F;
            v.cram[17] = 0x03;
            solid_sprite_tile(&mut v, 1, 1);

            for index in 0..sprite_count {
                set_sprite(&mut v, index, 0xFF, (index * 28) as u8, 1);
            }
            v.vram[0x3F00 + sprite_count] = 0xD0;

            let mut status = 0;
            let mut frame = vec![0; FRAME_SIZE];
            render_scanline_for_revision(
                &v.vram,
                &v.cram,
                &v.registers,
                &mut status,
                &mut frame,
                0,
                true,
                SmsVdpRevision::Sega3155124,
            );
            assert_eq!(status, 0, "unexpected status for {sprite_count} sprites");

            for index in 0..sprite_count {
                let x = index * 28;
                let doubled = index < sprite_count.saturating_sub(4);
                let width = if doubled { 16 } else { 8 };
                assert_eq!(
                    &frame[(x + width - 1) * 3..(x + width) * 3],
                    &[255, 0, 0],
                    "last pixel of sprite {index} at count {sprite_count}"
                );
                assert_eq!(
                    &frame[(x + width) * 3..(x + width + 1) * 3],
                    &[0, 0, 0],
                    "sprite {index} at count {sprite_count} exceeded its SMS1 width"
                );
            }
        }
    }

    #[test]
    fn sms1_opaque_sprite_overlap_sets_collision_and_preserves_earlier_sprite() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.cram[17] = 0x03;
        v.cram[18] = 0x0C;
        set_sprite(&mut v, 0, 0xFF, 20, 1);
        set_sprite(&mut v, 1, 0xFF, 20, 2);
        v.vram[0x3F02] = 0xD0;
        solid_sprite_tile(&mut v, 1, 1);
        solid_sprite_tile(&mut v, 2, 2);

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_ne!(status & COLLISION, 0);
        assert_eq!(&frame[20 * 3..20 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms1_transparent_sprite_overlap_does_not_set_collision() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        set_sprite(&mut v, 0, 0xFF, 20, 1);
        set_sprite(&mut v, 1, 0xFF, 20, 2);
        v.vram[0x3F02] = 0xD0;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_eq!(status & COLLISION, 0);
    }

    #[test]
    fn sms1_sprite_priority_respects_opaque_high_priority_background() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        v.cram[1] = 0x03;
        v.cram[18] = 0x0C;
        for column in 0..32 {
            v.vram[0x3800 + column * 2] = 0;
            v.vram[0x3801 + column * 2] = 0x10;
        }
        v.vram[0] = 0xFF;
        set_sprite(&mut v, 0, 0xFF, 20, 1);
        v.vram[0x3F01] = 0xD0;
        solid_sprite_tile(&mut v, 1, 2);

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_eq!(&frame[20 * 3..20 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms1_ninth_offscreen_sprite_sets_overflow() {
        let mut v = SmsVdp::new_with_profile(SmsVdpProfile::sms1_t0_ntsc());
        v.registers[0] = 0x0C;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7F;
        for index in 0..9 {
            set_sprite(&mut v, index, 0xFF, 0, 1);
        }
        v.vram[0x3F09] = 0xD0;

        let mut status = 0;
        let mut frame = vec![0; FRAME_SIZE];
        render_scanline_for_revision(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            0,
            true,
            SmsVdpRevision::Sega3155124,
        );

        assert_ne!(status & OVERFLOW, 0);
        assert_eq!(status & 0x1F, 8);
        assert!(frame.iter().all(|pixel| *pixel == 0));
    }

    #[test]
    fn sms_mode4_renders_cram_palette_entries_from_4bpp_tiles() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.cram[1] = 0x03;
        v.vram[0x3800] = 0;
        v.vram[0x3801] = 0;
        v.vram[0] = 0x80;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            0,
            true,
        );
        assert_eq!(&v.working_framebuffer[0..3], &[255, 0, 0]);
        assert_eq!(&v.working_framebuffer[3..6], &[0, 0, 0]);
    }

    #[test]
    fn sms_mode4_background_tile_index_is_not_offset_by_register_four() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[4] = 0x04;
        v.cram[1] = 0x03;
        v.vram[0x3800] = 1;
        v.vram[0x3801] = 0;
        v.vram[0x20] = 0x80;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            0,
            true,
        );
        assert_eq!(&v.working_framebuffer[0..3], &[255, 0, 0]);
    }

    #[test]
    fn sms_mode4_192_line_vertical_scroll_wraps_at_224_lines() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[9] = 224;
        v.cram[1] = 0x03;
        v.cram[2] = 0x0C;
        v.vram[0x3800] = 1;
        v.vram[0x3801] = 0;
        // If the renderer incorrectly wraps at 256, y=224 selects this row
        // from $3F00 instead of wrapping to the first name-table row.
        v.vram[0x3F00] = 2;
        v.vram[0x3F01] = 0;
        v.vram[0x20] = 0x80;
        v.vram[0x40] = 0x80;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            0,
            true,
        );
        assert_eq!(&v.working_framebuffer[0..3], &[255, 0, 0]);
    }

    #[test]
    fn unlocked_mode4_draws_sprite_nine_but_keeps_hardware_overflow_flag() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[17] = 0x03;
        v.vram[0x20] = 0x80;
        for index in 0..9 {
            v.vram[0x3F00 + index] = 0;
            let attr = 0x3F80 + index * 2;
            v.vram[attr] = if index == 8 { 50 } else { 10 };
            v.vram[attr + 1] = 1;
        }
        v.vram[0x3F09] = 0xD0;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            1,
            true,
        );
        assert_ne!(status & OVERFLOW, 0);
        assert_eq!(status & 0x1F, 8);
        let x50 = (SMS_VDP_WIDTH + 50) * 3;
        assert_eq!(&v.working_framebuffer[x50..x50 + 3], &[0, 0, 0]);

        let mut unlocked = v.clone();
        unlocked.status = 0x1F;
        unlocked.working_framebuffer.fill(0);
        let (vram, cram, regs, status, frame) = (
            &unlocked.vram,
            &unlocked.cram,
            &unlocked.registers,
            &mut unlocked.status,
            &mut unlocked.working_framebuffer,
        );
        render_scanline(vram, cram, regs, status, frame, 1, false);
        assert_ne!(*status & OVERFLOW, 0);
        assert_eq!(&frame[x50..x50 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms_mode4_horizontal_scroll_uses_subtractive_source_coordinates() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.registers[8] = 8;
        v.cram[1] = 0x03;
        // Source column 31 is blank; source column 0 starts with red.
        v.vram[0x3800 + 31 * 2] = 2;
        v.vram[0x3800 + 31 * 2 + 1] = 0;
        v.vram[0] = 0x80;
        v.vram[0x3F00] = 0xD0;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            0,
            true,
        );
        assert_eq!(&v.working_framebuffer[0..3], &[0, 0, 0]);
        assert_eq!(&v.working_framebuffer[8 * 3..8 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms_mode4_horizontal_fine_scroll_matches_source_pixel_offsets() {
        for scroll in [0_u8, 1, 7, 8, 255] {
            let mut v = SmsVdp::default();
            v.registers[0] = 0x04;
            v.registers[1] = 0x40;
            v.registers[2] = 0x0E;
            v.registers[5] = 0x7E;
            v.registers[8] = scroll;
            v.cram[1] = 0x03;
            for row in 0..8 {
                v.vram[row * 4] = 0x80;
            }
            for row in 0..28 {
                for col in 0..32 {
                    v.vram[0x3800 + row * 64 + col * 2] = 0;
                    v.vram[0x3800 + row * 64 + col * 2 + 1] = 0;
                }
            }
            v.vram[0x3F00] = 0xD0;
            let mut status = 0x1F;
            render_scanline(
                &v.vram,
                &v.cram,
                &v.registers,
                &mut status,
                &mut v.working_framebuffer,
                0,
                true,
            );
            for x in 0..32 {
                let source_x = (x + 0x100 - usize::from(scroll)) & 0xFF;
                let expected = if source_x & 7 == 0 {
                    [255, 0, 0]
                } else {
                    [0, 0, 0]
                };
                assert_eq!(
                    &v.working_framebuffer[x * 3..x * 3 + 3],
                    &expected,
                    "R8={scroll}, screen x={x}"
                );
            }
        }
    }

    #[test]
    fn sms_mode4_fine_scroll_leading_pixels_use_backdrop_not_wrapped_column() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.registers[7] = 1;
        v.registers[8] = 3;
        v.cram[1] = 0x03;
        v.cram[17] = 0x0C;
        v.vram[0x3800 + 31 * 2] = 1;
        v.vram[32] = 0xFF;
        v.vram[0] = 0x80;
        v.vram[0x3F00] = 0xD0;
        let mut status = 0;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            0,
            true,
        );
        let backdrop = super::palette_rgb(&v.cram, 17);
        let source_zero = super::palette_rgb(&v.cram, 1);
        for x in 0..3 {
            assert_eq!(
                &v.working_framebuffer[x * 3..x * 3 + 3],
                &backdrop,
                "leading fine-scroll pixel x={x}"
            );
        }
        assert_eq!(
            &v.working_framebuffer[3 * 3..3 * 3 + 3],
            &source_zero,
            "first non-blanked pixel must map to source x=0"
        );
    }

    #[test]
    fn sms_mode4_horizontal_scroll_is_fixed_for_the_first_sixteen_lines() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x44;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.registers[8] = 8;
        v.cram[1] = 0x03;
        for row in 0..8 {
            v.vram[row * 4] = 0x80;
        }
        for row in 0..28 {
            v.vram[0x3800 + row * 64 + 31 * 2] = 2;
        }
        v.vram[0x3F00] = 0xD0;
        let mut status = 0x1F;
        for y in [0, 15, 16] {
            render_scanline(
                &v.vram,
                &v.cram,
                &v.registers,
                &mut status,
                &mut v.working_framebuffer,
                y,
                true,
            );
        }
        assert_eq!(&v.working_framebuffer[0..3], &[255, 0, 0]);
        assert_eq!(
            &v.working_framebuffer[15 * SMS_VDP_WIDTH * 3..15 * SMS_VDP_WIDTH * 3 + 3],
            &[255, 0, 0]
        );
        assert_eq!(
            &v.working_framebuffer[16 * SMS_VDP_WIDTH * 3..16 * SMS_VDP_WIDTH * 3 + 3],
            &[0, 0, 0]
        );
    }

    #[test]
    fn sms_mode4_192_line_vertical_scroll_wrap_boundaries_are_223_224_and_255() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[1] = 0x03;
        v.cram[2] = 0x0C;
        v.cram[4] = 0x30;
        for col in 0..32 {
            v.vram[0x3800 + col * 2] = 0;
            v.vram[0x3800 + col * 2 + 1] = 0;
            v.vram[0x3840 + col * 2] = 0;
            v.vram[0x3840 + col * 2 + 1] = 0;
            v.vram[0x38C0 + col * 2] = 2;
            v.vram[0x38C0 + col * 2 + 1] = 0;
            v.vram[0x3EC0 + col * 2] = 1;
            v.vram[0x3EC0 + col * 2 + 1] = 0;
        }
        for row in 0..8 {
            v.vram[row * 4] = 0xFF;
            v.vram[32 + row * 4 + 1] = 0xFF;
            v.vram[64 + row * 4 + 2] = 0xFF;
        }
        v.vram[0x3F00] = 0xD0;
        let mut status = 0x1F;
        for (scroll, expected) in [
            (223_u8, [0, 255, 0]),
            (224_u8, [255, 0, 0]),
            (255_u8, [0, 0, 255]),
        ] {
            v.registers[9] = scroll;
            render_scanline(
                &v.vram,
                &v.cram,
                &v.registers,
                &mut status,
                &mut v.working_framebuffer,
                0,
                true,
            );
            assert_eq!(&v.working_framebuffer[0..3], &expected, "R9={scroll}");
        }
    }

    #[test]
    fn sms_mode4_vertical_scroll_lock_starts_at_column_24() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x84;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.registers[9] = 8;
        v.cram[1] = 0x03;
        v.cram[2] = 0x0C;
        for col in 0..32 {
            v.vram[0x3800 + col * 2] = 0;
            v.vram[0x3800 + col * 2 + 1] = 0;
            v.vram[0x3840 + col * 2] = 1;
            v.vram[0x3840 + col * 2 + 1] = 0;
        }
        for row in 0..8 {
            v.vram[row * 4] = 0xFF;
            v.vram[32 + row * 4 + 1] = 0xFF;
        }
        v.vram[0x3F00] = 0xD0;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            0,
            true,
        );
        assert_eq!(&v.working_framebuffer[0..3], &[0, 255, 0]);
        assert_eq!(&v.working_framebuffer[192 * 3..192 * 3 + 3], &[255, 0, 0]);
    }

    #[test]
    fn sms_mode4_left_column_mask_also_masks_sprites() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x24;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[17] = 0x03;
        v.vram[0x20] = 0x80;
        v.vram[0x3F00] = 0;
        v.vram[0x3F01] = 0xD0;
        v.vram[0x3F80] = 0;
        v.vram[0x3F81] = 1;
        let mut status = 0x1F;
        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut v.working_framebuffer,
            1,
            true,
        );
        assert_eq!(&v.working_framebuffer[0..3], &[0, 0, 0]);
    }

    #[test]
    fn mode4_r0_sprite_shift_and_left_column_mask_cover_all_combinations_for_sms1_and_sms2() {
        let revisions = [
            (SmsVdpRevision::Sega3155124, "SMS1", 0x7F),
            (SmsVdpRevision::Sega3155246, "SMS2", 0x7E),
        ];
        let cases = [
            (false, false, 0_u8),
            (false, false, 8),
            (true, false, 0),
            (true, false, 1),
            (false, true, 0),
            (false, true, 4),
            (true, true, 1),
            (true, true, 16),
        ];

        for (revision, revision_name, register5) in revisions {
            for (sprite_shift, left_mask, sprite_x) in cases {
                let mut v = SmsVdp::default();
                v.registers[0] =
                    0x04 | if sprite_shift { 0x08 } else { 0 } | if left_mask { 0x20 } else { 0 };
                v.registers[1] = 0x40;
                v.registers[2] = 0x0E;
                v.registers[5] = register5;
                v.cram[2] = 0x0C;
                v.cram[17] = 0x03;

                // Fill the background with palette color 2 (green).
                for column in 0..32 {
                    let entry = 0x3800 + column * 2;
                    v.vram[entry] = 1;
                    v.vram[entry + 1] = 0;
                }
                for row in 0..8 {
                    v.vram[0x20 + row * 4 + 1] = 0xFF;
                }

                // Sprite tile 2 is opaque in palette color 1 (red).
                v.vram[0x40] = 0xFF;
                v.vram[0x3F00] = 0;
                v.vram[0x3F01] = 0xD0;
                v.vram[0x3F80] = sprite_x;
                v.vram[0x3F81] = 2;

                let mut status = 0x1F;
                render_scanline_for_revision(
                    &v.vram,
                    &v.cram,
                    &v.registers,
                    &mut status,
                    &mut v.working_framebuffer,
                    1,
                    true,
                    revision,
                );

                let sprite_shift_pixels: i16 = if sprite_shift { 8 } else { 0 };
                let sprite_start = i16::from(sprite_x) - sprite_shift_pixels;
                for x in 0..SMS_VDP_WIDTH {
                    let expected = if left_mask && x < 8 {
                        [0, 0, 0]
                    } else if (sprite_start..sprite_start + 8).contains(&(x as i16)) {
                        [255, 0, 0]
                    } else {
                        [0, 255, 0]
                    };
                    let offset = (SMS_VDP_WIDTH + x) * 3;
                    assert_eq!(
                        &v.working_framebuffer[offset..offset + 3],
                        &expected,
                        "{revision_name}: R0 bit3={sprite_shift}, bit5={left_mask}, sprite X={sprite_x:#04x}, pixel X={x}"
                    );
                }
            }
        }
    }

    #[test]
    fn sms_mode4_r9_written_during_active_display_latches_after_active_period() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[1] = 0x03;
        v.cram[2] = 0x0C;
        for col in 0..32 {
            v.vram[0x3840 + col * 2] = 1;
            v.vram[0x3840 + col * 2 + 1] = 0;
        }
        for row in 0..8 {
            v.vram[row * 4] = 0xFF;
            v.vram[32 + row * 4 + 1] = 0xFF;
        }
        v.vram[0x3F00] = 0xD0;
        reg(&mut v, 9, 8);

        v.advance(228 * 192);
        assert_eq!(&v.framebuffer()[0..3], &[255, 0, 0]);

        v.advance(228 * (262 - 192));
        v.advance(228 * 192);
        assert_eq!(&v.framebuffer()[0..3], &[0, 255, 0]);
    }

    #[test]
    fn sms_mode4_extended_mode_d0_is_not_sprite_terminator_and_d1_stays_visible() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x50;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[17] = 0x03;
        v.vram[0x20] = 0x80;
        v.vram[0x3F00] = 0xD0;
        v.vram[0x3F01] = 0xD1;
        v.vram[0x3F80] = 10;
        v.vram[0x3F81] = 1;
        v.vram[0x3F82] = 30;
        v.vram[0x3F83] = 1;
        let mut status = 0x1F;
        let mut frame = vec![0; FRAME_SIZE];

        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            209,
            true,
        );
        assert_eq!(
            &frame[(209 * SMS_VDP_WIDTH + 10) * 3..(209 * SMS_VDP_WIDTH + 10) * 3 + 3],
            &[255, 0, 0]
        );
        assert_eq!(
            &frame[(209 * SMS_VDP_WIDTH + 30) * 3..(209 * SMS_VDP_WIDTH + 30) * 3 + 3],
            &[0, 0, 0]
        );

        render_scanline(
            &v.vram,
            &v.cram,
            &v.registers,
            &mut status,
            &mut frame,
            210,
            true,
        );
        assert_eq!(
            &frame[(210 * SMS_VDP_WIDTH + 30) * 3..(210 * SMS_VDP_WIDTH + 30) * 3 + 3],
            &[255, 0, 0]
        );
    }

    #[test]
    fn status_port_preserves_sprite_index_and_clears_status_flags() {
        let mut v = SmsVdp {
            status: VBLANK | OVERFLOW | COLLISION | 0x0A,
            ..SmsVdp::default()
        };
        let status = v.read_port(0xBF);
        assert_eq!(status, 0xEA);
        assert_eq!(v.status(), 0);
        assert!(!v.interrupt_pending());
    }

    #[test]
    fn cram_write_wraps_palette_index_but_keeps_fourteen_bit_address() {
        let mut v = SmsVdp::default();
        addr(&mut v, 0x001F, 0xC0);
        v.write_port(0xBE, 0xFF);
        assert_eq!(v.cram()[0x1F], 0x3F);
        assert_eq!(v.address(), 0x0020);
        v.write_port(0xBE, 0x12);
        assert_eq!(v.cram()[0], 0x12);
        assert_eq!(v.address(), 0x0021);
        assert_eq!(v.read_port(0xBE), 0x12);
    }

    #[test]
    fn register_write_command_updates_vdp_address_register() {
        let mut v = SmsVdp::default();
        reg(&mut v, 8, 0xA5);
        assert_eq!(v.registers()[8], 0xA5);
        assert_eq!(v.address(), 0x08A5);
    }

    #[test]
    fn vdp_ports_are_mirrored_by_the_low_address_decode() {
        let mut v = SmsVdp::default();
        v.write_port(0x9F, 0x03);
        v.write_port(0x9F, 0xC0);
        v.write_port(0x9E, 0x3F);
        assert_eq!(v.cram()[3], 0x3F);
    }

    #[test]
    fn legacy_sprite_limit_is_four_and_unlock_draws_the_fifth_sprite() {
        let mut v = SmsVdp::default();
        v.registers[5] = 0x7F;
        v.vram[8] = 0x80;
        for index in 0..5 {
            let attr = 0x3F80 + index * 4;
            v.vram[attr] = 0;
            v.vram[attr + 1] = if index == 4 { 50 } else { 10 };
            v.vram[attr + 2] = 1;
            v.vram[attr + 3] = 4;
        }
        v.vram[0x3F80 + 5 * 4] = 0xD0;
        let mut status = 0x1F;
        let mut frame = vec![0; FRAME_SIZE];
        render_legacy_sprites(&v.vram, &v.registers, &mut status, &mut frame, 1, false);
        assert_ne!(status & OVERFLOW, 0);
        assert_eq!(status & 0x1F, 4);
        let x50 = (SMS_VDP_WIDTH + 50) * 3;
        assert_eq!(&frame[x50..x50 + 3], &[0, 0, 0]);

        status = 0x1F;
        frame.fill(0);
        render_legacy_sprites(&v.vram, &v.registers, &mut status, &mut frame, 1, true);
        assert_eq!(&frame[x50..x50 + 3], &TMS_PALETTE[4]);
    }

    #[test]
    fn unknown_video_standard_uses_the_ntsc_counter_profile() {
        let mut v = SmsVdp::new(VideoStandard::Unknown, SpriteLimitMode::HardwareLimit);
        v.scanline = 219;
        assert_eq!(v.read_v_counter(), 0xD5);
    }

    #[test]
    fn pal_m_uses_ntsc_like_vertical_counter_profile() {
        let mut v = SmsVdp::new(VideoStandard::PalM, SpriteLimitMode::HardwareLimit);
        v.scanline = 219;
        assert_eq!(v.read_v_counter(), 0xD5);
        assert_eq!(v.total_lines(), 262);
        assert_eq!(v.system_clock_hz(), 3_579_545);
    }

    #[test]
    fn pal_224_line_counter_has_documented_wrap_sequence() {
        let mut v = SmsVdp::new(VideoStandard::Pal, SpriteLimitMode::HardwareLimit);
        reg(&mut v, 1, 0x10);
        v.scanline = 256;
        assert_eq!(v.read_v_counter(), 0x00);
        v.scanline = 258;
        assert_eq!(v.read_v_counter(), 0x02);
        v.scanline = 259;
        assert_eq!(v.read_v_counter(), 0xCA);
        v.scanline = 312;
        assert_eq!(v.read_v_counter(), 0xFF);
    }

    #[test]
    fn pal_240_line_counter_has_documented_wrap_sequence() {
        let mut v = SmsVdp::new(VideoStandard::Pal, SpriteLimitMode::HardwareLimit);
        reg(&mut v, 1, 0x08);
        v.scanline = 266;
        assert_eq!(v.read_v_counter(), 0x0A);
        v.scanline = 267;
        assert_eq!(v.read_v_counter(), 0xD2);
        v.scanline = 312;
        assert_eq!(v.read_v_counter(), 0xFF);
    }

    fn configure_horizontal_scroll_pattern(v: &mut SmsVdp) {
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[1] = 0x03;
        for row in 0..8 {
            v.vram[0x20 + row * 4] = 0xAA;
        }
        for row in 0..24 {
            for col in 0..32 {
                v.vram[0x3800 + row * 64 + col * 2] = 1;
                v.vram[0x3800 + row * 64 + col * 2 + 1] = 0;
            }
        }
        v.vram[0x3F00] = 0xD0;
    }

    #[test]
    fn horizontal_scroll_changes_between_scanlines_without_rewriting_prior_lines() {
        let mut v = SmsVdp::default();
        configure_horizontal_scroll_pattern(&mut v);

        v.advance(228);
        let first_line = v.working_framebuffer[..SMS_VDP_WIDTH * 3].to_vec();

        reg(&mut v, 8, 1);
        v.advance(228);
        let second_line = v.working_framebuffer[SMS_VDP_WIDTH * 3..SMS_VDP_WIDTH * 6].to_vec();

        assert_ne!(
            first_line, second_line,
            "R8 change must affect the next rendered line"
        );
        assert_eq!(
            &v.working_framebuffer[..SMS_VDP_WIDTH * 3],
            &first_line,
            "a later R8 write must not retroactively rewrite an already-rendered line"
        );
        assert_eq!(v.raster_position().line, 2);
        assert_eq!(v.raster_position().tstates_into_line, 0);
    }

    #[test]
    fn horizontal_scroll_and_raster_progress_are_chunking_invariant() {
        let mut whole_chunks = SmsVdp::default();
        let mut split_chunks = SmsVdp::default();
        configure_horizontal_scroll_pattern(&mut whole_chunks);
        configure_horizontal_scroll_pattern(&mut split_chunks);

        for scroll in [0_u8, 1, 7, 8] {
            whole_chunks.advance(228);
            split_chunks.advance(57);
            split_chunks.advance(171);
            reg(&mut whole_chunks, 8, scroll);
            reg(&mut split_chunks, 8, scroll);
        }

        assert_eq!(
            whole_chunks.raster_position(),
            split_chunks.raster_position()
        );
        assert_eq!(
            whole_chunks.working_framebuffer,
            split_chunks.working_framebuffer
        );
        assert_eq!(whole_chunks.registers()[8], split_chunks.registers()[8]);
    }

    #[test]
    fn sms_mode4_horizontal_scroll_all_register_values_match_full_width_pixel_oracle() {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[1] = 0x03;
        // Every tile is tile zero; its first bit in each row is set.
        // This creates a one-pixel vertical stripe every eight source pixels.
        for row in 0..8 {
            v.vram[row * 4] = 0x80;
        }
        for row in 0..24 {
            for col in 0..32 {
                let entry = 0x3800 + row * 64 + col * 2;
                v.vram[entry] = 0;
                v.vram[entry + 1] = 0;
            }
        }
        v.vram[0x3F00] = 0xD0;

        for scroll in 0_u16..=255 {
            v.registers[8] = scroll as u8;
            v.working_framebuffer.fill(0);
            let mut status = 0;
            render_scanline(
                &v.vram,
                &v.cram,
                &v.registers,
                &mut status,
                &mut v.working_framebuffer,
                0,
                true,
            );

            let fine_scroll = (scroll & 7) as usize;
            for x in 0..SMS_VDP_WIDTH {
                let expected = if x < fine_scroll {
                    [0, 0, 0]
                } else if (x - fine_scroll) & 7 == 0 {
                    [255, 0, 0]
                } else {
                    [0, 0, 0]
                };
                let offset = x * 3;
                assert_eq!(
                    &v.working_framebuffer[offset..offset + 3],
                    &expected,
                    "R8={scroll}, screen x={x}"
                );
            }
        }
    }

    #[test]
    fn sms_vdp_raster_is_invariant_for_every_single_line_advance_split() {
        for split in 1_u64..LINE_TSTATES {
            let mut whole = SmsVdp::default();
            let mut fragmented = whole.clone();
            configure_horizontal_scroll_pattern(&mut whole);
            configure_horizontal_scroll_pattern(&mut fragmented);

            whole.advance(LINE_TSTATES);
            fragmented.advance(split);
            fragmented.advance(LINE_TSTATES - split);

            assert_eq!(
                whole.raster_position(),
                fragmented.raster_position(),
                "raster mismatch at split={split}"
            );
            assert_eq!(
                whole.working_framebuffer, fragmented.working_framebuffer,
                "framebuffer mismatch at split={split}"
            );
            assert_eq!(
                whole.status(),
                fragmented.status(),
                "status at split={split}"
            );
        }
    }

    #[test]
    fn framebuffer_is_rgb24() {
        let v = SmsVdp::default();
        assert_eq!(v.framebuffer().len(), SMS_VDP_WIDTH * 192 * 3);
    }

    // Characterization tests for the current scanline renderer. These record
    // when the model samples mutable VDP state; they are not hardware oracles.
    fn setup_midline_write_fixture() -> SmsVdp {
        let mut v = SmsVdp::default();
        v.registers[0] = 0x04;
        v.registers[1] = 0x40;
        v.registers[2] = 0x0E;
        v.registers[5] = 0x7E;
        v.cram[1] = 0x03;
        v.vram[0] = 0x80;
        v.vram[0x3800] = 0;
        v.vram[0x3F00] = 0xD0;
        v
    }

    #[test]
    fn r8_write_at_hpos_21_updates_the_current_scanline() {
        let mut actual = setup_midline_write_fixture();
        actual.advance(14);
        reg(&mut actual, 8, 8);
        actual.advance(214);

        let mut expected = setup_midline_write_fixture();
        reg(&mut expected, 8, 8);
        expected.advance(228);

        let line_bytes = SMS_VDP_WIDTH * 3;
        assert_eq!(
            &actual.working_framebuffer[..line_bytes],
            &expected.working_framebuffer[..line_bytes]
        );
    }

    #[test]
    fn r8_write_after_hpos_21_updates_only_the_next_scanline() {
        let mut actual = setup_midline_write_fixture();
        actual.advance(15);
        reg(&mut actual, 8, 8);
        actual.advance(213);

        let mut expected_current = setup_midline_write_fixture();
        expected_current.advance(228);

        let line_bytes = SMS_VDP_WIDTH * 3;
        assert_eq!(
            &actual.working_framebuffer[..line_bytes],
            &expected_current.working_framebuffer[..line_bytes]
        );

        actual.advance(228);

        let mut expected_next = setup_midline_write_fixture();
        reg(&mut expected_next, 8, 8);
        expected_next.advance(456);

        assert_eq!(
            &actual.working_framebuffer[line_bytes..line_bytes * 2],
            &expected_next.working_framebuffer[line_bytes..line_bytes * 2]
        );
    }

    #[test]
    fn h_counter_conversion_matches_sms_reference_positions() {
        for (tstates, expected) in [
            (0, 0x93),
            (1, 0xE9),
            (31, 0xFF),
            (32, 0x00),
            (136, 0x4E),
        ] {
            let mut vdp = SmsVdp::default();
            vdp.advance(tstates);
            vdp.latch_h_counter();
            assert_eq!(
                vdp.read_h_counter(),
                expected,
                "H-counter at Z80 T-state {tstates}"
            );
        }
    }

    #[test]
    fn h_counter_remains_latched_while_the_vdp_advances() {
        let mut vdp = SmsVdp::default();
        assert_eq!(vdp.read_h_counter(), 0);

        vdp.advance(136);
        vdp.latch_h_counter();
        assert_eq!(vdp.read_h_counter(), 0x4E);

        vdp.advance(50);
        assert_eq!(vdp.read_h_counter(), 0x4E);
    }

    #[test]
    fn midline_cram_write_is_sampled_for_the_whole_scanline_by_current_model() {
        let mut v = setup_midline_write_fixture();

        v.advance(114);
        addr(&mut v, 1, 0xC0);
        v.write_port(0xBE, 0x0C);
        v.advance(114);

        // CRAM is mutated immediately and the line is rendered at its end;
        // therefore even pixels left of the write phase use the new color.
        // This is a model-characterization assertion, not a hardware oracle.
        assert_eq!(&v.working_framebuffer[0..3], &[0, 255, 0]);
    }
}
