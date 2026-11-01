//! Deterministic machine-level scheduler built on shared temporal primitives.
//!
//! This crate owns one machine timeline and advances registered device clock
//! domains to dispatched event boundaries. Hardware-specific event priorities
//! and payloads are supplied by the console machine; this crate does not assign
//! console semantics or execute device callbacks.

use std::collections::BTreeMap;
use std::fmt;

use sega_temporal_core::{
    ClockDomain, ClockDomainSnapshot, ClockError, ClockRatio, EventPriority, EventQueue,
    EventQueueError, MachineTime, ScheduledEvent,
};

/// Stable identity for one clock domain registered with a machine scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockDomainId(pub u16);

/// Newly elapsed clock edges for one registered device domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockAdvance {
    pub domain: ClockDomainId,
    pub elapsed_cycles: u128,
    pub total_cycles: u128,
    pub phase: u128,
}

/// An event together with all device-clock advances up to its timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineDispatch<T> {
    pub event: ScheduledEvent<T>,
    pub clock_advances: Vec<ClockAdvance>,
}

/// Errors raised while maintaining a machine timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    DuplicateClockDomain(ClockDomainId),
    UnknownClockDomain(ClockDomainId),
    Clock(ClockError),
    EventQueue(EventQueueError),
    TimeWentBackwards,
    EventPendingBeforeTarget,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateClockDomain(id) => {
                write!(f, "clock domain {} is already registered", id.0)
            }
            Self::UnknownClockDomain(id) => {
                write!(f, "clock domain {} is not registered", id.0)
            }
            Self::Clock(error) => write!(f, "clock-domain advancement failed: {error}"),
            Self::EventQueue(error) => write!(f, "event scheduling failed: {error}"),
            Self::TimeWentBackwards => write!(f, "machine scheduler cannot move backwards"),
            Self::EventPendingBeforeTarget => {
                write!(f, "cannot advance idle time past an undispatched event")
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

impl From<ClockError> for SchedulerError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

impl From<EventQueueError> for SchedulerError {
    fn from(error: EventQueueError) -> Self {
        Self::EventQueue(error)
    }
}

/// One deterministic timeline coordinating rational device clock domains.
///
/// Registration order does not affect clock advancement order: domains are
/// stored and reported by their stable numeric IDs. Events are dispatched in
/// timestamp, hardware-defined priority, then insertion-sequence order.
#[derive(Debug, Clone)]
pub struct MachineScheduler<T> {
    now: MachineTime,
    events: EventQueue<T>,
    clocks: BTreeMap<ClockDomainId, ClockDomain>,
}

impl<T> Default for MachineScheduler<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> MachineScheduler<T> {
    /// Construct an empty scheduler at machine time zero.
    pub fn new() -> Self {
        Self {
            now: MachineTime::ZERO,
            events: EventQueue::new(),
            clocks: BTreeMap::new(),
        }
    }

    /// Return the current machine timestamp.
    pub const fn now(&self) -> MachineTime {
        self.now
    }

    /// Register a clock domain at the scheduler's current timestamp.
    ///
    /// Each domain keeps its own rational phase, so chunked advances preserve
    /// exact edge counts. Duplicate IDs are rejected without changing state.
    pub fn register_clock_domain(
        &mut self,
        id: ClockDomainId,
        ratio: ClockRatio,
    ) -> Result<(), SchedulerError> {
        if self.clocks.contains_key(&id) {
            return Err(SchedulerError::DuplicateClockDomain(id));
        }
        self.clocks.insert(id, ClockDomain::new(ratio, self.now));
        Ok(())
    }

    /// Return a read-only snapshot of a registered clock domain.
    pub fn clock_snapshot(&self, id: ClockDomainId) -> Result<ClockDomainSnapshot, SchedulerError> {
        self.clocks
            .get(&id)
            .map(ClockDomain::snapshot)
            .ok_or(SchedulerError::UnknownClockDomain(id))
    }

    /// Schedule a typed machine event at an absolute machine timestamp.
    ///
    /// Events in the past are rejected. Priority meanings are defined by the
    /// owning console profile, not by this generic scheduler.
    pub fn schedule_at(
        &mut self,
        timestamp: MachineTime,
        priority: EventPriority,
        payload: T,
    ) -> Result<u64, SchedulerError> {
        if timestamp < self.now {
            return Err(SchedulerError::TimeWentBackwards);
        }
        self.events
            .schedule(timestamp, priority, payload)
            .map_err(SchedulerError::from)
    }

    /// Dispatch the next event after advancing every registered clock domain.
    ///
    /// No hardware callback is invoked here. The owning machine consumes the
    /// returned event and clock-edge counts in its own deterministic dispatch
    /// loop. If clock arithmetic fails, the queue is not popped and no clock
    /// domain is partially advanced.
    pub fn dispatch_next(&mut self) -> Result<Option<MachineDispatch<T>>, SchedulerError> {
        let Some(timestamp) = self.events.next_timestamp() else {
            return Ok(None);
        };

        let advances = self.calculate_advances(timestamp)?;
        self.apply_advances(timestamp, &advances)?;
        let event = self
            .events
            .pop_next()
            .expect("next_timestamp guarantees a queued event");
        Ok(Some(MachineDispatch {
            event,
            clock_advances: advances,
        }))
    }

    /// Advance all clock domains to an idle timestamp when no queued event
    /// would be skipped. This is used for bounded run-until operations.
    pub fn advance_idle_to(
        &mut self,
        target: MachineTime,
    ) -> Result<Vec<ClockAdvance>, SchedulerError> {
        if target < self.now {
            return Err(SchedulerError::TimeWentBackwards);
        }
        if self
            .events
            .next_timestamp()
            .is_some_and(|next| next <= target)
        {
            return Err(SchedulerError::EventPendingBeforeTarget);
        }
        let advances = self.calculate_advances(target)?;
        self.apply_advances(target, &advances)?;
        Ok(advances)
    }

    /// Return the timestamp of the next pending event.
    pub fn next_event_time(&self) -> Option<MachineTime> {
        self.events.next_timestamp()
    }

    /// Return the number of pending events.
    pub fn pending_events(&self) -> usize {
        self.events.len()
    }

    fn calculate_advances(&self, target: MachineTime) -> Result<Vec<ClockAdvance>, SchedulerError> {
        let mut advances = Vec::with_capacity(self.clocks.len());
        for (&id, domain) in &self.clocks {
            let elapsed = target
                .checked_duration_since(domain.current_time())
                .ok_or(SchedulerError::TimeWentBackwards)?;
            let ratio = domain.ratio();
            let scaled = elapsed
                .checked_mul(u128::from(ratio.numerator()))
                .and_then(|value| value.checked_add(domain.snapshot().phase))
                .ok_or(SchedulerError::Clock(ClockError::ArithmeticOverflow))?;
            let denominator = u128::from(ratio.denominator());
            let cycles = scaled / denominator;
            let total = domain
                .total_cycles()
                .checked_add(cycles)
                .ok_or(SchedulerError::Clock(ClockError::ArithmeticOverflow))?;
            advances.push(ClockAdvance {
                domain: id,
                elapsed_cycles: cycles,
                total_cycles: total,
                phase: scaled % denominator,
            });
        }
        Ok(advances)
    }

    fn apply_advances(
        &mut self,
        target: MachineTime,
        advances: &[ClockAdvance],
    ) -> Result<(), SchedulerError> {
        // All arithmetic was preflighted by calculate_advances. Repeating the
        // checked operation here cannot fail unless an internal invariant broke.
        for advance in advances {
            let domain = self
                .clocks
                .get_mut(&advance.domain)
                .ok_or(SchedulerError::UnknownClockDomain(advance.domain))?;
            domain.advance_to(target)?;
        }
        self.now = target;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ClockDomainId, MachineScheduler, SchedulerError};
    use sega_temporal_core::{ClockRatio, EventPriority, MachineTime};

    #[test]
    fn dispatch_advances_all_domains_and_preserves_rational_phase() {
        let mut scheduler = MachineScheduler::new();
        scheduler
            .register_clock_domain(ClockDomainId(2), ClockRatio::new(1, 3).unwrap())
            .unwrap();
        scheduler
            .register_clock_domain(ClockDomainId(1), ClockRatio::new(2, 1).unwrap())
            .unwrap();
        scheduler
            .schedule_at(
                MachineTime::from_ticks(2),
                EventPriority::new(4),
                "vdp-edge",
            )
            .unwrap();

        let dispatch = scheduler.dispatch_next().unwrap().unwrap();
        assert_eq!(dispatch.event.payload, "vdp-edge");
        assert_eq!(dispatch.event.timestamp, MachineTime::from_ticks(2));
        assert_eq!(dispatch.clock_advances[0].domain, ClockDomainId(1));
        assert_eq!(dispatch.clock_advances[0].elapsed_cycles, 4);
        assert_eq!(dispatch.clock_advances[1].domain, ClockDomainId(2));
        assert_eq!(dispatch.clock_advances[1].elapsed_cycles, 0);
        assert_eq!(dispatch.clock_advances[1].phase, 2);
        assert_eq!(scheduler.now(), MachineTime::from_ticks(2));
    }

    #[test]
    fn idle_advance_cannot_skip_pending_events() {
        let mut scheduler = MachineScheduler::<u8>::new();
        scheduler
            .schedule_at(MachineTime::from_ticks(5), EventPriority::new(0), 7)
            .unwrap();
        assert_eq!(
            scheduler.advance_idle_to(MachineTime::from_ticks(5)),
            Err(SchedulerError::EventPendingBeforeTarget)
        );
        assert_eq!(scheduler.now(), MachineTime::ZERO);
    }

    #[test]
    fn failed_clock_preflight_does_not_pop_event_or_advance_any_domain() {
        let mut scheduler = MachineScheduler::new();
        scheduler
            .register_clock_domain(ClockDomainId(0), ClockRatio::new(u64::MAX, 1).unwrap())
            .unwrap();
        scheduler
            .schedule_at(MachineTime::from_ticks(u128::MAX), EventPriority::new(0), 9)
            .unwrap();

        assert!(matches!(
            scheduler.dispatch_next(),
            Err(SchedulerError::Clock(_))
        ));
        assert_eq!(scheduler.pending_events(), 1);
        assert_eq!(scheduler.now(), MachineTime::ZERO);
        assert_eq!(
            scheduler
                .clock_snapshot(ClockDomainId(0))
                .unwrap()
                .total_cycles,
            0
        );
    }

    #[test]
    fn registration_and_past_events_are_validated() {
        let mut scheduler = MachineScheduler::<()>::new();
        let id = ClockDomainId(7);
        scheduler
            .register_clock_domain(id, ClockRatio::new(1, 1).unwrap())
            .unwrap();
        assert_eq!(
            scheduler.register_clock_domain(id, ClockRatio::new(1, 1).unwrap()),
            Err(SchedulerError::DuplicateClockDomain(id))
        );
        assert_eq!(
            scheduler.schedule_at(MachineTime::from_ticks(0), EventPriority::new(0), ()),
            Ok(0)
        );
        scheduler.dispatch_next().unwrap();
        scheduler
            .advance_idle_to(MachineTime::from_ticks(1))
            .unwrap();
        assert_eq!(
            scheduler.schedule_at(MachineTime::ZERO, EventPriority::new(0), ()),
            Err(SchedulerError::TimeWentBackwards)
        );
    }

    #[test]
    fn chunked_idle_advances_match_one_large_advance() {
        let ratio = ClockRatio::new(7, 3).unwrap();
        let mut whole = MachineScheduler::<()>::new();
        let mut chunked = MachineScheduler::<()>::new();
        whole
            .register_clock_domain(ClockDomainId(1), ratio)
            .unwrap();
        chunked
            .register_clock_domain(ClockDomainId(1), ratio)
            .unwrap();

        whole.advance_idle_to(MachineTime::from_ticks(10)).unwrap();
        chunked.advance_idle_to(MachineTime::from_ticks(3)).unwrap();
        chunked.advance_idle_to(MachineTime::from_ticks(7)).unwrap();
        chunked
            .advance_idle_to(MachineTime::from_ticks(10))
            .unwrap();

        assert_eq!(
            whole.clock_snapshot(ClockDomainId(1)).unwrap(),
            chunked.clock_snapshot(ClockDomainId(1)).unwrap()
        );
    }

    #[test]
    fn same_timestamp_events_dispatch_by_priority_then_insertion_order() {
        let mut scheduler = MachineScheduler::new();
        let timestamp = MachineTime::from_ticks(12);
        scheduler\n            .schedule_at(timestamp, EventPriority::new(3), "late")\n            .unwrap();
        scheduler\n            .schedule_at(timestamp, EventPriority::new(1), "early-a")\n            .unwrap();
        scheduler\n            .schedule_at(timestamp, EventPriority::new(1), "early-b")\n            .unwrap();

        let first = scheduler.dispatch_next().unwrap().unwrap();
        let second = scheduler.dispatch_next().unwrap().unwrap();
        let third = scheduler.dispatch_next().unwrap().unwrap();

        assert_eq!(first.event.payload, "early-a");
        assert_eq!(second.event.payload, "early-b");
        assert_eq!(third.event.payload, "late");
        assert_eq!(first.event.timestamp, timestamp);
        assert_eq!(second.event.timestamp, timestamp);
        assert_eq!(third.event.timestamp, timestamp);
        assert_eq!(scheduler.now(), timestamp);
        assert_eq!(scheduler.pending_events(), 0);
    }

    #[test]
    fn causal_same_timestamp_events_allow_same_or_later_priority_only() {
        use sega_temporal_core::EventQueueError;

        let mut scheduler = MachineScheduler::new();
        let timestamp = MachineTime::from_ticks(8);
        scheduler\n            .schedule_at(timestamp, EventPriority::new(3), "origin")\n            .unwrap();
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "origin"
        );

        assert_eq!(
            scheduler.schedule_at(timestamp, EventPriority::new(2), "retroactive"),
            Err(SchedulerError::EventQueue(
                EventQueueError::PriorityRegressionAtCurrentTime
            ))
        );
        scheduler\n            .schedule_at(timestamp, EventPriority::new(3), "same")\n            .unwrap();
        scheduler\n            .schedule_at(timestamp, EventPriority::new(4), "later")\n            .unwrap();

        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "same"
        );
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "later"
        );
    }

    #[test]
    fn failed_preflight_keeps_all_clock_domains_unchanged() {
        let mut scheduler = MachineScheduler::new();
        scheduler
            .register_clock_domain(ClockDomainId(0), ClockRatio::new(1, 1).unwrap())
            .unwrap();
        scheduler
            .register_clock_domain(
                ClockDomainId(1),
                ClockRatio::new(u64::MAX, 1).unwrap(),
            )
            .unwrap();
        scheduler
            .schedule_at(
                MachineTime::from_ticks(u128::MAX),
                EventPriority::new(0),
                "overflow",
            )
            .unwrap();

        let before_0 = scheduler.clock_snapshot(ClockDomainId(0)).unwrap();
        let before_1 = scheduler.clock_snapshot(ClockDomainId(1)).unwrap();
        assert!(matches!(
            scheduler.dispatch_next(),
            Err(SchedulerError::Clock(_))
        ));

        assert_eq!(\n            scheduler.clock_snapshot(ClockDomainId(0)).unwrap(),\n            before_0\n        );
        assert_eq!(\n            scheduler.clock_snapshot(ClockDomainId(1)).unwrap(),\n            before_1\n        );
        assert_eq!(scheduler.now(), MachineTime::ZERO);
        assert_eq!(scheduler.pending_events(), 1);
    }

}
