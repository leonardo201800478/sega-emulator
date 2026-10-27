//! Native TMS9918A-compatible VDP for the SG-1000.
use std::collections::HashSet;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 192;
const TSTATES_PER_LINE: u64 = 228;
const LINES_PER_FRAME: u16 = 262;
const VBLANK_LINE: u16 = 192;
const VRAM_SIZE: usize = 0x4000;
const VRAM_MASK: usize = VRAM_SIZE - 1;
const RGB_FRAME_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 3;

const PALETTE: [[u8; 3]; 16] = [
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tms9918A {
    vram: Vec<u8>,
    registers: [u8; 8],
    status: u8,
    address: u16,
    read_buffer: u8,
    control_first: Option<u8>,
    cycles_into_line: u16,
    scanline: u16,
    frame_number: u64,
    framebuffer: Vec<u8>,
    working_framebuffer: Vec<u8>,
    frame_ready: bool,
    sprite_limit_enabled: bool,
}

impl Tms9918A {
    pub fn new() -> Self {
        Self {
            vram: vec![0; VRAM_SIZE],
            registers: [0; 8],
            status: 0x1F,
            address: 0,
            read_buffer: 0,
            control_first: None,
            cycles_into_line: 0,
            scanline: 0,
            frame_number: 0,
            framebuffer: vec![0; RGB_FRAME_SIZE],
            working_framebuffer: vec![0; RGB_FRAME_SIZE],
            frame_ready: false,
            sprite_limit_enabled: true,
        }
    }

    /// Configure the SG-1000's four-sprites-per-scanline limit.
    pub fn set_sprite_limit_enabled(&mut self, enabled: bool) {
        self.sprite_limit_enabled = enabled;
    }

    pub fn sprite_limit_enabled(&self) -> bool {
        self.sprite_limit_enabled
    }

    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }

    pub fn registers(&self) -> &[u8; 8] {
        &self.registers
    }

    pub fn status(&self) -> u8 {
        self.status
    }

    pub fn address(&self) -> u16 {
        self.address
    }

    pub fn read_buffer(&self) -> u8 {
        self.read_buffer
    }

    pub fn scanline(&self) -> u16 {
        self.scanline
    }

    pub fn cycles_into_line(&self) -> u16 {
        self.cycles_into_line
    }

    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn display_mode(&self) -> u8 {
        display_mode(&self.registers)
    }

    pub fn interrupt_pending(&self) -> bool {
        (self.status & 0x80 != 0) && (self.registers[1] & 0x20 != 0)
    }

    pub fn reset(&mut self, clear_vram: bool) {
        self.registers.fill(0);
        self.status = 0x1F;
        self.address = 0;
        self.read_buffer = 0;
        self.control_first = None;
        self.cycles_into_line = 0;
        self.scanline = 0;
        self.frame_number = 0;
        self.framebuffer.fill(0);
        self.working_framebuffer.fill(0);
        self.frame_ready = false;
        if clear_vram {
            self.vram.fill(0);
        }
    }

    pub fn read_port(&mut self, port: u16) -> u8 {
        match (port as u8) & 0xC1 {
            0x80 => {
                let value = self.read_buffer;
                self.read_buffer = self.vram[usize::from(self.address) & VRAM_MASK];
                self.address = self.address.wrapping_add(1) & VRAM_MASK as u16;
                self.control_first = None;
                value
            }
            0x81 => {
                let value = self.status;
                self.status = 0x1F;
                self.control_first = None;
                value
            }
            _ => 0xFF,
        }
    }

    pub fn write_port(&mut self, port: u16, value: u8) {
        let port = (port as u8) & 0xC1;
        if port == 0x80 {
            let address = usize::from(self.address) & VRAM_MASK;
            self.vram[address] = value;
            self.read_buffer = value;
            self.address = ((address + 1) & VRAM_MASK) as u16;
            self.control_first = None;
            return;
        }
        if port != 0x81 {
            return;
        }
        let Some(first) = self.control_first.take() else {
            self.control_first = Some(value);
            return;
        };
        if value & 0x80 != 0 {
            self.registers[usize::from(value & 0x07)] = first;
            return;
        }
        self.address = ((((value & 0x3F) as u16) << 8) | u16::from(first)) & VRAM_MASK as u16;
        if value & 0x40 != 0 {
            return;
        }
        self.read_buffer = self.vram[usize::from(self.address) & VRAM_MASK];
        self.address = self.address.wrapping_add(1) & VRAM_MASK as u16;
    }

    pub fn advance(&mut self, tstates: u64) {
        let mut remaining = tstates;
        let cycles_to_line_end = TSTATES_PER_LINE - u64::from(self.cycles_into_line);
        if remaining < cycles_to_line_end {
            self.cycles_into_line += remaining as u16;
            return;
        }
        while remaining > 0 {
            let step = remaining.min(TSTATES_PER_LINE - u64::from(self.cycles_into_line));
            self.cycles_into_line += step as u16;
            remaining -= step;
            if u64::from(self.cycles_into_line) == TSTATES_PER_LINE {
                self.cycles_into_line = 0;
                let completed_line = self.scanline;
                if usize::from(completed_line) < SCREEN_HEIGHT {
                    render_scanline(
                        &self.vram,
                        &self.registers,
                        &mut self.status,
                        &mut self.working_framebuffer,
                        usize::from(completed_line),
                        true,
                        self.sprite_limit_enabled,
                    );
                } else if completed_line < LINES_PER_FRAME
                    && self.registers[1] & 0x40 != 0
                    && display_mode(&self.registers) != 1
                {
                    render_sprite_line(
                        &self.vram,
                        &self.registers,
                        &mut self.status,
                        None,
                        i32::from(completed_line),
                        true,
                        self.sprite_limit_enabled,
                    );
                }
                self.scanline += 1;
                if self.scanline == VBLANK_LINE {
                    self.status |= 0x80;
                    self.frame_ready = true;
                    self.framebuffer.clone_from(&self.working_framebuffer);
                }
                if self.scanline >= LINES_PER_FRAME {
                    self.scanline = 0;
                    self.frame_number += 1;
                    self.working_framebuffer.fill(0);
                }
            }
        }
    }

    pub fn consume_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    pub fn render_frame(&self) -> Vec<u8> {
        render_frame_bytes(
            &self.vram,
            &self.registers,
            self.status,
            self.sprite_limit_enabled,
        )
    }
}

