//! Explicit, resumable Z80 M-cycle progress contract.
//!
//! The contract distinguishes nominal T-states from inserted WAIT states and
//! exposes every rising-edge boundary. It does not assign master-clock timing,
//! bus pin waveforms, or machine-specific arbitration; the owning machine maps
//! each returned edge onto its single scheduler timeline.

use serde::{Deserialize, Serialize};

/// Logical class of one Z80 machine cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80MCycleKind {
    OpcodeFetch,
    MemoryRead,
    MemoryWrite,
    IoRead,
    IoWrite,
    InterruptAcknowledge,
    Internal,
}

/// Origin of an inserted wait T-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80MCycleWaitSource {
    /// A wait T-state required by the selected CPU operation.
    Mandatory,
    /// A wait T-state inserted because WAIT was asserted at its sampling edge.
    External,
}

/// The T-state whose rising edge has just been reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Z80MCycleTState {
    /// A nominal T-state; index is zero-based within the M-cycle.
    Base { index: u8 },
    /// An inserted TW state; index is zero-based across all waits in this M-cycle.
    Wait {
        index: u32,
        source: Z80MCycleWaitSource,
    },
}

/// One completed rising-edge transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Z80MCycleEdge {
    /// Zero-based ordinal across nominal and inserted T-states.
    pub elapsed_tstate_index: u32,
    pub tstate: Z80MCycleTState,
    /// True only on the final nominal T-state edge of this M-cycle.
    pub mcycle_completed: bool,
}

/// Serializable continuation of one M-cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Z80MCycleState {
    pub kind: Z80MCycleKind,
    pub nominal_tstates: u8,
    pub completed_base_tstates: u8,
    pub fixed_wait_tstates: u32,
    pub fixed_wait_remaining: u32,
    pub external_wait_tstates: u32,
    pub elapsed_tstates: u32,
}

/// Errors from the M-cycle progress contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Z80MCycleError {
    InvalidTiming,
    InvalidState,
    AlreadyComplete,
    WaitOnInternalCycle,
    TStateOverflow,
}

/// One M-cycle whose state advances only when its owner dispatches a T-state edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z80MCycle {
    state: Z80MCycleState,
}

impl Z80MCycle {
    /// Start an M-cycle with an explicit nominal length and mandatory wait count.
    ///
    /// Mandatory waits are kept separate from externally sampled WAIT states.
    /// For bus cycles, WAIT is sampled after nominal T2 and remains sampleable
    /// on each inserted TW state. Internal cycles do not accept WAIT.
    pub fn new(
        kind: Z80MCycleKind,
        nominal_tstates: u8,
        fixed_wait_tstates: u32,
    ) -> Result<Self, Z80MCycleError> {
        if nominal_tstates == 0
            || (kind == Z80MCycleKind::Internal && fixed_wait_tstates != 0)
            || (kind != Z80MCycleKind::Internal && nominal_tstates < 3)
        {
            return Err(Z80MCycleError::InvalidTiming);
        }
        Ok(Self {
            state: Z80MCycleState {
                kind,
                nominal_tstates,
                completed_base_tstates: 0,
                fixed_wait_tstates,
                fixed_wait_remaining: fixed_wait_tstates,
                external_wait_tstates: 0,
                elapsed_tstates: 0,
            },
        })
    }

    /// Restore a saved M-cycle after validating all progress invariants.
    pub fn restore_state(state: Z80MCycleState) -> Result<Self, Z80MCycleError> {
        Self::validate_state(state)?;
        Ok(Self { state })
    }

    /// Export the exact resumable state without advancing emulated time.
    pub const fn export_state(self) -> Z80MCycleState {
        self.state
    }

    /// Whether the final nominal T-state has completed.
    pub const fn is_complete(self) -> bool {
        self.state.completed_base_tstates == self.state.nominal_tstates
    }

    /// Number of nominal T-states completed.
    pub const fn completed_base_tstates(self) -> u8 {
        self.state.completed_base_tstates
    }

    /// Number of inserted WAIT T-states completed.
    pub const fn wait_tstates(self) -> u32 {
        self.state.fixed_wait_tstates - self.state.fixed_wait_remaining
            + self.state.external_wait_tstates
    }

    /// Total elapsed T-states, including mandatory and external waits.
    pub const fn elapsed_tstates(self) -> u32 {
        self.state.elapsed_tstates
    }

