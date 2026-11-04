use std::collections::{BTreeMap, BTreeSet};

use sega_z80_cpu::{
    Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerError, Z80MicroSequencerPhase,
    Z80MicroSequencerState, Z80MicroStep,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct BusEvent {
    operation: Z80BusOperation,
    address: u16,
    response: Option<u8>,
}

#[derive(Debug)]
struct RunResult {
    terminal: Z80MicroStep,
    state: Z80MicroSequencerState,
    events: Vec<BusEvent>,
    phases: BTreeSet<String>,
    snapshots: Vec<Z80MicroSequencerState>,
}

#[derive(Debug, Clone, Copy)]
enum InterruptSetup {
    Nmi,
    Int { mode: u8, vector: u8 },
}

fn phase_name(phase: &Z80MicroSequencerPhase) -> String {
    let debug = format!("{phase:?}");
    debug
        .split_once(" {")
        .map_or(debug.as_str(), |(name, _)| name)
        .to_owned()
}

fn execute_one(
    program: &[u8],
    flags: u8,
    start_pc: u16,
    interrupt: Option<InterruptSetup>,
    restore_each_boundary: bool,
) -> Result<RunResult, Z80MicroSequencerError> {
    let mut memory = vec![0_u8; 0x1_0000];
    memory[start_pc as usize..start_pc as usize + program.len()].copy_from_slice(program);
    memory[0x8000] = 0xA5;
    memory[0x8001] = 0x3C;
    memory[0x8100] = 0x5A;
    memory[0x8200] = 0xC3;
    memory[0x2080] = 0x34;
    memory[0x2081] = 0x12;

    let mut cpu = Z80MicroSequencer::default();
    cpu.registers.pc = start_pc;
    cpu.registers.sp = 0xFFFE;
    cpu.registers.a = 0x69;
    cpu.registers.f = flags;
    cpu.registers.b = 0;
    cpu.registers.c = 1;
    cpu.registers.d = 0x83;
    cpu.registers.e = 0;
    cpu.registers.h = 0x80;
    cpu.registers.l = 0;
    cpu.ix = 0x8100;
    cpu.iy = 0x8200;
    cpu.i = 0x20;

    if program.first() == Some(&0xED)
        && matches!(program.get(1).copied(), Some(0xA2 | 0xA3 | 0xAA | 0xAB))
    {
        cpu.registers.b = 1;
        cpu.registers.c = 0;
    }
    if program.first() == Some(&0xED) {
        match program.get(1).copied() {
            Some(0xB0 | 0xB1 | 0xB8 | 0xB9) => {
                cpu.registers.b = 0;
                cpu.registers.c = 2;
            }
            Some(0xB2 | 0xB3 | 0xBA | 0xBB) => {
                cpu.registers.b = 2;
                cpu.registers.c = 0;
            }
            _ => {}
        }
    }

    let acknowledge_vector = match interrupt {
        Some(InterruptSetup::Nmi) => {
            cpu.iff1 = true;
            cpu.iff2 = true;
            cpu.request_nmi();
            0xFF
        }
        Some(InterruptSetup::Int { mode, vector }) => {
            cpu.iff1 = true;
            cpu.iff2 = true;
            cpu.interrupt_mode = mode;
            cpu.set_int_line(Some(vector));
            vector
        }
        None => 0xFF,
    };

    let mut phases = BTreeSet::new();
    let initial_state = cpu.export_state();
    phases.insert(phase_name(&initial_state.phase));
    let mut snapshots = vec![initial_state];
    let mut events = Vec::new();
    let mut step = cpu.begin_instruction().expect("begin instruction");

    for _ in 0..256 {
        let state = cpu.export_state();
        phases.insert(phase_name(&state.phase));
        snapshots.push(state.clone());

        if restore_each_boundary {
            let encoded = state.encode().unwrap_or_else(|error| {
                panic!(
                    "encode generated in-flight state for {program:02X?}, phase {:?}: {error:?}; state={state:#?}",
                    state.phase
                )
            });
            let decoded = Z80MicroSequencerState::decode(&encoded).expect("decode in-flight state");
            assert_eq!(
                decoded, state,
                "codec round-trip must preserve the complete DTO"
            );
            cpu = Z80MicroSequencer::restore_state(decoded)
                .expect("every generated valid phase must restore");
        }

        match step {
            Z80MicroStep::BusRequest(request) => {
                let response = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(memory[request.address as usize])
                    }
                    Z80BusOperation::InterruptAcknowledge => Some(acknowledge_vector),
                    Z80BusOperation::PortRead => Some(0x5A),
                    Z80BusOperation::MemoryWrite { value } => {
                        memory[request.address as usize] = value;
                        None
                    }
                    Z80BusOperation::PortWrite { .. } => None,
                };
                events.push(BusEvent {
                    operation: request.operation,
                    address: request.address,
                    response,
                });
                step = match cpu.complete_bus_cycle(request.id, response) {
                    Ok(next) => next,
                    Err(error @ Z80MicroSequencerError::UnsupportedOpcode { .. }) => {
                        return Err(error);
                    }
                    Err(error) => {
                        panic!("complete generated bus request for {program:02X?}: {error:?}")
                    }
                };
            }
            Z80MicroStep::InternalCycle { .. } => {
                step = cpu
                    .complete_internal_cycle()
                    .expect("complete internal T-state");
            }
            Z80MicroStep::InterruptAcknowledge { .. } => {
                step = cpu
                    .begin_instruction()
                    .expect("begin interrupt-mode dispatch");
            }
            terminal @ Z80MicroStep::InstructionRetired { .. } => {
                let state = cpu.export_state();
                phases.insert(phase_name(&state.phase));
                snapshots.push(state.clone());
                if restore_each_boundary {
                    let encoded = state.encode().expect("encode terminal state");
                    let decoded =
                        Z80MicroSequencerState::decode(&encoded).expect("decode terminal state");
                    cpu = Z80MicroSequencer::restore_state(decoded)
                        .expect("restore terminal boundary");
                }
                return Ok(RunResult {
                    terminal,
                    state: cpu.export_state(),
                    events,
                    phases,
                    snapshots,
                });
            }
            terminal @ Z80MicroStep::InterruptServiced { .. } => {
                let state = cpu.export_state();
                phases.insert(phase_name(&state.phase));
                if restore_each_boundary {
                    let encoded = state.encode().expect("encode terminal interrupt state");
                    let decoded = Z80MicroSequencerState::decode(&encoded)
                        .expect("decode terminal interrupt state");
                    cpu = Z80MicroSequencer::restore_state(decoded)
                        .expect("restore terminal interrupt boundary");
                }
                return Ok(RunResult {
                    terminal,
                    state: cpu.export_state(),
                    events,
                    phases,
                    snapshots,
                });
            }
        }
    }

    panic!("instruction did not reach a terminal boundary: {program:02X?}");
}

