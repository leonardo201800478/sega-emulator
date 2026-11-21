//! Cycle-ordered MO/RO output pipeline for the original YM2413 branch of
//! Nuked-OPLL commit 1269cf5a783b65583b50fa2464d08be75830aaa0.
//!
//! This is a software-reference model, not a raw physical DAC bus, pin-voltage
//! model, or proof of silicon timing. The supplied next_channel_output is the
//! operator-stage result latched after the channel-output stage.

/// Version of the serial MO/RO output-pipeline snapshot format.
pub const YM2413_OUTPUT_PIPELINE_STATE_VERSION: u16 = 2;
/// Lowest valid one's-complement operator channel-output latch value.
pub const YM2413_CHANNEL_OUTPUT_MIN: i16 = -256;
/// Highest valid one's-complement operator channel-output latch value.
pub const YM2413_CHANNEL_OUTPUT_MAX: i16 = 255;

/// Exact signed integer emitted by the pinned software reference.
/// Distinct from Ym2413DacCode, which represents measured amplitudes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ym2413NukedOutputCode(i16);

impl Ym2413NukedOutputCode {
    /// Wrap an exact signed integer from the Nuked software output buffer.
    pub const fn new(value: i16) -> Self {
        Self(value)
    }
    /// Return the wrapped signed software-reference output integer.
    pub const fn value(self) -> i16 {
        self.0
    }
}

/// Values copied to the external buffer at the beginning of one clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413OutputPipelineFrame {
    /// Serial cycle whose output-buffer values were copied.
    pub cycle: u8,
    /// MO output-buffer value copied at the start of this clock.
    pub mo: Ym2413NukedOutputCode,
    /// RO output-buffer value copied at the start of this clock.
    pub ro: Ym2413NukedOutputCode,
}

/// Core-integrated frame. None means the channel/operator pipeline has not
/// supplied a validated value; unknown is never converted to digital silence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413CoreOutputPipelineFrame {
    /// Serial cycle whose output-buffer values were copied.
    pub cycle: u8,
    /// MO value copied at the beginning of this clock, if known.
    pub mo: Option<Ym2413NukedOutputCode>,
    /// RO value copied at the beginning of this clock, if known.
    pub ro: Option<Ym2413NukedOutputCode>,
}

/// Complete serial pipeline state, including both latches and rm_enable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413OutputPipelineState {
    /// Snapshot schema version.
    pub format_version: u16,
    /// Serial cycle required by the next process_cycle call.
    pub next_cycle: u8,
    /// Previous operator result consumed by the next channel stage.
    pub previous_channel_output: i16,
    /// Whether the previous operator result is validated/known.
    pub previous_channel_output_valid: bool,
    /// Channel-stage MO value copied to the external buffer next clock.
    pub output_m: i16,
    /// Whether the MO output latch is validated/known.
    pub output_m_valid: bool,
    /// Channel-stage RO value copied to the external buffer next clock.
    pub output_r: i16,
    /// Whether the RO output latch is validated/known.
    pub output_r_valid: bool,
    /// Signed Nuked int8_t mode-write shift register; right shifts are arithmetic.
    pub rm_enable: i8,
}

/// Validation failures from the serial output pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ym2413OutputPipelineError {
    /// Snapshot version, cycle cursor or latch range is invalid.
    InvalidState,
    /// Caller supplied a cycle other than the state cursor.
    InvalidCycle { expected: u8, actual: u8 },
    /// Operator channel output is outside the nine-bit one's-complement range.
    InvalidChannelOutput(i16),
    /// The standalone non-optional API cannot represent an unknown output.
    OutputUnknown,
}

impl core::fmt::Display for Ym2413OutputPipelineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState => f.write_str("invalid YM2413 output-pipeline state"),
            Self::InvalidCycle { expected, actual } => write!(
                f,
                "unexpected YM2413 output-pipeline cycle: expected {expected}, got {actual}"
            ),
            Self::InvalidChannelOutput(value) => {
                write!(f, "invalid YM2413 one's-complement channel output: {value}")
            }
            Self::OutputUnknown => f.write_str("YM2413 output is not known yet"),
        }
    }
}
impl std::error::Error for Ym2413OutputPipelineError {}

/// Models Nuked's OPLL_Clock output order:
/// copy old output_m/output_r; shift rm_enable; apply a qualified mode write;
/// run OPLL_Channel using old ch_out; run OPLL_Operator and latch next ch_out.
///
/// A supplied mode-write value is already qualified for OPLL_DoModeWrite
/// (pending address 0x0e and write_d_en asserted). Bus qualification itself
/// remains the responsibility of the timed register-bus implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ym2413OutputPipeline {
    state: Ym2413OutputPipelineState,
}

