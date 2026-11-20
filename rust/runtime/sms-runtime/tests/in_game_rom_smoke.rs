//! Opt-in smoke campaign for real, user-supplied SMS ROMs.
//!
//! Run on a machine with legally obtained test ROMs:
//!   $env:SEGA_SMS_TEST_ROM_DIR = "D:\\ROMs\\SMS"
//!   $env:SEGA_SMS_TEST_REGION = "europe" # usa|brazil|europe|japan
//!   $env:SEGA_SMS_SYSTEM = "sms2" # sms1|sms2
//!   $env:SEGA_SMS_TEST_FRAMES = "1800" # optional; default 1800
//!   $env:SEGA_SMS_EXPECT_HEIGHT = "224" # optional; 192|224|240
//!   cargo test --release -p sega-sms-runtime --test in_game_rom_smoke -- --ignored --nocapture
//!
//! ROM files are read in place and are never copied into the repository.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sega_sms_runtime::{DiagnosticsConsent, SmsDebugSnapshot, SmsTemporalWorker, SmsVdpRevision};
use sega_video_runtime::FrameMailbox;

const DEFAULT_FRAMES_PER_ROM: usize = 1800;
const MAX_INSTRUCTIONS_PER_FRAME: usize = 100_000;

fn is_sms_rom(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("sms") || extension.eq_ignore_ascii_case("bin")
        })
}

fn machine_state_digest(snapshot: &SmsDebugSnapshot) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    macro_rules! feed {
        ($value:expr) => {
            for byte in $value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        };
    }

    feed!(snapshot.pc);
    feed!(snapshot.sp);
    feed!(snapshot.af);
    feed!(snapshot.bc);
    feed!(snapshot.de);
    feed!(snapshot.hl);
    feed!(snapshot.af_alt);
    feed!(snapshot.bc_alt);
    feed!(snapshot.de_alt);
    feed!(snapshot.hl_alt);
    feed!(snapshot.ix);
    feed!(snapshot.iy);
    feed!(snapshot.wz);
    feed!(snapshot.instruction_tstates);
    feed!(snapshot.i);
    feed!(snapshot.r);
    feed!(u8::from(snapshot.iff1));
    feed!(u8::from(snapshot.iff2));
    feed!(snapshot.interrupt_mode);
    feed!(u8::from(snapshot.halted));
    feed!(snapshot.machine_time_ticks);
    feed!(snapshot.frame_number);
    feed!(snapshot.vdp_status);
    feed!(snapshot.vdp_scanline);
    feed!(snapshot.vdp_cycles_into_line);
    for bank in snapshot.mapper_banks {
        feed!(bank);
    }
    feed!(snapshot.mapper_control);
    feed!(snapshot.memory_control);
    hash
}

fn frame_digest(bytes: &[u8]) -> u64 {
    // FNV-1a is used only as a compact regression fingerprint, not as a
    // cryptographic identity or integrity check.
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

fn collect_roms(directory: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(directory)
        .unwrap_or_else(|error| {
            panic!("cannot read ROM directory {}: {error}", directory.display())
        })
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("cannot read ROM directory entry: {error}"))
                .path()
        })
        .filter(|path| path.is_file() && is_sms_rom(path))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[test]
