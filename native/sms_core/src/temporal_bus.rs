//! Transactional SMS memory/I/O bus adapter for the Scheduler-driven path.
//!
//! Memory and port transactions become visible only at their scheduled
//! completion boundary. Controller/control-port effects, the shared
//! scheduler-driven PSG, and SMS1 VDP port effects are applied there. The VDP
//! scanline clock advances from Scheduler-dispatched Z80 cycles; dot/fetch
//! arbitration and mid-scanline effects remain outside this slice.
//! Unconnected reads return open bus, port writes are retained in a bounded
//! diagnostic trace, and INT acknowledge data comes from an explicit external
//! vector input.

use core::fmt;

use crate::{
    CartridgeImage, ControllerButtons, PsgError, SMS_PSG_CLOCK_DOMAIN_ID, SmsCoreError, SmsIo,
    SmsIoError, SmsMemory, SmsTemporalCoordinator, SmsTemporalCoordinatorError,
    SmsTemporalCoordinatorState, SmsTemporalEvent, SmsVdp, Territory,
};
use sega_audio_runtime::{PsgClockError, ScheduledPsg};
use sega_sn76489::{PsgProfile, Sn76489Core};
use sega_temporal_core::MachineTime;
use sega_video::sms::SmsVdpProfile;
use sega_z80_cpu::{Z80_INT_ACK_AUTOMATIC_WAIT_TSTATES, Z80BusOperation, Z80BusRequest};

pub const SMS_TEMPORAL_BUS_STATE_VERSION: u16 = 10;
const DEFAULT_INTERRUPT_VECTOR: u8 = 0xFF;
const DEFAULT_SMS_PSG_CLOCK_HZ: u64 = 3_579_545;
const DEFAULT_AUDIO_SAMPLE_RATE: u64 = 44_100;
const MAX_PORT_WRITE_TRACE: usize = 64;

fn elapsed_tstates(
    started_at: MachineTime,
    completed_at: MachineTime,
    master_ticks_per_tstate: u64,
) -> Option<u128> {
    let scale = u128::from(master_ticks_per_tstate);
    if scale == 0 {
        return None;
    }
    let elapsed = completed_at.checked_duration_since(started_at)?;
    if elapsed == 0 || elapsed % scale != 0 {
        return None;
    }
    let tstates = elapsed / scale;
    (tstates <= u128::from(u32::MAX)).then_some(tstates)
}

fn valid_bus_deadline(
    started_at: MachineTime,
    deadline: MachineTime,
    master_ticks_per_tstate: u64,
    minimum_tstates: u16,
) -> bool {
    elapsed_tstates(started_at, deadline, master_ticks_per_tstate)
        .is_some_and(|total| total >= u128::from(minimum_tstates))
}

fn valid_internal_deadline(
    started_at: MachineTime,
    deadline: MachineTime,
    master_ticks_per_tstate: u64,
) -> bool {
    elapsed_tstates(started_at, deadline, master_ticks_per_tstate).is_some()
}

/// Result latched at the completion boundary of one bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsBusCompletion {
    pub request_id: u64,
    pub data: Option<u8>,
    pub completed_at: MachineTime,
}

/// Completion of a CPU-internal interval with no memory-bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsInternalCycleCompletion {
    pub continuation_id: u64,
    pub completed_at: MachineTime,
}

/// One dispatched event owned by the SMS temporal bus/coordinator boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemporalDispatch {
    BusCompletion(SmsBusCompletion),
    InternalCycleComplete(SmsInternalCycleCompletion),
    M1TStateComplete {
        continuation_id: u64,
        completed_at: MachineTime,
    },
}

/// Observable port write committed at a scheduled bus-completion boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmsTemporalPortWrite {
    pub port: u16,
    pub value: u8,
    pub completed_at: MachineTime,
}

/// Complete in-memory continuation of the transactional bus adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SmsTemporalBusState {
    pub format_version: u16,
    pub interrupt_vector: u8,
    pub memory: SmsMemory,
    pub io: SmsIo,
    pub psg: ScheduledPsg,
    pub vdp: SmsVdp,
    pub port_write_trace: Vec<SmsTemporalPortWrite>,
    pub coordinator: SmsTemporalCoordinatorState,
    pub pending_request: Option<Z80BusRequest>,
    pub request_started_at: Option<MachineTime>,
    pub completion: Option<SmsBusCompletion>,
    pub pending_internal_cycle: Option<u64>,
    pub internal_cycle_started_at: Option<MachineTime>,
    pub internal_completion: Option<SmsInternalCycleCompletion>,
}

/// Errors from the transactional SMS memory-bus adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmsTemporalBusError {
    Core(SmsCoreError),
    Psg(PsgError),
    PsgClock(PsgClockError),
    Coordinator(SmsTemporalCoordinatorError),
    Io(SmsIoError),
    Busy,
    NoPendingRequest,
    WrongRequestId { expected: u64, received: u64 },
    UnsupportedOperation,
    InvalidState,
}

impl fmt::Display for SmsTemporalBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => write!(f, "SMS memory initialization failed: {error}"),
            Self::Psg(error) => write!(f, "SMS temporal PSG initialization failed: {error}"),
            Self::PsgClock(error) => write!(f, "SMS temporal PSG clock advance failed: {error}"),
            Self::Coordinator(error) => write!(f, "SMS temporal coordinator failed: {error}"),
            Self::Io(error) => write!(f, "SMS temporal controller input failed: {error}"),
            Self::Busy => {
                f.write_str("SMS temporal bus already owns an unacknowledged transaction")
            }
            Self::NoPendingRequest => f.write_str("SMS temporal bus has no pending request"),
            Self::WrongRequestId { expected, received } => write!(
                f,
                "SMS temporal bus expected request {expected}, received {received}"
            ),
            Self::UnsupportedOperation => f.write_str(
                "this temporal bus operation is not implemented by the SMS temporal slice",
            ),
            Self::InvalidState => f.write_str("SMS temporal bus snapshot is inconsistent"),
        }
    }
}

impl std::error::Error for SmsTemporalBusError {}

impl From<SmsCoreError> for SmsTemporalBusError {
    fn from(error: SmsCoreError) -> Self {
        Self::Core(error)
    }
}

impl From<PsgClockError> for SmsTemporalBusError {
    fn from(error: PsgClockError) -> Self {
        Self::PsgClock(error)
    }
}

impl From<PsgError> for SmsTemporalBusError {
    fn from(error: PsgError) -> Self {
        Self::Psg(error)
    }
}

