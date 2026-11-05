//! Deterministic machine-level scheduler built on shared temporal primitives.
//!
//! This crate owns one machine timeline and advances registered device clock
//! domains to dispatched event boundaries. Hardware-specific event priorities
//! and payloads are supplied by the console machine; this crate does not assign
//! console semantics or execute device callbacks.

use std::collections::BTreeMap;
use std::fmt;

pub mod codec;
pub mod state;
pub use state::{ClockDomainState, SCHEDULER_STATE_FORMAT_VERSION, SchedulerState};

pub use sega_temporal_core::{ClockRatio, EventPriority, MachineInstant, MachineTime};

use sega_temporal_core::{
    ClockDomain, ClockDomainSnapshot, ClockError, EventQueue, EventQueueError, HardwarePhase,
    PlatformPriority, ScheduledEvent,
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

/// Deterministic upper bound for event dispatches at one machine timestamp.
///
/// This is a scheduler safety policy, not a claim about hardware event
/// precedence or a physical limit. A machine profile may choose a different
/// non-zero budget based on its event model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeltaEventPolicy {
    max_events_per_timestamp: u64,
}

impl DeltaEventPolicy {
    /// Construct a policy with a non-zero per-timestamp dispatch budget.
    pub const fn new(max_events_per_timestamp: u64) -> Result<Self, SchedulerError> {
        if max_events_per_timestamp == 0 {
            return Err(SchedulerError::ZeroDeltaEventLimit);
        }
        Ok(Self {
            max_events_per_timestamp,
        })
    }

    /// Return the maximum number of events dispatched at one timestamp.
    pub const fn max_events_per_timestamp(self) -> u64 {
        self.max_events_per_timestamp
    }
}

impl Default for DeltaEventPolicy {
    fn default() -> Self {
        Self {
            max_events_per_timestamp: 1024,
        }
    }
}

/// Errors raised while maintaining a machine timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    DuplicateClockDomain(ClockDomainId),
    UnknownClockDomain(ClockDomainId),
    ClockDomainTimeMismatch(ClockDomainId),
    Clock(ClockError),
    EventQueue(EventQueueError),
    TimeWentBackwards,
    SubTickEventUnsupported,
    EventPendingBeforeTarget,
    ZeroDeltaEventLimit,
    DeltaEventLimitExceeded { timestamp: MachineTime, limit: u64 },
    StateFormatVersionMismatch { expected: u16, actual: u16 },
    NonCanonicalClockDomainOrder,
    PendingEventBeforeNow,
    WatermarkAfterNow,
    DispatchHistoryMismatch,
    DispatchWatermarkMismatch,
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
            Self::ClockDomainTimeMismatch(id) => write!(
                f,
                "clock domain {} timestamp does not match scheduler time",
                id.0
            ),
            Self::Clock(error) => write!(f, "clock-domain advancement failed: {error}"),
            Self::EventQueue(error) => write!(f, "event scheduling failed: {error}"),
            Self::TimeWentBackwards => write!(f, "machine scheduler cannot move backwards"),
            Self::SubTickEventUnsupported => write!(f, "scheduler clock domains do not yet advance to rational sub-tick instants"),
            Self::EventPendingBeforeTarget => {
                write!(f, "cannot advance idle time past an undispatched event")
            }
            Self::ZeroDeltaEventLimit => write!(f, "delta-event limit must be positive"),
            Self::DeltaEventLimitExceeded { timestamp, limit } => write!(
                f,
                "delta-event dispatch limit {limit} exceeded at machine tick {}",
                timestamp.ticks()
            ),
            Self::StateFormatVersionMismatch { expected, actual } => write!(
                f,
                "scheduler state format version {actual} is unsupported (expected {expected})"
            ),
            Self::NonCanonicalClockDomainOrder => {
                write!(
                    f,
                    "clock-domain states are not in strict ascending ID order"
                )
            }
            Self::PendingEventBeforeNow => {
                write!(
                    f,
                    "restored event queue contains an event before scheduler time"
                )
            }
            Self::WatermarkAfterNow => {
                write!(f, "event queue watermark is later than scheduler time")
            }
            Self::DispatchHistoryMismatch => {
                write!(f, "scheduler dispatch timestamp and count are inconsistent")
            }
            Self::DispatchWatermarkMismatch => {
                write!(
                    f,
                    "scheduler dispatch timestamp does not match queue watermark"
                )
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
    delta_event_policy: DeltaEventPolicy,
    last_dispatch_timestamp: Option<MachineTime>,
    dispatched_at_timestamp: u64,
}

impl<T> Default for MachineScheduler<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> MachineScheduler<T> {
    /// Construct an empty scheduler at machine time zero.
    pub fn new() -> Self {
        Self::with_delta_event_policy(DeltaEventPolicy::default())
    }

