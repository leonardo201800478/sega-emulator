//! Deterministic event ordering for one emulated machine timeline.

use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeSet, BinaryHeap};

use crate::{EventQueueState, EventQueueWatermark};

/// Profile-defined ordering stratum at one absolute timestamp.
///
/// This is an ordinal tie-break label, not a duration, electrical phase
/// measurement, or sub-tick timestamp. Lower values dispatch first.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HardwarePhase(u16);

impl HardwarePhase {
    /// Construct a profile-defined ordering stratum.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Return the profile-defined stratum.
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Software dispatch priority, independent of hardware phase.
///
/// Lower values dispatch first within the same timestamp and hardware phase.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlatformPriority(u16);

impl PlatformPriority {
    /// Construct a platform dispatch priority.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Return the priority value.
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Backward-compatible name for the former single priority dimension.
pub type EventPriority = PlatformPriority;

/// One event removed from an EventQueue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledEvent<T> {
    /// Exact absolute rational timestamp on the machine timeline.
    pub timestamp: super::MachineInstant,
    /// Profile-defined ordering stratum at this timestamp.
    pub hardware_phase: HardwarePhase,
    /// Software dispatch priority within the hardware phase.
    pub priority: PlatformPriority,
    /// Stable insertion sequence assigned by the queue.
    pub sequence: u64,
    /// Event-specific typed payload.
    pub payload: T,
}

#[derive(Debug, Clone)]
struct QueueEntry<T> {
    event: ScheduledEvent<T>,
}

impl<T> QueueEntry<T> {
    fn key(&self) -> (super::MachineInstant, HardwarePhase, PlatformPriority, u64) {
        (
            self.event.timestamp,
            self.event.hardware_phase,
            self.event.priority,
            self.event.sequence,
        )
    }
}
impl<T> PartialEq for QueueEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl<T> Eq for QueueEntry<T> {}

impl<T> PartialOrd for QueueEntry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for QueueEntry<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.key().cmp(&self.key())
    }
}

/// Errors returned when inserting an event would violate timeline causality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventQueueError {
    /// No further insertion sequence can be represented.
    SequenceExhausted,
    /// An event was scheduled before the last dispatched timestamp.
    EventInPast,
    /// A same-timestamp event regressed the dispatched phase/priority key.
    OrderingRegressionAtCurrentTime,
    DuplicateSequence,
    SequenceOutOfRange,
    NonCanonicalState,
    WatermarkRegression,
}
impl fmt::Display for EventQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequenceExhausted => write!(f, "event insertion sequence exhausted"),
            Self::EventInPast => write!(f, "cannot schedule an event before dispatched time"),
            Self::OrderingRegressionAtCurrentTime => write!(
                f,
                "cannot insert an earlier phase/priority at the current timestamp"
            ),
            Self::DuplicateSequence => write!(f, "duplicate pending event sequence"),
            Self::SequenceOutOfRange => {
                write!(f, "pending event sequence is not below next_sequence")
            }
            Self::NonCanonicalState => write!(f, "pending events are not in canonical order"),
            Self::WatermarkRegression => write!(f, "pending event regresses the causal watermark"),
        }
    }
}
impl std::error::Error for EventQueueError {}

/// Min-priority queue ordered by exact rational timestamp, hardware phase,
/// platform priority, then stable insertion sequence. HardwarePhase remains
/// an ordinal tie-break and never substitutes for physical elapsed time.
#[derive(Debug, Clone)]
pub struct EventQueue<T> {
    heap: BinaryHeap<QueueEntry<T>>,
    next_sequence: u64,
    last_dispatched: Option<(super::MachineInstant, HardwarePhase, PlatformPriority, u64)>,
}
impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self {
            heap: BinaryHeap::new(),
            next_sequence: 0,
            last_dispatched: None,
        }
    }
}
impl<T> EventQueue<T> {
    /// Create an empty event queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Export pending events in canonical order, preserving assigned sequences.
    pub fn export_state(&self) -> EventQueueState<T>
    where
        T: Clone,
    {
        let mut pending: Vec<_> = self.heap.iter().map(|entry| entry.event.clone()).collect();
        pending.sort_by_key(|event| {
            (
                event.timestamp,
                event.hardware_phase,
                event.priority,
                event.sequence,
            )
        });
        EventQueueState {
            pending,
            next_sequence: self.next_sequence,
            last_dispatched: self.last_dispatched.map(
                |(timestamp, hardware_phase, priority, sequence)| EventQueueWatermark {
                    timestamp,
                    hardware_phase,
                    priority,
                    sequence,
                },
            ),
        }
    }

