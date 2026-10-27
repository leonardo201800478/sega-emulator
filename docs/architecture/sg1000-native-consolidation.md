# SG-1000 native core consolidation — PR #126

## Purpose

Keep the emulated hardware runtime in Rust without PyO3 types in `native/sg1000_core/src`. PyO3 is an adapter boundary for the frontend, not a dependency of hardware state.

## Integrated in this slice

- `alu.rs`: native Z80 8-bit ALU operations used by the SG-1000 CPU subset.
- `bus.rs`: adapter for the shared memory/port bus trait.
- `io.rs`: active-low controller bytes and ordered PSG-write records.
- `memory.rs`: ROM/RAM mapping.
- `psg.rs`: pure Rust SN76489 digital synthesis and sample queue.
- `vdp.rs`: pure Rust TMS9918A state, VRAM/register protocol, buffered reads, raster counter, VBlank/status, framebuffer and rendering helpers.
- `machine.rs`: split-borrow bus routes CPU memory, controller input, VDP ports and PSG writes; the machine advances VDP/PSG after each CPU instruction and exposes drained audio samples.

## Timing contract

CPU instructions currently return nominal base T-states. The machine advances video and audio at instruction boundaries. Z80 immediate IN/OUT are routed to ports using the high byte of A and immediate low byte, but the bus phase inside the instruction is not modeled. PSG writes scheduled at future nominal T-states are applied at the first machine step that reaches/passes that timestamp, advancing the PSG to the event before applying it.

The default PSG clock and sample rate are initialization defaults, not proof of board-level clock fidelity. Use `Sg1000Machine::new_with_audio` to provide explicit clock/sample-rate values. Verify clock sources and timing behavior before making accuracy claims.

## Acceptance limits

This PR is an incremental port, not a completed standalone emulator:

- The Z80 implementation in `cpu.rs` remains a subset; prefixes, interrupts and many opcodes are not yet implemented.
- The VDP still needs independent tests for all display modes, sprite priority/collision/overflow, status timing and hardware edge cases.
- PSG event timing is nominal and not cycle-exact.
- The PyO3/Qt adapter must be aligned to the native type APIs and validated through Release/Maturin and integration/parity tests.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, then relevant Maturin/Python checks on the final candidate SHA.

## Current evidence

The GitHub workspace workflow initially failed on formatting; fixes were pushed and validation reruns are in progress. Do not treat queued, cancelled, skipped, or failed checks as PASS. No merge is authorized by this document.
