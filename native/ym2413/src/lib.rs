//! YM2413 register bus, slot clock, serial EG, PG, and operator-synthesis components.
//!
//! The 18-slot EG timer/operator pipeline is integrated into `Ym2413Core` and
//! advances at the existing four-input-clock slot boundary. PG/FM synthesis
//! and PCM output remain separate candidate components.

use core::convert::Infallible;

pub mod am_lfo;
pub mod bus_timing;
pub mod channel;
pub mod envelope;
pub mod envelope_operator;
pub mod envelope_sequencer;
pub mod envelope_timer;
pub mod instrument;
pub mod key_scale;
pub mod operator;
pub mod output;
pub mod output_pipeline;
pub mod pg_serial;
pub mod phase;
pub mod pm_lfo;
pub mod rhythm;
pub mod rhythm_channel;
pub mod rhythm_dispatch;
pub mod rhythm_output;
pub mod rhythm_phase;
pub mod serial_operator;
pub mod slot;
mod slot_service;
pub mod synthesis;
pub mod synthesis_tables;
pub mod timed_bus;
pub use am_lfo::{
    AmLfo, AmLfoError, AmLfoState, YM2413_AM_LFO_LEVELS, YM2413_AM_LFO_PERIOD_SAMPLES,
    YM2413_AM_LFO_SAMPLES_PER_STEP, YM2413_AM_LFO_STATE_VERSION, YM2413_AM_LFO_TABLE_LENGTH,
    YM2413_SERIAL_AM_LFO_STATE_VERSION, Ym2413SerialAmLfo, Ym2413SerialAmLfoError,
    Ym2413SerialAmLfoState,
};
pub use bus_timing::{
    YM2413_ADDRESS_WRITE_WAIT_CLOCKS, YM2413_BUS_TIMING_STATE_VERSION,
    YM2413_DATA_WRITE_WAIT_CLOCKS, Ym2413BusTiming, Ym2413BusTimingError, Ym2413BusTimingState,
    Ym2413BusWriteKind,
};
pub use channel::{
    ChannelSynthesisConfig, ChannelSynthesisError, OperatorPhaseParameters,
    YM2413_EG_CYCLE_CHANNELS, decode_channel_synthesis, decode_envelope_slot_input_for_cycle,
    envelope_cycle_descriptor, envelope_cycle_is_carrier, envelope_slot_input_for_cycle,
};
pub use envelope::{
    AttackRate15Transition, DampTransitionConfig, DampTransitionCoordination, EnvelopeGenerator,
    EnvelopeGeneratorError, EnvelopeGeneratorState, EnvelopeParameters, EnvelopeStage,
    EnvelopeTickResult, YM2413_EG_LEVELS, YM2413_EG_MAX_LEVEL, YM2413_EG_STATE_VERSION,
};
pub use envelope_operator::{
    EnvelopeControlPhase, EnvelopeOperator, EnvelopeOperatorError, EnvelopeOperatorStage,
    EnvelopeOperatorState, EnvelopeOperatorTick, YM2413_ENVELOPE_OPERATOR_MAX_LEVEL,
    YM2413_ENVELOPE_OPERATOR_STATE_VERSION,
};
pub use envelope_sequencer::{
    EnvelopeRhythmSelect, EnvelopeSequencer, EnvelopeSequencerError, EnvelopeSequencerState,
    EnvelopeSequencerTick, EnvelopeSlotInput, YM2413_ENVELOPE_SEQUENCER_SLOTS,
    YM2413_ENVELOPE_SEQUENCER_STATE_VERSION,
};
pub use instrument::{
    ChannelRegisterState, InstrumentError, InstrumentPatch, OperatorPatch, YM2413_PRESET_NAMES,
    YM2413_PRESET_PATCH_LICENSE, YM2413_PRESET_PATCH_SOURCE, YM2413_PRESET_PATCH_SOURCE_COMMIT,
    YM2413_PRESET_PATCHES_CANDIDATE, YM2413_PRESET_PATCHES_HARDWARE_VERIFIED,
    decode_channel_registers, decode_patch_registers, decode_user_patch, preset_name, preset_patch,
    resolve_instrument, rhythm_mode_enabled,
};
pub use key_scale::{KeyScaleError, YM2413_KSL1_TABLE, key_scale_attenuation};
pub use operator::{
    OperatorPair, OperatorPairError, OperatorPairParameters, OperatorPairSample, OperatorPairState,
    YM2413_OPERATOR_PAIR_STATE_VERSION, exponential_lookup, log_sine_lookup,
};
pub use output::{
    YM2413_OUTPUT_ACTIVE_CLOCKS, YM2413_OUTPUT_NEUTRAL_CLOCKS, YM2413_OUTPUT_SLOT_CLOCKS,
    YM2413_RHYTHM_OUTPUT_SERVICE_STATE_VERSION, Ym2413OutputCycleFrame, Ym2413OutputPin,
    Ym2413OutputPulse, Ym2413OutputRoutingError, Ym2413OutputSlotResult, Ym2413OutputSource,
    Ym2413RhythmOutputFrame, Ym2413RhythmOutputService, Ym2413RhythmOutputServiceError,
    Ym2413RhythmOutputServiceState, melody_output_for_slot, mo_output_slot_for_nuked_cycle,
    rhythm_output_event, rhythm_slot_result, ro_output_slot_for_nuked_cycle,
};
pub use output_pipeline::{
    YM2413_CHANNEL_OUTPUT_MAX, YM2413_CHANNEL_OUTPUT_MIN, YM2413_OUTPUT_PIPELINE_STATE_VERSION,
    Ym2413NukedOutputCode, Ym2413OutputPipeline, Ym2413OutputPipelineError,
    Ym2413OutputPipelineFrame, Ym2413OutputPipelineState,
};
pub use pg_serial::{
    YM2413_PG_SERIAL_PIPELINE_STATE_VERSION, YM2413_PG_SERIAL_STATE_VERSION,
    Ym2413PgParameterLatch, Ym2413PgSerialAdvanceError, Ym2413PgSerialError,
    Ym2413PgSerialPipelineState, Ym2413PgSerialService, Ym2413PgSerialServiceState,
};
pub use phase::{
    PhaseGenerator, PhaseGeneratorError, PhaseGeneratorState, YM2413_ML_TABLE,
    YM2413_PG_STATE_VERSION, YM2413_PHASE_BITS, YM2413_PHASE_MASK, YM2413_PM_POSITIONS,
    YM2413_PM_TABLE,
};
pub use pm_lfo::{
    PhaseModulationLfo, PhaseModulationLfoError, PhaseModulationLfoState,
    YM2413_PM_LFO_COUNTER_BITS, YM2413_PM_LFO_COUNTER_MASK, YM2413_PM_LFO_PERIOD_SAMPLES,
    YM2413_PM_LFO_POSITIONS, YM2413_PM_LFO_SAMPLES_PER_POSITION, YM2413_PM_LFO_STATE_VERSION,
};
pub use rhythm::{
    YM2413_RHYTHM_HIHAT_NOISE_SLOT_INDEX, YM2413_RHYTHM_MODE_BIT, YM2413_RHYTHM_NOISE_LFSR_BITS,
    YM2413_RHYTHM_NOISE_LFSR_MASK, YM2413_RHYTHM_NOISE_LFSR_PERIOD, YM2413_RHYTHM_NOISE_LFSR_TAPS,
    YM2413_RHYTHM_NOISE_SLOTS_PER_ROTATION, YM2413_RHYTHM_NOISE_STATE_VERSION,
    YM2413_RHYTHM_SLOT_NOISE_SEQUENCER_STATE_VERSION, YM2413_RHYTHM_SLOT_NOISE_STATE_VERSION,
    YM2413_RHYTHM_SNARE_NOISE_SLOT_INDEX, YM2413_RHYTHM_TRIGGER_MASK, Ym2413RhythmChannelFrequency,
    Ym2413RhythmConfig, Ym2413RhythmNoiseError, Ym2413RhythmNoiseLfsr, Ym2413RhythmNoiseSlotSample,
    Ym2413RhythmNoiseState, Ym2413RhythmSlotNoiseLfsr, Ym2413RhythmSlotNoiseSequencer,
    Ym2413RhythmSlotNoiseSequencerState, Ym2413RhythmSlotNoiseState, Ym2413RhythmVoiceConfig,
    decode_rhythm_registers,
};
pub use rhythm_channel::{
    RhythmChannelSynthesisError, RhythmOperatorSynthesisConfig, RhythmVoiceSynthesisConfig,
    RhythmVoiceSynthesisError, Ym2413RhythmSynthesisConfig, decode_rhythm_synthesis,
};
pub use rhythm_dispatch::{
    Ym2413SerialSlotDispatch, Ym2413SerialSlotDispatchError, dispatch_serial_slot,
};
pub use rhythm_output::{
    YM2413_DAC_CODE_MAX, YM2413_DAC_CODE_MIN, YM2413_RHYTHM_OUTPUT_DELAY_STATE_VERSION,
    Ym2413DacCode, Ym2413RhythmOutputDelay, Ym2413RhythmOutputDelayState, Ym2413RhythmOutputError,
    Ym2413RhythmOutputEvent, Ym2413RhythmOutputSlotResult,
};
pub use rhythm_phase::{
    RhythmNoisePhaseInputs, RhythmPhaseCandidateError, RhythmPhaseSource,
    YM2413_RHYTHM_PHASE_CANDIDATE_SOURCE, YM2413_TOP_CYMBAL_EQUIVALENT_PHASE_VARIANTS,
    hi_hat_phase_candidate, rhythm_noise_phase_candidate, rhythm_phase_mix_bit_candidate,
    snare_drum_phase_candidate, top_cymbal_phase_candidate,
};
pub use serial_operator::{
    YM2413_SERIAL_OPERATOR_FEEDBACK_SLOTS, YM2413_SERIAL_OPERATOR_SLOTS,
    YM2413_SERIAL_OPERATOR_STATE_VERSION, Ym2413SerialOperator, Ym2413SerialOperatorError,
    Ym2413SerialOperatorPatch1Latches, Ym2413SerialOperatorPatch2Latches,
    Ym2413SerialOperatorSample, Ym2413SerialOperatorState,
};
pub use slot::{
    YM2413_RHYTHM_OUTPUT_MAP, YM2413_SLOT_COUNT, YM2413_SLOT_MAP, Ym2413Operator,
    Ym2413RhythmOutputOccurrence, Ym2413RhythmVoice, Ym2413SlotDescriptor, Ym2413SlotServiceEvent,
    nuked_cycle_selected_slot_descriptor, nuked_cycle_selected_slot_index, nuked_eg_output_index,
    nuked_eg_update_index, nuked_pg_phase_output_index, nuked_pg_phase_slot_descriptor,
    nuked_pg_phase_write_index, nuked_phase_reset_required,
    nuked_phase_reset_required_with_testmode, nuked_slot_service_event,
    nuked_slot_service_event_with_testmode, slot_descriptor,
};
pub use synthesis::{
    YM2413_MELODIC_CHANNELS, YM2413_OPERATORS_PER_CHANNEL, YM2413_SYNTHESIS_STATE_VERSION,
    Ym2413ChannelInitialState, Ym2413ChannelSynthesisState, Ym2413RhythmVoicePhases,
    Ym2413RhythmVoiceSample, Ym2413SynthesisBank, Ym2413SynthesisBankState, Ym2413SynthesisError,
    Ym2413SynthesisInitialState,
};
pub use synthesis_tables::{YM2413_EXP_TABLE, YM2413_LOG_SINE_TABLE};
pub use timed_bus::{
    YM2413_TIMED_BUS_STATE_VERSION, Ym2413TimedBus, Ym2413TimedBusError, Ym2413TimedBusState,
};