impl Default for Ym2413OutputPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Ym2413OutputPipeline {
    /// Zero initialization matching Nuked's software reset, not a claim about
    /// the physical chip's power-on latch contents.
    pub const fn new() -> Self {
        Self {
            state: Ym2413OutputPipelineState {
                format_version: YM2413_OUTPUT_PIPELINE_STATE_VERSION,
                next_cycle: 0,
                previous_channel_output: 0,
                previous_channel_output_valid: true,
                output_m: 0,
                output_m_valid: true,
                output_r: 0,
                output_r_valid: true,
                rm_enable: 0,
            },
        }
    }

    /// Construct a production pipeline with unknown output latches.
    ///
    /// This is used when the upstream serial operator intentionally suppresses
    /// an update because its source state (for example, an unseeded rhythm LFSR)
    /// is unknown. It does not substitute zero or the previous sample.
    pub const fn new_unknown() -> Self {
        Self {
            state: Ym2413OutputPipelineState {
                format_version: YM2413_OUTPUT_PIPELINE_STATE_VERSION,
                next_cycle: 0,
                previous_channel_output: 0,
                previous_channel_output_valid: false,
                output_m: 0,
                output_m_valid: false,
                output_r: 0,
                output_r_valid: false,
                rm_enable: 0,
            },
        }
    }

    /// Align the next serial cycle when constructing a service for a restored core.
    pub(crate) const fn with_next_cycle(
        mut self,
        next_cycle: u8,
    ) -> Result<Self, Ym2413OutputPipelineError> {
        if next_cycle >= 18 {
            return Err(Ym2413OutputPipelineError::InvalidState);
        }
        self.state.next_cycle = next_cycle;
        Ok(self)
    }

    /// Restore a validated snapshot, preserving both latches and signed rm_enable.
    pub const fn with_initial_state(
        state: Ym2413OutputPipelineState,
    ) -> Result<Self, Ym2413OutputPipelineError> {
        if state.format_version != YM2413_OUTPUT_PIPELINE_STATE_VERSION
            || state.next_cycle >= 18
            || state.previous_channel_output < YM2413_CHANNEL_OUTPUT_MIN
            || state.previous_channel_output > YM2413_CHANNEL_OUTPUT_MAX
            || state.output_m < -256
            || state.output_m > 256
            || state.output_r < -256
            || state.output_r > 256
        {
            return Err(Ym2413OutputPipelineError::InvalidState);
        }
        Ok(Self { state })
    }

    /// Return a complete copy of the state required for deterministic restoration.
    pub const fn export_state(&self) -> Ym2413OutputPipelineState {
        self.state
    }

    /// Process one clock. next_channel_output is the operator result to become
    /// the next ch_out latch (-256..=255). The returned frame is the buffer
    /// value from before this clock's channel stage; newly calculated values
    /// are visible on the next call.
    ///
    /// qualified_rhythm_write_data is supplied only at the exact
    /// OPLL_DoModeWrite phase when the upstream bus sequencer has qualified a
    /// pending 0x0e write. Bit 5 of the value loads rm_enable bit 7 after the
    /// per-clock shift, exactly as the pinned source does.
    pub fn process_cycle(
        &mut self,
        cycle: u8,
        next_channel_output: i16,
        qualified_rhythm_write_data: Option<u8>,
    ) -> Result<Ym2413OutputPipelineFrame, Ym2413OutputPipelineError> {
        if cycle != self.state.next_cycle {
            return Err(Ym2413OutputPipelineError::InvalidCycle {
                expected: self.state.next_cycle,
                actual: cycle,
            });
        }
        if !(YM2413_CHANNEL_OUTPUT_MIN..=YM2413_CHANNEL_OUTPUT_MAX).contains(&next_channel_output) {
            return Err(Ym2413OutputPipelineError::InvalidChannelOutput(
                next_channel_output,
            ));
        }
        if !self.state.output_m_valid || !self.state.output_r_valid {
            return Err(Ym2413OutputPipelineError::OutputUnknown);
        }

        let mut rm_enable = self.state.rm_enable >> 1;
        if let Some(data) = qualified_rhythm_write_data {
            rm_enable = ((rm_enable as u8 & 0x7f) | ((data & 0x20) << 2)) as i8;
        }
        let frame =
            self.process_cycle_with_core_state(cycle, rm_enable, Some(next_channel_output))?;
        Ok(Ym2413OutputPipelineFrame {
            cycle,
            mo: frame.mo.ok_or(Ym2413OutputPipelineError::OutputUnknown)?,
            ro: frame.ro.ok_or(Ym2413OutputPipelineError::OutputUnknown)?,
        })
    }

