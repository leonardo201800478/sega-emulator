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

The Othello map remains a separate implementation profile; its successful manual run is recorded below, but has not been differentially compared with another emulator.

## Implementation

- Cartridge profile selection uses known CRC32 values.
- DahJee/Jumbo Type A: 8 KiB cartridge/adapter RAM at 2000h–3FFFh; internal RAM remains 1 KiB at C000h–FFFFh.
- Othello: 2 KiB cartridge RAM mirrored across 8000h–BFFFh.
- The Castle (Japan) remains a separate 8 KiB cartridge-RAM profile at 8000h–BFFFh.
- Unknown ROMs retain the standard SG-1000 memory profile.

## Integration and milestone record

- Final Type-A RAM-map fix integrated into `main` by [PR #141](https://github.com/leonardo201800478/sega-emulator/pull/141), merge commit `f51c0fac8dfd850a761e65de48709a2e7002d11d`.
- Investigation issue [#136](https://github.com/leonardo201800478/sega-emulator/issues/136) closed as completed.
- Maintainer-reported manual functional result: PASS 8/8 — King's Valley (Taiwan), Knightmare (Taiwan), The Legend of Kage (Taiwan), Othello (Japan), Pippols (Taiwan), Star Soldier (Taiwan), Tank Battalion (Taiwan), and TwinBee (Taiwan).
- The maintainer reports other previously working SG-1000 games continue to work as before. This is a report from the maintainer's test environment, not a full catalog sweep.

## Validation status

- [x] Profile selection tests cover the known DahJee/Jumbo Type-A CRCs.
- [x] Native Rust memory test covers Type-A RAM writes, boundaries, ROM-window separation, and unchanged 1 KiB internal RAM.
- [x] Othello has a synthetic test for its 2 KiB mirrored window.
- [x] Full local Rust workspace tests: 114 passed, 0 failed.
- [x] Full local Python pytest suite: 192 passed, 0 failed.
- [x] `cargo fmt --all -- --check`: PASS.
- [x] `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- [x] Native Python extension rebuilt locally with Maturin for CPython 3.14 on Windows x64.
- [x] Maintainer manual functional report: all eight issue #136 games ran correctly in the local emulator.
- [ ] Differential comparison against Genesis Plus GX/MAME or Ares: NOT RUN.
- [ ] Python Ruff, compileall, and pip check acceptance commands: NOT RUN in this local validation session.

The manual functional result is based on the maintainer's local testing; the supplied session log records eight ROM-load events and confirms `rust_native_machine` as the effective runtime. It is not an independently reproduced differential test. The merge commit's GitHub Actions runs were not confirmed in the available query. No commercial ROMs are stored in the repository.
