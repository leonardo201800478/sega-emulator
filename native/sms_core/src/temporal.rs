//! SMS machine-level temporal coordinator.
//!
//! This layer owns the machine timeline and schedules typed boundaries. It
//! deliberately does not execute devices or assign electrical bus phases.
//! The caller supplies the master-tick scale for one Z80 T-state; no regional
//! SMS clock ratio is inferred by this module.

use core::fmt;

use sega_machine_scheduler::{
    ClockDomainId, ClockDomainSnapshot, MachineDispatch, MachineScheduler, SchedulerError,
    SchedulerState,
};
use sega_temporal_core::{
    ClockRatio, EventPriority, HardwarePhase, MachineTime, PlatformPriority,
};

pub const SMS_TEMPORAL_COORDINATOR_STATE_VERSION: u16 = 1;
pub const SMS_Z80_CLOCK_DOMAIN_ID: ClockDomainId = ClockDomainId(0);

/// Typed machine event consumed by the SMS coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemporalEvent {
    BusCompletion { request_id: u64 },
}

/// Versioned in-memory state for the coordinator and its scheduler timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsTemporalCoordinatorState {
    pub format_version: u16,
    pub master_ticks_per_tstate: u64,
    pub scheduler: SchedulerState<SmsTemporalEvent>,
}

/// Errors raised while coordinating the SMS machine timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemporalCoordinatorError {
    ZeroTStateScale,
    ZeroBusCycleDuration,
    DurationOverflow,
    DeadlineOverflow,
    InvalidState,
    Scheduler(SchedulerError),
}

impl fmt::Display for SmsTemporalCoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTStateScale => f.write_str("master ticks per Z80 T-state must be positive"),
            Self::ZeroBusCycleDuration => f.write_str("scheduled bus cycle duration must be positive"),
            Self::DurationOverflow => f.write_str("bus cycle duration overflow"),
            Self::DeadlineOverflow => f.write_str("bus completion deadline overflow"),
            Self::InvalidState => f.write_str("SMS temporal coordinator state is inconsistent"),
            Self::Scheduler(error) => write!(f, "SMS machine scheduler failed: {error}"),
        }
    }
}

impl std::error::Error for SmsTemporalCoordinatorError {}

impl From<SchedulerError> for SmsTemporalCoordinatorError {
    fn from(error: SchedulerError) -> Self {
        Self::Scheduler(error)
    }
}

/// One SMS timeline. Device execution remains in the owning machine layer.
#[derive(Debug, Clone)]
pub struct SmsTemporalCoordinator {
    scheduler: MachineScheduler<SmsTemporalEvent>,
    master_ticks_per_tstate: u64,
}

impl SmsTemporalCoordinator {
    /// Construct a timeline with an explicit, profile-supplied T-state scale.
    ///
    /// The scale is a configuration input, not a claim about any SMS region or
    /// board revision. The coordinator registers one Z80 domain and advances
    /// it only when the machine scheduler dispatches a boundary.
    pub fn new(
        master_ticks_per_tstate: u64,
    ) -> Result<Self, SmsTemporalCoordinatorError> {
        if master_ticks_per_tstate == 0 {
            return Err(SmsTemporalCoordinatorError::ZeroTStateScale);
        }
        let mut scheduler = MachineScheduler::new();
        scheduler.register_clock_domain(
            SMS_Z80_CLOCK_DOMAIN_ID,
            ClockRatio::new(1, master_ticks_per_tstate)
                .map_err(|_| SmsTemporalCoordinatorError::InvalidState)?,
        )?;
        Ok(Self {
            scheduler,
            master_ticks_per_tstate,
        })
    }

    /// Return the current absolute machine timestamp.
    pub const fn now(&self) -> MachineTime {
        self.scheduler.now()
    }

    /// Return the explicit profile scale used by this coordinator.
    pub const fn master_ticks_per_tstate(&self) -> u64 {
        self.master_ticks_per_tstate
    }

    /// Inspect the Z80 clock-domain state without changing the timeline.
    pub fn z80_clock_snapshot(
        &self,
    ) -> Result<ClockDomainSnapshot, SmsTemporalCoordinatorError> {
        self.scheduler
            .clock_snapshot(SMS_Z80_CLOCK_DOMAIN_ID)
            .map_err(Into::into)
    }

    /// Schedule a bus completion after base T-states plus fixture-supplied WAIT.
    ///
    /// This schedules only a typed deadline. It does not read/write memory,
    /// call the CPU, or imply an electrical sampling edge.
    pub fn schedule_bus_completion(
        &mut self,
        request_id: u64,
        base_tstates: u8,
        wait_tstates: u32,
    ) -> Result<MachineTime, SmsTemporalCoordinatorError> {
        if base_tstates == 0 {
            return Err(SmsTemporalCoordinatorError::ZeroBusCycleDuration);
        }
        let total_tstates = u32::from(base_tstates)
            .checked_add(wait_tstates)
            .ok_or(SmsTemporalCoordinatorError::DurationOverflow)?;
        let master_ticks = u128::from(total_tstates)
            .checked_mul(u128::from(self.master_ticks_per_tstate))
            .ok_or(SmsTemporalCoordinatorError::DurationOverflow)?;
        let deadline = self
            .now()
            .checked_add(master_ticks)
            .ok_or(SmsTemporalCoordinatorError::DeadlineOverflow)?;
        self.scheduler.schedule_at_with_phase(
            deadline,
            HardwarePhase::new(0),
            PlatformPriority::new(0),
            SmsTemporalEvent::BusCompletion { request_id },
        )?;
        Ok(deadline)
    }

