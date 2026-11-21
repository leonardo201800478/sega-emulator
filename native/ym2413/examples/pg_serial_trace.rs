//! Emit the integrated core-owned serial PG state for external Nuked comparison.
//!
//! Arguments: <events.csv> <OPLL_Clock call count>
//! Event CSV columns: call_index, port (0=address, 1=data), value.

use sega_ym2413::{
    AttackRate15Transition, DampTransitionConfig, DampTransitionCoordination, EnvelopeStage,
    YM2413_INPUT_CLOCKS_PER_SLOT, Ym2413ChannelInitialState, Ym2413Core, Ym2413Operator,
    Ym2413PgSerialService, Ym2413SynthesisBank, Ym2413SynthesisInitialState,
    nuked_pg_phase_slot_descriptor,
};
use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, Error as IoError, ErrorKind};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
struct Event {
    call_index: u64,
    port: u8,
    value: u8,
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}

fn load_events(path: &Path, calls: u64) -> Result<Vec<Event>, Box<dyn Error>> {
    let input = BufReader::new(File::open(path)?);
    let mut events = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = trimmed.split(',').map(str::trim).collect();
        if fields.len() != 3 {
            return Err(invalid_data(format!(
                "line {}: expected call_index, port, value",
                line_index + 1
            ))
            .into());
        }
        let event = Event {
            call_index: fields[0].parse()?,
            port: fields[1].parse()?,
            value: fields[2].parse()?,
        };
        if event.port > 1 {
            return Err(invalid_data(format!(
                "line {}: unsupported YM2413 port {}",
                line_index + 1,
                event.port
            ))
            .into());
        }
        if event.call_index >= calls {
            return Err(invalid_data(format!(
                "line {}: event call {} is outside requested trace length {}",
                line_index + 1,
                event.call_index,
                calls
            ))
            .into());
        }
        if events
            .last()
            .is_some_and(|previous: &Event| previous.call_index > event.call_index)
        {
            return Err(invalid_data("events must be sorted by call_index").into());
        }
        events.push(event);
    }

    Ok(events)
}

fn write_json_array<T: std::fmt::Display>(
    output: &mut String,
    values: impl IntoIterator<Item = T>,
) {
    output.push('[');
    for (index, value) in values.into_iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let _ = write!(output, "{value}");
    }
    output.push(']');
}

