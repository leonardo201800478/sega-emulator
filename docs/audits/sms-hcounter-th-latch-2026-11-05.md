# SMS H-counter latch — TH transitions

**Date:** 2026-11-05  
**PR:** #544 (stacked on PR #542 CPU/VDP I/O synchronization)

## Observed defect

The previous `SmsVdp::read_h_counter()` returned a linearly interpolated free-running value whenever no Light Phaser click was active. The SMS VDP contract instead exposes an H-counter latch associated with transitions on the controller TH line. The value remains frozen until another qualifying transition.

The previous model also had no path from software writes to I/O control port $3F to the VDP H-counter latch.

## Evidence and model sources

- SMS Power VDP documentation: https://www.smspower.org/Development/SMSVDP
  - Describes H-counter capture in relation to TH pin state changes.
- MAME SMS VDP implementation: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
  - Provides a comparative H-counter conversion from the 342-position horizontal line, offset by 46 positions, with the $E9-$FF wrap region.
- Ares SMS VDP: https://github.com/ares-emulator/ares/blob/master/ares/ms/vdp/io.cpp
  - Keeps H-counter latch state in VDP I/O state and resets it with the VDP.

MAME/Ares are independent emulator models, not hardware oracles. The conversion and reset value still require a hardware/test-ROM comparison.

## Implemented slice

- Store a persistent `h_counter_latch` in `SmsVdp`; reset value is deterministically initialized to zero.
- `read_h_counter()` returns the latched value rather than a free-running approximation.
- Software changes to TH output levels through port $3F are detected in `SmsMachineBus::write_port`; the VDP captures the H-counter at the synchronized I/O boundary.
- The existing synthetic Light Phaser event captures its horizontal position into the same latch.
- The current Light Phaser +8-count calibration is preserved; this patch does not claim to validate its optical/pulse offset.

## Regression coverage

- H-counter sequence around the $93 → $E9 → $FF → $00 wrap.
- Latched value remains stable while raster time advances.
- A second TH transition captures a new value.
- Writes to port $3F capture the H-counter at the CPU-synchronized I/O boundary.
- Existing Light Phaser raster-line test continues to validate its synthetic click position.

## Remaining limitations

- Light Phaser pulse duration and horizontal event timing are still scanline-granular.
- H-counter reset value and conversion need comparison with a hardware test ROM or measured console.
- The implementation does not claim pin-level or dot-accurate behavior.
- Save-state serialization of the latch depends on the broader SMS snapshot contract.

## Validation status

- Local Rust tests: NOT RUN in this session.
- CI: pending on the final branch HEAD.
- Hardware / real-game H-counter test: NOT RUN.