use core::fmt;

/// Number of addressable YM2413 register locations.
pub const YM2413_REGISTER_COUNT: usize = 0x40;

/// The YM2413 exposes six address bits on its register interface.
pub const YM2413_ADDRESS_MASK: u8 = 0x3f;

/// Input clocks consumed by one sequential YM2413 operator slot.
pub const YM2413_INPUT_CLOCKS_PER_SLOT: u64 = 4;

/// Number of sequential operator slots in one complete rotation.
pub const YM2413_SLOTS_PER_ROTATION: u64 = 18;

/// Derived input-clock count for one complete 18-slot rotation.
pub const YM2413_INPUT_CLOCKS_PER_ROTATION: u64 =
    YM2413_INPUT_CLOCKS_PER_SLOT * YM2413_SLOTS_PER_ROTATION;

const YM2413_STATE_FORMAT_VERSION: u16 = 2;

/// Errors raised by the YM2413 register, bus, clock, and EG foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413Error {
    /// The accumulated input-clock or rotation counter would overflow.
    ClockOverflow,
    /// The state version is not supported by this implementation.
    StateFormatVersionMismatch { expected: u16, actual: u16 },
    /// A snapshot contains an invalid latch, slot, phase, or clock relation.
    InvalidState,
    /// Register-derived EG inputs could not be decoded.
    EnvelopeInput(ChannelSynthesisError),
    /// The serial EG sequencer rejected its current slot or state.
    EnvelopeSequencer(EnvelopeSequencerError),
}

