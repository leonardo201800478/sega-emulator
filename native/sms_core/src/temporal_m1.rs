//! Scheduler adapter for advancing one explicit Z80 M1 T-state at a time.
//!
//! This adapter uses the SMS coordinator's existing timeline. It does not own
//! or create a scheduler, perform memory access, or replace the CPU sequencer.
//! The machine event loop dispatches the scheduled event and then passes its
//! payload to `complete_tstate`, keeping the M1 pins and machine clock aligned.

use crate::{SmsTemporalCoordinator, SmsTemporalCoordinatorError, SmsTemporalEvent};
use sega_z80_cpu::{Z80M1Cycle, Z80M1CycleState, Z80M1Error, Z80M1Phase, Z80M1Pins};

/// Serializable continuation for one scheduler-driven M1 cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsTemporalM1State {
    pub cycle: Z80M1CycleState,
    pub next_continuation_id: u64,
    pub pending_continuation_id: Option<u64>,
}

/// Errors raised while advancing an M1 cycle on the SMS machine timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemporalM1Error {
    Coordinator(SmsTemporalCoordinatorError),
    Cycle(Z80M1Error),
    EventMismatch,
    ContinuationIdExhausted,
    CycleComplete,
    EventAlreadyPending,
    NoEventPending,
}

impl From<SmsTemporalCoordinatorError> for SmsTemporalM1Error {
    fn from(error: SmsTemporalCoordinatorError) -> Self {
        Self::Coordinator(error)
    }
}

impl From<Z80M1Error> for SmsTemporalM1Error {
    fn from(error: Z80M1Error) -> Self {
        Self::Cycle(error)
    }
}

/// One M1 pin cycle advanced only at dispatched scheduler T-state boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsTemporalM1 {
    cycle: Z80M1Cycle,
    next_continuation_id: u64,
    pending_continuation_id: Option<u64>,
}

impl SmsTemporalM1 {
    /// Begin an M1 cycle using the opcode address and current I/R register values.
    pub fn new(address: u16, i: u8, refresh: u8) -> Self {
        Self {
            cycle: Z80M1Cycle::new(address, i, refresh),
            next_continuation_id: 0,
            pending_continuation_id: None,
        }
    }

    /// Current M1 phase.
    pub const fn phase(&self) -> Z80M1Phase {
        self.cycle.phase()
    }

    /// Current active-low bus pin levels.
    pub const fn pins(&self) -> Z80M1Pins {
        self.cycle.pins()
    }

    /// Opcode sampled at the end of T3, if present.
    pub const fn opcode(&self) -> Option<u8> {
        self.cycle.opcode()
    }

    /// Updated R value to commit once the M1 cycle is complete.
    pub const fn refresh_after_m1(&self) -> u8 {
        self.cycle.refresh_after_m1()
    }

    /// Schedule the next M1 T-state on the shared SMS machine timeline.
    pub fn schedule_next_tstate(
        &mut self,
        coordinator: &mut SmsTemporalCoordinator,
    ) -> Result<(), SmsTemporalM1Error> {
        if self.phase() == Z80M1Phase::Complete {
            return Err(SmsTemporalM1Error::CycleComplete);
        }
        // Reject the irreversible scheduler event before it is queued if the
        // opcode has not been sampled; the T3 completion edge cannot be retried.
        if self.phase() == Z80M1Phase::T3 && self.opcode().is_none() {
            return Err(SmsTemporalM1Error::Cycle(
                Z80M1Error::MissingOpcodeAtT3End,
            ));
        }
        if self.pending_continuation_id.is_some() {
            return Err(SmsTemporalM1Error::EventAlreadyPending);
        }
        let next_id = self
            .next_continuation_id
            .checked_add(1)
            .ok_or(SmsTemporalM1Error::ContinuationIdExhausted)?;
        coordinator.schedule_m1_tstate(self.next_continuation_id)?;
        self.pending_continuation_id = Some(self.next_continuation_id);
        self.next_continuation_id = next_id;
        Ok(())
    }

    /// Apply a dispatched M1 boundary and sample WAIT at the modeled edge.
    ///
    /// The owning machine event loop must dispatch the scheduler first and pass
    /// the resulting event payload here. WAIT is meaningful only at T2/Tw,
    /// according to the underlying M1 pin model.
    pub fn complete_tstate(
        &mut self,
        event: SmsTemporalEvent,
        wait_asserted: bool,
    ) -> Result<Z80M1Phase, SmsTemporalM1Error> {
        let Some(expected_id) = self.pending_continuation_id else {
            return Err(SmsTemporalM1Error::NoEventPending);
        };
        if event
            != (SmsTemporalEvent::M1TStateComplete {
                continuation_id: expected_id,
            })
        {
            return Err(SmsTemporalM1Error::EventMismatch);
        }
        let phase = self.cycle.advance_tstate(wait_asserted)?;
        self.pending_continuation_id = None;
        Ok(phase)
    }

    /// Sample the opcode while the cycle is in T3.
    pub fn sample_opcode(&mut self, opcode: u8) -> Result<(), SmsTemporalM1Error> {
        self.cycle.sample_opcode(opcode)?;
        Ok(())
    }

