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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Z80PrefixTransition {
    Continue(Z80PrefixState),
    Execute { index: Option<Z80IndexRegister> },
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
                _ => Z80PrefixTransition::Execute { index: None },
            },
            Self::Index(index) => match opcode {
                0xDD => Z80PrefixTransition::Continue(Self::Index(Z80IndexRegister::Ix)),
                0xFD => Z80PrefixTransition::Continue(Self::Index(Z80IndexRegister::Iy)),
                0xED => Z80PrefixTransition::Continue(Self::Ed),
                _ => Z80PrefixTransition::Execute { index: Some(index) },
            },
            Self::Cb | Self::Ed => Z80PrefixTransition::Execute { index: None },
        }
    }
}