impl fmt::Display for Ym2413Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClockOverflow => f.write_str("YM2413 input-clock counter overflow"),
            Self::StateFormatVersionMismatch { expected, actual } => write!(
                f,
                "YM2413 state version {actual} is unsupported (expected {expected})"
            ),
            Self::InvalidState => f.write_str("YM2413 register/clock/EG state is inconsistent"),
            Self::EnvelopeInput(error) => {
                write!(f, "YM2413 EG input decode error: {error:?}")
            }
            Self::EnvelopeSequencer(error) => {
                write!(f, "YM2413 EG sequencer error: {error}")
            }
        }
    }
}

impl std::error::Error for Ym2413Error {}

impl From<ChannelSynthesisError> for Ym2413Error {
    fn from(value: ChannelSynthesisError) -> Self {
        Self::EnvelopeInput(value)
    }
}

impl From<EnvelopeSequencerError> for Ym2413Error {
    fn from(value: EnvelopeSequencerError) -> Self {
        Self::EnvelopeSequencer(value)
    }
}

/// Complete snapshot of the YM2413 register bus, serial EG and slot clock.
///
/// The register image is internal device state. It is not a claim that the
/// YM2413 exposes readable registers on its external bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ym2413State {
    pub format_version: u16,
    pub registers: [u8; YM2413_REGISTER_COUNT],
    /// Most recently presented six-bit address-port value.
    pub address_latch: u8,
    /// Address currently consumed by the delayed FM register-write pipeline.
    pub fm_address: u8,
    pub write_data_latch: u8,
    pub data_latch: u8,
    pub write_a: u8,
    pub write_d: u8,
    pub write_a_enable: bool,
    pub write_d_enable: bool,
    pub write_fm_address: bool,
    pub write_fm_data: bool,
    pub write_mode_address: u8,
    pub rm_enable: i8,
    pub rm_select: u8,
    pub envelope: EnvelopeSequencerState,
    pub input_clock_phase: u8,
    pub slot_index: u8,
    pub elapsed_input_clocks: u64,
    pub completed_rotations: u64,
}

/// Deterministic YM2413 register-bus, serial-EG, and slot-clock core.
///
/// The EG sequencer receives one serial update whenever the existing four
/// input clocks per slot complete. This establishes slot-level causality only;
/// it does not assign PG/EG/operator work to unmeasured subphases within those
/// four clocks. The machine adapter remains responsible for supplying input
/// clocks at the correct MachineScheduler deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ym2413Core {
    registers: [u8; YM2413_REGISTER_COUNT],
    address_latch: u8,
    fm_address: u8,
    write_data_latch: u8,
    data_latch: u8,
    write_a: u8,
    write_d: u8,
    write_a_enable: bool,
    write_d_enable: bool,
    write_fm_address: bool,
    write_fm_data: bool,
    write_mode_address: u8,
    rm_enable: i8,
    rm_select: u8,
    envelope: EnvelopeSequencer,
    input_clock_phase: u8,
    slot_index: u8,
    elapsed_input_clocks: u64,
    completed_rotations: u64,
}

impl Default for Ym2413Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Ym2413Core {
    /// Construct a reset YM2413 bus, serial-EG and slot-clock core.
    pub const fn new() -> Self {
        Self {
            registers: [0; YM2413_REGISTER_COUNT],
            address_latch: 0,
            fm_address: 0,
            write_data_latch: 0,
            data_latch: 0,
            write_a: 0,
            write_d: 0,
            write_a_enable: false,
            write_d_enable: false,
            write_fm_address: false,
            write_fm_data: false,
            write_mode_address: 0,
            rm_enable: 0,
            rm_select: 6,
            envelope: EnvelopeSequencer::new(),
            input_clock_phase: 0,
            slot_index: 0,
            elapsed_input_clocks: 0,
            completed_rotations: 0,
        }
    }

    /// Reset the logical register image, delayed bus pipeline, EG and slot phase.
    ///
    /// The machine adapter remains responsible for the documented external reset-pulse timing.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Queue a six-bit address-port write for the delayed internal bus pipeline.
    ///
    /// The public address latch records the presented value immediately; the
    /// internal FM address is committed later by the serial bus pipeline.
    pub fn write_address(&mut self, address: u8) {
        self.address_latch = address & YM2413_ADDRESS_MASK;
        self.write_data_latch = self.address_latch;
        self.write_a |= 1;
    }

    /// Queue a data-port write for the delayed internal bus pipeline.
    ///
    /// The register image changes only when the chip's serial write pipeline
    /// reaches the selected register cycle.
    pub fn write_data(&mut self, value: u8) {
        self.write_data_latch = value;
        self.write_d |= 1;
    }

    /// Return the most recently presented address-port value.
    pub const fn address_latch(&self) -> u8 {
        self.address_latch
    }

    /// Return the next sequential operator slot index (0..17).
    pub const fn slot_index(&self) -> u8 {
        self.slot_index
    }

    /// Describe the next serial operator slot without advancing the clock.
    ///
    /// The returned descriptor follows the Yamaha Table II-3 ordering. It
    /// identifies channel/operator ownership only; it does not infer PG or
    /// operator subphase offsets.
    pub const fn next_slot_descriptor(&self) -> Option<Ym2413SlotDescriptor> {
        slot_descriptor(self.slot_index)
    }

    /// Return input clocks accumulated within the current four-clock slot.
    pub const fn input_clock_phase(&self) -> u8 {
        self.input_clock_phase
    }

    /// Return total input clocks elapsed since reset.
    pub const fn elapsed_input_clocks(&self) -> u64 {
        self.elapsed_input_clocks
    }

    /// Return complete 18-slot rotations elapsed since reset.
    pub const fn completed_rotations(&self) -> u64 {
        self.completed_rotations
    }

    /// Return a copy of the complete serial EG state for diagnostics and tests.
    pub fn envelope_state(&self) -> EnvelopeSequencerState {
        self.envelope.export_state()
    }

    /// Advance the slot clock and serial EG by an exact number of input clocks.
    ///
    /// Each completed four-clock slot services exactly one EG serial cycle.
    /// The delayed address/data and rhythm-mode latches are advanced in the
    /// same order as the serial slot. The operation is transactional: if an
    /// input or EG state is invalid, neither the clock nor the EG is committed.
    ///
    /// Returns the number of complete 18-slot rotations crossed by this call.
    /// The result and complete state are independent of block partitioning.
    pub fn advance_input_clocks(&mut self, clocks: u64) -> Result<u64, Ym2413Error> {
        match self.advance_input_clocks_inner(clocks, |core| {
            core.clock_one_slot()
                .map_err(slot_service::Ym2413SlotServiceAdvanceError::<Infallible>::Core)
        }) {
            Ok(rotations) => Ok(rotations),
            Err(slot_service::Ym2413SlotServiceAdvanceError::Core(error)) => Err(error),
            Err(slot_service::Ym2413SlotServiceAdvanceError::Service(never)) => match never {},
        }
    }