#[ignore = "requires user-supplied SMS ROMs; see test module documentation"]
fn smoke_test_user_supplied_sms_roms_in_the_temporal_runtime() {
    let directory = std::env::var_os("SEGA_SMS_TEST_ROM_DIR")
        .map(PathBuf::from)
        .expect("set SEGA_SMS_TEST_ROM_DIR to a directory containing .sms/.bin ROMs");
    let region = std::env::var("SEGA_SMS_TEST_REGION").unwrap_or_else(|_| "usa".to_owned());
    let system = std::env::var("SEGA_SMS_SYSTEM").unwrap_or_else(|_| "sms1".to_owned());
    let vdp_revision = match system.as_str() {
        "sms1" => SmsVdpRevision::Sega3155124,
        "sms2" => SmsVdpRevision::Sega3155246,
        other => panic!("unsupported SEGA_SMS_SYSTEM '{other}'; expected sms1 or sms2"),
    };
    let allowed_heights: &[u32] = match vdp_revision {
        SmsVdpRevision::Sega3155124 => &[192],
        SmsVdpRevision::Sega3155246 => &[192, 224, 240],
    };
    let frames_per_rom = std::env::var("SEGA_SMS_TEST_FRAMES")
        .map(|value| {
            value
                .parse::<usize>()
                .expect("SEGA_SMS_TEST_FRAMES must be an integer")
        })
        .unwrap_or(DEFAULT_FRAMES_PER_ROM);
    assert!(frames_per_rom > 0, "SEGA_SMS_TEST_FRAMES must be positive");
    let expected_height = std::env::var("SEGA_SMS_EXPECT_HEIGHT").ok().map(|value| {
        value
            .parse::<u32>()
            .expect("SEGA_SMS_EXPECT_HEIGHT must be an integer")
    });
    if let Some(height) = expected_height {
        assert!(
            allowed_heights.contains(&height),
            "expected height {height} is not supported by selected system {system}"
        );
    }
    let rom_paths = collect_roms(&directory);

    assert!(
        !rom_paths.is_empty(),
        "no .sms/.bin ROMs found in {}",
        directory.display()
    );

    for path in rom_paths {
        let rom = fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot read ROM {}: {error}", path.display()));
        assert!(!rom.is_empty(), "ROM {} is empty", path.display());

        let worker = SmsTemporalWorker::new_with_hardware_profile_and_cartridge_sram_size(
            rom,
            &region,
            vdp_revision,
            None,
            None,
        )
        .unwrap_or_else(|error| panic!("{}: worker rejected ROM: {error}", path.display()));
        worker.enable_diagnostics(&DiagnosticsConsent::from_explicit_user_action());
        let mailbox = FrameMailbox::new();
        worker
            .attach_video_mailbox(mailbox.clone())
            .unwrap_or_else(|error| {
                panic!("{}: cannot attach video mailbox: {error}", path.display())
            });

        let first_state_digest =
            machine_state_digest(&worker.debug_snapshot().unwrap_or_else(|error| {
                panic!(
                    "{}: initial machine snapshot failed: {error}",
                    path.display()
                )
            }));
        let mut first_digest = None;
        let mut last_digest = 0;
        let mut non_black_frames = 0_u32;
        let mut completed_frames = 0_u64;
        let mut observed_heights = BTreeMap::<u32, u32>::new();

        for frame_index in 0..frames_per_rom {
            let instructions = worker
                .step_frame(MAX_INSTRUCTIONS_PER_FRAME)
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: frame {} failed after {} completed frames: {error}",
                        path.display(),
                        frame_index + 1,
                        completed_frames
                    )
                });
            assert!(
                instructions > 0,
                "{}: frame {} completed without executing instructions",
                path.display(),
                frame_index + 1
            );

            let frame = mailbox
                .take_latest()
                .unwrap_or_else(|error| panic!("{}: mailbox failed: {error}", path.display()))
                .unwrap_or_else(|| {
                    panic!(
                        "{}: runtime completed frame {} without publishing a video frame",
                        path.display(),
                        frame_index + 1
                    )
                });

            assert_eq!(
                frame.width(),
                256,
                "{}: unexpected frame width",
                path.display()
            );
            let height = frame.height();
            assert!(
                allowed_heights.contains(&height),
                "{}: frame height {} is incompatible with selected system {}",
                path.display(),
                height,
                system
            );
            assert_eq!(
                frame.pixels().len(),
                256 * height as usize * 3,
                "{}: unexpected RGB888 frame payload length for {} lines",
                path.display(),
                height
            );
            *observed_heights.entry(height).or_default() += 1;

            let digest = frame_digest(frame.pixels());
            first_digest.get_or_insert(digest);
            last_digest = digest;
            if frame.pixels().iter().any(|channel| *channel != 0) {
                non_black_frames += 1;
            }
            completed_frames = completed_frames.saturating_add(1);

            if let Some(error) = worker.runtime_error() {
                panic!(
                    "{}: runtime error at frame {}: {error}",
                    path.display(),
                    frame_index + 1
                );
            }
        }

        assert!(
            non_black_frames > 0,
            "{}: all {} published frames were black; investigate ROM startup, mapper, CPU and VDP path",
            path.display(),
            frames_per_rom
        );
        if let Some(height) = expected_height {
            assert!(
                observed_heights.contains_key(&height),
                "{}: expected {}-line mode was never published by system {}",
                path.display(),
                height,
                system
            );
        }

        let last_state_digest =
            machine_state_digest(&worker.debug_snapshot().unwrap_or_else(|error| {
                panic!("{}: final machine snapshot failed: {error}", path.display())
            }));
        assert_ne!(
            first_state_digest,
            last_state_digest,
            "{}: machine state did not change during the ROM campaign",
            path.display()
        );

        println!(
            "SMS_IN_GAME_SMOKE rom={} system={} vdp={:?} region={} frames={} heights={:?} instructions={} non_black_frames={} first_frame_fnv1a={:016x} last_frame_fnv1a={:016x} first_state_fnv1a={:016x} last_state_fnv1a={:016x}",
            path.display(),
            system,
            vdp_revision,
            region,
            completed_frames,
            observed_heights,
            worker.total_instructions(),
            non_black_frames,
            first_digest.expect("at least one frame was processed"),
            last_digest,
            first_state_digest,
            last_state_digest
        );

        worker.detach_video_mailbox().unwrap_or_else(|error| {
            panic!(
                "{}: cannot detach video mailbox cleanly: {error}",
                path.display()
            )
        });
        mailbox.close().unwrap_or_else(|error| {
            panic!(
                "{}: cannot close video mailbox cleanly: {error}",
                path.display()
            )
        });
    }
}

#[test]
fn every_supported_region_keeps_its_timing_independent_of_sms_asic_selection() {
    use sega_sms_runtime::{sms_temporal_hardware_profile, sms_temporal_region_profile};

    for (region, expected_clock) in [
        ("usa", 3_579_545),
        ("brazil", 3_579_545),
        ("europe", 3_546_895),
        ("japan", 3_579_545),
    ] {
        for revision in [SmsVdpRevision::Sega3155124, SmsVdpRevision::Sega3155246] {
            let (_, profile, clock_hz) = sms_temporal_hardware_profile(region, revision)
                .unwrap_or_else(|error| panic!("{region}: {error}"));
            assert_eq!(profile.revision, revision);
            assert_eq!(clock_hz, expected_clock);
        }
    }

    // Compatibility constructors retain the legacy SMS1 profile.
    let (_, legacy_default, _) =
        sms_temporal_region_profile("europe").expect("legacy regional profile");
    assert_eq!(legacy_default.revision, SmsVdpRevision::Sega3155124);
    assert!(sms_temporal_hardware_profile("unknown", SmsVdpRevision::Sega3155246).is_err());
}
