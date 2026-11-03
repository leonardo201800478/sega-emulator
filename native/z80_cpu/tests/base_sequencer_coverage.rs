//! Explicit baseline inventory for the resumable Z80 BASE decoder.
//!
//! This test deliberately distinguishes the atomic executor's broad BASE
//! coverage from the current microsequencer subset. It is a gap ledger, not a
//! claim that the currently supported subset is semantically or temporally
//! complete. Update the expected inventory only alongside an implementation
//! slice and its differential/temporal evidence.

use sega_z80_cpu::{
    Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerError, Z80MicroStep,
};

const BASE_PREFIXES: [u8; 4] = [0xCB, 0xDD, 0xED, 0xFD];
const EXPECTED_IMPLEMENTED_BASE_ENCODINGS: usize = 178;
const EXPECTED_UNSUPPORTED_BASE_ENCODINGS: usize = 74;

fn is_base_prefix(opcode: u8) -> bool {
    BASE_PREFIXES.contains(&opcode)
}

/// Mirror the explicit handler families currently present in
/// Z80MicroSequencer::complete_bus_cycle_inner.
///
/// Keeping this list independent from the runtime dispatch makes an
/// implementation change visible to the inventory test.
fn expected_base_handler(opcode: u8) -> bool {
    matches!(
        opcode,
        0x00
            | 0x07
            | 0x0F
            | 0x17
            | 0x1F
            | 0x27
            | 0x2F
            | 0x37
            | 0x3F
            | 0xE9
            | 0xEB
            | 0x02
            | 0x12
            | 0x0A
            | 0x1A
            | 0x01
            | 0x11
            | 0x21
            | 0x31
            | 0x06
            | 0x0E
            | 0x16
            | 0x1E
            | 0x26
            | 0x2E
            | 0x3E
            | 0x36
            | 0xC6
            | 0xCE
            | 0xD6
            | 0xDE
            | 0xE6
            | 0xEE
            | 0xF6
            | 0xFE
    ) || ((0x04..=0x3D).contains(&opcode)
        && matches!(opcode & 0xC7, 0x04 | 0x05))
        || ((0x40..=0x7F).contains(&opcode) && opcode != 0x76)
        || (0x80..=0xBF).contains(&opcode)
}

/// Drive one non-prefixed BASE instruction until retirement or explicit
/// unsupported-decode rejection. Reads receive stable fixture data and writes
/// are acknowledged without data; no hardware timing claim is made here.
fn execute_base_opcode(
    opcode: u8,
) -> Result<u32, Z80MicroSequencerError> {
    let mut cpu = Z80MicroSequencer::default();
    let mut step = cpu.begin_instruction()?;
    let mut first_opcode_fetch = true;

    for _ in 0..128 {
        step = match step {
            Z80MicroStep::BusRequest(request) => {
                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch if first_opcode_fetch => {
                        first_opcode_fetch = false;
                        Some(opcode)
                    }
                    Z80BusOperation::OpcodeFetch => Some(0x00),
                    Z80BusOperation::MemoryRead | Z80BusOperation::PortRead => Some(0x5A),
                    Z80BusOperation::MemoryWrite { .. }
                    | Z80BusOperation::PortWrite { .. } => None,
                };
                cpu.complete_bus_cycle(request.id, data)?
            }
            Z80MicroStep::InternalCycle { .. } => cpu.complete_internal_cycle()?,
            Z80MicroStep::InstructionRetired { tstates, .. } => return Ok(tstates),
        };
    }

    panic!("BASE opcode {opcode:02X} did not retire within 128 micro-operations");
}

#[test]
fn base_prefix_bytes_are_recognized_as_prefix_fetches() {
    for prefix in BASE_PREFIXES {
        let mut cpu = Z80MicroSequencer::default();
        let fetch = match cpu.begin_instruction().unwrap() {
            Z80MicroStep::BusRequest(request) => request,
            other => panic!("expected initial M1 request, got {other:?}"),
        };
        let next = cpu.complete_bus_cycle(fetch.id, Some(prefix)).unwrap();
        assert!(
            matches!(
                next,
                Z80MicroStep::BusRequest(request)
                    if request.operation == Z80BusOperation::OpcodeFetch
            ),
            "prefix {prefix:02X} must request the following opcode byte"
        );
    }
}

#[test]
fn base_microsequencer_dispatch_matches_the_explicit_gap_baseline() {
    let mut implemented = 0usize;
    let mut unsupported = 0usize;

    for opcode in u8::MIN..=u8::MAX {
        if is_base_prefix(opcode) {
            continue;
        }

        match (expected_base_handler(opcode), execute_base_opcode(opcode)) {
            (true, Ok(tstates)) => {
                assert!(
                    tstates > 0,
                    "implemented BASE opcode {opcode:02X} retired with zero T-states"
                );
                implemented += 1;
            }
            (false, Err(Z80MicroSequencerError::UnsupportedOpcode {
                opcode: rejected,
                ..
            })) => {
                assert_eq!(rejected, opcode);
                unsupported += 1;
            }
            (true, Err(error)) => {
                panic!("BASE opcode {opcode:02X} is expected to be implemented: {error:?}")
            }
            (false, Ok(tstates)) => {
                panic!(
                    "BASE opcode {opcode:02X} unexpectedly retired in {tstates} T-states; update                      the inventory with its semantic and temporal evidence"
                )
            }
            (false, Err(error)) => {
                panic!(
                    "BASE opcode {opcode:02X} failed for a reason other than explicit                      unsupported decode: {error:?}"
                )
            }
        }
    }

    assert_eq!(
        implemented, EXPECTED_IMPLEMENTED_BASE_ENCODINGS,
        "update the implemented BASE baseline only with the corresponding implementation"
    );
    assert_eq!(
        unsupported, EXPECTED_UNSUPPORTED_BASE_ENCODINGS,
        "the unsupported BASE gap must remain explicit"
    );
}
