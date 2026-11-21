# SMS Plus GX / CrabEmu SN76489 trace adapter

This directory contains a small, project-authored C adapter for the CrabEmu PSG engine shipped as an option in SMS Plus GX. It is built against an external checkout pinned to a specific commit. The upstream C source and header are not copied into Sega Emulator and are never linked into the Rust backend.

## Reference pin

- Repository: https://github.com/libretro/smsplus-gx
- Commit: `3844b46caa926b6494987b97da63092818c4ddef`
- Engine: `source/sound/crabemu_sn76489/sn76489.c`
- Header: `source/sound/crabemu_sn76489/sn76489.h`
- License: GPL-2.0 for the CrabEmu engine. The reference is built only in an ephemeral CI runner.

SMS Plus GX also contains separate Maxim and MAME-derived engines. This adapter intentionally selects CrabEmu; it does not treat those engines as independent confirmations.

## Build

Clone the exact reference commit outside this repository:

```sh
git clone https://github.com/libretro/smsplus-gx.git /tmp/smsplus-gx
git -C /tmp/smsplus-gx checkout 3844b46caa926b6494987b97da63092818c4ddef
```

Compile the adapter with a C11 compiler:

```sh
cc -std=c11 -O2 -Wall -Wextra \
  -I/tmp/smsplus-gx/source/sound/crabemu_sn76489 \
  tools/psg-crabemu-oracle/trace.c \
  /tmp/smsplus-gx/source/sound/crabemu_sn76489/sn76489.c \
  -o /tmp/psg-crabemu-trace
```

The adapter initializes the SMS profile with 16 LFSR bits and taps 0/3 (`0x0009`). It sets the reference sample rate to input clock /16, so each adapter call advances one PSG internal clock. The reference's sample is computed before its counters and LFSR are advanced; both pre- and post-update state are recorded.

## Event format

CSV rows contain an internal PSG tick index and one PSG data byte:

```csv
# tick,byte
0,0xE4
0,0xF0
540,0xE0
560,0xE4
```

Rows must be ordered by nondecreasing tick. Multiple writes at one tick are applied in file order before that tick's sample is generated. The adapter does not emulate Z80 bus cycles; it is a chip-level diagnostic input schedule.

Run:

```sh
/tmp/psg-crabemu-trace --calls 600 --events events.csv > reference.jsonl
```

## Rust core trace

The companion Rust example emits the Sega Emulator's current SMS PSG state using the same event schedule and one Scheduler-domain PSG tick per row:

```sh
cargo run --locked -p sega-sn76489 --example psg_state_trace -- \
  --calls 600 --events events.csv > core.jsonl
```

The sample rate of the Rust core affects its PCM queue/resampler state but not the PSG hardware state. The example defaults to 48 kHz.

## Optional Gearsystem state trace

The same tool directory contains a second project-authored adapter for Gearsystem's SMS APU. It reads the public `Sms_Apu::GetState()` diagnostic state and compiles only the external APU and Blip_Buffer modules from the pinned checkout.

Reference pin:

- Repository: https://github.com/drhelius/Gearsystem
- Commit: `1be9a12b82cae84ddd6f9b7006d546abe2d58bd4`
- Source files: `src/audio/Sms_Apu.cpp`, `src/audio/Blip_Buffer.cpp`
- The repository is GPLv3; the two audio modules carry LGPL-2.1 notices. Oracle only.

Build:

```sh
git clone https://github.com/drhelius/Gearsystem.git /tmp/gearsystem
git -C /tmp/gearsystem checkout 1be9a12b82cae84ddd6f9b7006d546abe2d58bd4

g++ -std=c++11 -O2 -Wall -Wextra -include cassert \
  -I/tmp/gearsystem/src/audio \
  tools/psg-crabemu-oracle/gearsystem_trace.cpp \
  /tmp/gearsystem/src/audio/Sms_Apu.cpp \
  /tmp/gearsystem/src/audio/Blip_Buffer.cpp -lm \
  -o /tmp/psg-gearsystem-trace
```

Run with the same CSV schedule:

```sh
/tmp/psg-gearsystem-trace --calls 600 --events events.csv > gearsystem.jsonl
python3 tools/psg-crabemu-oracle/compare.py reference.jsonl core.jsonl gearsystem.jsonl
```

One row advances 16 Z80 T-states (one PSG input-divider period). The adapter attaches a Blip_Buffer so Gearsystem's oscillator state actually advances. Gearsystem uses a Galois-form LFSR; the comparator deliberately does not compare its raw state integers to the core's Fibonacci state. It compares bit-0 observations and timestamped shift events.

## Compare

```sh
python3 tools/psg-crabemu-oracle/compare.py reference.jsonl core.jsonl > comparison.json
```

The comparator uses identical tick indices and never aligns traces by correlation. It reports:

- first reference and core LFSR-shift tick;
- raw LFSR state mismatch count and first mismatch;
- reference sample bit versus the Rust core's LFSR bit before the tick;
- reference sample bit versus the Rust core's latched noise output;
- SHA-256 digests of both trace streams.

Differences are diagnostic results, not automatically test failures. In particular, the output-bit boundary and initial divider phase are open hardware questions. The existing `.github/workflows/ci.yml` PSG-focused job builds both pinned external adapters (CrabEmu and Gearsystem), checks trace determinism and their known scheduling behavior, then uploads all traces and the comparison summary as part of the PSG artifact.

## Noise-rate transition and Tone 2 tracking fixture

CI also captures a 1,400-tick diagnostic schedule across CrabEmu, Gearsystem,
Ares and the Rust SMS PSG core. It deliberately exercises all four noise-rate
selections and transitions while Tone 2 changes:

- Tick 0: Tone 2 period 7, white noise rate 0.
- Tick 256: white noise rate 1.
- Tick 512: white noise rate 2.
- Tick 768: white noise rate 3 (Tone 2-derived).
- Ticks 800, 832, 864 and 896: Tone 2 periods 8, 0, 1 and 7.
- Tick 1024: periodic noise rate 3.
- Tick 1100: white noise rate 3.
- Tick 1200: white noise rate 0.

All four adapters receive the same timestamped write schedule. Each capture is
repeated and compared byte-for-byte to establish adapter determinism. The
four-reference comparator then reports same-index differences without phase
alignment. This fixture is diagnostic: a mismatch is evidence to inspect, not
an automatic failure or a hardware-truth vote. It does not establish that the
four implementations share an identical clock-domain interpretation.

## Interpretation constraints

- The reference sample is computed from the current LFSR bit before the reference advances the divider and LFSR.
- The Rust trace exposes the core's current LFSR and its separately latched `noise_output`.
- A mismatch between the reference sample bit and `noise_output` does not by itself prove which model is physically correct; it identifies the exact tick and state where the conventions diverge.
- This adapter does not validate analog gain, filtering, PSG-to-FM routing, or host resampling.
- No reference code is copied, translated or vendored into the production crate.

## Optional Ares SN76489 component trace

The directory also contains a project-authored adapter for the pinned Ares
SN76489 component. Ares is checked out externally in CI; the exact upstream
`sn76489.cpp`, `sn76489.hpp`, and `serialization.cpp` are compiled directly
from that checkout. No Ares implementation source is copied into this repo.

Reference pin:

- Repository: https://github.com/ares-emulator/ares
- Commit: `8bc30c8c3d3c8081d2a66587add602df771441dd`
- Component: `ares/component/audio/sn76489`
- License: the Ares repository's permissive project license (ISC-style notice).

`ares-shim/ares/ares.hpp` is project-authored compatibility glue for the
small fixed-width integer, array, and serializer vocabulary expected by the
component. It implements width truncation and bit-slice access only; it does
not implement any PSG behavior. The adapter derives from the component to
expose diagnostic state and records the component's four channel outputs.

Build against an external checkout:

```sh
git clone https://github.com/ares-emulator/ares.git /tmp/ares
git -C /tmp/ares checkout 8bc30c8c3d3c8081d2a66587add602df771441dd

g++ -std=c++11 -O2 -Wall -Wextra \
  -Itools/psg-crabemu-oracle/ares-shim \
  -I/tmp/ares/ares/component/audio/sn76489 \
  tools/psg-crabemu-oracle/ares_trace.cpp \
  /tmp/ares/ares/component/audio/sn76489/sn76489.cpp \
  -o /tmp/psg-ares-trace

/tmp/psg-ares-trace --calls 600 --events events.csv > ares.jsonl
```

Ares' component clock is traced one call at a time. This is not an assertion
that its host integration clock divider is interchangeable with every other
reference's internal tick; the adapter header explicitly identifies this
clock domain. The first component call also exercises Ares' reset-time
counter/flip phase. Same-index observations are retained without automatic
alignment.


## Tone-boundary and Tone 2 noise-sync fixture

CI also generates a second event stream dedicated to the tone oscillators:

- Tone 0 period 0 and Tone 1 period 1 are established at reset.
- Tone 2 is changed through periods 7, 8, 0, 1 and 7.
- Noise is configured with register value `0xE7` (Tone 2 rate, white-noise feedback).
- The same writes are delivered to Ares, Gearsystem and the Rust core at identical internal tick indices.
- Ares and Rust pre/post states are compared field-by-field; Gearsystem's four channel periods/phases/volumes and noise state are captured separately.

The fixture is deliberately separate from the noise-only CrabEmu sample comparison: enabling tone channels would make its mixed sample unsuitable as a direct observation of the isolated noise bit. No automatic phase shift is introduced. The resulting mismatch counts are diagnostic, not pass/fail hardware assertions.