impl From<SmsTemporalCoordinatorError> for SmsTemporalBusError {
    fn from(error: SmsTemporalCoordinatorError) -> Self {
        Self::Coordinator(error)
    }
}

impl From<SmsIoError> for SmsTemporalBusError {
    fn from(error: SmsIoError) -> Self {
        Self::Io(error)
    }
}

/// SMS mapper-backed memory and its scheduler-owned bus completion timeline.
#[derive(Debug, Clone)]
pub struct SmsTemporalBus {
    memory: SmsMemory,
    io: SmsIo,
    psg: ScheduledPsg,
    vdp: SmsVdp,
    port_write_trace: Vec<SmsTemporalPortWrite>,
    coordinator: SmsTemporalCoordinator,
    pending_request: Option<Z80BusRequest>,
    request_started_at: Option<MachineTime>,
    completion: Option<SmsBusCompletion>,
    pending_internal_cycle: Option<u64>,
    internal_cycle_started_at: Option<MachineTime>,
    internal_completion: Option<SmsInternalCycleCompletion>,
    interrupt_vector: u8,
}

impl SmsTemporalBus {
    /// Construct a bus from a cartridge and an explicit master-tick/T-state scale.
    pub fn new(
        rom: impl Into<Vec<u8>>,
        master_ticks_per_tstate: u64,
    ) -> Result<Self, SmsTemporalBusError> {
        Self::new_with_psg_config(
            rom,
            master_ticks_per_tstate,
            DEFAULT_SMS_PSG_CLOCK_HZ,
            DEFAULT_AUDIO_SAMPLE_RATE,
        )
    }

    /// Construct a bus with an explicit SMS PSG input clock and host sample rate.
    ///
    /// The PSG input clock must follow the selected SMS CPU/PSG clock profile.
    /// The sample rate controls only output resampling; it never clocks the chip.
    pub fn new_with_psg_config(
        rom: impl Into<Vec<u8>>,
        master_ticks_per_tstate: u64,
        psg_clock_hz: u64,
        sample_rate: u64,
    ) -> Result<Self, SmsTemporalBusError> {
        Self::new_with_hardware_profile(
            rom,
            master_ticks_per_tstate,
            psg_clock_hz,
            sample_rate,
            SmsVdpProfile::sms1_t0_ntsc(),
            Territory::Export,
        )
    }

    /// Construct the temporal bus with explicit VDP ASIC and territory profiles.
    pub fn new_with_hardware_profile(
        rom: impl Into<Vec<u8>>,
        master_ticks_per_tstate: u64,
        psg_clock_hz: u64,
        sample_rate: u64,
        vdp_profile: SmsVdpProfile,
        territory: Territory,
    ) -> Result<Self, SmsTemporalBusError> {
        let cartridge = CartridgeImage::new(rom)?;
        let memory = SmsMemory::new(cartridge)?;
        let coordinator = SmsTemporalCoordinator::new(master_ticks_per_tstate)?;
        let psg = ScheduledPsg::new(
            SMS_PSG_CLOCK_DOMAIN_ID,
            PsgProfile::Sms,
            psg_clock_hz,
            sample_rate,
        )?;
        Ok(Self {
            memory,
            io: SmsIo::new(territory),
            psg,
            vdp: SmsVdp::new_with_profile(vdp_profile),
            port_write_trace: Vec::new(),
            coordinator,
            pending_request: None,
            request_started_at: None,
            completion: None,
            pending_internal_cycle: None,
            internal_cycle_started_at: None,
            internal_completion: None,
            interrupt_vector: DEFAULT_INTERRUPT_VECTOR,
        })
    }

