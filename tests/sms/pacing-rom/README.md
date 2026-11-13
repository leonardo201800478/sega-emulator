# SMS frame pacing reference ROM

This is an original, deterministic Sega Master System homebrew program for measuring the emulator's frame-production path. It contains no commercial ROM bytes, BIOS image, third-party graphics, or third-party game assets.

## Workload profiles

The same source is compiled into four independent ROMs:

| Profile | Compile-time value | Per-frame work |
|---|---:|---|
| CPU-light | 0 | VBlank synchronization and one deterministic name-table marker update |
| CPU-fixed | 1 | CPU-light plus a fixed 256-iteration arithmetic workload |
| VDP/sprites | 2 | CPU-light plus eight deterministic sprite attribute updates |
| Combined | 3 | CPU-fixed + VDP/sprites + audible deterministic PSG tone-period update |

All profiles use the same video mode, palette, tile set, initial name table, frame boundary, and deterministic frame-signature marker. The PSG is muted in CPU-light, CPU-fixed and VDP/sprites profiles; only the combined profile enables the audible tone and updates its period each frame. The sprite and tile marker movement is a visual workload signature, not a measurement of physical presentation.

The initial target is USA/NTSC. The runner must explicitly select the USA profile when executing these images. Region-specific ROM behavior is not inferred from the image itself.

## Build

The build is pinned to:

- SDCC 4.6.0 (official upstream Linux x64 archive in CI);
- devkitSMS commit `561bc2cb9829cdf2e31ad8cd43835037c8c44a63`;
- devkitSMS `crt0/crt0_sms.rel`, `SMSlib/src/SMSlib.c`, and `SMSlib/src/SMSlib_sprite.c` (the startup initialization path references the SMSlib core ISR/init symbols and sprite-table initialization);
- devkitSMS `tools/makesms/src/makesms.c`.

The build script checks the exact devkitSMS Git revision and SDCC version line. It compiles the four profile variants and the required SMSlib core and sprite objects, links the startup object with those real definitions (without suppressing unresolved symbols), converts each IHX image using the converter source from the same pinned devkitSMS checkout, checks the ROM size and records SHA-256 plus build provenance. Each `.build-info.txt` records the checked-out project commit, source SHA-256, USA/NTSC region, SDCC version line, host C compiler version, source/SMSlib/link/converter flags, hashes of `makesms` source/binary and devkitSMS CRT0/peephole rules, and the generated IHX/ROM hashes.

Run from the repository root:

```bash
DEVKIT_SMS_ROOT=/path/to/devkitSMS \
OUT_DIR=/tmp/sega-sms-pacing \
SDCC_BIN=/path/to/sdcc/bin/sdcc \
bash tests/sms/pacing-rom/build.sh
```

The output directory is external to the repository. ROM binaries and generated build metadata must not be committed.

## Execution and interpretation

These images are intended to be run by the full-machine SMS runtime, with the worker's actual frame pacing path. They are not CPU-only Z80 tests and are not a replacement for SMSmemtest.

A successful compile proves only that the source builds. It does not prove VDP, PSG, VBlank, worker cadence, or physical presentation correctness. Until the full-machine ROM runner is available on the target branch, runtime execution remains **BLOCKED**.

The visual frame marker is not a frame-presentation oracle. Physical presentation must be correlated separately with PresentMon/ETW or equivalent OS-level telemetry.

## License

The source in this directory is original Sega Emulator project code and is covered by the repository's MIT License. The pinned devkitSMS files are consumed from an external checkout at build time; they are not copied into this repository.
