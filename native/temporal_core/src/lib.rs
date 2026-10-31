//! Shared deterministic time primitives for Sega hardware emulation.
//!
//! This crate deliberately does not model any console's clock frequencies or
//! event priorities. Each machine profile supplies those hardware-specific
//! facts while one machine scheduler owns the timeline and coordinates devices.

pub mod clock;
pub mod event;

pub use clock::{ClockDomain, ClockDomainSnapshot, ClockError, ClockRatio, MachineTime};
pub use event::{EventPriority, EventQueue, EventQueueError, ScheduledEvent};
