# SMS1 Mode 4 — first-four horizontal sprite zoom, phase 4

**ASIC:** Sega 315-5124
**Scope:** sprite zoom quirk
**Class:** functional renderer behavior
**Status:** implementation and CI validation pending

## Evidence

The Sega Master System VDP reference describes an original SMS VDP quirk: when sprite zoom is enabled, only the first four sprites selected for a scanline are enlarged horizontally and vertically; the remaining selected sprites are enlarged vertically only. SMS2 and Game Gear apply horizontal and vertical zoom to all eight permitted sprites.

Reference: [Sega Master System VDP documentation, sprite zoom](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt#L3105-L3125).

This wording means that if fewer than four sprites are selected, all selected sprites are among the first four and therefore receive horizontal zoom. The previous expression based on `count - 4` inverted that condition for counts below eight.

## Change

For SMS1/315-5124 with 2x zoom:
- every selected sprite remains vertically doubled;
- SAT ranks 0 through 3 are horizontally doubled;
- SAT ranks 4 and above remain 8 pixels wide.

SMS2/GG behavior is unchanged and continues to apply horizontal zoom to all selected sprites.

## Regression

`sms1_horizontal_zoom_applies_to_first_four_selected_sprites` tests sprite counts 2, 4, 6 and 8:
- counts 2 and 4: every selected sprite is 16 pixels wide;
- count 6: first four are 16 pixels, last two are 8;
- count 8: first four are 16 pixels, last four are 8.

This is a functional quirk regression based on the documented behavior; no electrical timing claim is made.
