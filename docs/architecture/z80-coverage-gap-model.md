# Z80 coverage gap model

**Audited revision:** `b6765e53d8116bb784f27fb7693297f908f398aa` (test-infrastructure branch)  
**Status:** static audit + focused CI baseline; not a claim of complete Z80 compatibility.

## Evidence from the current baseline

- GitHub Actions run: [40150622609](https://github.com/leonardo201800478/sega-emulator/actions/runs/40150622609) — **success**.
- Focused regression suite: **367 passed in 1.60 s** on Python 3.14.
- Dispatch inventory: 1,792 encodings exercised across base, CB, ED, DD, FD, DD-CB and FD-CB; all returned normally, with zero `unsupported` and zero unexpected exceptions.
- Inventory artifact: [z80-test-infrastructure-baseline](https://github.com/leonardo201800478/sega-emulator/actions/runs/40150622609).
- Important interpretation: this inventory is only a one-step dispatch smoke test. It cannot prove instruction semantics. ED bytes without assigned classic-Z80 operations intentionally fall through to an 8-T-state ED-NOP; DD/FD prefixes can fall back to the base decoder when they are ignored prefixes. Both behaviors make “executed=256” unsuitable as a completeness metric.

## Three parallel audit workstreams

These are three independent review tracks. The current tool connection does not expose a way to spawn autonomous coding agents, so this document defines the work packets and acceptance criteria rather than pretending that separate agents were launched.

### Agent 1 — Opcode semantics and prefix substitution

**Scope:** base, CB, ED, DD/FD and DD/FD-CB instruction decode; document each encoding as documented, undocumented alias, ignored prefix, or unassigned.

**Observed gap / risk:**
- The inventory groups encodings by byte-space, not by semantic operation or expected prefix behavior.
- DD/FD fallback paths are expected for instructions unaffected by IX/IY, but this currently makes an accidentally missing IX/IY substitution indistinguishable from a valid ignored prefix in the smoke report.
- The ED fallback correctly models unassigned bytes as ED-NOP for the classic Z80, but dispatch success alone does not distinguish assigned instructions from these aliases/NOPs.

**Required model:**
1. Generate an instruction-family matrix with canonical mnemonic, byte pattern, prefix space, assigned/alias/unassigned classification, expected length and nominal T-states.
2. For every IX/IY substitution family, test the destination/source register mapping, including the documented exceptions where H/L remain H/L when paired with `(IX/IY+d)`.
3. Explicitly enumerate ED assigned encodings and prove every unassigned byte takes the ED-NOP path.
4. Do not mark an instruction complete based solely on returning from `step()`.

**Acceptance:** every documented classic-Z80 encoding maps to a named semantic family; all 256 encodings per prefix space have an expected classification; targeted tests fail if a prefix-sensitive encoding silently falls through to the base decoder.

### Agent 2 — Flags, hidden state, interrupts and edge cases

**Scope:** S/Z/Y/H/X/PV/N/C flags; Q and WZ/MEMPTR; R refresh behavior; alternate registers; EI delay; HALT; IM 0/1/2; NMI; block instructions; 16-bit wrapping.

**Current evidence:** exhaustive tests exist for 8-bit ALU combinations, DAA input combinations, BIT flag behavior and indexed BIT undocumented X/Y flags. The core also has focused tests for EI delay, IM 0 cases, HALT, R refresh, Q and WZ.

**Remaining validation gap:** the current tests are strong but selective; there is no evidence in this workflow of exhaustive state-transition differential testing for every documented instruction, interrupt boundary, or hidden-state interaction. WZ/Q/interrupt behavior must be treated as a separate semantic dimension, not inferred from opcode coverage.

**Required model:**
1. Build a state-transition test matrix for each instruction family: inputs, outputs, flags, PC/SP, WZ, Q/P latches, R and memory/I/O side effects.
2. Add boundary vectors for wraparound, carry/borrow, signed displacement, repeat termination, interrupt acceptance immediately before/after EI and during HALT.
3. Compare against pinned SingleStepTests vectors and report mismatches by state field; preserve provenance and checksums.
4. Distinguish confirmed hardware behavior, documented behavior, and unresolved/variant-specific hypotheses.

**Acceptance:** all existing focused tests stay green; each newly claimed behavior has a targeted regression and, where available, an external oracle case.

### Agent 3 — Timing, bus effects and differential harness

**Scope:** nominal T-states, M1/fetch accounting, memory/I/O ordering, interrupt acknowledge, prefix fetches, and path toward cycle-accurate execution.

**Confirmed architectural limitation:** `Z80Cpu.step()` returns nominal instruction T-states and increments aggregate cycle totals, but the memory interface is called at instruction level. There is no explicit T-state/M-cycle bus trace in the current CPU contract. Therefore the core is **not yet cycle-accurate**, even when nominal instruction totals match reference values.

**Required model:**
1. Define a stable trace schema: T-state or machine-cycle index, M1/fetch, memory read/write, I/O read/write, interrupt acknowledge, address, data, and wait/contention state if supported.
2. First validate instruction-level outcomes and nominal cycle totals against the pinned corpus; then add ordered bus-effect assertions.
3. Keep long-running stress tests in GitHub Actions (not Google Colab), upload JSON mismatch summaries and minimized failing cases.
4. Do not add CPU/device concurrency until a deterministic shared-clock model exists.

**Acceptance:** reproducible CI results, archived mismatch reports, and no claim of cycle accuracy until ordered bus traces are validated against an authoritative reference.

## Priority queue

| Priority | Gap to close | Why it matters | Completion evidence |
|---|---|---|---|
| P0 | Semantic instruction-family matrix, separate from dispatch inventory | Avoids false confidence from “all opcodes execute” | Matrix + tests for prefix-sensitive families |
| P0 | Expand SingleStepTests coverage and classify mismatch fields | Finds concrete semantic defects rather than guessing missing opcodes | Pinned corpus results with per-file pass/fail |
| P1 | ED aliases/unassigned-byte classification tests | Separates legal instructions from ED-NOP behavior | Explicit 256-byte ED classification |
| P1 | Interrupt/hidden-state differential tests | Interrupts and WZ/Q are easy to get subtly wrong | Reproducible state-by-state vectors |
| P1 | Bus trace design and ordered side-effect tests | Nominal T-state totals are not bus-cycle accuracy | Trace schema + golden traces |
| P2 | Variant-specific undocumented behavior (NMOS/CMOS/clones) | Must not be generalized without evidence | Variant policy + cited hardware evidence |

## Do not claim yet

- Full instruction-set semantic correctness from dispatch inventory.
- Complete undocumented-opcode compatibility across Z80 revisions/clones.
- Cycle-accurate CPU/bus timing.
- Full SingleStepTests coverage: the manifest is a selected subset, not proof that every instruction and state combination is covered.

## Next implementation slice

Implement the semantic matrix and a targeted regression layer first. Avoid rewriting the CPU decoder until the matrix identifies a reproducible missing behavior or failing oracle case. This keeps changes small and prevents speculative rewrites.


## Differential finding: repeating block-transfer/compare flags

The pinned SingleStepTests/Z80 corpus exposed a repeat-iteration difference in
`ED B0` (LDIR): when BC remains nonzero and PC is rewound to the instruction
start, undocumented flags F3/F5 match PC bits 11/13 rather than the ordinary
LDI/LDD data-sum rule. The same rule is applied to the corresponding repeating
compare operations CPIR/CPDR, with regression tests for LDIR, LDDR, CPIR, and
CPDR across several PC high-byte patterns.

Evidence: [SingleStepTests/Z80](https://github.com/SingleStepTests/z80) at pinned
commit `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, plus the [2026 discussion of
non-terminating block-repeat flags](https://www.reddit.com/r/EmuDev/comments/1vkkk21/z80_proper_handling_of_undocumented_x_and_y_flags/).
This is an observed compatibility rule from the differential vectors; it should
not be generalized to every Z80 revision without hardware-specific evidence.

The same 46-file corpus also exposed repeat-iteration flag behavior in
INIR/INDR/OTIR/OTDR. For these instructions F3/F5 are taken from PC.11/PC.13;
P/V and H receive the additional repeat-phase adjustment described by the
referenced 2018 flag investigation. OUTI/OTIR/OUTD/OTDR drive the port address
with the decremented B value. The implementation is checked against 4,000
SingleStepTests vectors. This remains corpus-backed compatibility evidence,
not a claim that every Z80 revision is identical.

Additional reference: [Z80Decoder undocumented-flags notes](https://github.com/hoglet67/Z80Decoder/wiki/Undocumented-Flags).
