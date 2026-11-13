# SMS external diagnostic ROM catalog

This directory tracks public upstream diagnostic suites for the Master System and the tooling used to execute them. The presence of a runner does **not** mean a ROM suite or any hardware behavior has passed.

## Full-machine runner

`native/sms_core/examples/sms_rom_runner.rs` executes through `SmsTemporalMachine`, `SmsTemporalBus`, the Z80 microsequencer, Scheduler, mapper-backed memory, PSG and SMS VDP. It is deliberately separate from the CPU-only `z80test_runner.rs`.

Run from the repository root:

```powershell
python tests/sms/scripts/run_rom.py --rom .\path\to\diagnostic.sms --suite-id smsmemtest --suite-revision 12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9 --region usa --frames 120
```

Supported regional profiles are `usa`, `brazil` (PAL-M), `europe` (PAL) and `japan`. The runner uses the same VDP revision, sprite-limit mode and PSG clocks selected by the SMS runtime worker. The current runner uses a direct-cartridge, no-BIOS startup profile; it does not reproduce BIOS initialization.

The Python wrapper copies the input into a temporary per-run snapshot while calculating SHA-256 over the exact bytes written, then passes that same snapshot to Rust. It also obtains the checked-out Git revision and invokes Cargo. The source ROM can change during Cargo compilation without changing the bytes executed by this run. The runner uses the canonical 48 kHz internal PCM source rate used by the SMS runtime and records the master-tick scale, PSG clock and sample rate in its start record. The Rust runner emits JSON Lines:
- `start`: ROM provenance, image size, selected machine profile and execution limits;
- `frame`: completed frame, instruction count, drained PSG sample count and virtual machine timestamp;
- `summary`: `EXECUTED_NO_ORACLE` for generic frame-budget completion, `INPUT_DRIVER_TIMEOUT_NO_ORACLE` if the SMSmemtest prompt driver cannot advance, `SMSMEMTEST_COLOR_RESULTS_UNVALIDATED_ON_ROM` when it observes the VRAM result color, or an execution error if stepping stops.

**`EXECUTED_NO_ORACLE` is not PASS.** The SMSmemtest-specific driver watches documented text-grid rows at completed-frame boundaries and pulses controller 1/button 1 after stable white-text row activity to advance from the initial prompt to RAM tests and from the RAM-complete prompt to VRAM tests. Before accepting a VRAM result, it requires a blank result row after the second pulse to reject a stale RAM result. It classifies the seven RAM result rows by the ROM's green/red result palette and classifies the VRAM result row as all-pass or first-failure-reported. This is not OCR: a red VRAM result does not identify which of the five tests failed. The color classifier is covered by synthetic unit tests but remains unvalidated against an actual built SMSmemtest ROM. The driver stops after observing the VRAM result and does not enter the cartridge-SRAM phase. The SMSmemtest classifier reports the result displayed by the ROM; it is not an independent oracle of hardware correctness and remains blocked until end-to-end ROM build/run validation. Other suites still require their own adapters/oracles. A frame-step limit error is an execution error, not evidence of a hardware defect.

Example options:
- `--frames N`: number of completed VDP frames (default 120; safety ceiling 100000);
- `--max-instructions-per-frame N`: safety bound per frame (default 100000);
- `--region usa|brazil|europe|japan`: explicit regional hardware tuple;
- `--debug`: use Cargo's debug profile instead of release.

Generated JSONL reports and ROM binaries should be stored outside Git unless a separate policy explicitly allows the artifact. Do not put ROM bytes, BIOS images, memory dumps or copyrighted traces in logs.

## Reproducible SMSmemtest ROM build

The upstream ROM can be built without copying its source or binary into this repository:

- SMSmemtest revision: `12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9` (Unlicense);
- devkitSMS revision: `561bc2cb9829cdf2e31ad8cd43835037c8c44a63`;
- SDCC: `4.6.0` (devkitSMS documents 4.2.0 as its minimum);
- the upstream modified `crt0_memtest.rel` is consumed directly from the pinned SMSmemtest checkout.

The build helper is `tests/sms/scripts/build_smsmemtest.sh`. It requires `SMSMEMTEST_ROOT`, `DEVKIT_SMS_ROOT`, `OUT_DIR` and `SDCC_BIN`; it verifies both Git revisions and the compiler version, builds SMSlib from the pinned devkitSMS source, compiles/links the ROM and checks that the resulting image is 32 KiB. It records the ROM SHA-256 in `SMSmemtest.build-info.txt`.

