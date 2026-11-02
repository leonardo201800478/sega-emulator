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
        // exercising equal-time ordering and persisted clock-domain configuration.
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
            assert_eq!(
                actual, expected,
                "dispatch divergence for seed {seed:#018x}"
            );
            if expected.is_none() {
                break;
            }
            dispatched += 1;

            if dispatched.is_multiple_of(4_096) {
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

#[test]
fn sequence_exhaustion_is_rejected_without_mutating_scheduler_state() {
    use sega_temporal_core::EventQueueError;

    let mut scheduler = MachineScheduler::<Payload>::new();
    let mut state = scheduler.export_state();
    state.event_queue.next_sequence = u64::MAX;
    scheduler = MachineScheduler::restore_state(state).unwrap();

    let before = scheduler.export_state();
    let result = scheduler.schedule_at_with_phase(
        MachineTime::ZERO,
        HardwarePhase::new(0),
        PlatformPriority::new(0),
        Payload(0xfeed_beef),
    );

    assert_eq!(
        result,
        Err(sega_machine_scheduler::SchedulerError::EventQueue(
            EventQueueError::SequenceExhausted
        ))
    );
    assert_eq!(scheduler.export_state(), before);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OperationPayload(u64);

struct OperationPayloadCodec;

impl sega_machine_scheduler::codec::EventPayloadCodec<OperationPayload> for OperationPayloadCodec {
    const VERSION: u16 = 1;
    type Error = &'static str;

    fn encode_payload(&self, payload: &OperationPayload) -> Result<Vec<u8>, Self::Error> {
        Ok(payload.0.to_be_bytes().to_vec())
    }

    fn decode_payload(&self, bytes: &[u8]) -> Result<OperationPayload, Self::Error> {
        let encoded: [u8; 8] = bytes
            .try_into()
            .map_err(|_| "payload must contain 8 bytes")?;
        Ok(OperationPayload(u64::from_be_bytes(encoded)))
    }
}

fn operation_scheduler() -> MachineScheduler<OperationPayload> {
    let mut scheduler =
        MachineScheduler::with_delta_event_policy(DeltaEventPolicy::new(32).unwrap());
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(0), ClockRatio::new(1, 3).unwrap(), 0)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(1), ClockRatio::new(3, 2).unwrap(), 1)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(2), ClockRatio::new(5, 3).unwrap(), 2)
        .unwrap();
    scheduler
}

fn codec_restore(
    scheduler: &MachineScheduler<OperationPayload>,
) -> MachineScheduler<OperationPayload> {
    use sega_machine_scheduler::codec::{decode_scheduler_state, encode_scheduler_state};

    let state = scheduler.export_state();
    let encoded = encode_scheduler_state(&state, &OperationPayloadCodec).unwrap();
    let decoded = decode_scheduler_state(&encoded, &OperationPayloadCodec).unwrap();
    let restored = MachineScheduler::restore_state(decoded).unwrap();
    assert_eq!(restored.export_state(), state);
    restored
}

#[test]
#[ignore = "explicit deterministic operation-replay stress profile"]
fn fixed_seed_operation_sequences_match_across_codec_restores() {
    let (seeds, operation_count): (Vec<u64>, u64) =
        match std::env::var("SCHEDULER_STRESS_PROFILE").as_deref() {
            Ok("short") => (vec![0x0a11, 0x0b22, 0x0c33, 0x0d44], 2_000),
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
                20_000,
            ),
            other => panic!("set SCHEDULER_STRESS_PROFILE to short or long; got {other:?}"),
        };

    eprintln!(
        "{} seeds x {} operations = {} total operations",
        seeds.len(),
        operation_count,
        seeds.len() as u128 * u128::from(operation_count)
    );

    for seed in seeds {
        let mut continuous = operation_scheduler();
        let mut restored = codec_restore(&continuous);
        let mut rng = FixedRng(seed);

        for operation in 0..operation_count {
            match rng.next() % 6 {
                0 | 1 if continuous.pending_events() < 512 => {
                    let timestamp = continuous.now().ticks() + u128::from(rng.next() % 64);
                    let phase = u16::try_from(rng.next() % 4).unwrap();
                    let priority = u16::try_from(rng.next() % 8).unwrap();
                    let payload = OperationPayload(rng.next());
                    let left = continuous.schedule_at_with_phase(
                        MachineTime::from_ticks(timestamp),
                        HardwarePhase::new(phase),
                        PlatformPriority::new(priority),
                        payload,
                    );
                    let right = restored.schedule_at_with_phase(
                        MachineTime::from_ticks(timestamp),
                        HardwarePhase::new(phase),
                        PlatformPriority::new(priority),
                        payload,
                    );
                    assert_eq!(left, right, "schedule divergence for seed {seed:#018x}");
                }
                2 | 0 | 1 => {
                    let left = continuous.dispatch_next();
                    let right = restored.dispatch_next();
                    assert_eq!(left, right, "dispatch divergence for seed {seed:#018x}");
                }
                3 => {
                    let target = MachineTime::from_ticks(
                        continuous.now().ticks() + u128::from(rng.next() % 8),
                    );
                    let left = continuous.advance_idle_to(target);
                    let right = restored.advance_idle_to(target);
                    assert_eq!(left, right, "idle-advance divergence for seed {seed:#018x}");
                }
                4 => {
                    let limit = 1 + rng.next() % 32;
                    let policy = DeltaEventPolicy::new(limit).unwrap();
                    continuous.set_delta_event_policy(policy);
                    restored.set_delta_event_policy(policy);
                }
                _ => {
                    assert_eq!(
                        continuous.export_state(),
                        restored.export_state(),
                        "pre-restore divergence for seed {seed:#018x}"
                    );
                    restored = codec_restore(&restored);
                }
            }

            assert_eq!(continuous.now(), restored.now());
            assert_eq!(continuous.pending_events(), restored.pending_events());

            if operation.is_multiple_of(127) {
                assert_eq!(
                    continuous.export_state(),
                    restored.export_state(),
                    "state divergence at operation {operation} for seed {seed:#018x}"
                );
                restored = codec_restore(&restored);
            }
        }

        let unlimited = DeltaEventPolicy::new(u64::MAX).unwrap();
        continuous.set_delta_event_policy(unlimited);
        restored.set_delta_event_policy(unlimited);
        loop {
            let left = continuous.dispatch_next().unwrap();
            let right = restored.dispatch_next().unwrap();
            assert_eq!(left, right, "final drain divergence for seed {seed:#018x}");
            if left.is_none() {
                break;
            }
        }

        assert_eq!(continuous.export_state(), restored.export_state());
        eprintln!("seed {seed:#018x}: {operation_count} operations verified");
    }
}