impl Default for Tms9918A {
    fn default() -> Self {
        Self::new()
    }
}

fn display_mode(registers: &[u8]) -> u8 {
    ((registers[1] >> 4) & 1) | ((registers[1] >> 2) & 2) | (((registers[0] >> 1) & 1) << 2)
}

fn write_color(pixels: &mut [u8], offset: usize, color: u8) {
    if offset + 3 <= pixels.len() {
        pixels[offset..offset + 3].copy_from_slice(&PALETTE[usize::from(color & 0x0F)]);
    }
}

fn fill_backdrop_line(pixels: &mut [u8], y: usize, backdrop: u8) {
    let start = y * SCREEN_WIDTH * 3;
    let color = PALETTE[usize::from(backdrop & 0x0F)];
    for pixel in pixels[start..start + SCREEN_WIDTH * 3]
        .as_chunks_mut::<3>()
        .0
    {
        pixel.copy_from_slice(&color);
    }
}

fn pattern_span(
    pixels: &mut [u8],
    offset: usize,
    pattern: u8,
    foreground: u8,
    background: u8,
    width: usize,
) {
    for bit in 0..width {
        let color = if pattern & (0x80 >> bit) != 0 {
            foreground
        } else {
            background
        };
        write_color(pixels, offset + bit * 3, color);
    }
}

fn color_quarter(pixels: &mut [u8], offset: usize, color: u8) {
    for pixel in 0..4 {
        write_color(pixels, offset + pixel * 3, color);
    }
}

