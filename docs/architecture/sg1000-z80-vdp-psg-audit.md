# SG-1000 Z80, VDP and PSG: audit and completion plan

## Purpose

Close the functional gaps in the native Rust SG-1000 machine by auditing the existing implementation against the Zilog Z80 programmer's model and documented SG-1000 peripherals. This document is an acceptance ledger, not a claim that the CPU or machine is already complete.

## Baseline observed on `main`

- `native/sg1000_core/src/cpu.rs` explicitly describes itself as an incremental subset and returns `CpuError::UnsupportedOpcode` for unimplemented instructions.
- The CPU currently models only the primary AF/BC/DE/HL registers, SP, PC, HALT and aggregate T-states. Alternate register banks, I/R, interrupt flip-flops/modes, and prefix state are not represented in the public CPU state.
- `native/sg1000_core/src/vdp.rs` and `psg.rs` exist as native modules. Their existence and unit tests alone do not establish complete hardware behavior or cycle-level synchronization.
- The current machine advances devices at instruction boundaries. Do not describe this as bus-cycle-accurate emulation; wait states, contention and sub-instruction event ordering require a separate explicit timing model.

## Progress in the current PR branch

The following implementation slices have been added, but this ledger remains open until independent coverage and integration gates pass on the final candidate SHA:

- Z80 state now includes alternate AF/BC/DE/HL, IX/IY, I/R, IFF1/IFF2, interrupt mode, WZ/MEMPTR, EI delay, NMI and INT request state.
- The primary opcode decoder is extended with the CB matrix, ED arithmetic/load/I/O/block families, and DD/FD index substitution, displacement, repeated-prefix and indexed-CB handling.
- The refresh register increments on M1 opcode fetches and HALT refresh cycles.
- NMI and maskable interrupt service paths cover NMI, IM 1, IM 2, and the common IM 0 RST/NOP acknowledge forms. Other IM 0 data-bus opcodes currently return an explicit error and remain a known limitation.
- The machine synchronizes the VDP's enabled VBlank interrupt line to the Z80 INT request at instruction boundaries.

### Known limitations that prevent a completion claim

- The scheduler still advances devices at instruction boundaries; this is not bus-cycle-accurate emulation.
- The new opcode families have focused unit regressions, but still require exhaustive independent vectors for all encodings, flag edge cases, prefix interactions and timing.
- IM 0 does not yet execute every possible externally supplied opcode.
- The VDP and PSG need the remaining independent behavior vectors listed below; existing unit tests alone do not prove full silicon fidelity.
- A green Python SingleStepTests workflow is evidence for the Python adapter only unless the workflow explicitly executes the native Rust CPU. Do not present it as native Rust differential coverage.

## Workstreams

### CPU: documented Z80 instruction coverage

Implement in independently reviewable slices, retaining explicit errors until each instruction family has tests:

1. Base opcode matrix: all 256 primary opcodes, including undocumented/undefined encodings classified explicitly rather than silently treated as NOP.
2. Register model: AF'/BC'/DE'/HL', IX/IY, I/R, IFF1/IFF2, interrupt mode, and HALT/interrupt state.
3. CB prefix: rotate/shift, BIT, RES, SET for registers and (HL), including flags and documented T-states.
4. ED prefix: 16-bit arithmetic/load, block transfer/search/I/O, interrupt-mode, I/R transfers, RETN/RETI and aliases, with each encoding classified.
5. DD/FD prefixes: IX/IY substitution and displacement forms, including prefix repetition and DD/FD interactions with CB/ED.
6. Flag correctness: S/Z/H/PV/N/C and undocumented X/Y bits; overflow, half-carry, carry preservation, DAA, block-operation flags, and edge cases.
7. Control state: DI/EI delay semantics, IM 0/1/2, interrupt acknowledge behavior, NMI, RETN/RETI, HALT exit, and reset behavior.
8. Timing and bus contract: distinguish instruction totals from individual memory/I/O machine cycles; do not claim cycle accuracy until bus events have timestamped tests.

### VDP: TMS9918A behavioral audit

- Verify register write latch and two-byte command sequencing, read-ahead buffer semantics, VRAM address wraparound, status-read side effects, interrupt/VBlank behavior, and reset defaults.
- Validate each supported display mode and name/pattern/color/sprite table addressing against independent golden vectors or a trusted reference.
- Test sprite limits, overflow/collision status, transparency, priority, palette/color mapping, scanline/frame boundaries and framebuffer determinism.
- Separate documented chip behavior from SG-1000 board wiring, inferred behavior, and unresolved hardware-specific questions.

### PSG: SN76489 behavioral audit

- Validate latch/data protocol, tone periods including zero-period behavior, attenuation table, periodic/white noise feedback taps and shift direction, noise clock selection, and tone-3-linked noise.
- Test exact clock-to-sample accounting, chunking invariance, reset, bounded buffers and deterministic output.
- Treat FIR/filtering and output scaling as analog/output-pipeline choices, not as proof of digital chip fidelity.
- Confirm the clock source and divider for the target SG-1000 board from reliable hardware documentation before fixing production defaults.

### Machine integration

- Route CPU memory and port operations through the machine bus with deterministic ordering.
- Test event timestamps and CPU/VDP/PSG advancement over split and unsplit execution windows.
- Add integration vectors covering ROM/RAM mapping, mirrored or unmapped regions, controller I/O, VDP ports, PSG writes, frame output and audio samples.
- Keep the frontend/Qt/PyO3 boundary out of per-instruction execution; expose batch/frame-level APIs only.

## Independent validation

Use fixed, independently sourced vectors where licensing and provenance permit. Candidate sources include Zilog manuals, the project's uploaded Z80 technical manuals, public SingleStepTests vectors (verify their license and schema before importing), and independently generated synthetic programs. Never treat expected values calculated by the implementation under test as an oracle.

For each opcode family, record:
- opcode/prefix and operand form;
- before/after register and memory state;
- flags, PC/SP and wraparound;
- T-states and bus access sequence (when modeled);
- unsupported/undefined classification;
- source/provenance of expected behavior.

## Required gates

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- Z80 differential tests and prefix/opcode coverage report.
- VDP and PSG fixed-vector regressions plus deterministic repeat/chunking tests.
- Integration tests for CPU + bus + VDP + PSG + machine scheduler.
- Python pytest/Ruff and Release/Maturin/Qt parity checks when the extension or frontend is touched.

Report each gate as PASS, FAIL, BLOCKED or NOT RUN against the exact candidate SHA. A skipped workflow is not a pass.

## Completion criteria

The CPU is complete for the selected Z80 silicon profile only when every encoding is implemented or explicitly classified, all supported instruction families have independent state/flag/timing tests, and interrupt/prefix behavior is covered. The SG-1000 standalone milestone remains open until CPU, bus/memory, I/O, VDP, PSG, scheduler and the required frontend integration are tested together. No claim of cycle accuracy is allowed without timestamped bus-level evidence.
