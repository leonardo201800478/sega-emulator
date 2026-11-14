# SMS1 Mode 4 — sprite collision HBlank latch, phase 5

**ASIC:** Sega 315-5124
**Scope:** collision status timing and status-port acknowledgement
**Model:** scanline renderer; line-close is only a coarse proxy for the documented HBlank status latch
**Status:** focused regression added; CI validation pending

## Hardware evidence

The SMS Power sprite reference states that collision is set when opaque pixels from any two sprites overlap, independently of their positions in the eight-entry sprite buffer, and that the collision flag remains set until the status flags are read. It distinguishes opaque overlap from transparent pixels and states that sprite priority controls the displayed pixel, not whether the overlap is detected.

The SMS Official Documentation's I/O appendix describes VDP status as latched at HBlank and cleared by reading port $BF. SMS Power's Control Port reference likewise states that reading the status port resets all status flags.

References:
- [SMS Power — Sprites](https://www.smspower.org/Development/Sprites)
- [SMS Power — Control Port](https://www.smspower.org/Development/ControlPort)
- [SMS Official Documentation — Appendix B, Input/Output System](https://www.smspower.org/Development/SMSOfficialDocs)

## Existing implementation behavior

The shared SMS VDP composes a scanline when its 228-T-state line interval closes. During sprite composition it:
- ignores transparent color-zero sprite pixels;
- sets collision when an opaque pixel overlaps an earlier opaque sprite pixel;
- preserves the earlier SAT sprite as the displayed pixel;
- exposes the latched status through port $BF, whose read clears status.

This is directionally consistent with the documented HBlank status latch, but the current renderer commits at the end of the 228-T-state scanline, not at a measured HBlank-start dot. Treat line-close as a coarse approximation only. It is not a dot-accurate collision timestamp: the model does not identify the first overlapping pixel's H-counter position.

## Regression added

`sms1_sprite_collision_is_committed_at_scanline_close_and_cleared_by_status_read` checks:
1. no collision flag before the end of the current scanline;
2. collision is visible when the scanline renderer closes the line containing two opaque overlapping sprites;
3. reading $BF returns the collision flag and clears it;
4. the following scanline, whose corresponding sprite pattern row is transparent, does not reassert collision.

The test verifies the current scanline-level contract and status acknowledgement. It does not claim pixel-level collision timing.

## Remaining limits

- The exact dot at which the collision flag first asserts is not modeled.
- Sprite X down-counter and per-dot pattern shifting are not implemented as timed events.
- Mid-line VRAM/CRAM writes and raster register changes remain separate work.