fn render_scanline(
    vram: &[u8],
    registers: &[u8],
    status: &mut u8,
    pixels: &mut [u8],
    y: usize,
    update_sprite_status: bool,
    sprite_limit_enabled: bool,
) {
    let backdrop = registers[7] & 0x0F;
    if registers[1] & 0x40 == 0 {
        fill_backdrop_line(pixels, y, backdrop);
        return;
    }
    let mode = display_mode(registers);
    let row_start = y * SCREEN_WIDTH * 3;
    if mode == 1 {
        let name_base = usize::from(registers[2] & 0x0F) << 10;
        let pattern_base = usize::from(registers[4] & 0x07) << 11;
        let text_row = y / 8;
        let glyph_row = y % 8;
        let foreground = if registers[7] >> 4 != 0 {
            registers[7] >> 4
        } else {
            backdrop
        };
        for column in 0..40 {
            let character = vram[(name_base + text_row * 40 + column) & VRAM_MASK];
            let pattern = vram[(pattern_base + usize::from(character) * 8 + glyph_row) & VRAM_MASK];
            pattern_span(
                pixels,
                row_start + (8 + column * 6) * 3,
                pattern,
                foreground,
                backdrop,
                6,
            );
        }
        return;
    }
    if mode == 4 {
        let name_base = usize::from(registers[2] & 0x0F) << 10;
        let pattern_base = usize::from(registers[4] & 0x04) << 11;
        let color_base = usize::from(registers[3] & 0x80) << 6;
        let row = y & 7;
        let section = (y & 0xC0) << 5;
        for tile_x in 0..32 {
            let name = vram[(name_base + (y / 8) * 32 + tile_x) & VRAM_MASK];
            let offset = section + usize::from(name) * 8 + row;
            let pattern = vram[(pattern_base + offset) & VRAM_MASK];
            let color = vram[(color_base + offset) & VRAM_MASK];
            let foreground = if color >> 4 != 0 {
                color >> 4
            } else {
                backdrop
            };
            let background = if color & 0x0F != 0 {
                color & 0x0F
            } else {
                backdrop
            };
            pattern_span(
                pixels,
                row_start + tile_x * 24,
                pattern,
                foreground,
                background,
                8,
            );
        }
    } else if mode == 2 {
        let name_base = usize::from(registers[2] & 0x0F) << 10;
        let pattern_base = usize::from(registers[4] & 0x07) << 11;
        let tile_y = y / 8;
        let pattern_row = (y & 0x1C) >> 2;
        for tile_x in 0..32 {
            let name = vram[(name_base + tile_y * 32 + tile_x) & VRAM_MASK];
            let pattern = vram[(pattern_base + usize::from(name) * 8 + pattern_row) & VRAM_MASK];
            let left = if pattern >> 4 != 0 {
                pattern >> 4
            } else {
                backdrop
            };
            let right = if pattern & 0x0F != 0 {
                pattern & 0x0F
            } else {
                backdrop
            };
            color_quarter(pixels, row_start + tile_x * 24, left);
            color_quarter(pixels, row_start + tile_x * 24 + 12, right);
        }
    } else {
        let name_base = usize::from(registers[2] & 0x0F) << 10;
        let pattern_base = usize::from(registers[4] & 0x07) << 11;
        let color_base = (usize::from(registers[3]) << 6) & 0x3FC0;
        let tile_y = y / 8;
        let row = y % 8;
        for tile_x in 0..32 {
            let name = vram[(name_base + tile_y * 32 + tile_x) & VRAM_MASK];
            let pattern = vram[(pattern_base + usize::from(name) * 8 + row) & VRAM_MASK];
            let color = vram[(color_base + usize::from(name) / 8) & VRAM_MASK];
            let foreground = if color >> 4 != 0 {
                color >> 4
            } else {
                backdrop
            };
            let background = if color & 0x0F != 0 {
                color & 0x0F
            } else {
                backdrop
            };
            pattern_span(
                pixels,
                row_start + tile_x * 24,
                pattern,
                foreground,
                background,
                8,
            );
        }
    }
    render_sprite_line(
        vram,
        registers,
        status,
        Some(pixels),
        y as i32,
        update_sprite_status,
        sprite_limit_enabled,
    );
}

fn render_frame_bytes(
    vram: &[u8],
    registers: &[u8],
    status: u8,
    sprite_limit_enabled: bool,
) -> Vec<u8> {
    let backdrop = registers[7] & 0x0F;
    let mut pixels = vec![0; RGB_FRAME_SIZE];
    for pixel in pixels.as_chunks_mut::<3>().0 {
        pixel.copy_from_slice(&PALETTE[usize::from(backdrop)]);
    }
    if registers[1] & 0x40 == 0 {
        return pixels;
    }
    let mode = display_mode(registers);
    if mode == 1 {
        render_text(vram, registers, &mut pixels, backdrop);
    } else if mode == 4 {
        render_graphics_ii(vram, registers, &mut pixels, backdrop);
    } else if mode == 2 {
        render_multicolor(vram, registers, &mut pixels, backdrop);
    } else {
        render_graphics_i(vram, registers, &mut pixels, backdrop);
    }
    if mode != 1 {
        let mut status_copy = status;
        render_sprites(
            vram,
            registers,
            &mut status_copy,
            &mut pixels,
            false,
            sprite_limit_enabled,
        );
    }
    pixels
}

