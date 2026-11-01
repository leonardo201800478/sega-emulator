//! Persistence-state DTOs for the machine scheduler.
//!
//! The DTOs in this module are a stable, versioned state model, not live
//! scheduler objects. Runtime export, validation, and restoration are added
//! in subsequent snapshot milestones.

use sega_temporal_core::{EventQueueState, MachineTime};

use crate::ClockDomainId;

/// Version of the top-level scheduler state schema.
pub const SCHEDULER_STATE_FORMAT_VERSION: u16 = 1;

/// Complete persistence DTO for one registered clock domain.
///
/// The registration origin and initial phase are retained separately from
/// the current clock state so restoration can verify the exact arithmetic
/// history rather than infer it from the current remainder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockDomainState {
    /// Stable scheduler-local identity of this clock domain.
    pub id: ClockDomainId,
    /// Numerator of local cycles per master-time tick.
    pub ratio_numerator: u64,
    /// Denominator of local cycles per master-time tick.
    pub ratio_denominator: u64,
    /// Integer machine timestamp through which the domain was advanced.
    pub current_time: MachineTime,
    /// Accumulated local clock edges since registration.
    pub total_cycles: u128,
    /// Retained rational phase; must be less than `ratio_denominator`.
    pub phase: u128,
    /// Integer machine timestamp at which this domain was registered.
    pub registration_start: MachineTime,
    /// Rational phase supplied when the domain was registered.
    pub initial_phase: u128,
}

/// Complete persistence DTO for a machine scheduler.
///
/// Clock domains must be ordered by ascending `id`. `event_queue.pending`
/// uses the canonical event ordering documented by `EventQueueState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerState<T> {
    /// Schema version. Unknown versions must be rejected by the codec.
    pub format_version: u16,
    /// Current absolute integer machine timestamp.
    pub now: MachineTime,
    /// All registered domains in canonical ascending ID order.
    pub clock_domains: Vec<ClockDomainState>,
    /// Event queue state, including its allocator and causal watermark.
    pub event_queue: EventQueueState<T>,
    /// Maximum number of events dispatchable at one timestamp; non-zero.
    pub delta_event_limit: u64,
    /// Timestamp of the most recent scheduler dispatch, if any.
    pub last_dispatch_timestamp: Option<MachineTime>,
    /// Number of events dispatched at `last_dispatch_timestamp`.
    pub dispatched_at_timestamp: u64,
}