    /// Begin one memory or port request and schedule its completion deadline.
    ///
    /// WAIT is supplied by a deterministic fixture; this API does not sample
    /// or claim a physical SMS WAIT signal.
    pub fn begin_request(
        &mut self,
        request: Z80BusRequest,
        fixture_wait_tstates: u32,
    ) -> Result<MachineTime, SmsTemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
            || self.internal_completion.is_some()
        {
            return Err(SmsTemporalBusError::Busy);
        }
        if !matches!(
            request.operation,
            Z80BusOperation::OpcodeFetch
                | Z80BusOperation::MemoryRead
                | Z80BusOperation::MemoryWrite { .. }
                | Z80BusOperation::PortRead
                | Z80BusOperation::PortWrite { .. }
                | Z80BusOperation::InterruptAcknowledge
        ) {
            return Err(SmsTemporalBusError::UnsupportedOperation);
        }
        let started_at = self.coordinator.now();
        let automatic_waits = if request.operation == Z80BusOperation::InterruptAcknowledge {
            u32::from(Z80_INT_ACK_AUTOMATIC_WAIT_TSTATES)
        } else {
            0
        };
        let total_wait_tstates = fixture_wait_tstates
            .checked_add(automatic_waits)
            .ok_or(SmsTemporalBusError::InvalidState)?;
        let deadline = self.coordinator.schedule_bus_completion(
            request.id,
            request.base_tstates,
            total_wait_tstates,
        )?;
        self.pending_request = Some(request);
        self.request_started_at = Some(started_at);
        Ok(deadline)
    }

    /// Schedule one internal CPU interval without creating a fake bus access.
    pub fn schedule_internal_cycle(
        &mut self,
        continuation_id: u64,
        tstates: u32,
    ) -> Result<MachineTime, SmsTemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
        {
            return Err(SmsTemporalBusError::Busy);
        }
        let started_at = self.coordinator.now();
        let deadline = self
            .coordinator
            .schedule_internal_cycle(continuation_id, tstates)?;
        self.pending_internal_cycle = Some(continuation_id);
        self.internal_cycle_started_at = Some(started_at);
        Ok(deadline)
    }

    /// Schedule one explicit M1 T-state on the same coordinator as memory cycles.
    pub fn schedule_m1_tstate(
        &mut self,
        continuation_id: u64,
    ) -> Result<MachineTime, SmsTemporalBusError> {
        if self.pending_request.is_some()
            || self.completion.is_some()
            || self.pending_internal_cycle.is_some()
            || self.internal_completion.is_some()
        {
            return Err(SmsTemporalBusError::Busy);
        }
        Ok(self.coordinator.schedule_m1_tstate(continuation_id)?)
    }

    /// Dispatch the next scheduled boundary and apply its bus side effect once.
    ///
    /// A read samples memory at this completion boundary. A write mutates memory
    /// at this completion boundary. The response is latched until acknowledged.
    pub fn dispatch_next(&mut self) -> Result<Option<SmsTemporalDispatch>, SmsTemporalBusError> {
        // Dispatched boundaries remain latched until the CPU accepts them.
        if let Some(completion) = self.completion {
            return Ok(Some(SmsTemporalDispatch::BusCompletion(completion)));
        }
        if let Some(completion) = self.internal_completion {
            return Ok(Some(SmsTemporalDispatch::InternalCycleComplete(completion)));
        }
        let Some(dispatch) = self.coordinator.dispatch_next()? else {
            return Ok(None);
        };
        let psg_advance = dispatch
            .clock_advances
            .iter()
            .find(|advance| advance.domain == SMS_PSG_CLOCK_DOMAIN_ID)
            .ok_or(SmsTemporalBusError::InvalidState)?;
        self.psg.apply_clock_advance(psg_advance)?;
        let z80_advance = dispatch
            .clock_advances
            .iter()
            .find(|advance| advance.domain == crate::SMS_VDP_CLOCK_DOMAIN_ID)
            .ok_or(SmsTemporalBusError::InvalidState)?;
        let vdp_tstates = u64::try_from(z80_advance.elapsed_cycles)
            .map_err(|_| SmsTemporalBusError::InvalidState)?;
        self.vdp.advance(vdp_tstates);
        match dispatch.event.payload {
            SmsTemporalEvent::BusCompletion { request_id } => {
                let request = self
                    .pending_request
                    .ok_or(SmsTemporalBusError::NoPendingRequest)?;
                if request.id != request_id {
                    return Err(SmsTemporalBusError::WrongRequestId {
                        expected: request.id,
                        received: request_id,
                    });
                }
                if self.completion.is_some() {
                    return Err(SmsTemporalBusError::Busy);
                }

                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(self.memory.read(request.address))
                    }
                    Z80BusOperation::InterruptAcknowledge => Some(self.interrupt_vector),
                    Z80BusOperation::MemoryWrite { value } => {
                        self.memory.write(request.address, value);
                        None
                    }
                    Z80BusOperation::PortRead => Some(self.read_port(request.address)),
                    Z80BusOperation::PortWrite { value } => {
                        self.write_port(request.address, value, dispatch.event.timestamp);
                        None
                    }
                };
                let completion = SmsBusCompletion {
                    request_id,
                    data,
                    completed_at: dispatch.event.timestamp,
                };
                self.completion = Some(completion);
                Ok(Some(SmsTemporalDispatch::BusCompletion(completion)))
            }
            SmsTemporalEvent::InternalCycleComplete { continuation_id } => {
                if self.pending_internal_cycle != Some(continuation_id) {
                    return Err(SmsTemporalBusError::InvalidState);
                }
                let completion = SmsInternalCycleCompletion {
                    continuation_id,
                    completed_at: dispatch.event.timestamp,
                };
                self.internal_completion = Some(completion);
                Ok(Some(SmsTemporalDispatch::InternalCycleComplete(completion)))
            }
            SmsTemporalEvent::M1TStateComplete { continuation_id } => {
                if self.pending_request.is_some()
                    || self.pending_internal_cycle.is_some()
                    || self.completion.is_some()
                    || self.internal_completion.is_some()
                {
                    return Err(SmsTemporalBusError::InvalidState);
                }
                Ok(Some(SmsTemporalDispatch::M1TStateComplete {
                    continuation_id,
                    completed_at: dispatch.event.timestamp,
                }))
            }
        }
    }

    /// Consume the latched response exactly once after the CPU accepts it.
    pub fn acknowledge_completion(
        &mut self,
        request_id: u64,
    ) -> Result<SmsBusCompletion, SmsTemporalBusError> {
        let completion = self
            .completion
            .ok_or(SmsTemporalBusError::NoPendingRequest)?;
        if completion.request_id != request_id {
            return Err(SmsTemporalBusError::WrongRequestId {
                expected: completion.request_id,
                received: request_id,
            });
        }
        self.completion = None;
        self.pending_request = None;
        self.request_started_at = None;
        Ok(completion)
    }

    /// Acknowledge an internal interval only after the CPU continuation commits.
    pub fn acknowledge_internal_completion(
        &mut self,
        continuation_id: u64,
    ) -> Result<SmsInternalCycleCompletion, SmsTemporalBusError> {
        let completion = self
            .internal_completion
            .ok_or(SmsTemporalBusError::NoPendingRequest)?;
        if completion.continuation_id != continuation_id
            || self.pending_internal_cycle != Some(continuation_id)
        {
            return Err(SmsTemporalBusError::InvalidState);
        }
        self.internal_completion = None;
        self.pending_internal_cycle = None;
        self.internal_cycle_started_at = None;
        Ok(completion)
    }

    /// Return the currently outstanding CPU request, if any.
    pub const fn pending_request(&self) -> Option<Z80BusRequest> {
        self.pending_request
    }

    /// Timestamp at which the outstanding bus request was scheduled.
    pub const fn request_started_at(&self) -> Option<MachineTime> {
        self.request_started_at
    }

    /// Timestamp at which the outstanding internal interval was scheduled.
    pub const fn internal_cycle_started_at(&self) -> Option<MachineTime> {
        self.internal_cycle_started_at
    }

    /// Return the latched bus completion, if dispatch has reached its deadline.
    pub const fn completion(&self) -> Option<SmsBusCompletion> {
        self.completion
    }

    /// Return the current machine timestamp.
    pub const fn now(&self) -> MachineTime {
        self.coordinator.now()
    }

    pub(crate) fn coordinator_mut(&mut self) -> &mut SmsTemporalCoordinator {
        &mut self.coordinator
    }

    #[cfg(test)]
    pub(crate) fn write_memory_for_test(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
    }

    pub fn io(&self) -> &SmsIo {
        &self.io
    }

    pub fn io_mut(&mut self) -> &mut SmsIo {
        &mut self.io
    }

    /// Update controller levels without advancing scheduler time.
    pub fn set_controller(
        &mut self,
        player: u8,
        buttons: ControllerButtons,
    ) -> Result<(), SmsTemporalBusError> {
        self.io.set_controller(player, buttons)?;
        Ok(())
    }

    /// Set the byte driven by the external interrupting device during acknowledge.
    /// This changes the input level only; it does not advance scheduler time.
    pub fn set_interrupt_vector(&mut self, vector: u8) {
        self.interrupt_vector = vector;
    }

    pub fn port_write_trace(&self) -> &[SmsTemporalPortWrite] {
        &self.port_write_trace
    }

    /// Inspect the integrated SMS-profile PSG without advancing machine time.
    pub fn psg(&self) -> &Sn76489Core {
        self.psg.core()
    }

    /// Consume samples already generated by scheduler-driven PSG clocks.
    pub fn drain_audio_samples(&mut self, max_samples: Option<usize>) -> Vec<f64> {
        self.psg.core_mut().drain_samples(max_samples)
    }

    fn read_port(&mut self, port: u16) -> u8 {
        match (port as u8) & 0xC1 {
            0x40 => self.vdp.read_v_counter(),
            0x41 => self.vdp.read_h_counter(),
            0x80 | 0x81 => self.vdp.read_port(port),
            _ => self.io.read_port(port),
        }
    }

    fn write_port(&mut self, port: u16, value: u8, completed_at: MachineTime) {
        let port_byte = port as u8;
        match port_byte & 0xC1 {
            0x80 | 0x81 => self.vdp.write_port(port, value),
            _ if (port_byte & 0xC0) == 0x40 => self.psg.write_at_dispatch(value),
            _ if port_byte <= 0x3F && port_byte & 1 == 0 => {
                self.memory.write_control_port(value);
            }
            _ if port_byte <= 0x3F && port_byte & 1 == 1 => {
                // The scheduler has advanced the VDP to this bus-completion
                // boundary before the I/O side effect is applied. A TH rising
                // edge latches H-counter only when the resulting pin is input.
                let previous_control = self.io.io_control();
                self.io.write_port(port, value);
                let io_control = self.io.io_control();
                let th_levels = self.io.read_port(0xDD);

                // Match the SMS I/O-control condition: the TH pin must have
                // been driven low (direction + output level both zero), then
                // become an input whose external level is high. Do not infer
                // the previous electrical level from port readback: Japanese
                // SMS intentionally hides driven TH output levels on $DD.
                let th1_rising = io_control & 0x02 != 0
                    && previous_control & 0x22 == 0
                    && th_levels & 0x40 != 0;
                let th2_rising = io_control & 0x08 != 0
                    && previous_control & 0x88 == 0
                    && th_levels & 0x80 != 0;
                if th1_rising || th2_rising {
                    self.vdp.latch_h_counter();
                }
            }
            _ => self.io.write_port(port, value),
        }
        if self.port_write_trace.len() == MAX_PORT_WRITE_TRACE {
            self.port_write_trace.remove(0);
        }
        self.port_write_trace.push(SmsTemporalPortWrite {
            port,
            value,
            completed_at,
        });
    }

    /// Read-only access to the mapper-backed memory image.
    pub const fn memory(&self) -> &SmsMemory {
        &self.memory
    }

    /// Read-only access to the SMS1 VDP connected to the temporal bus.
    pub const fn vdp(&self) -> &SmsVdp {
        &self.vdp
    }

    /// Export memory, scheduler, and in-flight response state.
    pub fn export_state(&self) -> SmsTemporalBusState {
        SmsTemporalBusState {
            format_version: SMS_TEMPORAL_BUS_STATE_VERSION,
            interrupt_vector: self.interrupt_vector,
            memory: self.memory.clone(),
            io: self.io.clone(),
            psg: self.psg.clone(),
            vdp: self.vdp.clone(),
            port_write_trace: self.port_write_trace.clone(),
            coordinator: self.coordinator.export_state(),
            pending_request: self.pending_request,
            request_started_at: self.request_started_at,
            completion: self.completion,
            pending_internal_cycle: self.pending_internal_cycle,
            internal_cycle_started_at: self.internal_cycle_started_at,
            internal_completion: self.internal_completion,
        }
    }

    /// Restore only when the scheduler event and bus transaction agree.
    pub fn restore_state(state: SmsTemporalBusState) -> Result<Self, SmsTemporalBusError> {
        if state.format_version != SMS_TEMPORAL_BUS_STATE_VERSION {
            return Err(SmsTemporalBusError::InvalidState);
        }
        if state.psg.clock_domain() != SMS_PSG_CLOCK_DOMAIN_ID
            || state.psg.core().profile() != PsgProfile::Sms
            || state.psg.core().clock_hz() == 0
            || state.psg.core().sample_rate() == 0
        {
            return Err(SmsTemporalBusError::InvalidState);
        }
        let coordinator = SmsTemporalCoordinator::restore_state(state.coordinator.clone())?;
        let pending_events = &state.coordinator.scheduler.event_queue.pending;
        let scale = state.coordinator.master_ticks_per_tstate;
        let now = coordinator.now();
        if state.port_write_trace.len() > MAX_PORT_WRITE_TRACE
            || state
                .port_write_trace
                .iter()
                .any(|write| write.completed_at > now)
            || state
                .port_write_trace
                .windows(2)
                .any(|pair| pair[0].completed_at > pair[1].completed_at)
        {
            return Err(SmsTemporalBusError::InvalidState);
        }
        let last_dispatch = state.coordinator.scheduler.last_dispatch_timestamp;
        match (
            state.pending_request,
            state.request_started_at,
            state.completion,
            state.pending_internal_cycle,
            state.internal_cycle_started_at,
            state.internal_completion,
        ) {
            (None, None, None, None, None, None)
                if pending_events.is_empty()
                    || (pending_events.len() == 1
                        && matches!(
                            pending_events[0].payload,
                            SmsTemporalEvent::M1TStateComplete { .. }
                        )) => {}
            (Some(request), Some(started_at), None, None, None, None)
                if pending_events.len() == 1
                    && pending_events[0].payload
                        == (SmsTemporalEvent::BusCompletion {
                            request_id: request.id,
                        })
                    && started_at <= now
                    && pending_events[0].timestamp >= now
                    && valid_bus_deadline(
                        started_at,
                        pending_events[0].timestamp,
                        scale,
                        u16::from(request.base_tstates)
                            + if request.operation == Z80BusOperation::InterruptAcknowledge {
                                u16::from(Z80_INT_ACK_AUTOMATIC_WAIT_TSTATES)
                            } else {
                                0
                            },
                    ) => {}
            (Some(request), Some(started_at), Some(completion), None, None, None)
                if request.id == completion.request_id
                    && pending_events.is_empty()
                    && completion.completed_at == now
                    && last_dispatch == Some(completion.completed_at)
                    && valid_bus_deadline(
                        started_at,
                        completion.completed_at,
                        scale,
                        u16::from(request.base_tstates)
                            + if request.operation == Z80BusOperation::InterruptAcknowledge {
                                u16::from(Z80_INT_ACK_AUTOMATIC_WAIT_TSTATES)
                            } else {
                                0
                            },
                    ) => {}
            (None, None, None, Some(continuation_id), Some(started_at), None)
                if pending_events.len() == 1
                    && pending_events[0].payload
                        == (SmsTemporalEvent::InternalCycleComplete { continuation_id })
                    && started_at <= now
                    && pending_events[0].timestamp >= now
                    && valid_internal_deadline(started_at, pending_events[0].timestamp, scale) => {}
            (None, None, None, Some(continuation_id), Some(started_at), Some(completion))
                if continuation_id == completion.continuation_id
                    && pending_events.is_empty()
                    && completion.completed_at == now
                    && last_dispatch == Some(completion.completed_at)
                    && valid_internal_deadline(started_at, completion.completed_at, scale) => {}
            _ => return Err(SmsTemporalBusError::InvalidState),
        }
        Ok(Self {
            memory: state.memory,
            io: state.io,
            psg: state.psg,
            vdp: state.vdp,
            port_write_trace: state.port_write_trace,
            coordinator,
            pending_request: state.pending_request,
            request_started_at: state.request_started_at,
            completion: state.completion,
            pending_internal_cycle: state.pending_internal_cycle,
            internal_cycle_started_at: state.internal_cycle_started_at,
            internal_completion: state.internal_completion,
            interrupt_vector: state.interrupt_vector,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_SMS_PSG_CLOCK_HZ, PsgProfile, SmsTemporalBus, SmsTemporalBusError,
        SmsTemporalDispatch,
    };
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

    fn request(id: u64, operation: Z80BusOperation, address: u16, tstates: u8) -> Z80BusRequest {
        Z80BusRequest {
            id,
            operation,
            address,
            base_tstates: tstates,
        }
    }

    fn patterned_rom(bank_count: usize) -> Vec<u8> {
        let mut rom = vec![0; bank_count * 0x4000];
        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start..start + 0x4000].fill(bank as u8);
        }
        rom
    }

    #[test]
    fn sega_mapper_writes_take_effect_at_scheduler_deadline_and_reads_see_selected_bank() {
        let mut bus = SmsTemporalBus::new(patterned_rom(4), 3).unwrap();
        let cases = [
            (10, 0xFFFD, 0x0400, 0, 0xDFFD),
            (11, 0xFFFE, 0x4000, 1, 0xDFFE),
            (12, 0xFFFF, 0x8000, 2, 0xDFFF),
        ];

        for (request_id, register, banked_address, slot, ram_mirror) in cases {
            let before = bus.memory().selected_banks();
            assert_eq!(bus.memory().read(banked_address), before[slot]);

            let write_request = request(
                request_id,
                Z80BusOperation::MemoryWrite { value: 3 },
                register,
                3,
            );
            let started_at = bus.now();
            let deadline = bus.begin_request(write_request, 2).unwrap();
            assert_eq!(
                deadline.checked_duration_since(started_at),
                Some(15),
                "three nominal T-states plus two fixture WAIT T-states at three master ticks each"
            );

            // Snapshot/restore before dispatch must not expose the mapper write.
            let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
            assert_eq!(restored.memory().selected_banks(), before);
            assert_eq!(restored.memory().read(banked_address), before[slot]);
            assert_eq!(
                restored.memory().read(ram_mirror),
                0,
                "the mirrored work-RAM byte must remain unchanged until the mapper write deadline"
            );

            let Some(SmsTemporalDispatch::BusCompletion(completion)) =
                restored.dispatch_next().unwrap()
            else {
                panic!("expected mapper-register write completion");
            };
            assert_eq!(completion.request_id, request_id);
            assert_eq!(completion.completed_at, deadline);
            assert_eq!(restored.memory().selected_banks()[slot], 3);
            assert_eq!(
                restored.memory().mapper_register_write_count_for_test(),
                request_id - 9,
                "each Sega mapper register write is applied exactly once"
            );
            assert_eq!(restored.memory().read(ram_mirror), 3);
            assert_eq!(
                restored.memory().read(0x03FF),
                0,
                "the Sega mapper's first 1 KiB remains fixed to bank zero"
            );
            assert_eq!(restored.memory().read(banked_address), 3);

            restored.acknowledge_completion(request_id).unwrap();
            let read_request = request(
                request_id + 100,
                Z80BusOperation::MemoryRead,
                banked_address,
                3,
            );
            restored.begin_request(read_request, 0).unwrap();
            let Some(SmsTemporalDispatch::BusCompletion(read_completion)) =
                restored.dispatch_next().unwrap()
            else {
                panic!("expected selected-bank read completion");
            };
            assert_eq!(read_completion.data, Some(3));
            restored.acknowledge_completion(read_request.id).unwrap();

            // Continue the next register case from the restored timeline.
            bus = restored;
        }
    }

    #[test]
    fn mapper_write_completion_remains_latched_across_restore_without_reapplying_effect() {
        let mut bus = SmsTemporalBus::new(patterned_rom(4), 3).unwrap();
        let request = request(20, Z80BusOperation::MemoryWrite { value: 3 }, 0xFFFD, 3);
        bus.begin_request(request, 1).unwrap();
        let first = bus.dispatch_next().unwrap().unwrap();
        let SmsTemporalDispatch::BusCompletion(completion) = first else {
            panic!("expected mapper-register write completion");
        };
        assert_eq!(bus.memory().selected_banks(), [3, 1, 2]);
        assert_eq!(bus.memory().read(0x0400), 3);
        assert_eq!(bus.memory().mapper_register_write_count_for_test(), 1);

        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        let latched_state = restored.export_state();
        assert_eq!(restored.dispatch_next().unwrap(), Some(first));
        assert_eq!(
            restored.export_state(),
            latched_state,
            "a latched completion must not dispatch the scheduler event or mutate memory twice"
        );
        assert_eq!(restored.memory().selected_banks(), [3, 1, 2]);
        assert_eq!(restored.memory().read(0x0400), 3);
        assert_eq!(restored.memory().mapper_register_write_count_for_test(), 1);
        assert_eq!(restored.acknowledge_completion(20).unwrap(), completion);
        assert_eq!(restored.dispatch_next().unwrap(), None);
        assert_eq!(restored.memory().selected_banks(), [3, 1, 2]);
    }

    #[test]
    fn interrupt_acknowledge_returns_external_vector_after_two_automatic_wait_states() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        bus.set_interrupt_vector(0xCF);
        let request = request(90, Z80BusOperation::InterruptAcknowledge, 0x1234, 3);
        let started_at = bus.now();
        let deadline = bus.begin_request(request, 1).unwrap();

        assert_eq!(
            deadline.checked_duration_since(started_at),
            Some(18),
            "three base T-states plus two automatic and one external WAIT"
        );

        let restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.export_state(), bus.export_state());
        let Some(SmsTemporalDispatch::BusCompletion(completion)) = bus.dispatch_next().unwrap()
        else {
            panic!("expected interrupt acknowledge completion");
        };
        assert_eq!(completion.data, Some(0xCF));
        assert_eq!(completion.completed_at, deadline);
    }

    #[test]
    fn memory_read_is_sampled_at_dispatch_and_acknowledged_once() {
        let mut bus = SmsTemporalBus::new(vec![0xA5; 0x4000], 3).unwrap();
        let request = request(1, Z80BusOperation::MemoryRead, 0x0100, 3);
        bus.begin_request(request, 2).unwrap();
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        assert_eq!(completion.request_id, 1);
        assert_eq!(completion.data, Some(0xA5));
        assert_eq!(bus.acknowledge_completion(1).unwrap(), completion);
        assert_eq!(
            bus.acknowledge_completion(1),
            Err(SmsTemporalBusError::NoPendingRequest)
        );
    }

    #[test]
    fn memory_write_occurs_only_at_its_scheduled_completion() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(2, Z80BusOperation::MemoryWrite { value: 0x5A }, 0xC000, 3);
        bus.begin_request(request, 0).unwrap();
        assert_eq!(bus.memory().read(0xC000), 0);
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        assert_eq!(completion.data, None);
        assert_eq!(bus.memory().read(0xC000), 0x5A);
        bus.acknowledge_completion(2).unwrap();
        assert_eq!(bus.memory().read(0xC000), 0x5A);
    }

    #[test]
    fn snapshot_restore_preserves_pending_transaction_before_effect() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(3, Z80BusOperation::MemoryWrite { value: 0x77 }, 0xC001, 3);
        bus.begin_request(request, 1).unwrap();
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().read(0xC001), 0);
        let SmsTemporalDispatch::BusCompletion(completion) =
            restored.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        assert_eq!(restored.memory().read(0xC001), 0x77);
        restored
            .acknowledge_completion(completion.request_id)
            .unwrap();
    }

    #[test]
    fn snapshot_restore_preserves_completed_unacknowledged_write() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(4, Z80BusOperation::MemoryWrite { value: 0x99 }, 0xC002, 3);
        bus.begin_request(request, 0).unwrap();
        let SmsTemporalDispatch::BusCompletion(completion) = bus.dispatch_next().unwrap().unwrap()
        else {
            panic!("expected bus completion");
        };
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.memory().read(0xC002), 0x99);
        assert_eq!(restored.acknowledge_completion(4).unwrap(), completion);
        assert_eq!(restored.memory().read(0xC002), 0x99);
    }

    #[test]
    fn restore_rejects_bus_deadlines_shorter_than_nominal_cycle() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(1, Z80BusOperation::OpcodeFetch, 0, 4);
        bus.begin_request(request, 0).unwrap();

        let mut state = bus.export_state();
        state.coordinator.scheduler.event_queue.pending[0].timestamp = MachineTime::from_ticks(3);
        assert_eq!(
            SmsTemporalBus::restore_state(state).err(),
            Some(SmsTemporalBusError::InvalidState)
        );
    }

    #[test]
    fn restore_rejects_bus_deadlines_not_aligned_to_tstate_scale() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let request = request(1, Z80BusOperation::OpcodeFetch, 0, 4);
        bus.begin_request(request, 0).unwrap();

        let mut state = bus.export_state();
        state.coordinator.scheduler.event_queue.pending[0].timestamp = MachineTime::from_ticks(13);
        assert_eq!(
            SmsTemporalBus::restore_state(state).err(),
            Some(SmsTemporalBusError::InvalidState)
        );
    }

    #[test]
    fn internal_cycle_dispatch_has_no_memory_side_effect() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let deadline = bus.schedule_internal_cycle(77, 1).unwrap();
        let Some(SmsTemporalDispatch::InternalCycleComplete(completion)) =
            bus.dispatch_next().unwrap()
        else {
            panic!("expected internal-cycle completion");
        };
        assert_eq!(completion.continuation_id, 77);
        assert_eq!(completion.completed_at, deadline);
        assert_eq!(bus.memory().read(0xC000), 0);
    }

    #[test]
    fn internal_completion_is_latched_until_acknowledged_and_survives_restore() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let deadline = bus.schedule_internal_cycle(77, 1).unwrap();
        let first = bus.dispatch_next().unwrap().unwrap();
        assert_eq!(
            first,
            SmsTemporalDispatch::InternalCycleComplete(super::SmsInternalCycleCompletion {
                continuation_id: 77,
                completed_at: deadline,
            })
        );
        assert_eq!(bus.dispatch_next().unwrap(), Some(first));

        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.dispatch_next().unwrap(), Some(first));
        assert_eq!(
            restored.acknowledge_internal_completion(77).unwrap(),
            match first {
                SmsTemporalDispatch::InternalCycleComplete(completion) => completion,
                _ => unreachable!(),
            }
        );
        assert_eq!(restored.dispatch_next().unwrap(), None);
    }

    #[test]
    fn psg_writes_commit_at_the_bus_deadline_and_restore_with_clock_phase() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        assert_eq!(bus.psg().profile(), PsgProfile::Sms);
        assert_eq!(bus.psg().clock_hz(), DEFAULT_SMS_PSG_CLOCK_HZ);
        assert_eq!(bus.psg().clock_divider_phase(), 0);

        let write = request(70, Z80BusOperation::PortWrite { value: 0x85 }, 0x7E, 4);
        let deadline = bus.begin_request(write, 0).unwrap();
        let mut restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.psg().clock_divider_phase(), 0);
        assert_eq!(restored.psg().latched_register(), 0);

        let Some(SmsTemporalDispatch::BusCompletion(completion)) =
            restored.dispatch_next().unwrap()
        else {
            panic!("expected PSG port-write completion");
        };
        assert_eq!(completion.completed_at, deadline);
        assert_eq!(deadline, MachineTime::from_ticks(12));
        assert_eq!(restored.psg().clock_divider_phase(), 4);
        assert_eq!(restored.psg().latched_register(), 0);
        assert_eq!(restored.psg().tone_periods()[0], 5);
        assert_eq!(restored.port_write_trace().last().unwrap().port, 0x7E);
        restored.acknowledge_completion(70).unwrap();

        let data = request(71, Z80BusOperation::PortWrite { value: 0x12 }, 0x7F, 4);
        restored.begin_request(data, 0).unwrap();
        restored.dispatch_next().unwrap();
        assert_eq!(restored.psg().tone_periods()[0], 0x125);
        assert_eq!(restored.psg().clock_divider_phase(), 8);
        restored.acknowledge_completion(71).unwrap();

        let volume = request(72, Z80BusOperation::PortWrite { value: 0x90 }, 0x40, 4);
        restored.begin_request(volume, 0).unwrap();
        restored.dispatch_next().unwrap();
        assert_eq!(restored.psg().attenuations()[0], 0);
        assert_eq!(restored.psg().clock_divider_phase(), 12);
        restored.acknowledge_completion(72).unwrap();

        let snapshot = restored.export_state();
        let round_trip = SmsTemporalBus::restore_state(snapshot.clone()).unwrap();
        assert_eq!(round_trip.export_state(), snapshot);
        assert_eq!(round_trip.psg().clock_divider_phase(), 12);
    }

    #[test]
    fn psg_output_is_identical_for_continuous_and_chunked_scheduler_advancement() {
        let mut whole = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let mut chunked = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();

        whole.schedule_internal_cycle(1, 100_000).unwrap();
        let Some(SmsTemporalDispatch::InternalCycleComplete(_)) = whole.dispatch_next().unwrap()
        else {
            panic!("expected one whole-interval completion");
        };
        whole.acknowledge_internal_completion(1).unwrap();

        for continuation_id in 1..=10 {
            chunked
                .schedule_internal_cycle(continuation_id, 10_000)
                .unwrap();
            let Some(SmsTemporalDispatch::InternalCycleComplete(_)) =
                chunked.dispatch_next().unwrap()
            else {
                panic!("expected chunked interval completion");
            };
            chunked
                .acknowledge_internal_completion(continuation_id)
                .unwrap();
        }

        assert_eq!(whole.psg(), chunked.psg());
        assert_eq!(
            whole.drain_audio_samples(None),
            chunked.drain_audio_samples(None)
        );
        assert_eq!(whole.psg(), chunked.psg());
        assert_eq!(whole.now(), chunked.now());
    }

    #[test]
    fn psg_clock_advances_with_cpu_clock_domain_for_each_dispatched_boundary() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 5).unwrap();
        let request = request(73, Z80BusOperation::MemoryRead, 0x0000, 7);
        bus.begin_request(request, 2).unwrap();
        bus.dispatch_next().unwrap();
        assert_eq!(
            bus.coordinator.z80_clock_snapshot().unwrap().total_cycles,
            9
        );
        assert_eq!(
            bus.coordinator.psg_clock_snapshot().unwrap().total_cycles,
            9
        );
        assert_eq!(bus.psg().clock_divider_phase(), 9);
    }

    #[test]
    fn port_reads_are_accepted_and_complete_at_the_scheduled_boundary() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let deadline = bus
            .begin_request(request(5, Z80BusOperation::PortRead, 0x00DC, 4), 0)
            .unwrap();
        let Some(SmsTemporalDispatch::BusCompletion(completion)) = bus.dispatch_next().unwrap()
        else {
            panic!("expected port-read completion");
        };
        assert_eq!(completion.request_id, 5);
        assert_eq!(completion.data, Some(0xFF));
        assert_eq!(completion.completed_at, deadline);
        bus.acknowledge_completion(5).unwrap();
    }
}

