# SMS VDP — sprite evaluation, phase 1

**Branch:** `fix/sms-vdp-sprite-evaluation-20261114`  
**Base:** `76795360896bfc8651ecf86f0a85ce7c18597875`  
**Status:** refactor and focused unit coverage; no temporal-latch claim.

## Motivation

The active Mode 4 renderer previously combined SAT scanning, candidate geometry, overflow selection, pattern lookup, and pixel composition in one scanline function. This made it difficult to model the hardware's separate SAT evaluation and sprite drawing phases.

SMS Power's sprite documentation states that the VDP parses the SAT on one scanline to select sprites for the next line, in table order, with an eight-entry internal buffer and a 192-line terminator. MAME's 315-5124 implementation also separates `select_sprites()` from `draw_sprites_mode4()`. These are comparative references, not physical proof of every timing detail.

References:
- https://www.smspower.org/Development/Sprites
- https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp

## Changes

- Extracted `evaluate_mode4_sprites()` as a pure SAT candidate-selection function.
- Kept the existing renderer consuming its ordered candidate list, preserving current rendering behavior in this phase.
- Added tests for SAT order, Y-coordinate +1 rule, candidate row, and the 192-line `$D0` terminator.

## Deliberate non-goals

This phase does **not** yet implement the hardware's persistent eight-sprite-number line buffer or prove its exact evaluation boundary. The active candidate list is still produced for the line being rendered. Sprite X/tile/pattern data are still read in the same rendering pass. No change to overflow/collision visibility timing is claimed.

The next phase must add an explicit per-line selected-sprite-number buffer, determine the evaluation boundary against diagnostic ROMs and independent implementations, and keep X/tile/pattern fetch semantics distinct from the buffered sprite numbers.

No Scheduler, Z80, audio, GUI, or frame pacing changes.
