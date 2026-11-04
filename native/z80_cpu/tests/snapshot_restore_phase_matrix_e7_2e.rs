use std::collections::BTreeSet;

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
        && matches!(
            program.get(1).copied(),
            Some(0xA2 | 0xA3 | 0xAA | 0xAB | 0xB2 | 0xB3 | 0xBA | 0xBB)
        )
    {
        cpu.registers.b = 1;
        cpu.registers.c = 0;
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
    phases.insert(phase_name(&cpu.export_state().phase));
    let mut events = Vec::new();
    let mut step = cpu.begin_instruction().expect("begin instruction");

    for _ in 0..256 {
        let state = cpu.export_state();
        phases.insert(phase_name(&state.phase));

        if restore_each_boundary {
            let encoded = state.encode().expect("encode valid in-flight state");
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
) -> (BTreeSet<String>, bool) {
    let continuous = execute_one(program, flags, start_pc, interrupt, false);
    let restored = execute_one(program, flags, start_pc, interrupt, true);

    match (continuous, restored) {
        (
            Err(Z80MicroSequencerError::UnsupportedOpcode { .. }),
            Err(Z80MicroSequencerError::UnsupportedOpcode { .. }),
        ) => (BTreeSet::new(), false),
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

            (restored.phases, true)
        }
        (continuous, restored) => panic!(
            "continuous/restored execution diverged for {program:02X?}: {continuous:?} vs {restored:?}"
        ),
    }
}

#[test]
fn e7_2e_every_generated_phase_round_trips_and_continues_equivalently() {
    let mut observed_phases = BTreeSet::new();
    let mut supported_cases = 0_usize;
    let mut unsupported_cases = 0_usize;

    for opcode in 0_u8..=u8::MAX {
        for flags in [0x00, 0xFF] {
            let (phases, supported) =
                assert_continuous_equals_restored(&[opcode, 0, 0, 0, 0], flags, 0, None);
            observed_phases.extend(phases);
            if supported {
                supported_cases += 1;
            } else {
                unsupported_cases += 1;
            }
        }

        let (phases, supported) =
            assert_continuous_equals_restored(&[0xCB, opcode, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported) =
            assert_continuous_equals_restored(&[0xED, opcode, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported) =
            assert_continuous_equals_restored(&[0xDD, opcode, 0, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported) =
            assert_continuous_equals_restored(&[0xFD, opcode, 0, 0, 0], 0xFF, 0, None);
        observed_phases.extend(phases);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported) =
            assert_continuous_equals_restored(&[0xDD, 0xCB, 0, opcode], 0xFF, 0, None);
        observed_phases.extend(phases);
        if supported {
            supported_cases += 1;
        } else {
            unsupported_cases += 1;
        }
        let (phases, supported) =
            assert_continuous_equals_restored(&[0xFD, 0xCB, 0, opcode], 0xFF, 0, None);
        observed_phases.extend(phases);
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
        let (phases, supported) =
            assert_continuous_equals_restored(&[0, 0, 0, 0], 0, 0x2345, Some(interrupt));
        assert!(
            supported,
            "interrupt entry must be supported: {interrupt:?}"
        );
        observed_phases.extend(phases);
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
}