    /// Advance exactly one T-state to its rising edge.
    ///
    /// wait_asserted is the WAIT level sampled for the next TW decision.
    /// It is consulted only after T2 and while the wait window remains open.
    /// The returned completion marker identifies the exact final M-cycle edge
    /// on which BUSREQ arbitration may later be attached.
    pub fn advance_tstate(&mut self, wait_asserted: bool) -> Result<Z80MCycleEdge, Z80MCycleError> {
        if self.is_complete() {
            return Err(Z80MCycleError::AlreadyComplete);
        }
        if self.state.kind == Z80MCycleKind::Internal && wait_asserted {
            return Err(Z80MCycleError::WaitOnInternalCycle);
        }

        let elapsed_tstate_index = self.state.elapsed_tstates;
        let mut next = self.state;

        if next.kind != Z80MCycleKind::Internal
            && next.nominal_tstates > 2
            && next.completed_base_tstates == 2
        {
            if next.fixed_wait_remaining > 0 {
                next.fixed_wait_remaining -= 1;
                next.elapsed_tstates = next
                    .elapsed_tstates
                    .checked_add(1)
                    .ok_or(Z80MCycleError::TStateOverflow)?;
                let wait_index = next
                    .fixed_wait_tstates
                    .checked_sub(next.fixed_wait_remaining)
                    .and_then(|count| count.checked_sub(1))
                    .and_then(|count| count.checked_add(next.external_wait_tstates))
                    .ok_or(Z80MCycleError::TStateOverflow)?;
                self.state = next;
                return Ok(Z80MCycleEdge {
                    elapsed_tstate_index,
                    tstate: Z80MCycleTState::Wait {
                        index: wait_index,
                        source: Z80MCycleWaitSource::Mandatory,
                    },
                    mcycle_completed: false,
                });
            }
            if wait_asserted {
                next.external_wait_tstates = next
                    .external_wait_tstates
                    .checked_add(1)
                    .ok_or(Z80MCycleError::TStateOverflow)?;
                next.elapsed_tstates = next
                    .elapsed_tstates
                    .checked_add(1)
                    .ok_or(Z80MCycleError::TStateOverflow)?;
                let wait_index = next
                    .fixed_wait_tstates
                    .checked_add(next.external_wait_tstates)
                    .and_then(|count| count.checked_sub(1))
                    .ok_or(Z80MCycleError::TStateOverflow)?;
                self.state = next;
                return Ok(Z80MCycleEdge {
                    elapsed_tstate_index,
                    tstate: Z80MCycleTState::Wait {
                        index: wait_index,
                        source: Z80MCycleWaitSource::External,
                    },
                    mcycle_completed: false,
                });
            }
        }

        let base_index = next.completed_base_tstates;
        next.completed_base_tstates += 1;
        next.elapsed_tstates = next
            .elapsed_tstates
            .checked_add(1)
            .ok_or(Z80MCycleError::TStateOverflow)?;
        let mcycle_completed = next.completed_base_tstates == next.nominal_tstates;
        self.state = next;
        Ok(Z80MCycleEdge {
            elapsed_tstate_index,
            tstate: Z80MCycleTState::Base { index: base_index },
            mcycle_completed,
        })
    }