    /// Construct an empty scheduler with an explicit same-timestamp budget.
    pub fn with_delta_event_policy(delta_event_policy: DeltaEventPolicy) -> Self {
        Self {
            now: MachineTime::ZERO,
            events: EventQueue::new(),
            clocks: BTreeMap::new(),
            delta_event_policy,
            last_dispatch_timestamp: None,
            dispatched_at_timestamp: 0,
        }
    }

    /// Return the active same-timestamp dispatch policy.
    pub const fn delta_event_policy(&self) -> DeltaEventPolicy {
        self.delta_event_policy
    }

    /// Replace the same-timestamp dispatch policy.
    ///
    /// This does not discard pending events or reset dispatch history. Raising
    /// the limit can therefore resume a queue that was previously blocked.
    pub fn set_delta_event_policy(&mut self, policy: DeltaEventPolicy) {
        self.delta_event_policy = policy;
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
        self.register_clock_domain_with_phase(id, ratio, 0)
    }

    /// Register a clock domain with an explicit fractional phase.
    ///
    /// The phase uses the denominator of `ratio` as its unit and must be
    /// strictly smaller than that denominator. Registration is atomic: an
    /// invalid phase or duplicate ID leaves the scheduler unchanged.
    pub fn register_clock_domain_with_phase(
        &mut self,
        id: ClockDomainId,
        ratio: ClockRatio,
        phase: u128,
    ) -> Result<(), SchedulerError> {
        if self.clocks.contains_key(&id) {
            return Err(SchedulerError::DuplicateClockDomain(id));
        }
        let domain = ClockDomain::with_phase(ratio, self.now, phase)?;
        self.clocks.insert(id, domain);
        Ok(())
    }

    /// Return a read-only snapshot of a registered clock domain.
    pub fn clock_snapshot(&self, id: ClockDomainId) -> Result<ClockDomainSnapshot, SchedulerError> {
        self.clocks
            .get(&id)
            .map(ClockDomain::snapshot)
            .ok_or(SchedulerError::UnknownClockDomain(id))
    }

    /// Export one clock domain with its complete registration and phase history.
    pub fn export_clock_domain_state(
        &self,
        id: ClockDomainId,
    ) -> Result<ClockDomainState, SchedulerError> {
        self.clocks
            .get(&id)
            .map(|domain| ClockDomainState::from_domain(id, domain))
            .ok_or(SchedulerError::UnknownClockDomain(id))
    }

    /// Restore one clock domain into this scheduler without partial mutation.
    ///
    /// The restored domain must be at the scheduler's current timestamp and
    /// its ID must not already be registered.
    pub fn restore_clock_domain_state(
        &mut self,
        state: ClockDomainState,
    ) -> Result<(), SchedulerError> {
        if self.clocks.contains_key(&state.id) {
            return Err(SchedulerError::DuplicateClockDomain(state.id));
        }
        if state.current_time != self.now {
            return Err(SchedulerError::ClockDomainTimeMismatch(state.id));
        }
        let (id, domain) = state.into_domain()?;
        self.clocks.insert(id, domain);
        Ok(())
    }

    /// Export the complete scheduler state in canonical clock-domain order.
    ///
    /// Event payloads are cloned by the underlying queue export; callers must
    /// provide payloads whose clone operation preserves their logical value.
    pub fn export_state(&self) -> SchedulerState<T>
    where
        T: Clone,
    {
        SchedulerState {
            format_version: SCHEDULER_STATE_FORMAT_VERSION,
            now: self.now,
            clock_domains: self
                .clocks
                .iter()
                .map(|(&id, domain)| ClockDomainState::from_domain(id, domain))
                .collect(),
            event_queue: self.events.export_state(),
            delta_event_limit: self.delta_event_policy.max_events_per_timestamp,
            last_dispatch_timestamp: self.last_dispatch_timestamp,
            dispatched_at_timestamp: self.dispatched_at_timestamp,
        }
    }