    /// Dispatch the next boundary after advancing the registered clock domain.
    pub fn dispatch_next(
        &mut self,
    ) -> Result<Option<MachineDispatch<SmsTemporalEvent>>, SmsTemporalCoordinatorError> {
        self.scheduler.dispatch_next().map_err(Into::into)
    }

    /// Advance idle machine time without skipping any pending event.
    pub fn advance_idle_to(
        &mut self,
        target: MachineTime,
    ) -> Result<(), SmsTemporalCoordinatorError> {
        self.scheduler.advance_idle_to(target)?;
        Ok(())
    }

    /// Export the complete scheduler timeline and the profile scale.
    pub fn export_state(&self) -> SmsTemporalCoordinatorState {
        SmsTemporalCoordinatorState {
            format_version: SMS_TEMPORAL_COORDINATOR_STATE_VERSION,
            master_ticks_per_tstate: self.master_ticks_per_tstate,
            scheduler: self.scheduler.export_state(),
        }
    }

    /// Restore the coordinator only if its Z80 clock domain matches the profile.
    pub fn restore_state(
        state: SmsTemporalCoordinatorState,
    ) -> Result<Self, SmsTemporalCoordinatorError> {
        if state.format_version != SMS_TEMPORAL_COORDINATOR_STATE_VERSION
            || state.master_ticks_per_tstate == 0
        {
            return Err(SmsTemporalCoordinatorError::InvalidState);
        }
        let domain = state
            .scheduler
            .clock_domains
            .iter()
            .find(|domain| domain.id == SMS_Z80_CLOCK_DOMAIN_ID)
            .ok_or(SmsTemporalCoordinatorError::InvalidState)?;
        if domain.ratio_numerator != 1
            || domain.ratio_denominator != state.master_ticks_per_tstate
        {
            return Err(SmsTemporalCoordinatorError::InvalidState);
        }
        let scheduler = MachineScheduler::restore_state(state.scheduler)?;
        Ok(Self {
            scheduler,
            master_ticks_per_tstate: state.master_ticks_per_tstate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SMS_Z80_CLOCK_DOMAIN_ID, SmsTemporalCoordinator, SmsTemporalCoordinatorError,
        SmsTemporalEvent,
    };
    use sega_temporal_core::MachineTime;

    #[test]
    fn scheduled_bus_boundary_advances_only_the_registered_cpu_domain() {
        let mut machine = SmsTemporalCoordinator::new(3).unwrap();
        let deadline = machine.schedule_bus_completion(7, 4, 1).unwrap();
        assert_eq!(deadline, MachineTime::from_ticks(15));
        assert_eq!(machine.now(), MachineTime::ZERO);

        let dispatch = machine.dispatch_next().unwrap().unwrap();
        assert_eq!(dispatch.event.timestamp, deadline);
        assert_eq!(
            dispatch.event.payload,
            SmsTemporalEvent::BusCompletion { request_id: 7 }
        );
        assert_eq!(machine.now(), deadline);
        assert_eq!(dispatch.clock_advances.len(), 1);
        assert_eq!(dispatch.clock_advances[0].domain, SMS_Z80_CLOCK_DOMAIN_ID);
        assert_eq!(dispatch.clock_advances[0].elapsed_cycles, 5);
        assert_eq!(machine.z80_clock_snapshot().unwrap().total_cycles, 5);
    }

    #[test]
    fn scheduler_snapshot_restore_preserves_pending_deadline_and_clock_phase() {
        let mut machine = SmsTemporalCoordinator::new(3).unwrap();
        let deadline = machine.schedule_bus_completion(9, 3, 2).unwrap();
        let restored =
            SmsTemporalCoordinator::restore_state(machine.export_state()).unwrap();
        assert_eq!(restored.now(), MachineTime::ZERO);
        assert_eq!(restored.export_state(), machine.export_state());

        let mut restored = restored;
        let dispatch = restored.dispatch_next().unwrap().unwrap();
        assert_eq!(dispatch.event.timestamp, deadline);
        assert_eq!(dispatch.clock_advances[0].elapsed_cycles, 5);
    }

    #[test]
    fn coordinator_rejects_invalid_scale_and_zero_duration() {
        assert_eq!(
            SmsTemporalCoordinator::new(0).err(),
            Some(SmsTemporalCoordinatorError::ZeroTStateScale)
        );
        let mut machine = SmsTemporalCoordinator::new(3).unwrap();
        assert_eq!(
            machine.schedule_bus_completion(1, 0, 0),
            Err(SmsTemporalCoordinatorError::ZeroBusCycleDuration)
        );
    }

    #[test]
    fn idle_advance_cannot_skip_a_pending_bus_deadline() {
        let mut machine = SmsTemporalCoordinator::new(3).unwrap();
        let deadline = machine.schedule_bus_completion(2, 4, 0).unwrap();
        assert!(machine.advance_idle_to(deadline).is_err());
        assert_eq!(machine.now(), MachineTime::ZERO);
    }
}
