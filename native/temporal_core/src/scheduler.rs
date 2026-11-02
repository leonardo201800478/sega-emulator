//! One deterministic machine timeline coordinating rational clock domains
//! and timestamped device events.
//!
//! This layer is hardware-agnostic: machine profiles provide clock ratios,
//! event priorities and typed payloads. It does not decide CPU/VDP ordering,
//! invent clock rates, execute instructions or advance any device internally.

use core::fmt;
use std::collections::BTreeMap;

use crate::{
    ClockDomain, ClockDomainSnapshot, ClockError, ClockRatio, EventPriority, EventQueue,
    EventQueueError, MachineTime, ScheduledEvent,
};

/// Stable machine-local identifier for a registered clock domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockDomainId(u16);

impl ClockDomainId {
    /// Construct a machine-local clock-domain identifier.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Return the numeric identifier.
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Errors returned by the machine timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineSchedulerError {
    /// A requested machine timestamp precedes the scheduler's current time.
    TimeWentBackwards,
    /// Adding a relative delay exceeded the timestamp range.
    TimeOverflow,
    /// Advancing would cross an event that the caller has not dispatched.
    EventPendingBeforeTarget {
        /// Timestamp of the next pending event.
        event_time: MachineTime,
        /// Requested target timestamp.
        target: MachineTime,
    },
    /// A clock-domain identifier is already registered.
    DuplicateClockDomain(ClockDomainId),
    /// A requested clock-domain identifier is not registered.
    UnknownClockDomain(ClockDomainId),
    /// Rational clock conversion failed.
    Clock(ClockError),
    /// Event insertion or ordering failed.
    EventQueue(EventQueueError),
}

impl fmt::Display for MachineSchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeWentBackwards => {
                write!(formatter, "machine scheduler cannot move backwards")
            }
            Self::TimeOverflow => write!(formatter, "machine scheduler timestamp overflow"),
            Self::EventPendingBeforeTarget { event_time, target } => write!(
                formatter,
                "event at tick {} must be dispatched before advancing to tick {}",
                event_time.ticks(),
                target.ticks()
            ),
            Self::DuplicateClockDomain(id) => {
                write!(formatter, "clock domain {} is already registered", id.value())
            }
            Self::UnknownClockDomain(id) => {
                write!(formatter, "clock domain {} is not registered", id.value())
            }
            Self::Clock(error) => write!(formatter, "clock-domain error: {error}"),
            Self::EventQueue(error) => write!(formatter, "event-queue error: {error}"),
        }
    }
}

impl std::error::Error for MachineSchedulerError {}

impl From<ClockError> for MachineSchedulerError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

impl From<EventQueueError> for MachineSchedulerError {
    fn from(error: EventQueueError) -> Self {
        Self::EventQueue(error)
    }
}

/// Deterministic master timeline for one emulated machine.
///
/// Clock domains are advanced atomically to the next event timestamp before
/// that event is returned to the machine dispatcher. If any domain overflows,
/// neither the timeline, its clock domains nor its event queue are mutated.
///
/// The scheduler deliberately has no built-in hardware priorities. A machine
/// profile defines those values only after documenting the corresponding
/// hardware ordering. Device callbacks remain in the machine layer, outside
/// this generic timeline primitive.
#[derive(Debug, Clone)]
pub struct MachineScheduler<T> {
    now: MachineTime,
    events: EventQueue<T>,
    clock_domains: BTreeMap<ClockDomainId, ClockDomain>,
}

impl<T> MachineScheduler<T> {
    /// Create an empty machine timeline at the supplied absolute timestamp.
    pub fn new(start: MachineTime) -> Self {
        Self {
            now: start,
            events: EventQueue::new(),
            clock_domains: BTreeMap::new(),
        }
    }

    /// Return the scheduler's current absolute machine timestamp.
    pub const fn now(&self) -> MachineTime {
        self.now
    }

    /// Register a rational clock domain starting at the current machine time.
    pub fn register_clock_domain(
        &mut self,
        id: ClockDomainId,
        ratio: ClockRatio,
    ) -> Result<(), MachineSchedulerError> {
        self.register_domain(id, ClockDomain::new(ratio, self.now))
    }

    /// Register a clock domain with an explicit fractional phase.
    pub fn register_clock_domain_with_phase(
        &mut self,
        id: ClockDomainId,
        ratio: ClockRatio,
        phase: u128,
    ) -> Result<(), MachineSchedulerError> {
        let domain = ClockDomain::with_phase(ratio, self.now, phase)?;
        self.register_domain(id, domain)
    }

    fn register_domain(
        &mut self,
        id: ClockDomainId,
        domain: ClockDomain,
    ) -> Result<(), MachineSchedulerError> {
        if self.clock_domains.contains_key(&id) {
            return Err(MachineSchedulerError::DuplicateClockDomain(id));
        }
        self.clock_domains.insert(id, domain);
        Ok(())
    }

