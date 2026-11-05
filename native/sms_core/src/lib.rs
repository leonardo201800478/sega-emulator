//! Standalone, Python-independent Sega Master System core.
//!
//! The Scheduler-driven SMS path now combines the shared Z80 microsequencer,
//! mapper-backed memory, controller I/O, SN76489, and the shared SMS1 VDP with
//! scanline/frame output. It remains a staged temporal implementation: dot-level
//! raster/fetch arbitration, physical WAIT wiring, complete machine interrupt
//! integration, host audio output, and persistent serialized save states remain
//! separate acceptance gates. The legacy machine is retained during migration.

pub mod alu;
pub mod bus;
pub mod cartridge;
pub mod config;
pub mod cpu;
pub mod error;
pub mod input;
pub mod io;
pub mod machine;
pub mod memory;
pub mod psg;
pub mod temporal;
pub mod temporal_bus;
pub mod temporal_m1;
pub mod temporal_machine;
pub mod vdp;

pub use bus::{
    CpuBus, Z80BusEvent, Z80BusEventKind, Z80CpuSnapshot, Z80InstructionOutcome,
    Z80InstructionTrace, Z80TStatePhase, Z80TStateSample,
};
pub use cartridge::CartridgeImage;
pub use config::{
    FmHardware, SmsConsoleModel, SmsMachineConfig, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode,
    Territory, VideoStandard,
};
pub use cpu::{CpuError, Z80};
pub use error::SmsCoreError;
pub use input::{ControllerButtons, LightPhaserState, SmsInputState};
pub use io::{SmsIo, SmsIoError};
pub use machine::{SegaScopeLens, SegaScopeState, SmsMachine, SmsMachineBus};
pub use memory::SmsMemory;
pub use psg::{PsgError, Sn76489};
pub use temporal::{
    SMS_PSG_CLOCK_DOMAIN_ID, SMS_TEMPORAL_COORDINATOR_STATE_VERSION, SMS_VDP_CLOCK_DOMAIN_ID,
    SMS_Z80_CLOCK_DOMAIN_ID,
    SmsTemporalCoordinator, SmsTemporalCoordinatorError, SmsTemporalCoordinatorState,
    SmsTemporalEvent,
};
pub use temporal_bus::{
    SMS_TEMPORAL_BUS_STATE_VERSION, SmsBusCompletion, SmsInternalCycleCompletion, SmsTemporalBus,
    SmsTemporalBusError, SmsTemporalBusState, SmsTemporalDispatch,
};
pub use temporal_m1::{SmsTemporalM1, SmsTemporalM1Error, SmsTemporalM1State};
pub use temporal_machine::{
    SMS_TEMPORAL_MACHINE_STATE_VERSION, SmsTemporalMachine, SmsTemporalMachineError,
    SmsTemporalMachineState,
};
pub use vdp::{
    SMS_VDP_MAX_HEIGHT, SMS_VDP_WIDTH, SmsRasterPosition, SmsVdp, SmsVdpEye,
    StereoPresentation,
};
