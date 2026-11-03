//! Explicit Z80 M1 opcode-fetch cycle model.
//!
//! This models the externally visible T-state phases of an opcode fetch,
//! including the refresh sub-cycle and WAIT sampling points. It is deliberately
//! independent of the instruction decoder; the machine scheduler remains the
//! owner of elapsed time and calls `advance_tstate` once per T-state.

use serde::{Deserialize, Serialize};

/// Position within one M1 opcode-fetch cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80M1Phase {
    T1,
    T2,
    Wait,
    T3,
    T4,
    Complete,
}

/// Active-low Z80 control pins and the address presented during an M1 phase.
///
/// A `false` value means the corresponding active-low pin is asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80M1Pins {
    pub address: u16,
    pub m1_n: bool,
    pub mreq_n: bool,
    pub rd_n: bool,
    pub rfsh_n: bool,
    /// WAIT is sampled on the falling edge at the end of T2 and each Tw.
    pub wait_sample: bool,
    /// Opcode data is valid for sampling at the end of T3.
    pub opcode_sample: bool,
}

/// Serializable continuation for a single M1 cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Z80M1CycleState {
    pub address: u16,
    pub refresh_address: u16,
    pub refresh_after_m1: u8,
    pub phase: Z80M1Phase,
    pub wait_tstates: u32,
    pub opcode: Option<u8>,
}

/// Errors returned by the explicit M1 cycle model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80M1Error {
    AlreadyComplete,
    OpcodeSampleOutsideT3,
    OpcodeAlreadySampled,
    MissingOpcodeAtT3End,
    WaitCounterOverflow,
    InvalidState,
}

/// One resumable M1 fetch cycle.
///
/// The low seven bits of R increment once when the cycle is created; bit 7 is
/// preserved. The refresh address is I:R after that increment. The refresh
/// address is exposed during T4 while RFSH and MREQ are asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80M1Cycle {
    state: Z80M1CycleState,
}

impl Z80M1Cycle {
    /// Start M1 at `address`, using the CPU's current I and R registers.
    pub fn new(address: u16, i: u8, refresh: u8) -> Self {
        let refresh_after_m1 = (refresh & 0x80) | (refresh.wrapping_add(1) & 0x7F);
        Self {
            state: Z80M1CycleState {
                address,
                refresh_address: u16::from_be_bytes([i, refresh_after_m1]),
                refresh_after_m1,
                phase: Z80M1Phase::T1,
                wait_tstates: 0,
                opcode: None,
            },
        }
    }

    /// Current T-state phase.
    pub const fn phase(&self) -> Z80M1Phase {
        self.state.phase
    }

    /// R value to commit for this M1 fetch.
    pub const fn refresh_after_m1(&self) -> u8 {
        self.state.refresh_after_m1
    }

    /// Opcode sampled during T3, if available.
    pub const fn opcode(&self) -> Option<u8> {
        self.state.opcode
    }

    /// Number of inserted WAIT states in this M1 cycle.
    pub const fn wait_tstates(&self) -> u32 {
        self.state.wait_tstates
    }

    /// Current externally visible pin levels.
    pub const fn pins(&self) -> Z80M1Pins {
        match self.state.phase {
            Z80M1Phase::T1 => Z80M1Pins {
                address: self.state.address,
                m1_n: false,
                mreq_n: true,
                rd_n: true,
                rfsh_n: true,
                wait_sample: false,
                opcode_sample: false,
            },
            Z80M1Phase::T2 | Z80M1Phase::Wait | Z80M1Phase::T3 => Z80M1Pins {
                address: self.state.address,
                m1_n: false,
                mreq_n: false,
                rd_n: false,
                rfsh_n: true,
                wait_sample: matches!(self.state.phase, Z80M1Phase::T2 | Z80M1Phase::Wait),
                opcode_sample: matches!(self.state.phase, Z80M1Phase::T3),
            },
            Z80M1Phase::T4 => Z80M1Pins {
                address: self.state.refresh_address,
                m1_n: true,
                mreq_n: false,
                rd_n: true,
                rfsh_n: false,
                wait_sample: false,
                opcode_sample: false,
            },
            Z80M1Phase::Complete => Z80M1Pins {
                address: self.state.refresh_address,
                m1_n: true,
                mreq_n: true,
                rd_n: true,
                rfsh_n: true,
                wait_sample: false,
                opcode_sample: false,
            },
        }
    }

    /// Sample the opcode at the end of T3.
    pub fn sample_opcode(&mut self, opcode: u8) -> Result<(), Z80M1Error> {
        if self.state.phase != Z80M1Phase::T3 {
            return Err(Z80M1Error::OpcodeSampleOutsideT3);
        }
        if self.state.opcode.is_some() {
            return Err(Z80M1Error::OpcodeAlreadySampled);
        }
        self.state.opcode = Some(opcode);
        Ok(())
    }

    /// Advance one T-state. `wait_asserted` is sampled only at the end of T2
    /// and each Tw; asserting it at other phases has no effect.
    pub fn advance_tstate(&mut self, wait_asserted: bool) -> Result<Z80M1Phase, Z80M1Error> {
        self.state.phase = match self.state.phase {
            Z80M1Phase::T1 => Z80M1Phase::T2,
            Z80M1Phase::T2 if wait_asserted => {
                self.state.wait_tstates = self
                    .state
                    .wait_tstates
                    .checked_add(1)
                    .ok_or(Z80M1Error::WaitCounterOverflow)?;
                Z80M1Phase::Wait
            }
            Z80M1Phase::T2 => Z80M1Phase::T3,
            Z80M1Phase::Wait if wait_asserted => {
                self.state.wait_tstates = self
                    .state
                    .wait_tstates
                    .checked_add(1)
                    .ok_or(Z80M1Error::WaitCounterOverflow)?;
                Z80M1Phase::Wait
            }
            Z80M1Phase::Wait => Z80M1Phase::T3,
            Z80M1Phase::T3 if self.state.opcode.is_some() => Z80M1Phase::T4,
            Z80M1Phase::T3 => return Err(Z80M1Error::MissingOpcodeAtT3End),
            Z80M1Phase::T4 => Z80M1Phase::Complete,
            Z80M1Phase::Complete => return Err(Z80M1Error::AlreadyComplete),
        };
        Ok(self.state.phase)
    }