    /// Export the exact continuation for a machine snapshot.
    pub const fn export_state(&self) -> SmsTemporalM1State {
        SmsTemporalM1State {
            cycle: self.cycle.export_state(),
            next_continuation_id: self.next_continuation_id,
            pending_continuation_id: self.pending_continuation_id,
        }
    }

    /// Restore a continuation after validating scheduler-event identity.
    pub fn restore_state(state: SmsTemporalM1State) -> Result<Self, SmsTemporalM1Error> {
        if state
            .pending_continuation_id
            .is_some_and(|id| id >= state.next_continuation_id)
            || (state.pending_continuation_id.is_some()
                && state.cycle.phase == Z80M1Phase::Complete)
        {
            return Err(SmsTemporalM1Error::EventMismatch);
        }
        let cycle = Z80M1Cycle::restore_state(state.cycle)?;
        Ok(Self {
            cycle,
            next_continuation_id: state.next_continuation_id,
            pending_continuation_id: state.pending_continuation_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsTemporalM1, SmsTemporalM1Error};
    use crate::{SmsTemporalCoordinator, SmsTemporalEvent};
    use sega_z80_cpu::Z80M1Phase;

    fn tick(
        m1: &mut SmsTemporalM1,
        coordinator: &mut SmsTemporalCoordinator,
        wait_asserted: bool,
    ) -> Z80M1Phase {
        m1.schedule_next_tstate(coordinator).unwrap();
        let dispatch = coordinator.dispatch_next().unwrap().unwrap();
        m1.complete_tstate(dispatch.event.payload, wait_asserted)
            .unwrap()
    }

    #[test]
    fn m1_fetch_refresh_and_opcode_sampling_follow_scheduler_boundaries() {
        let mut timeline = SmsTemporalCoordinator::new(3).unwrap();
        let mut m1 = SmsTemporalM1::new(0x1234, 0xAB, 0xFE);

        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::T2);
        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::T3);
        assert!(m1.pins().opcode_sample);
        m1.sample_opcode(0x3E).unwrap();
        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::T4);
        assert_eq!(m1.pins().address, 0xABFF);
        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::Complete);
        assert_eq!(m1.opcode(), Some(0x3E));
        assert_eq!(m1.refresh_after_m1(), 0xFF);
        assert_eq!(
            timeline.now(),
            sega_temporal_core::MachineTime::from_ticks(12)
        );
    }

    #[test]
    fn wait_states_are_individual_scheduler_events_and_snapshot_safe() {
        let mut timeline = SmsTemporalCoordinator::new(2).unwrap();
        let mut m1 = SmsTemporalM1::new(0, 0x20, 0x10);
        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::T2);
        assert_eq!(tick(&mut m1, &mut timeline, true), Z80M1Phase::Wait);

        m1.schedule_next_tstate(&mut timeline).unwrap();
        let snapshot = m1.export_state();
        let mut restored = SmsTemporalM1::restore_state(snapshot).unwrap();
        assert_eq!(
            restored.schedule_next_tstate(&mut timeline),
            Err(SmsTemporalM1Error::EventAlreadyPending)
        );
        let dispatch = timeline.dispatch_next().unwrap().unwrap();
        assert_eq!(
            restored
                .complete_tstate(dispatch.event.payload, true)
                .unwrap(),
            Z80M1Phase::Wait
        );
        assert_eq!(tick(&mut restored, &mut timeline, false), Z80M1Phase::T3);
        restored.sample_opcode(0x00).unwrap();
        assert_eq!(tick(&mut restored, &mut timeline, false), Z80M1Phase::T4);
        assert_eq!(
            tick(&mut restored, &mut timeline, false),
            Z80M1Phase::Complete
        );
        assert_eq!(timeline.z80_clock_snapshot().unwrap().total_cycles, 6);
    }

    #[test]
    fn t3_requires_opcode_before_scheduling_irreversible_completion() {
        let mut timeline = SmsTemporalCoordinator::new(1).unwrap();
        let mut m1 = SmsTemporalM1::new(0, 0, 0);
        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::T2);
        assert_eq!(tick(&mut m1, &mut timeline, false), Z80M1Phase::T3);
        assert_eq!(
            m1.schedule_next_tstate(&mut timeline),
            Err(SmsTemporalM1Error::Cycle(
                sega_z80_cpu::Z80M1Error::MissingOpcodeAtT3End
            ))
        );
        assert_eq!(timeline.now(), sega_temporal_core::MachineTime::from_ticks(2));
        assert_eq!(m1.phase(), Z80M1Phase::T3);
    }

    #[test]
    fn wrong_event_does_not_consume_pending_m1_continuation() {
        let mut timeline = SmsTemporalCoordinator::new(1).unwrap();
        let mut m1 = SmsTemporalM1::new(0, 0, 0);
        m1.schedule_next_tstate(&mut timeline).unwrap();
        assert_eq!(
            m1.complete_tstate(
                SmsTemporalEvent::InternalCycleComplete { continuation_id: 0 },
                false
            ),
            Err(SmsTemporalM1Error::EventMismatch)
        );
        assert_eq!(m1.phase(), Z80M1Phase::T1);
        assert_eq!(
            timeline.dispatch_next().unwrap().unwrap().event.payload,
            SmsTemporalEvent::M1TStateComplete { continuation_id: 0 }
        );
    }
}
