# SMS1 Mode 4 — overflow latching during SAT evaluation, phase 3

**ASIC:** Sega 315-5124
**Scope:** sprite overflow status timing relative to the SAT line buffer
**Implementation class:** scanline-level functional model
**Status:** merged to main and focused CI validation passed (PR #818)
**Physical subline oracle:** unavailable

## 1. Hardware evidence

The SMS VDP technical reference describes the following sequence:

1. On each scanline, the VDP parses the SAT to select sprites for the next scanline.
2. Selected sprites enter an internal eight-entry buffer.
3. If the eight entries are occupied and another sprite also intersects the target line, the overflow status flag is set during table parsing.
4. The status flag remains set until the VDP status/control port is read.

Reference: [Sega Master System VDP documentation, sprite table parsing](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt#L3127-L3155).

The same document says the sprite X coordinate acts as a down-counter on the next scanline, but explicitly presents that as an assumption based on the TMS9918 description rather than a verified SMS measurement. It also states that exact event positions within a scanline are not known and warns that its horizontal counter timing is derived from TMS9918/Genesis material, not confirmed for a real SMS. See [sprite output and caveat](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt#L3157-L3194) and [horizontal timing caveat](https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt#L3389-L3444).

## 2. Change

For SMS1/315-5124:

- The overflow flag is now latched when the next-line SAT evaluation is prepared.
- The renderer does not reassert overflow from an already-buffered evaluation.
- Reading the status port clears the latched overflow flag.
- A later SAT change affects a later evaluation, not the already-prepared line.
- SMS2 remains on its prior path in this phase.

The model boundary is still the preceding scanline's completion. The source supports the scanline-to-next-scanline relationship and the overflow-during-parse semantics, but it does not identify the exact dot/T-state within the preceding scanline when parsing begins or ends.

## 3. Regression

`sms1_sprite_overflow_is_set_during_sat_evaluation_and_clears_on_status_read` creates nine eligible sprites for the next line and verifies:

- overflow is visible immediately after the preceding scanline completes, before the target line is rendered;
- the reported sprite index is the ninth SAT entry;
- reading status clears overflow;
- rendering the already-prepared line does not reassert the cleared flag;
- a subsequent SAT update is observed by the next evaluation.

This validates the chosen scanline-level model. It does not prove the electrical phase of the real ASIC.

## 4. Remaining limitations

- The exact SAT parsing start/end dot is unknown.
- X down-counter behavior is described by the reference as an assumption, not verified SMS hardware fact.
- X/tile/pattern fetch is not yet modeled as timed bus activity.
- Sprite collision is still detected during scanline composition, not at an independently scheduled pixel event.
- Mid-line VRAM/CRAM visibility and raster writes remain separate blocked work.

Do not claim dot-accurate sprite timing from this phase.