    /// Process one serial cycle using the core-owned post-shift rm_enable.
    ///
    /// Unlike process_cycle, this entry point does not shift rm_enable or
    /// apply a mode write: Ym2413Core has already performed both operations in
    /// begin_slot. The optional operator result is latched for the next cycle.
    /// An unknown prior ch_out makes the newly calculated MO/RO latches
    /// unknown; it is not replaced with zero or held as a stale value.
    pub fn process_cycle_with_core_state(
        &mut self,
        cycle: u8,
        post_update_rm_enable: i8,
        next_channel_output: Option<i16>,
    ) -> Result<Ym2413CoreOutputPipelineFrame, Ym2413OutputPipelineError> {
        if cycle != self.state.next_cycle {
            return Err(Ym2413OutputPipelineError::InvalidCycle {
                expected: self.state.next_cycle,
                actual: cycle,
            });
        }
        if let Some(value) = next_channel_output
            && !(YM2413_CHANNEL_OUTPUT_MIN..=YM2413_CHANNEL_OUTPUT_MAX).contains(&value)
        {
            return Err(Ym2413OutputPipelineError::InvalidChannelOutput(value));
        }

        let frame = Ym2413CoreOutputPipelineFrame {
            cycle,
            mo: self
                .state
                .output_m_valid
                .then(|| Ym2413NukedOutputCode::new(self.state.output_m)),
            ro: self
                .state
                .output_r_valid
                .then(|| Ym2413NukedOutputCode::new(self.state.output_r)),
        };

        self.state.rm_enable = post_update_rm_enable;
        if self.state.previous_channel_output_valid {
            let mut active = self.state.previous_channel_output;
            let mut neutral = active >> 8;
            if active >= 0 {
                active += 1;
                neutral += 1;
            }

            let rhythm_pipeline_active = post_update_rm_enable & 0x40 != 0;
            let mute_m =
                (cycle / 3) & 1 != 0 || (rhythm_pipeline_active && (cycle + 15) % 18 >= 12);
            let mute_r = !rhythm_pipeline_active
                || !matches!(cycle, 16 | 17 | 0 | 1 | 2 | 3 | 4 | 5 | 9 | 10);

            self.state.output_m = if mute_m { neutral } else { active };
            self.state.output_r = if mute_r { neutral } else { active };
            self.state.output_m_valid = true;
            self.state.output_r_valid = true;
        } else {
            self.state.output_m_valid = false;
            self.state.output_r_valid = false;
        }

        if let Some(value) = next_channel_output {
            self.state.previous_channel_output = value;
            self.state.previous_channel_output_valid = true;
        } else {
            self.state.previous_channel_output_valid = false;
        }
        self.state.next_cycle = (cycle + 1) % 18;
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        YM2413_OUTPUT_PIPELINE_STATE_VERSION, Ym2413NukedOutputCode, Ym2413OutputPipeline,
        Ym2413OutputPipelineError, Ym2413OutputPipelineState,
    };

    fn tick(p: &mut Ym2413OutputPipeline, c: u8, n: i16, w: Option<u8>) -> (i16, i16) {
        let f = p.process_cycle(c, n, w).unwrap();
        (f.mo.value(), f.ro.value())
    }

    #[test]
    fn output_buffer_is_one_clock_after_channel_stage() {
        let mut p = Ym2413OutputPipeline::new();
        assert_eq!(tick(&mut p, 0, 7, None), (0, 0));
        assert_eq!(tick(&mut p, 1, 0, None), (1, 1));
        assert_eq!(tick(&mut p, 2, 0, None), (8, 1));
    }

    #[test]
    fn channel_stage_consumes_previous_ch_out_not_current_operator_result() {
        let mut p = Ym2413OutputPipeline::new();
        assert_eq!(
            p.process_cycle(0, 19, None).unwrap().mo,
            Ym2413NukedOutputCode::new(0)
        );
        assert_eq!(
            p.process_cycle(1, -11, None).unwrap().mo,
            Ym2413NukedOutputCode::new(1)
        );
        assert_eq!(
            p.process_cycle(2, 0, None).unwrap().mo,
            Ym2413NukedOutputCode::new(20)
        );
    }