fn assert_continuous_equals_restored(
    program: &[u8],
    flags: u8,
    start_pc: u16,
    interrupt: Option<InterruptSetup>,
) -> (BTreeSet<String>, bool, BTreeMap<String, Z80MicroSequencerState>) {
    let continuous = execute_one(program, flags, start_pc, interrupt, false);
    let restored = execute_one(program, flags, start_pc, interrupt, true);

    match (continuous, restored) {
        (
            Err(Z80MicroSequencerError::UnsupportedOpcode { .. }),
            Err(Z80MicroSequencerError::UnsupportedOpcode { .. }),
        ) => (BTreeSet::new(), false, BTreeMap::new()),
        (Ok(continuous), Ok(restored)) => {
            assert_eq!(
                restored.terminal, continuous.terminal,
                "terminal microstep: {program:02X?}"
            );
            assert_eq!(
                restored.state, continuous.state,
                "final CPU state: {program:02X?}"
            );
            assert_eq!(
                restored.events, continuous.events,
                "bus sequence: {program:02X?}"
            );
            assert_eq!(
                restored.phases, continuous.phases,
                "phase sequence: {program:02X?}"
            );

            let mut phase_states = BTreeMap::new();
            for state in restored.snapshots {
                phase_states
                    .entry(phase_name(&state.phase))
                    .or_insert(state);
            }
            (restored.phases, true, phase_states)
        }
        (continuous, restored) => panic!(
            "continuous/restored execution diverged for {program:02X?}: {continuous:?} vs {restored:?}"
        ),
    }
}


fn assert_invalid_snapshot(state: Z80MicroSequencerState, context: &str) {
    assert!(
        state.encode().is_err(),
        "invalid DTO must not encode: {context}"
    );
    assert_eq!(
        Z80MicroSequencer::restore_state(state),
        Err(Z80MicroSequencerError::InvalidState),
        "invalid DTO must not restore: {context}"
    );
}

fn mismatched_operation(operation: Z80BusOperation) -> Z80BusOperation {
    match operation {
        Z80BusOperation::OpcodeFetch => Z80BusOperation::MemoryRead,
        Z80BusOperation::MemoryRead => Z80BusOperation::MemoryWrite { value: 0 },
        Z80BusOperation::MemoryWrite { value } => {
            Z80BusOperation::MemoryWrite { value: value ^ 1 }
        }
        Z80BusOperation::PortRead => Z80BusOperation::PortWrite { value: 0 },
        Z80BusOperation::PortWrite { .. } => Z80BusOperation::PortRead,
        Z80BusOperation::InterruptAcknowledge => Z80BusOperation::MemoryRead,
    }
}

