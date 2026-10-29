//! SMS adapter for the hardware-neutral Z80 bus contract.
//!
//! The CPU is shared only at the trait boundary. Memory maps and device port
//! decoding remain owned by the SMS machine.
pub use sega_z80_bus::{
    CpuBus, Z80BusEvent, Z80BusEventKind, Z80CpuSnapshot, Z80InstructionOutcome,
    Z80InstructionTrace, Z80TStatePhase, Z80TStateSample,
};