    #[test]
    fn positive_negative_and_signed_zero_neutral_codes_match_reference_rule() {
        let mut p = Ym2413OutputPipeline::new();
        assert_eq!(tick(&mut p, 0, 9, None), (0, 0));
        assert_eq!(tick(&mut p, 1, -9, None), (1, 1));
        assert_eq!(tick(&mut p, 2, 0, None), (10, 1));
        assert_eq!(tick(&mut p, 3, -1, None), (-9, -1));
        assert_eq!(tick(&mut p, 4, 0, None), (1, 1));
        assert_eq!(tick(&mut p, 5, 0, None), (-1, -1));

        let mut positive_zero = Ym2413OutputPipeline::new();
        tick(&mut positive_zero, 0, 0, None);
        assert_eq!(tick(&mut positive_zero, 1, 0, None).0, 1);

        let mut negative_zero = Ym2413OutputPipeline::new();
        tick(&mut negative_zero, 0, -1, None);
        assert_eq!(tick(&mut negative_zero, 1, 0, None).0, 1);
        assert_eq!(tick(&mut negative_zero, 2, 0, None).0, -1);
    }

    #[test]
    fn register_0e_write_sign_extends_through_nuked_signed_rm_enable() {
        let mut p = Ym2413OutputPipeline::new();
        assert_eq!(tick(&mut p, 0, 12, Some(0x20)), (0, 0));
        assert_eq!(p.export_state().rm_enable as u8, 0x80);
        assert_eq!(tick(&mut p, 1, 0, None), (1, 1));
        assert_eq!(p.export_state().rm_enable as u8, 0xc0);
        assert_eq!(tick(&mut p, 2, 0, None), (1, 13));
        assert_eq!(p.export_state().rm_enable as u8, 0xe0);
        assert_eq!(tick(&mut p, 3, 0, None), (1, 1));
        assert_eq!(p.export_state().rm_enable as u8, 0xf0);
        for cycle in 4..10 {
            tick(&mut p, cycle, 0, None);
        }
        assert_eq!(p.export_state().rm_enable, -1);
    }

    #[test]
    fn disabling_rhythm_clears_sign_extension_after_one_final_bit_six_cycle() {
        let mut p = Ym2413OutputPipeline::new();
        tick(&mut p, 0, 0, Some(0x20));
        tick(&mut p, 1, 0, Some(0x00));
        assert_eq!(p.export_state().rm_enable, 0x40);
        tick(&mut p, 2, 0, None);
        assert_eq!(p.export_state().rm_enable, 0x20);
    }

    #[test]
    fn ro_gate_remains_active_while_signed_rm_enable_is_set() {
        let mut p = Ym2413OutputPipeline::new();
        tick(&mut p, 0, 5, Some(0x20));
        assert_eq!(p.process_cycle(1, 0, None).unwrap().ro.value(), 1);
        assert_eq!(p.export_state().rm_enable as u8, 0xc0);
        assert_eq!(p.process_cycle(2, 0, None).unwrap().ro.value(), 6);
        assert_eq!(p.export_state().rm_enable as u8, 0xe0);
        assert_eq!(p.process_cycle(3, 0, None).unwrap().ro.value(), 1);
        assert_eq!(p.export_state().rm_enable as u8, 0xf0);
    }

    #[test]
    fn all_eighteen_cycles_advance_in_order_and_wrap() {
        let mut p = Ym2413OutputPipeline::new();
        for c in 0..18 {
            assert_eq!(p.process_cycle(c, 0, None).unwrap().cycle, c);
            assert_eq!(p.export_state().next_cycle, (c + 1) % 18);
        }
        assert_eq!(p.export_state().next_cycle, 0);
    }

    #[test]
    fn invalid_cycle_and_input_are_transactional() {
        let mut p = Ym2413OutputPipeline::new();
        let before = p.export_state();
        assert_eq!(
            p.process_cycle(1, 0, None),
            Err(Ym2413OutputPipelineError::InvalidCycle {
                expected: 0,
                actual: 1
            })
        );
        assert_eq!(p.export_state(), before);
        assert_eq!(
            p.process_cycle(0, 256, None),
            Err(Ym2413OutputPipelineError::InvalidChannelOutput(256))
        );
        assert_eq!(p.export_state(), before);
    }

    #[test]
    fn snapshot_restore_matches_continuous_execution() {
        let mut continuous = Ym2413OutputPipeline::new();
        let mut split = Ym2413OutputPipeline::new();
        let mut expected = Vec::new();
        for c in 0..18 {
            expected.push(tick(
                &mut continuous,
                c,
                i16::from(c) - 9,
                (c == 4).then_some(0x20),
            ));
        }
        let mut prefix = Vec::new();
        for c in 0..7 {
            prefix.push(tick(
                &mut split,
                c,
                i16::from(c) - 9,
                (c == 4).then_some(0x20),
            ));
        }
        let mut restored = Ym2413OutputPipeline::with_initial_state(split.export_state()).unwrap();
        let mut resumed = Vec::new();
        for c in 7..18 {
            resumed.push(tick(
                &mut restored,
                c,
                i16::from(c) - 9,
                (c == 4).then_some(0x20),
            ));
        }
        assert_eq!(prefix, expected[..7]);
        assert_eq!(resumed, expected[7..]);
        assert_eq!(restored.export_state(), continuous.export_state());
    }

