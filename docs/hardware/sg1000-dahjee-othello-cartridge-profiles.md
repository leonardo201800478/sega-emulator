# SG-1000 cartridge RAM profiles: DahJee/Jumbo and Othello

## Scope and evidence

The MAME SG-1000 software list identifies seven DahJee/Jumbo conversions with slot `dahjee_typea` and a RAM data area of 9,216 bytes (8 KiB adapter plus 1 KiB base RAM). The adapter documentation describes 8 KiB of expanded system RAM mapped at C000h–FFFFh. The profile therefore selects the existing 8 KiB system-RAM map; it does not add a separate 9 KiB window.

| ROM | MAME CRC32 | Expected profile |
|---|---:|---|
| King's Valley (Taiwan) | 223397a1 | DahJee Type A |
| Knightmare (Taiwan) | 281d2888 | DahJee Type A |
| The Legend of Kage (Taiwan) | 2e7166d5 | DahJee Type A |
| Pippols (Taiwan) | df7cbfa5 | DahJee Type A |
| Star Soldier (Taiwan) | e0816bb7 | DahJee Type A |
| Tank Battalion (Taiwan) | 5cbd1163 | DahJee Type A |
| TwinBee (Taiwan) | c550b4f0 | DahJee Type A |
| Othello (Japan) | af4f14bc | Othello candidate, 2 KiB RAM |

Source catalog: https://github.com/mamedev/mame/blob/master/hash/sg1000.xml

## Implemented in this branch

- CRC32 profile selection for the seven DahJee/Jumbo catalog entries.
- DahJee Type A selects the 8 KiB system RAM map at C000h–FFFFh.
- Othello catalog CRCs select a candidate 2 KiB cartridge RAM window at 8000h–BFFFh; the 2 KiB RAM is mirrored within that window.
- Native Rust memory validation and regression test for 2 KiB RAM mapping.
- Native Rust machine API for enabling/disabling the VDP sprite-limit diagnostic option, with a unit test.

## Validation limits

- These tests validate profile selection and the modeled memory map, not successful game execution.
- Othello slot behavior remains a hypothesis until checked against MAME's slot implementation and actual ROM execution.
- Actual ROM boot/playability: NOT RUN.
- PyO3/Maturin + Qt integration: BLOCKED until the extension binding source/build target is available and the frontend toggle is tested end to end.

No ROM dumps are stored in this repository.