#[cfg(test)]
mod immediate_port_transaction_tests {
    use super::{SmsTemporalBus, SmsTemporalDispatch};
    use crate::input::ControllerButtons;
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

    fn request(id: u64, operation: Z80BusOperation, address: u16) -> Z80BusRequest {
        Z80BusRequest {
            id,
            operation,
            address,
            base_tstates: 4,
        }
    }

    #[test]
    fn port_reads_writes_and_controller_state_commit_at_scheduled_boundaries() {
        let mut bus = SmsTemporalBus::new(vec![0x00], 3).unwrap();
        bus.io_mut()
            .set_controller(
                1,
                ControllerButtons {
                    up: true,
                    ..ControllerButtons::default()
                },
            )
            .unwrap();

        bus.begin_request(request(0, Z80BusOperation::PortRead, 0x12DC), 0)
            .unwrap();
        let read = bus.dispatch_next().unwrap().unwrap();
        assert_eq!(
            read,
            SmsTemporalDispatch::BusCompletion(super::SmsBusCompletion {
                request_id: 0,
                data: Some(0xFE),
                completed_at: MachineTime::from_ticks(12),
            })
        );
        bus.acknowledge_completion(0).unwrap();

        bus.begin_request(
            request(1, Z80BusOperation::PortWrite { value: 0xD5 }, 0xD53F),
            0,
        )
        .unwrap();
        assert!(matches!(
            bus.dispatch_next().unwrap(),
            Some(SmsTemporalDispatch::BusCompletion(_))
        ));
        bus.acknowledge_completion(1).unwrap();
        assert_eq!(bus.io().io_control(), 0xD5);

        bus.begin_request(
            request(2, Z80BusOperation::PortWrite { value: 0x55 }, 0x553E),
            0,
        )
        .unwrap();
        assert!(matches!(
            bus.dispatch_next().unwrap(),
            Some(SmsTemporalDispatch::BusCompletion(_))
        ));
        bus.acknowledge_completion(2).unwrap();
        assert_eq!(bus.memory().control_port(), 0x55);
        assert_eq!(bus.port_write_trace().len(), 2);
        assert_eq!(bus.port_write_trace()[0].port, 0xD53F);
        assert_eq!(bus.port_write_trace()[0].value, 0xD5);
        assert_eq!(bus.port_write_trace()[1].port, 0x553E);
        assert_eq!(bus.port_write_trace()[1].value, 0x55);

        let restored = SmsTemporalBus::restore_state(bus.export_state()).unwrap();
        assert_eq!(restored.io().io_control(), 0xD5);
        assert_eq!(restored.memory().control_port(), 0x55);
        assert_eq!(restored.port_write_trace(), bus.port_write_trace());
    }
}

