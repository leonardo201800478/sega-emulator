//! Shared Z80 CPU implementation, initially extracted from the SMS core.
//!
//! The CPU owns architectural state and instruction semantics. Machine-specific
//! memory maps, ports, interrupt wiring, and clock scheduling remain outside this
//! crate. The current executor preserves the SMS instruction-step behavior and
//! reports nominal T-states; it is not yet a complete micro-operation sequencer.

pub mod architectural_state;

pub mod alu {
    pub use sega_z80_alu::*;
}

pub mod bus {
    pub use sega_z80_bus::*;
}

pub mod cpu;
pub mod m1;
pub mod micro_ops;
pub mod sequencer;
pub mod snapshot;

pub const Z80_MICRO_SEQUENCER_STATE_VERSION: u16 = 17;

pub use architectural_state::{Z80ArchitecturalState, Z80ArchitecturalStateError};
pub use cpu::{CpuError, Z80, Z80AlternateRegisters, Z80Registers};
pub use m1::{Z80M1Cycle, Z80M1CycleState, Z80M1Error, Z80M1Phase, Z80M1Pins};
pub use micro_ops::{
    Z80BusHandshake, Z80BusHandshakeError, Z80BusHandshakeState, Z80BusOperation, Z80BusRequest,
    Z80BusResponse,
};
pub use sequencer::{
    Z80MicroSequencer, Z80MicroSequencerError, Z80MicroSequencerPhase, Z80MicroSequencerState,
    Z80MicroStep,
};
pub use snapshot::Z80MicroSequencerStateCodecError;
