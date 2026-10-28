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
    IndexSensitive,
    IndexedMemoryUsesRealHl,
    IgnoredIndexPrefix,
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
    matches!(opcode, 0x40..=0x76 | 0x78..=0x7E)
        || matches!(
            opcode,
            0xA0..=0xA3 | 0xA8..=0xAB | 0xB0..=0xB3 | 0xB8..=0xBB
        )
}

fn is_index_sensitive(opcode: u8) -> bool {
    if matches!(
        opcode,
        0x09 | 0x19 | 0x21..=0x26 | 0x29..=0x2E | 0x34..=0x36 | 0x39
            | 0xE1 | 0xE3 | 0xE5 | 0xE9 | 0xF9
    ) {
        return true;
    }

    if (0x40..=0x7F).contains(&opcode) && opcode != 0x76 {
        let destination = (opcode >> 3) & 0x07;
        let source = opcode & 0x07;
        return matches!(destination, 4..=6) || matches!(source, 4..=6);
    }

    (0x80..=0xBF).contains(&opcode) && matches!(opcode & 0x07, 4..=6)
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
        Space::Dd | Space::Fd => match opcode {
            0xCB | 0xDD | 0xED | 0xFD => Classification::Prefix,
            // When indexed memory is paired with H/L, these forms use the
            // ordinary H/L registers rather than IXH/IXL or IYH/IYL.
            0x66 | 0x6E | 0x74 | 0x75 => Classification::IndexedMemoryUsesRealHl,
            _ if is_index_sensitive(opcode) => Classification::IndexSensitive,
            _ => Classification::IgnoredIndexPrefix,
        },
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
fn dd_fd_classification_distinguishes_indexed_forms_and_ignored_prefixes() {
    for space in [Space::Dd, Space::Fd] {
        let mut counts = [0usize; 4];
        for opcode in u8::MIN..=u8::MAX {
            match classify(space, opcode) {
                Classification::Prefix => {}
                Classification::IndexSensitive => counts[0] += 1,
                Classification::IndexedMemoryUsesRealHl => counts[1] += 1,
                Classification::IgnoredIndexPrefix => counts[2] += 1,
                _ => panic!("{space:?} opcode {opcode:02X} has an invalid classification"),
            }
        }

        assert_eq!(counts[0], 81, "{space:?}: index-sensitive encodings");
        assert_eq!(counts[1], 4, "{space:?}: H/L memory exceptions");
        assert_eq!(counts[2], 167, "{space:?}: ignored index prefixes");
    }

    assert_eq!(classify(Space::Dd, 0x65), Classification::IndexSensitive);
    assert_eq!(
        classify(Space::Fd, 0x66),
        Classification::IndexedMemoryUsesRealHl
    );
    assert_eq!(
        classify(Space::Dd, 0x74),
        Classification::IndexedMemoryUsesRealHl
    );
    assert_eq!(
        classify(Space::Fd, 0x41),
        Classification::IgnoredIndexPrefix
    );
    assert_eq!(
        classify(Space::Dd, 0x76),
        Classification::IgnoredIndexPrefix
    );
    assert_eq!(classify(Space::Dd, 0xCB), Classification::Prefix);
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
            assert!(
                cycles > 0,
                "{space:?} opcode {opcode:02X} returned zero T-states"
            );
            assert_eq!(
                cpu.total_tstates,
                u64::from(cycles),
                "{space:?} opcode {opcode:02X} must account for its aggregate T-states"
            );
        }
    }
}
