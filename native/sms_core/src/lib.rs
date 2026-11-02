//! Standalone, Python-independent Sega Master System core.
//!
//! The SMS-owned Z80 instruction engine, standard cartridge mapper/work RAM,
//! and initial CPU-to-memory/I/O integration are present. VDP, audio, full
//! interrupt behavior and the machine scheduler remain separate milestones;
//! this crate does not yet represent a complete runnable console.

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
pub mod vdp;
pub mod vdp_sms1;
pub mod temporal_bus;
pub mod temporal_machine;

pub use bus::{
    CpuBus, Z80BusEvent, Z80BusEventKind, Z80CpuSnapshot, Z80InstructionOutcome,
    Z80InstructionTrace, Z80TStatePhase, Z80TStateSample,
};
pub use cartridge::CartridgeImage;
pub use config::{
    FmHardware, SmsConsoleModel, SmsMachineConfig, SmsVdpRevision, SpriteLimitMode, Territory,
    VideoStandard,
};
pub use cpu::{CpuError, Z80};
pub use error::SmsCoreError;
pub use input::{ControllerButtons, LightPhaserState, SmsInputState};
pub use io::{SmsIo, SmsIoError};
pub use machine::{SegaScopeLens, SegaScopeState, SmsMachine, SmsMachineBus};
pub use memory::SmsMemory;
pub use psg::{PsgError, Sn76489};
pub use temporal_bus::{
    SmsBusCompletion, SmsInternalCycleCompletion, SmsTemporalBus, SmsTemporalBusError,
    SmsTemporalBusState, SmsTemporalDispatch,
    SMS_TEMPORAL_BUS_STATE_VERSION,
};
pub use temporal_machine::{
    SmsTemporalMachine, SmsTemporalMachineError, SmsTemporalMachineState,
    SMS_TEMPORAL_MACHINE_STATE_VERSION,
};
pub use temporal::{
    SMS_TEMPORAL_COORDINATOR_STATE_VERSION, SMS_Z80_CLOCK_DOMAIN_ID, SmsTemporalCoordinator,
    SmsTemporalCoordinatorError, SmsTemporalCoordinatorState, SmsTemporalEvent,
};
pub use vdp::{
    SMS_VDP_MAX_HEIGHT, SMS_VDP_WIDTH, SmsRasterPosition, SmsVdp, SmsVdpEye, StereoPresentation,
};
