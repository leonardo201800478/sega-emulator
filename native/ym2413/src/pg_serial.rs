//! Serial YM2413 Phase Generator service.
//!
//! This service is called by the core-owned pre/post EG slot hooks. It keeps
//! Nuked's delayed PG array write, phase-next latch, increment latch and PM
//! counter in one cloneable state machine. The core remains the sole owner of
//! input clocks and the serial EG sequencer.
//!
//! Melodic and explicitly seeded Rhythm Mode slots share this serial PG/operator
//! service. Unknown physical LFSR reset state and unsettled mode transitions
//! remain gated rather than being filled with guessed noise or patch state.

use crate::am_lfo::{Ym2413SerialAmLfo, Ym2413SerialAmLfoError, Ym2413SerialAmLfoState};
use crate::channel::{ChannelSynthesisError, OperatorPhaseParameters, decode_channel_synthesis};
use crate::envelope_sequencer::{
    EnvelopeRhythmSelect, EnvelopeSequencerState, EnvelopeSequencerTick,
};
use crate::output_pipeline::{
    Ym2413CoreOutputPipelineFrame, Ym2413OutputPipeline, Ym2413OutputPipelineError,
    Ym2413OutputPipelineState,
};
use crate::phase::{PhaseGenerator, PhaseGeneratorError};
use crate::rhythm::{
    Ym2413RhythmNoiseError, Ym2413RhythmNoiseSlotSample, Ym2413RhythmSlotNoiseSequencer,
    Ym2413RhythmSlotNoiseSequencerState,
};
use crate::rhythm_channel::{
    RhythmOperatorSynthesisConfig, RhythmVoiceSynthesisConfig, decode_rhythm_synthesis,
};
use crate::rhythm_dispatch::{Ym2413SerialSlotDispatch, dispatch_serial_slot};
use crate::serial_operator::{
    Ym2413SerialOperator, Ym2413SerialOperatorError, Ym2413SerialOperatorPatch1Latches,
    Ym2413SerialOperatorPatch2Latches, Ym2413SerialOperatorState,
};
use crate::slot::{Ym2413Operator, Ym2413SlotDescriptor, nuked_pg_phase_slot_descriptor};
use crate::slot_service::{Ym2413SlotService, Ym2413SlotServiceContext};
use crate::synthesis::{
    Ym2413SynthesisBank, Ym2413SynthesisBankState, Ym2413SynthesisError,
    Ym2413SynthesisInitialState,
};

pub const YM2413_PG_SERIAL_STATE_VERSION: u16 = 10;
pub const YM2413_PG_SERIAL_PIPELINE_STATE_VERSION: u16 = 4;

/// Maximum increment reachable from the legal FNUM/BLOCK/MUL/PM input domain.
pub const YM2413_PG_INCREMENT_MAX: u32 = 987_840;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413PgSerialError {
    InvalidState,
    InvalidSlotIndex(u8),
    UnsupportedRhythmMode { cycle: u8, channel: u8 },
    Channel(ChannelSynthesisError),
    Phase(PhaseGeneratorError),
    Synthesis(Ym2413SynthesisError),
    Operator(Ym2413SerialOperatorError),
    AmLfo(Ym2413SerialAmLfoError),
    RhythmNoise(Ym2413RhythmNoiseError),
    OutputPipeline(Ym2413OutputPipelineError),
}

/// Error returned by the public coarse-grained core + PG advancement API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413PgSerialAdvanceError {
    Core(crate::Ym2413Error),
    Service(Ym2413PgSerialError),
}

impl std::fmt::Display for Ym2413PgSerialAdvanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => write!(f, "YM2413 core error: {error}"),
            Self::Service(error) => write!(f, "YM2413 PG service error: {error:?}"),
        }
    }
}

impl std::error::Error for Ym2413PgSerialAdvanceError {}

impl From<ChannelSynthesisError> for Ym2413PgSerialError {
    fn from(value: ChannelSynthesisError) -> Self {
        Self::Channel(value)
    }
}
impl From<PhaseGeneratorError> for Ym2413PgSerialError {
    fn from(value: PhaseGeneratorError) -> Self {
        Self::Phase(value)
    }
}
impl From<Ym2413SynthesisError> for Ym2413PgSerialError {
    fn from(value: Ym2413SynthesisError) -> Self {
        Self::Synthesis(value)
    }
}
impl From<Ym2413SerialOperatorError> for Ym2413PgSerialError {
    fn from(value: Ym2413SerialOperatorError) -> Self {
        Self::Operator(value)
    }
}
impl From<Ym2413SerialAmLfoError> for Ym2413PgSerialError {
    fn from(value: Ym2413SerialAmLfoError) -> Self {
        Self::AmLfo(value)
    }
}
impl From<Ym2413RhythmNoiseError> for Ym2413PgSerialError {
    fn from(value: Ym2413RhythmNoiseError) -> Self {
        Self::RhythmNoise(value)
    }
}
impl From<Ym2413OutputPipelineError> for Ym2413PgSerialError {
    fn from(value: Ym2413OutputPipelineError) -> Self {
        Self::OutputPipeline(value)
    }
}

/// Captured PG phase bits shared by the three special Rhythm Mode phase paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ym2413RhythmPhaseLatches {
    pub hh_bit2: bool,
    pub hh_bit3: bool,
    pub hh_bit7: bool,
    pub hh_bit8: bool,
    pub tc_bit3: bool,
    pub tc_bit5: bool,
}

impl Ym2413RhythmPhaseLatches {
    const fn rhythm_mix_bit(self) -> bool {
        (self.hh_bit2 ^ self.hh_bit7)
            || (self.hh_bit3 ^ self.tc_bit5)
            || (self.tc_bit3 ^ self.tc_bit5)
    }
}

/// Parameters retained by Nuked's c_fnum/c_block/c_multi/c_vib latches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413PgParameterLatch {
    pub fnum: u16,
    pub block: u8,
    pub multiplier: u8,
    pub pm_enabled: bool,
}

impl Ym2413PgParameterLatch {
    const RESET: Self = Self {
        fnum: 0,
        block: 0,
        multiplier: 0,
        pm_enabled: false,
    };

    fn from_operator(parameters: OperatorPhaseParameters) -> Self {
        Self {
            fnum: parameters.fnum,
            block: parameters.block,
            multiplier: parameters.multiplier,
            pm_enabled: parameters.pm_enabled,
        }
    }

    const fn is_valid(self) -> bool {
        self.fnum <= 0x01ff && self.block <= 7 && self.multiplier <= 0x0f
    }
}

/// Latches owned by the serial PG pipeline, separate from the 18 accumulators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413PgSerialPipelineState {
    pub format_version: u16,
    pub pg_increment: u32,
    pub pg_phase_next: u32,
    pub pg_output: u16,
    /// Nuked's three-bit lfo_vib_counter, which can diverge from lfo_counter
    /// while Test Mode bit 3 is active.
    pub lfo_vib_counter: u8,
    pub parameters: Ym2413PgParameterLatch,
    /// HH/TCY phase bits captured at their serial PG boundaries.
    pub rhythm_phase_latches: Ym2413RhythmPhaseLatches,
}

/// Complete serial PG service snapshot, including all eighteen PG accumulators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413PgSerialServiceState {
    pub format_version: u16,
    pub pipeline: Ym2413PgSerialPipelineState,
    pub synthesis: Ym2413SynthesisBankState,
    pub operator: Ym2413SerialOperatorState,
    pub serial_am_lfo: Ym2413SerialAmLfoState,
    /// Explicitly seeded slot-rate noise state; None means physical reset seed is unresolved.
    pub rhythm_noise: Option<Ym2413RhythmSlotNoiseSequencerState>,
    /// Core-slot cursor for DoRhythm, including the unseeded/unknown-register model.
    pub rhythm_noise_cursor: u8,
    /// Number of DoRhythm slot updates since service construction or reset.
    pub rhythm_noise_steps: u64,
    /// Last sampled Hi-Hat and Snare Drum noise bits, retained for later phase routing.
    pub rhythm_noise_samples: [Option<bool>; 2],
    /// Digital MO/RO pipeline, including explicit output-validity state.
    pub output_pipeline: Ym2413OutputPipelineState,
    /// Most recently emitted output-buffer frame, retained for exact resumption.
    pub last_output_frame: Option<Ym2413CoreOutputPipelineFrame>,
}

/// Resolve one serial rhythm selector to its static voice/operator configuration.
fn rhythm_voice_operator(
    registers: &[u8; 64],
    selector: EnvelopeRhythmSelect,
    cycle: u8,
) -> Result<
    (
        RhythmVoiceSynthesisConfig,
        RhythmOperatorSynthesisConfig,
        bool,
    ),
    Ym2413PgSerialError,
> {
    let config = decode_rhythm_synthesis(registers)
        .map_err(|error| Ym2413PgSerialError::Channel(ChannelSynthesisError::Rhythm(error)))?;
    let (voice_index, carrier) = match selector {
        EnvelopeRhythmSelect::BassDrumFirst => (0, false),
        EnvelopeRhythmSelect::HiHat => (1, false),
        EnvelopeRhythmSelect::TomTom => (3, false),
        EnvelopeRhythmSelect::BassDrumSecond => (0, true),
        EnvelopeRhythmSelect::SnareDrum => (2, true),
        EnvelopeRhythmSelect::TopCymbal => (4, true),
        EnvelopeRhythmSelect::None => {
            return Err(Ym2413PgSerialError::UnsupportedRhythmMode { cycle, channel: 0 });
        }
    };
    let voice = config.voices[voice_index];
    let operator = (if carrier {
        voice.carrier
    } else {
        voice.modulator
    })
    .ok_or(Ym2413PgSerialError::Channel(ChannelSynthesisError::Rhythm(
        crate::rhythm_channel::RhythmChannelSynthesisError::MissingEnvelopeOperator { cycle },
    )))?;
    Ok((voice, operator, carrier))
}