fn put_pixel(pixels: &mut [u8], x: i32, y: i32, color: u8) {
    if x < 0 || x >= SCREEN_WIDTH as i32 || y < 0 || y >= SCREEN_HEIGHT as i32 {
        return;
    }
    let offset = ((y as usize) * SCREEN_WIDTH + x as usize) * 3;
    write_color(pixels, offset, color);
}

fn render_graphics_i(vram: &[u8], registers: &[u8], pixels: &mut [u8], backdrop: u8) {
    let name_base = usize::from(registers[2] & 0x0F) << 10;
    let pattern_base = usize::from(registers[4] & 0x07) << 11;
    let color_base = (usize::from(registers[3]) << 6) & 0x3FC0;
    for y in 0..SCREEN_HEIGHT {
        let tile_y = y / 8;
        let row = y % 8;
        let row_start = y * SCREEN_WIDTH * 3;
        for tile_x in 0..32 {
            let name = vram[(name_base + tile_y * 32 + tile_x) & VRAM_MASK];
            let pattern = vram[(pattern_base + usize::from(name) * 8 + row) & VRAM_MASK];
            let color = vram[(color_base + usize::from(name) / 8) & VRAM_MASK];
            let foreground = if color >> 4 != 0 {
                color >> 4
            } else {
                backdrop
            };
            let background = if color & 0x0F != 0 {
                color & 0x0F
            } else {
                backdrop
            };
            pattern_span(
                pixels,
                row_start + tile_x * 24,
                pattern,
                foreground,
                background,
                8,
            );
        }
    }
}

fn render_graphics_ii(vram: &[u8], registers: &[u8], pixels: &mut [u8], backdrop: u8) {
    let name_base = usize::from(registers[2] & 0x0F) << 10;
    let pattern_base = usize::from(registers[4] & 0x04) << 11;
    let color_base = usize::from(registers[3] & 0x80) << 6;
    for y in 0..SCREEN_HEIGHT {
        let row = y & 7;
        let section = (y & 0xC0) << 5;
        let row_start = y * SCREEN_WIDTH * 3;
        for tile_x in 0..32 {
            let name = vram[(name_base + (y / 8) * 32 + tile_x) & VRAM_MASK];
            let offset = section + usize::from(name) * 8 + row;
            let pattern = vram[(pattern_base + offset) & VRAM_MASK];
            let color = vram[(color_base + offset) & VRAM_MASK];
            let foreground = if color >> 4 != 0 {
                color >> 4
            } else {
                backdrop
            };
            let background = if color & 0x0F != 0 {
                color & 0x0F
            } else {
                backdrop
            };
            pattern_span(
                pixels,
                row_start + tile_x * 24,
                pattern,
                foreground,
                background,
                8,
            );
        }
    }
}

fn render_text(vram: &[u8], registers: &[u8], pixels: &mut [u8], backdrop: u8) {
    let name_base = usize::from(registers[2] & 0x0F) << 10;
    let pattern_base = usize::from(registers[4] & 0x07) << 11;
    let foreground = if registers[7] >> 4 != 0 {
        registers[7] >> 4
    } else {
        backdrop
    };
    for row in 0..24 {
        for column in 0..40 {
            let character = vram[(name_base + row * 40 + column) & VRAM_MASK];
            for line in 0..8 {
                let pattern = vram[(pattern_base + usize::from(character) * 8 + line) & VRAM_MASK];
                let row_start = (row * 8 + line) * SCREEN_WIDTH * 3;
                pattern_span(
                    pixels,
                    row_start + (8 + column * 6) * 3,
                    pattern,
                    foreground,
                    backdrop,
                    6,
                );
            }
        }
    }
}

