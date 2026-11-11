# E7.7 — ED absolute word transfers at address wrap

**Date:** 2026-11-09
**Baseline:** `main` = `0011eb41cee2352d2aaea71cae568e2c4d3a21e1`
**Scope:** ED 43/53/63/73 (`LD (nn),rr`) and ED 4B/5B/6B/7B (`LD rr,(nn)`), especially `nn = 0xFFFF`.

## 1. Gap identified

The existing directed test `ed_absolute_word_loads_and_stores_cover_all_register_pairs` exercised BC, DE, HL and SP at `nn = 0xC200`. The runtime implementation already uses 16-bit wrapping for the second byte address and WZ:

- store: writes low byte at `nn`, high byte at `nn.wrapping_add(1)`;
- load: reads low byte at `nn`, high byte at `nn.wrapping_add(1)`;
- both set WZ to `nn.wrapping_add(1)`.

The missing evidence was the boundary case where `nn = 0xFFFF`, so the second transfer and WZ must wrap to `0x0000`.

## 2. Change

Added `ed_absolute_word_transfers_wrap_address_and_wz_at_ffff` to the existing `tests` module in `native/z80_cpu/src/cpu.rs`, where the private `TestBus` is defined.

For all four register pairs BC, DE, HL and SP, the test verifies:

- all four store opcodes write little-endian bytes to FFFF and 0000;
- all four load opcodes reconstruct the expected 16-bit value from FFFF and 0000;
- WZ wraps to 0000;
- PC consumes exactly the four instruction bytes;
- the nominal duration is 20 T-states;
- F remains unchanged.

The test does not change runtime behavior.

## 3. Evidence and limits

The ZiLOG UM008011-0816 manual documents the absolute 16-bit register-pair transfer forms and their nominal 20 T-states. The wrap assertion validates the 16-bit address arithmetic already used by the core; it is a boundary regression test, not a claim about an additional undocumented silicon behavior.

This test compares the expected architectural result directly and does not rely on the executor's own helper as an oracle.

## 4. Acceptance

- Focused unit test: pending CI on the exact PR HEAD.
- Rust formatting, Clippy and workspace regression: pending CI on the exact PR HEAD.
- Runtime changes: none.
- Differential hardware/oracle test: NOT RUN.
- E7.7 global: IN_PROGRESS.
- E8: BLOCKED until E7 closes.