    /// Export the exact M1 continuation without advancing emulated time.
    pub const fn export_state(&self) -> Z80M1CycleState {
        self.state
    }

    /// Restore a continuation after validating phase/data invariants.
    pub fn restore_state(state: Z80M1CycleState) -> Result<Self, Z80M1Error> {
        let opcode_valid = match state.phase {
            Z80M1Phase::T1 | Z80M1Phase::T2 | Z80M1Phase::Wait => state.opcode.is_none(),
            Z80M1Phase::T3 => true,
            Z80M1Phase::T4 | Z80M1Phase::Complete => state.opcode.is_some(),
        };
        let refresh_valid = state.refresh_address as u8 == state.refresh_after_m1;
        if !opcode_valid || !refresh_valid {
            return Err(Z80M1Error::InvalidState);
        }
        Ok(Self { state })
    }
}

#[cfg(test)]
mod tests {
    use super::{Z80M1Cycle, Z80M1Error, Z80M1Phase};

    #[test]
    fn m1_exposes_fetch_then_refresh_pin_phases() {
        let mut cycle = Z80M1Cycle::new(0x1234, 0xAB, 0xFE);
        let t1 = cycle.pins();
        assert_eq!(t1.address, 0x1234);
        assert!(!t1.m1_n);
        assert!(t1.mreq_n && t1.rd_n && t1.rfsh_n);

        assert_eq!(cycle.advance_tstate(false).unwrap(), Z80M1Phase::T2);
        let t2 = cycle.pins();
        assert_eq!(t2.address, 0x1234);
        assert!(!t2.mreq_n && !t2.rd_n);
        assert!(t2.wait_sample);

        assert_eq!(cycle.advance_tstate(false).unwrap(), Z80M1Phase::T3);
        assert!(cycle.pins().opcode_sample);
        cycle.sample_opcode(0x3E).unwrap();

        assert_eq!(cycle.advance_tstate(false).unwrap(), Z80M1Phase::T4);
        let t4 = cycle.pins();
        assert_eq!(t4.address, 0xABFF);
        assert!(t4.m1_n && t4.rd_n);
        assert!(!t4.mreq_n && !t4.rfsh_n);

        assert_eq!(cycle.advance_tstate(false).unwrap(), Z80M1Phase::Complete);
        assert_eq!(cycle.opcode(), Some(0x3E));
        assert_eq!(cycle.refresh_after_m1(), 0xFF);
    }

    #[test]
    fn wait_is_sampled_at_t2_and_each_tw_until_released() {
        let mut cycle = Z80M1Cycle::new(0x0000, 0x00, 0x00);
        cycle.advance_tstate(false).unwrap();
        assert_eq!(cycle.advance_tstate(true).unwrap(), Z80M1Phase::Wait);
        assert_eq!(cycle.wait_tstates(), 1);
        assert!(cycle.pins().wait_sample);

        assert_eq!(cycle.advance_tstate(true).unwrap(), Z80M1Phase::Wait);
        assert_eq!(cycle.wait_tstates(), 2);
        assert_eq!(cycle.advance_tstate(true).unwrap(), Z80M1Phase::Wait);
        assert_eq!(cycle.wait_tstates(), 3);
        assert_eq!(cycle.advance_tstate(false).unwrap(), Z80M1Phase::T3);
        assert_eq!(cycle.wait_tstates(), 3);
    }

    #[test]
    fn refresh_increments_low_seven_bits_and_preserves_bit_seven() {
        let cycle = Z80M1Cycle::new(0, 0x5A, 0xFF);
        assert_eq!(cycle.refresh_after_m1(), 0x80);
        assert_eq!(cycle.pins().address, 0);
        let mut cycle = cycle;
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.sample_opcode(0).unwrap();
        cycle.advance_tstate(false).unwrap();
        assert_eq!(cycle.pins().address, 0x5A80);
    }

    #[test]
    fn opcode_can_only_be_sampled_once_during_t3() {
        let mut cycle = Z80M1Cycle::new(0, 0, 0);
        assert_eq!(cycle.sample_opcode(0), Err(Z80M1Error::OpcodeSampleOutsideT3));
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.sample_opcode(0x00).unwrap();
        assert_eq!(cycle.sample_opcode(0x01), Err(Z80M1Error::OpcodeAlreadySampled));
    }

    #[test]
    fn snapshot_restore_preserves_mid_wait_and_refresh_state() {
        let mut cycle = Z80M1Cycle::new(0x4567, 0x12, 0xFE);
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(true).unwrap();
        cycle.advance_tstate(true).unwrap();
        let restored = Z80M1Cycle::restore_state(cycle.export_state()).unwrap();
        assert_eq!(restored.phase(), Z80M1Phase::Wait);
        assert_eq!(restored.wait_tstates(), 2);
        assert_eq!(restored.pins(), cycle.pins());
    }

    #[test]
    fn t3_cannot_end_without_an_opcode_sample() {
        let mut cycle = Z80M1Cycle::new(0, 0, 0);
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(false).unwrap();
        assert_eq!(
            cycle.advance_tstate(false),
            Err(Z80M1Error::MissingOpcodeAtT3End)
        );
    }
}
