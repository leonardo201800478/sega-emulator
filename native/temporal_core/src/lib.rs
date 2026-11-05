//! Shared deterministic time primitives for Sega hardware emulation.
//!
//! This crate deliberately does not model any console's clock frequencies or
//! event priorities. Each machine profile supplies those hardware-specific
//! facts while one machine scheduler owns the timeline and coordinates devices.

pub mod clock;
pub mod contract;
pub mod event;
pub mod state;

pub use clock::{
    ClockDomain, ClockDomainSnapshot, ClockEdgeTime, ClockError, ClockRatio, MachineInstant,
    MachineTime,
};
pub use contract::{
    BusContractError, BusCycleKind, BusDirection, BusOperationResult, BusPhase, BusTransaction,
    DeviceProgress, MCycleIndex, TStateIndex,
};
pub use event::{
    EventPriority, EventQueue, EventQueueError, HardwarePhase, PlatformPriority, ScheduledEvent,
};
pub use state::{EventQueueState, EventQueueWatermark};
