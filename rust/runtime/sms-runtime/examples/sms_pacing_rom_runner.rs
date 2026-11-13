//! Measure real SMS worker frame pacing while executing an external diagnostic ROM.
//!
//! This exercises SmsTemporalWorker's production frame loop and recv_timeout
//! pacing. Video presentation and physical audio output are intentionally absent
//! so the worker/host wait path can be isolated from renderer and CPAL effects.

use std::env;
use std::fs;
use std::process;
use std::thread;
use std::time::{Duration, Instant};

use sega_sms_runtime::SmsTemporalWorker;
use sega_sms_runtime::worker::ControllerButtons;

#[derive(Debug)]
struct Args {
    rom_path: String,
    rom_sha256: String,
    emulator_sha: String,
    suite_id: String,
    suite_revision: String,
    region: String,
    duration_seconds: u64,
    warmup_seconds: u64,
    sample_interval_ms: u64,
    button1_pulse_frames: Vec<u64>,
}

fn usage() -> &'static str {
    "Usage: sms_pacing_rom_runner --rom PATH --rom-sha256 HEX64 [--emulator-sha SHA] [--suite-id ID] [--suite-revision REV] [--region usa|brazil|europe|japan] [--duration-seconds N] [--warmup-seconds N] [--sample-interval-ms N] [--button1-pulse-frames N,N,...]"
}

fn parse_frame_list(value: &str) -> Result<Vec<u64>, String> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut frames = value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<u64>()
                .map_err(|_| format!("invalid frame number in --button1-pulse-frames: {part}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    frames.sort_unstable();
    if frames.contains(&0) {
        return Err("button pulse frame numbers must be greater than zero".to_owned());
    }
    if frames.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("button pulse frame numbers must be unique".to_owned());
    }
    Ok(frames)
}

fn parse_args() -> Result<Args, String> {
    let mut args = env::args().skip(1);
    let mut rom_path = None;
    let mut rom_sha256 = None;
    let mut emulator_sha = String::from("unknown");
    let mut suite_id = String::from("unspecified");
    let mut suite_revision = String::from("unspecified");
    let mut region = String::from("usa");
    let mut duration_seconds = 300;
    let mut warmup_seconds = 10;
    let mut sample_interval_ms = 1_000;
    let mut button1_pulse_frames = Vec::new();

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
            "--duration-seconds" => {
                duration_seconds = value
                    .parse()
                    .map_err(|_| format!("invalid --duration-seconds value: {value}"))?;
            }
            "--warmup-seconds" => {
                warmup_seconds = value
                    .parse()
                    .map_err(|_| format!("invalid --warmup-seconds value: {value}"))?;
            }
            "--sample-interval-ms" => {
                sample_interval_ms = value
                    .parse()
                    .map_err(|_| format!("invalid --sample-interval-ms value: {value}"))?;
            }
            "--button1-pulse-frames" => button1_pulse_frames = parse_frame_list(&value)?,
            _ => return Err(format!("unknown argument {flag}\n{}", usage())),
        }
    }

    let rom_path = rom_path.ok_or_else(|| format!("--rom is required\n{}", usage()))?;
    let rom_sha256 = rom_sha256.ok_or_else(|| format!("--rom-sha256 is required\n{}", usage()))?;
    if rom_sha256.len() != 64 || !rom_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("--rom-sha256 must contain exactly 64 hexadecimal characters".to_owned());
    }
    if duration_seconds == 0 || duration_seconds > 3_600 {
        return Err("--duration-seconds must be between 1 and 3600".to_owned());
    }
    if warmup_seconds > 300 {
        return Err("--warmup-seconds must not exceed 300".to_owned());
    }
    if sample_interval_ms < 250 || sample_interval_ms > 10_000 {
        return Err("--sample-interval-ms must be between 250 and 10000".to_owned());
    }
    if !matches!(region.as_str(), "usa" | "brazil" | "europe" | "japan") {
        return Err(format!("unsupported region '{region}'"));
    }

    Ok(Args {
        rom_path,
        rom_sha256: rom_sha256.to_ascii_lowercase(),
        emulator_sha,
        suite_id,
        suite_revision,
        region,
        duration_seconds,
        warmup_seconds,
        sample_interval_ms,
        button1_pulse_frames,
    })
}

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