    /// Internal serial-slot advancement used by the integrated FM pipeline.
    ///
    /// The core and service are advanced as one transaction: both are staged
    /// from their current states and committed only after every completed slot
    /// succeeds. A failed callback therefore leaves both caller-owned states
    /// unchanged, making a retry deterministic.
    ///
    /// The service must be a cloneable, in-memory state machine. `Clone` must
    /// produce an independent copy of every mutable field that callbacks can
    /// change; shallowly shared mutation through `Arc<Mutex<_>>`, atomics, or
    /// external handles violates this transaction contract unless mutations are
    /// independently staged/copy-on-write. Callbacks must not perform externally
    /// visible side effects because cloning cannot roll those effects back.
    /// The host still advances the chip in
    /// coarse-grained input-clock blocks; these hooks are not host callbacks.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn advance_input_clocks_with_slot_service<
        S: slot_service::Ym2413SlotService + Clone,
    >(
        &mut self,
        clocks: u64,
        service: &mut S,
    ) -> Result<u64, slot_service::Ym2413SlotServiceAdvanceError<S::Error>> {
        let mut staged_service = service.clone();
        let result = self.advance_input_clocks_inner(clocks, |core| {
            core.clock_one_slot_with_service(&mut staged_service)
        });

        match result {
            Ok(rotations) => {
                *service = staged_service;
                Ok(rotations)
            }
            Err(error) => Err(error),
        }
    }

    /// Advance the YM2413 core and its serial PG service in one transaction.
    ///
    /// The caller supplies a coarse-grained input-clock block. The core invokes
    /// the PG service internally at serial slot boundaries; no host-side
    /// per-slot or per-instruction callback is exposed.
    pub fn advance_input_clocks_with_pg_service(
        &mut self,
        clocks: u64,
        service: &mut pg_serial::Ym2413PgSerialService,
    ) -> Result<u64, pg_serial::Ym2413PgSerialAdvanceError> {
        match self.advance_input_clocks_with_slot_service(clocks, service) {
            Ok(rotations) => Ok(rotations),
            Err(slot_service::Ym2413SlotServiceAdvanceError::Core(error)) => {
                Err(pg_serial::Ym2413PgSerialAdvanceError::Core(error))
            }
            Err(slot_service::Ym2413SlotServiceAdvanceError::Service(error)) => {
                Err(pg_serial::Ym2413PgSerialAdvanceError::Service(error))
            }
        }
    }

    fn advance_input_clocks_inner<E, F>(
        &mut self,
        clocks: u64,
        mut service_slot: F,
    ) -> Result<u64, slot_service::Ym2413SlotServiceAdvanceError<E>>
    where
        F: FnMut(&mut Self) -> Result<(), slot_service::Ym2413SlotServiceAdvanceError<E>>,
    {
        let next_elapsed = self.elapsed_input_clocks.checked_add(clocks).ok_or(
            slot_service::Ym2413SlotServiceAdvanceError::Core(Ym2413Error::ClockOverflow),
        )?;

        let total_slot_clocks = u128::from(self.input_clock_phase) + u128::from(clocks);
        let completed_slots = total_slot_clocks / u128::from(YM2413_INPUT_CLOCKS_PER_SLOT);
        let next_clock_phase =
            u8::try_from(total_slot_clocks % u128::from(YM2413_INPUT_CLOCKS_PER_SLOT)).map_err(
                |_| slot_service::Ym2413SlotServiceAdvanceError::Core(Ym2413Error::InvalidState),
            )?;

        let total_slots = u128::from(self.slot_index) + completed_slots;
        let rotations = u64::try_from(total_slots / u128::from(YM2413_SLOTS_PER_ROTATION))
            .map_err(|_| {
                slot_service::Ym2413SlotServiceAdvanceError::Core(Ym2413Error::ClockOverflow)
            })?;
        let next_completed_rotations = self.completed_rotations.checked_add(rotations).ok_or(
            slot_service::Ym2413SlotServiceAdvanceError::Core(Ym2413Error::ClockOverflow),
        )?;
        let next_slot =
            u8::try_from(total_slots % u128::from(YM2413_SLOTS_PER_ROTATION)).map_err(|_| {
                slot_service::Ym2413SlotServiceAdvanceError::Core(Ym2413Error::InvalidState)
            })?;

        let mut next = self.clone();
        let mut slots_remaining = completed_slots;
        while slots_remaining != 0 {
            service_slot(&mut next)?;
            next.slot_index = (next.slot_index + 1) % YM2413_SLOTS_PER_ROTATION as u8;
            slots_remaining -= 1;
        }

        next.elapsed_input_clocks = next_elapsed;
        next.input_clock_phase = next_clock_phase;
        next.slot_index = next_slot;
        next.completed_rotations = next_completed_rotations;
        *self = next;

        Ok(rotations)
    }

    /// Apply the serial mode/rhythm preamble and decode the EG input for the
    /// current cycle. Both service paths share this exact boundary.
    fn begin_slot(&mut self) -> Result<(u8, EnvelopeSlotInput), Ym2413Error> {
        let cycle = self.slot_index;

        // Nuked's serial mode selector shifts and applies mode writes before
        // patch selection, PG/Operator service and the current EG update.
        self.rm_enable >>= 1;
        self.apply_mode_write();
        self.rm_select = (self.rm_select + 1).min(6);
        if cycle == 11 && (self.rm_enable as u8 & 0x80) != 0 {
            self.rm_select = 0;
        }

        let mut input = decode_envelope_slot_input_for_cycle(&self.registers, cycle)?;
        input.rhythm_select =
            EnvelopeRhythmSelect::from_code(self.rm_select).ok_or(Ym2413Error::InvalidState)?;
        input.rhythm_bits = self.registers[0x0e] & 0x3f;
        input.test_mode = self.registers[0x0f] & 0x0f;
        input.write_data = self.write_data_latch;
        Ok((cycle, input))
    }

    /// Standalone core path: one EG update and the timed register-write stage.
    fn clock_one_slot(&mut self) -> Result<(), Ym2413Error> {
        let (cycle, input) = self.begin_slot()?;
        self.envelope.tick_slot(cycle, input)?;
        self.apply_register_write(cycle);
        Ok(())
    }