/// Resolve per-slot patch fields that are not shared by the two BD operators.
///
/// The capture-derived BD candidate stores FB and DM in modulator-specific
/// fields. The pinned Nuked rhythm table models BD-carrier separately with
/// FB=0 and DM=0. Keep the serial latches operator-specific; this does not
/// certify the complete physical rhythm ROM or the analog BD transfer.
/// Evidence and the unresolved SD SL candidate are recorded in
/// `docs/audits/ym2413-fm5-2b-serial-rhythm-pipeline-2026-11-21.md`.
fn rhythm_operator_patch2_latches(
    selector: EnvelopeRhythmSelect,
    voice: RhythmVoiceSynthesisConfig,
    operator: RhythmOperatorSynthesisConfig,
) -> Ym2413SerialOperatorPatch2Latches {
    let bass_drum_modulator = matches!(selector, EnvelopeRhythmSelect::BassDrumFirst);
    Ym2413SerialOperatorPatch2Latches {
        feedback: if bass_drum_modulator {
            voice.feedback
        } else {
            0
        },
        dc_waveform: voice.carrier.is_some_and(|value| value.patch.half_wave),
        dm_waveform: bass_drum_modulator
            && voice.modulator.is_some_and(|value| value.patch.half_wave),
        am_enabled: operator.patch.am_enabled,
        vibrato_enabled: operator.patch.vibrato_enabled,
        multiplier: operator.phase.multiplier,
        sustain_level: operator.patch.sustain_level,
        fnum: voice.channel_frequency.fnum,
        block: voice.channel_frequency.block,
    }
}

/// Capture the special HH/TCY phase bits and form the corresponding PG address.
/// The equations and capture cycles mirror Nuked-OPLL OPLL_PhaseGenerate.
fn rhythm_pg_output(
    cycle: u8,
    phase: u32,
    rm_enable: u8,
    rhythm_noise_available: bool,
    noise_bit: Option<bool>,
    latches: &mut Ym2413RhythmPhaseLatches,
) -> u16 {
    if cycle == 13 {
        latches.hh_bit2 = (phase >> 11) & 1 != 0;
        latches.hh_bit3 = (phase >> 12) & 1 != 0;
        latches.hh_bit7 = (phase >> 16) & 1 != 0;
        latches.hh_bit8 = (phase >> 17) & 1 != 0;
    } else if cycle == 17 && rm_enable & 0x80 != 0 {
        latches.tc_bit3 = (phase >> 12) & 1 != 0;
        latches.tc_bit5 = (phase >> 14) & 1 != 0;
    }

    if rm_enable & 0x80 == 0 || !rhythm_noise_available {
        return (phase >> 9) as u16;
    }

    match cycle {
        13 => match noise_bit {
            Some(noise) => {
                let phase_bit = latches.rhythm_mix_bit();
                let low = if phase_bit ^ noise { 0x0d0 } else { 0x034 };
                (if phase_bit { 0x0200 } else { 0 }) | low
            }
            None => (phase >> 9) as u16,
        },
        16 => match noise_bit {
            Some(noise) => {
                (if latches.hh_bit8 { 0x0200 } else { 0 })
                    | (if latches.hh_bit8 ^ noise { 0x0100 } else { 0 })
            }
            None => (phase >> 9) as u16,
        },
        17 => (if latches.rhythm_mix_bit() { 0x0200 } else { 0 }) | 0x0100,
        _ => (phase >> 9) as u16,
    }
}

/// Cloneable serial PG state machine. Its Clone is deep: all mutable state is
/// owned values or the synthesis bank's independently cloned value state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ym2413PgSerialService {
    synthesis: Ym2413SynthesisBank,
    pipeline: Ym2413PgSerialPipelineState,
    operator: Ym2413SerialOperator,
    serial_am_lfo: Ym2413SerialAmLfo,
    rhythm_noise: Option<Ym2413RhythmSlotNoiseSequencer>,
    rhythm_noise_cursor: u8,
    rhythm_noise_steps: u64,
    rhythm_noise_samples: [Option<bool>; 2],
    output_pipeline: Ym2413OutputPipeline,
    last_output_frame: Option<Ym2413CoreOutputPipelineFrame>,
}

impl Ym2413PgSerialService {
    /// Create a serial PG service around an explicitly initialized synthesis bank.
    pub fn new(synthesis: Ym2413SynthesisBank) -> Self {
        Self {
            synthesis,
            pipeline: Ym2413PgSerialPipelineState {
                format_version: YM2413_PG_SERIAL_PIPELINE_STATE_VERSION,
                pg_increment: 0,
                pg_phase_next: 0,
                pg_output: 0,
                lfo_vib_counter: 0,
                parameters: Ym2413PgParameterLatch::RESET,
                rhythm_phase_latches: Ym2413RhythmPhaseLatches::default(),
            },
            operator: Ym2413SerialOperator::new(),
            serial_am_lfo: Ym2413SerialAmLfo::new(),
            rhythm_noise: None,
            rhythm_noise_cursor: 0,
            rhythm_noise_steps: 0,
            rhythm_noise_samples: [None, None],
            output_pipeline: Ym2413OutputPipeline::new(),
            last_output_frame: None,
        }
    }

    /// Build a reset candidate bank for tests and explicit initial-state users.
    pub fn with_initial_state(
        initial: Ym2413SynthesisInitialState,
    ) -> Result<Self, Ym2413SynthesisError> {
        Ok(Self::new(Ym2413SynthesisBank::with_initial_state(initial)?))
    }

    /// Enable the slot-rate rhythm-noise candidate with an explicit seed and
    /// the core slot that will be serviced next. The physical power-on seed
    /// is unknown; this API deliberately has no default seed.
    pub fn with_rhythm_noise_seed(
        mut self,
        seed: u32,
        next_slot_index: u8,
    ) -> Result<Self, Ym2413PgSerialError> {
        self.rhythm_noise = Some(Ym2413RhythmSlotNoiseSequencer::with_seed_at_slot(
            seed,
            next_slot_index,
        )?);
        self.rhythm_noise_cursor = next_slot_index;
        self.rhythm_noise_samples = [None, None];
        self.output_pipeline = self.output_pipeline.with_next_cycle(next_slot_index)?;
        Ok(self)
    }

    /// Enable an explicit 23-bit LFSR register candidate, including zero.
    ///
    /// Zero reproduces the pinned Nuked software reset state only; it is not
    /// asserted to be the physical YM2413 power-on register value.
    pub fn with_rhythm_noise_register(
        mut self,
        register: u32,
        next_slot_index: u8,
    ) -> Result<Self, Ym2413PgSerialError> {
        self.rhythm_noise = Some(Ym2413RhythmSlotNoiseSequencer::with_register_at_slot(
            register,
            next_slot_index,
        )?);
        self.rhythm_noise_cursor = next_slot_index;
        self.rhythm_noise_samples = [None, None];
        self.output_pipeline = self.output_pipeline.with_next_cycle(next_slot_index)?;
        Ok(self)
    }

    /// Align the unseeded symbolic DoRhythm cursor to a core-owned next slot.
    ///
    /// This is useful when constructing a service for a restored core whose
    /// LFSR register remains unknown. It does not choose a seed or fabricate
    /// the noise output bits.
    pub fn with_rhythm_noise_cursor(
        mut self,
        next_slot_index: u8,
    ) -> Result<Self, Ym2413PgSerialError> {
        if next_slot_index >= crate::rhythm::YM2413_RHYTHM_NOISE_SLOTS_PER_ROTATION {
            return Err(Ym2413PgSerialError::InvalidSlotIndex(next_slot_index));
        }
        if self.rhythm_noise.is_some() {
            return Err(Ym2413PgSerialError::InvalidState);
        }
        self.rhythm_noise_cursor = next_slot_index;
        self.rhythm_noise_samples = [None, None];
        self.output_pipeline = self.output_pipeline.with_next_cycle(next_slot_index)?;
        Ok(self)
    }

    /// Return the last sampled Hi-Hat and Snare Drum noise bits, if available.
    pub const fn rhythm_noise_samples(&self) -> [Option<bool>; 2] {
        self.rhythm_noise_samples
    }

    /// Return the next core-owned DoRhythm slot cursor.
    pub const fn rhythm_noise_cursor(&self) -> u8 {
        self.rhythm_noise_cursor
    }

    /// Return the number of DoRhythm slot updates since service construction.
    pub const fn rhythm_noise_steps(&self) -> u64 {
        self.rhythm_noise_steps
    }

    /// Return the optional slot-rate noise sequencer state.
    pub const fn rhythm_noise_state(&self) -> Option<Ym2413RhythmSlotNoiseSequencerState> {
        match self.rhythm_noise {
            Some(sequencer) => Some(sequencer.export_state()),
            None => None,
        }
    }

    /// Return the current ten-bit PG output latch.
    pub const fn pg_output(&self) -> u16 {
        self.pipeline.pg_output
    }

    /// Return the retained PG increment.
    pub const fn pg_increment(&self) -> u32 {
        self.pipeline.pg_increment
    }

