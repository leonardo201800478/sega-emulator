use std::{env, fs, process};

use sega_sn76489::{PsgProfile, Sn76489Core};

#[derive(Clone, Copy)]
struct Event {
    tick: u64,
    value: u8,
}

fn parse_u8(value: &str) -> Result<u8, String> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u8::from_str_radix(hex, 16).map_err(|error| error.to_string())
    } else {
        value.parse::<u8>().map_err(|error| error.to_string())
    }
}

fn load_events(path: &str) -> Result<Vec<Event>, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut events = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut fields = line.split(',');
        let tick = fields
            .next()
            .ok_or_else(|| format!("missing tick at line {}", line_index + 1))?
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("invalid tick at line {}: {error}", line_index + 1))?;
        let value = parse_u8(
            fields
                .next()
                .ok_or_else(|| format!("missing byte at line {}", line_index + 1))?
                .trim(),
        )
        .map_err(|error| format!("invalid byte at line {}: {error}", line_index + 1))?;

        if fields.next().is_some() {
            return Err(format!("too many CSV fields at line {}", line_index + 1));
        }
        if events
            .last()
            .is_some_and(|previous: &Event| tick < previous.tick)
        {
            return Err("event ticks must be nondecreasing".to_string());
        }
        events.push(Event { tick, value });
    }

    Ok(events)
}

fn argument_value(args: &[String], index: &mut usize, name: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut calls = 0_u64;
    let mut clock_hz = 3_579_545_u64;
    let mut sample_rate = 48_000_u64;
    let mut events_path = None::<String>;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--calls" => {
                calls = argument_value(&args, &mut index, "--calls")?
                    .parse()
                    .map_err(|error| format!("invalid --calls: {error}"))?;
            }
            "--clock-hz" => {
                clock_hz = argument_value(&args, &mut index, "--clock-hz")?
                    .parse()
                    .map_err(|error| format!("invalid --clock-hz: {error}"))?;
            }
            "--sample-rate" => {
                sample_rate = argument_value(&args, &mut index, "--sample-rate")?
                    .parse()
                    .map_err(|error| format!("invalid --sample-rate: {error}"))?;
            }
            "--events" => {
                events_path = Some(argument_value(&args, &mut index, "--events")?);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }

    if calls == 0 || clock_hz == 0 || sample_rate == 0 {
        return Err("--calls, --clock-hz and --sample-rate must be positive".to_string());
    }

    let events = match events_path.as_deref() {
        Some(path) => load_events(path)?,
        None => Vec::new(),
    };

    let mut psg = Sn76489Core::new_with_profile(clock_hz, sample_rate, PsgProfile::Sms)
        .map_err(|error| format!("cannot initialize SMS PSG: {error:?}"))?;

    println!(
        "{{\"schema\":\"sega-sn76489-core-state-trace-v1\",\"source_repo\":\"leonardo201800478/sega-emulator\",\"profile\":\"sms\",\"time_unit\":\"psg_internal_clock\",\"clock_hz\":{clock_hz},\"sample_rate_hz\":{sample_rate},\"input_clocks_per_row\":16}}"
    );

    let mut event_index = 0_usize;
    for tick in 0..calls {
        let mut write_count = 0_u32;
        while event_index < events.len() && events[event_index].tick == tick {
            psg.write(events[event_index].value);
            event_index += 1;
            write_count += 1;
        }
        if event_index < events.len() && events[event_index].tick < tick {
            return Err("event tick was skipped".to_string());
        }

        let lfsr_before = psg.lfsr();
        let noise_output_before = psg.noise_output();
        let noise_counter_before = psg.noise_counter();
        let noise_phase_before = psg.noise_counter_phase();
        let tone_periods_before = psg.tone_periods();
        let tone_counters_before = psg.tone_counters();
        let tone_outputs_before = psg.tone_outputs();
        let attenuations_before = psg.attenuations();
        let noise_control_before = psg.noise_control();
        let latched_register_before = psg.latched_register();

        psg.advance_clocks(16);

        let tone_periods_after = psg.tone_periods();
        let tone_counters_after = psg.tone_counters();
        let tone_outputs_after = psg.tone_outputs();
        let attenuations_after = psg.attenuations();
        let latched_register_after = psg.latched_register();
        let noise_control_after = psg.noise_control();

        println!(
            concat!(
                "{{\"tick\":{},\"write_count\":{},",
                "\"latched_register_before\":{},",
                "\"noise_control_before\":{},",
                "\"tone_periods_before\":{:?},",
                "\"tone_periods_after\":{:?},",
                "\"tone_counters_before\":{:?},",
                "\"tone_counters_after\":{:?},",
                "\"tone_outputs_before\":{:?},",
                "\"tone_outputs_after\":{:?},",
                "\"attenuations_before\":{:?},",
                "\"attenuations_after\":{:?},",
                "\"latched_register_after\":{},",
                "\"noise_control_after\":{},",
                "\"lfsr_before\":{},\"lfsr_after\":{},",
                "\"lfsr_bit_before\":{},\"lfsr_bit_after\":{},",
                "\"noise_output_before\":{},\"noise_output_after\":{},",
                "\"noise_counter_before\":{},\"noise_counter_after\":{},",
                "\"noise_phase_before\":{},\"noise_phase_after\":{},",
                "\"noise_period_after\":{},\"elapsed_clocks_after\":{},",
                "\"clock_divider_phase_after\":{},\"mixed_amplitude_after\":{:.12}",
                "}}"
            ),
            tick,
            write_count,
            latched_register_before,
            noise_control_before,
            tone_periods_before,
            tone_periods_after,
            tone_counters_before,
            tone_counters_after,
            tone_outputs_before,
            tone_outputs_after,
            attenuations_before,
            attenuations_after,
            latched_register_after,
            noise_control_after,
            lfsr_before,
            psg.lfsr(),
            lfsr_before & 1,
            psg.lfsr() & 1,
            noise_output_before,
            psg.noise_output(),
            noise_counter_before,
            psg.noise_counter(),
            noise_phase_before,
            psg.noise_counter_phase(),
            psg.noise_period(),
            psg.elapsed_clocks(),
            psg.clock_divider_phase(),
            psg.mixed_amplitude(),
        )
    }

    if event_index != events.len() {
        return Err("one or more events occur after the requested trace window".to_string());
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(2);
    }
}