    /// Integrated path: service pre-EG and post-EG stages around exactly one
    /// core-owned serial EG update.
    #[cfg_attr(not(test), allow(dead_code))]
    fn clock_one_slot_with_service<S: slot_service::Ym2413SlotService>(
        &mut self,
        service: &mut S,
    ) -> Result<(), slot_service::Ym2413SlotServiceAdvanceError<S::Error>> {
        let (cycle, input) = self
            .begin_slot()
            .map_err(slot_service::Ym2413SlotServiceAdvanceError::Core)?;

        let envelope_state = self.envelope.export_state();
        let event = nuked_slot_service_event_with_testmode(
            cycle,
            self.rm_enable as u8,
            envelope_state.eg_dokon,
            self.registers[0x0f],
        )
        .ok_or(slot_service::Ym2413SlotServiceAdvanceError::Core(
            Ym2413Error::InvalidState,
        ))?;
        let context = slot_service::Ym2413SlotServiceContext {
            event,
            registers: self.registers,
            envelope_input: input,
            envelope_state,
            rm_enable: self.rm_enable as u8,
            rm_select: self.rm_select,
        };

        service
            .before_eg_update(&context)
            .map_err(slot_service::Ym2413SlotServiceAdvanceError::Service)?;

        let tick = self
            .envelope
            .tick_slot(cycle, input)
            .map_err(|error| slot_service::Ym2413SlotServiceAdvanceError::Core(error.into()))?;

        service
            .after_eg_update(&context, tick, self.envelope.export_state())
            .map_err(slot_service::Ym2413SlotServiceAdvanceError::Service)?;

        // The ordinary timed write stage remains last, after both service
        // hooks and the single EG update.
        self.apply_register_write(cycle);
        Ok(())
    }

    /// Apply delayed user-patch, rhythm and test-mode register writes.
    fn apply_mode_write(&mut self) {
        if self.write_mode_address & 0x10 == 0 || !self.write_d_enable {
            return;
        }

        let register = self.write_mode_address & 0x0f;
        match register {
            0x00..=0x07 => {
                self.registers[usize::from(register)] = self.write_data_latch;
            }
            0x0e => {
                let rhythm = self.write_data_latch & 0x3f;
                self.registers[0x0e] = rhythm;
                self.rm_enable = (self.rm_enable & 0x7f) | (((rhythm << 2) & 0x80) as i8);
            }
            0x0f => {
                self.registers[0x0f] = self.write_data_latch & 0x0f;
            }
            _ => {}
        }
    }

    /// Apply cycle-qualified FM register writes and advance the IO shift latches.
    fn apply_register_write(&mut self, cycle: u8) {
        if self.write_a_enable {
            if self.write_data_latch & 0xc0 == 0 {
                self.write_fm_address = true;
                self.fm_address = self.write_data_latch;
            } else {
                self.write_fm_address = false;
            }
        }
        if self.write_fm_address && self.write_d_enable {
            self.data_latch = self.write_data_latch;
        }
        if self.write_fm_data
            && !self.write_a_enable
            && cycle < 16
            && (self.fm_address & 0x0f) == cycle
        {
            let channel = usize::from(cycle % 9);
            match self.fm_address & 0xf0 {
                0x10 | 0x20 | 0x30 => {
                    self.registers[usize::from(self.fm_address & 0xf0) + channel] = self.data_latch;
                }
                _ => {}
            }
        }
        if self.write_a_enable {
            self.write_fm_data = false;
        }
        if self.write_fm_address && self.write_d_enable {
            self.write_fm_data = true;
        }
        if self.write_a_enable {
            self.write_mode_address = if self.write_data_latch & 0xf0 == 0 {
                0x10 | (self.write_data_latch & 0x0f)
            } else {
                0
            };
        }

        self.write_a_enable = self.write_a & 0x03 == 0x01;
        self.write_d_enable = self.write_d & 0x03 == 0x01;
        self.write_a <<= 1;
        self.write_d <<= 1;
    }

    /// Export the complete register bus, serial EG and slot-clock state.
    pub fn export_state(&self) -> Ym2413State {
        Ym2413State {
            format_version: YM2413_STATE_FORMAT_VERSION,
            registers: self.registers,
            address_latch: self.address_latch,
            fm_address: self.fm_address,
            write_data_latch: self.write_data_latch,
            data_latch: self.data_latch,
            write_a: self.write_a,
            write_d: self.write_d,
            write_a_enable: self.write_a_enable,
            write_d_enable: self.write_d_enable,
            write_fm_address: self.write_fm_address,
            write_fm_data: self.write_fm_data,
            write_mode_address: self.write_mode_address,
            rm_enable: self.rm_enable,
            rm_select: self.rm_select,
            envelope: self.envelope.export_state(),
            input_clock_phase: self.input_clock_phase,
            slot_index: self.slot_index,
            elapsed_input_clocks: self.elapsed_input_clocks,
            completed_rotations: self.completed_rotations,
        }
    }