#[cfg(test)]
mod temporal_vdp_tests {
    use super::{SmsTemporalBus, SmsTemporalDispatch};
    use sega_temporal_core::MachineTime;
    use sega_z80_cpu::{Z80BusOperation, Z80BusRequest};

    fn transact(
        bus: &mut SmsTemporalBus,
        id: u64,
        operation: Z80BusOperation,
        address: u16,
    ) -> super::SmsBusCompletion {
        let request = Z80BusRequest {
            id,
            operation,
            address,
            base_tstates: 4,
        };
        bus.begin_request(request, 0).unwrap();
        let Some(SmsTemporalDispatch::BusCompletion(completion)) = bus.dispatch_next().unwrap()
        else {
            panic!("expected scheduled port transaction");
        };
        bus.acknowledge_completion(id).unwrap();
        completion
    }

    fn write_vdp_control(bus: &mut SmsTemporalBus, id: u64, value: u8) {
        transact(bus, id, Z80BusOperation::PortWrite { value }, 0x00BF);
    }

    #[test]
    fn vdp_data_write_is_invisible_until_the_scheduled_port_boundary() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        assert_eq!(bus.vdp().vram()[0], 0);

        // Select VRAM address zero for writing through the two-byte control word.
        write_vdp_control(&mut bus, 1, 0x00);
        write_vdp_control(&mut bus, 2, 0x40);

