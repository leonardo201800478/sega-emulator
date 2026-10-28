//! Curated differential vectors from SingleStepTests/Z80.
//!
//! Source: https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1
//! Upstream commit: ebe1875d48f374bcfd4b505d8eb8ee751568b5f7 (MIT).
//! These are the first fixed vectors from 00.json, 06.json, cb%2000.json,
//! and ed%2044.json. They compare architectural state and aggregate T-states;
//! they do not compare the ordering of individual bus cycles.

use sega_sms_core::{CpuBus, Z80};

#[derive(Clone, Copy)]
struct State {
    pc: u16,
    sp: u16,
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    i: u8,
    r: u8,
    ix: u16,
    iy: u16,
    wz: u16,
    af_alt: u16,
    bc_alt: u16,
    de_alt: u16,
    hl_alt: u16,
    im: u8,
    p: bool,
    q: u8,
    iff1: bool,
    iff2: bool,
    ei_delay: u8,
}

impl State {
    fn apply(self, cpu: &mut Z80) {
        cpu.registers.pc = self.pc;
        cpu.registers.sp = self.sp;
        cpu.registers.a = self.a;
        cpu.registers.f = self.f;
        cpu.registers.b = self.b;
        cpu.registers.c = self.c;
        cpu.registers.d = self.d;
        cpu.registers.e = self.e;
        cpu.registers.h = self.h;
        cpu.registers.l = self.l;
        cpu.i = self.i;
        cpu.r = self.r;
        cpu.ix = self.ix;
        cpu.iy = self.iy;
        cpu.wz = self.wz;
        [cpu.alternate.a, cpu.alternate.f] = self.af_alt.to_be_bytes();
        [cpu.alternate.b, cpu.alternate.c] = self.bc_alt.to_be_bytes();
        [cpu.alternate.d, cpu.alternate.e] = self.de_alt.to_be_bytes();
        [cpu.alternate.h, cpu.alternate.l] = self.hl_alt.to_be_bytes();
        cpu.interrupt_mode = self.im;
        cpu.p = self.p;
        cpu.q = self.q;
        cpu.iff1 = self.iff1;
        cpu.iff2 = self.iff2;
        cpu.ei_delay = self.ei_delay;
    }

    fn assert_matches(self, cpu: &Z80, vector: &str) {
        macro_rules! check {
            ($field:expr, $expected:expr, $name:literal) => {
                assert_eq!($field, $expected, "{vector}: {}", $name);
            };
        }
        check!(cpu.registers.pc, self.pc, "PC");
        check!(cpu.registers.sp, self.sp, "SP");
        check!(cpu.registers.a, self.a, "A");
        check!(cpu.registers.f, self.f, "F");
        check!(cpu.registers.b, self.b, "B");
        check!(cpu.registers.c, self.c, "C");
        check!(cpu.registers.d, self.d, "D");
        check!(cpu.registers.e, self.e, "E");
        check!(cpu.registers.h, self.h, "H");
        check!(cpu.registers.l, self.l, "L");
        check!(cpu.i, self.i, "I");
        check!(cpu.r, self.r, "R");
        check!(cpu.ix, self.ix, "IX");
        check!(cpu.iy, self.iy, "IY");
        check!(cpu.wz, self.wz, "WZ");
        check!(
            u16::from_be_bytes([cpu.alternate.a, cpu.alternate.f]),
            self.af_alt,
            "AF'"
        );
        check!(
            u16::from_be_bytes([cpu.alternate.b, cpu.alternate.c]),
            self.bc_alt,
            "BC'"
        );
        check!(
            u16::from_be_bytes([cpu.alternate.d, cpu.alternate.e]),
            self.de_alt,
            "DE'"
        );
        check!(
            u16::from_be_bytes([cpu.alternate.h, cpu.alternate.l]),
            self.hl_alt,
            "HL'"
        );
        check!(cpu.interrupt_mode, self.im, "IM");
        check!(cpu.p, self.p, "P latch");
        check!(cpu.q, self.q, "Q latch");
        check!(cpu.iff1, self.iff1, "IFF1");
        check!(cpu.iff2, self.iff2, "IFF2");
        check!(cpu.ei_delay, self.ei_delay, "EI delay");
    }
}

