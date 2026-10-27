# SG-1000 Rust I/O boundary — Step 1

## Goal

Move the currently observable host-input and port-write boundary into plain Rust types without relocating Python production modules or claiming the SG-1000 runtime is complete.

## Included in this slice

- `Sg1000Io` owns the two controller bytes, initialized to `0xFF`.
- Controller reads preserve the current Python aliases: player 1 at `0x00`/`0xDC`, player 2 at `0x01`/`0xDD`.
- Writes in the current PSG range `0x40..=0x7F` are captured as ordered `PsgWriteEvent { tstate, value }` records.
- The machine exposes controller input, port reads/writes, and event draining without Python or PyO3 dependencies.
- Unit tests cover defaults, aliases, invalid player values, PSG range boundaries, equal-timestamp ordering, and draining.

## Explicit limitations

- This slice does **not** connect Z80 `IN`/`OUT` instructions to the new interface.
- The native TMS9918A VDP is not yet integrated; VDP port reads are therefore not implemented by this module.
- Captured PSG events are not yet consumed by a native audio synchronizer.
- The timestamp passed by the caller is a nominal machine T-state. This is not cycle-exact I/O bus timing.
- Unknown port reads returning `0xFF` preserve the current placeholder behavior; they are not asserted as a universal hardware fact.
- No Python production module is moved to `legacy/` in this step.

## Acceptance evidence

The branch CI must validate formatting, Clippy, the full locked Rust workspace tests, and relevant Python regressions. A green result validates this slice only; it does not satisfy the standalone SG-1000 gate, which still requires CPU I/O opcodes, VDP, PSG scheduling, and integrated machine scheduling with independent evidence.