    /// Restore a snapshot after validating the bus, EG, and clock relationships.
    pub fn restore_state(state: Ym2413State) -> Result<Self, Ym2413Error> {
        if state.format_version != YM2413_STATE_FORMAT_VERSION {
            return Err(Ym2413Error::StateFormatVersionMismatch {
                expected: YM2413_STATE_FORMAT_VERSION,
                actual: state.format_version,
            });
        }
        if state.address_latch & !YM2413_ADDRESS_MASK != 0
            || state.fm_address & !YM2413_ADDRESS_MASK != 0
            || u64::from(state.input_clock_phase) >= YM2413_INPUT_CLOCKS_PER_SLOT
            || u64::from(state.slot_index) >= YM2413_SLOTS_PER_ROTATION
            || state.rm_select > 6
            || (state.write_mode_address != 0 && !(0x10..=0x1f).contains(&state.write_mode_address))
        {
            return Err(Ym2413Error::InvalidState);
        }

        let expected_elapsed = u128::from(state.completed_rotations)
            * u128::from(YM2413_INPUT_CLOCKS_PER_ROTATION)
            + u128::from(state.slot_index) * u128::from(YM2413_INPUT_CLOCKS_PER_SLOT)
            + u128::from(state.input_clock_phase);
        if expected_elapsed != u128::from(state.elapsed_input_clocks) {
            return Err(Ym2413Error::InvalidState);
        }

        let envelope = EnvelopeSequencer::restore_state(state.envelope)?;
        if envelope.timer_state().next_slot != state.slot_index {
            return Err(Ym2413Error::InvalidState);
        }

        Ok(Self {
            registers: state.registers,
            address_latch: state.address_latch,
            fm_address: state.fm_address,
            write_data_latch: state.write_data_latch,
            data_latch: state.data_latch,
            write_a: state.write_a,
            write_d: state.write_d,
            write_a_enable: state.write_a_enable,
            write_d_enable: state.write_d_enable,
            write_fm_address: state.write_fm_address,
            write_fm_data: state.write_fm_data,
            write_mode_address: state.write_mode_address,
            rm_enable: state.rm_enable,
            rm_select: state.rm_select,
            envelope,
            input_clock_phase: state.input_clock_phase,
            slot_index: state.slot_index,
            elapsed_input_clocks: state.elapsed_input_clocks,
            completed_rotations: state.completed_rotations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EnvelopeSequencerState, EnvelopeSequencerTick, YM2413_EG_CYCLE_CHANNELS,
        YM2413_INPUT_CLOCKS_PER_ROTATION, YM2413_INPUT_CLOCKS_PER_SLOT, YM2413_SLOT_COUNT,
        YM2413_SLOT_MAP, Ym2413Core, Ym2413Error, Ym2413State, nuked_phase_reset_required,
        slot_service::{
            Ym2413SlotService, Ym2413SlotServiceAdvanceError, Ym2413SlotServiceContext,
        },
    };
    use core::convert::Infallible;

    #[test]
    fn address_latch_masks_to_six_bits() {
        let mut core = Ym2413Core::new();

        core.write_address(0xe7);

        assert_eq!(core.address_latch(), 0x27);
    }

    #[test]
    fn data_write_targets_only_the_latched_register_after_the_serial_bus_pipeline() {
        let mut core = Ym2413Core::new();

        core.write_address(0x21);
        core.advance_input_clocks(2 * YM2413_INPUT_CLOCKS_PER_SLOT)
            .unwrap();
        core.write_data(0x9a);
        core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_ROTATION)
            .unwrap();

        core.write_address(0x32);
        core.advance_input_clocks(2 * YM2413_INPUT_CLOCKS_PER_SLOT)
            .unwrap();
        core.write_data(0x47);
        core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_ROTATION)
            .unwrap();

        let state = core.export_state();
        assert_eq!(state.registers[0x21], 0x9a);
        assert_eq!(state.registers[0x32], 0x47);
        assert_eq!(state.registers[0x20], 0);
    }

