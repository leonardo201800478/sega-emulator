//! Deterministic event ordering for one emulated machine timeline.

use core::cmp::Ordering;
use core::fmt;
use std::collections::BinaryHeap;

/// Platform-defined event ordering class.
///
/// Lower numeric values are dispatched first at the same timestamp. The
/// temporal core intentionally does not assign hardware meanings to values.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventPriority(u16);

impl EventPriority {
    /// Construct a platform-defined priority value.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Return the underlying priority value.
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// One event removed from an EventQueue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledEvent<T> {
    /// Absolute timestamp in the machine profile's time unit.
    pub timestamp: super::MachineTime,
    /// Platform-defined ordering class.
    pub priority: EventPriority,
    /// Stable insertion sequence assigned by the queue.
    pub sequence: u64,
    /// Event-specific typed payload.
    pub payload: T,
}

#[derive(Debug)]
struct QueueEntry<T> {
    event: ScheduledEvent<T>,
}

impl<T> QueueEntry<T> {
    fn ordering_key(&self) -> (super::MachineTime, EventPriority, u64) {
        (self.event.timestamp, self.event.priority, self.event.sequence)
    }
}

impl<T> PartialEq for QueueEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ordering_key() == other.ordering_key()
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
        other.ordering_key().cmp(&self.ordering_key())
    }
}

/// Errors returned when inserting an event would violate timeline causality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventQueueError {
    /// No further insertion sequence can be represented.
    SequenceExhausted,
    /// An event was scheduled before the last dispatched timestamp.
    EventInPast,
    /// A same-timestamp event was inserted with an earlier priority after
    /// that timestamp had already begun dispatching.
    PriorityRegressionAtCurrentTime,
}

impl fmt::Display for EventQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequenceExhausted => write!(f, "event insertion sequence exhausted"),
            Self::EventInPast => write!(f, "cannot schedule an event before dispatched time"),
            Self::PriorityRegressionAtCurrentTime => {
                write!(f, "cannot insert an earlier priority at the current timestamp")
            }
        }
    }
}

impl std::error::Error for EventQueueError {}

/// Min-priority event queue with stable deterministic tie-breaking.
///
/// Events are ordered by timestamp, platform priority, and insertion sequence.
/// Producers must enqueue same-timestamp events of earlier priority before
/// dispatch reaches that timestamp. Causal events emitted while handling an
/// event may use the same timestamp only at the same or a later priority.
#[derive(Debug, Clone)]
pub struct EventQueue<T> {
    heap: BinaryHeap<QueueEntry<T>>,
    next_sequence: u64,
    last_dispatched: Option<(super::MachineTime, EventPriority)>,
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

    /// Insert an event and return its stable insertion sequence.
    pub fn schedule(
        &mut self,
        timestamp: super::MachineTime,
        priority: EventPriority,
        payload: T,
    ) -> Result<u64, EventQueueError> {
        if let Some((last_time, last_priority)) = self.last_dispatched {
            if timestamp < last_time {
                return Err(EventQueueError::EventInPast);
            }
            if timestamp == last_time && priority < last_priority {
                return Err(EventQueueError::PriorityRegressionAtCurrentTime);
            }
        }

        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.checked_add(1)
            .ok_or(EventQueueError::SequenceExhausted)?;
        self.heap.push(QueueEntry {
            event: ScheduledEvent { timestamp, priority, sequence, payload },
        });
        Ok(sequence)
    }

    /// Remove the next event in timestamp/priority/sequence order.
    pub fn pop_next(&mut self) -> Option<ScheduledEvent<T>> {
        let entry = self.heap.pop()?;
        self.last_dispatched = Some((entry.event.timestamp, entry.event.priority));
        Some(entry.event)
    }

    /// Return the timestamp of the next queued event, if any.
    pub fn next_timestamp(&self) -> Option<super::MachineTime> {
        self.heap.peek().map(|entry| entry.event.timestamp)
    }

    /// Return the number of queued events.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Return whether the queue contains no events.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Remove all events and reset ordering history and insertion sequence.
    pub fn clear(&mut self) {
        self.heap.clear();
        self.next_sequence = 0;
        self.last_dispatched = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{EventPriority, EventQueue, EventQueueError};
    use crate::MachineTime;

    #[test]
    fn events_are_ordered_by_time_then_priority_then_insertion_sequence() {
        let mut queue = EventQueue::new();
        let time_8 = MachineTime::from_ticks(8);
        let time_4 = MachineTime::from_ticks(4);

        queue.schedule(time_8, EventPriority::new(2), "late-priority").unwrap();
        queue.schedule(time_4, EventPriority::new(5), "early-time").unwrap();
        queue.schedule(time_8, EventPriority::new(1), "early-priority-a").unwrap();
        queue.schedule(time_8, EventPriority::new(1), "early-priority-b").unwrap();

        let events: Vec<_> = core::iter::from_fn(|| queue.pop_next())
            .map(|event| event.payload)
            .collect();
        assert_eq!(
            events,
            vec!["early-time", "early-priority-a", "early-priority-b", "late-priority"]
        );
    }

    #[test]
    fn causal_same_time_events_cannot_retroactively_change_priority_order() {
        let mut queue = EventQueue::new();
        let now = MachineTime::from_ticks(10);
        queue.schedule(now, EventPriority::new(3), "first").unwrap();
        assert_eq!(queue.pop_next().unwrap().payload, "first");

        assert_eq!(
            queue.schedule(now, EventPriority::new(2), "retroactive"),
            Err(EventQueueError::PriorityRegressionAtCurrentTime)
        );
        assert_eq!(queue.schedule(now, EventPriority::new(3), "causal"), Ok(1));
        assert_eq!(queue.pop_next().unwrap().payload, "causal");
    }

    #[test]
    fn events_cannot_be_scheduled_before_dispatched_time() {
        let mut queue = EventQueue::new();
        queue.schedule(MachineTime::from_ticks(10), EventPriority::new(0), "now").unwrap();
        queue.pop_next();

        assert_eq!(
            queue.schedule(MachineTime::from_ticks(9), EventPriority::new(0), "past"),
            Err(EventQueueError::EventInPast)
        );
    }

    #[test]
    fn clear_resets_queue_and_sequence() {
        let mut queue = EventQueue::new();
        assert_eq!(
            queue.schedule(MachineTime::from_ticks(1), EventPriority::new(0), 1),
            Ok(0)
        );
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(
            queue.schedule(MachineTime::from_ticks(2), EventPriority::new(0), 2),
            Ok(0)
        );
    }

    #[test]
    fn queue_reports_next_timestamp_and_length() {
        let mut queue = EventQueue::new();
        queue.schedule(MachineTime::from_ticks(5), EventPriority::new(0), ()).unwrap();
        queue.schedule(MachineTime::from_ticks(3), EventPriority::new(0), ()).unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.next_timestamp(), Some(MachineTime::from_ticks(3)));
    }
}
