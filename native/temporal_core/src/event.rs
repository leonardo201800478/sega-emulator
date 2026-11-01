//! Deterministic event ordering for one emulated machine timeline.

use core::cmp::Ordering;
use core::fmt;
use std::collections::BinaryHeap;

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
    /// Absolute timestamp in the machine profile's time unit.
    pub timestamp: super::MachineTime,
    /// Profile-defined ordering stratum at this timestamp.
    pub hardware_phase: HardwarePhase,
    /// Software dispatch priority within the hardware phase.
    pub priority: PlatformPriority,
    /// Stable insertion sequence assigned by the queue.
    pub sequence: u64,
    /// Event-specific typed payload.
    pub payload: T,
}


/// Causal watermark retained after the most recently dispatched event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventQueueWatermark {
    /// Absolute integer timestamp of the last dispatched event.
    pub timestamp: super::MachineTime,
    /// Hardware-ordering stratum of the last dispatched event.
    pub hardware_phase: HardwarePhase,
    /// Software dispatch priority of the last dispatched event.
    pub priority: PlatformPriority,
}

/// Canonical persistence DTO for one event queue.
///
/// Pending events must be sorted ascending by
/// (timestamp, hardware_phase, priority, sequence), independently of the
/// runtime heap layout. Sequence values are preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventQueueState<T> {
    /// Pending events in canonical dispatch order.
    pub pending: Vec<ScheduledEvent<T>>,
    /// Next sequence value to assign within this queue.
    pub next_sequence: u64,
    /// Causal watermark from the last dispatched event, if any.
    pub last_dispatched: Option<EventQueueWatermark>,
}

#[derive(Debug, Clone)]
struct QueueEntry<T> {
    event: ScheduledEvent<T>,
}

impl<T> QueueEntry<T> {
    fn key(&self) -> (super::MachineTime, HardwarePhase, PlatformPriority, u64) {
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
        }
    }
}
impl std::error::Error for EventQueueError {}

/// Min-priority queue ordered by timestamp, hardware phase, platform priority,
/// then stable insertion sequence. The queue does not accept rational sub-tick
/// timestamps; HardwarePhase is not a substitute for one.
#[derive(Debug, Clone)]
pub struct EventQueue<T> {
    heap: BinaryHeap<QueueEntry<T>>,
    next_sequence: u64,
    last_dispatched: Option<(super::MachineTime, HardwarePhase, PlatformPriority)>,
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

    /// Insert using the compatibility zero-phase ordering.
    pub fn schedule(
        &mut self,
        timestamp: super::MachineTime,
        priority: PlatformPriority,
        payload: T,
    ) -> Result<u64, EventQueueError> {
        self.schedule_with_phase(timestamp, HardwarePhase::default(), priority, payload)
    }

    /// Insert an event with independent phase and platform-priority keys.
    pub fn schedule_with_phase(
        &mut self,
        timestamp: super::MachineTime,
        hardware_phase: HardwarePhase,
        priority: PlatformPriority,
        payload: T,
    ) -> Result<u64, EventQueueError> {
        if let Some((last_time, last_phase, last_priority)) = self.last_dispatched {
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
        ));
        Some(entry.event)
    }
    /// Return the next queued absolute timestamp.
    pub fn next_timestamp(&self) -> Option<super::MachineTime> {
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
    use super::{EventPriority, EventQueue, EventQueueError, HardwarePhase, PlatformPriority};
    use crate::MachineTime;

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
}