    #[test]
    fn serial_eg_advances_only_when_a_four_clock_slot_completes() {
        let mut core = Ym2413Core::new();

        assert_eq!(core.envelope_state().timer.next_slot, 0);
        core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_SLOT - 1)
            .unwrap();
        assert_eq!(core.envelope_state().timer.next_slot, 0);
        core.advance_input_clocks(1).unwrap();
        assert_eq!(core.envelope_state().timer.next_slot, 1);
        assert_eq!(core.slot_index(), 1);
    }

    #[test]
    fn rhythm_mode_eg_cycles_are_driven_by_the_serial_selector_pipeline() {
        let mut core = Ym2413Core::new();
        // Queue $0E = RHY + all five percussion triggers.
        core.write_address(0x0e);
        core.advance_input_clocks(2 * YM2413_INPUT_CLOCKS_PER_SLOT)
            .unwrap();
        core.write_data(0x3f);
        core.advance_input_clocks(20 * YM2413_INPUT_CLOCKS_PER_SLOT)
            .unwrap();

        let state = core.export_state();
        assert_eq!(state.registers[0x0e], 0x3f);
        assert_eq!(state.rm_enable, -1);
        assert_eq!(state.rm_select, 6);
        assert_eq!(state.envelope.timer.next_slot, state.slot_index);
    }

    #[test]
    fn core_rhythm_triggers_follow_the_six_serial_eg_selectors() {
        let cases = [
            (11_u8, 0x10_u8), // Bass Drum, first selector.
            (12, 0x01),       // Hi-Hat.
            (13, 0x04),       // Tom-Tom.
            (14, 0x10),       // Bass Drum, second selector.
            (15, 0x08),       // Snare Drum.
            (16, 0x02),       // Top Cymbal.
        ];

        for (target_cycle, trigger) in cases {
            let mut core = Ym2413Core::new();
            core.advance_input_clocks(8 * YM2413_INPUT_CLOCKS_PER_SLOT)
                .unwrap();
            core.write_address(0x0e);
            core.advance_input_clocks(2 * YM2413_INPUT_CLOCKS_PER_SLOT)
                .unwrap();
            core.write_data(0x20 | trigger);

            let slots_to_target = u64::from(target_cycle - 10 + 1);
            core.advance_input_clocks(slots_to_target * YM2413_INPUT_CLOCKS_PER_SLOT)
                .unwrap();

            let state = core.export_state();
            assert_eq!(
                state.registers[0x0e],
                0x20 | trigger,
                "cycle {target_cycle}"
            );
            assert_eq!(
                state.rm_select,
                target_cycle - 11,
                "rhythm selector at cycle {target_cycle}"
            );
            assert_eq!(
                state.envelope.eg_kon & 1,
                1,
                "rhythm trigger did not reach EG KON at cycle {target_cycle}"
            );
        }
    }

    #[test]
    fn restore_rejects_eg_timer_slot_misaligned_with_core_slot() {
        let core = Ym2413Core::new();
        let mut state = core.export_state();
        state.envelope.timer.next_slot = 1;

        assert_eq!(
            Ym2413Core::restore_state(state),
            Err(Ym2413Error::InvalidState)
        );
    }

    #[test]
    fn one_complete_rotation_requires_seventy_two_input_clocks() {
        let mut core = Ym2413Core::new();

        assert_eq!(
            core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_ROTATION - 1),
            Ok(0)
        );
        assert_eq!(core.completed_rotations(), 0);
        assert_eq!(core.advance_input_clocks(1), Ok(1));
        assert_eq!(core.completed_rotations(), 1);
        assert_eq!(core.slot_index(), 0);
        assert_eq!(core.input_clock_phase(), 0);
    }

    #[test]
    fn clock_advance_is_invariant_under_block_partitioning() {
        let mut continuous = Ym2413Core::new();
        let mut chunked = Ym2413Core::new();

        continuous.advance_input_clocks(5_003).unwrap();
        for clocks in [1, 3, 4, 17, 72, 511, 1_024, 3_371] {
            chunked.advance_input_clocks(clocks).unwrap();
        }

        assert_eq!(continuous.export_state(), chunked.export_state());
    }

    #[test]
    fn snapshot_restore_preserves_registers_and_partial_slot_phase() {
        let mut core = Ym2413Core::new();
        core.write_address(0x18);
        core.write_data(0x73);
        core.advance_input_clocks(77).unwrap();

        let restored = Ym2413Core::restore_state(core.export_state()).unwrap();

        assert_eq!(restored.export_state(), core.export_state());
        assert_eq!(restored.slot_index(), 1);
        assert_eq!(restored.input_clock_phase(), 1);
    }

    #[test]
    fn restore_rejects_inconsistent_clock_state() {
        let mut core = Ym2413Core::new();
        core.advance_input_clocks(73).unwrap();
        let mut state: Ym2413State = core.export_state();
        state.elapsed_input_clocks += 1;

        assert_eq!(
            Ym2413Core::restore_state(state),
            Err(Ym2413Error::InvalidState)
        );
    }

    #[test]
    fn reset_clears_registers_latch_and_clock_phase() {
        let mut core = Ym2413Core::new();
        core.write_address(0x10);
        core.write_data(0xff);
        core.advance_input_clocks(91).unwrap();

        core.reset();

        let state = core.export_state();
        assert_eq!(state.registers, [0; 0x40]);
        assert_eq!(state.address_latch, 0);
        assert_eq!(state.input_clock_phase, 0);
        assert_eq!(state.slot_index, 0);
        assert_eq!(state.elapsed_input_clocks, 0);
        assert_eq!(state.completed_rotations, 0);
    }

    #[test]
    fn next_slot_descriptor_tracks_the_serial_clock_and_wraps_after_eighteen_slots() {
        let mut core = Ym2413Core::new();

        for expected_slot in 0..18_u8 {
            let descriptor = core.next_slot_descriptor().unwrap();
            assert_eq!(descriptor.slot_number, expected_slot + 1);
            core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_SLOT)
                .unwrap();
        }

        assert_eq!(core.slot_index(), 0);
        assert_eq!(core.next_slot_descriptor().unwrap().slot_number, 1);
    }

    #[test]
    fn slot_descriptor_changes_only_at_four_clock_boundaries_and_wraps_after_one_rotation() {
        let mut core = Ym2413Core::new();

        for (slot_index, expected) in YM2413_SLOT_MAP.iter().copied().enumerate() {
            assert_eq!(core.next_slot_descriptor(), Some(expected));

            for clock_in_slot in 1..=YM2413_INPUT_CLOCKS_PER_SLOT {
                let crossed_rotation = slot_index + 1 == YM2413_SLOT_COUNT
                    && clock_in_slot == YM2413_INPUT_CLOCKS_PER_SLOT;
                assert_eq!(
                    core.advance_input_clocks(1),
                    Ok(if crossed_rotation { 1 } else { 0 })
                );

                if clock_in_slot < YM2413_INPUT_CLOCKS_PER_SLOT {
                    assert_eq!(core.next_slot_descriptor(), Some(expected));
                    assert_eq!(core.input_clock_phase(), clock_in_slot as u8);
                } else {
                    let next_slot = (slot_index + 1) % YM2413_SLOT_COUNT;
                    assert_eq!(
                        core.next_slot_descriptor(),
                        Some(YM2413_SLOT_MAP[next_slot])
                    );
                    assert_eq!(core.input_clock_phase(), 0);
                }
            }
        }

        assert_eq!(
            core.elapsed_input_clocks(),
            YM2413_INPUT_CLOCKS_PER_ROTATION
        );
        assert_eq!(core.completed_rotations(), 1);
        assert_eq!(core.slot_index(), 0);
    }

    #[test]
    fn snapshot_restore_preserves_slot_boundary_state_at_every_clock_offset() {
        for offset in 0..YM2413_INPUT_CLOCKS_PER_ROTATION {
            let mut core = Ym2413Core::new();
            core.advance_input_clocks(offset).unwrap();

            let expected_descriptor = core.next_slot_descriptor();
            let expected_phase = core.input_clock_phase();
            let expected_rotation = core.completed_rotations();
            let restored = Ym2413Core::restore_state(core.export_state()).unwrap();

            assert_eq!(restored.next_slot_descriptor(), expected_descriptor);
            assert_eq!(restored.input_clock_phase(), expected_phase);
            assert_eq!(restored.completed_rotations(), expected_rotation);
            assert_eq!(restored.elapsed_input_clocks(), offset);
        }
    }
    #[derive(Clone, Default)]
    struct RecordingSlotService {
        order: Vec<(bool, u8)>,
        before: Vec<Ym2413SlotServiceContext>,
        after: Vec<(u8, u8, u8, u8, u8, u8)>,
    }

    impl Ym2413SlotService for RecordingSlotService {
        type Error = Infallible;

        fn before_eg_update(
            &mut self,
            context: &Ym2413SlotServiceContext,
        ) -> Result<(), Self::Error> {
            self.order.push((true, context.event.cycle));
            self.before.push(*context);
            Ok(())
        }

        fn after_eg_update(
            &mut self,
            context: &Ym2413SlotServiceContext,
            tick: EnvelopeSequencerTick,
            post_state: EnvelopeSequencerState,
        ) -> Result<(), Self::Error> {
            self.order.push((false, context.event.cycle));
            let post_operator = post_state.operators[usize::from(tick.operator_slot)];
            self.after.push((
                context.event.cycle,
                tick.operator_slot,
                tick.operator.level,
                post_operator.level,
                (tick.eg_dokon & 0xff) as u8,
                (post_state.eg_dokon & 0xff) as u8,
            ));
            Ok(())
        }
    }

    #[test]
    fn internal_slot_service_runs_before_and_after_exactly_one_core_eg_update() {
        let mut core = Ym2413Core::new();
        let mut service = RecordingSlotService::default();

        assert_eq!(
            core.advance_input_clocks_with_slot_service(
                YM2413_INPUT_CLOCKS_PER_SLOT * YM2413_SLOT_COUNT as u64,
                &mut service,
            ),
            Ok(1)
        );

        assert_eq!(service.before.len(), YM2413_SLOT_COUNT);
        assert_eq!(service.after.len(), YM2413_SLOT_COUNT);
        assert_eq!(service.order.len(), YM2413_SLOT_COUNT * 2);

        for cycle in 0..YM2413_SLOT_COUNT as u8 {
            let before = service.before[usize::from(cycle)];
            let after = service.after[usize::from(cycle)];
            assert_eq!(before.event.cycle, cycle);
            assert_eq!(
                before.envelope_input.rhythm_bits,
                before.registers[0x0e] & 0x3f
            );
            assert!(before.rm_select <= 6);
            assert_eq!(after.0, cycle);
            assert_eq!(after.1, before.event.eg_update_index);
            assert_eq!(after.2, after.3);
            assert_eq!(after.4, after.5);
            assert_eq!(
                before.event.selected_operator.channel,
                YM2413_EG_CYCLE_CHANNELS[usize::from(cycle)]
            );
            assert_eq!(
                before.event.phase_reset_required,
                nuked_phase_reset_required(cycle, before.rm_enable, before.envelope_state.eg_dokon)
                    .unwrap()
            );
            assert_eq!(service.order[usize::from(cycle) * 2], (true, cycle));
            assert_eq!(service.order[usize::from(cycle) * 2 + 1], (false, cycle));
        }
    }

    #[test]
    fn internal_slot_service_is_equivalent_for_continuous_and_partitioned_blocks() {
        let total = YM2413_INPUT_CLOCKS_PER_ROTATION + 7;
        let mut whole = Ym2413Core::new();
        let mut whole_service = RecordingSlotService::default();
        whole
            .advance_input_clocks_with_slot_service(total, &mut whole_service)
            .unwrap();

        let mut standalone = Ym2413Core::new();
        standalone.advance_input_clocks(total).unwrap();
        assert_eq!(whole.export_state(), standalone.export_state());

        let mut partitioned = Ym2413Core::new();
        let mut partitioned_service = RecordingSlotService::default();
        let mut remaining = total;
        for block in [1_u64, 2, 3, 5, 8, 13, 21, 27] {
            if remaining == 0 {
                break;
            }
            let count = block.min(remaining);
            partitioned
                .advance_input_clocks_with_slot_service(count, &mut partitioned_service)
                .unwrap();
            remaining -= count;
        }
        if remaining != 0 {
            partitioned
                .advance_input_clocks_with_slot_service(remaining, &mut partitioned_service)
                .unwrap();
        }

        assert_eq!(partitioned.export_state(), whole.export_state());
        assert_eq!(partitioned_service.order, whole_service.order);
        assert_eq!(partitioned_service.before, whole_service.before);
        assert_eq!(partitioned_service.after, whole_service.after);
    }

    #[test]
    fn internal_slot_service_error_rolls_back_core_and_service_and_retry_is_idempotent() {
        #[derive(Clone, Debug, Default, PartialEq, Eq)]
        struct MutatingFailingService {
            before_count: usize,
            after_count: usize,
            history: Vec<(u8, bool)>,
            fail_cycle: Option<u8>,
        }

        impl Ym2413SlotService for MutatingFailingService {
            type Error = u8;

            fn before_eg_update(
                &mut self,
                context: &Ym2413SlotServiceContext,
            ) -> Result<(), Self::Error> {
                self.before_count += 1;
                self.history.push((context.event.cycle, false));
                Ok(())
            }

            fn after_eg_update(
                &mut self,
                context: &Ym2413SlotServiceContext,
                _tick: EnvelopeSequencerTick,
                _post_state: EnvelopeSequencerState,
            ) -> Result<(), Self::Error> {
                self.after_count += 1;
                self.history.push((context.event.cycle, true));
                if self.fail_cycle == Some(context.event.cycle) {
                    Err(context.event.cycle)
                } else {
                    Ok(())
                }
            }
        }

        let clocks = YM2413_INPUT_CLOCKS_PER_SLOT * 6;
        let mut core = Ym2413Core::new();
        let mut service = MutatingFailingService {
            fail_cycle: Some(3),
            ..MutatingFailingService::default()
        };
        let before_core = core.export_state();
        let before_service = service.clone();

        let result = core.advance_input_clocks_with_slot_service(clocks, &mut service);

        assert_eq!(result, Err(Ym2413SlotServiceAdvanceError::Service(3)));
        assert_eq!(core.export_state(), before_core);
        assert_eq!(service, before_service);

        // Disable the injected fault and retry from the exact pre-call state.
        service.fail_cycle = None;
        assert_eq!(
            core.advance_input_clocks_with_slot_service(clocks, &mut service),
            Ok(0)
        );

        let mut reference_core = Ym2413Core::new();
        let mut reference_service = MutatingFailingService::default();
        assert_eq!(
            reference_core.advance_input_clocks_with_slot_service(clocks, &mut reference_service),
            Ok(0)
        );
        assert_eq!(core.export_state(), reference_core.export_state());
        assert_eq!(service, reference_service);
        assert_eq!(service.before_count, 6);
        assert_eq!(service.after_count, 6);
    }
    #[test]
    fn slot_service_phase_reset_includes_testmode_register_bit_two() {
        for cycle in 0..YM2413_SLOT_COUNT as u8 {
            let mut core = Ym2413Core::new();
            core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_SLOT * u64::from(cycle))
                .unwrap();
            core.write_mode_address = 0x1f;
            core.write_d_enable = true;
            core.write_data_latch = 0x04;
            let mut envelope_state = core.envelope.export_state();
            envelope_state.eg_dokon = 0;
            core.envelope = super::EnvelopeSequencer::restore_state(envelope_state).unwrap();

            let mut service = RecordingSlotService::default();
            core.advance_input_clocks_with_slot_service(YM2413_INPUT_CLOCKS_PER_SLOT, &mut service)
                .unwrap();

            assert!(
                service.before[0].event.phase_reset_required,
                "cycle {cycle}"
            );
        }
    }

    #[test]
    fn slot_service_phase_reset_uses_post_shift_rhythm_mode_latch() {
        for cycle in [13_u8, 14_u8] {
            for (eg_dokon, expected) in [
                (0x8000_u32, false),
                (0x0001, true),
                (0x0000, false),
                (0x8001, true),
            ] {
                let mut core = Ym2413Core::new();
                core.advance_input_clocks(YM2413_INPUT_CLOCKS_PER_SLOT * u64::from(cycle))
                    .unwrap();
                core.rm_enable = 0x80_u8 as i8;
                let mut envelope_state = core.envelope.export_state();
                envelope_state.eg_dokon = eg_dokon;
                core.envelope = super::EnvelopeSequencer::restore_state(envelope_state).unwrap();

                let mut service = RecordingSlotService::default();
                core.advance_input_clocks_with_slot_service(
                    YM2413_INPUT_CLOCKS_PER_SLOT,
                    &mut service,
                )
                .unwrap();

                let context = service.before[0];
                assert_ne!(context.rm_enable & 0x40, 0);
                assert_eq!(context.event.phase_reset_required, expected);
                assert_eq!(
                    nuked_phase_reset_required(cycle, context.rm_enable, eg_dokon),
                    Some(expected)
                );
                // The pre-shift latch selects the opposite operator path in this
                // probe; retain it as a guard against accidentally sampling early.
                assert_eq!(
                    nuked_phase_reset_required(cycle, 0x80, eg_dokon),
                    Some(eg_dokon & 0x8000 != 0)
                );
            }
        }
    }
}
