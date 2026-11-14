# SMS1 Mode 4 — zoom rule source reconciliation

**ASIC:** Sega 315-5124
**Scope:** horizontal sprite zoom when R1 bit 0 is set
**Decision:** retain the SMS Power N-minus-four rule

## Conflicting references

Two references use different wording:

- SMS Power, VDP Registers, states that if N sprites are on a scanline, the first N minus 4 are double-width, with all SMS1 sprites vertically doubled. SMS2/GG apply horizontal doubling to all allowed sprites.
- EmulationResources' Sega Master System VDP document says only the first four of the eight shown are horizontally doubled and the remaining four are vertically doubled. This wording is ambiguous when fewer than eight sprites are selected.

The project names SMS Power as its primary technical reference. The implementation therefore follows the explicit N-minus-four rule, which also matches the previous regression coverage for N = 2, 4, 6 and 8.

References:
- [SMS Power — VDP Registers, R1 bit 0](https://www.smspower.org/Development/VDPRegisters)
- [EmulationResources — SMS VDP documentation, sprite zoom](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt#L3105-L3125)

## Implemented contract

For N selected SMS1 sprites:
- the first N−4 receive 2x horizontal and vertical scaling;
- the final four receive vertical scaling only;
- for N ≤ 4, no sprite receives horizontal scaling.

SMS2/GG retain 2x horizontal scaling for all selected sprites.

The proposed rank < 4 change was withdrawn and must not be used as the SMS1 contract. The regression is sms1_horizontal_zoom_skips_last_four_selected_sprites.
