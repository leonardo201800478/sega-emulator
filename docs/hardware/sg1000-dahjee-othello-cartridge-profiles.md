# SG-1000 cartridge RAM profiles: DahJee/Jumbo and Othello

## Scope and evidence

This document records distinct RAM maps for SG-1000 cartridge hardware. CRC32 identifies known dumps; ROM size alone is not used to infer a mapper.

### DahJee/Jumbo Type A

The Genesis Plus GX source (core/cart_hw/sms_cart.c) explicitly lists the seven CRCs below as MAPPER_RAM_8K_EXT1 and maps the adapter's 8 KiB external RAM at **2000h–3FFFh**. This is a cartridge/adapter RAM window that overlays the normal ROM read map; it is not the SG-1000's internal RAM at C000h–FFFFh.

Reference implementation (used for independent behavioral study, not copied): https://github.com/ekeeke/Genesis-Plus-GX/blob/master/core/cart_hw/sms_cart.c

| ROM | CRC32 | Profile |
|---|---:|---|
| King's Valley (Taiwan) | 223397a1 | DahJee/Jumbo Type A |
| Knightmare (Taiwan) | 281d2888 | DahJee/Jumbo Type A |
| The Legend of Kage (Taiwan) | 2e7166d5 | DahJee/Jumbo Type A |
| Pippols (Taiwan) | df7cbfa5 | DahJee/Jumbo Type A |
| Star Soldier (Taiwan) | e0816bb7 | DahJee/Jumbo Type A |
| Tank Battalion (Taiwan) | 5cbd1163 | DahJee/Jumbo Type A |
| TwinBee (Taiwan) | c550b4f0 | DahJee/Jumbo Type A |

The MAME software list identifies these titles with slot dahjee_typea and reports a 9,216-byte RAM data area for the listed conversions. The 9,216-byte metadata is not modeled as a single contiguous 9 KiB window: the Type-A mapper behavior above describes an 8 KiB external window, while the baseline 1 KiB internal RAM remains mapped separately.

MAME metadata: https://github.com/mamedev/mame/blob/master/hash/sg1000.xml

### Othello

Othello uses a different 2 KiB RAM mapper. The Genesis Plus GX game list labels the Japanese and Taiwan CRCs as MAPPER_RAM_2K; its implementation maps that RAM mirrored across 8000h–BFFFh.

| ROM | CRC32 | Profile |
|---|---:|---|
| Othello (Japan) | af4f14bc | Othello 2 KiB RAM |
| Othello (Taiwan) | 1d1a0ca3 | Othello 2 KiB RAM |

The 2 KiB window is modeled as a candidate pending differential execution of the ROMs against an independent emulator.

## Implementation

- Cartridge profile selection uses known CRC32 values.
- DahJee/Jumbo Type A: 8 KiB cartridge/adapter RAM at 2000h–3FFFh; internal RAM remains 1 KiB at C000h–FFFFh.
- Othello: 2 KiB cartridge RAM mirrored across 8000h–BFFFh.
- The Castle (Japan) remains a separate 8 KiB cartridge-RAM profile at 8000h–BFFFh.
- Unknown ROMs retain the standard SG-1000 memory profile.

## Validation status

- [x] Profile selection tests cover the known DahJee/Jumbo Type-A CRCs.
- [x] Native Rust memory test covers Type-A RAM writes, boundaries, ROM-window separation, and unchanged 1 KiB internal RAM.
- [x] Othello has a synthetic test for its 2 KiB mirrored window.
- [ ] Full Rust and Python acceptance suites for this corrective change: NOT RUN in this editing session.
- [ ] Actual boot/gameplay of the eight issue #136 ROMs: NOT RUN.
- [ ] Differential comparison against Genesis Plus GX/MAME or Ares: NOT RUN.
- [ ] Othello slot behavior confirmed against the MAME implementation and actual ROM execution: NOT RUN.

Do not declare the games compatible based on profile-selection or synthetic memory tests alone. No commercial ROMs are stored in the repository.
