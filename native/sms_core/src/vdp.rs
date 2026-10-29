//! SMS VDP (315-5124/315-5246 family), implemented independently from SG-1000.
//! Deterministic scanline model: Mode 4, VRAM/CRAM, scroll, sprites, IRQ, light-gun
//! click injection and standard-monitor SegaScope output. Not dot-cycle exact.

use crate::config::{SpriteLimitMode, VideoStandard};

pub const SMS_VDP_WIDTH: usize = 256;
pub const SMS_VDP_MAX_HEIGHT: usize = 240;
const VRAM_SIZE: usize = 0x4000;
const CRAM_SIZE: usize = 0x20;
const LINE_TSTATES: u64 = 228;
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
    video_standard: VideoStandard,
    sprite_limit: SpriteLimitMode,
    line_counter: u8,
    line_irq_pending: bool,
    light_x: u16,
    light_y: u16,
    light_trigger: bool,
    light_pulse: bool,
    light_h_counter: Option<u8>,
    stereo_eye: SmsVdpEye,
    stereo_presentation: StereoPresentation,
    left_frame: Vec<u8>,
    right_frame: Vec<u8>,
}

impl SmsVdp {
    pub fn new(video_standard: VideoStandard, sprite_limit: SpriteLimitMode) -> Self {
        Self {
            vram: vec![0; VRAM_SIZE],
            cram: [0; CRAM_SIZE],
            registers: [0; 16],
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
            video_standard,
            sprite_limit,
            line_counter: 0,
            line_irq_pending: false,
            light_x: 0,
            light_y: 0,
            light_trigger: false,
            light_pulse: false,
            light_h_counter: None,
            stereo_eye: SmsVdpEye::Right,
            stereo_presentation: StereoPresentation::Alternating,
            left_frame: vec![0; FRAME_SIZE],
            right_frame: vec![0; FRAME_SIZE],
        }
    }