    /// Return a snapshot of one registered clock domain.
    pub fn clock_snapshot(
        &self,
        id: ClockDomainId,
    ) -> Result<ClockDomainSnapshot, MachineSchedulerError> {
        self.clock_domains
            .get(&id)
            .map(ClockDomain::snapshot)
            .ok_or(MachineSchedulerError::UnknownClockDomain(id))
    }

    /// Schedule a typed event at an absolute machine timestamp.
    pub fn schedule_at(
        &mut self,
        timestamp: MachineTime,
        priority: EventPriority,
        payload: T,
    ) -> Result<u64, MachineSchedulerError> {
        if timestamp < self.now {
            return Err(MachineSchedulerError::TimeWentBackwards);
        }
        Ok(self.events.schedule(timestamp, priority, payload)?)
    }

    /// Schedule a typed event after a relative number of master-time ticks.
    pub fn schedule_after(
        &mut self,
        delay_ticks: u128,
        priority: EventPriority,
        payload: T,
    ) -> Result<u64, MachineSchedulerError> {
        let timestamp = self
            .now
            .checked_add(delay_ticks)
            .ok_or(MachineSchedulerError::TimeOverflow)?;
        self.schedule_at(timestamp, priority, payload)
    }

    /// Return the timestamp of the next event, if one is queued.
    pub fn next_event_time(&self) -> Option<MachineTime> {
        self.events.next_timestamp()
    }

    /// Return the number of queued events.
    pub fn pending_event_count(&self) -> usize {
        self.events.len()
    }

    /// Dispatch the next event after advancing every clock domain to its time.
    ///
    /// The returned event is not executed here. The owning machine dispatches
    /// its payload to the relevant device(s) and may schedule causal events.
    pub fn dispatch_next(
        &mut self,
    ) -> Result<Option<ScheduledEvent<T>>, MachineSchedulerError> {
        let Some(timestamp) = self.events.next_timestamp() else {
            return Ok(None);
        };

        self.advance_domains_atomically(timestamp)?;
        let event = self.events.pop_next();
        self.now = timestamp;
        Ok(event)
    }

    /// Advance an interval containing no pending events.
    ///
    /// The call fails if an event is queued at or before target; callers must
    /// dispatch that event first so no device event is skipped.
    pub fn advance_to(&mut self, target: MachineTime) -> Result<(), MachineSchedulerError> {
        if target < self.now {
            return Err(MachineSchedulerError::TimeWentBackwards);
        }

        if let Some(event_time) = self.events.next_timestamp()
            && event_time <= target
        {
            return Err(MachineSchedulerError::EventPendingBeforeTarget {
                event_time,
                target,
            });
        }

        self.advance_domains_atomically(target)?;
        self.now = target;
        Ok(())
    }

    fn advance_domains_atomically(
        &mut self,
        target: MachineTime,
    ) -> Result<(), MachineSchedulerError> {
        let mut advanced = self.clock_domains.clone();
        for domain in advanced.values_mut() {
            domain.advance_to(target)?;
        }
        self.clock_domains = advanced;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ClockDomainId, MachineScheduler, MachineSchedulerError};
    use crate::{ClockRatio, EventPriority, MachineTime};

    const CPU: ClockDomainId = ClockDomainId::new(1);
    const VDP: ClockDomainId = ClockDomainId::new(2);

    #[test]
    fn dispatch_advances_all_clock_domains_before_returning_event() {
        let mut scheduler = MachineScheduler::new(MachineTime::ZERO);
        scheduler
            .register_clock_domain(CPU, ClockRatio::new(3, 1).unwrap())
            .unwrap();
        scheduler
            .register_clock_domain(VDP, ClockRatio::new(1, 1).unwrap())
            .unwrap();
        scheduler
            .schedule_at(
                MachineTime::from_ticks(10),
                EventPriority::new(4),
                "raster event",
            )
            .unwrap();

        let event = scheduler.dispatch_next().unwrap().unwrap();

        assert_eq!(event.payload, "raster event");
        assert_eq!(event.timestamp, MachineTime::from_ticks(10));
        assert_eq!(scheduler.now(), MachineTime::from_ticks(10));
        assert_eq!(scheduler.clock_snapshot(CPU).unwrap().total_cycles, 30);
        assert_eq!(scheduler.clock_snapshot(VDP).unwrap().total_cycles, 10);
    }

    #[test]
    fn advance_to_refuses_to_skip_a_pending_event() {
        let mut scheduler = MachineScheduler::<()>::new(MachineTime::ZERO);
        scheduler
            .schedule_at(
                MachineTime::from_ticks(5),
                EventPriority::new(0),
                (),
            )
            .unwrap();

        assert_eq!(
            scheduler.advance_to(MachineTime::from_ticks(8)),
            Err(MachineSchedulerError::EventPendingBeforeTarget {
                event_time: MachineTime::from_ticks(5),
                target: MachineTime::from_ticks(8),
            })
        );
        assert_eq!(scheduler.now(), MachineTime::ZERO);
        assert_eq!(scheduler.pending_event_count(), 1);
    }