    /// Rebuild the heap from a validated canonical DTO without reallocating sequences.
    pub fn restore_state(state: EventQueueState<T>) -> Result<Self, EventQueueError> {
        if state
            .last_dispatched
            .is_some_and(|watermark| watermark.sequence >= state.next_sequence)
        {
            return Err(EventQueueError::SequenceOutOfRange);
        }

        let mut sequences = BTreeSet::new();
        let mut previous_key = None;
        for event in &state.pending {
            if !sequences.insert(event.sequence) {
                return Err(EventQueueError::DuplicateSequence);
            }
            if event.sequence >= state.next_sequence {
                return Err(EventQueueError::SequenceOutOfRange);
            }
            let key = (
                event.timestamp,
                event.hardware_phase,
                event.priority,
                event.sequence,
            );
            if previous_key.is_some_and(|previous| previous >= key) {
                return Err(EventQueueError::NonCanonicalState);
            }
            previous_key = Some(key);
            if let Some(watermark) = state.last_dispatched {
                if event.sequence == watermark.sequence {
                    return Err(EventQueueError::DuplicateSequence);
                }
                let event_key = (
                    event.timestamp,
                    event.hardware_phase,
                    event.priority,
                    event.sequence,
                );
                let watermark_key = (
                    watermark.timestamp,
                    watermark.hardware_phase,
                    watermark.priority,
                    watermark.sequence,
                );
                if event_key <= watermark_key {
                    return Err(EventQueueError::WatermarkRegression);
                }
            }
        }
        let heap = state
            .pending
            .into_iter()
            .map(|event| QueueEntry { event })
            .collect();
        let last_dispatched = state
            .last_dispatched
            .map(|w| (w.timestamp, w.hardware_phase, w.priority, w.sequence));
        Ok(Self {
            heap,
            next_sequence: state.next_sequence,
            last_dispatched,
        })
    }

    /// Insert using the compatibility zero-phase ordering.
    pub fn schedule(
        &mut self,
        timestamp: impl Into<super::MachineInstant>,
        priority: PlatformPriority,
        payload: T,
    ) -> Result<u64, EventQueueError> {
        self.schedule_with_phase(timestamp, HardwarePhase::default(), priority, payload)
    }

    /// Insert an event with independent phase and platform-priority keys.
    pub fn schedule_with_phase(
        &mut self,
        timestamp: impl Into<super::MachineInstant>,
        hardware_phase: HardwarePhase,
        priority: PlatformPriority,
        payload: T,
    ) -> Result<u64, EventQueueError> {
        let timestamp = timestamp.into();
        if let Some((last_time, last_phase, last_priority, _last_sequence)) = self.last_dispatched {
            if timestamp < last_time {
                return Err(EventQueueError::EventInPast);
            }
            if timestamp == last_time && (hardware_phase, priority) < (last_phase, last_priority) {
                return Err(EventQueueError::OrderingRegressionAtCurrentTime);
            }
        }
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(EventQueueError::SequenceExhausted)?;
        self.heap.push(QueueEntry {
            event: ScheduledEvent {
                timestamp,
                hardware_phase,
                priority,
                sequence,
                payload,
            },
        });
        Ok(sequence)
    }

    /// Remove the next event in timestamp/phase/priority/sequence order.
    pub fn pop_next(&mut self) -> Option<ScheduledEvent<T>> {
        let entry = self.heap.pop()?;
        self.last_dispatched = Some((
            entry.event.timestamp,
            entry.event.hardware_phase,
            entry.event.priority,
            entry.event.sequence,
        ));
        Some(entry.event)
    }
    /// Return the next queued absolute timestamp.
    pub fn next_timestamp(&self) -> Option<super::MachineInstant> {
        self.heap.peek().map(|entry| entry.event.timestamp)
    }
    /// Return the number of queued events.
    pub fn len(&self) -> usize {
        self.heap.len()
    }
    /// Return whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
    /// Clear events and reset sequence and causal history.
    pub fn clear(&mut self) {
        self.heap.clear();
        self.next_sequence = 0;
        self.last_dispatched = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EventPriority, EventQueue, EventQueueError, HardwarePhase, PlatformPriority, ScheduledEvent,
    };
    use crate::{EventQueueState, EventQueueWatermark, MachineInstant, MachineTime};

    #[test]
    fn events_order_by_absolute_time_phase_priority_and_stable_sequence() {
        let mut q = EventQueue::new();
        let t = MachineTime::from_ticks(8);
        q.schedule_with_phase(t, HardwarePhase::new(2), PlatformPriority::new(0), "phase2")
            .unwrap();
        q.schedule_with_phase(
            t,
            HardwarePhase::new(1),
            PlatformPriority::new(2),
            "priority2",
        )
        .unwrap();
        q.schedule_with_phase(t, HardwarePhase::new(1), PlatformPriority::new(1), "first")
            .unwrap();
        q.schedule_with_phase(t, HardwarePhase::new(1), PlatformPriority::new(1), "second")
            .unwrap();
        q.schedule(
            MachineTime::from_ticks(4),
            EventPriority::new(9),
            "earlier-time",
        )
        .unwrap();
        let events: Vec<_> = core::iter::from_fn(|| q.pop_next())
            .map(|event| event.payload)
            .collect();
        assert_eq!(
            events,
            vec!["earlier-time", "first", "second", "priority2", "phase2"]
        );
    }

