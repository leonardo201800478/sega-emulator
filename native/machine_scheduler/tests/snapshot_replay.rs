use sega_machine_scheduler::{
    ClockDomainId, DeltaEventPolicy, MachineDispatch, MachineScheduler, SchedulerError,
};
use sega_temporal_core::{ClockError, ClockRatio, HardwarePhase, MachineTime, PlatformPriority};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayPayload {
    token: u64,
}

#[derive(Debug, Clone, Copy)]
struct FixedSeedRng {
    state: u64,
}

impl FixedSeedRng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

fn build_scheduler(seed: u64) -> MachineScheduler<ReplayPayload> {
    let policy = DeltaEventPolicy::new(64).unwrap();
    let mut scheduler = MachineScheduler::with_delta_event_policy(policy);

    // Deliberately register out of ID order; export and restore must remain canonical.
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(7), ClockRatio::new(3, 2).unwrap(), 1)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(2), ClockRatio::new(5, 3).unwrap(), 2)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(9), ClockRatio::new(1, 7).unwrap(), 6)
        .unwrap();

    let mut rng = FixedSeedRng::new(seed);
    for index in 0..20_u64 {
        // Five events share timestamp 5; the rest exercise later timestamps.
        let timestamp = if index % 4 == 0 {
            5
        } else {
            6 + u128::from(rng.next_u64() % 5)
        };
        let phase = u16::try_from(rng.next_u64() % 3).unwrap();
        let priority = u16::try_from(rng.next_u64() % 4).unwrap();
        let payload = ReplayPayload {
            token: rng.next_u64() ^ index,
        };
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

fn drain(
    scheduler: &mut MachineScheduler<ReplayPayload>,
) -> Vec<Result<Option<MachineDispatch<ReplayPayload>>, SchedulerError>> {
    let mut outcomes = Vec::new();
    loop {
        let outcome = scheduler.dispatch_next();
        let finished = matches!(&outcome, Ok(None) | Err(_));
        outcomes.push(outcome);
        if finished {
            return outcomes;
        }
    }
}

#[test]
fn fixed_seed_replay_matches_continuous_execution_across_independent_snapshots() {
    let seeds = [0x5eed_0001, 0x5eed_0021, 0x5eed_00a5, 0x5eed_ffff];

    for seed in seeds {
        for checkpoint in 0..5 {
            let mut uninterrupted = build_scheduler(seed);

            for _ in 0..checkpoint {
                assert!(uninterrupted.dispatch_next().unwrap().is_some());
            }

            let snapshot = uninterrupted.export_state();
            let mut restored = MachineScheduler::restore_state(snapshot.clone()).unwrap();
            assert_eq!(uninterrupted.export_state(), restored.export_state());

            // A rejected past event must produce the same error and leave both
            // timelines unchanged.
            if uninterrupted.now() > MachineTime::ZERO {
                let past = MachineTime::from_ticks(uninterrupted.now().ticks() - 1);
                let left = uninterrupted.schedule_at_with_phase(
                    past,
                    HardwarePhase::new(0),
                    PlatformPriority::new(0),
                    ReplayPayload { token: u64::MAX },
                );
                let right = restored.schedule_at_with_phase(
                    past,
                    HardwarePhase::new(0),
                    PlatformPriority::new(0),
                    ReplayPayload { token: u64::MAX },
                );
                assert_eq!(left, right);
                assert_eq!(uninterrupted.export_state(), restored.export_state());
            }

            // Allocate one additional future event on both schedulers. Its
            // sequence must be identical after restore.
            let left_sequence = uninterrupted
                .schedule_at_with_phase(
                    MachineTime::from_ticks(20),
                    HardwarePhase::new(0),
                    PlatformPriority::new(0),
                    ReplayPayload { token: seed },
                )
                .unwrap();
            let right_sequence = restored
                .schedule_at_with_phase(
                    MachineTime::from_ticks(20),
                    HardwarePhase::new(0),
                    PlatformPriority::new(0),
                    ReplayPayload { token: seed },
                )
                .unwrap();
            assert_eq!(left_sequence, right_sequence);
            assert_eq!(uninterrupted.export_state(), restored.export_state());

            // Lowering the budget below the already-dispatched count must
            // preserve the same failure. At checkpoint zero, the first event
            // dispatches and the second attempt reaches the same limit.
            let dispatched = uninterrupted.export_state().dispatched_at_timestamp;
            let reduced_limit = dispatched.max(1);
            let reduced_policy = DeltaEventPolicy::new(reduced_limit).unwrap();
            uninterrupted.set_delta_event_policy(reduced_policy);
            restored.set_delta_event_policy(reduced_policy);

            let first_left = uninterrupted.dispatch_next();
            let first_right = restored.dispatch_next();
            assert_eq!(first_left, first_right);

            if matches!(&first_left, Ok(Some(_))) {
                let second_left = uninterrupted.dispatch_next();
                let second_right = restored.dispatch_next();
                assert_eq!(second_left, second_right);
                assert!(matches!(
                    second_left,
                    Err(SchedulerError::DeltaEventLimitExceeded { .. })
                ));
            } else {
                assert!(matches!(
                    first_left,
                    Err(SchedulerError::DeltaEventLimitExceeded { .. })
                ));
            }
            assert_eq!(uninterrupted.export_state(), restored.export_state());

            // Raising the budget resumes both queues. Dispatch values include
            // payloads, timestamps, phase/priority, sequence and clock advances.
            let resumed_policy = DeltaEventPolicy::new(64).unwrap();
            uninterrupted.set_delta_event_policy(resumed_policy);
            restored.set_delta_event_policy(resumed_policy);

            let uninterrupted_trace = drain(&mut uninterrupted);
            let restored_trace = drain(&mut restored);
            assert_eq!(uninterrupted_trace, restored_trace);
            assert_eq!(uninterrupted.export_state(), restored.export_state());
        }
    }
}

#[test]
fn clock_overflow_errors_and_unmodified_state_match_after_restore() {
    let mut uninterrupted = MachineScheduler::<u64>::new();
    uninterrupted
        .register_clock_domain(ClockDomainId(1), ClockRatio::new(u64::MAX, 1).unwrap())
        .unwrap();
    uninterrupted
        .schedule_at_with_phase(
            MachineTime::from_ticks(u128::MAX),
            HardwarePhase::new(0),
            PlatformPriority::new(0),
            0xfeed_beef,
        )
        .unwrap();

    let snapshot = uninterrupted.export_state();
    let mut restored = MachineScheduler::restore_state(snapshot.clone()).unwrap();
    let before_uninterrupted = uninterrupted.export_state();
    let before_restored = restored.export_state();

    let left = uninterrupted.dispatch_next();
    let right = restored.dispatch_next();
    assert_eq!(left, right);
    assert!(matches!(
        left,
        Err(SchedulerError::Clock(ClockError::ArithmeticOverflow))
    ));

    // Overflow is atomic: no event is popped and no domain is partially advanced.
    assert_eq!(uninterrupted.export_state(), before_uninterrupted);
    assert_eq!(restored.export_state(), before_restored);
    assert_eq!(uninterrupted.export_state(), restored.export_state());
}
