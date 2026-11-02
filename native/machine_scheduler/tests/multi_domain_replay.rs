//! Multi-domain scheduler advancement and snapshot/replay integration tests.
//!
//! These tests validate integer MachineTime boundaries only. They do not
//! claim sub-tick event dispatch or device safe-point execution.

use sega_machine_scheduler::{ClockDomainId, MachineScheduler, SchedulerError};
use sega_temporal_core::{ClockError, ClockRatio, HardwarePhase, MachineTime, PlatformPriority};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload {
    token: u64,
}

const TARGETS: [u128; 7] = [1, 1, 2, 5, 5, 8, 13];

fn populated_scheduler() -> MachineScheduler<Payload> {
    let mut scheduler = MachineScheduler::new();

    // Register deliberately out of ID order. Scheduler advancement and
    // exported state must remain canonical by stable domain ID.
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(9), ClockRatio::new(1, 3).unwrap(), 2)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(2), ClockRatio::new(3, 2).unwrap(), 1)
        .unwrap();
    scheduler
        .register_clock_domain_with_phase(ClockDomainId(5), ClockRatio::new(5, 7).unwrap(), 4)
        .unwrap();

    for (index, timestamp) in TARGETS.into_iter().enumerate() {
        scheduler
            .schedule_at_with_phase(
                MachineTime::from_ticks(timestamp),
                HardwarePhase::new((index % 2) as u16),
                PlatformPriority::new((index % 3) as u16),
                Payload {
                    token: index as u64,
                },
            )
            .unwrap();
    }

    scheduler
}

fn expected_domain(
    ratio: ClockRatio,
    initial_phase: u128,
    target: MachineTime,
) -> sega_temporal_core::ClockDomainSnapshot {
    // Independent arithmetic oracle: do not call ClockDomain, which is the
    // implementation exercised by MachineScheduler.
    let numerator = u128::from(ratio.numerator());
    let denominator = u128::from(ratio.denominator());
    let accumulated = target
        .ticks()
        .checked_mul(numerator)
        .and_then(|value| value.checked_add(initial_phase))
        .expect("test oracle arithmetic must fit u128");
    sega_temporal_core::ClockDomainSnapshot {
        time: target,
        total_cycles: accumulated / denominator,
        phase: accumulated % denominator,
    }
}

#[test]
fn multi_domain_advancement_is_chunk_invariant_and_replay_stable() {
    let mut uninterrupted = populated_scheduler();
    let mut replayed = MachineScheduler::restore_state(uninterrupted.export_state()).unwrap();

    let expected_dispatch_tokens = [0_u64, 1, 2, 4, 3, 5, 6];
    let mut observed_dispatch_tokens = Vec::new();
    let mut final_timestamp = MachineTime::ZERO;
    for (index, expected_timestamp) in TARGETS.into_iter().enumerate() {
        let expected_timestamp = MachineTime::from_ticks(expected_timestamp);
        let expected = uninterrupted.dispatch_next().unwrap().unwrap();
        let actual = replayed.dispatch_next().unwrap().unwrap();

        assert_eq!(actual, expected, "dispatch mismatch at event {index}");
        assert_eq!(actual.event.timestamp, expected_timestamp);
        observed_dispatch_tokens.push(actual.event.payload.token);
        final_timestamp = expected_timestamp;

        // Restore after every second dispatch, including same-timestamp
        // dispatches, so both queue history and domain phase are exercised.
        if index % 2 == 1 {
            replayed = MachineScheduler::restore_state(replayed.export_state()).unwrap();
        }
        assert_eq!(
            uninterrupted.export_state(),
            replayed.export_state(),
            "state mismatch after event {index}"
        );
    }

    assert_eq!(observed_dispatch_tokens, expected_dispatch_tokens);
    assert_eq!(uninterrupted.now(), final_timestamp);
    assert_eq!(replayed.now(), final_timestamp);

    // A separate scheduler advances the same domains through irregular idle
    // chunks. Its final rational states must equal event-driven advancement.
    let mut chunked = MachineScheduler::<Payload>::new();
    chunked
        .register_clock_domain_with_phase(ClockDomainId(2), ClockRatio::new(3, 2).unwrap(), 1)
        .unwrap();
    chunked
        .register_clock_domain_with_phase(ClockDomainId(5), ClockRatio::new(5, 7).unwrap(), 4)
        .unwrap();
    chunked
        .register_clock_domain_with_phase(ClockDomainId(9), ClockRatio::new(1, 3).unwrap(), 2)
        .unwrap();

    let domains = [
        (ClockDomainId(2), ClockRatio::new(3, 2).unwrap(), 1),
        (ClockDomainId(5), ClockRatio::new(5, 7).unwrap(), 4),
        (ClockDomainId(9), ClockRatio::new(1, 3).unwrap(), 2),
    ];
    for target in [2, 3, 7, 8, 13] {
        let timestamp = MachineTime::from_ticks(target);
        chunked.advance_idle_to(timestamp).unwrap();

        // Check every intermediate chunk against the independent rational
        // arithmetic oracle, not only the final accumulated state.
        for (id, ratio, phase) in domains {
            assert_eq!(
                chunked.clock_snapshot(id).unwrap(),
                expected_domain(ratio, phase, timestamp),
                "chunked domain mismatch at machine timestamp {target}, domain {}",
                id.0
            );
        }
    }

    for (id, ratio, phase) in domains {
        let expected = expected_domain(ratio, phase, final_timestamp);
        assert_eq!(uninterrupted.clock_snapshot(id).unwrap(), expected);
        assert_eq!(replayed.clock_snapshot(id).unwrap(), expected);
        assert_eq!(chunked.clock_snapshot(id).unwrap(), expected);
    }
}

#[test]
fn multi_domain_preflight_failure_is_atomic_for_every_domain_and_event() {
    let mut scheduler = MachineScheduler::<u8>::new();
    scheduler
        .register_clock_domain(ClockDomainId(1), ClockRatio::new(1, 1).unwrap())
        .unwrap();
    scheduler
        .register_clock_domain(ClockDomainId(2), ClockRatio::new(u64::MAX, 1).unwrap())
        .unwrap();
    scheduler
        .schedule_at(
            MachineTime::from_ticks(u128::MAX),
            PlatformPriority::new(0),
            0xA5,
        )
        .unwrap();

    let before = scheduler.export_state();
    assert_eq!(
        scheduler.dispatch_next(),
        Err(SchedulerError::Clock(ClockError::ArithmeticOverflow))
    );
    assert_eq!(scheduler.export_state(), before);
    assert_eq!(scheduler.pending_events(), 1);
    assert_eq!(scheduler.now(), MachineTime::ZERO);
}