    /// Construct a scheduler from a complete DTO after validating every
    /// component and cross-component invariant before exposing the result.
    ///
    /// All reconstructed domains and the event queue are held in temporary
    /// values. Any error returns without producing a partially restored
    /// scheduler.
    pub fn restore_state(state: SchedulerState<T>) -> Result<Self, SchedulerError> {
        if state.format_version != SCHEDULER_STATE_FORMAT_VERSION {
            return Err(SchedulerError::StateFormatVersionMismatch {
                expected: SCHEDULER_STATE_FORMAT_VERSION,
                actual: state.format_version,
            });
        }
        if state.delta_event_limit == 0 {
            return Err(SchedulerError::ZeroDeltaEventLimit);
        }

        match (state.last_dispatch_timestamp, state.dispatched_at_timestamp) {
            (None, 0) => {}
            (Some(timestamp), count) if count > 0 => {
                if timestamp > state.now {
                    return Err(SchedulerError::DispatchHistoryMismatch);
                }
            }
            _ => return Err(SchedulerError::DispatchHistoryMismatch),
        }

        if let Some(watermark) = state.event_queue.last_dispatched {
            let watermark_time = watermark
                .timestamp
                .as_machine_time()
                .ok_or(SchedulerError::SubTickEventUnsupported)?;
            if watermark_time > state.now {
                return Err(SchedulerError::WatermarkAfterNow);
            }
            if Some(watermark_time) != state.last_dispatch_timestamp {
                return Err(SchedulerError::DispatchWatermarkMismatch);
            }
        } else if state.last_dispatch_timestamp.is_some() {
            return Err(SchedulerError::DispatchWatermarkMismatch);
        }

        // The number of dispatches at one timestamp cannot exceed the total
        // number of events ever allocated by the queue.
        if state.dispatched_at_timestamp > state.event_queue.next_sequence {
            return Err(SchedulerError::DispatchHistoryMismatch);
        }

        for event in &state.event_queue.pending {
            let event_time = event
                .timestamp
                .as_machine_time()
                .ok_or(SchedulerError::SubTickEventUnsupported)?;
            if event_time < state.now {
                return Err(SchedulerError::PendingEventBeforeNow);
            }
        }

        let mut clocks = BTreeMap::new();
        let mut previous_id = None;
        for domain_state in state.clock_domains {
            if previous_id.is_some_and(|previous| previous >= domain_state.id) {
                return Err(SchedulerError::NonCanonicalClockDomainOrder);
            }
            previous_id = Some(domain_state.id);
            if domain_state.current_time != state.now {
                return Err(SchedulerError::ClockDomainTimeMismatch(domain_state.id));
            }
            let (id, domain) = domain_state.into_domain()?;
            if clocks.insert(id, domain).is_some() {
                return Err(SchedulerError::DuplicateClockDomain(id));
            }
        }

        let events = EventQueue::restore_state(state.event_queue)?;
        Ok(Self {
            now: state.now,
            events,
            clocks,
            delta_event_policy: DeltaEventPolicy {
                max_events_per_timestamp: state.delta_event_limit,
            },
            last_dispatch_timestamp: state.last_dispatch_timestamp,
            dispatched_at_timestamp: state.dispatched_at_timestamp,
        })
    }

    /// Schedule at an absolute timestamp using the compatibility zero phase.
    pub fn schedule_at(
        &mut self,
        timestamp: MachineTime,
        priority: EventPriority,
        payload: T,
    ) -> Result<u64, SchedulerError> {
        self.schedule_at_with_phase(timestamp, HardwarePhase::default(), priority, payload)
    }

    /// Schedule with independent hardware-phase and platform-priority keys.
    ///
    /// HardwarePhase is an ordering stratum, not a fractional timestamp.
    pub fn schedule_at_with_phase(
        &mut self,
        timestamp: MachineTime,
        hardware_phase: HardwarePhase,
        priority: PlatformPriority,
        payload: T,
    ) -> Result<u64, SchedulerError> {
        if timestamp < self.now {
            return Err(SchedulerError::TimeWentBackwards);
        }
        self.events
            .schedule_with_phase(timestamp, hardware_phase, priority, payload)
            .map_err(SchedulerError::from)
    }

    /// Dispatch the next event after advancing every registered clock domain.
    ///
    /// No hardware callback is invoked here. The owning machine consumes the
    /// returned event and clock-edge counts in its own deterministic dispatch
    /// loop. If clock arithmetic fails, the queue is not popped and no clock
    /// domain is partially advanced.
    pub fn dispatch_next(&mut self) -> Result<Option<MachineDispatch<T>>, SchedulerError> {
        let Some(instant) = self.events.next_timestamp() else {
            return Ok(None);
        };
        let timestamp = instant
            .as_machine_time()
            .ok_or(SchedulerError::SubTickEventUnsupported)?;

        let next_dispatch_count = if self.last_dispatch_timestamp == Some(timestamp) {
            if self.dispatched_at_timestamp >= self.delta_event_policy.max_events_per_timestamp {
                return Err(SchedulerError::DeltaEventLimitExceeded {
                    timestamp,
                    limit: self.delta_event_policy.max_events_per_timestamp,
                });
            }
            self.dispatched_at_timestamp.checked_add(1).ok_or(
                SchedulerError::DeltaEventLimitExceeded {
                    timestamp,
                    limit: self.delta_event_policy.max_events_per_timestamp,
                },
            )?
        } else {
            1
        };

        let advances = self.calculate_advances(timestamp)?;
        self.apply_advances(timestamp, &advances)?;
        let event = self
            .events
            .pop_next()
            .expect("next_timestamp guarantees a queued event");
        self.last_dispatch_timestamp = Some(timestamp);
        self.dispatched_at_timestamp = next_dispatch_count;
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
            .is_some_and(|next| next <= MachineInstant::from(target))
        {
            return Err(SchedulerError::EventPendingBeforeTarget);
        }
        let advances = self.calculate_advances(target)?;
        self.apply_advances(target, &advances)?;
        Ok(advances)
    }

