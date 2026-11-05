# SMS Sega mapper — cartridge SRAM implementation

**Date:** 2026-11-05  
**PR:** #543 (stacked on the CPU/VDP I/O-boundary correction in PR #542)  
**Runtime scope:** Sega Master System mapper only; SG-1000 unchanged.

## Hardware contract

The SMS Power mapper reference documents:

- FFFC bit 3 maps cartridge RAM into $8000-$BFFF, overriding slot-2 ROM.
- FFFC bit 4 maps cartridge RAM into the $C000-$FFFF system-RAM window; software should disable internal RAM through port $3E to avoid bus contention.
- FFFC bit 2 selects one of two 16 KiB SRAM banks when present.
- Most known cartridge RAM implementations are 8 KiB and mirror across the 16 KiB slot.
- The Sega mapper is used by the overwhelming majority of SMS software.

Primary reference: https://www.smspower.org/Development/Mappers  
Memory map: https://www.smspower.org/Development/MemoryMap

## Implemented slice

- Allocate an 8 KiB cartridge SRAM image in `SmsMemory`.
- Map it into slot 2 only for the Sega mapper when FFFC bit 3 is set.
- Mirror the 8 KiB image across the 16 KiB slot.
- Map the SRAM into $C000-$FFFF when FFFC bit 4 is set and the internal RAM is disabled through port $3E.
- Preserve SRAM across CPU reset and expose Rust export/restore methods.
- Expose the image through PyO3 and the thin Python SMS adapter; the host owns save-file I/O.

## Deliberate limits

- The current slice models 8 KiB SRAM. 16 KiB and 32 KiB banked SRAM, including FFFC bit 2 selection, remain NOT IMPLEMENTED.
- No per-ROM capability database is introduced. SRAM is allocated but is visible only when software enables the relevant mapper bits.
- Codemasters SRAM (the documented Ernie Els Golf case) is not part of this Sega-mapper change.
- Automatic .sav discovery, writeback policy, atomic file replacement and frontend lifecycle integration remain separate host tasks.
- This is functional mapper support, not a claim that every cartridge RAM board revision is represented.

## Regression coverage

- SRAM enable overrides slot-2 ROM and 8 KiB mirrors through the 16 KiB slot.
- Disabling SRAM restores the selected ROM bank.
- The high RAM window maps cartridge SRAM only when the internal RAM is disabled.
- SRAM image export/restore validates exact 8192-byte length.
- PyO3 and Python adapter round-trip tests exercise the persistence boundary.

## Validation status

- Local Rust/Python toolchains: NOT RUN in this session.
- CI: pending on the final branch HEAD.
- Real-game save/load validation: NOT RUN.
- Hardware equivalence: NOT CLAIMED.
