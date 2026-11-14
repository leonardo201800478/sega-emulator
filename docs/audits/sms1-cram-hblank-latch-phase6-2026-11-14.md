# SMS1 Mode 4 — CRAM HBlank latch, phase 6

**ASIC:** Sega 315-5124
**Scope:** visibility of CRAM writes across scanlines
**Class:** scanline-level palette latch approximation
**Status:** implementation and CI validation pending

## Evidence

SMS Power forum discussion on mid-scanline changes reports that palette values are cached during HBlank and take effect on the next scanline. A separate discussion reports that active-display CRAM writes can produce a brief single-pixel 'CRAM dot' glitch, and recommends palette updates off-screen.

References:
- [SMS Power — mid scanline changes](https://www.smspower.org/forums/13110-MidScanlineChanges)
- [SMS Power — writing to VDP and VRAM outside VBLANK](https://www.smspower.org/forums/18738-WritingToVDPAndVRAMOutsideVBLANK)
- [SMS Power — H-counter/active-display ranges](https://www.smspower.org/Development/GGOfficialDocs%26num%3D9)

These discussions establish the qualitative HBlank caching behavior and the existence of CRAM dots, but do not specify the exact ASIC dot at which the palette latch occurs.

## Model chosen

For SMS1/315-5124 only:
- `cram` remains the CPU-visible writable CRAM state.
- `latched_cram` is the palette snapshot used to render the current scanline.
- The model captures CRAM at HPos 48 / Z80 T-state 32, where the current H-counter conversion enters active display HCounter 00.
- A CRAM write before that boundary is visible on the current line; a write after it is visible starting on the next line.
- The VDP `advance()` loop splits at the latch boundary so whole-line and chunked advances preserve the same result.
- SMS2 remains unchanged in this phase.

The HPos 48/T32 boundary is an explicit model choice derived from the existing H-counter mapping and the HBlank-cache description. It is not claimed to be a measured 315-5124 latch timestamp.

## Regression coverage

- `sms1_cram_write_during_active_display_is_visible_from_the_next_scanline`: write after the latch; current line keeps the old color, next line uses the new color.
- `sms1_cram_write_before_hblank_latch_affects_the_current_scanline`: write before the latch; current line uses the new color.
- `sms1_cram_hblank_latch_is_chunking_invariant`: split and whole advances produce identical VDP state.
- Existing SMS2 scanline characterization remains unchanged.

## Known omission

The active-display single-pixel CRAM dot/glitch is not simulated. VRAM writes remain immediate in backing VRAM and are still sampled by the scanline renderer at line completion; exact tile/pattern fetch slots and mid-line VRAM visibility remain separate work. Do not infer full raster accuracy from the CRAM latch alone.