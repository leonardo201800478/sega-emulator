//! Shared architectural state for the atomic Z80 executor and resumable sequencer.
//!
//! This DTO contains emulated CPU state only. Host diagnostics, instruction
//! counters, trace buffers, and in-flight sequencer transactions are separate.

use crate::{Z80AlternateRegisters, Z80Registers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80ArchitecturalState {
    pub registers: Z80Registers,
    pub alternate: Z80AlternateRegisters,
    pub ix: u16,
    pub iy: u16,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: u8,
    pub wz: u16,
    pub ei_delay: u8,
    pub nmi_pending: bool,
    pub int_pending: Option<u8>,
    pub q: u8,
    pub q_previous: u8,
    pub p: bool,
    pub halted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80ArchitecturalStateError {
    InvalidInterruptMode(u8),
    InvalidEiDelay(u8),
}

impl Z80ArchitecturalState {
    /// Validate invariants required before a state is installed in either core.
    pub fn validate(self) -> Result<(), Z80ArchitecturalStateError> {
        if self.interrupt_mode > 2 {
            return Err(Z80ArchitecturalStateError::InvalidInterruptMode(
                self.interrupt_mode,
            ));
        }
        if self.ei_delay > 1 {
            return Err(Z80ArchitecturalStateError::InvalidEiDelay(self.ei_delay));
        }
        Ok(())
    }
}
