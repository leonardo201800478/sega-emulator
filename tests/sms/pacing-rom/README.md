# SMS frame pacing reference ROM

This is an original deterministic Sega Master System homebrew program used as an external workload for the P0 frame-pacing investigation. It contains no commercial ROM bytes, BIOS image, third-party graphics, or game assets.

## Workload profiles

| Profile | Compile-time value | Per-frame work |
|---|---:|---|
| CPU-light | 0 | VBlank synchronization and deterministic name-table marker update |
| CPU-fixed | 1 | CPU-light plus a fixed 256-iteration arithmetic workload |
| VDP/sprites | 2 | CPU-light plus eight deterministic sprite attribute updates |
| Combined | 3 | CPU-fixed + VDP/sprites + audible deterministic PSG tone-period update |

All profiles use the same video mode, palette, tile set, initial name table, frame boundary and frame counter. The PSG is muted in CPU-light, CPU-fixed and VDP/sprites profiles; only the combined profile enables the audible tone and updates its period each frame. The visual marker is a workload signature, not a physical presentation oracle.

## Reproducible build

Pinned inputs:

- SDCC 4.6.0;
- devkitSMS commit `561bc2cb9829cdf2e31ad8cd43835037c8c44a63`;
- devkitSMS `crt0/crt0_sms.rel`, `SMSlib/src/SMSlib.c`, and `SMSlib/src/SMSlib_sprite.c`;
- converter source `tools/makesms/src/makesms.c` from the same checkout.

The build script checks both compiler version and exact devkitSMS revision, links real SMSlib definitions (without suppressing unresolved symbols), validates the ROM size, and records SHA-256 provenance.

## Full-machine execution gate

`tests/sms/pacing-rom/run_profiles.py` runs every ROM twice using the existing `tests/sms/scripts/run_rom.py` wrapper and the full-machine `sms_rom_runner` from the SMS external diagnostic runner branch. Each run uses `SmsTemporalMachine`, Scheduler, Z80, VDP, PSG and mapper-backed memory. The gate checks:

- 600 completed frames for each workload per run;
- `EXECUTED_NO_ORACLE` summary (execution only, never a functional PASS);
- identical JSONL records across the two executions of each profile;
- deterministic ROM and emulator provenance.

The offline runner advances emulated time as fast as the host allows. This validates ROM execution and deterministic temporal progression; it does **not** measure real-time worker cadence, WGPU presentation, Windows VSync, or physical frame pacing. Those require a separate live-host capture with PresentMon/ETW and runtime telemetry.

A successful compile or frame budget is not a hardware certification. VBlank/VDP/PSG behavior and visual output still require independent validation.

## Build locally

```bash
DEVKIT_SMS_ROOT=/path/to/devkitSMS \
OUT_DIR=/tmp/sega-sms-pacing \
SDCC_BIN=/path/to/sdcc/bin/sdcc \
bash tests/sms/pacing-rom/build.sh
```

Run the full-machine regression on a checkout containing `sms_rom_runner`:

```bash
python tests/sms/pacing-rom/run_profiles.py \
  --rom-dir /tmp/sega-sms-pacing \
  --report-dir /tmp/sega-sms-pacing-reports
```

ROM binaries and generated reports stay outside Git. The source in this directory is original project code under the repository MIT License; pinned devkitSMS sources are consumed from an external checkout and are not copied into this repository.
