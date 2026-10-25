//! Native TMS9918A-compatible VDP for the SG-1000.
use std::collections::HashSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};

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

#[pyclass(module = "sega_z80_core")]
pub(crate) struct NativeTms9918A {
    vram: Vec<u8>,
    registers: Vec<u8>,
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
}

#[pymethods]
impl NativeTms9918A {
    #[new]
    fn new() -> Self {
        Self {
            vram: vec![0; VRAM_SIZE],
            registers: vec![0; 8],
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
        }
    }

    #[getter]
    fn vram<'py>(&self, py: Python<'py>) -> Bound<'py, PyByteArray> {
        PyByteArray::new(py, &self.vram)
    }

    #[getter]
    fn registers<'py>(&self, py: Python<'py>) -> Bound<'py, PyByteArray> {
        PyByteArray::new(py, &self.registers)
    }

    #[getter]
    fn status(&self) -> u8 {
        self.status
    }

    #[getter]
    fn address(&self) -> u16 {
        self.address
    }

    #[getter]
    fn read_buffer(&self) -> u8 {
        self.read_buffer
    }

    #[getter]
    fn scanline(&self) -> u16 {
        self.scanline
    }

    #[getter(_cycles_into_line)]
    fn cycles_into_line(&self) -> u16 {
        self.cycles_into_line
    }

    #[getter]
    fn frame_number(&self) -> u64 {
        self.frame_number
    }

    #[getter]
    fn framebuffer<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.framebuffer)
    }

    #[getter(_display_mode)]
    fn display_mode(&self) -> u8 {
        display_mode(&self.registers)
    }

    #[getter]
    fn interrupt_pending(&self) -> bool {
        (self.status & 0x80 != 0) && (self.registers[1] & 0x20 != 0)
    }

    #[pyo3(signature = (*, clear_vram=false))]
    fn reset(&mut self, clear_vram: bool) -> PyResult<()> {
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
        Ok(())
    }

    fn read_port(&mut self, port: i64) -> u8 {
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

    fn write_port(&mut self, port: i64, value: i64) {
        let port = (port as u8) & 0xC1;
        let value = value as u8;
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

    fn advance(&mut self, tstates: i64) -> PyResult<()> {
        if tstates < 0 {
            return Err(PyValueError::new_err(
                "tstates deve ser um inteiro não negativo.",
            ));
        }
        let mut remaining = tstates as u64;
        let cycles_to_line_end = TSTATES_PER_LINE - u64::from(self.cycles_into_line);
        if remaining < cycles_to_line_end {
            self.cycles_into_line += remaining as u16;
            return Ok(());
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
        Ok(())
    }

    fn consume_frame_ready(&mut self) -> bool {
        let ready = self.frame_ready;
        self.frame_ready = false;
        ready
    }

    fn render_frame<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(
            py,
            &render_frame_bytes(&self.vram, &self.registers, self.status),
        )
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
    for pixel in pixels[start..start + SCREEN_WIDTH * 3].chunks_exact_mut(3) {
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
    );
}

fn render_frame_bytes(vram: &[u8], registers: &[u8], status: u8) -> Vec<u8> {
    let backdrop = registers[7] & 0x0F;
    let mut pixels = vec![0; RGB_FRAME_SIZE];
    for pixel in pixels.chunks_exact_mut(3) {
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
        render_sprites(vram, registers, &mut status_copy, &mut pixels, false);
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
        if active.len() == 4 {
            fifth_index = Some(index);
            break;
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
) {
    let (active, fifth_index) = sprite_line_active(vram, registers, line);
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
                if let Some(target) = pixels.as_deref_mut() {
                    if (0..SCREEN_WIDTH as i32).contains(&px)
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
}

fn render_sprites(
    vram: &[u8],
    registers: &[u8],
    status: &mut u8,
    pixels: &mut [u8],
    update_status: bool,
) {
    for line in 0..SCREEN_HEIGHT {
        render_sprite_line(
            vram,
            registers,
            status,
            Some(&mut *pixels),
            line as i32,
            update_status,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::NativeTms9918A;

    #[test]
    fn reset_preserves_vram_unless_explicitly_cleared() {
        let mut vdp = NativeTms9918A::new();
        vdp.vram[0x123] = 0xA5;
        vdp.reset(false).unwrap();
        assert_eq!(vdp.vram[0x123], 0xA5);
        vdp.reset(true).unwrap();
        assert_eq!(vdp.vram[0x123], 0);
    }

    #[test]
    fn vblank_and_frame_boundaries_follow_raster_clock() {
        let mut vdp = NativeTms9918A::new();
        vdp.advance(228 * 192 - 1).unwrap();
        assert_eq!(vdp.scanline, 191);
        assert_eq!(vdp.status & 0x80, 0);
        vdp.advance(1).unwrap();
        assert_eq!(vdp.scanline, 192);
        assert_ne!(vdp.status & 0x80, 0);
        assert!(vdp.consume_frame_ready());
        assert!(!vdp.consume_frame_ready());
        vdp.advance(228 * 70).unwrap();
        assert_eq!(vdp.frame_number, 1);
        assert_eq!(vdp.scanline, 0);
    }
}
