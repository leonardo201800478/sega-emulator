# Component status — migration branch snapshot

**Scope:** this document describes the Rust-only migration branch, not the still-unmodified `main`. The authoritative state becomes `main` only after an authorized merge.

| Component | Current state | Evidence / remaining gate |
|---|---|---|
| Shared Z80 | Single Rust crate `native/z80_cpu` used by SMS and SG-1000 | Continue E7 semantic audit; E7.7 remains IN_PROGRESS and E8 remains BLOCKED. Temporal sequencer RESET now preserves architectural registers and INT while clearing CPU control state |
| MachineScheduler | Rust crate remains active | Deterministic stress job passed on the first PR #609 candidate SHA; recheck the latest PR head |
| SMS core | Temporal-only first vertical slice | The old atomic SMS machine module and its duplicate Z80 facade are absent from the branch; the core composes the shared Z80, mapper-backed memory/I/O, Scheduler, PSG and scanline VDP. Dot/fetch arbitration and physical WAIT remain pending |
| SMS runtime | `rust/runtime/sms-runtime` contains the extracted worker | New crate is not yet connected to the egui app; exact-head CI is pending |
| Audio runtime | Rust audio session/mixer crate remains active | Host session is injected through a factory invoked on the worker thread; application host factory integration is pending |
| Audio output | Standalone Rust WASAPI/XAudio2 workspace | Windows compile/tests remain required; physical audio NOT RUN |
| Video runtime | Rust frame/mailbox and D3D12 host remain active | Windows compile/tests required; physical GPU presentation NOT RUN |
| Input runtime | Rust input crate remains in the workspace | Not yet wired to the egui application; physical input/latency NOT RUN |
| egui application | Native Rust shell only | Does not yet load ROMs or create an SMS machine; runtime integration pending |
| SG-1000 core | Still in the workspace | Retained temporarily as the existing SG-1000 product/reference while SMS becomes executable; later roadmap step removes it from active workspace before a new SG-1000 runtime is built on the shared Z80 |
| Python / Qt / PyO3 | Removed from this migration branch | 141 Python/configuration files and the PyO3 extension crate removed; tree scan found no Python source/config files |
| External Z80 conformance | NOT RUN | Python runner removed; Rust-native ZEXDOC/ZEXALL harness remains pending |

## PR #609 validation

- On SHA `4311ec6d41dccee28649f8d687aa1d9ea7a2bd13`, run `42255686065` reported **success** for the required jobs: `cargo fmt --all -- --check`, workspace Clippy with `-D warnings`, workspace tests, Scheduler deterministic stress, Windows PowerShell tooling syntax and Windows x64/D3D12 compile/tests.
- The workflow aggregate conclusion was `cancelled`, even though those required jobs each concluded `success`; the manual lockfile-refresh job was `skipped`. Treat the individual required-job conclusions as PASS for that SHA, not as a fully successful aggregate workflow.
- After the validated candidate SHA `4311ec6d41dccee28649f8d687aa1d9ea7a2bd13`, the branch received formatting-only corrections and a temporal CPU RESET implementation with tests for peripheral/timeline preservation and rejection during an in-flight instruction.
- Those later commits have no confirmed CI result in this audit. Formatting, Clippy, workspace tests, Scheduler stress and Windows/D3D12 on the exact latest HEAD remain NOT RUN until a run is observed. A later run `42263870310` was observed as pending for an earlier branch commit (`8f43fb166f1ddb14b33245edf0802b61297f3f44`); it is not proof for the subsequent HEAD.
- `scripts/project.ps1 locks -RefreshOffline` now performs `cargo update --offline` for each active workspace and validates the resulting lockfile with locked Cargo metadata.
- Physical D3D12 presentation, WASAPI/XAudio2 output and input latency: NOT RUN.
- Local Cargo validation: NOT RUN because the local environment has no Rust toolchain.

No component in this table is declared functionally complete solely because its crate compiles.