#[derive(Default)]
struct FlatBus {
    memory: Box<[u8; 0x1_0000]>,
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

fn run_vector(name: &str, initial: State, expected: State, bytes: &[(u16, u8)], tstates: u32) {
    let mut bus = FlatBus::default();
    for &(address, value) in bytes {
        bus.memory[usize::from(address)] = value;
    }
    let mut cpu = Z80::default();
    initial.apply(&mut cpu);
    assert_eq!(cpu.step(&mut bus), Ok(tstates), "{name}: T-states");
    assert_eq!(
        cpu.total_tstates,
        u64::from(tstates),
        "{name}: accumulated T-states"
    );
    expected.assert_matches(&cpu, name);
}

#[test]
fn singlestep_00_nop_vector_0000() {
    run_vector(
        "SingleStepTests 00 0000",
        State {
            pc: 19935,
            sp: 59438,
            a: 110,
            f: 250,
            b: 185,
            c: 144,
            d: 208,
            e: 190,
            h: 131,
            l: 147,
            i: 166,
            r: 16,
            ix: 35859,
            iy: 45708,
            wz: 62861,
            af_alt: 30257,
            bc_alt: 17419,
            de_alt: 13842,
            hl_alt: 28289,
            im: 0,
            p: true,
            q: 0,
            iff1: true,
            iff2: true,
            ei_delay: 1,
        },
        State {
            pc: 19936,
            sp: 59438,
            a: 110,
            f: 250,
            b: 185,
            c: 144,
            d: 208,
            e: 190,
            h: 131,
            l: 147,
            i: 166,
            r: 17,
            ix: 35859,
            iy: 45708,
            wz: 62861,
            af_alt: 30257,
            bc_alt: 17419,
            de_alt: 13842,
            hl_alt: 28289,
            im: 0,
            p: false,
            q: 0,
            iff1: true,
            iff2: true,
            ei_delay: 0,
        },
        &[(19935, 0)],
        4,
    );
}

#[test]
fn singlestep_06_ld_b_immediate_vector_0000() {
    run_vector(
        "SingleStepTests 06 0000",
        State {
            pc: 4891,
            sp: 53014,
            a: 22,
            f: 57,
            b: 255,
            c: 116,
            d: 94,
            e: 3,
            h: 213,
            l: 176,
            i: 179,
            r: 76,
            ix: 19560,
            iy: 41603,
            wz: 44966,
            af_alt: 35379,
            bc_alt: 36097,
            de_alt: 29623,
            hl_alt: 54766,
            im: 1,
            p: true,
            q: 57,
            iff1: false,
            iff2: true,
            ei_delay: 1,
        },
        State {
            pc: 4893,
            sp: 53014,
            a: 22,
            f: 57,
            b: 51,
            c: 116,
            d: 94,
            e: 3,
            h: 213,
            l: 176,
            i: 179,
            r: 77,
            ix: 19560,
            iy: 41603,
            wz: 44966,
            af_alt: 35379,
            bc_alt: 36097,
            de_alt: 29623,
            hl_alt: 54766,
            im: 1,
            p: false,
            q: 0,
            iff1: false,
            iff2: true,
            ei_delay: 0,
        },
        &[(4891, 0x06), (4892, 0x33)],
        7,
    );
}

#[test]
fn singlestep_cb_00_rlc_b_vector_0000() {
    run_vector(
        "SingleStepTests CB 00 0000",
        State {
            pc: 51223,
            sp: 58464,
            a: 33,
            f: 234,
            b: 189,
            c: 39,
            d: 55,
            e: 109,
            h: 80,
            l: 163,
            i: 160,
            r: 69,
            ix: 35319,
            iy: 52854,
            wz: 51741,
            af_alt: 56319,
            bc_alt: 36638,
            de_alt: 60073,
            hl_alt: 62241,
            im: 1,
            p: true,
            q: 234,
            iff1: true,
            iff2: false,
            ei_delay: 1,
        },
        State {
            pc: 51225,
            sp: 58464,
            a: 33,
            f: 45,
            b: 123,
            c: 39,
            d: 55,
            e: 109,
            h: 80,
            l: 163,
            i: 160,
            r: 71,
            ix: 35319,
            iy: 52854,
            wz: 51741,
            af_alt: 56319,
            bc_alt: 36638,
            de_alt: 60073,
            hl_alt: 62241,
            im: 1,
            p: false,
            q: 45,
            iff1: true,
            iff2: false,
            ei_delay: 0,
        },
        &[(51223, 0xCB), (51224, 0x00)],
        8,
    );
}

#[test]
fn singlestep_ed_44_neg_vector_0000() {
    run_vector(
        "SingleStepTests ED 44 0000",
        State {
            pc: 43025,
            sp: 52678,
            a: 82,
            f: 255,
            b: 48,
            c: 46,
            d: 5,
            e: 71,
            h: 174,
            l: 0,
            i: 85,
            r: 49,
            ix: 11136,
            iy: 5985,
            wz: 15580,
            af_alt: 5045,
            bc_alt: 29769,
            de_alt: 17952,
            hl_alt: 52923,
            im: 0,
            p: false,
            q: 0,
            iff1: false,
            iff2: true,
            ei_delay: 0,
        },
        State {
            pc: 43027,
            sp: 52678,
            a: 174,
            f: 187,
            b: 48,
            c: 46,
            d: 5,
            e: 71,
            h: 174,
            l: 0,
            i: 85,
            r: 51,
            ix: 11136,
            iy: 5985,
            wz: 15580,
            af_alt: 5045,
            bc_alt: 29769,
            de_alt: 17952,
            hl_alt: 52923,
            im: 0,
            p: false,
            q: 187,
            iff1: false,
            iff2: true,
            ei_delay: 0,
        },
        &[(43025, 0xED), (43026, 0x44)],
        8,
    );
}
