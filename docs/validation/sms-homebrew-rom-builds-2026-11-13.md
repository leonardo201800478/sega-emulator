# SMS Homebrew ROM Builds — 2026-11-13

## Purpose

Record reproducible build procedures for two upstream Master System homebrew diagnostic ROMs. These builds add external diagnostic material to the validation workflow; they do not modify or validate the emulator runtime.

## Build results

| ROM | Source revision | Toolchain | Output | SHA-256 |
|---|---|---|---:|---|
| SmsVdpTest | `FluBBaOfWard/SmsVdpTest@490c00b38c83012dee5a7a582097f080c210a3e7` | WLA-DX `91c52b1f4ef3cc8ba3c0638f7536539579af6a9f` | 32,768 bytes (2 × 16 KiB) | `14e52a545a2a77bdd7f0ec1e52d78bd1abb3f64dd269f1f4639bbc8b2608c2f2` |
| SMS Test Suite 0.38 | `sverx/SMSTestSuite@985bc04fef6f3dc3c3450fbc84242676f241d316` | SDCC 4.5.0; devkitSMS `561bc2cb9829cdf2e31ad8cd43835037c8c44a63`; Banjo `7aaf837ebbe67e567e019b5819b5467bf62253ad` | 65,536 bytes (4 × 16 KiB) | `4ef4f1865772cc1efdc69a7ec23cd5da0f7784ef2b9a08a86c7ab7bf4e1464ef` |

The SMS Test Suite build verifies the `TMR SEGA` signature at ROM offset `0x7FF0`; the `makesms` converter updates the SEGA checksum and the SDSC release date. The build fixes `SOURCE_DATE_EPOCH=1767225600` so the metadata date does not vary between runs.

## Build toolchains and layout

### SmsVdpTest

- Z80 Assembly source: `testmain.asm`.
- Build WLA-DX from the pinned source commit.
- Assemble with `wla-z80 -o object.o testmain.asm`.
- Link with `wlalink -d -r -v linkfile output.sms`.
- Validate non-empty output and a size divisible by 16 KiB.

The upstream `Compile.bat` argument order was not compatible with the WLA-DX command-line syntax used here; the workflow uses the explicit input/output order.

### SMS Test Suite

- C source compiled for Z80 with SDCC 4.5.0.
- Build SMSlib with `MD_PAD_SUPPORT` and `VDPTYPE_DETECTION`, matching the suite's compile-time requirements.
- Build PSGlib from the pinned devkitSMS sources.
- Use the pinned Banjo devkitSMS modules and the suite's `sn_opll_loop.rel`.
- Generate asset banks 2 and 3 as separate SDCC const segments. This avoids placing the asset payload over the fixed 32 KiB area and the ROM header.
- Link with `BANK2=0x28000` and `BANK3=0x38000`.
- Convert the extended IHX output with the bank-aware `makesms` converter, then verify the ROM size and `TMR SEGA` header.

The upstream `build.bat` references a legacy layout and the `ihx2sms` converter. In this build, the asset banks are explicitly assigned to ROM banks 2 and 3 and the bank-aware `makesms` converter is used. This also avoids the legacy converter's handling of extended IHX segments.

## Diagnostic relevance and limits

### SmsVdpTest — primary timing/VDP candidate

The suite includes cases for HCounter, VCounter, the timing of VCounter changes, Frame IRQ, Line IRQ, VINT flag timing, and the HCounter latch on TH transitions.

Its upstream documentation states that the tests require 228 CPU cycles per scanline and recommends ZEXDOC or ZEXALL before interpreting results. **Do not treat execution results as authoritative until the Z80 semantic closure prerequisite (E7) is demonstrably complete.** E8 must not be started ahead of that gate.

### SMS Test Suite — complementary hardware/AV checks

The suite provides video patterns, drop-shadow frame alternation, PSG/FM audio, controller/paddle tests, and console/TV identification. Its PAL/NTSC detection reports 50 Hz or 60 Hz mode; it is not a direct measurement of the emulator's master-clock frequency or frame cadence. It is therefore complementary, not the primary P0 clock-timing oracle.

## Validation boundary

A successful build proves that the pinned sources and toolchains produce ROM images with valid bank sizing; for SMS Test Suite, the SEGA header and checksum are also generated. It does **not** prove that the emulator passes the ROM tests, that VDP/CPU timing is cycle-accurate, or that physical audio/video presentation is synchronized.

No emulator runtime, CPU, VDP, PSG, Scheduler, worker, or host timing code is changed by this build workflow.

## Redistribution

The pinned SmsVdpTest and SMS Test Suite revisions did not expose an explicit license granting redistribution in the files inspected. The workflow checks out the upstream sources at pinned revisions and builds the ROMs in the CI runner, but does not commit or publish the generated ROM binaries. Revisit binary distribution only after obtaining an explicit license or permission from the respective authors.

## CI

The workflow is `.github/workflows/sms-vdp-homebrew-build.yml`. It runs both builds independently and records their output sizes and SHA-256 hashes in the job logs.