fn emit_sample(
    elapsed: Duration,
    worker: &SmsTemporalWorker,
    sample_index: u64,
    previous_frame: &mut u64,
    previous_sample: &mut Instant,
) {
    let metrics = worker.frame_pacing_metrics();
    let now = Instant::now();
    let frames = worker.frame_number();
    let interval = now.saturating_duration_since(*previous_sample);
    let delta_frames = frames.saturating_sub(*previous_frame);
    let observed_fps = if interval.is_zero() {
        0.0
    } else {
        delta_frames as f64 / interval.as_secs_f64()
    };

    println!(
        "{{\"record\":\"sample\",\"sample_index\":{},\"elapsed_ns\":{},\"frame_number\":{},\"observed_fps\":{:.3},\"frame_interval_p50_ns\":{},\"frame_interval_p95_ns\":{},\"frame_interval_p99_ns\":{},\"step_frame_p50_ns\":{},\"step_frame_p95_ns\":{},\"step_frame_p99_ns\":{},\"frame_processing_p50_ns\":{},\"frame_processing_p95_ns\":{},\"frame_processing_p99_ns\":{},\"frame_budget_p50_ns\":{},\"pacing_wait_budget_p50_ns\":{},\"pacing_wait_elapsed_p50_ns\":{},\"pacing_timeout_lateness_p50_ns\":{},\"pacing_timeout_lateness_p95_ns\":{},\"pacing_timeout_lateness_p99_ns\":{},\"pacing_timeout_count_total\":{},\"machine_ticks_per_frame_min\":{},\"machine_ticks_per_frame_p50\":{},\"machine_ticks_per_frame_max\":{},\"audio_samples_p50\":{},\"audio_samples_p95\":{},\"audio_samples_p99\":{},\"window_sample_count\":{},\"total_frame_samples\":{},\"total_instructions\":{},\"machine_time_ticks\":{},\"machine_clock_hz\":{}}}",
        sample_index,
        elapsed.as_nanos(),
        frames,
        observed_fps,
        metrics.frame_interval_p50_ns,
        metrics.frame_interval_p95_ns,
        metrics.frame_interval_p99_ns,
        metrics.step_frame_p50_ns,
        metrics.step_frame_p95_ns,
        metrics.step_frame_p99_ns,
        metrics.frame_processing_p50_ns,
        metrics.frame_processing_p95_ns,
        metrics.frame_processing_p99_ns,
        metrics.frame_budget_p50_ns,
        metrics.pacing_wait_budget_p50_ns,
        metrics.pacing_wait_elapsed_p50_ns,
        metrics.pacing_timeout_lateness_p50_ns,
        metrics.pacing_timeout_lateness_p95_ns,
        metrics.pacing_timeout_lateness_p99_ns,
        metrics.pacing_timeout_count_total,
        metrics.machine_ticks_per_frame_min,
        metrics.machine_ticks_per_frame_p50,
        metrics.machine_ticks_per_frame_max,
        metrics.audio_samples_p50,
        metrics.audio_samples_p95,
        metrics.audio_samples_p99,
        metrics.sample_count,
        metrics.total_samples,
        worker.total_instructions(),
        worker.machine_time_ticks(),
        worker.machine_clock_hz(),
    );

    *previous_frame = frames;
    *previous_sample = now;
}

