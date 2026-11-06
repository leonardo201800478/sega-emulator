# Component status — migration branch snapshot

**Scope:** this document describes the Rust-only migration branch, not the still-unmodified `main`. The authoritative state becomes `main` only after an authorized merge.

| Component | Current state | Evidence / remaining gate |
|---|---|---|
| Shared Z80 | Rust crate remains in the active workspace | Continue the E7 semantic audit; do not start E8 external conformance until E7 is closed |
| MachineScheduler | Rust crate remains active | Deterministic stress job passed on the first PR #609 candidate SHA; recheck the latest PR head |
| SMS core | Hybrid temporal/atomic crate | Dependency audit and separation remain pending; no exclusive temporal SMS claim |
| SMS runtime | `rust/runtime/sms-runtime` contains the extracted worker | New crate is not yet connected to the egui app; exact-head CI is pending |
| Audio runtime | Rust audio session/mixer crate remains active | Host session is injected through a factory invoked on the worker thread; application host factory integration is pending |
| Audio output | Standalone Rust WASAPI/XAudio2 workspace | Windows compile/tests remain required; physical audio NOT RUN |
| Video runtime | Rust frame/mailbox and D3D12 host remain active | Windows compile/tests required; physical GPU presentation NOT RUN |
| Input runtime | Rust input crate remains in the workspace | Not yet wired to the egui application; physical input/latency NOT RUN |
| egui application | Native Rust shell only | Does not yet load ROMs or create an SMS machine; runtime integration pending |
| SG-1000 atomic core | Still in the workspace | Must be disconnected/removed after SMS dependency boundaries are proven; not an active product runtime |
| Python / Qt / PyO3 | Removed from this migration branch | 141 Python/configuration files and the PyO3 extension crate removed; tree scan found no Python source/config files |
| External Z80 conformance | NOT RUN | Python runner removed; Rust-native ZEXDOC/ZEXALL harness remains pending |

## PR #609 validation

- `cargo fmt --all -- --check`: PASS on SHA `4311ec6d41dccee28649f8d687aa1d9ea7a2bd13` after correcting the initial formatting failure.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS on that SHA after correcting the audio-session thread-ownership contract and target-gating the Windows mailbox command.
- `cargo test --workspace --locked`: PASS on that SHA.
- `Scheduler deterministic stress`: PASS on that SHA.
- `Windows PowerShell tooling syntax`: PASS on that SHA.
- `Windows x64/D3D12 compile`: PASS on that SHA, including the native egui shell, SMS runtime target check/Clippy, and PowerShell workspace audit/lock validation.
- After the validated candidate SHA `4311ec6d41dccee28649f8d687aa1d9ea7a2bd13`, the branch received formatting-only corrections and a temporal CPU RESET implementation with tests for peripheral/timeline preservation and rejection during an in-flight instruction.
- Those later commits have no confirmed CI result in this audit. Formatting, Clippy, workspace tests, Scheduler stress and Windows/D3D12 on the exact latest HEAD remain NOT RUN until a run is observed.
- Physical D3D12 presentation, WASAPI/XAudio2 output and input latency: NOT RUN.
- Local Cargo validation: NOT RUN because the local environment has no Rust toolchain.

No component in this table is declared functionally complete solely because its crate compiles.