fn reset_service(rhythm_register: Option<u32>) -> Result<Ym2413PgSerialService, Box<dyn Error>> {
    let damp_config = DampTransitionConfig::new(
        124,
        DampTransitionCoordination::IndependentPerOperator,
        AttackRate15Transition::SkipAttackToDecay,
    )
    .map_err(|error| invalid_data(format!("invalid damp configuration: {error:?}")))?;
    let initial = Ym2413SynthesisInitialState {
        channels: std::array::from_fn(|_| Ym2413ChannelInitialState {
            phase: [0; 2],
            envelope_level: [127; 2],
            envelope_stage: [EnvelopeStage::Damp; 2],
            envelope_key_on: [false; 2],
            feedback_history: [0; 2],
            previous_modulator_phase_index: None,
        }),
        pm_lfo_counter: 0,
        am_lfo_phase: 0,
        damp_config,
    };
    let bank = Ym2413SynthesisBank::with_initial_state(initial)
        .map_err(|error| invalid_data(format!("invalid synthesis bank state: {error:?}")))?;
    let service = Ym2413PgSerialService::new(bank);
    match rhythm_register {
        Some(register) => service
            .with_rhythm_noise_register(register, 0)
            .map_err(|error| {
                invalid_data(format!("invalid rhythm register candidate: {error:?}")).into()
            }),
        None => Ok(service),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "usage: pg_serial_trace <events.csv> <OPLL_Clock call count> [rhythm LFSR register candidate]",
        )
        .into());
    }
    let calls: u64 = args[1].parse()?;
    let events = load_events(Path::new(&args[0]), calls)?;
    let rhythm_register = args.get(2).map(|value| value.parse::<u32>()).transpose()?;

    let mut core = Ym2413Core::new();
    let mut service = reset_service(rhythm_register)?;
    let mut event_index = 0usize;
    let mut phases = [0u32; 18];

    for call_index in 0..calls {
        while event_index < events.len() && events[event_index].call_index == call_index {
            let event = events[event_index];
            match event.port {
                0 => core.write_address(event.value),
                1 => core.write_data(event.value),
                _ => unreachable!("event port was validated"),
            }
            event_index += 1;
        }

        if event_index < events.len() && events[event_index].call_index < call_index {
            return Err(invalid_data("an event call_index was skipped").into());
        }

        let cycle_before = core.slot_index();
        let lfo_vib_counter_before = service.lfo_vib_counter();
        let operator_before = service.export_state().operator;
        core.advance_input_clocks_with_pg_service(YM2413_INPUT_CLOCKS_PER_SLOT, &mut service)?;

        for slot in 0u8..18 {
            let descriptor = nuked_pg_phase_slot_descriptor(slot)
                .ok_or_else(|| invalid_data(format!("invalid physical PG slot {slot}")))?;
            let carrier = matches!(descriptor.operator, Ym2413Operator::Carrier);
            phases[usize::from(slot)] = service
                .synthesis()
                .phase_accumulator(descriptor.channel, carrier)
                .map_err(|error| invalid_data(format!("invalid PG accumulator: {error:?}")))?;
        }
        let core_state = core.export_state();
        let envelope = core_state.envelope;
        let timer = envelope.timer;
        let service_state = service.export_state();
        let operator = service_state.operator;
        let serial_am = service_state.serial_am_lfo;
        let eg_levels: [u8; 18] = std::array::from_fn(|index| envelope.operators[index].level);
        let eg_states: [u8; 18] =
            std::array::from_fn(|index| envelope.operators[index].stage as u8);
        let rm_enable = core_state.rm_enable as u8;
        let rhythm_control = core_state.registers[0x0e];
        let rhythm_mode_enabled = u8::from(rhythm_control & 0x20 != 0);
        let rhythm_noise_register = service_state
            .rhythm_noise
            .map_or(-1_i64, |noise| i64::from(noise.lfsr.register));
        let rhythm_phase_latches = service_state.pipeline.rhythm_phase_latches;

        let mut trace_line = String::with_capacity(4096);
        let _ = write!(
            &mut trace_line,
            "{{\"call_index\":{call_index},\"cycle_before\":{cycle_before},\"cycle_after\":{},",
            core.slot_index()
        );
        let _ = write!(
            &mut trace_line,
            "\"lfo_vib_counter_before\":{lfo_vib_counter_before},\"lfo_vib_counter_after\":{},\"lfo_vib_counter\":{},\"lfo_counter\":{},",
            service.lfo_vib_counter(),
            service_state.pipeline.lfo_vib_counter,
            service.synthesis().pm_lfo_counter()
        );
        let _ = write!(
            &mut trace_line,
            "\"pg_increment\":{},\"pg_inc\":{},\"pg_phase_next\":{},\"pg_output\":{},\"pg_out\":{},",
            service_state.pipeline.pg_increment,
            service_state.pipeline.pg_increment,
            service_state.pipeline.pg_phase_next,
            service_state.pipeline.pg_output,
            service_state.pipeline.pg_output
        );
        trace_line.push_str("\"phases\":");
        write_json_array(&mut trace_line, phases);
        trace_line.push_str(",\"pg_phase\":");
        write_json_array(&mut trace_line, phases);

        let _ = write!(
            &mut trace_line,
            ",\"op_calc_eg_out\":{},\"op_calc_eg_silent\":{},\"op_calc_c_fb\":{},\"op_calc_c_dc\":{},\"op_calc_c_dm\":{},\"op_calc_pg_out\":{},\"op_calc_op_mod\":{},\"op_calc_op_fbsum\":{}",
            operator_before.eg_out,
            u8::from(operator_before.eg_silent),
            operator_before.c_fb,
            operator_before.c_dc,
            operator_before.c_dm,
            service_state.pipeline.pg_output,
            operator_before.op_mod,
            operator_before.op_fbsum
        );
        trace_line.push_str(",\"op_calc_op_fb1\":");
        write_json_array(&mut trace_line, operator_before.op_fb1);
        trace_line.push_str(",\"op_calc_op_fb2\":");
        write_json_array(&mut trace_line, operator_before.op_fb2);
        let _ = write!(
            &mut trace_line,
            ",\"lfo_am_counter\":{},\"lfo_am_dir\":{},\"lfo_am_car\":{},\"lfo_am_step\":{},\"lfo_am_out\":{}",
            serial_am.counter,
            u8::from(serial_am.direction),
            u8::from(serial_am.carry),
            u8::from(serial_am.step),
            serial_am.output
        );
        let _ = write!(
            &mut trace_line,
            ",\"c_instr\":{},\"c_op\":{},\"c_tl\":{},\"c_fb\":{},\"c_am\":{},\"c_vib\":{},\"c_ksr\":{},\"c_ksl\":{},\"c_multi\":{},\"c_fnum\":{},\"c_block\":{}",
            operator.c_instr,
            operator.c_op,
            operator.c_tl,
            operator.c_fb,
            u8::from(operator.c_am),
            u8::from(operator.c_vib),
            u8::from(operator.c_ksr),
            operator.c_ksl,
            operator.c_multi,
            operator.c_fnum,
            operator.c_block
        );
        trace_line.push_str(",\"c_adrr\":");
        write_json_array(&mut trace_line, operator.c_adrr);
        let _ = write!(
            &mut trace_line,
            ",\"c_et\":{},\"c_sl\":{},\"c_ksr_freq\":{},\"c_dc\":{},\"c_dm\":{},\"c_ksl_freq\":{},\"c_ksl_block\":{}",
            u8::from(operator.c_et),
            operator.c_sl,
            operator.c_ksr_freq,
            operator.c_dc,
            operator.c_dm,
            operator.c_ksl_freq,
            operator.c_ksl_block
        );
        let _ = write!(
            &mut trace_line,
            ",\"eg_counter_state\":{},\"eg_counter_state_prev\":{},\"eg_timer\":{},\"eg_timer_carry\":{},\"eg_timer_low_lock\":{},\"eg_timer_shift\":{},\"eg_timer_shift_lock\":{},\"eg_timer_shift_stop\":{}",
            timer.counter_state,
            timer.counter_state_prev,
            timer.timer,
            timer.carry,
            timer.low_lock,
            timer.shift,
            timer.shift_lock,
            u8::from(timer.shift_stop)
        );
        let _ = write!(
            &mut trace_line,
            ",\"eg_kon\":{},\"eg_dokon\":{},\"eg_off\":{},\"eg_rate\":{},\"eg_rate_hi\":{},\"eg_maxrate\":{},\"eg_zerorate\":{},\"eg_inc_lo\":{},\"eg_inc_hi\":{},\"eg_sl\":{},\"eg_silent\":{}",
            envelope.eg_kon,
            envelope.eg_dokon,
            envelope.eg_off,
            timer.pending_rate,
            timer.latched_rate_hi,
            u8::from(timer.latched_max_rate),
            u8::from(timer.pending_zero_rate),
            u8::from(timer.latched_inc_lo),
            u8::from(timer.latched_inc_hi),
            envelope.eg_sl,
            u8::from(operator.eg_silent)
        );
        trace_line.push_str(",\"eg_state\":");
        write_json_array(&mut trace_line, eg_states);
        trace_line.push_str(",\"eg_level\":");
        write_json_array(&mut trace_line, eg_levels);
        let _ = write!(
            &mut trace_line,
            ",\"eg_out\":{},\"eg_ksltl\":{},\"c_ksl_attenuation\":{}",
            operator.eg_out, operator.eg_ksltl, operator.c_ksl_attenuation
        );
        trace_line.push_str(",\"op_fb1\":");
        write_json_array(&mut trace_line, operator.op_fb1);
        trace_line.push_str(",\"op_fb2\":");
        write_json_array(&mut trace_line, operator.op_fb2);
        let _ = write!(
            &mut trace_line,
            ",\"op_fbsum\":{},\"op_mod\":{},\"op_neg\":{},\"op_logsin\":{},\"op_exp_m\":{},\"op_exp_s\":{},\"op_phase_index\":{},\"op_output\":{},\"ch_out\":{},\"op_feedback_phase\":{}",
            operator.op_fbsum,
            operator.op_mod,
            operator.op_neg,
            operator.op_logsin,
            operator.op_exp_m,
            operator.op_exp_s,
            operator.last_phase_index,
            operator.last_operator_output,
            operator.last_channel_output,
            operator.last_feedback_phase
        );
        let _ = write!(
            &mut trace_line,
            ",\"eg_pending_rate\":{},\"eg_pending_zero_rate\":{},\"eg_next_slot\":{},\"rm_enable\":{},\"op_calc_rm_enable\":{},\"rm_select\":{},\"rhythm_control\":{},\"rhythm_mode_enabled\":{},\"rm_noise\":{},\"rhythm_noise_register\":{},\"rhythm_noise_cursor\":{},\"rhythm_noise_steps\":{},\"rhythm_hh_bit2\":{},\"rhythm_hh_bit3\":{},\"rhythm_hh_bit7\":{},\"rhythm_hh_bit8\":{},\"rhythm_tc_bit3\":{},\"rhythm_tc_bit5\":{},\"ch_out_hh\":{},\"ch_out_tm\":{},\"ch_out_bd\":{},\"ch_out_sd\":{},\"ch_out_tc\":{}}}",
            timer.pending_rate,
            u8::from(timer.pending_zero_rate),
            timer.next_slot,
            rm_enable,
            rm_enable,
            core_state.rm_select,
            rhythm_control,
            rhythm_mode_enabled,
            rhythm_noise_register,
            rhythm_noise_register,
            service_state.rhythm_noise_cursor,
            service_state.rhythm_noise_steps,
            u8::from(rhythm_phase_latches.hh_bit2),
            u8::from(rhythm_phase_latches.hh_bit3),
            u8::from(rhythm_phase_latches.hh_bit7),
            u8::from(rhythm_phase_latches.hh_bit8),
            u8::from(rhythm_phase_latches.tc_bit3),
            u8::from(rhythm_phase_latches.tc_bit5),
            operator.rhythm_channel_outputs[0],
            operator.rhythm_channel_outputs[1],
            operator.rhythm_channel_outputs[2],
            operator.rhythm_channel_outputs[3],
            operator.rhythm_channel_outputs[4]
        );
        println!("{trace_line}");
    }

    if event_index != events.len() {
        return Err(invalid_data("not all input events were consumed").into());
    }
    Ok(())
}