The CI workflow `.github/workflows/ci.yml` includes a Linux job that builds the pinned ROM and full-machine runner, then executes the ROM twice for up to 1800 frames per run. The job gates the seven supported system-RAM cases, the aggregate VRAM result, and byte-for-byte equality of deterministic JSONL records. It does **not** gate or certify cartridge SRAM, which remains unsupported. The ROM, build metadata and both JSONL outputs are retained as a short-lived Actions artifact, never committed to Git.

## Current execution boundary

The existing `native/sms_core/src/bin/z80test_runner.rs` is a CPU-only runner. It loads a flat 64 KiB image into `FlatBus`; it does not instantiate the complete SMS machine, VDP, PSG, mapper, or Scheduler. It must not be used to claim SMS ROM compatibility.

The production temporal slice integrates Z80 and scheduled bus transactions with PSG and SMS VDP. The VDP currently advances at scanline granularity. Dot/fetch arbitration and mid-scanline effects are not implemented. Therefore, the full-machine runner can execute a ROM, but timing-sensitive sub-scanline assertions remain blocked until the corresponding hardware granularity exists.

## Catalog

See [manifest.toml](manifest.toml) for upstream URLs, immutable source revisions, license observations, intended scope, and execution gates.

| Suite | Primary coverage | Current gate |
|---|---|---|
| SMSmemtest | Internal RAM, VRAM and cartridge RAM | Two identical real-ROM runs: 7/7 RAM PASS and VRAM PASS_ALL_REPORTED_BY_ROM; SRAM BLOCKED_CAPABILITY |
| SMS Test Suite | Video patterns, PSG/FM audio, pads, paddle and system identification | Runner available; build dependencies and case oracles remain to be validated |
| SmsVdpTest | VDP ports, VRAM/CRAM protocol, counters, IRQ, sprite collision/overflow | Runner available; timing assertions blocked by missing dot/fetch and sub-scanline model |
| ZEXALL-SMS | Z80 documented and undocumented behavior/flags | E8 gate only; do not execute before formal E7 closure |
| NotOnlyWords | Candidate VDP edge-case suite | Unresolved source identity; do not invent a source or gate |

## Acquisition and provenance policy

- Never commit downloaded ROM binaries by default.
- Obtain sources from the pinned upstream revision recorded in `manifest.toml`; do not use a moving `master` URL for reproducible validation.
- Record exact build tools, source revision, resulting ROM SHA-256, emulator commit, machine profile, region, and test output in each run record.
- Do not treat public repository visibility or non-commercial use as a substitute for an explicit license. SMSmemtest declares Unlicense; ZEXALL-SMS declares GPL-2.0. For the two suites without an explicit license file found during this audit, use upstream material for local testing only and do not redistribute their sources or binaries in this repository unless the rights holder clarifies permission.
- Respect upstream attribution and license terms if a source or binary is ever vendored.
- Generated ROMs, commercial ROMs, BIOS images, memory dumps, and traces containing ROM bytes do not belong in Git history.

## Result classification

Every test case must be classified independently as `PASS`, `FAIL`, `BLOCKED`, or `NOT RUN`. A ROM booting, a menu appearing, a successful compile, a completed frame budget, or a green unrelated CI job is not a test pass.

A valid result record must include:

1. emulator commit SHA and build profile;
2. upstream revision and source/build provenance;
3. ROM SHA-256;
4. SMS VDP revision, territory/region and timing profile;
5. exact test cases selected and expected behavior;
6. raw output or a reproducible observation, without embedding copyrighted ROM data;
7. whether the test exercised the full SMS machine or only the CPU.

## Next engineering steps

1. CI validates the Rust runner and its synthetic in-memory fixture. The separate SMSmemtest build/diagnostic workflow now builds the pinned upstream ROM and collects full-machine output; its first real-ROM run must be inspected before the prompt/color pipeline is considered validated.
2. SMSmemtest has a deterministic frame-boundary input driver based on two-frame-stable white-text activity in documented rows. Two real-ROM executions produced identical frame/input/result records: all seven RAM cases reported PASS and VRAM reported aggregate PASS. This is not a physical-hardware certification and does not test cartridge SRAM. The cartridge SRAM phase remains blocked because SmsMemory has no cartridge SRAM backing store. See [the dedicated audit](../../docs/validation/smsmemtest-oracle-audit-2026-11-13.md).
3. Expose additional read-only diagnostic observations only where a test's expected behavior requires them; do not bypass Scheduler dispatch.
4. Enable only SmsVdpTest cases whose required behavior is represented by the current VDP model; keep dot/fetch-dependent cases blocked.
5. Open the ZEXALL-SMS E8 campaign only after the Z80 scorecard formally closes E7 and the shared CPU harness is audited.
