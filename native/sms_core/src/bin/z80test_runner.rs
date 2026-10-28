//! CPU-level runner for Patrik Rak's external Z80 test suite.
//!
//! This runs the SMS core's CPU engine with flat 64 KiB memory. It is not a
//! complete SMS machine and does not model per-T-state bus events.

use sega_sms_core::{CpuBus, Z80};
use std::{env, fs, path::PathBuf, process::ExitCode, time::Instant};

const PROGRAM_START: u16 = 0x8000;
const RETURN_SENTINEL: u16 = 0x7fff;
const STACK_SLOT: u16 = 0xfffc;
const DEFAULT_MAX_STEPS: u64 = 250_000_000;

struct FlatBus {
    memory: Box<[u8]>,
    port_reads: u64,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            port_reads: 0,
        }
    }
}

impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    fn read_port(&mut self, address: u16) -> u8 {
        self.port_reads += 1;
        if address & 0x00ff == 0x00fe { 0xbf } else { 0xff }
    }

    fn write_port(&mut self, _address: u16, _value: u8) {}
}

fn parse_args() -> Result<(PathBuf, u64), String> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "z80test_runner".to_owned());
    let Some(image) = args.next() else {
        eprintln!("Usage: {program} <assembled-z80test.out> [--max-steps N]");
        return Err("missing assembled suite image".to_owned());
    };
    if image == "--help" || image == "-h" {
        println!("Usage: {program} <assembled-z80test.out> [--max-steps N]");
        return Err(String::new());
    }

    let mut max_steps = DEFAULT_MAX_STEPS;
    while let Some(argument) = args.next() {
        if argument != "--max-steps" {
            return Err(format!("unexpected argument: {argument}"));
        }
        let value = args.next().ok_or("--max-steps requires an integer")?;
        max_steps = value.parse().map_err(|_| format!("invalid --max-steps value: {value}"))?;
        if max_steps == 0 {
            return Err("--max-steps must be greater than zero".to_owned());
        }
    }
    Ok((PathBuf::from(image), max_steps))
}

fn run(image_path: PathBuf, max_steps: u64) -> Result<(), String> {
    let image = fs::read(&image_path)
        .map_err(|error| format!("cannot read {}: {error}", image_path.display()))?;
    if image.is_empty() {
        return Err("suite image is empty".to_owned());
    }
    if image.len() > usize::from(u16::MAX - PROGRAM_START - 4) {
        return Err(format!("suite image is too large: {} bytes", image.len()));
    }

    let mut bus = FlatBus::default();
    let start = usize::from(PROGRAM_START);
    bus.memory[start..start + image.len()].copy_from_slice(&image);
    // The suite returns to BASIC; replace BASIC with a sentinel return address.
    let stack = usize::from(STACK_SLOT);
    bus.memory[stack] = RETURN_SENTINEL as u8;
    bus.memory[stack + 1] = (RETURN_SENTINEL >> 8) as u8;

    let mut cpu = Z80::default();
    cpu.registers.pc = PROGRAM_START;
    cpu.registers.sp = STACK_SLOT;
    let started = Instant::now();
    let mut steps = 0_u64;

    while steps < max_steps {
        if cpu.registers.pc == RETURN_SENTINEL {
            let failures = cpu.registers.b;
            let tests = cpu.registers.c;
            println!("suite: {}", image_path.display());
            println!("instructions: {steps}");
            println!("tstates: {}", cpu.total_tstates);
            println!("elapsed_ms: {}", started.elapsed().as_millis());
            println!("port_reads: {}", bus.port_reads);
            println!("tests_reported: {tests}");
            println!("failures_reported: {failures}");
            if tests == 0 {
                return Err("suite returned without reporting tests".to_owned());
            }
            if failures != 0 {
                return Err(format!("{failures} upstream Z80 test(s) failed"));
            }
            println!("result: PASS");
            return Ok(());
        }

        cpu.step(&mut bus).map_err(|error| {
            format!("CPU error at PC={:04X} after {steps} instructions: {error:?}", cpu.registers.pc)
        })?;
        steps += 1;
    }

    Err(format!(
        "instruction budget exhausted: max_steps={max_steps}, PC={:04X}, SP={:04X}, T-states={}, elapsed_ms={}",
        cpu.registers.pc, cpu.registers.sp, cpu.total_tstates, started.elapsed().as_millis()
    ))
}

fn main() -> ExitCode {
    let (image, max_steps) = match parse_args() {
        Ok(args) => args,
        Err(error) if error.is_empty() => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    match run(image, max_steps) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("result: FAIL");
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
