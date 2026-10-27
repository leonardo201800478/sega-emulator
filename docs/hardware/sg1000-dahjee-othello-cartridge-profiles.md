# SG-1000 cartridge RAM profiles: DahJee/Jumbo and Othello

## Scope and evidence

The MAME SG-1000 software list (hash/sg1000.xml) identifies the seven attached DahJee/Jumbo conversions below with slot dahjee_typea and a RAM data area of 9,216 bytes. The MAME data area reports 9,216 bytes (8 KiB expansion plus 1 KiB base RAM). The DahJee/Jumbo adapter documentation says the adapter maps 8 KiB over C000h–FFFFh, with software avoiding the overlap with the original RAM. The emulator profile therefore selects the existing 8 KiB system-RAM map rather than inventing a separate 9 KiB cartridge window.

| ROM | MAME CRC32 | MAME SHA-1 | Expected profile |
|---|---:|---|---|
| King's Valley (Taiwan) | 223397a1 | 922d23895c12707b2fc382ea5cb769c5cb9b5755 | DahJee Type A |
| Knightmare (Taiwan) | 281d2888 | babdb6b109b20e81c28cd892c2042723e9cbff49 | DahJee Type A |
| The Legend of Kage (Taiwan) | 2e7166d5 | 902dbb122bf0efc76604a876c3fae51c074bdc6a | DahJee Type A |
| Pippols (Taiwan) | df7cbfa5 | 4a95b98e127fb0c6428d6debcfc92152069ac531 | DahJee Type A |
| Star Soldier (Taiwan) | e0816bb7 | c1984257809ec5bce80a65d22b9b32561e04922c | DahJee Type A |
| Tank Battalion (Taiwan) | 5cbd1163 | 0ca3b740b28266a2a606127ed8967c684b67cb44 | DahJee Type A |
| TwinBee (Taiwan) | c550b4f0 | a8c30043c145e63f79a3265dde167529f0e08d49 | DahJee Type A |
| Othello (Japan) | af4f14bc | d0cd594ddb321f707ddba8a044fa3e9b906e720a | Othello slot, 2 KiB RAM |

Source: https://github.com/mamedev/mame/blob/master/hash/sg1000.xml

## Implemented in this branch

- Python cartridge profile selection by the MAME CRC32 values for the seven Type-A ROMs.
- Type-A profile selects the 8 KiB system-RAM map at 0xC000–0xFFFF, consistent with the DahJee/Jumbo RAM adapter documentation.
- Othello profile selected by its catalog CRC32, with 2 KiB RAM mirrored through the 0x8000–0xBFFF slot window. The Taiwan Othello catalog CRC is also recognized.
- Native Rust memory accepts 2 KiB cartridge RAM and includes a mapping regression test.
- The native Rust machine exposes the VDP sprite-limit setter and getter, with a unit test for toggling the setting.

## Integration limitation requiring validation

The repository's current main tree contains the standalone native/sg1000_core Rust crate, but does not contain the source for the sega_z80_core.NativeSG1000Machine PyO3 extension referenced by src/sega_emulator/machine/sg1000.py. The observed runtime error occurs at this Python/native integration boundary. Adding a method to the standalone Rust machine does not by itself prove the installed extension exposes that method. Do not mark the frontend sprite-limit option fixed until the actual extension binding source is restored/located, updated to forward the setting to Tms9918A, rebuilt with Maturin, and tested through the Qt frontend.

## Validation status

- Profile mapping tests added; not yet executed in this remote editing session.
- Rust memory and machine unit tests added; not yet executed in this remote editing session.
- Actual ROM boot/playability tests: NOT RUN.
- PyO3/Maturin extension integration for sprite-limit toggle: BLOCKED pending the extension source and a buildable integration target.

No ROM dumps are stored in this repository.