    fn validate_state(state: Z80MCycleState) -> Result<(), Z80MCycleError> {
        if state.nominal_tstates == 0
            || state.completed_base_tstates > state.nominal_tstates
            || state.fixed_wait_remaining > state.fixed_wait_tstates
            || (state.kind == Z80MCycleKind::Internal
                && (state.nominal_tstates == 0
                    || state.fixed_wait_tstates != 0
                    || state.fixed_wait_remaining != 0
                    || state.external_wait_tstates != 0))
            || (state.kind != Z80MCycleKind::Internal && state.nominal_tstates < 3)
        {
            return Err(Z80MCycleError::InvalidState);
        }

        let fixed_waits_completed = state.fixed_wait_tstates - state.fixed_wait_remaining;
        let expected_elapsed = u32::from(state.completed_base_tstates)
            .checked_add(fixed_waits_completed)
            .and_then(|count| count.checked_add(state.external_wait_tstates))
            .ok_or(Z80MCycleError::InvalidState)?;
        if state.elapsed_tstates != expected_elapsed
            || (state.completed_base_tstates < 2
                && (fixed_waits_completed != 0 || state.external_wait_tstates != 0))
            || (state.fixed_wait_remaining > 0 && state.external_wait_tstates != 0)
            || (state.completed_base_tstates > 2 && state.fixed_wait_remaining != 0)
            || (state.completed_base_tstates == state.nominal_tstates
                && state.fixed_wait_remaining != 0)
        {
            return Err(Z80MCycleError::InvalidState);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Z80MCycle, Z80MCycleError, Z80MCycleKind, Z80MCycleTState, Z80MCycleWaitSource};

    #[test]
    fn memory_cycle_exposes_each_nominal_tstate_and_exact_final_edge() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::MemoryRead, 3, 0).unwrap();
        for index in 0..3 {
            let edge = cycle.advance_tstate(false).unwrap();
            assert_eq!(edge.tstate, Z80MCycleTState::Base { index });
            assert_eq!(edge.elapsed_tstate_index, u32::from(index));
            assert_eq!(edge.mcycle_completed, index == 2);
        }
        assert!(cycle.is_complete());
        assert_eq!(cycle.elapsed_tstates(), 3);
        assert_eq!(
            cycle.advance_tstate(false),
            Err(Z80MCycleError::AlreadyComplete)
        );
    }

    #[test]
    fn external_wait_is_inserted_after_t2_and_sampled_on_each_tw() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::MemoryRead, 3, 0).unwrap();
        assert_eq!(
            cycle.advance_tstate(false).unwrap().tstate,
            Z80MCycleTState::Base { index: 0 }
        );
        assert_eq!(
            cycle.advance_tstate(false).unwrap().tstate,
            Z80MCycleTState::Base { index: 1 }
        );
        for index in 0..2 {
            let edge = cycle.advance_tstate(true).unwrap();
            assert_eq!(
                edge.tstate,
                Z80MCycleTState::Wait {
                    index,
                    source: Z80MCycleWaitSource::External,
                }
            );
            assert!(!edge.mcycle_completed);
        }
        let final_edge = cycle.advance_tstate(false).unwrap();
        assert_eq!(final_edge.tstate, Z80MCycleTState::Base { index: 2 });
        assert!(final_edge.mcycle_completed);
        assert_eq!(cycle.wait_tstates(), 2);
        assert_eq!(cycle.elapsed_tstates(), 5);
    }

    #[test]
    fn mandatory_waits_and_external_waits_remain_distinguishable() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::InterruptAcknowledge, 4, 2).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(false).unwrap();
        for index in 0..2 {
            assert_eq!(
                cycle.advance_tstate(true).unwrap().tstate,
                Z80MCycleTState::Wait {
                    index,
                    source: Z80MCycleWaitSource::Mandatory,
                }
            );
        }
        assert_eq!(
            cycle.advance_tstate(true).unwrap().tstate,
            Z80MCycleTState::Wait {
                index: 2,
                source: Z80MCycleWaitSource::External,
            }
        );
        assert_eq!(
            cycle.advance_tstate(false).unwrap().tstate,
            Z80MCycleTState::Base { index: 2 }
        );
        assert_eq!(
            cycle.advance_tstate(false).unwrap().tstate,
            Z80MCycleTState::Base { index: 3 }
        );
        assert!(cycle.is_complete());
        assert_eq!(cycle.wait_tstates(), 3);
        assert_eq!(cycle.elapsed_tstates(), 7);
    }

    #[test]
    fn internal_cycles_do_not_accept_wait() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::Internal, 1, 0).unwrap();
        let before = cycle.export_state();
        assert_eq!(
            cycle.advance_tstate(true),
            Err(Z80MCycleError::WaitOnInternalCycle)
        );
        assert_eq!(cycle.export_state(), before);
        assert!(cycle.advance_tstate(false).unwrap().mcycle_completed);
    }

    #[test]
    fn snapshot_restore_preserves_wait_window_and_exact_next_edge() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::OpcodeFetch, 4, 0).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(true).unwrap();
        let snapshot = cycle.export_state();
        let mut restored = Z80MCycle::restore_state(snapshot).unwrap();
        assert_eq!(restored.export_state(), snapshot);
        assert_eq!(
            restored.advance_tstate(false).unwrap(),
            cycle.advance_tstate(false).unwrap()
        );
        assert_eq!(restored.export_state(), cycle.export_state());
    }

    #[test]
    fn snapshot_state_round_trips_through_the_versioned_serde_codec() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::InterruptAcknowledge, 4, 2).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(false).unwrap();
        cycle.advance_tstate(true).unwrap();
        let snapshot = cycle.export_state();
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        let decoded = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, snapshot);
        assert_eq!(
            Z80MCycle::restore_state(decoded).unwrap().export_state(),
            snapshot
        );
    }

    #[test]
    fn malformed_snapshots_are_rejected() {
        let mut cycle = Z80MCycle::new(Z80MCycleKind::MemoryRead, 3, 1).unwrap();
        cycle.advance_tstate(false).unwrap();
        let mut invalid = cycle.export_state();
        invalid.elapsed_tstates = 99;
        assert_eq!(
            Z80MCycle::restore_state(invalid),
            Err(Z80MCycleError::InvalidState)
        );
    }

    #[test]
    fn invalid_cycle_shapes_are_rejected() {
        assert_eq!(
            Z80MCycle::new(Z80MCycleKind::MemoryRead, 2, 0).err(),
            Some(Z80MCycleError::InvalidTiming)
        );
        assert_eq!(
            Z80MCycle::new(Z80MCycleKind::Internal, 1, 1).err(),
            Some(Z80MCycleError::InvalidTiming)
        );
    }
}
