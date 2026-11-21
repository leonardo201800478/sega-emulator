//! Replay the serial YM2413 operator stage against pre-call Nuked latches.
//!
//! Input CSV columns:
//! call,cycle,pg_out,eg_out,eg_silent,c_fb,c_dc,c_dm,exp_m,exp_s,logsin,neg,op_mod,op_fbsum,
//! fb1[0..8],fb2[0..8],ch_out,phase_index,operator_output
//!
//! The comparator supplies only the pre-call inputs consumed by OPLL_Operator.
//! All operator pipeline outputs and feedback histories are produced by the
//! production Rust Ym2413SerialOperator implementation.

use sega_ym2413::Ym2413SerialOperator;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

fn parse<T: std::str::FromStr>(fields: &[&str], index: usize) -> Result<T, Box<dyn Error>>
where
    T::Err: Error + 'static,
{
    Ok(fields[index].parse()?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or(
        "usage: cargo run --manifest-path native/ym2413/Cargo.toml --example serial_operator_trace -- input.csv",
    )?;
    let input = BufReader::new(File::open(Path::new(&path))?);
    let mut operator = Ym2413SerialOperator::new();
    let mut compared = 0_u64;

    for (line_number, line) in input.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = trimmed.split(',').map(str::trim).collect();
        if fields.len() != 41 {
            return Err(format!(
                "line {}: expected 41 CSV columns, got {}",
                line_number + 1,
                fields.len()
            )
            .into());
        }

        let call: u64 = parse(&fields, 0)?;
        let cycle: u8 = parse(&fields, 1)?;
        let pg_out: u16 = parse(&fields, 2)?;
        let eg_out: u8 = parse(&fields, 3)?;
        let eg_silent = match fields[4] {
            "0" => false,
            "1" => true,
            other => {
                return Err(format!("line {}: invalid eg_silent {other}", line_number + 1).into());
            }
        };
        let c_fb: u8 = parse(&fields, 5)?;
        let c_dc: u8 = parse(&fields, 6)?;
        let c_dm: u8 = parse(&fields, 7)?;

        let mut state = operator.export_state();
        state.eg_out = eg_out;
        state.eg_silent = eg_silent;
        state.c_fb = c_fb;
        state.c_dc = c_dc;
        state.c_dm = c_dm;
        operator = Ym2413SerialOperator::restore_state(state).map_err(|error| {
            format!(
                "line {}: invalid pre-call latches: {error:?}",
                line_number + 1
            )
        })?;

        let rm_enable: u8 = parse(&fields, 35)?;
        let sample = operator
            .tick_operator_with_rhythm_mode(cycle, pg_out, rm_enable)
            .map_err(|error| {
                format!("line {}: operator stage failed: {error:?}", line_number + 1)
            })?;
        let actual = operator.export_state();

        let expected_exp_m: u16 = parse(&fields, 8)?;
        let expected_exp_s: u8 = parse(&fields, 9)?;
        let expected_logsin: u16 = parse(&fields, 10)?;
        let expected_neg: u8 = parse(&fields, 11)?;
        let expected_mod: u16 = parse(&fields, 12)?;
        let expected_fbsum: i16 = parse(&fields, 13)?;
        let expected_fb1: [i16; 9] = std::array::from_fn(|index| {
            fields[14 + index]
                .parse()
                .expect("validated signed feedback input")
        });
        let expected_fb2: [i16; 9] = std::array::from_fn(|index| {
            fields[23 + index]
                .parse()
                .expect("validated signed feedback input")
        });
        let expected_ch_out: i16 = parse(&fields, 32)?;
        let expected_phase_index: u16 = parse(&fields, 33)?;
        let expected_operator_output: i16 = parse(&fields, 34)?;
        let expected_rhythm_outputs: [i16; 5] = std::array::from_fn(|index| {
            fields[36 + index]
                .parse()
                .expect("validated signed rhythm output latch")
        });

        if actual.op_exp_m != expected_exp_m
            || actual.op_exp_s != expected_exp_s
            || actual.op_logsin != expected_logsin
            || actual.op_neg != expected_neg
            || actual.op_mod != expected_mod
            || actual.op_fbsum != expected_fbsum
            || actual.op_fb1 != expected_fb1
            || actual.op_fb2 != expected_fb2
            || sample.channel_output != expected_ch_out
            || sample.phase_index != expected_phase_index
            || sample.operator_output != expected_operator_output
            || actual.rhythm_channel_outputs != expected_rhythm_outputs
        {
            return Err(format!(
                "call {call} cycle {cycle}: Nuked operator mismatch:                  exp_m {}/{} exp_s {}/{} logsin {}/{} neg {}/{} mod {}/{}                  fbsum {}/{} fb1 {:?}/{:?} fb2 {:?}/{:?} ch_out {}/{} phase_index {}/{} operator_output {}/{}",
                actual.op_exp_m,
                expected_exp_m,
                actual.op_exp_s,
                expected_exp_s,
                actual.op_logsin,
                expected_logsin,
                actual.op_neg,
                expected_neg,
                actual.op_mod,
                expected_mod,
                actual.op_fbsum,
                expected_fbsum,
                actual.op_fb1,
                expected_fb1,
                actual.op_fb2,
                expected_fb2,
                sample.channel_output,
                expected_ch_out,
                sample.phase_index,
                expected_phase_index,
                sample.operator_output,
                expected_operator_output
            )
            .into());
        }
        compared += 1;
    }

    println!(
        "{{\"status\":\"PASS\",\"compared_calls\":{compared},\"fields\":[\"op_exp_m\",\"op_exp_s\",\"op_logsin\",\"op_neg\",\"op_mod\",\"op_fbsum\",\"op_fb1[9]\",\"op_fb2[9]\",\"ch_out\",\"phase_index\",\"operator_output\"],\"scope\":\"serial OPLL_Operator arithmetic and feedback state with Nuked pre-call latches\"}}"
    );
    Ok(())
}
