use sega_ym2413::Ym2413OutputPipeline;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| invalid_data("usage: output_pipeline_trace <nuked-output-pipeline.csv>"))?;
    let reader = BufReader::new(File::open(path)?);
    let mut pipeline = Ym2413OutputPipeline::new();
    let mut calls = 0_u64;
    let mut qualified_mode_writes = 0_u64;
    let mut nonzero_channel_outputs = 0_u64;

    for (line_index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with("call_index,") {
            continue;
        }
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() != 11 {
            return Err(invalid_data(format!(
                "line {}: expected 11 CSV fields, found {}",
                line_index + 1,
                fields.len()
            ))
            .into());
        }

        let call_index = fields[0].parse::<u64>()?;
        let cycle = fields[1].parse::<u8>()?;
        let expected_ch_out_before = fields[2].parse::<i16>()?;
        let next_channel_output = fields[3].parse::<i16>()?;
        let mode_write_data = if fields[4].is_empty() {
            None
        } else {
            Some(fields[4].parse::<u8>()?)
        };
        let expected_buffer_m = fields[5].parse::<i16>()?;
        let expected_buffer_r = fields[6].parse::<i16>()?;
        let expected_pin_m = fields[7].parse::<i16>()?;
        let expected_pin_r = fields[8].parse::<i16>()?;
        let expected_rm_enable_before = fields[9].parse::<i8>()?;
        let expected_rm_enable_after = fields[10].parse::<i8>()?;
        let mo_slot = (cycle + 3) % 18;
        let ro_slot = (cycle + 15) % 18;

        if call_index != calls || cycle != (call_index % 18) as u8 {
            return Err(invalid_data(format!(
                "non-contiguous trace at line {}: call={} cycle={} expected_call={} expected_cycle={}",
                line_index + 1,
                call_index,
                cycle,
                calls,
                calls % 18
            ))
            .into());
        }

        let before = pipeline.export_state();
        if before.previous_channel_output != expected_ch_out_before
            || before.rm_enable != expected_rm_enable_before
            || before.next_cycle != cycle
        {
            return Err(invalid_data(format!(
                "pre-state mismatch call={} cycle={} MO-slot={} RO-slot={} mode_write_0x0e={:?}: expected ch_out_before={} rm_enable_before={} next_cycle={}; got ch_out_before={} rm_enable_before={} next_cycle={}",
                call_index,
                cycle,
                mo_slot,
                ro_slot,
                mode_write_data,
                expected_ch_out_before,
                expected_rm_enable_before,
                cycle,
                before.previous_channel_output,
                before.rm_enable,
                before.next_cycle
            ))
            .into());
        }

        let frame = pipeline.process_cycle(cycle, next_channel_output, mode_write_data)?;
        let state = pipeline.export_state();

        if frame.mo.value() != expected_buffer_m || frame.ro.value() != expected_buffer_r {
            return Err(invalid_data(format!(
                "buffer mismatch call={} cycle={} MO-slot={} RO-slot={} mode_write_0x0e={:?} ch_out_before={} ch_out_after={} rm_enable_before={} rm_enable_after_expected={}: expected buffer MO/RO={}/{} got={}/{}",
                call_index,
                cycle,
                mo_slot,
                ro_slot,
                mode_write_data,
                expected_ch_out_before,
                next_channel_output,
                expected_rm_enable_before,
                expected_rm_enable_after,
                expected_buffer_m,
                expected_buffer_r,
                frame.mo.value(),
                frame.ro.value()
            ))
            .into());
        }

        if state.output_m != expected_pin_m
            || state.output_r != expected_pin_r
            || state.rm_enable != expected_rm_enable_after
            || state.previous_channel_output != next_channel_output
            || state.next_cycle != (cycle + 1) % 18
        {
            return Err(invalid_data(format!(
                "post-state mismatch call={} cycle={} MO-slot={} RO-slot={} mode_write_0x0e={:?} ch_out_before={} ch_out_after={} rm_enable_before={} rm_enable_after_expected={}: expected pins MO/RO={}/{} got={}/{}; actual rm_enable_after={} ch_out_after={} next_cycle={}",
                call_index,
                cycle,
                mo_slot,
                ro_slot,
                mode_write_data,
                expected_ch_out_before,
                next_channel_output,
                expected_rm_enable_before,
                expected_rm_enable_after,
                expected_pin_m,
                expected_pin_r,
                state.output_m,
                state.output_r,
                state.rm_enable,
                state.previous_channel_output,
                state.next_cycle
            ))
            .into());
        }

        if mode_write_data.is_some() {
            qualified_mode_writes += 1;
        }
        if next_channel_output != 0 {
            nonzero_channel_outputs += 1;
        }
        calls += 1;
    }

    if calls == 0 {
        return Err(invalid_data("oracle trace contained no samples").into());
    }
    if qualified_mode_writes < 2 {
        return Err(invalid_data(format!(
            "expected Rhythm Mode enable and disable writes, observed {qualified_mode_writes}"
        ))
        .into());
    }

    println!(
        "{{\"schema\":\"ym2413-output-pipeline-differential-v1\",\"result\":\"PASS\",\"calls\":{calls},\"qualified_rhythm_writes\":{qualified_mode_writes},\"nonzero_channel_outputs\":{nonzero_channel_outputs}}}"
    );
    Ok(())
}