    #[test]
    fn same_timestamp_events_keep_platform_priority_and_stable_order() {
        let mut scheduler = MachineScheduler::new(MachineTime::ZERO);
        scheduler
            .schedule_at(MachineTime::from_ticks(3), EventPriority::new(2), "second")
            .unwrap();
        scheduler
            .schedule_at(MachineTime::from_ticks(3), EventPriority::new(1), "first")
            .unwrap();

        assert_eq!(scheduler.dispatch_next().unwrap().unwrap().payload, "first");
        assert_eq!(scheduler.dispatch_next().unwrap().unwrap().payload, "second");
        assert_eq!(scheduler.now(), MachineTime::from_ticks(3));
    }

    #[test]
    fn causal_event_cannot_retroactively_change_current_timestamp_priority() {
        let mut scheduler = MachineScheduler::new(MachineTime::ZERO);
        scheduler
            .schedule_at(MachineTime::from_ticks(3), EventPriority::new(2), "dispatch")
            .unwrap();
        scheduler.dispatch_next().unwrap();

        assert_eq!(
            scheduler.schedule_at(
                MachineTime::from_ticks(3),
                EventPriority::new(1),
                "retroactive",
            ),
            Err(MachineSchedulerError::EventQueue(
                crate::EventQueueError::PriorityRegressionAtCurrentTime
            ))
        );
        assert_eq!(
            scheduler.schedule_at(
                MachineTime::from_ticks(3),
                EventPriority::new(2),
                "causal",
            ),
            Ok(1)
        );
    }

    #[test]
    fn relative_scheduling_uses_current_master_time() {
        let mut scheduler = MachineScheduler::new(MachineTime::from_ticks(7));
        scheduler.schedule_after(5, EventPriority::new(0), "future").unwrap();

        assert_eq!(
            scheduler.next_event_time(),
            Some(MachineTime::from_ticks(12))
        );
    }

    #[test]
    fn clock_domain_registration_rejects_duplicate_ids() {
        let mut scheduler = MachineScheduler::<()>::new(MachineTime::ZERO);
        let ratio = ClockRatio::new(1, 3).unwrap();
        scheduler.register_clock_domain(CPU, ratio).unwrap();

        assert_eq!(
            scheduler.register_clock_domain(CPU, ratio),
            Err(MachineSchedulerError::DuplicateClockDomain(CPU))
        );
    }

    #[test]
    fn failed_multi_domain_advance_is_atomic_and_keeps_event_queued() {
        let mut scheduler = MachineScheduler::new(MachineTime::ZERO);
        scheduler
            .register_clock_domain(CPU, ClockRatio::new(1, 1).unwrap())
            .unwrap();
        scheduler
            .register_clock_domain(VDP, ClockRatio::new(u64::MAX, 1).unwrap())
            .unwrap();
        scheduler
            .schedule_at(
                MachineTime::from_ticks(u128::MAX),
                EventPriority::new(0),
                "overflow",
            )
            .unwrap();

        let cpu_before = scheduler.clock_snapshot(CPU).unwrap();
        let vdp_before = scheduler.clock_snapshot(VDP).unwrap();

        assert_eq!(
            scheduler.dispatch_next(),
            Err(MachineSchedulerError::Clock(
                crate::ClockError::ArithmeticOverflow
            ))
        );
        assert_eq!(scheduler.now(), MachineTime::ZERO);
        assert_eq!(scheduler.pending_event_count(), 1);
        assert_eq!(scheduler.clock_snapshot(CPU).unwrap(), cpu_before);
        assert_eq!(scheduler.clock_snapshot(VDP).unwrap(), vdp_before);
    }

    #[test]
    fn idle_advance_updates_clock_domains_and_machine_time() {
        let mut scheduler = MachineScheduler::<()>::new(MachineTime::ZERO);
        scheduler
            .register_clock_domain(CPU, ClockRatio::new(3, 2).unwrap())
            .unwrap();

        scheduler.advance_to(MachineTime::from_ticks(10)).unwrap();

        let snapshot = scheduler.clock_snapshot(CPU).unwrap();
        assert_eq!(scheduler.now(), MachineTime::from_ticks(10));
        assert_eq!(snapshot.total_cycles, 15);
        assert_eq!(snapshot.phase, 0);
    }

    #[test]
    fn unknown_clock_domain_is_reported() {
        let scheduler = MachineScheduler::<()>::new(MachineTime::ZERO);
        assert_eq!(
            scheduler.clock_snapshot(CPU),
            Err(MachineSchedulerError::UnknownClockDomain(CPU))
        );
    }
}