fn render_multicolor(vram: &[u8], registers: &[u8], pixels: &mut [u8], backdrop: u8) {
    let name_base = usize::from(registers[2] & 0x0F) << 10;
    let pattern_base = usize::from(registers[4] & 0x07) << 11;
    for tile_y in 0..24 {
        for tile_x in 0..32 {
            let name = vram[(name_base + tile_y * 32 + tile_x) & VRAM_MASK];
            for local_y in 0..8 {
                let y = tile_y * 8 + local_y;
                let pattern_row = (y & 0x1C) >> 2;
                let pattern =
                    vram[(pattern_base + usize::from(name) * 8 + pattern_row) & VRAM_MASK];
                let left = if pattern >> 4 != 0 {
                    pattern >> 4
                } else {
                    backdrop
                };
                let right = if pattern & 0x0F != 0 {
                    pattern & 0x0F
                } else {
                    backdrop
                };
                let row_start = y * SCREEN_WIDTH * 3;
                color_quarter(pixels, row_start + tile_x * 24, left);
                color_quarter(pixels, row_start + tile_x * 24 + 12, right);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveSprite {
    x: i32,
    pattern_number: u8,
    color: u8,
    raster_y: i32,
}

fn sprite_line_active(
    vram: &[u8],
    registers: &[u8],
    line: i32,
    sprite_limit_enabled: bool,
) -> (Vec<ActiveSprite>, Option<usize>) {
    let attr_base = usize::from(registers[5] & 0x7F) << 7;
    let size: i32 = if registers[1] & 0x02 != 0 { 16 } else { 8 };
    let scale: i32 = if registers[1] & 0x01 != 0 { 2 } else { 1 };
    let mut active = Vec::new();
    let mut fifth_index = None;
    for index in 0..32 {
        let offset = (attr_base + index * 4) & VRAM_MASK;
        let raw_y = vram[offset];
        let mut x = i32::from(vram[offset + 1]);
        let pattern_number = vram[offset + 2];
        let color_byte = vram[offset + 3];
        if raw_y == 0xD0 {
            break;
        }
        let mut y = i32::from(raw_y.wrapping_add(1));
        if y >= 0xE0 {
            y -= 0x100;
        }
        let raster_y = y.rem_euclid(i32::from(LINES_PER_FRAME));
        let row = (line - raster_y).rem_euclid(i32::from(LINES_PER_FRAME));
        if row >= size * scale {
            continue;
        }
        // The overflow flag/index describe hardware evaluation, even when
        // the compatibility renderer is allowed to draw every sprite.
        if active.len() == 4 && fifth_index.is_none() {
            fifth_index = Some(index);
            if sprite_limit_enabled {
                break;
            }
        }
        if color_byte & 0x80 != 0 {
            x -= 32;
        }
        active.push(ActiveSprite {
            x,
            pattern_number,
            color: color_byte & 0x0F,
            raster_y,
        });
    }
    (active, fifth_index)
}

fn render_sprite_line(
    vram: &[u8],
    registers: &[u8],
    status: &mut u8,
    mut pixels: Option<&mut [u8]>,
    line: i32,
    update_status: bool,
    sprite_limit_enabled: bool,
) {
    let (active, fifth_index) = sprite_line_active(vram, registers, line, sprite_limit_enabled);
    if update_status
        && line < i32::from(VBLANK_LINE)
        && *status & 0x80 == 0
        && *status & 0x40 == 0
        && let Some(index) = fifth_index
    {
        *status = (*status & 0xA0) | 0x40 | index as u8;
    }
    let pattern_base = usize::from(registers[6] & 0x07) << 11;
    let size: usize = if registers[1] & 0x02 != 0 { 16 } else { 8 };
    let scale: usize = if registers[1] & 0x01 != 0 { 2 } else { 1 };
    let mut occupied = HashSet::new();
    let mut visible = HashSet::new();
    for sprite in active {
        let source_y =
            ((line - sprite.raster_y).rem_euclid(i32::from(LINES_PER_FRAME)) as usize) / scale;
        for source_x in 0..size {
            let tile = if size == 16 {
                usize::from(sprite.pattern_number & 0xFC) + (source_x / 8) * 2 + source_y / 8
            } else {
                usize::from(sprite.pattern_number)
            };
            let address = pattern_base + tile * 8 + (source_y & 7);
            let pattern = vram[address & VRAM_MASK];
            if pattern & (0x80 >> (source_x & 7)) == 0 {
                continue;
            }
            for dx in 0..scale {
                let px = sprite.x + (source_x * scale + dx) as i32;
                if !occupied.insert(px) && update_status {
                    *status |= 0x20;
                }
                if let Some(target) = pixels.as_deref_mut()
                    && (0..SCREEN_WIDTH as i32).contains(&px)
                    && sprite.color != 0
                    && !visible.contains(&px)
                {
                    put_pixel(target, px, line, sprite.color);
                    visible.insert(px);
                }
            }
        }
    }
}

fn render_sprites(
    vram: &[u8],
    registers: &[u8],
    status: &mut u8,
    pixels: &mut [u8],
    update_status: bool,
    sprite_limit_enabled: bool,
) {
    for line in 0..SCREEN_HEIGHT {
        render_sprite_line(
            vram,
            registers,
            status,
            Some(&mut *pixels),
            line as i32,
            update_status,
            sprite_limit_enabled,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Tms9918A, sprite_line_active};

    #[test]
    fn reset_preserves_vram_unless_explicitly_cleared() {
        let mut vdp = Tms9918A::new();
        vdp.vram[0x123] = 0xA5;
        vdp.reset(false);
        assert_eq!(vdp.vram[0x123], 0xA5);
        vdp.reset(true);
        assert_eq!(vdp.vram[0x123], 0);
    }

    #[test]
    fn control_latch_vram_read_ahead_and_address_wrap_are_deterministic() {
        let mut vdp = Tms9918A::new();

        // Register writes are committed only after the second control byte.
        vdp.write_port(0x81, 0x20);
        assert_eq!(vdp.registers[1], 0);
        vdp.write_port(0x81, 0x81);
        assert_eq!(vdp.registers[1], 0x20);

        // Set the VRAM write address to 0x3FFF, then cross the 14-bit boundary.
        vdp.write_port(0x81, 0xFF);
        vdp.write_port(0x81, 0x7F);
        assert_eq!(vdp.address(), 0x3FFF);
        vdp.write_port(0x80, 0xA5);
        assert_eq!(vdp.address(), 0);
        vdp.write_port(0x80, 0x3C);
        assert_eq!(vdp.address(), 1);

        // A read command prefetches the addressed byte; port reads return the
        // buffer first and then advance it to the following VRAM address.
        vdp.write_port(0x81, 0xFF);
        vdp.write_port(0x81, 0x3F);
        assert_eq!(vdp.address(), 0);
        assert_eq!(vdp.read_port(0x80), 0xA5);
        assert_eq!(vdp.read_port(0x80), 0x3C);
        assert_eq!(vdp.address(), 2);
    }

    #[test]
    fn enabled_vblank_interrupt_is_level_asserted_until_status_is_read() {
        let mut vdp = Tms9918A::new();
        vdp.write_port(0x81, 0x20);
        vdp.write_port(0x81, 0x81);
        assert!(!vdp.interrupt_pending());

        vdp.advance(228 * 192);
        assert_ne!(vdp.status() & 0x80, 0);
        assert!(vdp.interrupt_pending());

        let status = vdp.read_port(0x81);
        assert_ne!(status & 0x80, 0);
        assert_eq!(vdp.status(), 0x1F);
        assert!(!vdp.interrupt_pending());
    }

    #[test]
    fn supported_display_modes_match_fixed_pixels_and_raster_output() {
        let check_mode = |mut vdp: Tms9918A,
                          foreground_x: usize,
                          foreground: [u8; 3],
                          background_x: usize,
                          background: [u8; 3]| {
            let frame = vdp.render_frame();
            let foreground_offset = foreground_x * 3;
            let background_offset = background_x * 3;
            assert_eq!(
                &frame[foreground_offset..foreground_offset + 3],
                &foreground[..]
            );
            assert_eq!(
                &frame[background_offset..background_offset + 3],
                &background[..]
            );
            vdp.advance(228 * 192);
            assert_eq!(vdp.framebuffer(), frame.as_slice());
        };

        // Graphics I: pattern bits select foreground/background colors.
        let mut graphics_i = Tms9918A::new();
        graphics_i.registers[1] = 0x40;
        graphics_i.registers[2] = 0x01;
        graphics_i.registers[3] = 0x01;
        graphics_i.registers[4] = 0x01;
        graphics_i.registers[7] = 0x41;
        graphics_i.vram[0x400] = 1;
        graphics_i.vram[0x808] = 0x80;
        graphics_i.vram[0x40] = 0x42;
        check_mode(graphics_i, 0, [84, 85, 237], 1, [33, 200, 66]);

        // Text: the 40-column glyph area begins at x=8 and uses six pixels.
        let mut text = Tms9918A::new();
        text.registers[1] = 0x50;
        text.registers[2] = 0x01;
        text.registers[4] = 0x01;
        text.registers[7] = 0x41;
        text.vram[0x400] = 1;
        text.vram[0x808] = 0x80;
        check_mode(text, 8, [84, 85, 237], 9, [0, 0, 0]);

        // Multicolor: each nibble paints a four-pixel horizontal quarter.
        let mut multicolor = Tms9918A::new();
        multicolor.registers[1] = 0x48;
        multicolor.registers[2] = 0x01;
        multicolor.registers[4] = 0x01;
        multicolor.registers[7] = 0x01;
        multicolor.vram[0x400] = 1;
        multicolor.vram[0x808] = 0x42;
        check_mode(multicolor, 0, [84, 85, 237], 4, [33, 200, 66]);

        // Graphics II uses the pattern and color tables for the active section.
        let mut graphics_ii = Tms9918A::new();
        graphics_ii.registers[0] = 0x02;
        graphics_ii.registers[1] = 0x40;
        graphics_ii.registers[2] = 0x01;
        graphics_ii.registers[3] = 0x80;
        graphics_ii.registers[7] = 0x01;
        graphics_ii.vram[0x400] = 1;
        graphics_ii.vram[8] = 0x80;
        graphics_ii.vram[0x2008] = 0x42;
        check_mode(graphics_ii, 0, [84, 85, 237], 1, [33, 200, 66]);
    }

    #[test]
    fn transparent_and_offscreen_sprite_pixels_still_trigger_collision() {
        let mut transparent = Tms9918A::new();
        transparent.registers[1] = 0x40;
        transparent.registers[5] = 0x20;
        transparent.vram[0] = 0x80;
        transparent.vram[0x1000] = 0;
        transparent.vram[0x1001] = 0;
        transparent.vram[0x1002] = 0;
        transparent.vram[0x1003] = 0;
        transparent.vram[0x1004] = 0;
        transparent.vram[0x1005] = 0;
        transparent.vram[0x1006] = 0;
        transparent.vram[0x1007] = 4;
        transparent.vram[0x1008] = 0xD0;
        transparent.advance(228 * 2);
        assert_ne!(transparent.status() & 0x20, 0);

        let frame = transparent.render_frame();
        let first_visible_line = 256 * 3;
        assert_eq!(
            &frame[first_visible_line..first_visible_line + 3],
            &[84, 85, 237]
        );

        let mut offscreen = Tms9918A::new();
        offscreen.registers[1] = 0x40;
        offscreen.registers[5] = 0x20;
        offscreen.vram[0] = 0x80;
        for index in 0..2 {
            let offset = 0x1000 + index * 4;
            offscreen.vram[offset] = 0;
            offscreen.vram[offset + 1] = 0;
            offscreen.vram[offset + 2] = 0;
            offscreen.vram[offset + 3] = 0x80 | if index == 0 { 0 } else { 4 };
        }
        offscreen.vram[0x1000 + 2 * 4] = 0xD0;
        offscreen.advance(228 * 2);
        assert_ne!(offscreen.status() & 0x20, 0);
    }

    #[test]
    fn sprite_y_wrap_exposes_the_correct_pattern_row_at_the_top_edge() {
        let mut y_255 = Tms9918A::new();
        y_255.registers[1] = 0x40;
        y_255.registers[5] = 0x20;
        y_255.vram[0] = 0x80;
        y_255.vram[0x1000] = 0xFF;
        y_255.vram[0x1001] = 0;
        y_255.vram[0x1002] = 0;
        y_255.vram[0x1003] = 4;
        y_255.vram[0x1004] = 0xD0;
        y_255.advance(228 * 192);
        assert_eq!(&y_255.framebuffer()[..3], &[84, 85, 237]);

        let mut y_254 = Tms9918A::new();
        y_254.registers[1] = 0x40;
        y_254.registers[5] = 0x20;
        y_254.vram[0] = 0;
        y_254.vram[1] = 0x80;
        y_254.vram[0x1000] = 0xFE;
        y_254.vram[0x1001] = 0;
        y_254.vram[0x1002] = 0;
        y_254.vram[0x1003] = 4;
        y_254.vram[0x1004] = 0xD0;
        y_254.advance(228 * 192);
        assert_eq!(&y_254.framebuffer()[..3], &[84, 85, 237]);
    }

    #[test]
    fn disabling_sprite_limit_keeps_hardware_overflow_status_but_draws_fifth_sprite() {
        let mut vdp = Tms9918A::new();
        vdp.registers[1] = 0x40; // Display enabled; 8x8 sprites.
        vdp.registers[5] = 0x20; // Sprite attributes at 0x1000.
        vdp.vram[0] = 0xFF; // Opaque row for pattern zero.
        for index in 0..5 {
            let offset = 0x1000 + index * 4;
            vdp.vram[offset..offset + 4].copy_from_slice(&[0xFF, (index * 8) as u8, 0, 0x0F]);
        }
        vdp.vram[0x1000 + 5 * 4] = 0xD0;

        vdp.set_sprite_limit_enabled(false);
        vdp.advance(228);

        // The compatibility option changes drawing only, not observable status.
        assert_ne!(vdp.status() & 0x40, 0);
        assert_eq!(vdp.status() & 0x1F, 4);

        let frame = vdp.render_frame();
        let fifth_sprite_pixel = 32 * 3;
        assert_eq!(
            &frame[fifth_sprite_pixel..fifth_sprite_pixel + 3],
            &[255, 255, 255]
        );
    }

    #[test]
    fn sprite_overflow_and_collision_set_distinct_status_bits() {
        let mut overflow = Tms9918A::new();
        overflow.registers[1] = 0x40;
        overflow.registers[5] = 0x20;
        overflow.vram[0] = 0x80;
        for index in 0..5 {
            let offset = 0x1000 + index * 4;
            overflow.vram[offset] = 0;
            overflow.vram[offset + 1] = (index * 8) as u8;
            overflow.vram[offset + 2] = 0;
            overflow.vram[offset + 3] = 4;
        }
        overflow.vram[0x1000 + 5 * 4] = 0xD0;
        overflow.advance(228 * 2);
        assert_ne!(overflow.status() & 0x40, 0);
        assert_eq!(overflow.status() & 0x1F, 4);
        assert_eq!(overflow.status() & 0x20, 0);

        let mut collision = Tms9918A::new();
        collision.registers[1] = 0x40;
        collision.registers[5] = 0x20;
        collision.vram[0] = 0x80;
        for index in 0..2 {
            let offset = 0x1000 + index * 4;
            collision.vram[offset] = 0;
            collision.vram[offset + 1] = 0;
            collision.vram[offset + 2] = 0;
            collision.vram[offset + 3] = 4;
        }
        collision.vram[0x1000 + 2 * 4] = 0xD0;
        collision.advance(228 * 2);
        assert_ne!(collision.status() & 0x20, 0);
        assert_eq!(collision.status() & 0x40, 0);
    }

    #[test]
    fn vblank_and_frame_boundaries_follow_raster_clock() {
        let mut vdp = Tms9918A::new();
        vdp.advance(228 * 192 - 1);
        assert_eq!(vdp.scanline, 191);
        assert_eq!(vdp.status & 0x80, 0);
        vdp.advance(1);
        assert_eq!(vdp.scanline, 192);
        assert_ne!(vdp.status & 0x80, 0);
        assert!(vdp.consume_frame_ready());
        assert!(!vdp.consume_frame_ready());
        vdp.advance(228 * 70);
        assert_eq!(vdp.frame_number, 1);
        assert_eq!(vdp.scanline, 0);
    }

    #[test]
    fn sprite_limit_can_be_disabled_for_diagnostic_rendering() {
        let mut vdp = Tms9918A::new();
        for index in 0..5 {
            let offset = index * 4;
            vdp.vram[offset..offset + 4].copy_from_slice(&[0xFF, (index * 8) as u8, 0, 0x0F]);
        }

        let (active, fifth) = sprite_line_active(&vdp.vram, &vdp.registers, 0, true);
        assert_eq!(active.len(), 4);
        assert_eq!(fifth, Some(4));

        vdp.set_sprite_limit_enabled(false);
        let (active, fifth) =
            sprite_line_active(&vdp.vram, &vdp.registers, 0, vdp.sprite_limit_enabled());
        assert_eq!(active.len(), 5);
        assert_eq!(fifth, None);
    }
}
