use std::{
    env,
    error::Error,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

use sega_sn76489::{PsgProfile, Sn76489Core};

const CLOCK_HZ: u64 = 3_579_545;
const SAMPLE_RATE: u64 = 48_000;
const TOTAL_CLOCKS: u64 = 16_000_000;

// Clock-stamped PSG register writes. Keep this sequence identical when running
// the tool against the historical baseline and the current implementation.
const TRACE: [(u64, u8); 19] = [
    (0, 0x85),             // Tone 0 period low = 5.
    (0, 0x12),             // Tone 0 period high = 0x12.
    (0, 0x90),             // Tone 0 full volume.
    (0, 0xA8),             // Tone 1 period low = 8.
    (0, 0x05),             // Tone 1 period high = 5.
    (0, 0xB4),             // Tone 1 attenuation = 4.
    (0, 0xC0),             // Tone 2 period low = 0.
    (0, 0x10),             // Tone 2 period high = 1.
    (0, 0xD8),             // Tone 2 attenuation = 8.
    (0, 0xE4),             // White noise, fixed divider.
    (0, 0xF6),             // Noise attenuation = 6.
    (2_000_000, 0x94),     // Change tone 0 volume during playback.
    (4_000_000, 0xE7),     // White noise, tone-2-derived divider.
    (6_000_000, 0xF2),     // Raise noise level during playback.
    (8_000_000, 0x89),     // Change tone 0 period low nibble.
    (8_000_000, 0x11),     // Change tone 0 period high bits.
    (10_000_000, 0xB7),    // Change tone 1 attenuation.
    (12_000_000, 0xE4),    // Reset white-noise generator with fixed divider.
    (14_000_000, 0xF4),    // Change noise attenuation.
];

fn advance(psg: &mut Sn76489Core, mut clocks: u64, chunk_limit: Option<u64>) {
    while clocks > 0 {
        let step = chunk_limit
            .map(|limit| clocks.min(limit))
            .unwrap_or(clocks);
        psg.advance_clocks(step);
        clocks -= step;
    }
}

fn write_wav(path: &PathBuf, samples: &[f64]) -> Result<(), Box<dyn Error>> {
    let data_bytes = u32::try_from(samples.len() * 4)?;
    let riff_size = 36_u32.checked_add(data_bytes).ok_or("WAV too large")?;
    let mut writer = BufWriter::new(File::create(path)?);

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&3_u16.to_le_bytes())?; // IEEE float PCM.
    writer.write_all(&1_u16.to_le_bytes())?; // Original PSG source is mono.
    writer.write_all(&(SAMPLE_RATE as u32).to_le_bytes())?;
    writer.write_all(&((SAMPLE_RATE as u32) * 4).to_le_bytes())?;
    writer.write_all(&4_u16.to_le_bytes())?;
    writer.write_all(&32_u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_bytes.to_le_bytes())?;

    for sample in samples {
        writer.write_all(&(*sample as f32).to_le_bytes())?;
    }
    writer.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let profile_name = args.next().ok_or("usage: psg_trace <sms|sg1000> <output.wav> [chunk-clocks]")?;
    let output_path = PathBuf::from(
        args.next().ok_or("usage: psg_trace <sms|sg1000> <output.wav> [chunk-clocks]")?,
    );
    let chunk_limit = args
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .filter(|value| *value > 0);

    let profile = match profile_name.as_str() {
        "sms" => PsgProfile::Sms,
        "sg1000" => PsgProfile::Sg1000,
        _ => return Err("profile must be 'sms' or 'sg1000'".into()),
    };

    let mut psg = Sn76489Core::new_with_profile(CLOCK_HZ, SAMPLE_RATE, profile)?;
    let mut elapsed = 0_u64;
    for (clock, value) in TRACE {
        advance(&mut psg, clock - elapsed, chunk_limit);
        psg.write(value);
        elapsed = clock;
    }
    advance(&mut psg, TOTAL_CLOCKS - elapsed, chunk_limit);

    let samples = psg.drain_samples(None);
    let peak = samples.iter().map(|sample| sample.abs()).fold(0.0_f64, f64::max);
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let rms = (samples.iter().map(|sample| sample * sample).sum::<f64>()
        / samples.len() as f64)
        .sqrt();
    let checksum = samples.iter().fold(0xcbf29ce484222325_u64, |hash, sample| {
        sample.to_bits().to_le_bytes().into_iter().fold(hash, |state, byte| {
            (state ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
    });

    write_wav(&output_path, &samples)?;
    println!("profile={profile_name}");
    println!("clock_hz={CLOCK_HZ}");
    println!("sample_rate={SAMPLE_RATE}");
    println!("channels=1 (raw PSG; dual-mono is a host concern)");
    println!("total_clocks={TOTAL_CLOCKS}");
    println!("samples={}", samples.len());
    println!("duration_seconds={:.9}", samples.len() as f64 / SAMPLE_RATE as f64);
    println!("peak={peak:.12}");
    println!("rms={rms:.12}");
    println!("mean={mean:.12}");
    println!("fnv1a64={checksum:016x}");
    println!("wav={}", output_path.display());
    Ok(())
}