fn run(args: Args) -> Result<(), String> {
    let rom = fs::read(&args.rom_path)
        .map_err(|error| format!("cannot read ROM '{}': {error}", args.rom_path))?;
    if rom.is_empty() {
        return Err("ROM image is empty".to_owned());
    }

    let mut worker = SmsTemporalWorker::new(rom, &args.region)?;
    worker.set_diagnostics_enabled(true);

    if args.warmup_seconds > 0 {
        worker.start()?;
        thread::sleep(Duration::from_secs(args.warmup_seconds));
        worker.pause()?;
        worker.reset()?;
    }

    println!(
        "{{\"record\":\"start\",\"status\":\"RUNNING\",\"runner\":\"sms-temporal-worker\",\"pacing_path\":\"production_worker_recv_timeout\",\"video_output\":\"disabled\",\"physical_audio_output\":\"disabled\",\"suite_id\":\"{}\",\"suite_revision\":\"{}\",\"rom_sha256\":\"{}\",\"emulator_sha\":\"{}\",\"region\":\"{}\",\"duration_seconds\":{},\"warmup_seconds\":{},\"sample_interval_ms\":{},\"button1_pulse_frames\":\"{}\",\"machine_clock_hz\":{}}}",
        json_escape(&args.suite_id),
        json_escape(&args.suite_revision),
        args.rom_sha256,
        json_escape(&args.emulator_sha),
        json_escape(&args.region),
        args.duration_seconds,
        args.warmup_seconds,
        args.sample_interval_ms,
        args.button1_pulse_frames
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(","),
        worker.machine_clock_hz(),
    );

    worker.start()?;
    let capture_started = Instant::now();
    let capture_duration = Duration::from_secs(args.duration_seconds);
    let sample_interval = Duration::from_millis(args.sample_interval_ms);
    let mut next_sample = sample_interval;
    let mut sample_index = 0_u64;
    let mut previous_frame = worker.frame_number();
    let mut previous_sample = capture_started;
    let mut next_pulse = 0_usize;
    let mut release_after_frame = None;
    let mut button_pressed = false;

    while capture_started.elapsed() < capture_duration {
        let elapsed = capture_started.elapsed();
        let current_frame = worker.frame_number();

        if let Some(release_frame) = release_after_frame
            && current_frame >= release_frame
        {
            worker.set_controller(1, ControllerButtons::default())?;
            release_after_frame = None;
            button_pressed = false;
        }

        if !button_pressed
            && let Some(target_frame) = args.button1_pulse_frames.get(next_pulse)
            && current_frame >= *target_frame
        {
            worker.set_controller(
                1,
                ControllerButtons {
                    button_1: true,
                    ..ControllerButtons::default()
                },
            )?;
            release_after_frame = Some(current_frame.saturating_add(1));
            button_pressed = true;
            next_pulse += 1;
        }

        if elapsed >= next_sample {
            sample_index += 1;
            emit_sample(
                elapsed,
                &worker,
                sample_index,
                &mut previous_frame,
                &mut previous_sample,
            );
            next_sample = next_sample.saturating_add(sample_interval);
        }

        if let Some(error) = worker.runtime_error() {
            return Err(format!("SMS worker reported a runtime error: {error}"));
        }
        thread::sleep(Duration::from_millis(1));
    }

    if button_pressed {
        worker.set_controller(1, ControllerButtons::default())?;
    }
    worker.pause()?;

    let metrics = worker.frame_pacing_metrics();
    let elapsed = capture_started.elapsed();
    let frames = worker.frame_number();
    let fps = if elapsed.is_zero() {
        0.0
    } else {
        frames as f64 / elapsed.as_secs_f64()
    };
    let runtime_error = worker.runtime_error();
    let status = if runtime_error.is_some() { "RUNTIME_ERROR" } else { "CAPTURED" };

    println!(
        "{{\"record\":\"summary\",\"status\":\"{}\",\"elapsed_ns\":{},\"completed_frames\":{},\"observed_fps\":{:.3},\"frame_interval_p50_ns\":{},\"frame_interval_p95_ns\":{},\"frame_interval_p99_ns\":{},\"step_frame_p50_ns\":{},\"step_frame_p95_ns\":{},\"step_frame_p99_ns\":{},\"frame_processing_p50_ns\":{},\"frame_processing_p95_ns\":{},\"frame_processing_p99_ns\":{},\"pacing_wait_budget_p50_ns\":{},\"pacing_wait_elapsed_p50_ns\":{},\"pacing_timeout_lateness_p50_ns\":{},\"pacing_timeout_lateness_p95_ns\":{},\"pacing_timeout_lateness_p99_ns\":{},\"pacing_timeout_count_total\":{},\"machine_ticks_per_frame_min\":{},\"machine_ticks_per_frame_p50\":{},\"machine_ticks_per_frame_max\":{},\"audio_samples_p50\":{},\"audio_samples_p95\":{},\"audio_samples_p99\":{},\"window_sample_count\":{},\"total_frame_samples\":{},\"total_instructions\":{},\"machine_time_ticks\":{},\"machine_clock_hz\":{},\"input_pulses_sent\":{},\"runtime_error\":{}}}",
        status,
        elapsed.as_nanos(),
        frames,
        fps,
        metrics.frame_interval_p50_ns,
        metrics.frame_interval_p95_ns,
        metrics.frame_interval_p99_ns,
        metrics.step_frame_p50_ns,
        metrics.step_frame_p95_ns,
        metrics.step_frame_p99_ns,
        metrics.frame_processing_p50_ns,
        metrics.frame_processing_p95_ns,
        metrics.frame_processing_p99_ns,
        metrics.pacing_wait_budget_p50_ns,
        metrics.pacing_wait_elapsed_p50_ns,
        metrics.pacing_timeout_lateness_p50_ns,
        metrics.pacing_timeout_lateness_p95_ns,
        metrics.pacing_timeout_lateness_p99_ns,
        metrics.pacing_timeout_count_total,
        metrics.machine_ticks_per_frame_min,
        metrics.machine_ticks_per_frame_p50,
        metrics.machine_ticks_per_frame_max,
        metrics.audio_samples_p50,
        metrics.audio_samples_p95,
        metrics.audio_samples_p99,
        metrics.sample_count,
        metrics.total_samples,
        worker.total_instructions(),
        worker.machine_time_ticks(),
        worker.machine_clock_hz(),
        next_pulse,
        runtime_error
            .as_deref()
            .map(|value| format!("\"{}\"", json_escape(value)))
            .unwrap_or_else(|| "null".to_owned()),
    );

    if status == "RUNTIME_ERROR" {
        return Err(runtime_error.unwrap_or_else(|| "unknown runtime error".to_owned()));
    }
    Ok(())
}

fn main() {
    let result = parse_args().and_then(run);
    if let Err(error) = result {
        eprintln!("{error}");
        process::exit(1);
    }
}
