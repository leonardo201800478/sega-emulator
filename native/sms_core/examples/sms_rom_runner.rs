//! Bounded offline runner for SMS diagnostic cartridge images.
//!
//! Run through the repository helper:
//! python tests/sms/scripts/run_rom.py --rom path/to/test.sms --suite-id smsmemtest --region usa
//!
//! This executes the real SmsTemporalMachine path. It is not a CPU-only FlatBus
//! harness and it does not infer suite PASS/FAIL from successful frame execution.

use std::env;
use std::fs;
use std::process;

use sega_sms_core::{
    SmsTemporalMachine, SmsVdpProfile, SmsVdpRevision, SpriteLimitMode, Territory, VideoStandard,
};

const DEFAULT_FRAMES: u64 = 120;
const DEFAULT_MAX_INSTRUCTIONS_PER_FRAME: usize = 100_000;
const MASTER_TICKS_PER_TSTATE: u64 = 1;
const AUDIO_SAMPLE_RATE: u64 = 44_100;

#[derive(Debug)]
struct Args {
    rom_path: String,
    rom_sha256: String,
    emulator_sha: String,
    suite_id: String,
    suite_revision: String,
    region: String,
    frames: u64,
    max_instructions_per_frame: usize,
}

fn usage() -> &'static str {
    "Usage: sms_rom_runner --rom PATH --rom-sha256 HEX64 [--emulator-sha SHA] [--suite-id ID] [--suite-revision REV] [--region usa|brazil|europe|japan] [--frames N] [--max-instructions-per-frame N]"
}

fn parse_args() -> Result<Args, String> {
    let mut args = env::args().skip(1);
    let mut rom_path = None;
    let mut rom_sha256 = None;
    let mut emulator_sha = String::from("unknown");
    let mut suite_id = String::from("unspecified");
    let mut suite_revision = String::from("unspecified");
    let mut region = String::from("usa");
    let mut frames = DEFAULT_FRAMES;
    let mut max_instructions_per_frame = DEFAULT_MAX_INSTRUCTIONS_PER_FRAME;

    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            println!("{}", usage());
            process::exit(0);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}\n{}", usage()))?;
        match flag.as_str() {
            "--rom" => rom_path = Some(value),
            "--rom-sha256" => rom_sha256 = Some(value),
            "--emulator-sha" => emulator_sha = value,
            "--suite-id" => suite_id = value,
            "--suite-revision" => suite_revision = value,
            "--region" => region = value,
            "--frames" => {
                frames = value
                    .parse()
                    .map_err(|_| format!("invalid --frames value: {value}"))?;
            }
            "--max-instructions-per-frame" => {
                max_instructions_per_frame = value
                    .parse()
                    .map_err(|_| format!("invalid instruction limit: {value}"))?;
            }
            _ => return Err(format!("unknown argument {flag}\n{}", usage())),
        }
    }

    let rom_path = rom_path.ok_or_else(|| format!("--rom is required\n{}", usage()))?;
    let rom_sha256 = rom_sha256.ok_or_else(|| format!("--rom-sha256 is required\n{}", usage()))?;
    if rom_sha256.len() != 64 || !rom_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("--rom-sha256 must contain exactly 64 hexadecimal characters".to_owned());
    }
    if frames == 0 || max_instructions_per_frame == 0 {
        return Err("frame and instruction limits must be greater than zero".to_owned());
    }
    if frames > 100_000 {
        return Err("--frames exceeds the safety ceiling of 100000".to_owned());
    }

    Ok(Args {
        rom_path,
        rom_sha256: rom_sha256.to_ascii_lowercase(),
        emulator_sha,
        suite_id,
        suite_revision,
        region,
        frames,
        max_instructions_per_frame,
    })
}

/// Escape a string for one JSON string value without adding a serialization dependency.
fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

/// Select the same regional hardware tuple used by the SMS runtime worker.
fn regional_profile(region: &str) -> Result<(Territory, SmsVdpProfile, u64), String> {
    let revision = SmsVdpRevision::Sega3155124;
    let sprite_limit = SpriteLimitMode::HardwareLimit;
    match region {
        "usa" => Ok((Territory::Export, SmsVdpProfile::sms1_t0_ntsc(), 3_579_545)),
        "brazil" => Ok((
            Territory::Export,
            SmsVdpProfile::new(revision, VideoStandard::PalM, sprite_limit),
            3_579_545,
        )),
        "europe" => Ok((
            Territory::Export,
            SmsVdpProfile::new(revision, VideoStandard::Pal, sprite_limit),
            3_546_895,
        )),
        "japan" => Ok((
            Territory::Japan,
            SmsVdpProfile::new(revision, VideoStandard::Ntsc, sprite_limit),
            3_579_545,
        )),
        _ => Err(format!(
            "unsupported region '{region}'; expected usa, brazil, europe or japan"
        )),
    }
}