    #[test]
    fn rational_subtick_instants_precede_phase_and_priority_ordering() {
        let mut queue = EventQueue::new();
        let one_third =
            MachineInstant::from_parts(MachineTime::ZERO, 1, 3).unwrap();
        let one_half =
            MachineInstant::from_parts(MachineTime::ZERO, 1, 2).unwrap();

        queue
            .schedule_with_phase(
                one_half,
                HardwarePhase::new(0),
                PlatformPriority::new(0),
                "one-half",
            )
            .unwrap();
        queue
            .schedule_with_phase(
                one_third,
                HardwarePhase::new(9),
                PlatformPriority::new(9),
                "one-third",
            )
            .unwrap();

        assert_eq!(queue.pop_next().unwrap().payload, "one-third");
        assert_eq!(
            queue.schedule_with_phase(
                one_third,
                HardwarePhase::new(9),
                PlatformPriority::new(9),
                "same-instant",
            ),
            Ok(2)
        );
        let earlier = MachineInstant::from_parts(MachineTime::ZERO, 1, 4).unwrap();
        assert_eq!(
            queue.schedule(earlier, PlatformPriority::new(0), "causal-regression"),
            Err(EventQueueError::EventInPast)
        );
    }

    #[test]
    fn causal_events_cannot_regress_phase_or_priority() {
        let mut q = EventQueue::new();
        let t = MachineTime::from_ticks(10);
        q.schedule_with_phase(t, HardwarePhase::new(2), PlatformPriority::new(3), "origin")
            .unwrap();
        q.pop_next();
        assert_eq!(
            q.schedule_with_phase(
                t,
                HardwarePhase::new(1),
                PlatformPriority::new(9),
                "phase-back"
            ),
            Err(EventQueueError::OrderingRegressionAtCurrentTime)
        );
        assert_eq!(
            q.schedule_with_phase(
                t,
                HardwarePhase::new(2),
                PlatformPriority::new(2),
                "priority-back"
            ),
            Err(EventQueueError::OrderingRegressionAtCurrentTime)
        );
        assert_eq!(
            q.schedule_with_phase(t, HardwarePhase::new(2), PlatformPriority::new(3), "same"),
            Ok(1)
        );
        assert_eq!(
            q.schedule_with_phase(
                t,
                HardwarePhase::new(3),
                PlatformPriority::new(0),
                "later-phase"
            ),
            Ok(2)
        );
    }

    #[test]
    fn compatibility_schedule_uses_zero_phase_and_sequence_is_stable() {
        let mut q = EventQueue::new();
        let t = MachineTime::from_ticks(1);
        assert_eq!(q.schedule(t, EventPriority::new(0), "a"), Ok(0));
        assert_eq!(q.schedule(t, EventPriority::new(0), "b"), Ok(1));
        assert_eq!(q.pop_next().unwrap().payload, "a");
        assert_eq!(q.pop_next().unwrap().payload, "b");
    }

    #[test]
    fn past_events_are_rejected_and_clear_resets_history() {
        let mut q = EventQueue::new();
        q.schedule(MachineTime::from_ticks(10), EventPriority::new(0), ())
            .unwrap();
        q.pop_next();
        assert_eq!(
            q.schedule(MachineTime::from_ticks(9), EventPriority::new(0), ()),
            Err(EventQueueError::EventInPast)
        );
        q.clear();
        assert_eq!(
            q.schedule(MachineTime::from_ticks(2), EventPriority::new(0), ()),
            Ok(0)
        );
    }

