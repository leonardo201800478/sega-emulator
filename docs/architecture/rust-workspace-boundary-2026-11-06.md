# Rust workspace boundary and relocation plan

**Date:** 2026-11-06  
**Status:** R1 runtime extraction implemented on migration branch; validation pending  
**Parent decision:** ADR 0010 — Rust-only, SMS temporal first

## 1. Objective

Create one physically separate Rust product tree under `rust/`. The repository
root remains for project governance, documentation, CI, licensing and
non-executable project metadata. Active source code, Cargo manifests,
lockfiles, application code, host adapters and Rust tests move under
`rust/`.

Python is not a second implementation, a reference runtime, a test harness,
a build tool or a fallback. The final repository must contain no Python
source, Python package metadata, Python lockfiles, Python workflows, PyO3 or
Maturin artifacts.

## 2. Target topology

The canonical topology is documented in [rust/README.md](../../rust/README.md)
and has these boundaries:

| Area | Responsibility |
|---|---|
| `rust/apps/sega-app` | egui/eframe shell, menus and presentation |
| `rust/runtime/sms-runtime` | SMS session, worker, coarse-grained lifecycle |
| `rust/crates/z80-*` | one shared Z80 implementation and bus/ALU contracts |
| `rust/crates/machine-scheduler` | sole authority for machine time and event ordering |
| `rust/crates/temporal-core` | shared temporal primitives and serializable state |
| `rust/crates/sn76489` | shared PSG implementation |
| `rust/crates/video-core` | shared SMS VDP implementation |
| `rust/crates/audio-runtime` | scheduled audio production |
| `rust/crates/video-runtime` | frame transport and host presentation boundary |
| `rust/crates/input-runtime` | native input abstraction |
| `rust/hosts/audio-output` | Windows WASAPI/XAudio2 output adapters |
| `rust/tests/system` | Rust-only integration, replay and determinism tests |

No future-platform runtime is activated by this relocation. SMS temporal is the
first and only machine runtime in the active workspace.

## 3. Dependency constraints

The intended runtime dependency direction is:

```text
sega-app (egui/eframe)
    -> application/session API
        -> sms-runtime
            -> sms-core temporal path
                -> shared Z80 + bus
                -> MachineScheduler + temporal-core
                -> SN76489 + audio-runtime
                -> video-core + video-runtime
            -> native input / audio host interfaces
```

The UI sends coarse-grained commands only. It does not call CPU instructions,
advance a clock, schedule hardware events or own machine state.

## 4. Ordered migration

### R0 — establish the boundary

- Define `rust/` as the only destination for active Rust product source.
- Inventory every Rust crate, standalone Cargo workspace, lockfile, CI path,
  documentation link and build script.
- Identify all consumers of the worker and every module in the hybrid SMS core.

### R1 — extract the SMS temporal runtime

- Extract `SmsTemporalWorker` and its required session/host dependencies from
  `native/z80_core` into `rust/runtime/sms-runtime`.
- Remove PyO3 annotations and dependencies from the runtime implementation.
- Preserve exactly one worker, one SMS temporal machine and one Scheduler.
- Add Rust integration tests for lifecycle, input, frame production, PCM
  production, shutdown and deterministic block/continuous execution.

### R2 — isolate the temporal SMS core

- Separate temporal modules from atomic CPU/machine/PSG/VDP modules.
- Preserve only memory, cartridge/mapper, I/O and VDP wrapper code proven to be
  consumed by the temporal path.
- Keep the shared Z80, PSG, VDP and Scheduler implementations unique.
- Do not perform a filename-only move or delete shared dependencies.

### R3 — consolidate the Rust workspace

- Move Rust source, manifests and lockfiles under `rust/`.
- Consolidate the active crates into one Cargo workspace and one canonical
  lockfile where dependency constraints allow.
- Put the app under `rust/apps`, host adapters under `rust/hosts`, and
  machine-independent crates under `rust/crates`.
- Remove obsolete root-level Cargo workspace files only in the same change
  that updates all CI and developer commands.

### R4 — retire legacy implementations

- Remove Python application/runtime, tests, reference code, scripts,
  packaging, lockfiles, Qt frontend, PyO3 adapter, Maturin configuration,
  wheels and Python-specific workflows.
- Remove atomic SMS and SG-1000 implementations from the active workspace.
- The old SG-1000 implementation is not a runtime fallback; a future SG-1000
  must be rebuilt on the shared Z80 after SMS is executable.

### R5 — validate and close

