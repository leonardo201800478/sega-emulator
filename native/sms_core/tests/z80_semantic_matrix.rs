//! Executable classification matrix for all classic Z80 opcode spaces.
//!
//! This is a dispatch/classification gate, not proof of semantic conformance.
//! Semantic evidence is tracked separately in docs/architecture/z80-semantic-matrix.md.

use sega_sms_core::{CpuBus, Z80};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Space {
    Base,
    Cb,
    Ed,
    Dd,
    Fd,
    DdCb,
    FdCb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Classification {
    AssignedBaseInstruction,
    Prefix,
    AssignedCbInstruction,
    AssignedEdInstruction,
    UnassignedEdFallback,
    IndexSpaceCandidate,
    IndexedCbInstruction,
}

const SPACES: [Space; 7] = [
    Space::Base,
    Space::Cb,
    Space::Ed,
    Space::Dd,
    Space::Fd,
    Space::DdCb,
    Space::FdCb,
];

fn is_assigned_ed(opcode: u8) -> bool {
    matches!(
        opcode,
        0x40..=0x7F
            if matches!(
                opcode,
                0x40 | 0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x48 | 0x49
                    | 0x4A | 0x4B | 0x4C | 0x4D | 0x4E | 0x4F | 0x50 | 0x51 | 0x52
                    | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x58 | 0x59 | 0x5A | 0x5B
                    | 0x5C | 0x5D | 0x5E | 0x5F | 0x60 | 0x61 | 0x62 | 0x63 | 0x64
                    | 0x65 | 0x66 | 0x67 | 0x68 | 0x69 | 0x6A | 0x6B | 0x6C | 0x6D
                    | 0x6E | 0x6F | 0x70 | 0x71 | 0x72 | 0x73 | 0x74 | 0x75 | 0x76
                    | 0x77 | 0x78 | 0x79 | 0x7A | 0x7B | 0x7C | 0x7D | 0x7E | 0x7F
            )
    ) && opcode != 0x77 && opcode != 0x7F || matches!(
        opcode,
        0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA8 | 0xA9 | 0xAA | 0xAB | 0xB0 | 0xB1
            | 0xB2 | 0xB3 | 0xB8 | 0xB9 | 0xBA | 0xBB
    )
}

fn classify(space: Space, opcode: u8) -> Classification {
    match space {
        Space::Base => match opcode {
            0xCB | 0xDD | 0xED | 0xFD => Classification::Prefix,
            _ => Classification::AssignedBaseInstruction,
        },
        Space::Cb => Classification::AssignedCbInstruction,
        Space::Ed if is_assigned_ed(opcode) => Classification::AssignedEdInstruction,
        Space::Ed => Classification::UnassignedEdFallback,
        Space::Dd | Space::Fd => {
            // DD/FD can substitute IX/IY, be ignored, or introduce another
            // prefix. Per-opcode semantics are documented in the companion matrix.
            if matches!(opcode, 0xCB | 0xDD | 0xED | 0xFD) {
                Classification::Prefix
            } else {
                Classification::IndexSpaceCandidate
            }
        }
        Space::DdCb | Space::FdCb => Classification::IndexedCbInstruction,
    }
}

struct FlatBus {
    memory: Box<[u8]>,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

fn program_for(space: Space, opcode: u8, bus: &mut FlatBus) {
    match space {
        Space::Base => bus.memory[0] = opcode,
        Space::Cb => bus.memory[..2].copy_from_slice(&[0xCB, opcode]),
        Space::Ed => bus.memory[..2].copy_from_slice(&[0xED, opcode]),
        Space::Dd => bus.memory[..2].copy_from_slice(&[0xDD, opcode]),
        Space::Fd => bus.memory[..2].copy_from_slice(&[0xFD, opcode]),
        Space::DdCb => bus.memory[..4].copy_from_slice(&[0xDD, 0xCB, 0x00, opcode]),
        Space::FdCb => bus.memory[..4].copy_from_slice(&[0xFD, 0xCB, 0x00, opcode]),
    }
    // Stable operands keep a one-step dispatch probe deterministic. BC is
    // nonzero so block instructions exercise one iteration rather than wrap
    // from zero unless their own documented behavior requires it.
    bus.memory[0x8000] = 0xA5;
    bus.memory[0x8001] = 0x5A;
    bus.memory[0x8100] = 0x3C;
}

#[test]
fn every_encoding_in_every_opcode_space_has_an_explicit_classification() {
    let mut total = 0usize;
    let mut ed_assigned = 0usize;
    let mut ed_fallback = 0usize;

    for space in SPACES {
        for byte in u8::MIN..=u8::MAX {
            let classification = classify(space, byte);
            total += 1;
            match classification {
                Classification::AssignedEdInstruction => ed_assigned += 1,
                Classification::UnassignedEdFallback => ed_fallback += 1,
                _ => {}
            }
        }
    }

    assert_eq!(total, 7 * 256, "all opcode spaces must be enumerated");
    assert_eq!(ed_assigned, 78, "keep assigned ED encodings explicit");
    assert_eq!(
        ed_fallback, 178,
        "unassigned ED bytes must be classified as fallback"
    );
}

#[test]
fn every_opcode_space_encoding_executes_one_step_without_unsupported_opcode() {
    for space in SPACES {
        for opcode in u8::MIN..=u8::MAX {
            let mut bus = FlatBus::default();
            program_for(space, opcode, &mut bus);

            let mut cpu = Z80::default();
            cpu.registers.sp = 0x9000;
            cpu.registers.set_bc(0x0101);
            cpu.registers.set_de(0x8100);
            cpu.registers.set_hl(0x8000);
            cpu.ix = 0x8000;
            cpu.iy = 0x8000;

            let cycles = cpu.step(&mut bus).unwrap_or_else(|error| {
                panic!("{space:?} opcode {opcode:02X} was not dispatched: {error:?}")
            });
            assert!(cycles > 0, "{space:?} opcode {opcode:02X} returned zero T-states");
            assert_eq!(
                cpu.total_tstates, u64::from(cycles),
                "{space:?} opcode {opcode:02X} must account for its aggregate T-states"
            );
        }
    }
}
