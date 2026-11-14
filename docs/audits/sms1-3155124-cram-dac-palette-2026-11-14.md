# SMS1 315-5124 — CRAM DAC output palette

**ASIC:** Sega 315-5124  
**Scope:** conversion of 6-bit CRAM entries to RGB24 framebuffer values  
**Implementation:** revision-specific digital output-level table  
**Status:** functional implementation and focused tests pending CI; physical calibration remains open

## Evidence and provenance

The MAME 315-5124 implementation uses these 4-level output tables:

- Red and green: `[0, 78, 160, 238]`
- Blue: `[0, 98, 160, 238]`

Its source comments identify the blue-channel nonlinearity as verified from a die shot and reference the RetroRGB investigation. The red/green levels are attributed to the Sega-16 output-level discussion. RetroRGB summarizes oscilloscope measurements showing that the 315-5124 blue channel does not follow the red/green output-voltage levels. ConsoleMods' hardware notes distinguish 315-5124 systems from 315-5246 systems and describe the blue offset as internal to the chip.

References:
- [MAME — 315_5124.cpp, sega315_5124_palette](https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp) — comparative emulator implementation; includes source provenance comments
- [RetroRGB — Sega Master System non-linear blue channel findings](https://retrorgb.com/sega-master-system-non-linear-blue-channel-findings.html) — community measurement summary
- [ConsoleMods — Master System Video Output Notes](https://consolemods.org/wiki/Master_System:Video_Output_Notes) — hardware revision distinction and output observation
- [SMS Power — VDP Registers](https://www.smspower.org/Development/VDPRegisters) — 6-bit CRAM layout

These are community measurement and independent implementation sources, not a Sega primary electrical specification. The implementation therefore records the adopted RGB conversion as a comparative reference profile, not a fully calibrated analog model.

## Change

The SMS1/315-5124 Mode 4 renderer now converts each CRAM channel using the revision-specific output levels above. The blue channel's level 1 is 98 rather than 78.

SMS2/315-5246 intentionally retains the previous project mapping in this phase. Its separate MAME table is `[0, 89, 174, 255]`, but changing it belongs to the SMS2 profile phase and must receive its own regression/audit.

The CRAM storage contract remains unchanged: 32 entries, six bits per entry. No timing, CRAM write visibility, or analog filtering behavior is changed.

## Regression

`sms1_cram_palette_uses_315_5124_output_levels_and_nonlinear_blue` covers:
- R level 1 and maximum;
- G level 1 and maximum;
- B level 1 (the non-linear 98 value) and maximum;
- all channels at maximum.

`sms2_palette_output_remains_unchanged_during_sms1_calibration` prevents this SMS1 correction from silently changing the SMS2 output path.

Existing SMS1 rendering tests now expect the 315-5124 output levels. SMS2 rendering tests retain their prior expectations.

## Remaining validation

- Capture all 64 CRAM colors from a physical 315-5124 board using a calibrated RGB capture chain.
- Record board/ASIC marking, output connector, cable, capture device, levels, gamma/range settings and measurement tolerance.
- Compare the physical captures with the MAME-derived RGB table and adjust only with reproducible evidence.
- Keep composite video, RF, display calibration and CRT phosphor characteristics outside this digital CRAM conversion function.
