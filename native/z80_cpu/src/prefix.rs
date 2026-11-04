//! Explicit, resumable prefix-state transitions for the shared Z80 decoder.
//!
//! This layer classifies only prefix progression. Instruction semantics and
//! bus sequencing remain owned by the Z80 microsequencer.

use serde::{Deserialize, Serialize};

/// Index register selected by the most recent DD/FD prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80IndexRegister {
    Ix,
    Iy,
}

/// Prefix latch retained across M1 fetches and serialized in sequencer snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80PrefixState {
    None,
    Cb,
    Ed,
    Index(Z80IndexRegister),
}

/// Classification of the opcode immediately following an index prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80IndexOpcodeClass {
    /// The instruction substitutes HL/H/L/(HL) with IX/IY or indexed memory.
    UsesIndex,
    /// The instruction ignores DD/FD and retains its ordinary BASE meaning.
    Ignored,
    /// DD/FD-CB enters the separate displacement/subopcode path implemented in E5.
    IndexedCb,
}

/// Classify the effect of DD/FD on the following BASE opcode.
///
/// This classifies prefix semantics only; indexed handlers are implemented in E3-E5.
/// General execution of ignored-prefix fallbacks is implemented in E6.
pub fn classify_index_opcode(opcode: u8) -> Z80IndexOpcodeClass {
    if opcode == 0xCB {
        return Z80IndexOpcodeClass::IndexedCb;
    }

    if matches!(
        opcode,
        0x09 | 0x19
            | 0x21
            | 0x22
            | 0x23
            | 0x29
            | 0x2A
            | 0x2B
            | 0x39
            | 0xE1
            | 0xE3
            | 0xE5
            | 0xE9
            | 0xF9
    ) {
        return Z80IndexOpcodeClass::UsesIndex;
    }

    if (0x04..=0x3D).contains(&opcode) && matches!(opcode & 0xC7, 0x04..=0x06) {
        return if matches!((opcode >> 3) & 0x07, 4..=6) {
            Z80IndexOpcodeClass::UsesIndex
        } else {
            Z80IndexOpcodeClass::Ignored
        };
    }

    if (0x40..=0x7F).contains(&opcode) && opcode != 0x76 {
        let destination = (opcode >> 3) & 0x07;
        let source = opcode & 0x07;
        return if matches!(destination, 4..=6) || matches!(source, 4..=6) {
            Z80IndexOpcodeClass::UsesIndex
        } else {
            Z80IndexOpcodeClass::Ignored
        };
    }

    if (0x80..=0xBF).contains(&opcode) {
        return if matches!(opcode & 0x07, 4..=6) {
            Z80IndexOpcodeClass::UsesIndex
        } else {
            Z80IndexOpcodeClass::Ignored
        };
    }

    Z80IndexOpcodeClass::Ignored
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Z80PrefixTransition {
    Continue(Z80PrefixState),
    Execute {
        index: Option<Z80IndexRegister>,
        index_opcode_class: Option<Z80IndexOpcodeClass>,
    },
}

impl Z80PrefixState {
    /// Advance the prefix FSM after a completed opcode-fetch byte.
    ///
    /// Repeated/alternating DD/FD prefixes replace the selected index register;
    /// ED cancels index substitution. CB and ED are terminal prefix families.
    pub(crate) fn transition(self, opcode: u8) -> Z80PrefixTransition {
        match self {
            Self::None => match opcode {
                0xCB => Z80PrefixTransition::Continue(Self::Cb),
                0xED => Z80PrefixTransition::Continue(Self::Ed),
                0xDD => Z80PrefixTransition::Continue(Self::Index(Z80IndexRegister::Ix)),
                0xFD => Z80PrefixTransition::Continue(Self::Index(Z80IndexRegister::Iy)),
                _ => Z80PrefixTransition::Execute {
                    index: None,
                    index_opcode_class: None,
                },
            },
            Self::Index(index) => match opcode {
                0xDD => Z80PrefixTransition::Continue(Self::Index(Z80IndexRegister::Ix)),
                0xFD => Z80PrefixTransition::Continue(Self::Index(Z80IndexRegister::Iy)),
                0xED => Z80PrefixTransition::Continue(Self::Ed),
                _ => Z80PrefixTransition::Execute {
                    index: Some(index),
                    index_opcode_class: Some(classify_index_opcode(opcode)),
                },
            },
            Self::Cb | Self::Ed => Z80PrefixTransition::Execute {
                index: None,
                index_opcode_class: None,
            },
        }
    }
}
