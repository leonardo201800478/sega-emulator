SMS VDP architecture / first implementation slice

This branch adds an independent SmsVdp in Rust and leaves native/sg1000_core/src/vdp.rs untouched.

Implemented: 16 KiB VRAM, 32-byte CRAM with 6-bit colors, $BE/$BF ports, control latch, read-ahead, address wrap, Mode 4 tilemaps and 4bpp patterns, tile flips and palettes, background priority, scroll and locks, left-column mask, 64 sprite table entries, 8-per-line hardware limit, unlocked compatibility drawing while preserving overflow status, collision, 8x8/8x16 and zoom, regional 262/313-line timing, 192/224/240 height selection, VBlank and line IRQ, V/H counter ports, raster-qualified mouse-click Light Phaser pulse, SegaScope eye frame capture and alternating/anaglyph/side-by-side output.

Limits: legacy TMS modes 0-3 are not rendered yet; H-counter phase is approximate outside the synthetic click latch; CPU/VDP integration is instruction-boundary timed, not dot-cycle exact; SMS1/SMS2 ASIC mask and zoom quirks are not selectable; extended modes need model gating and NTSC 240 is compatibility-only; palette is a deterministic digital approximation, not analog video; mouse clicks synthesize a raster event and do not simulate photodiode luminance; SegaScope composition is a host display adaptation, not LCD shutter physics.

References: https://www.smspower.org/Development/VDPRegisters ; https://www.smspower.org/Development/Modes ; https://www.smspower.org/Development/Sprites ; https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt ; https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp

Three workstreams: (1) VDP/rendering; (2) timing and CPU/I/O integration; (3) independent test/fidelity audit. These are planned review roles, not three autonomous agents spawned by this session; GitHub Actions are validators, not agents.