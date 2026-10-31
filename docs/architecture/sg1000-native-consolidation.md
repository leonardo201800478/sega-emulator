> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust. O código Python antigo de hardware, os testes/ferramentas dependentes e os workflows históricos foram arquivados sob `legacy/`; não são importados nem executados pelo projeto ativo. Este documento mantém observações históricas quando indicadas.

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

CPU instructions currently return nominal base T-states. The machine advances video and audio at instruction boundaries. Z80 immediate IN/OUT are routed to ports using the high byte of A and immediate low byte, but the bus phase inside the instruction is not modeled. CPU `OUT (n),A` writes to PSG and VDP ports are queued at the nominal 11-T-state I/O edge. CPU T-states are converted to configured PSG input clocks using the CPU/PSG clock ratio with a carried fractional remainder. Explicit future PSG and VDP port writes are queued and applied in timestamp order when the machine reaches/passes their nominal T-state; the VDP advances to each queued event before the write is applied.

The default PSG clock and sample rate are initialization defaults, not proof of board-level clock fidelity. Use `Sg1000Machine::new_with_audio` to provide explicit clock/sample-rate values. Verify clock sources and timing behavior before making accuracy claims.

## Acceptance limits

This PR is an incremental port, not a completed standalone emulator:

- The Z80 implementation in `cpu.rs` remains a subset; prefixes, interrupts and many opcodes are not yet implemented.
- The VDP still needs independent tests for all display modes, sprite priority/collision/overflow, status timing and hardware edge cases.
- PSG event timing is nominal and not cycle-exact.
- The PyO3/Qt adapter must be aligned to the native type APIs and validated through Release/Maturin and integration/parity tests.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, then relevant Maturin/Python checks on the final candidate SHA.

## Current evidence

Validation is determined by GitHub checks and review on the final candidate SHA; queued, cancelled, skipped, or failed checks are not PASS. This document records an incremental native-core slice only. A merge of this slice does not authorize declaring the SG-1000 standalone gate complete; the CPU, hardware-reference, and PyO3/Qt integration gates above remain open.


## Host audio session boundary

O PSG e sua fila de amostras pertencem ao hardware Rust. A sessão host nativa faz resampling e entrega ao backend sem exigir transporte PCM no frontend. WASAPI Shared é o padrão Windows e XAudio2 a alternativa. Esta saída não altera a temporização lógica do PSG nem prova precisão ciclo a ciclo. Ver [Runtime de áudio nativo](native-audio-runtime.md).