- Run rustfmt, Clippy with warnings denied and all workspace tests.
- Run Windows-target D3D12, WASAPI/XAudio2 and input checks.
- Run synthetic/homebrew SMS integration and deterministic replay tests.
- Classify every required suite as PASS, FAIL, BLOCKED or NOT RUN.
- Do not mark hardware-dependent tests PASS based on headless CI.

## 5. Acceptance gates

The relocation is complete only when:

1. All active Rust code and Cargo metadata live under `rust/`.
2. No Python, PyO3, Maturin, PySide6 or Qt artifacts remain.
3. One Cargo workspace and lockfile define the active product.
4. SMS temporal worker is pure Rust and is the only worker implementation.
5. Only SMS temporal is an active machine runtime.
6. UI is limited to presentation and coarse-grained application commands.
7. Required Rust, Windows and system-integration checks pass on the exact
   candidate SHA.
8. No essential semantic, temporal, snapshot or determinism contract was
   removed without a tested Rust replacement.

## 6. Baseline audit

At the baseline `edbec9272efe87a6a408bac74283d86a91d87b23`:

- 566 tracked blobs were returned by the recursive Git tree.
- 188 blobs were under `native/` or were root Cargo workspace files.
- 141 files matched Python source or Python packaging/configuration patterns.
  This includes `legacy/python_reference`, active `src/` and `tests/`,
  plus the PyO3 crate's own Python packaging metadata.
- Root Cargo workspace still includes `native/sg1000_core` and
  `native/sms_core`; `native/audio_output`, `native/z80_core` and
  `native/sega_app` are separate workspaces.
- `native/sms_core` is hybrid: temporal modules depend on shared memory,
  cartridge, I/O and VDP types in the same crate.
- `SmsTemporalWorker` remains inside `native/z80_core`, which depends on
  PyO3 and exposes both atomic and temporal bindings.
- The baseline CI defined Python, Qt, PyO3, Maturin and PowerShell tooling jobs. Python/Qt/PyO3 workflows were removed on the migration branch; PowerShell remains only as a native Rust build/validation wrapper.

These counts are an inventory of tracked paths, not a claim that all files are
runtime-active.

## 7. Implementation status

R0 established the destination and migration contract. R1 created
`rust/runtime/sms-runtime` as a pure-Rust workspace member and extracted the
SMS worker. The Python application, tests, Python tooling, PyO3 adapter and
associated workflows were removed rather than ported. The shell egui app is
not yet wired to the worker; this is the next integration increment. CI on
the exact PR head is required before considering the extraction validated.


## 8. SMS worker extraction — source-level dependency audit

The source inspection of the baseline identifies a relatively narrow extraction
boundary:

- `native/z80_core/src/sms_temporal_worker.rs` is 1,056 lines and contains
  the worker, command channel, lifecycle, telemetry, frame execution and its
  unit tests. Its direct imports are standard-library synchronization/thread
  primitives, `sega_sms_core`, `sega_audio_runtime`,
  `sega_video_runtime`, and the local `crate::audio` adapter.
- `native/z80_core/src/audio.rs` is 77 lines. It is already Python-independent
  and adapts `sega-audio-output` WASAPI/XAudio2 sinks to
  `sega-audio-runtime::NativeAudioSession`. This adapter belongs with the
  new SMS runtime/host composition, not in a PyO3 module.
- `native/z80_core/src/sms_temporal_machine.rs` is the PyO3 wrapper around
  `SmsTemporalWorker`; it should not be copied into the new runtime.
- `native/z80_core/src/input.rs` is a separate PyO3 input-manager surface.
  The worker itself receives `ControllerButtons`; native input capture is
  already represented by `sega-input-runtime` and should be integrated from
  the Rust application/session boundary, not imported from this PyO3 module.
- The current `sega-z80-core` manifest also depends on the atomic SG-1000
  core and PyO3. Neither dependency is required by the worker source itself.
- The worker's Windows video output depends on the existing
  `sega-video-runtime::d3d12::D3d12MailboxSession`; the runtime extraction
  must retain this dependency until the host viewport contract is resolved.

### Proposed first implementation slice

Create `rust/runtime/sms-runtime` as a pure Rust library with dependencies
on SMS core, audio runtime and video runtime. The worker has been extracted
with a public coarse-grained API. Physical audio output is supplied by an
application-owned factory that is invoked on the machine worker thread; this
keeps WASAPI/XAudio2 host objects thread-affine and avoids a dependency from
the runtime crate to the host backend. Do not bring over
`sms_temporal_machine.rs`, `input.rs`, `machine.rs`, PyO3 macros, or the
atomic SG-1000 dependency.