    pub fn video_standard(&self) -> VideoStandard {
        self.video_standard
    }
    pub fn system_clock_hz(&self) -> u64 {
        match self.video_standard {
            VideoStandard::Pal => 3_546_895,
            VideoStandard::Ntsc | VideoStandard::Unknown => 3_579_545,
        }
    }
    pub fn total_lines(&self) -> u16 {
        match self.video_standard {
            VideoStandard::Pal => 313,
            _ => 262,
        }
    }
    pub fn visible_height(&self) -> u16 {
        if self.registers[1] & 0x08 != 0 {
            240
        } else if self.registers[1] & 0x10 != 0 {
            224
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
        self.sprite_limit
    }
    pub fn set_sprite_limit_mode(&mut self, mode: SpriteLimitMode) {
        self.sprite_limit = mode;
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
            self.light_h_counter = None;
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
        self.light_h_counter = None;
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
                let value = self.status | 0x1F;
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
        match (self.video_standard, self.visible_height()) {
            (VideoStandard::Ntsc | VideoStandard::Unknown, 192) if line >= 219 => {
                0xD5_u8.wrapping_add((line - 219) as u8)
            }
            (VideoStandard::Ntsc | VideoStandard::Unknown, 224) if line >= 235 => {
                0xE5_u8.wrapping_add((line - 235) as u8)
            }
            (VideoStandard::Pal, 192) if line >= 243 => 0xBA_u8.wrapping_add((line - 243) as u8),
            (VideoStandard::Pal, 224) if line >= 259 => {
                0xCA_u8.wrapping_add((line - 259) as u8)
            }
            (VideoStandard::Pal, 240) if line >= 267 => {
                0xD2_u8.wrapping_add((line - 267) as u8)
            }
            _ => line as u8,
        }
    }
    /// Approximate H-counter outside the click latch; sub-instruction timing is pending.
    pub fn read_h_counter(&self) -> u8 {
        self.light_h_counter
            .unwrap_or_else(|| ((u64::from(self.tstates_into_line) * 256) / LINE_TSTATES) as u8)
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
        self.light_h_counter = None;
        let height = self.visible_height();
        if line < height {
            let mut render_registers = self.registers;
            render_registers[9] = self.latched_scroll_y;
            render_scanline(
                &self.vram,
                &self.cram,
                &render_registers,
                &mut self.status,
                &mut self.working_framebuffer,
                usize::from(line),
                self.sprite_limit == SpriteLimitMode::HardwareLimit,
            );
            if self.light_trigger && self.light_y == line {
                self.light_pulse = true;
                self.light_h_counter = Some(0x08 + (self.light_x / 2) as u8);
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
                if register == 9 {
                    if self.scanline < self.visible_height() {
                        self.pending_scroll_y = Some(first);
                    } else {
                        self.latched_scroll_y = first;
                        self.pending_scroll_y = None;
                    }
                }
                if register == 10 {
                    self.line_counter = first;
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

fn render_scanline(
    vram: &[u8],
    cram: &[u8; CRAM_SIZE],
    regs: &[u8; 16],
    status: &mut u8,
    frame: &mut [u8],
    y: usize,
    hardware_sprite_limit: bool,
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
    for x in 0..SMS_VDP_WIDTH {
        if regs[0] & 0x20 != 0 && x < 8 {
            continue;
        }
        let hs = if regs[0] & 0x40 != 0 && y < 16 {
            0
        } else {
            usize::from(regs[8])
        };
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
        let raw_y = vram[(sat + index) & 0x3FFF];
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
        let attr = (sat + 0x80 + index * 2) & 0x3FFF;
        let mut sx = i32::from(vram[attr]);
        if regs[0] & 0x08 != 0 {
            sx -= 8;
        }
        let mut tile = usize::from(vram[(attr + 1) & 0x3FFF]);
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
    for &(_index, sx, tile, row) in active.iter().take(count) {
        let tile_row = if sprite_height == 16 && row >= 8 {
            tile + 1
        } else {
            tile
        };
        let py = if sprite_height == 16 { row & 7 } else { row };
        let p = (sprite_pattern + tile_row * 32 + py * 4) & 0x3FFF;
        for px in 0..8 {
            let mut color = 0_u8;
            for plane in 0..4 {
                color |= ((vram[(p + plane) & 0x3FFF] >> (7 - px)) & 1) << plane;
            }
            if color == 0 {
                continue;
            }
            for zx in 0..zoom {
                let x = sx + (px * zoom + zx);
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
    use super::{
        COLLISION, FRAME_SIZE, OVERFLOW, SMS_VDP_WIDTH, SmsVdp, StereoPresentation, TMS_PALETTE,
        VBLANK,
        render_legacy_scanline, render_legacy_sprites, render_scanline,
    };
    use crate::config::{SpriteLimitMode, VideoStandard};
    fn reg(v: &mut SmsVdp, i: u8, value: u8) {
        v.write_port(0xBF, value);
        v.write_port(0xBF, 0x80 | i);
    }
    fn addr(v: &mut SmsVdp, a: u16, mode: u8) {
        v.write_port(0xBF, a as u8);
        v.write_port(0xBF, ((a >> 8) as u8 & 0x3F) | mode);
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
        let p = SmsVdp::new(VideoStandard::Pal, SpriteLimitMode::HardwareLimit);
        assert_eq!((n.total_lines(), n.system_clock_hz()), (262, 3_579_545));
        assert_eq!((p.total_lines(), p.system_clock_hz()), (313, 3_546_895));
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
    fn status_port_sets_reserved_low_bits_and_clears_status_flags() {
        let mut v = SmsVdp::default();
        v.status = VBLANK | OVERFLOW | COLLISION;
        let status = v.read_port(0xBF);
        assert_eq!(status, 0xFF);
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

    #[test]
    fn framebuffer_is_rgb24() {
        let v = SmsVdp::default();
        assert_eq!(v.framebuffer().len(), SMS_VDP_WIDTH * 192 * 3);
    }
}
