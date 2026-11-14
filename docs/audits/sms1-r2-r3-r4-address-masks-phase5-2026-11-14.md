# SMS1 Mode 4 — R2/R3/R4 address masks, phase 5

**ASIC:** Sega 315-5124
**Scope:** Mode 4 name-table and background pattern address generation
**Class:** functional address decoding
**Status:** implementation and CI validation pending

## Evidence

SMS Power documents the differences between SMS1 and SMS2 VDP address generation:
- R2 bit 0 is ANDed with the high bit of the name-table Y coordinate.
- R3 bits 7–0 gate address bits 13–6 for background pattern fetches in planes 0 and 1.
- R4 bits 2–0 gate address bits 13–11 for background pattern fetches in planes 2 and 3.
- SMS2/GG feed ones into these gates, so the address bits are not masked.

Reference: [SMS Power — VDP Registers, R2/R3/R4](https://www.smspower.org/Development/VDPRegisters).

## Implementation

- R2 bit 0 is applied to row bit 4 in 192-line Mode 4 name-table addressing for SMS1 only.
- R3 mask is applied independently to plane 0/1 pattern addresses.
- R4 mask is applied independently to plane 2/3 pattern addresses.
- The fine-scroll tile-$100 selector fetch uses the same R3 plane-1 address mask.
- SMS2/GG continue to use unmasked Mode 4 background pattern addresses.

## Regression coverage

Public-port tests exercise the actual SMS1 profile and complete frame path:
- R2 bit 0 mirrors row 16 to row 0 when clear and exposes row 16 when set.
- R3 bit 0 gates tile-address bit 6 for planes 0/1.
- R4 bit 0 gates tile-address bit 11 for planes 2/3.

These tests validate address decoding and visible frame output. They do not assert subline fetch timing or bus arbitration.

## Remaining limits

- The masks are applied when the scanline renderer resolves background tile fetches; per-dot VRAM fetch timestamps remain unmodeled.
- The exact point at which mid-line writes to R2/R3/R4 become visible remains part of the separate raster-event audit.
- SMS2/GG regression coverage must continue to verify that these SMS1-only masks are ignored there.