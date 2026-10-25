//! Rust implementation of the SG-1000 TMS9918A model.
//!
//! This is a behavioral port of the existing Python model. Timing remains in
//! nominal Z80 T-states; this does not model per-dot VRAM fetches or bus contention.
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};
use std::collections::HashSet;

const W: usize = 256;
const H: usize = 192;
const LINE_TSTATES: u32 = 228;
const FRAME_LINES: u16 = 262;
const VRAM_SIZE: usize = 0x4000;
const PALETTE: [[u8; 3]; 16] = [
    [0,0,0], [0,0,0], [33,200,66], [94,220,120], [84,85,237], [125,118,252],
    [212,82,77], [66,235,245], [252,85,84], [255,121,120], [212,193,84],
    [230,206,128], [33,176,59], [201,91,186], [204,204,204], [255,255,255],
];

#[pyclass(module = "sega_z80_core")]
pub struct RustTMS9918A {
    vram: Py<PyByteArray>,
    registers: Py<PyByteArray>,
    #[pyo3(get, set)] pub status: u8,
    #[pyo3(get, set)] pub address: u16,
    #[pyo3(get, set)] pub read_buffer: u8,
    control_first: Option<u8>,
    cycles_into_line: u32,
    #[pyo3(get, set)] pub scanline: u16,
    #[pyo3(get, set)] pub frame_number: u64,
    framebuffer: Vec<u8>,
    working: Vec<u8>,
    frame_ready: bool,
}
#[pymethods]
impl RustTMS9918A {
    #[new]
    fn new(py: Python<'_>) -> Self {
        Self {
            vram: PyByteArray::new(py, &vec![0; VRAM_SIZE]).unbind(),
            registers: PyByteArray::new(py, &[0; 8]).unbind(),
            status: 0x1f, address: 0, read_buffer: 0, control_first: None,
            cycles_into_line: 0, scanline: 0, frame_number: 0,
            framebuffer: vec![0; W*H*3], working: vec![0; W*H*3], frame_ready: false,
        }
    }
    #[getter]
    fn vram(&self, py: Python<'_>) -> Py<PyByteArray> { self.vram.clone_ref(py) }
    #[setter]
    fn set_vram(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let bytes: Vec<u8> = value.extract()?;
        if bytes.len() != VRAM_SIZE { return Err(PyTypeError::new_err("vram must contain 16384 bytes")); }
        unsafe { self.vram.bind(py).as_bytes_mut().copy_from_slice(&bytes); }
        Ok(())
    }
    #[getter]
    fn registers(&self, py: Python<'_>) -> Py<PyByteArray> { self.registers.clone_ref(py) }
    #[setter]
    fn set_registers(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let bytes: Vec<u8> = value.extract()?;
        if bytes.len() != 8 { return Err(PyTypeError::new_err("registers must contain 8 bytes")); }
        unsafe { self.registers.bind(py).as_bytes_mut().copy_from_slice(&bytes); }
        Ok(())
    }
    #[getter]
    fn framebuffer<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> { PyBytes::new(py, &self.framebuffer) }
    #[getter]
    fn _cycles_into_line(&self) -> u32 { self.cycles_into_line }
    #[getter]
    fn _control_first(&self) -> Option<u8> { self.control_first }
    #[getter]
    fn _display_mode(&self, py: Python<'_>) -> u8 {
        let r = self.regs(py); ((r[1] >> 4) & 1) | ((r[1] >> 2) & 2) | (((r[0] >> 1) & 1) << 2)
    }
    #[getter]
    fn interrupt_pending(&self, py: Python<'_>) -> bool {
        let r = self.regs(py); self.status & 0x80 != 0 && r[1] & 0x20 != 0
    }
    #[pyo3(signature = (clear_vram=false))]
    fn reset(&mut self, py: Python<'_>, clear_vram: bool) {
        { let mut regs = self.registers.bind(py); unsafe { regs.as_bytes_mut().fill(0); } }
        self.status = 0x1f; self.address = 0; self.read_buffer = 0; self.control_first = None;
        self.cycles_into_line = 0; self.scanline = 0; self.frame_number = 0;
        self.framebuffer.fill(0); self.working.fill(0); self.frame_ready = false;
        if clear_vram { unsafe { self.vram.bind(py).as_bytes_mut().fill(0); } }
    }
    fn read_port(&mut self, py: Python<'_>, port: u8) -> u8 {
        match port & 0xc1 {
            0x80 => {
                let value = self.read_buffer;
                self.read_buffer = self.vram_at(py, self.address as usize & 0x3fff);
                self.address = self.address.wrapping_add(1) & 0x3fff;
                self.control_first = None; value
            }
            0x81 => { let value = self.status; self.status = 0x1f; self.control_first = None; value }
            _ => 0xff,
        }
    }
    fn write_port(&mut self, py: Python<'_>, port: u8, value: u8) {
        match port & 0xc1 {
            0x80 => {
                let addr = self.address as usize & 0x3fff;
                self.set_vram_at(py, addr, value); self.read_buffer = value;
                self.address = ((addr as u16) + 1) & 0x3fff; self.control_first = None;
            }
            0x81 => {
                if let Some(first) = self.control_first.take() {
                    if value & 0x80 != 0 { self.set_reg(py, (value & 7) as usize, first); }
                    else {
                        self.address = ((((value & 0x3f) as u16) << 8) | first as u16) & 0x3fff;
                        if value & 0x40 == 0 {
                            self.read_buffer = self.vram_at(py, self.address as usize);
                            self.address = self.address.wrapping_add(1) & 0x3fff;
                        }
                    }
                } else { self.control_first = Some(value); }
            }
            _ => {}
        }
    }
    fn advance(&mut self, py: Python<'_>, tstates: i64) -> PyResult<()> {
        if tstates < 0 { return Err(PyValueError::new_err("tstates must be a non-negative integer")); }
        let mut remaining = tstates as u64;
        while remaining > 0 {
            let step = remaining.min((LINE_TSTATES - self.cycles_into_line) as u64) as u32;
            self.cycles_into_line += step; remaining -= step;
            if self.cycles_into_line == LINE_TSTATES {
                self.cycles_into_line = 0;
                let line = self.scanline;
                if line < H as u16 { self.render_scanline(py, line as usize, true); }
                else if line < FRAME_LINES && self.regs(py)[1] & 0x40 != 0 && self.display_mode(py) != 1 {
                    self.render_sprite_line(py, None, line as usize, true);
                }
                self.scanline += 1;
                if self.scanline == H as u16 {
                    self.status |= 0x80; self.frame_ready = true; self.framebuffer.clone_from(&self.working);
                }
                if self.scanline >= FRAME_LINES {
                    self.scanline = 0; self.frame_number += 1; self.working.fill(0);
                }
            }
        }
        Ok(())
    }
    fn consume_frame_ready(&mut self) -> bool { let v = self.frame_ready; self.frame_ready = false; v }
    fn render_frame(&mut self, py: Python<'_>) -> Bound<'_, PyBytes> {
        let backdrop = (self.regs(py)[7] & 0x0f) as usize;
        let mut pixels = vec![0; W*H*3];
        for p in pixels.chunks_exact_mut(3) { p.copy_from_slice(&PALETTE[backdrop]); }
        if self.regs(py)[1] & 0x40 != 0 {
            for y in 0..H { self.render_scanline_to(py, &mut pixels, y, false); }
        }
        PyBytes::new(py, &pixels)
    }
}
impl RustTMS9918A {
    fn regs(&self, py: Python<'_>) -> Vec<u8> { unsafe { self.registers.bind(py).as_bytes().to_vec() } }
    fn set_reg(&self, py: Python<'_>, index: usize, value: u8) { unsafe { self.registers.bind(py).as_bytes_mut()[index] = value; } }
    fn vram_at(&self, py: Python<'_>, a: usize) -> u8 { unsafe { self.vram.bind(py).as_bytes()[a & 0x3fff] } }
    fn set_vram_at(&self, py: Python<'_>, a: usize, v: u8) { unsafe { self.vram.bind(py).as_bytes_mut()[a & 0x3fff] = v; } }
    fn display_mode(&self, py: Python<'_>) -> u8 {
        let r=self.regs(py); ((r[1]>>4)&1)|((r[1]>>2)&2)|(((r[0]>>1)&1)<<2)
    }
    fn render_scanline(&mut self, py: Python<'_>, y: usize, status: bool) {
        // Temporarily move the framebuffer out to avoid aliasing &mut self
        // with a mutable borrow of one of its fields during rendering.
        let mut frame = std::mem::take(&mut self.working);
        self.render_scanline_to(py, &mut frame, y, status);
        self.working = frame;
    }
    fn render_scanline_to(&mut self, py: Python<'_>, pixels: &mut [u8], y: usize, update_status: bool) {
        let r = self.regs(py).to_vec();
        let backdrop = (r[7]&15) as usize;
        let row = y*W*3;
        if r[1]&0x40 == 0 {
            for p in pixels[row..row+W*3].chunks_exact_mut(3) { p.copy_from_slice(&PALETTE[backdrop]); }
            return;
        }
        let mode = self.display_mode(py);
        if mode == 1 {
            let name_base=((r[2]&15) as usize)<<10; let pattern_base=((r[4]&7) as usize)<<11;
            let fg=if r[7]>>4 != 0 {(r[7]>>4) as usize} else {backdrop};
            for col in 0..40 {
                let ch=self.vram_at(py,name_base+(y/8)*40+col);
                let pat=self.vram_at(py,pattern_base+ch as usize*8+y%8);
                for bit in 0..6 { let c=if pat&(0x80>>bit)!=0 {fg} else {backdrop}; let o=row+(8+col*6+bit)*3; pixels[o..o+3].copy_from_slice(&PALETTE[c]); }
            }
            return;
        }
        if mode == 2 {
            let name_base=((r[2]&15) as usize)<<10; let pattern_base=((r[4]&7) as usize)<<11;
            let ty=y/8; let rowpat=(y&0x1c)>>2;
            for tx in 0..32 {
                let n=self.vram_at(py,name_base+ty*32+tx);
                let p=self.vram_at(py,pattern_base+n as usize*8+rowpat);
                for x in 0..8 { let c=if x<4 {(p>>4)&15} else {p&15}; let c=if c==0 {backdrop} else {c as usize}; let o=row+(tx*8+x)*3; pixels[o..o+3].copy_from_slice(&PALETTE[c]); }
            }
        } else if mode == 4 {
            let nb=((r[2]&15) as usize)<<10; let pb=((r[4]&4) as usize)<<11; let cb=((r[3]&0x80) as usize)<<6;
            let off=((y&0xc0)<<5);
            for tx in 0..32 {
                let n=self.vram_at(py,nb+(y/8)*32+tx) as usize; let o=off+(n<<3)+(y&7);
                let p=self.vram_at(py,pb+o); let c=self.vram_at(py,cb+o);
                for x in 0..8 { let v=if p&(0x80>>x)!=0 {(c>>4)&15} else {c&15}; let v=if v==0 {backdrop} else {v as usize}; let q=row+(tx*8+x)*3; pixels[q..q+3].copy_from_slice(&PALETTE[v]); }
            }
        } else {
            let nb=((r[2]&15) as usize)<<10; let pb=((r[4]&7) as usize)<<11; let cb=((r[3] as usize)<<6)&0x3fc0;
            for tx in 0..32 {
                let n=self.vram_at(py,nb+(y/8)*32+tx) as usize; let p=self.vram_at(py,pb+n*8+y%8); let c=self.vram_at(py,cb+n/8);
                for x in 0..8 { let v=if p&(0x80>>x)!=0 {(c>>4)&15} else {c&15}; let v=if v==0 {backdrop} else {v as usize}; let q=row+(tx*8+x)*3; pixels[q..q+3].copy_from_slice(&PALETTE[v]); }
            }
        }
        if mode != 1 { self.render_sprite_line(py, Some(pixels), y, update_status); }
    }
    fn render_sprite_line(&mut self, py: Python<'_>, pixels: Option<&mut [u8]>, line: usize, update_status: bool) {
        let r=self.regs(py).to_vec(); let attr=((r[5]&0x7f) as usize)<<7;
        let size=if r[1]&2!=0 {16} else {8}; let scale=if r[1]&1!=0 {2} else {1};
        let mut active: Vec<(usize,i32,usize,usize,usize)> = Vec::new(); let mut fifth=None;
        for i in 0..32 {
            let o=(attr+i*4)&0x3fff; let raw=self.vram_at(py,o);
            if raw==0xd0 { break; }
            let mut sy=((raw as i32+1)&255) as i32; if sy>=0xe0 {sy-=256;}
            let sy=sy.rem_euclid(FRAME_LINES as i32) as usize;
            let dy=(line+FRAME_LINES as usize-sy)%FRAME_LINES as usize;
            if dy>=size*scale {continue;}
            if active.len()==4 {fifth=Some(i);break;}
            let mut x=self.vram_at(py,o+1) as i32; let pat=self.vram_at(py,o+2) as usize; let c=self.vram_at(py,o+3);
            if c&0x80!=0 {x-=32;}
            active.push((i,x,pat,(c&15) as usize,sy));
        }
        if update_status && line< H && fifth.is_some() && self.status&0xc0==0 {
            self.status=(self.status&0xa0)|0x40|fifth.unwrap() as u8;
        }
        let pb=((r[6]&7) as usize)<<11;
        let mut occupied: HashSet<i32> = HashSet::new();
        let mut visible: HashSet<usize> = HashSet::new();
        let mut pixels=pixels;
        for (_,x,pat,color,sy) in active {
            let source_y=((line+FRAME_LINES as usize-sy)%FRAME_LINES as usize)/scale;
            for sx in 0..size {
                let tile=if size==16 {(pat&0xfc)+(sx/8)*2+source_y/8} else {pat};
                let p=self.vram_at(py,pb+tile*8+(source_y&7));
                if p&(0x80>>(sx&7))==0 {continue;}
                for dx in 0..scale {
                    let px=x+(sx*scale+dx) as i32;
                    if !occupied.insert(px) && update_status { self.status |= 0x20; }
                    if px >= 0 && (px as usize) < W {
                        let px = px as usize;
                        if let Some(ref mut buf) = pixels {
                            if color != 0 && !visible.contains(&px) {
                                let o = (line * W + px) * 3;
                                buf[o..o+3].copy_from_slice(&PALETTE[color]);
                                visible.insert(px);
                            }
                        }
                    }
                }
            }
        }
    }
}
