# Project completion contract — Sega Emulator

**Status:** permanent Draft pull request; this contract must not be finalized before the entire project is complete.

**Normative roadmap:** [`docs/project-roadmap.md`](project-roadmap.md). The roadmap remains the only source of phase order and priorities; this document defines the final acceptance gate, not a competing roadmap.

**Project scope:** [`docs/project-scope.md`](project-scope.md).

**Architecture decisions:** [ADR 0010 — Rust runtime and temporal ownership](architecture/adr/0010-rust-runtime-temporal-ownership.md), [ADR 0011 — Slint/WGPU/CPAL desktop platform](adr/0011-slint-wgpu-cpal-desktop-platform.md), and [ADR-0016 — observable fidelity and proportional timing](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

**Source of truth:** the latest integrated `main` SHA and its verified CI evidence.

## 1. Purpose and lifecycle

This contract is the project-wide final acceptance gate. It records what must be true before the Sega Emulator can be declared complete. It is intentionally kept in a Draft PR throughout development and must not be merged, closed as completed, or marked ready for review while any mandatory project phase or acceptance criterion remains open.

The contract is not a progress dashboard. Component-level implementation status, evidence, and active work remain in the canonical scorecards, audits, roadmap and implementation PRs. The roadmap remains the only source of phase order and priorities.

## 2. Non-negotiable architecture

- **Hardware and runtime are Rust-only.** CPU, memory, bus, mapper, VDP, PSG, interrupts, Scheduler, timing, save-state and machine state belong to Rust.
- **MachineScheduler is the temporal authority.** It owns machine time, clock-domain coordination, event ordering, causal progression and safe-point application of host commands.
- **Desktop GUI is Slint 1.18.x.** Slint presents the application and collects user input; it does not execute hardware or advance emulated time.
- **Graphics use WGPU 30.x**, with Vulkan as the initial explicit backend on Windows x64. WGPU owns the graphics API boundary; Vulkan is not a parallel renderer.
- **Physical audio output uses CPAL 0.18.2.** Audio-device clocks, callbacks and buffer consumption cannot advance or rescale emulated time.
- **Gamepad/input integration uses SDL3** through the Rust input runtime. Keyboard gameplay input may use native Slint events. Input backends deliver normalized host events; Rust decides how and when those events enter the machine timeline.
- **Host integration is coarse-grained.** Session commands such as `load_rom`, `start`, `pause`, `stop`, `reset` and `step_frame` cross the UI/runtime boundary. No callback per instruction, T-state, bus access, pixel, audio sample or hardware tick is permitted.
- Share CPU cores and hardware components when the underlying hardware is compatible; do not duplicate a CPU/runtime per console.
- GUI, graphics, audio and input workers cannot mutate hardware state or independently advance machine time.
- Save states and replay preserve the state needed for deterministic continuation, including relevant device and Scheduler state.
- Commercial ROMs, proprietary BIOS/firmware and protected dumps must never be committed.

Python utilities that may exist under `tools/` are offline analysis/development tools only; Python, PySide6/Qt, PyO3 and Maturin are not the active product frontend or hardware runtime.

## 3. Fidelity contract

Functional semantics are mandatory for every supported instruction and operation. The required execution granularity is proportional to observable behavior:

1. **Gameplay-sensitive temporal paths:** preserve order, duration and interaction points when CPU, memory/bus, mapper, VDP/raster, audio, interrupts, DMA or peripherals can affect gameplay according to timing.
2. **Atomic deterministic baseline:** operations without observable intermediate effects may execute atomically while preserving exact results, flags, state transitions, side effects, determinism and nominal duration where known.
3. M-cycles, T-states, WAIT, arbitration, resumable transactions and internal phases are required only when a supported console/game/peripheral has a demonstrated dependency on them.
4. Pin-level, transistor-level, analog and universal electrical simulation are not project completion requirements.

Every increase in temporal detail must identify the affected console/game/revision/peripheral, observable effect, evidence, minimum sufficient granularity and regression test. Absence of unnecessary microsequencing is not a functional defect.

The shared Z80 is **CLOSED_WITH_LIMITATIONS** for the supported SMS/SG-1000 profile based on the defined Zilog NMOS behavior. E8/ZEXDOC/ZEXALL and further adversarial review are complementary validation, not permanent gates for advancing the roadmap. Reopen E7 only for a reproducible defect/regression or new technical evidence. CMOS/clones outside the declared profile, physical WAIT, DMA and global electrical timing remain NOT_VERIFIED/NOT_RUN.

## 4. Final acceptance gates

The project may be declared complete only when **all** of the following are satisfied on the final integrated `main`:

- [ ] Every platform phase P1–P7 in the canonical [project roadmap](project-roadmap.md) has met its documented exit criteria in the prescribed order.
- [ ] Every desktop migration stage R1–R5 in the same roadmap and ADR 0011 has met its exit criteria; historical Qt/PySide6/egui snapshots are not accepted as proof of the current Slint target.
- [ ] Every platform and accessory phase in scope (SMS1/SMS2, SG-1000, Game Gear, 8-bit expansions/accessories, Mega Drive/Genesis, Sega CD and 32X) has its required hardware runtime and host integration implemented; deferred or unsupported behavior is explicit rather than silently approximated.
- [ ] Shared CPU cores and reusable components are consolidated without per-console duplicates or parallel legacy hardware runtimes.
- [ ] Functional behavior is covered by unit, integration, diagnostic-ROM and representative game tests appropriate to each platform.
- [ ] Gameplay-sensitive timing interactions have reproducible evidence and tests at the minimum sufficient temporal granularity. No universal pin-accuracy claim is required.
- [ ] Deterministic execution is demonstrated across continuous execution and chunked/frame execution wherever the public runtime supports both.
- [ ] Save-state save/load and deterministic replay restore all state required for continuation, including relevant device and Scheduler state.
- [ ] Cross-domain CPU/video/audio/bus/interrupt/peripheral ordering is validated wherever those domains interact observably.
- [ ] Slint desktop, WGPU/Vulkan presentation, CPAL output, SDL3 input, packaging, supported host builds and user-facing workflows meet the canonical roadmap's exit criteria; hardware timing remains outside the GUI and host workers.
- [ ] Compatibility and regression matrices for all supported platforms are current, with no unresolved release-blocking failures.
- [ ] Documentation, ADRs, component status, build instructions and release notes describe the integrated implementation and contain no stale architecture snapshots or contradictory requirements.
- [ ] All required Rust quality gates pass on the exact final `main` SHA:
  ```text
  cargo fmt --all -- --check
  cargo clippy --release --workspace --all-targets --locked -- -D warnings
  cargo test --release --workspace --locked
  ```
- [ ] Required platform-specific CI, native build and host-integration jobs pass on that same SHA. A job is required only where the final diff/product scope requires it, but every mandatory gate must actually run; SKIPPED, NOT RUN or BLOCKED is never PASS.
- [ ] Physical Windows x64 validation is recorded where the roadmap/ADR requires it: effective Vulkan backend and surface lifecycle, sustained presentation, CPAL playback/hotplug/underrun behavior, SDL3 controls/hotplug and the relevant SMS/gameplay paths. CI compilation is not proof of physical device behavior.
- [ ] BIOS boot is considered physically validated only after the required test with a real, identified BIOS and reproducible handoff/first-cartridge-instruction evidence. Other physical tests are required only where a platform-specific acceptance criterion explicitly calls for them.
- [ ] Every mandatory project phase and required implementation PR is merged or explicitly rejected/closed with rationale; no required work remains only on an unmerged branch.
- [ ] E7 remains CLOSED_WITH_LIMITATIONS for the declared NMOS profile. E8/ZEXDOC/ZEXALL, universal CMOS/clones coverage, physical WAIT, DMA and global electrical timing are complementary or out-of-profile validations, not permanent project-completion gates.
- [ ] PRs and architecture changes have review evidence appropriate to repository policy. Review by another AI and physical-console testing are not universal acceptance requirements; hardware-specific gates such as BIOS boot remain mandatory where explicitly declared.
- [ ] The final repository audit confirms reproducible dependencies/lockfiles, clean architecture boundaries, no protected ROM/firmware assets and no unresolved critical or high-severity defects.

Test results must be classified individually as PASS, FAIL, BLOCKED or NOT RUN. SKIPPED is never PASS. Each result must identify the exact commit SHA and the jobs that actually ran. Evidence from another SHA is historical and must not be transferred to the final SHA.

## 5. Finalization procedure

When all gates above are satisfied:

1. Re-audit the remote `main`, all open PRs, workflows, release artifacts and the canonical roadmap.
2. Re-run or verify every required gate against the exact final `main` SHA; rerun any check whose evidence belongs to another SHA.
3. Update the canonical component status and roadmap evidence without creating a parallel roadmap.
4. Complete this checklist with links to the evidence and record the final SHA.
5. Convert this Draft PR to ready only after the evidence is complete, then merge it only after all repository policies and CI requirements are satisfied.

Until then, this PR remains a permanent, non-mergeable-in-practice Draft completion gate. Its existence does not authorize bypassing normal branch, review, test or merge policies.