The extraction must preserve the existing worker tests and add integration
coverage for construction, lifecycle, ROM replacement, frame stepping,
controller state, audio failure isolation, video detachment, shutdown/join,
and deterministic execution. Existing tests inside the source file are not
sufficient by themselves to prove the public crate boundary.

The source-level dependency audit was used to define the extraction. The new
crate and worker now exist on the migration branch. Compilation and tests
remain pending CI on the exact PR head.


## 9. Única correção funcional autorizada durante a migração: desempenho

A migração pode conter **uma única frente de correção funcional além da reorganização**:
resolver o desempenho insuficiente já identificado, porque a lentidão impede
validar o emulador com ROMs e dispositivos reais em condições úteis.

Essa exceção tem escopo estrito:

- Corrigir somente a causa-raiz de desempenho já identificada e documentada
  pelo projeto; não abrir uma campanha genérica de otimização.
- Antes de alterar código, vincular a causa a evidência reproduzível
  (benchmark/profiling, caminho de execução e commit/issue de origem). Se a
  evidência histórica se referir ao runtime Python/SG-1000 legado, ela não
  pode ser presumida como causa do runtime SMS temporal Rust atual.
- A correção deve ocorrer no caminho Rust ativo e na nova estrutura
  `rust/`, sem reintroduzir Python, PyO3, JIT, frame skipping, hacks de
  temporização ou paralelismo especulativo.
- Não alterar a semântica de CPU, M-cycles/T-states, barramento, Scheduler,
  ordem causal, VDP, PSG, input ou sincronização A/V para obter FPS.
- Não misturar essa correção com refatorações oportunistas, mudanças gráficas,
  alterações de áudio/input ou features novas.
- Manter benchmark comparável antes/depois com a mesma ROM identificada por
  hash, configuração, máquina/runner e build; reportar FPS/tempo por frame,
  p50/p95, uso de CPU e resultados funcionais relevantes.
- Aceitar a correção somente se o ganho for mensurável e não houver regressão
  em determinismo, hashes de framebuffer/áudio, input, precisão temporal ou
  lifecycle.

### Gate de validação real

O objetivo da correção é tornar possível executar os testes reais de:

1. qualidade e estabilidade gráfica (frames/pixels e apresentação);
2. latência e comportamento de input;
3. geração, continuidade e saída física de áudio;
4. gameplay contínuo em ROM sintética/homebrew apropriada.

O desempenho mínimo necessário será definido pelo benchmark de referência e
pelo alvo de vídeo do hardware; não inventar um número de FPS sem fixar a
carga e o ambiente. Testes físicos Windows permanecem distintos dos testes
headless. Nenhuma correção de desempenho será declarada concluída só porque
o shell ou a janela abre.

**Status:** exceção de escopo aprovada pelo usuário; causa-raiz específica
ainda precisa ser ligada à evidência correta do runtime SMS temporal atual
antes da alteração de código. Não confundir com a auditoria histórica
Python/SG-1000 da issue #44.


## 10. R1 extraction — implementation record

- Added `rust/runtime/sms-runtime` to the active root Cargo workspace and
  default members.
- Extracted the existing 1,056-line `SmsTemporalWorker` implementation and
  its unit tests into `rust/runtime/sms-runtime/src/worker.rs`.
- Added a public `AudioSession` contract. The application supplies a
  `Send` audio-session factory; the factory is executed on the worker thread,
  and the resulting host session never crosses thread boundaries.
- The runtime depends only on `sega-sms-core`, `sega-audio-runtime`, and
  `sega-video-runtime`; it has no PyO3, Python, Qt, or physical audio-output
  dependency.
- Added a public-API integration test for ROM loading, stepping, controller
  input and reset, plus a worker-thread audio-factory test.
- Added Windows-target compile and Clippy checks for the new D3D12-enabled
  runtime.

**Validation status:** source, workspace metadata and legacy cleanup are
committed on the migration branch. Local Cargo validation is unavailable in
this environment; formatting, Clippy, Linux unit tests and Windows target
compilation are NOT RUN locally and remain dependent on CI for the PR head.
The Python orchestration for external Z80 conformance was removed. A low-level
Rust runner remains under the SMS core but still depends on compatibility
exports and lacks Rust-native suite acquisition/assembly orchestration; ZEXDOC/
ZEXALL are NOT RUN until that harness is relocated and completed.
