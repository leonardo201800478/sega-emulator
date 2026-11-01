//! Semantic validation for the BASE opcode 0x23 (INC HL).
//!
//! Manufacturer reference: ZiLOG Z80 CPU User Manual UM008011-0816,
//! "INC ss", printed page 198: ss <- ss + 1, no condition bits affected,
//! one M-cycle and six T-states.
//!
//! Differential fixture: SingleStepTests/Z80 v1/23.json, vector "23 0000",
//! upstream commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7 (MIT).
//! The upstream README states that the corpus was translated from the Ares
//! Z80 core and corrected; it is useful regression evidence, not an
//! implementation-independent Ares oracle or hardware proof.

use sega_sms_core::{CpuBus, Z80};

struct FlatBus(Box<[u8]>);

impl Default for FlatBus {
    fn default() -> Self {
        Self(vec![0; 0x1_0000].into_boxed_slice())
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.0[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.0[usize::from(address)] = value;
    }
}

#[test]
fn base_inc_hl_matches_pinned_singlestep_vector_23_0000() {
    let mut bus = FlatBus::default();
    bus.0[0xFE2C] = 0x23;

    let mut cpu = Z80::default();
    cpu.registers.pc = 0xFE2C;
    cpu.registers.sp = 0x02D9;
    cpu.registers.a = 0xFC;
    cpu.registers.f = 0xCB;
    cpu.registers.set_bc(0xABC2);
    cpu.registers.set_de(0x8B89);
    cpu.registers.set_hl(0xBC4B);
    cpu.i = 0xCC;
    cpu.r = 0x36;
    cpu.ix = 0x4BCF;
    cpu.iy = 0x1CFF;
    cpu.wz = 0xC586;
    [cpu.alternate.a, cpu.alternate.f] = 0x7063_u16.to_be_bytes();
    [cpu.alternate.b, cpu.alternate.c] = 0x961F_u16.to_be_bytes();
    [cpu.alternate.d, cpu.alternate.e] = 0xAD37_u16.to_be_bytes();
    [cpu.alternate.h, cpu.alternate.l] = 0x9CF7_u16.to_be_bytes();
    cpu.interrupt_mode = 0;
    cpu.p = false;
    cpu.q = 0xCB;
    cpu.iff1 = true;
    cpu.iff2 = false;
    cpu.ei_delay = 1;

    assert_eq!(cpu.step(&mut bus), Ok(6));

    assert_eq!(cpu.registers.pc, 0xFE2D);
    assert_eq!(cpu.registers.sp, 0x02D9);
    assert_eq!(cpu.registers.a, 0xFC);
    assert_eq!(cpu.registers.f, 0xCB);
    assert_eq!(cpu.registers.bc(), 0xABC2);
    assert_eq!(cpu.registers.de(), 0x8B89);
    assert_eq!(cpu.registers.hl(), 0xBC4C);
    assert_eq!(cpu.i, 0xCC);
    assert_eq!(cpu.r, 0x37);
    assert_eq!(cpu.ix, 0x4BCF);
    assert_eq!(cpu.iy, 0x1CFF);
    assert_eq!(cpu.wz, 0xC586);
    assert_eq!(
        u16::from_be_bytes([cpu.alternate.a, cpu.alternate.f]),
        0x7063
    );
    assert_eq!(
        u16::from_be_bytes([cpu.alternate.b, cpu.alternate.c]),
        0x961F
    );
    assert_eq!(
        u16::from_be_bytes([cpu.alternate.d, cpu.alternate.e]),
        0xAD37
    );
    assert_eq!(
        u16::from_be_bytes([cpu.alternate.h, cpu.alternate.l]),
        0x9CF7
    );
    assert_eq!(cpu.interrupt_mode, 0);
    assert!(!cpu.p);
    assert_eq!(cpu.q, 0);
    assert!(cpu.iff1);
    assert!(!cpu.iff2);
    assert_eq!(cpu.ei_delay, 0);
    assert_eq!(cpu.total_tstates, 6);
    assert_eq!(bus.0[0xFE2C], 0x23);
}

#[test]
fn base_inc_hl_exhaustively_increments_every_u16_value() {
    let mut bus = FlatBus::default();
    bus.0[0x1000] = 0x23;

    for initial_hl in u16::MIN..=u16::MAX {
        let mut cpu = Z80::default();
        cpu.registers.pc = 0x1000;
        cpu.registers.sp = 0xA55A;
        cpu.registers.a = 0x3C;
        cpu.registers.f = 0xA5;
        cpu.registers.set_bc(0x1234);
        cpu.registers.set_de(0x5678);
        cpu.registers.set_hl(initial_hl);
        cpu.i = 0x5A;
        cpu.ix = 0x9ABC;
        cpu.iy = 0xDEF0;
        cpu.r = 0;

        assert_eq!(
            cpu.step(&mut bus),
            Ok(6),
            "INC HL timing for initial HL={initial_hl:04X}"
        );
        assert_eq!(
            cpu.registers.hl(),
            initial_hl.wrapping_add(1),
            "INC HL result for initial HL={initial_hl:04X}"
        );
        assert_eq!(
            cpu.registers.f, 0xA5,
            "INC HL must preserve F for initial HL={initial_hl:04X}"
        );
        assert_eq!(cpu.registers.pc, 0x1001);
        assert_eq!(cpu.registers.sp, 0xA55A);
        assert_eq!(cpu.registers.a, 0x3C);
        assert_eq!(cpu.registers.bc(), 0x1234);
        assert_eq!(cpu.registers.de(), 0x5678);
        assert_eq!(cpu.i, 0x5A);
        assert_eq!(cpu.ix, 0x9ABC);
        assert_eq!(cpu.iy, 0xDEF0);
        assert_eq!(cpu.r, 1);
        assert_eq!(cpu.total_tstates, 6);
    }
}

#[test]
fn base_inc_hl_preserves_all_flag_bytes_at_boundary_values() {
    let mut bus = FlatBus::default();
    bus.0[0x1000] = 0x23;
    let boundary_values = [0x0000, 0x0001, 0x7FFF, 0xFFFE, 0xFFFF];

    for flags in u8::MIN..=u8::MAX {
        for initial_hl in boundary_values {
            let mut cpu = Z80::default();
            cpu.registers.pc = 0x1000;
            cpu.registers.f = flags;
            cpu.registers.set_hl(initial_hl);

            assert_eq!(
                cpu.step(&mut bus),
                Ok(6),
                "INC HL timing for HL={initial_hl:04X}, F={flags:02X}"
            );
            assert_eq!(
                cpu.registers.hl(),
                initial_hl.wrapping_add(1),
                "INC HL result for HL={initial_hl:04X}, F={flags:02X}"
            );
            assert_eq!(
                cpu.registers.f, flags,
                "INC HL must preserve every F bit for HL={initial_hl:04X}"
            );
        }
    }
}