        let request = Z80BusRequest {
            id: 3,
            operation: Z80BusOperation::PortWrite { value: 0xA5 },
            address: 0x00BE,
            base_tstates: 4,
        };
        bus.begin_request(request, 0).unwrap();
        assert_eq!(bus.vdp().vram()[0], 0);

        let Some(SmsTemporalDispatch::BusCompletion(completion)) = bus.dispatch_next().unwrap()
        else {
            panic!("expected VDP data-port write completion");
        };
        assert_eq!(completion.completed_at, MachineTime::from_ticks(36));
        assert_eq!(bus.vdp().vram()[0], 0xA5);
        bus.acknowledge_completion(3).unwrap();
    }

    #[test]
    fn vdp_read_ahead_is_sampled_at_the_scheduled_data_port_boundary() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        write_vdp_control(&mut bus, 1, 0x00);
        write_vdp_control(&mut bus, 2, 0x40);
        transact(
            &mut bus,
            3,
            Z80BusOperation::PortWrite { value: 0x5A },
            0x00BE,
        );

        // Select address zero for VRAM read; the control-word completion
        // preloads the VDP read-ahead buffer.
        write_vdp_control(&mut bus, 4, 0x00);
        write_vdp_control(&mut bus, 5, 0x00);

        let request = Z80BusRequest {
            id: 6,
            operation: Z80BusOperation::PortRead,
            address: 0x00BE,
            base_tstates: 4,
        };
        bus.begin_request(request, 0).unwrap();
        let Some(SmsTemporalDispatch::BusCompletion(completion)) = bus.dispatch_next().unwrap()
        else {
            panic!("expected VDP data-port read completion");
        };
        assert_eq!(completion.data, Some(0x5A));
        bus.acknowledge_completion(6).unwrap();
    }

    #[test]
    fn vdp_counters_are_read_through_the_sms_mirrored_ports() {
        let mut bus = SmsTemporalBus::new(vec![0; 0x4000], 3).unwrap();
        let v_counter = transact(&mut bus, 1, Z80BusOperation::PortRead, 0x127E);
        assert_eq!(v_counter.data, Some(0));

        // H-counter is latched and remains at its reset value until TH rises.
        let h_counter = transact(&mut bus, 2, Z80BusOperation::PortRead, 0x127F);
        assert_eq!(h_counter.data, Some(0));

        let h_counter_after_more_cycles = transact(&mut bus, 3, Z80BusOperation::PortRead, 0x007F);
        assert_eq!(h_counter_after_more_cycles.data, Some(0));
    }

    #[test]
    fn japanese_output_high_to_input_high_does_not_fake_a_th_rising_edge() {
        fn japanese_bus() -> SmsTemporalBus {
            SmsTemporalBus::new_with_hardware_profile(
                vec![0; 0x4000],
                3,
                super::DEFAULT_SMS_PSG_CLOCK_HZ,
                super::DEFAULT_AUDIO_SAMPLE_RATE,
                SmsVdpProfile::sms1_t0_ntsc(),
                super::Territory::Japan,
            )
            .unwrap()
        }

        let mut th1_bus = japanese_bus();
        // TH1: output-high -> input-high. Japan hides the output-high readback.
        transact(
            &mut th1_bus,
            1,
            Z80BusOperation::PortWrite { value: 0xFD },
            0x003F,
        );
        transact(
            &mut th1_bus,
            2,
            Z80BusOperation::PortWrite { value: 0xFF },
            0x003F,
        );
        assert_eq!(th1_bus.vdp().read_h_counter(), 0);

        let mut th2_bus = japanese_bus();
        // TH2: output-high -> input-high, with TH1 already left as input-high.
        transact(
            &mut th2_bus,
            1,
            Z80BusOperation::PortWrite { value: 0xF7 },
            0x003F,
        );
        transact(
            &mut th2_bus,
            2,
            Z80BusOperation::PortWrite { value: 0xFF },
            0x003F,
        );
        assert_eq!(th2_bus.vdp().read_h_counter(), 0);
    }

    #[test]
    fn vdp_raster_progress_is_chunk_invariant_and_survives_bus_restore() {
        const FRAME_TSTATES: u32 = 228 * 262;
        let mut whole = SmsTemporalBus::new(vec![0; 0x4000], 1).unwrap();
        let mut chunked = SmsTemporalBus::new(vec![0; 0x4000], 1).unwrap();

        whole.schedule_internal_cycle(1, FRAME_TSTATES).unwrap();
        assert!(matches!(
            whole.dispatch_next().unwrap(),
            Some(SmsTemporalDispatch::InternalCycleComplete(_))
        ));
        whole.acknowledge_internal_completion(1).unwrap();

        for (id, tstates) in [(1, FRAME_TSTATES / 2), (2, FRAME_TSTATES / 2)] {
            chunked.schedule_internal_cycle(id, tstates).unwrap();
            assert!(matches!(
                chunked.dispatch_next().unwrap(),
                Some(SmsTemporalDispatch::InternalCycleComplete(_))
            ));
            chunked.acknowledge_internal_completion(id).unwrap();
        }

        assert_eq!(whole.vdp().frame_number(), 1);
        assert_eq!(whole.vdp(), chunked.vdp());

        let snapshot = whole.export_state();
        let restored = SmsTemporalBus::restore_state(snapshot.clone()).unwrap();
        assert_eq!(restored.export_state(), snapshot);
        assert_eq!(restored.vdp(), whole.vdp());
    }
}
