# SMS external diagnostic ROM catalog

This directory tracks public upstream diagnostic suites for the Master System. It does **not** claim that these ROMs already run in the emulator or that any hardware behavior has passed.

## Current execution boundary

The existing `native/sms_core/src/bin/z80test_runner.rs` is a CPU-only runner. It loads a flat 64 KiB image into `FlatBus`; it does not instantiate the complete SMS machine, VDP, PSG, mapper, or the machine Scheduler. It must not be used to claim SMS ROM compatibility.

The production temporal slice is more advanced: `SmsTemporalMachine` owns the resumable Z80 and `SmsTemporalBus`; the bus commits PSG and SMS VDP port effects at scheduled transaction completion. The current VDP integration advances at scanline granularity. Dot/fetch arbitration and mid-scanline effects are explicitly not implemented. Consequently, the VDP suite can be catalogued now, but timing-sensitive sub-scanline assertions must remain NOT RUN / BLOCKED until the corresponding hardware granularity exists and an end-to-end ROM runner can observe it.

## Catalog

See [manifest.toml](manifest.toml) for upstream URLs, immutable source revisions, license observations, intended scope, and execution gates.

| Suite | Primary coverage | Current gate |
|---|---|---|
| SMSmemtest | Internal RAM, VRAM and cartridge RAM | Candidate for full-machine functional execution; ROM runner still required |
| SMS Test Suite | Video patterns, PSG/FM audio, pads, paddle and system identification | Candidate for manual/automated functional checks; no explicit license file found at pinned revision |
| SmsVdpTest | VDP ports, VRAM/CRAM protocol, counters, IRQ, sprite collision/overflow | Timing assertions blocked by missing dot/fetch and sub-scanline model; no explicit license file found at pinned revision |
| ZEXALL-SMS | Z80 documented and undocumented behavior/flags | E8 gate only; do not execute before formal E7 closure |
| NotOnlyWords | Candidate VDP edge-case suite | Unresolved source identity; do not invent a source or gate |

## Acquisition and provenance policy

- Never commit downloaded ROM binaries by default.
- Obtain sources from the pinned upstream revision recorded in `manifest.toml`; do not use a moving `master` URL for reproducible validation.
- Record the exact build tools, source revision, resulting ROM SHA-256, emulator commit, machine profile, region, and test output in each run record.
- Do not treat public repository visibility or non-commercial use as a substitute for an explicit license. SMSmemtest declares Unlicense; ZEXALL-SMS declares GPL-2.0. For the two suites without an explicit license file found during this audit, use upstream material for local testing only and do not redistribute their sources or binaries in this repository unless the rights holder clarifies permission.
- Respect upstream attribution and license terms if a source or binary is ever vendored.
- Generated ROMs, commercial ROMs, BIOS images, memory dumps, and traces containing ROM bytes do not belong in Git history.

## Result classification

Every run must be classified independently as `PASS`, `FAIL`, `BLOCKED`, or `NOT RUN`. A ROM booting, a menu appearing, a successful compile, or a green unrelated CI job is not a test pass.

A valid result record must include:

1. emulator commit SHA and build profile;
2. upstream revision and source/build provenance;
3. ROM SHA-256;
4. SMS VDP revision, territory/region and timing profile;
5. exact test cases selected and expected behavior;
6. raw output or a reproducible observation, without embedding copyrighted ROM data;
7. whether the test exercised the full SMS machine or only the CPU.

## Next engineering steps

1. Implement a Rust-native full-machine ROM diagnostic runner around `SmsTemporalMachine`, not `FlatBus`.
2. Add bounded execution, frame/instruction limits, deterministic input scripting, and machine-readable results.
3. Expose read-only diagnostic observations at the machine boundary without bypassing the Scheduler or changing hardware timing.
4. Start with SMSmemtest and non-timing-sensitive SMSTestSuite checks.
5. Enable only the SmsVdpTest cases whose required behavior is represented by the current VDP model; keep dot/fetch-dependent cases blocked.
6. Open the ZEXALL-SMS E8 campaign only after the Z80 scorecard formally closes E7 and the shared CPU harness is audited.
