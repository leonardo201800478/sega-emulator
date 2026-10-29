# SMS memory, I/O and peripheral boundary

## Implemented in this slice

- SmsMachine joins the existing SMS Z80 executor to the existing standard Sega
  cartridge mapper and 8 KiB mirrored work RAM through the shared CpuBus trait.
- CPU instruction fetches, memory reads/writes, and IN/OUT now traverse the same
  SMS-owned bus implementation.
- Controller ports $DC and $DD use active-low input bits and the documented
  address mirrors. Port $3F stores the TH/TR direction/output control latch.
- Input snapshots are host-independent. The Qt/XInput/SDL host must sample input
  at the frontend boundary and pass the snapshot to Rust; no host API runs per
  instruction.
- Light Phaser trigger and aim coordinates can be supplied by a controller/mouse.
  The active-low light-pulse line is deliberately separate and must be qualified
  by future VDP raster/light detection. A mouse position alone does not fake a
  detected bright pixel.
- Writes to $FFF8-$FFFB update the SegaScope left/right shutter latch while
  preserving the mirrored work-RAM write/read behavior.

## Port map in this slice

| Port | Implemented behavior |
|---|---|
| $DC, mirrors $C0-$FE even | P1 directions/buttons and P2 up/down |
| $DD, mirrors $C1-$FF odd | P2 directions/buttons, reset input, TH lines |
| $3F, mirrors odd $01-$3F | TH/TR directions and output levels |
| Other reads | Open-bus placeholder $FF |
| Other writes | Ignored until the corresponding device exists |

## Explicit limits

This is not a complete SMS console and does not claim ROM gameplay. The existing
mapper covers standard Sega mapper banking and work RAM, not cartridge SRAM or
all mapper variants. Port $3E memory-enable behavior, VDP ports $BE/$BF, PSG
writes, FM, interrupts, reset hardware variants, and scheduler integration
remain pending. CPU T-state totals are instruction-level, not cycle-exact.

The SegaScope latch is implemented as a bus side effect only. Real stereoscopic
output requires two correctly rendered eye images, alternating frame presentation,
and synchronized host shutter state. The VDP and scheduler are not yet present.
The Light Phaser aim/trigger input is represented, but accurate hit detection
requires the future VDP to report raster-qualified light pulses.

## Technical references

- SMS Power, [Memory Map](https://www.smspower.org/Development/MemoryMap)
- SMS Power, [Peripheral Ports](https://www.smspower.org/Development/PeripheralPorts)
- SMS Power, [Light Phaser](https://www.smspower.org/Development/LightPhaser)
- SMS Power, [3D Glasses](https://www.smspower.org/Development/3DGlasses)
