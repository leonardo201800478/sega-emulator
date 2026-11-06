//! Temporary Rust SG-1000 reference core retained during the SMS-first migration.
//!
//! This instruction-boundary implementation is not an active product runtime.
//! It remains in the workspace temporarily for regression/reference purposes
//! and will be retired after the SMS runtime is executable and validated. The
//! future SG-1000 implementation will reuse the shared Z80 and temporal devices.
//!
//! Host controller APIs are normalized at the application boundary; the core
//! accepts deterministic active-low snapshots without polling SDL or Windows.

/// Size of the Z80's logical address space.
pub const SG1000_CPU_ADDRESS_SPACE_SIZE: usize = 0x1_0000;

pub mod alu;
pub mod bus;
pub mod cpu;
pub mod input;
pub mod io;
pub mod machine;
pub mod memory;
pub mod psg;
pub mod temporal;
pub mod temporal_bus;
pub mod vdp;

pub use bus::CpuBus;
pub use cpu::{CpuError, Z80};
pub use input::{
    ControllerBackend, ControllerBackendPreference, ControllerButtons, ControllerFrame,
};
pub use io::{IoError, PsgWriteEvent, Sg1000Io};
pub use machine::{MachineError, Sg1000Machine};
pub use memory::{MemoryConfig, MemoryError, Sg1000Memory};
pub use psg::{PsgError, Sn76489};
pub use temporal::{
    Sg1000TemporalCoordinator, Sg1000TemporalCoordinatorError, Sg1000TemporalCoordinatorState,
    Sg1000TemporalEvent,
};
pub use temporal_bus::{
    Sg1000BusCompletion, Sg1000InternalCycleCompletion, Sg1000TemporalBus, Sg1000TemporalBusError,
    Sg1000TemporalBusState, Sg1000TemporalDispatch,
};
pub use vdp::Tms9918A;