    #[test]
    fn queue_snapshot_round_trip_preserves_sequences_watermark_and_allocator() {
        let mut queue = EventQueue::new();
        let t = MachineTime::from_ticks(10);
        queue
            .schedule_with_phase(t, HardwarePhase::new(1), PlatformPriority::new(2), "b")
            .unwrap();
        queue
            .schedule_with_phase(t, HardwarePhase::new(1), PlatformPriority::new(1), "a")
            .unwrap();
        queue
            .schedule(MachineTime::from_ticks(12), PlatformPriority::new(0), "c")
            .unwrap();
        assert_eq!(queue.pop_next().unwrap().payload, "a");
        let state = queue.export_state();
        assert_eq!(state.next_sequence, 3);
        assert_eq!(
            state
                .pending
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        let mut restored = EventQueue::restore_state(state.clone()).unwrap();
        assert_eq!(restored.export_state(), state);
        assert_eq!(
            restored.schedule(MachineTime::from_ticks(13), PlatformPriority::new(0), "d"),
            Ok(3)
        );
        assert_eq!(restored.pop_next().unwrap().payload, "b");
    }

    #[test]
    fn restore_rejects_duplicate_and_out_of_range_sequences() {
        let event = |sequence| ScheduledEvent {
            timestamp: MachineInstant::from_ticks(4),
            hardware_phase: HardwarePhase::new(0),
            priority: PlatformPriority::new(0),
            sequence,
            payload: (),
        };
        let duplicate = EventQueueState {
            pending: vec![event(0), event(0)],
            next_sequence: 1,
            last_dispatched: None,
        };
        assert_eq!(
            EventQueue::restore_state(duplicate).err(),
            Some(EventQueueError::DuplicateSequence)
        );
        let out_of_range = EventQueueState {
            pending: vec![event(2)],
            next_sequence: 2,
            last_dispatched: None,
        };
        assert_eq!(
            EventQueue::restore_state(out_of_range).err(),
            Some(EventQueueError::SequenceOutOfRange)
        );
    }

    #[test]
    fn restore_rejects_noncanonical_order_and_watermark_regression() {
        let event = |timestamp, phase, priority, sequence| ScheduledEvent {
            timestamp: MachineInstant::from_ticks(timestamp),
            hardware_phase: HardwarePhase::new(phase),
            priority: PlatformPriority::new(priority),
            sequence,
            payload: (),
        };
        let reversed = EventQueueState {
            pending: vec![event(5, 1, 0, 0), event(5, 0, 0, 1)],
            next_sequence: 2,
            last_dispatched: None,
        };
        assert_eq!(
            EventQueue::restore_state(reversed).err(),
            Some(EventQueueError::NonCanonicalState)
        );
        let regressed = EventQueueState {
            pending: vec![event(5, 1, 0, 0)],
            next_sequence: 2,
            last_dispatched: Some(EventQueueWatermark {
                timestamp: MachineInstant::from_ticks(5),
                hardware_phase: HardwarePhase::new(2),
                priority: PlatformPriority::new(0),
                sequence: 1,
            }),
        };
        assert_eq!(
            EventQueue::restore_state(regressed).err(),
            Some(EventQueueError::WatermarkRegression)
        );
    }

    #[test]
    fn restore_validates_full_watermark_sequence_tiebreak() {
        let event = |sequence| ScheduledEvent {
            timestamp: MachineInstant::from_ticks(5),
            hardware_phase: HardwarePhase::new(1),
            priority: PlatformPriority::new(1),
            sequence,
            payload: (),
        };
        let watermark = EventQueueWatermark {
            timestamp: MachineInstant::from_ticks(5),
            hardware_phase: HardwarePhase::new(1),
            priority: PlatformPriority::new(1),
            sequence: 1,
        };

        let regressed = EventQueueState {
            pending: vec![event(0)],
            next_sequence: 2,
            last_dispatched: Some(watermark),
        };
        assert_eq!(
            EventQueue::restore_state(regressed).err(),
            Some(EventQueueError::WatermarkRegression)
        );

        let valid = EventQueueState {
            pending: vec![event(2)],
            next_sequence: 3,
            last_dispatched: Some(watermark),
        };
        assert!(EventQueue::restore_state(valid).is_ok());

        let reused_dispatched_sequence = EventQueueState {
            pending: vec![ScheduledEvent {
                timestamp: MachineInstant::from_ticks(6),
                hardware_phase: HardwarePhase::new(0),
                priority: PlatformPriority::new(0),
                sequence: 1,
                payload: (),
            }],
            next_sequence: 2,
            last_dispatched: Some(watermark),
        };
        assert_eq!(
            EventQueue::restore_state(reused_dispatched_sequence).err(),
            Some(EventQueueError::DuplicateSequence)
        );
    }

    #[test]
    fn restore_rejects_watermark_sequence_outside_allocator_range() {
        let state = EventQueueState::<()> {
            pending: vec![],
            next_sequence: 2,
            last_dispatched: Some(EventQueueWatermark {
                timestamp: MachineInstant::from_ticks(5),
                hardware_phase: HardwarePhase::new(1),
                priority: PlatformPriority::new(1),
                sequence: 2,
            }),
        };
        assert_eq!(
            EventQueue::restore_state(state).err(),
            Some(EventQueueError::SequenceOutOfRange)
        );
    }

    #[test]
    fn restore_preserves_exhausted_sequence_allocator() {
        let state = EventQueueState::<()> {
            pending: vec![],
            next_sequence: u64::MAX,
            last_dispatched: None,
        };
        let mut queue = EventQueue::restore_state(state).unwrap();
        assert_eq!(
            queue.schedule(MachineTime::ZERO, PlatformPriority::new(0), ()),
            Err(EventQueueError::SequenceExhausted)
        );
    }
}