    /// Return the delayed PG next-phase latch.
    pub const fn pg_phase_next(&self) -> u32 {
        self.pipeline.pg_phase_next
    }

    /// Return the current three-bit vibrato LFO counter.
    pub const fn lfo_vib_counter(&self) -> u8 {
        self.pipeline.lfo_vib_counter
    }

    /// Borrow the eighteen-operator synthesis bank.
    pub const fn synthesis(&self) -> &Ym2413SynthesisBank {
        &self.synthesis
    }

    /// Return the most recently emitted digital MO/RO output-buffer frame.
    pub const fn output_frame(&self) -> Option<Ym2413CoreOutputPipelineFrame> {
        self.last_output_frame
    }

    /// Return the complete digital MO/RO pipeline state.
    pub const fn output_pipeline_state(&self) -> Ym2413OutputPipelineState {
        self.output_pipeline.export_state()
    }

    /// Export every accumulator, latch and LFO state needed for exact resumption.
    pub const fn export_state(&self) -> Ym2413PgSerialServiceState {
        Ym2413PgSerialServiceState {
            format_version: YM2413_PG_SERIAL_STATE_VERSION,
            pipeline: self.pipeline,
            synthesis: self.synthesis.export_state(),
            operator: self.operator.export_state(),
            serial_am_lfo: self.serial_am_lfo.export_state(),
            rhythm_noise: self.rhythm_noise_state(),
            rhythm_noise_cursor: self.rhythm_noise_cursor,
            rhythm_noise_steps: self.rhythm_noise_steps,
            rhythm_noise_samples: self.rhythm_noise_samples,
            output_pipeline: self.output_pipeline.export_state(),
            last_output_frame: self.last_output_frame,
        }
    }

    /// Restore the serial PG service after validating its complete state.
    pub fn restore_state(state: Ym2413PgSerialServiceState) -> Result<Self, Ym2413PgSerialError> {
        let pipeline = state.pipeline;
        if state.format_version != YM2413_PG_SERIAL_STATE_VERSION
            || pipeline.format_version != YM2413_PG_SERIAL_PIPELINE_STATE_VERSION
            || pipeline.lfo_vib_counter > 7
            || pipeline.pg_increment > YM2413_PG_INCREMENT_MAX
            || !pipeline.parameters.is_valid()
        {
            return Err(Ym2413PgSerialError::InvalidState);
        }
        if state.rhythm_noise_cursor >= crate::rhythm::YM2413_RHYTHM_NOISE_SLOTS_PER_ROTATION
            || state.output_pipeline.next_cycle != state.rhythm_noise_cursor
            || state.last_output_frame.is_some_and(|frame| {
                frame.cycle >= 18 || (frame.cycle + 1) % 18 != state.output_pipeline.next_cycle
            })
            || (state.rhythm_noise.is_none()
                && state.rhythm_noise_samples.iter().any(Option::is_some))
        {
            return Err(Ym2413PgSerialError::InvalidState);
        }
        if state
            .rhythm_noise
            .is_some_and(|noise| noise.slot_index != state.rhythm_noise_cursor)
        {
            return Err(Ym2413PgSerialError::InvalidState);
        }
        let rhythm_noise = state
            .rhythm_noise
            .map(Ym2413RhythmSlotNoiseSequencer::restore_state)
            .transpose()?;
        let output_pipeline = Ym2413OutputPipeline::with_initial_state(state.output_pipeline)?;
        Ok(Self {
            synthesis: Ym2413SynthesisBank::restore_state(state.synthesis)?,
            pipeline,
            operator: Ym2413SerialOperator::restore_state(state.operator)?,
            serial_am_lfo: Ym2413SerialAmLfo::restore_state(state.serial_am_lfo)?,
            rhythm_noise,
            rhythm_noise_cursor: state.rhythm_noise_cursor,
            rhythm_noise_steps: state.rhythm_noise_steps,
            rhythm_noise_samples: state.rhythm_noise_samples,
            output_pipeline,
            last_output_frame: state.last_output_frame,
        })
    }

    /// Consume one PG phase-array write and one delayed output/reset decision.
    fn generate_phase(
        &mut self,
        context: &Ym2413SlotServiceContext,
        noise_bit: Option<bool>,
    ) -> Result<(), Ym2413PgSerialError> {
        let write = nuked_pg_phase_slot_descriptor(context.event.pg_phase_write_index).ok_or(
            Ym2413PgSerialError::InvalidSlotIndex(context.event.pg_phase_write_index),
        )?;
        let write_carrier = matches!(write.operator, Ym2413Operator::Carrier);
        let phase_write = self
            .pipeline
            .pg_phase_next
            .wrapping_add(self.pipeline.pg_increment);
        self.synthesis
            .write_operator_phase(write.channel, write_carrier, phase_write)?;

        let output = nuked_pg_phase_slot_descriptor(context.event.pg_phase_output_index).ok_or(
            Ym2413PgSerialError::InvalidSlotIndex(context.event.pg_phase_output_index),
        )?;
        let output_carrier = matches!(output.operator, Ym2413Operator::Carrier);
        let phase = self
            .synthesis
            .phase_accumulator(output.channel, output_carrier)?;

        // Nuked stores the reset decision in pg_phase_next. It does not reset
        // the current output accumulator or the phase array entry just written.
        self.pipeline.pg_phase_next = if context.event.phase_reset_required {
            0
        } else {
            phase
        };
        self.pipeline.pg_output = rhythm_pg_output(
            context.event.cycle,
            phase,
            context.rm_enable,
            self.rhythm_noise.is_some(),
            noise_bit,
            &mut self.pipeline.rhythm_phase_latches,
        );
        Ok(())
    }

    /// Calculate pg_inc from the previous cycle's retained c_* parameter latches.
    fn calculate_increment(&mut self) -> Result<(), Ym2413PgSerialError> {
        self.pipeline.pg_increment = PhaseGenerator::phase_step(
            self.pipeline.parameters.fnum,
            self.pipeline.parameters.block,
            self.pipeline.parameters.multiplier,
            self.pipeline.parameters.pm_enabled,
            self.pipeline.lfo_vib_counter,
        )?;
        Ok(())
    }

    /// Advance the PM counters at cycle 17, matching the OPLL_DoLFO stage.
    fn clock_pm_lfo(&mut self, testmode: u8) {
        let counter = self.synthesis.pm_lfo_counter();
        let rollover = u8::from((counter & 0x03ff) == 0x03ff);
        let test_step = (testmode >> 3) & 1;
        self.pipeline.lfo_vib_counter = self
            .pipeline
            .lfo_vib_counter
            .wrapping_add(rollover | test_step)
            & 7;
        self.synthesis.advance_pm_lfo_samples(1);
    }

    /// Resolve the core cycle to a melodic slot. Rhythm-owned slots use the
    /// dedicated rhythm decoder and never borrow channels 7–9 as melodic inputs.
    fn require_melodic_slot(
        context: &Ym2413SlotServiceContext,
    ) -> Result<Ym2413SlotDescriptor, Ym2413PgSerialError> {
        let dispatch = dispatch_serial_slot(
            context.event.cycle,
            &context.registers,
            context.rm_enable,
            context.envelope_input.rhythm_select,
        )
        .map_err(|_| Ym2413PgSerialError::InvalidSlotIndex(context.event.cycle))?;

        match dispatch {
            Ym2413SerialSlotDispatch::Melodic { slot }
                if slot == context.event.selected_operator =>
            {
                Ok(slot)
            }
            Ym2413SerialSlotDispatch::Melodic { .. } => Err(Ym2413PgSerialError::InvalidState),
            _ => Err(Ym2413PgSerialError::UnsupportedRhythmMode {
                cycle: context.event.cycle,
                channel: context.event.selected_operator.channel,
            }),
        }
    }

    /// Return the current pre-clock LFSR bit for this core-owned serial slot.
    fn current_rhythm_noise_bit(&self, cycle: u8) -> Result<Option<bool>, Ym2413PgSerialError> {
        if self.rhythm_noise_cursor != cycle {
            return Err(Ym2413PgSerialError::InvalidState);
        }
        match self.rhythm_noise {
            Some(sequencer) if sequencer.slot_index() == cycle => Ok(Some(sequencer.noise_bit())),
            Some(_) => Err(Ym2413PgSerialError::InvalidState),
            None => Ok(None),
        }
    }

    /// Execute the DoRhythm noise step at the post-EG boundary.
    ///
    /// The seeded candidate advances once on every serial slot, irrespective
    /// of RHY mode. The sequencer cursor must match the core-owned slot index;
    /// a mismatch rejects the staged service transaction without partial state.
    fn clock_rhythm_noise(
        &mut self,
        context: &Ym2413SlotServiceContext,
    ) -> Result<(), Ym2413PgSerialError> {
        if self.rhythm_noise_cursor != context.event.cycle {
            return Err(Ym2413PgSerialError::InvalidState);
        }
        if let Some(sequencer) = self.rhythm_noise.as_mut() {
            if sequencer.slot_index() != context.event.cycle {
                return Err(Ym2413PgSerialError::InvalidState);
            }
            if let Some(Ym2413RhythmNoiseSlotSample {
                voice, noise_bit, ..
            }) = sequencer.advance_slot_with_test_mode(context.registers[0x0f])
            {
                match voice {
                    crate::slot::Ym2413RhythmVoice::HiHat => {
                        self.rhythm_noise_samples[0] = Some(noise_bit);
                    }
                    crate::slot::Ym2413RhythmVoice::SnareDrum => {
                        self.rhythm_noise_samples[1] = Some(noise_bit);
                    }
                    _ => return Err(Ym2413PgSerialError::InvalidState),
                }
            }
        }
        // Even without a candidate seed, retain the core-aligned DoRhythm
        // cadence symbolically; no noise bit is fabricated in that state.
        self.rhythm_noise_cursor =
            (self.rhythm_noise_cursor + 1) % crate::rhythm::YM2413_RHYTHM_NOISE_SLOTS_PER_ROTATION;
        self.rhythm_noise_steps = self.rhythm_noise_steps.wrapping_add(1);
        Ok(())
    }