    #[test]
    fn core_owned_rm_enable_is_consumed_without_a_second_shift() {
        let mut pipeline = Ym2413OutputPipeline::new();
        let frame = pipeline
            .process_cycle_with_core_state(0, -64, Some(12))
            .unwrap();
        assert_eq!(frame.mo, Some(Ym2413NukedOutputCode::new(0)));
        assert_eq!(pipeline.export_state().rm_enable, -64);

        pipeline
            .process_cycle_with_core_state(1, 0x40, Some(0))
            .unwrap();
        assert_eq!(pipeline.export_state().rm_enable, 0x40);
    }

    #[test]
    fn unknown_operator_result_propagates_without_fabricating_zero_or_stale_output() {
        let mut pipeline = Ym2413OutputPipeline::new_unknown();
        let first = pipeline.process_cycle_with_core_state(0, 0, None).unwrap();
        assert_eq!(first.mo, None);
        assert_eq!(first.ro, None);
        assert!(!pipeline.export_state().previous_channel_output_valid);

        let second = pipeline
            .process_cycle_with_core_state(1, 0, Some(17))
            .unwrap();
        assert_eq!(second.mo, None);
        assert_eq!(second.ro, None);
        assert!(pipeline.export_state().previous_channel_output_valid);
        assert!(!pipeline.export_state().output_m_valid);
        assert!(!pipeline.export_state().output_r_valid);

        let third = pipeline
            .process_cycle_with_core_state(2, 0, Some(-9))
            .unwrap();
        assert_eq!(third.mo, Some(Ym2413NukedOutputCode::new(18)));
        assert_eq!(third.ro, Some(Ym2413NukedOutputCode::new(1)));
    }

    #[test]
    fn unknown_validity_survives_snapshot_restore_and_block_partitioning() {
        let mut continuous = Ym2413OutputPipeline::new_unknown();
        let mut split = Ym2413OutputPipeline::new_unknown();
        let inputs = [
            None,
            None,
            Some(4),
            Some(-7),
            Some(0),
            None,
            Some(12),
            Some(3),
        ];
        let mut expected = Vec::new();
        for (cycle, output) in inputs.into_iter().enumerate() {
            expected.push(
                continuous
                    .process_cycle_with_core_state(cycle as u8, 0, output)
                    .unwrap(),
            );
        }

        let mut prefix = Vec::new();
        for (cycle, output) in inputs[..4].iter().copied().enumerate() {
            prefix.push(
                split
                    .process_cycle_with_core_state(cycle as u8, 0, output)
                    .unwrap(),
            );
        }
        let mut restored = Ym2413OutputPipeline::with_initial_state(split.export_state()).unwrap();
        let mut resumed = Vec::new();
        for (offset, output) in inputs[4..].iter().copied().enumerate() {
            resumed.push(
                restored
                    .process_cycle_with_core_state((offset + 4) as u8, 0, output)
                    .unwrap(),
            );
        }
        assert_eq!(prefix, expected[..4]);
        assert_eq!(resumed, expected[4..]);
        assert_eq!(restored.export_state(), continuous.export_state());
    }

    #[test]
    fn standalone_api_rejects_unknown_output_transactionally() {
        let mut pipeline = Ym2413OutputPipeline::new_unknown();
        let before = pipeline.export_state();
        assert_eq!(
            pipeline.process_cycle(0, 0, None),
            Err(Ym2413OutputPipelineError::OutputUnknown)
        );
        assert_eq!(pipeline.export_state(), before);
    }

    #[test]
    fn restore_rejects_invalid_version_cycle_and_latch_ranges() {
        let valid = Ym2413OutputPipeline::new().export_state();
        for invalid in [
            Ym2413OutputPipelineState {
                format_version: YM2413_OUTPUT_PIPELINE_STATE_VERSION + 1,
                ..valid
            },
            Ym2413OutputPipelineState {
                next_cycle: 18,
                ..valid
            },
            Ym2413OutputPipelineState {
                previous_channel_output: 256,
                ..valid
            },
            Ym2413OutputPipelineState {
                output_m: 257,
                ..valid
            },
        ] {
            assert_eq!(
                Ym2413OutputPipeline::with_initial_state(invalid),
                Err(Ym2413OutputPipelineError::InvalidState)
            );
        }
    }
}