fn assert_invalid_mutations(states: BTreeMap<String, Z80MicroSequencerState>) {
    assert_eq!(states.len(), 70, "invalid-state matrix needs one valid fixture per phase");
    let mut mutation_count = 0_usize;

    for (phase, state) in states {
        let mut invalid = state.clone();
        invalid.mcycle_index = u32::MAX;
        assert_invalid_snapshot(invalid, &format!("{phase}: invalid M-cycle index"));
        mutation_count += 1;

        let mut invalid = state.clone();
        invalid.tstate_index = u8::MAX;
        assert_invalid_snapshot(invalid, &format!("{phase}: invalid T-state index"));
        mutation_count += 1;

        if let Some(pending) = state.handshake.pending_request.as_ref() {
            let mut invalid = state.clone();
            let next_request_id = invalid.handshake.next_request_id;
            invalid.handshake.pending_request.as_mut().unwrap().id = next_request_id;
            assert_invalid_snapshot(invalid, &format!("{phase}: request ID not allocated"));
            mutation_count += 1;

            let mut invalid = state.clone();
            invalid.handshake.pending_request.as_mut().unwrap().address ^= 1;
            assert_invalid_snapshot(invalid, &format!("{phase}: request address mismatch"));
            mutation_count += 1;

            let mut invalid = state.clone();
            invalid.handshake.pending_request.as_mut().unwrap().operation =
                mismatched_operation(pending.operation);
            assert_invalid_snapshot(invalid, &format!("{phase}: request operation/payload mismatch"));
            mutation_count += 1;

            let mut invalid = state.clone();
            invalid.handshake.pending_request.as_mut().unwrap().base_tstates = 0;
            assert_invalid_snapshot(invalid, &format!("{phase}: zero-duration bus request"));
            mutation_count += 1;

            let mut invalid = state.clone();
            let request_id = invalid.handshake.pending_request.as_ref().unwrap().id;
            invalid.handshake.next_request_id = request_id;
            assert_invalid_snapshot(invalid, &format!("{phase}: next request ID not monotonic"));
            mutation_count += 1;

            let mut invalid = state.clone();
            invalid.handshake.pending_request = None;
            assert_invalid_snapshot(invalid, &format!("{phase}: required request missing"));
            mutation_count += 1;

            let mut invalid = state.clone();
            invalid.phase = Z80MicroSequencerPhase::Ready;
            assert_invalid_snapshot(invalid, &format!("{phase}: phase/request mismatch"));
            mutation_count += 1;
        } else {
            let mut invalid = state.clone();
            invalid.handshake.wait_tstates = 1;
            assert_invalid_snapshot(invalid, &format!("{phase}: WAIT without pending request"));
            mutation_count += 1;
        }

        if state.opcode.is_some() {
            let mut invalid = state.clone();
            invalid.opcode = None;
            assert_invalid_snapshot(invalid, &format!("{phase}: required opcode missing"));
            mutation_count += 1;
        }
    }

    assert!(
        mutation_count >= 70 * 3,
        "invalid-state matrix must exercise multiple contracts per phase: {mutation_count}"
    );
}

#[test]
fn e7_2e_every_generated_phase_round_trips_and_continues_equivalently() {
    let mut observed_phases = BTreeSet::new();
    let mut valid_phase_states = BTreeMap::new();
    let mut supported_cases = 0_usize;
    let mut unsupported_cases = 0_usize;

    for opcode in 0_u8..=u8::MAX {
        for flags in [0x00, 0xFF] {
            let (phases, supported, snapshots) =
                assert_continuous_equals_restored(&[opcode, 0, 0, 0, 0], flags, 0, None);
            observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
            valid_phase_states.extend(snapshots);
            if supported {
                supported_cases += 1;
            } else {
                unsupported_cases += 1;
            }
        }

        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0xCB, opcode, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0xED, opcode, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0xDD, opcode, 0, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0xFD, opcode, 0, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0xDD, 0xCB, 0, opcode], 0xFF, 0, None);
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0xFD, 0xCB, 0, opcode], 0xFF, 0, None);
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
    }

    for interrupt in [
        InterruptSetup::Nmi,
        InterruptSetup::Int {
            mode: 0,
            vector: 0xC7,
        },
        InterruptSetup::Int {
            mode: 1,
            vector: 0xFF,
        },
        InterruptSetup::Int {
            mode: 2,
            vector: 0x80,
        },
    ] {
        let (phases, supported, snapshots) =
            assert_continuous_equals_restored(&[0, 0, 0, 0], 0, 0x2345, Some(interrupt));
        assert!(
            supported,
            "interrupt entry must be supported: {interrupt:?}"
        );
        observed_phases.extend(phases);
        valid_phase_states.extend(snapshots);
    }

    assert!(
        supported_cases > 0,
        "the corpus must exercise supported instructions"
    );
    assert!(
        unsupported_cases < 7 * 256 + 4,
        "too many opcode cases are unsupported: {unsupported_cases}"
    );
    assert_eq!(
        observed_phases.len(),
        70,
        "expected all 70 sequencer phases to be exercised by full-state restore; observed: {observed_phases:#?}"
    );
    assert_eq!(valid_phase_states.len(), 70);
    assert_invalid_mutations(valid_phase_states);
}
