//! Versioned persistence-state model for one deterministic event queue.
//!
//! These DTOs are deliberately separate from the live queue implementation.
//! They define a canonical, codec-independent representation; validation and
//! heap reconstruction are implemented by the queue in a later milestone.

use crate::{HardwarePhase, MachineInstant, PlatformPriority, ScheduledEvent};

/// Causal watermark retained after the most recently dispatched event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventQueueWatermark {
    /// Absolute integer timestamp of the last dispatched event.
    pub timestamp: MachineInstant,
    /// Hardware-ordering stratum of the last dispatched event.
    pub hardware_phase: HardwarePhase,
    /// Software dispatch priority of the last dispatched event.
    pub priority: PlatformPriority,
    /// Stable queue sequence of the last dispatched event.
    pub sequence: u64,
}

/// Persistence DTO for an event queue.
///
/// `pending` is required to be in canonical ascending order by
/// `(timestamp, hardware_phase, priority, sequence)`, independent of the
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
