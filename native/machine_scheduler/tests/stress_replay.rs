//! Opt-in deterministic stress replay for the machine scheduler.
//!
//! Run with SCHEDULER_STRESS_PROFILE=short|long and:
//! cargo test -p sega-machine-scheduler --test stress_replay -- --ignored --nocapture

use sega_machine_scheduler::{ClockDomainId, DeltaEventPolicy, MachineScheduler};
use sega_temporal_core::{ClockRatio, HardwarePhase, MachineTime, PlatformPriority};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload(u64);

struct FixedRng(u64);

impl FixedRng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

fn populated(seed: u64, count: u64) -> MachineScheduler<Payload> {
    let mut scheduler =
        MachineScheduler::with_delta_event_policy(DeltaEventPolicy::new(64).unwrap());
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(0), ClockRatio::new(1, 3).unwrap(), 0)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(1), ClockRatio::new(3, 2).unwrap(), 1)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(2), ClockRatio::new(5, 3).unwrap(), 2)
        .unwrap();

    let mut rng = FixedRng(seed);
    for index in 0..count {
        // Sixteen events per timestamp keep the dispatch budget bounded while
        // exercising equal-time ordering, all key dimensions and clock ratios.
        let timestamp = u128::from(index / 16 + 1);
        let phase = u16::try_from(rng.next() % 4).unwrap();
        let priority = u16::try_from(rng.next() % 8).unwrap();
        let payload = Payload(rng.next() ^ index);
        scheduler
            .schedule_at_with_phase(
                MachineTime::from_ticks(timestamp),
                HardwarePhase::new(phase),
                PlatformPriority::new(priority),
                payload,
            )
            .unwrap();
    }
    scheduler
}

#[test]
#[ignore = "explicit deterministic stress profile"]
fn fixed_seed_stress_replay_survives_repeated_restores() {
    let (seeds, event_count): (Vec<u64>, u64) =
        match std::env::var("SCHEDULER_STRESS_PROFILE").as_deref() {
            Ok("short") => (vec![0x13, 0x3435_a11c, 0x5e6a_0013, 0xd15a_7c42], 20_000),
            Ok("long") => (
                vec![
                    0x13,
                    0x3435_a11c,
                    0x5e6a_0013,
                    0xd15a_7c42,
                    0x5eed_0001,
                    0x5eed_0021,
                    0x5eed_00a5,
                    0x5eed_ffff,
                    0x1234_5678,
                    0x8765_4321,
                    0xc001_d00d,
                    0xface_b00c,
                    0xdead_beef,
                    0x0bad_f00d,
                    0xa5a5_5a5a,
                    0xf00d_cafe,
                ],
                250_000,
            ),
            other => panic!("set SCHEDULER_STRESS_PROFILE to short or long; got {other:?}"),
        };

    eprintln!(
        "{} seeds x {} events = {} total events",
        seeds.len(),
        event_count,
        seeds.len() as u128 * u128::from(event_count)
    );

    for seed in seeds {
        let initial = populated(seed, event_count).export_state();
        let mut continuous = MachineScheduler::restore_state(initial.clone()).unwrap();
        let mut restored = MachineScheduler::restore_state(initial).unwrap();
        let mut dispatched = 0_u64;

        loop {
            let expected = continuous.dispatch_next().unwrap();
            let actual = restored.dispatch_next().unwrap();
            assert_eq!(actual, expected, "dispatch divergence for seed {seed:#018x}");
            if expected.is_none() {
                break;
            }
            dispatched += 1;

            if dispatched % 4_096 == 0 {
                restored = MachineScheduler::restore_state(restored.export_state()).unwrap();
                assert_eq!(
                    continuous.export_state(),
                    restored.export_state(),
                    "snapshot divergence at dispatch {dispatched} for seed {seed:#018x}"
                );
            }
        }

        assert_eq!(dispatched, event_count);
        assert_eq!(continuous.export_state(), restored.export_state());
        eprintln!("seed {seed:#018x}: {dispatched} dispatches verified");
    }
}