fn emit_error(args: &Args, message: &str, completed_frames: u64, total_instructions: u64) {
    println!(
        "{{\"record\":\"summary\",\"status\":\"EXECUTION_ERROR\",\"suite_id\":\"{}\",\"suite_revision\":\"{}\",\"rom_sha256\":\"{}\",\"emulator_sha\":\"{}\",\"region\":\"{}\",\"completed_frames\":{},\"total_instructions\":{},\"error\":\"{}\"}}",
        json_escape(&args.suite_id),
        json_escape(&args.suite_revision),
        args.rom_sha256,
        json_escape(&args.emulator_sha),
        json_escape(&args.region),
        completed_frames,
        total_instructions,
        json_escape(message),
    );
}

fn run(args: Args) -> Result<(), String> {
    let rom = fs::read(&args.rom_path)
        .map_err(|error| format!("cannot read ROM '{}': {error}", args.rom_path))?;
    if rom.is_empty() {
        return Err("ROM image is empty".to_owned());
    }

    let (territory, vdp_profile, psg_clock_hz) = regional_profile(&args.region)?;
    let rom_size = rom.len();
    let mut machine = SmsTemporalMachine::new_with_hardware_profile(
        rom,
        MASTER_TICKS_PER_TSTATE,
        psg_clock_hz,
        AUDIO_SAMPLE_RATE,
        vdp_profile,
        territory,
    )
    .map_err(|error| format!("cannot construct SMS temporal machine: {error}"))?;

    println!(
        "{{\"record\":\"start\",\"status\":\"RUNNING\",\"runner\":\"sms-temporal-machine\",\"boot_profile\":\"direct_cartridge_no_bios\",\"suite_id\":\"{}\",\"suite_revision\":\"{}\",\"rom_sha256\":\"{}\",\"rom_size_bytes\":{},\"emulator_sha\":\"{}\",\"region\":\"{}\",\"vdp_profile\":\"Sega3155124\",\"frames_requested\":{},\"max_instructions_per_frame\":{}}}",
        json_escape(&args.suite_id),
        json_escape(&args.suite_revision),
        args.rom_sha256,
        rom_size,
        json_escape(&args.emulator_sha),
        json_escape(&args.region),
        args.frames,
        args.max_instructions_per_frame,
    );

    let mut total_instructions = 0_u64;
    let mut total_audio_samples = 0_u64;
    let initial_frame = machine.frame_number();

    for index in 0..args.frames {
        let instructions = match machine.step_frame(args.max_instructions_per_frame) {
            Ok(instructions) => instructions,
            Err(error) => {
                emit_error(
                    &args,
                    &format!("frame {}: {error}", index + 1),
                    index,
                    total_instructions,
                );
                return Err(format!("frame {} did not complete: {error}", index + 1));
            }
        };
        total_instructions = total_instructions.saturating_add(instructions as u64);

        // Drain generated PCM after each completed frame so this offline
        // diagnostic run cannot accumulate an unbounded host sample buffer.
        let audio_samples = machine.drain_audio_samples(None).len() as u64;
        total_audio_samples = total_audio_samples.saturating_add(audio_samples);

        println!(
            "{{\"record\":\"frame\",\"status\":\"FRAME_COMPLETED\",\"frame_index\":{},\"machine_frame_number\":{},\"instructions\":{},\"total_instructions\":{},\"audio_samples_drained\":{},\"total_audio_samples_drained\":{},\"machine_time\":\"{}\"}}",
            index + 1,
            machine.frame_number(),
            instructions,
            total_instructions,
            audio_samples,
            total_audio_samples,
            json_escape(&format!("{:?}", machine.now())),
        );
    }

    let frames_completed = machine.frame_number().saturating_sub(initial_frame);
    println!(
        "{{\"record\":\"summary\",\"status\":\"EXECUTED_NO_ORACLE\",\"suite_id\":\"{}\",\"suite_revision\":\"{}\",\"rom_sha256\":\"{}\",\"emulator_sha\":\"{}\",\"region\":\"{}\",\"frames_requested\":{},\"frames_completed\":{},\"total_instructions\":{},\"total_audio_samples_drained\":{},\"mapper_banks\":[{},{},{}],\"memory_control\":{}}}",
        json_escape(&args.suite_id),
        json_escape(&args.suite_revision),
        args.rom_sha256,
        json_escape(&args.emulator_sha),
        json_escape(&args.region),
        args.frames,
        frames_completed,
        total_instructions,
        total_audio_samples,
        machine.selected_mapper_banks()[0],
        machine.selected_mapper_banks()[1],
        machine.selected_mapper_banks()[2],
        machine.memory_control(),
    );

    Ok(())
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            process::exit(64);
        }
    };

    if let Err(error) = run(args) {
        eprintln!("{error}");
        process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_preserves_json_string_boundaries() {
        assert_eq!(
            json_escape("quote \" slash \\ line\n"),
            "quote \\\" slash \\\\ line\\n"
        );
    }

    #[test]
    fn regional_profiles_are_explicit_and_reject_unknown_values() {
        assert!(regional_profile("usa").is_ok());
        assert!(regional_profile("brazil").is_ok());
        assert!(regional_profile("europe").is_ok());
        assert!(regional_profile("japan").is_ok());
        assert!(regional_profile("pal").is_err());
    }
}