    /// Reproduce PreparePatch2's channel/operator parameter selection for the
    /// next serial cycle in melodic mode.
    fn latch_next_parameters(
        &mut self,
        context: &Ym2413SlotServiceContext,
    ) -> Result<(), Ym2413PgSerialError> {
        let descriptor = Self::require_melodic_slot(context)?;
        let config = decode_channel_synthesis(&context.registers, descriptor.channel)?;
        let parameters = match descriptor.operator {
            Ym2413Operator::Modulator => config.modulator_phase,
            Ym2413Operator::Carrier => config.carrier_phase,
        };
        self.pipeline.parameters = Ym2413PgParameterLatch::from_operator(parameters);
        let operator_patch = match descriptor.operator {
            Ym2413Operator::Modulator => config.patch.modulator,
            Ym2413Operator::Carrier => config.patch.carrier,
        };
        self.operator
            .prepare_patch2(Ym2413SerialOperatorPatch2Latches {
                feedback: config.patch.feedback,
                dc_waveform: config.patch.carrier.half_wave,
                dm_waveform: config.patch.modulator.half_wave,
                am_enabled: operator_patch.am_enabled,
                vibrato_enabled: operator_patch.vibrato_enabled,
                multiplier: operator_patch.multiplier,
                sustain_level: operator_patch.sustain_level,
                fnum: config.channel.fnum,
                block: config.channel.block,
            })?;
        Ok(())
    }

    /// Latch the rhythm patch parameters selected by the current serial cycle.
    fn latch_next_rhythm_parameters(
        &mut self,
        context: &Ym2413SlotServiceContext,
        selector: EnvelopeRhythmSelect,
    ) -> Result<(), Ym2413PgSerialError> {
        let (voice, operator, _) =
            rhythm_voice_operator(&context.registers, selector, context.event.cycle)?;
        self.pipeline.parameters = Ym2413PgParameterLatch::from_operator(operator.phase);
        self.operator
            .prepare_patch2(rhythm_operator_patch2_latches(selector, voice, operator))?;
        Ok(())
    }

    /// Apply the rhythm voice's PreparePatch1 latches for the current slot.
    fn prepare_rhythm_patch1(
        &mut self,
        context: &Ym2413SlotServiceContext,
        selector: EnvelopeRhythmSelect,
    ) -> Result<(), Ym2413PgSerialError> {
        let (voice, operator, carrier) =
            rhythm_voice_operator(&context.registers, selector, context.event.cycle)?;
        let channel = voice.channel_frequency;
        let register = context.registers[0x30 + usize::from(channel.channel)];
        let mcsel = ((context.event.cycle + 1) / 3) & 1;
        let total_level = if matches!(
            selector,
            EnvelopeRhythmSelect::HiHat | EnvelopeRhythmSelect::TomTom
        ) {
            (register >> 4) << 2
        } else if mcsel != 0 {
            voice.volume << 2
        } else {
            operator.patch.total_level
        };
        self.operator
            .prepare_patch1(Ym2413SerialOperatorPatch1Latches {
                total_level,
                ksl_attenuation: operator.ksl,
                ksl: operator.patch.key_scale_level,
                adrr: [
                    operator.patch.attack_rate,
                    operator.patch.decay_rate,
                    operator.patch.release_rate,
                ],
                envelope_type: operator.patch.sustained_envelope,
                key_scale_rate: operator.patch.key_scale_rate,
                key_scale_rate_frequency: (channel.block << 1) | ((channel.fnum >> 8) as u8 & 1),
                key_scale_level_frequency: (channel.fnum >> 5) as u8,
                key_scale_level_block: channel.block,
            })?;
        self.operator
            .prepare_channel(register >> 4, u8::from(carrier))?;
        Ok(())
    }

    /// Advance the digital output pipeline using the core-owned post-update
    /// rm_enable value and an optional, validated next operator result.
    fn clock_output_pipeline(
        &mut self,
        context: &Ym2413SlotServiceContext,
        next_channel_output: Option<i16>,
    ) -> Result<(), Ym2413PgSerialError> {
        self.last_output_frame = Some(self.output_pipeline.process_cycle_with_core_state(
            context.event.cycle,
            context.rm_enable as i8,
            next_channel_output,
        )?);
        Ok(())
    }

    /// Run the serial operator path for one rhythm-owned slot when the LFSR
    /// seed is explicit. Unknown physical reset state remains non-synthesizing.
    fn before_rhythm_operator_update(
        &mut self,
        context: &Ym2413SlotServiceContext,
        selector: EnvelopeRhythmSelect,
    ) -> Result<(), Ym2413PgSerialError> {
        self.prepare_rhythm_patch1(context, selector)?;
        self.serial_am_lfo.capture_output(context.event.cycle)?;
        let noise_bit = if self.rhythm_noise.is_some() {
            self.current_rhythm_noise_bit(context.event.cycle)?
        } else {
            None
        };
        self.generate_phase(context, noise_bit)?;
        let operator_rm_enable = if self.rhythm_noise.is_some() {
            context.rm_enable
        } else {
            0
        };
        let sample = self.operator.tick_operator_with_rhythm_mode(
            context.event.cycle,
            self.pipeline.pg_output,
            operator_rm_enable,
        )?;
        self.clock_output_pipeline(context, Some(sample.channel_output))?;
        self.calculate_increment()?;
        let eg_levels = std::array::from_fn(|index| context.envelope_state.operators[index].level);
        self.operator.advance_envelope_output(
            context.event.cycle,
            &eg_levels,
            self.serial_am_lfo.output(),
            context.registers[0x0f],
        )?;
        Ok(())
    }
}

impl Ym2413SlotService for Ym2413PgSerialService {
    type Error = Ym2413PgSerialError;

    fn before_eg_update(&mut self, context: &Ym2413SlotServiceContext) -> Result<(), Self::Error> {
        let dispatch = dispatch_serial_slot(
            context.event.cycle,
            &context.registers,
            context.rm_enable,
            context.envelope_input.rhythm_select,
        )
        .map_err(|_| Ym2413PgSerialError::InvalidSlotIndex(context.event.cycle))?;
        let descriptor = match dispatch {
            Ym2413SerialSlotDispatch::Melodic { slot }
                if slot == context.event.selected_operator =>
            {
                slot
            }
            Ym2413SerialSlotDispatch::Melodic { .. } => {
                return Err(Ym2413PgSerialError::InvalidState);
            }
            // Unknown-seed Rhythm Mode and unsettled transitions still pass
            // through core EG and DoRhythm, but do not fabricate synthesis state.
            Ym2413SerialSlotDispatch::Rhythm { slot, selector, .. }
                if self.rhythm_noise.is_some() && slot == context.event.selected_operator =>
            {
                return self.before_rhythm_operator_update(context, selector);
            }
            Ym2413SerialSlotDispatch::Rhythm { .. } if self.rhythm_noise.is_some() => {
                return Err(Ym2413PgSerialError::InvalidState);
            }
            _ => {
                self.serial_am_lfo.capture_output(context.event.cycle)?;
                self.clock_output_pipeline(context, None)?;
                return Ok(());
            }
        };
        // Nuked order in this pre-EG hook:
        // PreparePatch1 -> Channel -> PhaseGenerate -> Operator
        // -> PhaseCalcIncrement -> EnvelopeOutput -> EnvelopeKSLTL.
        let config = decode_channel_synthesis(&context.registers, descriptor.channel)?;
        let (operator_patch, ksl_attenuation, total_level, operator_index) =
            match descriptor.operator {
                Ym2413Operator::Modulator => (
                    config.patch.modulator,
                    config.modulator_ksl,
                    config.patch.modulator.total_level,
                    0,
                ),
                Ym2413Operator::Carrier => (
                    config.patch.carrier,
                    config.carrier_ksl,
                    config.channel.volume << 2,
                    1,
                ),
            };
        self.operator
            .prepare_patch1(Ym2413SerialOperatorPatch1Latches {
                total_level,
                ksl_attenuation,
                ksl: operator_patch.key_scale_level,
                adrr: [
                    operator_patch.attack_rate,
                    operator_patch.decay_rate,
                    operator_patch.release_rate,
                ],
                envelope_type: operator_patch.sustained_envelope,
                key_scale_rate: operator_patch.key_scale_rate,
                key_scale_rate_frequency: (config.channel.block << 1)
                    | ((config.channel.fnum >> 8) as u8 & 1),
                key_scale_level_frequency: (config.channel.fnum >> 5) as u8,
                key_scale_level_block: config.channel.block,
            })?;
        self.operator
            .prepare_channel(config.channel.instrument, operator_index)?;
        self.serial_am_lfo.capture_output(context.event.cycle)?;
        let noise_bit = if self.rhythm_noise.is_some() {
            self.current_rhythm_noise_bit(context.event.cycle)?
        } else {
            None
        };
        self.generate_phase(context, noise_bit)?;
        let operator_rm_enable = if self.rhythm_noise.is_some() {
            context.rm_enable
        } else {
            0
        };
        let sample = self.operator.tick_operator_with_rhythm_mode(
            context.event.cycle,
            self.pipeline.pg_output,
            operator_rm_enable,
        )?;
        self.clock_output_pipeline(context, Some(sample.channel_output))?;
        self.calculate_increment()?;
        let eg_levels = std::array::from_fn(|index| context.envelope_state.operators[index].level);
        self.operator.advance_envelope_output(
            context.event.cycle,
            &eg_levels,
            self.serial_am_lfo.output(),
            context.registers[0x0f],
        )?;
        Ok(())
    }