    /// Return the timestamp of the next pending event.
    pub fn next_event_time(&self) -> Option<MachineTime> {
        self.events.next_timestamp().and_then(MachineInstant::as_machine_time)
    }

    /// Return the number of pending events.
    pub fn pending_events(&self) -> usize {
        self.events.len()
    }

    fn calculate_advances(&self, target: MachineTime) -> Result<Vec<ClockAdvance>, SchedulerError> {
        let mut advances = Vec::with_capacity(self.clocks.len());
        for (&id, domain) in &self.clocks {
            let (elapsed_cycles, snapshot) = domain.preview_advance_to(target)?;
            advances.push(ClockAdvance {
                domain: id,
                elapsed_cycles,
                total_cycles: snapshot.total_cycles,
                phase: snapshot.phase,
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
    use super::{
        ClockDomainId, ClockDomainState, ClockError, DeltaEventPolicy, MachineScheduler,
        SchedulerError, SchedulerState,
    };
    use sega_temporal_core::{
        ClockRatio, EventPriority, HardwarePhase, MachineInstant, MachineTime, PlatformPriority,
        ScheduledEvent,
    };

    #[test]
    fn explicit_initial_phase_is_preserved_and_advances_rationally() {
        let mut scheduler = MachineScheduler::<()>::new();
        let id = ClockDomainId(4);
        scheduler
            .register_clock_domain_with_phase(id, ClockRatio::new(1, 3).unwrap(), 2)
            .unwrap();

        let initial = scheduler.clock_snapshot(id).unwrap();
        assert_eq!(initial.time, MachineTime::ZERO);
        assert_eq!(initial.total_cycles, 0);
        assert_eq!(initial.phase, 2);

        scheduler
            .advance_idle_to(MachineTime::from_ticks(1))
            .unwrap();
        let advanced = scheduler.clock_snapshot(id).unwrap();
        assert_eq!(advanced.total_cycles, 1);
        assert_eq!(advanced.phase, 0);
    }

    #[test]
    fn explicit_initial_phase_is_chunk_invariant() {
        let ratio = ClockRatio::new(5, 3).unwrap();
        let mut whole = MachineScheduler::<()>::new();
        let mut chunked = MachineScheduler::<()>::new();
        whole
            .register_clock_domain_with_phase(ClockDomainId(1), ratio, 2)
            .unwrap();
        chunked
            .register_clock_domain_with_phase(ClockDomainId(1), ratio, 2)
            .unwrap();

        whole.advance_idle_to(MachineTime::from_ticks(11)).unwrap();
        for target in [2, 5, 7, 11] {
            chunked
                .advance_idle_to(MachineTime::from_ticks(target))
                .unwrap();
        }

        assert_eq!(
            whole.clock_snapshot(ClockDomainId(1)).unwrap(),
            chunked.clock_snapshot(ClockDomainId(1)).unwrap()
        );
    }

    #[test]
    fn invalid_initial_phase_does_not_register_a_domain() {
        let mut scheduler = MachineScheduler::<()>::new();
        let id = ClockDomainId(9);
        assert_eq!(
            scheduler.register_clock_domain_with_phase(id, ClockRatio::new(2, 3).unwrap(), 3),
            Err(SchedulerError::Clock(ClockError::PhaseOutOfRange))
        );
        assert_eq!(
            scheduler.clock_snapshot(id),
            Err(SchedulerError::UnknownClockDomain(id))
        );
    }

    #[test]
    fn zero_phase_registration_remains_backward_compatible() {
        let mut conventional = MachineScheduler::<()>::new();
        let mut explicit_zero = MachineScheduler::<()>::new();
        let ratio = ClockRatio::new(7, 3).unwrap();

        conventional
            .register_clock_domain(ClockDomainId(2), ratio)
            .unwrap();
        explicit_zero
            .register_clock_domain_with_phase(ClockDomainId(2), ratio, 0)
            .unwrap();

        assert_eq!(
            conventional.clock_snapshot(ClockDomainId(2)).unwrap(),
            explicit_zero.clock_snapshot(ClockDomainId(2)).unwrap()
        );
    }

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
        assert_eq!(dispatch.event.timestamp, MachineInstant::from_ticks(2));
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
    fn restore_rejects_subtick_events_until_clock_domains_are_rational() {
        let scheduler = MachineScheduler::<()>::new();
        let mut state = scheduler.export_state();
        state.event_queue.pending.push(ScheduledEvent {
            timestamp: MachineInstant::from_parts(MachineTime::ZERO, 1, 2).unwrap(),
            hardware_phase: HardwarePhase::new(0),
            priority: PlatformPriority::new(0),
            sequence: 0,
            payload: (),
        });
        state.event_queue.next_sequence = 1;

        assert!(matches!(
            MachineScheduler::restore_state(state),
            Err(SchedulerError::SubTickEventUnsupported)
        ));
        // The source scheduler remains untouched by the rejected DTO.
        assert_eq!(scheduler.now(), MachineTime::ZERO);
        assert_eq!(scheduler.pending_events(), 0);
    }

    #[test]
    fn scheduler_preflight_handles_large_exact_rational_advances() {
        let ratio = ClockRatio::new(u64::MAX, u64::MAX).unwrap();
        let target = MachineTime::from_ticks(1_u128 << 65);

        let mut idle = MachineScheduler::<()>::new();
        idle.register_clock_domain(ClockDomainId(0), ratio).unwrap();
        let idle_advances = idle.advance_idle_to(target).unwrap();
        assert_eq!(idle_advances[0].elapsed_cycles, 1_u128 << 65);
        assert_eq!(idle_advances[0].total_cycles, 1_u128 << 65);
        assert_eq!(idle_advances[0].phase, 0);

        let mut dispatched = MachineScheduler::<()>::new();
        dispatched
            .register_clock_domain(ClockDomainId(0), ratio)
            .unwrap();
        dispatched
            .schedule_at(target, EventPriority::new(0), ())
            .unwrap();
        let event = dispatched.dispatch_next().unwrap().unwrap();
        assert_eq!(event.clock_advances[0].elapsed_cycles, 1_u128 << 65);
        assert_eq!(event.clock_advances[0].total_cycles, 1_u128 << 65);
        assert_eq!(event.clock_advances[0].phase, 0);
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
        scheduler
            .schedule_at(timestamp, EventPriority::new(3), "late")
            .unwrap();
        scheduler
            .schedule_at(timestamp, EventPriority::new(1), "early-a")
            .unwrap();
        scheduler
            .schedule_at(timestamp, EventPriority::new(1), "early-b")
            .unwrap();

        let first = scheduler.dispatch_next().unwrap().unwrap();
        let second = scheduler.dispatch_next().unwrap().unwrap();
        let third = scheduler.dispatch_next().unwrap().unwrap();

        assert_eq!(first.event.payload, "early-a");
        assert_eq!(second.event.payload, "early-b");
        assert_eq!(third.event.payload, "late");
        assert_eq!(first.event.timestamp, MachineInstant::from(timestamp));
        assert_eq!(second.event.timestamp, MachineInstant::from(timestamp));
        assert_eq!(third.event.timestamp, MachineInstant::from(timestamp));
        assert_eq!(scheduler.now(), timestamp);
        assert_eq!(scheduler.pending_events(), 0);
    }

    #[test]
    fn scheduler_orders_hardware_phase_before_platform_priority() {
        let mut scheduler = MachineScheduler::new();
        let timestamp = MachineTime::from_ticks(15);
        scheduler
            .schedule_at_with_phase(
                timestamp,
                HardwarePhase::new(2),
                PlatformPriority::new(0),
                "phase-two",
            )
            .unwrap();
        scheduler
            .schedule_at_with_phase(
                timestamp,
                HardwarePhase::new(1),
                PlatformPriority::new(3),
                "phase-one-priority-three",
            )
            .unwrap();
        scheduler
            .schedule_at_with_phase(
                timestamp,
                HardwarePhase::new(1),
                PlatformPriority::new(1),
                "phase-one-priority-one",
            )
            .unwrap();

        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "phase-one-priority-one"
        );
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "phase-one-priority-three"
        );
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "phase-two"
        );
    }

    #[test]
    fn causal_same_timestamp_events_allow_same_or_later_priority_only() {
        use sega_temporal_core::EventQueueError;

        let mut scheduler = MachineScheduler::new();
        let timestamp = MachineTime::from_ticks(8);
        scheduler
            .schedule_at(timestamp, EventPriority::new(3), "origin")
            .unwrap();
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "origin"
        );

        assert_eq!(
            scheduler.schedule_at(timestamp, EventPriority::new(2), "retroactive"),
            Err(SchedulerError::EventQueue(
                EventQueueError::OrderingRegressionAtCurrentTime
            ))
        );
        scheduler
            .schedule_at(timestamp, EventPriority::new(3), "same")
            .unwrap();
        scheduler
            .schedule_at(timestamp, EventPriority::new(4), "later")
            .unwrap();

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
    fn delta_event_policy_rejects_zero_budget() {
        assert_eq!(
            DeltaEventPolicy::new(0),
            Err(SchedulerError::ZeroDeltaEventLimit)
        );
        assert_eq!(
            DeltaEventPolicy::new(3).unwrap().max_events_per_timestamp(),
            3
        );
    }

    #[test]
    fn same_timestamp_budget_blocks_without_dropping_pending_events() {
        let policy = DeltaEventPolicy::new(2).unwrap();
        let mut scheduler = MachineScheduler::with_delta_event_policy(policy);
        let timestamp = MachineTime::from_ticks(5);
        for payload in ["first", "second", "third"] {
            scheduler
                .schedule_at(timestamp, EventPriority::new(0), payload)
                .unwrap();
        }

        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "first"
        );
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "second"
        );
        assert_eq!(
            scheduler.dispatch_next(),
            Err(SchedulerError::DeltaEventLimitExceeded {
                timestamp,
                limit: 2
            })
        );
        assert_eq!(scheduler.pending_events(), 1);
        assert_eq!(scheduler.next_event_time(), Some(timestamp));
        assert_eq!(scheduler.now(), timestamp);
    }

    #[test]
    fn delta_event_budget_bounds_causal_same_timestamp_chains() {
        let timestamp = MachineTime::from_ticks(9);
        let policy = DeltaEventPolicy::new(3).unwrap();
        let mut scheduler = MachineScheduler::with_delta_event_policy(policy);
        scheduler
            .schedule_at(timestamp, EventPriority::new(0), 0_u8)
            .unwrap();

        for expected in 0..3 {
            let dispatch = scheduler.dispatch_next().unwrap().unwrap();
            assert_eq!(dispatch.event.payload, expected);
            scheduler
                .schedule_at(timestamp, EventPriority::new(0), expected + 1)
                .unwrap();
        }

        assert_eq!(
            scheduler.dispatch_next(),
            Err(SchedulerError::DeltaEventLimitExceeded {
                timestamp,
                limit: 3
            })
        );
        assert_eq!(scheduler.pending_events(), 1);
    }

    #[test]
    fn delta_event_budget_resets_at_a_later_timestamp() {
        let policy = DeltaEventPolicy::new(2).unwrap();
        let mut scheduler = MachineScheduler::with_delta_event_policy(policy);
        let first_time = MachineTime::from_ticks(4);
        let second_time = MachineTime::from_ticks(5);
        scheduler
            .schedule_at(first_time, EventPriority::new(0), 1)
            .unwrap();
        scheduler
            .schedule_at(first_time, EventPriority::new(0), 2)
            .unwrap();
        scheduler
            .schedule_at(second_time, EventPriority::new(0), 3)
            .unwrap();

        assert_eq!(scheduler.dispatch_next().unwrap().unwrap().event.payload, 1);
        assert_eq!(scheduler.dispatch_next().unwrap().unwrap().event.payload, 2);
        assert_eq!(scheduler.dispatch_next().unwrap().unwrap().event.payload, 3);
        assert_eq!(scheduler.pending_events(), 0);
    }

    #[test]
    fn increasing_delta_budget_resumes_blocked_queue_without_resetting_it() {
        let mut scheduler =
            MachineScheduler::with_delta_event_policy(DeltaEventPolicy::new(1).unwrap());
        let timestamp = MachineTime::from_ticks(6);
        scheduler
            .schedule_at(timestamp, EventPriority::new(0), "one")
            .unwrap();
        scheduler
            .schedule_at(timestamp, EventPriority::new(0), "two")
            .unwrap();

        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "one"
        );
        assert_eq!(
            scheduler.dispatch_next(),
            Err(SchedulerError::DeltaEventLimitExceeded {
                timestamp,
                limit: 1
            })
        );
        scheduler.set_delta_event_policy(DeltaEventPolicy::new(2).unwrap());
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "two"
        );
    }

    #[test]
    fn clock_domain_state_export_and_restore_preserve_history() {
        let mut source = MachineScheduler::<()>::new();
        let id = ClockDomainId(12);
        source
            .register_clock_domain_with_phase(id, ClockRatio::new(5, 3).unwrap(), 2)
            .unwrap();
        source.advance_idle_to(MachineTime::from_ticks(7)).unwrap();

        let state = source.export_clock_domain_state(id).unwrap();
        assert_eq!(state.current_time, MachineTime::from_ticks(7));
        assert_eq!(state.registration_start, MachineTime::ZERO);
        assert_eq!(state.initial_phase, 2);

        let mut target = MachineScheduler::<()>::new();
        target.advance_idle_to(MachineTime::from_ticks(7)).unwrap();
        target.restore_clock_domain_state(state).unwrap();
        assert_eq!(
            target.clock_snapshot(id).unwrap(),
            source.clock_snapshot(id).unwrap()
        );
        source.advance_idle_to(MachineTime::from_ticks(13)).unwrap();
        target.advance_idle_to(MachineTime::from_ticks(13)).unwrap();
        assert_eq!(
            target.clock_snapshot(id).unwrap(),
            source.clock_snapshot(id).unwrap()
        );
    }

    #[test]
    fn invalid_clock_domain_state_does_not_mutate_scheduler() {
        let mut scheduler = MachineScheduler::<()>::new();
        let before_now = scheduler.now();
        let invalid = ClockDomainState {
            id: ClockDomainId(4),
            ratio_numerator: 5,
            ratio_denominator: 3,
            current_time: MachineTime::from_ticks(4),
            total_cycles: 1,
            phase: 0,
            registration_start: MachineTime::ZERO,
            initial_phase: 0,
        };
        assert_eq!(
            scheduler.restore_clock_domain_state(invalid),
            Err(SchedulerError::ClockDomainTimeMismatch(ClockDomainId(4)))
        );
        assert_eq!(scheduler.now(), before_now);
        assert_eq!(
            scheduler.clock_snapshot(ClockDomainId(4)),
            Err(SchedulerError::UnknownClockDomain(ClockDomainId(4)))
        );
    }

    #[test]
    fn invalid_arithmetic_clock_state_is_rejected_without_registration() {
        let mut scheduler = MachineScheduler::<()>::new();
        let invalid = ClockDomainState {
            id: ClockDomainId(5),
            ratio_numerator: 5,
            ratio_denominator: 3,
            current_time: MachineTime::ZERO,
            total_cycles: 1,
            phase: 0,
            registration_start: MachineTime::ZERO,
            initial_phase: 0,
        };
        assert!(matches!(
            scheduler.restore_clock_domain_state(invalid),
            Err(SchedulerError::Clock(ClockError::InvalidClockHistory))
        ));
        assert_eq!(
            scheduler.clock_snapshot(ClockDomainId(5)),
            Err(SchedulerError::UnknownClockDomain(ClockDomainId(5)))
        );
    }

    #[test]
    fn failed_preflight_keeps_all_clock_domains_unchanged() {
        let mut scheduler = MachineScheduler::new();
        scheduler
            .register_clock_domain(ClockDomainId(0), ClockRatio::new(1, 1).unwrap())
            .unwrap();
        scheduler
            .register_clock_domain(ClockDomainId(1), ClockRatio::new(u64::MAX, 1).unwrap())
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

        assert_eq!(
            scheduler.clock_snapshot(ClockDomainId(0)).unwrap(),
            before_0
        );
        assert_eq!(
            scheduler.clock_snapshot(ClockDomainId(1)).unwrap(),
            before_1
        );
        assert_eq!(scheduler.now(), MachineTime::ZERO);
        assert_eq!(scheduler.pending_events(), 1);
    }

    fn scheduler_snapshot_fixture() -> SchedulerState<&'static str> {
        let mut scheduler =
            MachineScheduler::with_delta_event_policy(DeltaEventPolicy::new(4).unwrap());
        scheduler
            .register_clock_domain_with_phase(ClockDomainId(3), ClockRatio::new(5, 3).unwrap(), 2)
            .unwrap();
        scheduler
            .register_clock_domain(ClockDomainId(1), ClockRatio::new(1, 3).unwrap())
            .unwrap();
        scheduler
            .schedule_at_with_phase(
                MachineTime::from_ticks(5),
                HardwarePhase::new(1),
                PlatformPriority::new(1),
                "dispatched",
            )
            .unwrap();
        scheduler
            .schedule_at_with_phase(
                MachineTime::from_ticks(5),
                HardwarePhase::new(1),
                PlatformPriority::new(2),
                "same-time-pending",
            )
            .unwrap();
        scheduler
            .schedule_at(MachineTime::from_ticks(9), EventPriority::new(0), "future")
            .unwrap();
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.payload,
            "dispatched"
        );
        scheduler.export_state()
    }

    #[test]
    fn full_scheduler_snapshot_restore_preserves_state_and_continuation() {
        let state = scheduler_snapshot_fixture();
        let mut restored = MachineScheduler::restore_state(state.clone()).unwrap();
        assert_eq!(restored.export_state(), state);

        let mut uninterrupted = MachineScheduler::restore_state(state).unwrap();
        loop {
            let expected = uninterrupted.dispatch_next().unwrap();
            let actual = restored.dispatch_next().unwrap();
            assert_eq!(actual, expected);
            if expected.is_none() {
                break;
            }
        }
        assert_eq!(restored.export_state(), uninterrupted.export_state());
    }

    #[test]
    fn restore_preserves_dispatch_count_above_reduced_delta_limit() {
        let mut scheduler =
            MachineScheduler::with_delta_event_policy(DeltaEventPolicy::new(3).unwrap());
        let timestamp = MachineTime::from_ticks(4);
        scheduler
            .schedule_at(timestamp, EventPriority::new(0), "first")
            .unwrap();
        scheduler
            .schedule_at(timestamp, EventPriority::new(0), "second")
            .unwrap();
        scheduler.dispatch_next().unwrap();
        scheduler.set_delta_event_policy(DeltaEventPolicy::new(1).unwrap());

        let state = scheduler.export_state();
        assert_eq!(state.dispatched_at_timestamp, 1);
        assert_eq!(state.delta_event_limit, 1);
        let mut restored = MachineScheduler::restore_state(state.clone()).unwrap();
        assert_eq!(restored.export_state(), state);
        assert_eq!(
            restored.dispatch_next(),
            Err(SchedulerError::DeltaEventLimitExceeded {
                timestamp,
                limit: 1
            })
        );
    }

    #[test]
    fn restore_rejects_invalid_format_and_zero_delta_limit() {
        let mut unsupported = scheduler_snapshot_fixture();
        unsupported.format_version += 1;
        assert!(matches!(
            MachineScheduler::restore_state(unsupported),
            Err(SchedulerError::StateFormatVersionMismatch {
                expected,
                actual,
            }) if expected == super::SCHEDULER_STATE_FORMAT_VERSION
                && actual == super::SCHEDULER_STATE_FORMAT_VERSION + 1
        ));

        let mut zero_limit = scheduler_snapshot_fixture();
        zero_limit.delta_event_limit = 0;
        assert!(matches!(
            MachineScheduler::restore_state(zero_limit),
            Err(SchedulerError::ZeroDeltaEventLimit)
        ));
    }

    #[test]
    fn restore_rejects_dispatch_count_above_total_allocated_sequences() {
        let mut state = scheduler_snapshot_fixture();
        state.dispatched_at_timestamp = state.event_queue.next_sequence + 1;
        assert!(matches!(
            MachineScheduler::restore_state(state),
            Err(SchedulerError::DispatchHistoryMismatch)
        ));
    }

    #[test]
    fn restore_accepts_dispatch_order_not_monotonic_in_insertion_sequence() {
        let mut scheduler =
            MachineScheduler::with_delta_event_policy(DeltaEventPolicy::new(4).unwrap());
        let timestamp = MachineTime::from_ticks(4);
        scheduler
            .schedule_at(timestamp, EventPriority::new(1), "inserted-first")
            .unwrap();
        scheduler
            .schedule_at(timestamp, EventPriority::new(0), "inserted-second")
            .unwrap();

        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.sequence,
            1
        );
        assert_eq!(
            scheduler.dispatch_next().unwrap().unwrap().event.sequence,
            0
        );

        let state = scheduler.export_state();
        assert_eq!(state.dispatched_at_timestamp, 2);
        assert_eq!(state.event_queue.last_dispatched.unwrap().sequence, 0);
        assert!(MachineScheduler::restore_state(state).is_ok());
    }

    #[test]
    fn restore_rejects_cross_component_dispatch_history_mismatch() {
        let mut state = scheduler_snapshot_fixture();
        state.dispatched_at_timestamp = 0;
        assert!(matches!(
            MachineScheduler::restore_state(state),
            Err(SchedulerError::DispatchHistoryMismatch)
        ));

        let mut state = scheduler_snapshot_fixture();
        state.last_dispatch_timestamp = Some(MachineTime::from_ticks(4));
        assert!(matches!(
            MachineScheduler::restore_state(state),
            Err(SchedulerError::DispatchWatermarkMismatch)
        ));
    }

    #[test]
    fn restore_rejects_noncanonical_domains_and_events_before_now() {
        let mut unsorted = scheduler_snapshot_fixture();
        unsorted.clock_domains.swap(0, 1);
        assert!(matches!(
            MachineScheduler::restore_state(unsorted),
            Err(SchedulerError::NonCanonicalClockDomainOrder)
        ));

        let mut event_before_now = scheduler_snapshot_fixture();
        event_before_now.event_queue.pending[0].timestamp = MachineInstant::from_ticks(4);
        assert!(matches!(
            MachineScheduler::restore_state(event_before_now),
            Err(SchedulerError::PendingEventBeforeNow)
        ));
    }

    #[test]
    fn restore_rejects_watermark_after_now_and_domain_time_mismatch() {
        let mut watermark_after_now = scheduler_snapshot_fixture();
        watermark_after_now
            .event_queue
            .last_dispatched
            .as_mut()
            .unwrap()
            .timestamp = MachineInstant::from_ticks(6);
        assert!(matches!(
            MachineScheduler::restore_state(watermark_after_now),
            Err(SchedulerError::WatermarkAfterNow)
        ));

        let mut domain_time_mismatch = scheduler_snapshot_fixture();
        domain_time_mismatch.clock_domains[0].current_time = MachineTime::from_ticks(4);
        assert!(matches!(
            MachineScheduler::restore_state(domain_time_mismatch),
            Err(SchedulerError::ClockDomainTimeMismatch(ClockDomainId(1)))
        ));
    }
}
