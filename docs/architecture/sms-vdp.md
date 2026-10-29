SMS VDP architecture / implementation status

This branch adds an independent SmsVdp in Rust and leaves native/sg1000_core/src/vdp.rs unchanged. The TMS9918A Rust implementation is used as the behavioral and testing reference for the SMS legacy modes; Mode 4 has its own address generation and renderer.

Implemented:
- 16 KiB VRAM, 32-byte CRAM with 6-bit colors, $BE/$BF ports, command latch, read-ahead, address wrap and mirrored port decode.
- SMS Mode 4: 4bpp tiles, flips, palettes, background priority, scroll and locks, left-column mask, 64-entry sprite table, 8 sprites per line, overflow/collision, 8x8/8x16 and zoom.
- Legacy TMS-compatible Graphics I, Text, Multicolor and Graphics II modes, using the fixed 16-color TMS palette and legacy 4-sprites-per-line policy.
- NTSC/PAL raster profiles, 192/224/240-line selections, VBlank, line counter/IRQ, V/H counter ports and CPU instruction-boundary integration.
- Optional unlocked Mode 4/legacy sprite drawing while preserving hardware overflow indication.
- Host-normalized Light Phaser click/aim creates a raster-qualified TH pulse and latches an active-display H-counter bucket. This is a compatibility input, not analog photodiode emulation.
- SegaScope eye frame capture with alternating/latest-eye, red/cyan anaglyph and side-by-side standard-monitor presentation.

Known limits:
- No dot/T-state-exact timing, VRAM contention, mid-scanline register effects, or sub-instruction IRQ edges.
- H-counter phase outside the synthetic click latch is approximate; region/revision-specific V-counter details and SMS1/SMS2 ASIC quirks need additional oracle-backed tests.
- 224/240 modes are exposed but not yet gated by VDP/console revision; NTSC 240 is compatibility-only.
- The CRAM conversion is a deterministic 2-bit/channel digital palette, not analog encoder/composite output or measured per-revision colorimetry.
- Light Phaser pulse is synthetic from click + logical aim and raster row; it does not evaluate pixel luminance.
- SegaScope presentation is a normal-monitor adaptation, not physical shutter timing. Stereo captures are frame-boundary snapshots.
- The implementation is not yet cycle-exact and requires independent ROM/image oracles before claiming complete VDP fidelity.

References:
https://www.smspower.org/Development/VDPRegisters
https://www.smspower.org/Development/Modes
https://www.smspower.org/Development/Sprites
https://www.smspower.org/Development/VDP-Index
https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp

Three review workstreams: (1) VDP/rendering; (2) timing/CPU/I/O integration; (3) independent verification. These are planned review roles, not three autonomous AI agents spawned by this session. GitHub Actions are automated validators.