    fn after_eg_update(
        &mut self,
        context: &Ym2413SlotServiceContext,
        _tick: EnvelopeSequencerTick,
        _post_state: EnvelopeSequencerState,
    ) -> Result<(), Self::Error> {
        // Nuked order after the core-owned EnvelopeGenerate stage:
        // DoLFO -> DoRhythm -> PreparePatch2.
        let testmode = context.registers[0x0f];
        // OPLL_DoLFO advances the serial AM network on every call and
        // derives its cycle-17 step from the PM counter before that counter
        // is advanced by the PM stage.
        let pm_counter = self.synthesis.pm_lfo_counter();
        self.serial_am_lfo
            .clock(context.event.cycle, pm_counter, testmode)?;
        if testmode & 0x02 != 0 {
            self.pipeline.lfo_vib_counter = 0;
            self.synthesis.reset_pm_lfo();
        } else if context.event.cycle == 17 {
            self.clock_pm_lfo(testmode);
        }
        self.clock_rhythm_noise(context)?;
        match dispatch_serial_slot(
            context.event.cycle,
            &context.registers,
            context.rm_enable,
            context.envelope_input.rhythm_select,
        )
        .map_err(|_| Ym2413PgSerialError::InvalidSlotIndex(context.event.cycle))?
        {
            Ym2413SerialSlotDispatch::Melodic { .. } => self.latch_next_parameters(context),
            Ym2413SerialSlotDispatch::Rhythm { slot, selector, .. }
                if self.rhythm_noise.is_some() && slot == context.event.selected_operator =>
            {
                self.latch_next_rhythm_parameters(context, selector)
            }
            Ym2413SerialSlotDispatch::Rhythm { .. } if self.rhythm_noise.is_some() => {
                Err(Ym2413PgSerialError::InvalidState)
            }
            // Unknown-seed and unsettled transitions remain explicitly gated.
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        YM2413_PG_SERIAL_PIPELINE_STATE_VERSION, YM2413_PG_SERIAL_STATE_VERSION,
        Ym2413PgParameterLatch, Ym2413PgSerialError, Ym2413PgSerialPipelineState,
        Ym2413PgSerialService, Ym2413RhythmPhaseLatches,
    };
    use crate::envelope::{
        AttackRate15Transition, DampTransitionConfig, DampTransitionCoordination, EnvelopeStage,
    };
    use crate::envelope_sequencer::{
        EnvelopeRhythmSelect, EnvelopeSequencer, EnvelopeSequencerState, EnvelopeSequencerTick,
        EnvelopeSlotInput,
    };
    use crate::phase::PhaseGenerator;
    use crate::slot::{nuked_pg_phase_slot_descriptor, nuked_slot_service_event_with_testmode};
    use crate::slot_service::{
        Ym2413SlotService, Ym2413SlotServiceAdvanceError, Ym2413SlotServiceContext,
    };
    use crate::synthesis::{
        Ym2413ChannelInitialState, Ym2413SynthesisBank, Ym2413SynthesisInitialState,
    };
    use crate::{YM2413_INPUT_CLOCKS_PER_SLOT, Ym2413Core};

    #[test]
    fn core_and_pg_output_pipeline_snapshot_restore_is_block_equivalent() {
        let mut continuous_core = Ym2413Core::new();
        let mut continuous_service = Ym2413PgSerialService::new(initial_bank(0));
        continuous_core
            .advance_input_clocks_with_pg_service(4 * 11, &mut continuous_service)
            .unwrap();

        let mut split_core = Ym2413Core::new();
        let mut split_service = Ym2413PgSerialService::new(initial_bank(0));
        split_core
            .advance_input_clocks_with_pg_service(4 * 3, &mut split_service)
            .unwrap();
        let core_state = split_core.export_state();
        let service_state = split_service.export_state();
        let mut restored_core = Ym2413Core::restore_state(core_state).unwrap();
        let mut restored_service = Ym2413PgSerialService::restore_state(service_state).unwrap();
        restored_core
            .advance_input_clocks_with_pg_service(4 * 8, &mut restored_service)
            .unwrap();

        assert_eq!(
            restored_service.output_frame(),
            continuous_service.output_frame()
        );
        assert_eq!(
            restored_service.export_state(),
            continuous_service.export_state()
        );
        assert_eq!(restored_core.export_state(), continuous_core.export_state());
    }

    #[test]
    fn unseeded_rhythm_slots_propagate_unknown_output_through_core_pipeline() {
        let mut registers = [0; 64];
        registers[0x0e] = 0x3f;
        let mut service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_cursor(11)
            .unwrap();

        let first = rhythm_context(11, registers, EnvelopeRhythmSelect::BassDrumFirst);
        service.before_eg_update(&first).unwrap();
        assert!(service.output_frame().unwrap().mo.is_some());
        assert!(
            !service
                .output_pipeline_state()
                .previous_channel_output_valid
        );
        assert_eq!(service.output_pipeline_state().rm_enable, -1);

        let second = rhythm_context(12, registers, EnvelopeRhythmSelect::HiHat);
        service.before_eg_update(&second).unwrap();
        assert!(service.output_frame().unwrap().mo.is_some());
        assert!(!service.output_pipeline_state().output_m_valid);
        assert!(!service.output_pipeline_state().output_r_valid);

        let third = rhythm_context(13, registers, EnvelopeRhythmSelect::TomTom);
        service.before_eg_update(&third).unwrap();
        let frame = service.output_frame().unwrap();
        assert_eq!(frame.mo, None);
        assert_eq!(frame.ro, None);
    }

    fn initial_bank(pm_lfo_counter: u16) -> Ym2413SynthesisBank {
        let initial = Ym2413SynthesisInitialState {
            channels: std::array::from_fn(|channel| Ym2413ChannelInitialState {
                phase: [0x1000 * channel as u32, 0x1800 * channel as u32],
                envelope_level: [127; 2],
                envelope_stage: [EnvelopeStage::Damp; 2],
                envelope_key_on: [false; 2],
                feedback_history: [0; 2],
                previous_modulator_phase_index: None,
            }),
            pm_lfo_counter,
            am_lfo_phase: 0,
            damp_config: DampTransitionConfig::new(
                124,
                DampTransitionCoordination::IndependentPerOperator,
                AttackRate15Transition::SkipAttackToDecay,
            )
            .unwrap(),
        };
        Ym2413SynthesisBank::with_initial_state(initial).unwrap()
    }

    fn context(cycle: u8, registers: [u8; 64], testmode: u8) -> Ym2413SlotServiceContext {
        let eg = EnvelopeSequencer::new();
        let state = eg.export_state();
        let event =
            nuked_slot_service_event_with_testmode(cycle, 0, state.eg_dokon, testmode).unwrap();
        let mut registers = registers;
        registers[0x0f] = testmode;
        Ym2413SlotServiceContext {
            event,
            registers,
            envelope_input: EnvelopeSlotInput::default(),
            envelope_state: state,
            rm_enable: 0,
            rm_select: 6,
        }
    }

    fn rhythm_context(
        cycle: u8,
        registers: [u8; 64],
        selector: EnvelopeRhythmSelect,
    ) -> Ym2413SlotServiceContext {
        let eg = EnvelopeSequencer::new();
        let state = eg.export_state();
        let event = nuked_slot_service_event_with_testmode(cycle, 0xff, state.eg_dokon, 0).unwrap();
        let mut registers = registers;
        registers[0x0f] = 0;
        let envelope_input = EnvelopeSlotInput {
            rhythm_select: selector,
            rhythm_bits: registers[0x0e] & 0x3f,
            ..EnvelopeSlotInput::default()
        };
        Ym2413SlotServiceContext {
            event,
            registers,
            envelope_input,
            envelope_state: state,
            rm_enable: 0xff,
            rm_select: match selector {
                EnvelopeRhythmSelect::BassDrumFirst => 0,
                EnvelopeRhythmSelect::HiHat => 1,
                EnvelopeRhythmSelect::TomTom => 2,
                EnvelopeRhythmSelect::BassDrumSecond => 3,
                EnvelopeRhythmSelect::SnareDrum => 4,
                EnvelopeRhythmSelect::TopCymbal => 5,
                EnvelopeRhythmSelect::None => 6,
            },
        }
    }

    fn rhythm_selector(cycle: u8) -> EnvelopeRhythmSelect {
        match cycle {
            11 => EnvelopeRhythmSelect::BassDrumFirst,
            12 => EnvelopeRhythmSelect::HiHat,
            13 => EnvelopeRhythmSelect::TomTom,
            14 => EnvelopeRhythmSelect::BassDrumSecond,
            15 => EnvelopeRhythmSelect::SnareDrum,
            16 => EnvelopeRhythmSelect::TopCymbal,
            _ => EnvelopeRhythmSelect::None,
        }
    }

    fn advance_rhythm_slot(service: &mut Ym2413PgSerialService, cycle: u8, registers: [u8; 64]) {
        let context = rhythm_context(cycle, registers, rhythm_selector(cycle));
        service.before_eg_update(&context).unwrap();
        let (tick, post_state) = tick(cycle);
        service.after_eg_update(&context, tick, post_state).unwrap();
    }

    fn tick(cycle: u8) -> (crate::EnvelopeSequencerTick, crate::EnvelopeSequencerState) {
        let mut eg = EnvelopeSequencer::new();
        for prior_cycle in 0..cycle {
            eg.tick_slot(prior_cycle, EnvelopeSlotInput::default())
                .unwrap();
        }
        let result = eg.tick_slot(cycle, EnvelopeSlotInput::default()).unwrap();
        (result, eg.export_state())
    }

    fn service_with_pipeline(
        bank: Ym2413SynthesisBank,
        pg_increment: u32,
        pg_phase_next: u32,
        parameters: Ym2413PgParameterLatch,
        lfo_vib_counter: u8,
    ) -> Ym2413PgSerialService {
        let mut service = Ym2413PgSerialService::new(bank);
        let mut state = service.export_state();
        state.pipeline = Ym2413PgSerialPipelineState {
            format_version: YM2413_PG_SERIAL_PIPELINE_STATE_VERSION,
            pg_increment,
            pg_phase_next,
            pg_output: 0,
            lfo_vib_counter,
            parameters,
            rhythm_phase_latches: Ym2413RhythmPhaseLatches::default(),
        };
        state.format_version = YM2413_PG_SERIAL_STATE_VERSION;
        service = Ym2413PgSerialService::restore_state(state).unwrap();
        service
    }

    #[test]
    fn rhythm_patch2_keeps_bd_modulator_fb_and_dm_out_of_the_bd_carrier_slot() {
        let mut registers = [0; 64];
        registers[0x0e] = 0x20;
        let cases = [
            (11, EnvelopeRhythmSelect::BassDrumFirst, 7, true),
            (12, EnvelopeRhythmSelect::HiHat, 0, false),
            (13, EnvelopeRhythmSelect::TomTom, 0, false),
            (14, EnvelopeRhythmSelect::BassDrumSecond, 0, false),
            (15, EnvelopeRhythmSelect::SnareDrum, 0, false),
            (16, EnvelopeRhythmSelect::TopCymbal, 0, false),
        ];

        for (cycle, selector, expected_feedback, expected_dm) in cases {
            let (voice, operator, _) =
                super::rhythm_voice_operator(&registers, selector, cycle).unwrap();
            let latches = super::rhythm_operator_patch2_latches(selector, voice, operator);
            assert_eq!(latches.feedback, expected_feedback, "cycle {cycle}");
            assert_eq!(latches.dm_waveform, expected_dm, "cycle {cycle}");
        }
    }

    #[test]
    fn seeded_rhythm_pg_operator_path_is_snapshot_and_block_partition_deterministic() {
        let mut registers = [0; 64];
        registers[0x0e] = 0x3f;
        registers[0x36] = 0xa5;
        registers[0x37] = 0xb6;
        registers[0x38] = 0xc7;

        let mut continuous = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(0x5a_1234, 0)
            .unwrap();
        for cycle in 0..18 {
            advance_rhythm_slot(&mut continuous, cycle, registers);
        }

        let final_state = continuous.export_state();
        assert_eq!(final_state.rhythm_noise_steps, 18);
        assert_eq!(final_state.rhythm_noise_cursor, 0);
        assert!(final_state.rhythm_noise_samples.iter().all(Option::is_some));
        let restored = Ym2413PgSerialService::restore_state(final_state).unwrap();
        assert_eq!(restored.export_state(), final_state);

        let mut partitioned = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(0x5a_1234, 0)
            .unwrap();
        for cycle in 0..9 {
            advance_rhythm_slot(&mut partitioned, cycle, registers);
        }
        let midpoint = partitioned.export_state();
        partitioned = Ym2413PgSerialService::restore_state(midpoint).unwrap();
        for cycle in 9..18 {
            advance_rhythm_slot(&mut partitioned, cycle, registers);
        }
        assert_eq!(partitioned.export_state(), final_state);
    }

    #[test]
    fn rhythm_pg_phase_outputs_match_nuked_hihat_snare_and_cymbal_equations() {
        let mut latches = super::Ym2413RhythmPhaseLatches {
            tc_bit3: true,
            tc_bit5: false,
            ..super::Ym2413RhythmPhaseLatches::default()
        };
        let hihat_phase = (1_u32 << 11) | (1_u32 << 17);
        assert_eq!(
            super::rhythm_pg_output(13, hihat_phase, 0xff, true, Some(false), &mut latches),
            0x2d0
        );
        assert!(latches.hh_bit2);
        assert!(latches.hh_bit8);
        assert!(!latches.hh_bit3);
        assert!(!latches.hh_bit7);

        assert_eq!(
            super::rhythm_pg_output(16, 0, 0xff, true, Some(true), &mut latches),
            0x200
        );

        let cymbal_phase = (1_u32 << 12) | (1_u32 << 14);
        assert_eq!(
            super::rhythm_pg_output(17, cymbal_phase, 0xff, true, None, &mut latches),
            0x300
        );
        assert!(latches.tc_bit3);
        assert!(latches.tc_bit5);
    }

    #[test]
    fn rhythm_pg_phase_output_is_not_fabricated_without_an_explicit_noise_state() {
        let mut latches = super::Ym2413RhythmPhaseLatches::default();
        let phase = 0x12_3456;
        assert_eq!(
            super::rhythm_pg_output(13, phase, 0xff, false, None, &mut latches),
            (phase >> 9) as u16
        );
        assert_eq!(
            super::rhythm_pg_output(17, phase, 0xff, false, None, &mut latches),
            (phase >> 9) as u16
        );
    }

    #[test]
    fn pg_output_latch_preserves_bits_above_the_ten_bit_sine_address() {
        let mut bank = initial_bank(0);
        let mut bank_state = bank.export_state();
        bank_state.channels[1].modulator_phase.phase = 0x0008_4000;
        bank = Ym2413SynthesisBank::restore_state(bank_state).unwrap();
        let mut service = service_with_pipeline(bank, 0, 0, Ym2413PgParameterLatch::RESET, 0);

        service.before_eg_update(&context(0, [0; 64], 0)).unwrap();
        assert_eq!(service.pg_output(), 0x0420);

        let snapshot = service.export_state();
        let restored = Ym2413PgSerialService::restore_state(snapshot).unwrap();
        assert_eq!(restored.pg_output(), 0x0420);
        assert_eq!(restored.export_state(), snapshot);
    }

    #[test]
    fn phase_write_and_phase_output_use_distinct_physical_slot_indices() {
        let mut service = service_with_pipeline(
            initial_bank(0),
            0x1000,
            0x2000,
            Ym2413PgParameterLatch::RESET,
            0,
        );
        let context = context(0, [0; 64], 0);
        service.before_eg_update(&context).unwrap();

        let written = nuked_pg_phase_slot_descriptor(17).unwrap();
        let output = nuked_pg_phase_slot_descriptor(0).unwrap();
        assert_eq!(
            service
                .synthesis()
                .phase_accumulator(
                    written.channel,
                    matches!(written.operator, crate::Ym2413Operator::Carrier),
                )
                .unwrap(),
            0x3000
        );
        assert_eq!(
            service
                .synthesis()
                .phase_accumulator(
                    output.channel,
                    matches!(output.operator, crate::Ym2413Operator::Carrier),
                )
                .unwrap(),
            0x1000
        );
        assert_eq!(service.pg_phase_next(), 0x1000);
        assert_eq!(service.pg_output(), 8);
    }

    #[test]
    fn key_on_reset_changes_only_the_delayed_next_phase_latch() {
        let mut service = service_with_pipeline(
            initial_bank(0),
            0x0200,
            0x1000,
            Ym2413PgParameterLatch::RESET,
            0,
        );
        let context = context(0, [0; 64], 0x04);
        service.before_eg_update(&context).unwrap();

        let output = nuked_pg_phase_slot_descriptor(0).unwrap();
        let written = nuked_pg_phase_slot_descriptor(17).unwrap();
        assert_eq!(
            service
                .synthesis()
                .phase_accumulator(
                    output.channel,
                    matches!(output.operator, crate::Ym2413Operator::Carrier),
                )
                .unwrap(),
            0x1000
        );
        assert_eq!(
            service
                .synthesis()
                .phase_accumulator(
                    written.channel,
                    matches!(written.operator, crate::Ym2413Operator::Carrier),
                )
                .unwrap(),
            0x1200
        );
        assert_eq!(service.pg_phase_next(), 0);
    }

    #[test]
    fn phase_increment_uses_previous_parameter_latch_before_latching_next_slot() {
        let mut registers = [0; 64];
        registers[..8].copy_from_slice(&[0x41, 0x02, 0, 0, 0, 0, 0, 0]);
        registers[0x11] = 0xc0;
        registers[0x21] = 0x0d;
        registers[0x31] = 0x00;

        let parameters = Ym2413PgParameterLatch {
            fnum: 0x1c0,
            block: 6,
            multiplier: 2,
            pm_enabled: true,
        };
        let mut service = service_with_pipeline(initial_bank(0), 0, 0, parameters, 3);
        let context = context(0, registers, 0);
        service.before_eg_update(&context).unwrap();
        let (tick, post) = tick(0);
        service.after_eg_update(&context, tick, post).unwrap();

        assert_eq!(
            service.pg_increment(),
            PhaseGenerator::phase_step(0x1c0, 6, 2, true, 3).unwrap()
        );
        assert_eq!(
            service.pipeline.parameters,
            Ym2413PgParameterLatch {
                fnum: 0x1c0,
                block: 6,
                multiplier: 1,
                pm_enabled: true,
            }
        );
    }

    #[test]
    fn cycle_seventeen_advances_vibrato_and_pm_latches_after_increment() {
        let mut service =
            service_with_pipeline(initial_bank(0x03ff), 0, 0, Ym2413PgParameterLatch::RESET, 2)
                .with_rhythm_noise_cursor(17)
                .unwrap();
        let context = context(17, [0; 64], 0x08);
        service.before_eg_update(&context).unwrap();
        let (tick, post) = tick(17);
        service.after_eg_update(&context, tick, post).unwrap();
        assert_eq!(service.lfo_vib_counter(), 3);
        assert_eq!(service.synthesis().pm_lfo_counter(), 0x0400);
    }

    #[test]
    fn test_mode_lfo_reset_is_applied_on_each_serial_cycle() {
        let mut service =
            service_with_pipeline(initial_bank(0x0555), 0, 0, Ym2413PgParameterLatch::RESET, 6)
                .with_rhythm_noise_cursor(3)
                .unwrap();
        let context = context(3, [0; 64], 0x02);
        service.before_eg_update(&context).unwrap();
        let (tick, post) = tick(3);
        service.after_eg_update(&context, tick, post).unwrap();
        assert_eq!(service.lfo_vib_counter(), 0);
        assert_eq!(service.synthesis().pm_lfo_counter(), 0);
    }

    #[test]
    fn test_mode_lfo_reset_occurs_after_cycle_seventeen_update() {
        let mut service =
            service_with_pipeline(initial_bank(0x03ff), 0, 0, Ym2413PgParameterLatch::RESET, 6)
                .with_rhythm_noise_cursor(17)
                .unwrap();
        let context = context(17, [0; 64], 0x0a);
        service.before_eg_update(&context).unwrap();
        let (tick, post) = tick(17);
        service.after_eg_update(&context, tick, post).unwrap();
        assert_eq!(service.lfo_vib_counter(), 0);
        assert_eq!(service.synthesis().pm_lfo_counter(), 0);
    }

    #[test]
    fn rhythm_mode_keeps_unaffected_melodic_slots_on_the_normal_serial_path() {
        for cycle in (0..18_u8).filter(|cycle| !(11..=16).contains(cycle)) {
            let mut service = Ym2413PgSerialService::new(initial_bank(0))
                .with_rhythm_noise_cursor(cycle)
                .unwrap();
            let before = service.export_state();
            let mut registers = [0; 64];
            registers[0x0e] = 0x20;
            let mut context = context(cycle, registers, 0);
            context.rm_enable = 0xff;
            context.rm_select = 6;
            context.envelope_input.rhythm_select = EnvelopeRhythmSelect::None;

            service.before_eg_update(&context).unwrap();
            let (tick, post) = tick(cycle);
            service.after_eg_update(&context, tick, post).unwrap();
            assert_ne!(
                service.export_state(),
                before,
                "melodic cycle {cycle} did not advance in Rhythm Mode"
            );
        }
    }

    #[test]
    fn rhythm_owned_slots_use_seeded_rhythm_operator_path_and_reach_do_rhythm() {
        let rhythm_cycles = [
            (11, EnvelopeRhythmSelect::BassDrumFirst),
            (12, EnvelopeRhythmSelect::HiHat),
            (13, EnvelopeRhythmSelect::TomTom),
            (14, EnvelopeRhythmSelect::BassDrumSecond),
            (15, EnvelopeRhythmSelect::SnareDrum),
            (16, EnvelopeRhythmSelect::TopCymbal),
        ];

        for (cycle, selector) in rhythm_cycles {
            let mut service = Ym2413PgSerialService::new(initial_bank(0))
                .with_rhythm_noise_seed(0x5a_1234, cycle)
                .unwrap();
            let synthesis_before = service.synthesis().export_state();
            let operator_before = service.operator.export_state();
            let mut registers = [0; 64];
            registers[0x0e] = 0x20;
            let mut slot_context = context(cycle, registers, 0);
            slot_context.rm_enable = 0xff;
            slot_context.rm_select = selector as u8;
            slot_context.envelope_input.rhythm_select = selector;
            slot_context.event = nuked_slot_service_event_with_testmode(
                cycle,
                0xff,
                slot_context.envelope_state.eg_dokon,
                0,
            )
            .unwrap();

            service.before_eg_update(&slot_context).unwrap();
            assert_ne!(service.synthesis().export_state(), synthesis_before);
            assert_ne!(service.operator.export_state(), operator_before);
            let (tick, post) = tick(cycle);
            service.after_eg_update(&slot_context, tick, post).unwrap();
            assert_eq!(
                service.rhythm_noise_state().unwrap().slot_index,
                (cycle + 1) % 18
            );
        }
    }

    #[test]
    fn rhythm_mode_and_selector_transitions_do_not_reassign_melodic_latches() {
        let cases = [
            (true, 0x80, EnvelopeRhythmSelect::HiHat),
            (false, 0x7f, EnvelopeRhythmSelect::None),
            (true, 0xff, EnvelopeRhythmSelect::None),
        ];

        for (rhythm_enabled, rm_enable, selector) in cases {
            let mut service = Ym2413PgSerialService::new(initial_bank(0))
                .with_rhythm_noise_seed(0x5a_1234, 12)
                .unwrap();
            let synthesis_before = service.synthesis().export_state();
            let operator_before = service.operator.export_state();
            let mut registers = [0; 64];
            registers[0x0e] = if rhythm_enabled { 0x20 } else { 0 };
            let mut slot_context = context(12, registers, 0);
            slot_context.rm_enable = rm_enable;
            slot_context.rm_select = selector as u8;
            slot_context.envelope_input.rhythm_select = selector;

            service.before_eg_update(&slot_context).unwrap();
            assert_eq!(service.synthesis().export_state(), synthesis_before);
            assert_eq!(service.operator.export_state(), operator_before);
            let (tick, post) = tick(12);
            service.after_eg_update(&slot_context, tick, post).unwrap();
            assert_eq!(service.rhythm_noise_state().unwrap().slot_index, 13);
        }
    }

    #[test]
    fn service_clone_is_independent_and_snapshot_restores_every_pg_latch() {
        let service = service_with_pipeline(
            initial_bank(1234),
            0x12345,
            0x23456,
            Ym2413PgParameterLatch {
                fnum: 0x1c0,
                block: 6,
                multiplier: 2,
                pm_enabled: true,
            },
            5,
        );
        let snapshot = service.export_state();
        let restored = Ym2413PgSerialService::restore_state(snapshot).unwrap();
        assert_eq!(restored.export_state(), snapshot);

        let mut clone = service.clone();
        clone.before_eg_update(&context(0, [0; 64], 0)).unwrap();
        assert_eq!(service.export_state(), snapshot);
        assert_ne!(clone.export_state(), snapshot);
    }

    #[test]
    fn service_rejects_invalid_pipeline_snapshot_fields() {
        let service = Ym2413PgSerialService::new(initial_bank(0));
        let mut state = service.export_state();
        state.pipeline.lfo_vib_counter = 8;
        assert_eq!(
            Ym2413PgSerialService::restore_state(state),
            Err(Ym2413PgSerialError::InvalidState)
        );
        state = service.export_state();
        state.pipeline.parameters.fnum = 0x0200;
        assert_eq!(
            Ym2413PgSerialService::restore_state(state),
            Err(Ym2413PgSerialError::InvalidState)
        );
    }

    #[test]
    fn restore_state_enforces_the_reachable_pg_increment_bound() {
        let service = Ym2413PgSerialService::new(initial_bank(0));
        let mut state = service.export_state();
        state.pipeline.pg_increment = super::YM2413_PG_INCREMENT_MAX + 1;
        assert_eq!(
            Ym2413PgSerialService::restore_state(state),
            Err(Ym2413PgSerialError::InvalidState)
        );

        let mut state = service.export_state();
        state.pipeline.pg_increment = super::YM2413_PG_INCREMENT_MAX;
        assert!(Ym2413PgSerialService::restore_state(state).is_ok());
    }

    #[test]
    fn maximum_pg_increment_is_reachable_from_legal_parameters() {
        // FNUM=511 selects PM row 7; position 2 contributes +7.
        // ((511 * 2 + 7) * 2^7 / 2) * 30 / 2 = 987_840.
        assert_eq!(
            PhaseGenerator::phase_step(0x01ff, 7, 14, true, 2),
            Ok(super::YM2413_PG_INCREMENT_MAX)
        );
        assert_eq!(
            PhaseGenerator::phase_step(0x01ff, 7, 15, true, 2),
            Ok(super::YM2413_PG_INCREMENT_MAX)
        );
    }

    #[test]
    fn unseeded_do_rhythm_cadence_advances_symbolically_without_fabricating_noise_bits() {
        let mut service = Ym2413PgSerialService::new(initial_bank(0));
        let mut core = Ym2413Core::new();
        core.advance_input_clocks_with_slot_service(72, &mut service)
            .unwrap();
        assert_eq!(service.rhythm_noise_state(), None);
        assert_eq!(service.rhythm_noise_cursor(), 0);
        assert_eq!(service.rhythm_noise_steps(), 18);
        assert_eq!(service.rhythm_noise_samples(), [None, None]);
    }

    #[test]
    fn seeded_rhythm_noise_advances_once_on_all_eighteen_slots_in_melodic_mode() {
        use crate::rhythm::Ym2413RhythmSlotNoiseSequencer;

        let seed = 0x5a_1234;
        let mut service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(seed, 0)
            .unwrap();
        let mut core = Ym2413Core::new();
        core.advance_input_clocks_with_slot_service(72, &mut service)
            .unwrap();

        let mut expected = Ym2413RhythmSlotNoiseSequencer::with_seed_at_slot(seed, 0).unwrap();
        let mut expected_samples = [None, None];
        for _ in 0..18 {
            if let Some(sample) = expected.advance_slot() {
                match sample.voice {
                    crate::Ym2413RhythmVoice::HiHat => expected_samples[0] = Some(sample.noise_bit),
                    crate::Ym2413RhythmVoice::SnareDrum => {
                        expected_samples[1] = Some(sample.noise_bit)
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert_eq!(service.rhythm_noise_state(), Some(expected.export_state()));
        assert_eq!(service.rhythm_noise_samples(), expected_samples);
        assert_eq!(service.rhythm_noise_state().unwrap().slot_index, 0);
    }

    #[test]
    fn do_rhythm_uses_test_mode_bit_one_and_preserves_zero_register_candidate() {
        let zero_service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_register(0, 0)
            .unwrap();
        let zero_snapshot = zero_service.export_state();
        assert_eq!(
            Ym2413PgSerialService::restore_state(zero_snapshot)
                .unwrap()
                .export_state(),
            zero_snapshot
        );

        let register = 0x12_3456;
        let mut service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_register(register, 0)
            .unwrap();
        let slot_context = context(0, [0; 64], 0x02);
        service.before_eg_update(&slot_context).unwrap();
        let (tick, post) = tick(0);
        service.after_eg_update(&slot_context, tick, post).unwrap();
        assert_eq!(
            service.rhythm_noise_state().unwrap().lfsr.register,
            (register >> 1) | (1 << 22)
        );
    }

    #[test]
    fn seeded_rhythm_noise_snapshot_restore_and_block_partitioning_are_equivalent() {
        let seed = 0x31_4567;
        let mut whole_core = Ym2413Core::new();
        let mut whole_service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(seed, 0)
            .unwrap();
        whole_core
            .advance_input_clocks_with_slot_service(18 * 72, &mut whole_service)
            .unwrap();

        let mut split_core = Ym2413Core::new();
        let mut split_service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(seed, 0)
            .unwrap();
        for clocks in [
            1, 3, 4, 17, 29, 7, 11, 53, 72, 91, 8, 101, 17, 29, 37, 61, 23, 103, 201, 417,
        ] {
            split_core
                .advance_input_clocks_with_slot_service(clocks, &mut split_service)
                .unwrap();
        }
        let elapsed = split_core.elapsed_input_clocks();
        let total = 18 * 72;
        if elapsed < total {
            split_core
                .advance_input_clocks_with_slot_service(total - elapsed, &mut split_service)
                .unwrap();
        }
        assert_eq!(whole_core.export_state(), split_core.export_state());
        assert_eq!(whole_service.export_state(), split_service.export_state());

        let mut checkpoint_core = Ym2413Core::new();
        let mut checkpoint_service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(seed, 0)
            .unwrap();
        checkpoint_core
            .advance_input_clocks_with_slot_service(4 * 13 + 2, &mut checkpoint_service)
            .unwrap();
        let mut restored_core = Ym2413Core::restore_state(checkpoint_core.export_state()).unwrap();
        let mut restored_service =
            Ym2413PgSerialService::restore_state(checkpoint_service.export_state()).unwrap();
        let remaining = 18 * 72 - checkpoint_core.elapsed_input_clocks();
        restored_core
            .advance_input_clocks_with_slot_service(remaining, &mut restored_service)
            .unwrap();
        let mut reference_core = Ym2413Core::new();
        let mut reference_service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(seed, 0)
            .unwrap();
        reference_core
            .advance_input_clocks_with_slot_service(18 * 72, &mut reference_service)
            .unwrap();
        assert_eq!(reference_core.export_state(), restored_core.export_state());
        assert_eq!(
            reference_service.export_state(),
            restored_service.export_state()
        );
    }

    #[test]
    fn rhythm_noise_slot_cursor_mismatch_rejects_the_staged_service_transaction() {
        let mut service = Ym2413PgSerialService::new(initial_bank(0))
            .with_rhythm_noise_seed(0x5a_1234, 1)
            .unwrap();
        let before = service.export_state();
        let mut core = Ym2413Core::new();
        assert!(matches!(
            core.advance_input_clocks_with_slot_service(4, &mut service),
            Err(Ym2413SlotServiceAdvanceError::Service(
                Ym2413PgSerialError::InvalidState
            ))
        ));
        assert_eq!(service.export_state(), before);
        assert_eq!(core.slot_index(), 0);
    }

    #[derive(Clone)]
    struct FailingPgService {
        inner: Ym2413PgSerialService,
        fail_cycle: Option<u8>,
    }

    impl Ym2413SlotService for FailingPgService {
        type Error = u8;

        fn before_eg_update(
            &mut self,
            context: &Ym2413SlotServiceContext,
        ) -> Result<(), Self::Error> {
            self.inner
                .before_eg_update(context)
                .map_err(|_| context.event.cycle)
        }

        fn after_eg_update(
            &mut self,
            context: &Ym2413SlotServiceContext,
            tick: EnvelopeSequencerTick,
            post_state: EnvelopeSequencerState,
        ) -> Result<(), Self::Error> {
            self.inner
                .after_eg_update(context, tick, post_state)
                .map_err(|_| context.event.cycle)?;
            if self.fail_cycle == Some(context.event.cycle) {
                Err(context.event.cycle)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn core_and_pg_service_snapshot_restore_and_transactional_retry_are_equivalent() {
        let initial = initial_bank(0);
        let mut whole_core = Ym2413Core::new();
        let mut whole_service = Ym2413PgSerialService::new(initial.clone());
        let total_clocks = 18 * YM2413_INPUT_CLOCKS_PER_SLOT;
        whole_core
            .advance_input_clocks_with_slot_service(total_clocks, &mut whole_service)
            .unwrap();

        let mut checkpoint_core = Ym2413Core::new();
        let mut checkpoint_service = Ym2413PgSerialService::new(initial.clone());
        checkpoint_core
            .advance_input_clocks_with_slot_service(37, &mut checkpoint_service)
            .unwrap();
        let mut restored_core = Ym2413Core::restore_state(checkpoint_core.export_state()).unwrap();
        let mut restored_service =
            Ym2413PgSerialService::restore_state(checkpoint_service.export_state()).unwrap();
        restored_core
            .advance_input_clocks_with_slot_service(total_clocks - 37, &mut restored_service)
            .unwrap();
        assert_eq!(whole_core.export_state(), restored_core.export_state());
        assert_eq!(
            whole_service.export_state(),
            restored_service.export_state()
        );

        let mut rollback_core = Ym2413Core::new();
        let mut rollback_service = FailingPgService {
            inner: Ym2413PgSerialService::new(initial.clone()),
            fail_cycle: Some(3),
        };
        let core_before = rollback_core.export_state();
        let service_before = rollback_service.inner.export_state();
        assert_eq!(
            rollback_core.advance_input_clocks_with_slot_service(
                4 * YM2413_INPUT_CLOCKS_PER_SLOT,
                &mut rollback_service,
            ),
            Err(Ym2413SlotServiceAdvanceError::Service(3))
        );
        assert_eq!(rollback_core.export_state(), core_before);
        assert_eq!(rollback_service.inner.export_state(), service_before);

        rollback_service.fail_cycle = None;
        rollback_core
            .advance_input_clocks_with_slot_service(
                4 * YM2413_INPUT_CLOCKS_PER_SLOT,
                &mut rollback_service,
            )
            .unwrap();

        let mut reference_core = Ym2413Core::new();
        let mut reference_service = Ym2413PgSerialService::new(initial);
        reference_core
            .advance_input_clocks_with_slot_service(
                4 * YM2413_INPUT_CLOCKS_PER_SLOT,
                &mut reference_service,
            )
            .unwrap();
        assert_eq!(rollback_core.export_state(), reference_core.export_state());
        assert_eq!(
            rollback_service.inner.export_state(),
            reference_service.export_state()
        );
    }

    #[test]
    fn core_and_serial_pg_service_are_equivalent_across_block_partitions() {
        let initial = initial_bank(0);
        let mut whole_core = Ym2413Core::new();
        let mut split_core = Ym2413Core::new();
        let mut whole = Ym2413PgSerialService::new(initial.clone());
        let mut split = Ym2413PgSerialService::new(initial);

        whole_core
            .advance_input_clocks_with_slot_service(18 * YM2413_INPUT_CLOCKS_PER_SLOT, &mut whole)
            .unwrap();
        for clocks in [1, 3, 4, 17, 29, 7, 11] {
            split_core
                .advance_input_clocks_with_slot_service(clocks, &mut split)
                .unwrap();
        }
        let elapsed = split_core.elapsed_input_clocks();
        let total = 18 * YM2413_INPUT_CLOCKS_PER_SLOT;
        if elapsed < total {
            split_core
                .advance_input_clocks_with_slot_service(total - elapsed, &mut split)
                .unwrap();
        }

        assert_eq!(whole_core.export_state(), split_core.export_state());
        assert_eq!(whole.export_state(), split.export_state());
    }
}
