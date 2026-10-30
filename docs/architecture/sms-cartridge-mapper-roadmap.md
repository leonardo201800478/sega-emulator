# SMS cartridge architecture and game-compatibility roadmap

**Status:** RESEARCH_BASELINE / IMPLEMENTATION_PENDING  
**Baseline audited:** `main` at `0a6e77c2f01b87692accd033525197eb8812707b` (2026-10-30).  
**Scope:** Sega Master System / Mark III cartridge mapping and cartridge-dependent hardware. This is a staged engineering plan, not a claim that the listed features are implemented.

Related research inventory: [Special SMS mappers and known game families](../hardware/sms-special-mappers-research.md).

## 1. Why this work is required

The SMS runtime currently has a single standard Sega mapper path. The cartridge image is immutable, and the mapper registers and console work RAM are handled inside `SmsMemory`. This is a sound initial baseline, but it cannot correctly boot all cartridge families.

The immediate known compatibility targets are:

- **128 Hap (Korea) (Unl):** existing investigation PR [#262](https://github.com/leonardo201800478/sega-emulator/pull/262) identifies the Korean multicart mapper as a leading hypothesis, not a proven root cause. The local dump's size/hash and first divergent fetch must be captured before selecting a variant.
- **Codemasters titles:** Cosmic Spacehead, CJ Elephant Fugitive, Fantastic Dizzy and Micro Machines are listed as SMS releases using a mapper incompatible with the Sega register map.
- **Korean cartridge families:** simple Korean, MSX/Nemesis, Janggun and multicart variants use different register decoders and bank widths.
- **Battery-backed games:** Golden Axe Warrior, Golfamania, Miracle Warriors, Monopoly, Penguin Land, Phantasy Star, Ultima IV and Ys require cartridge RAM persistence. The exact SRAM size and banking must be validated per cartridge profile.
- **Independent game investigations:** [Ace of Aces #263](https://github.com/leonardo201800478/sega-emulator/pull/263) and [Andre Agassi Tennis #266](https://github.com/leonardo201800478/sega-emulator/pull/266) remain separate boot/region/VDP/audio investigations. Do not label them mapper bugs without a trace proving that.

A game compatibility database must not become a collection of ad-hoc game-name conditionals. Resolve cartridge hardware from deterministic content identity and explicit user override, then keep console model, territory, video standard, FM availability and peripherals as separate capabilities.

## 2. Audited implementation baseline

At the baseline SHA above:

| Area | Current code | Gap |
|---|---|---|
| Cartridge image | `native/sms_core/src/cartridge/image.rs` stores immutable bytes and rejects empty images | No mapper identity, cartridge metadata, SRAM or save-memory policy |
| Cartridge module | `native/sms_core/src/cartridge/mod.rs` exports only `CartridgeImage` | No mapper enum/factory or database resolver |
| Memory | `native/sms_core/src/memory.rs` implements three 16 KiB ROM windows, fixed first 1 KiB, mapper registers at `$FFFC-$FFFF`, and 8 KiB mirrored work RAM | No ROM-only selection, cartridge SRAM mapping, or alternate mapper decode |
| ROM banking | Power-of-two ROM sizes/bank counts are mirrored; out-of-range banks in non-power-of-two images return `$FF` | Must be checked against documented board wiring and reference vectors for every supported mapper |
| I/O/peripherals | `SmsMachineBus` handles the shared CPU bus; `SmsMachine` has a SegaScope latch | SegaScope presentation, Light Phaser raster-qualified detection, Paddle and Sports Pad remain incomplete |
| Audio | SN76489 PSG is integrated | YM2413/OPLL FM hardware and its control/detection ports are not integrated |

Important consequence: the current runtime applies the Sega mapper path even to small ROMs. SMS Power says typical 32 KiB releases do not require a mapper. Add an explicit ROM-only path and tests rather than relying on incidental bank mirroring to make small images appear to work.

This baseline is specific to the audited SHA. Re-check the source files after the BIOS feature branch is integrated; BIOS overlay and the memory-control latch must compose with cartridge selection without creating a second memory owner.

## 3. Target architecture

Keep the cartridge model in the Rust runtime. Python/Qt may identify files, store preferences and expose a mapper override, but must not implement bank switching or become a second hardware runtime.

Proposed responsibilities:

- `CartridgeImage`: immutable original bytes, original size and computed identity. Never truncate or rewrite the dump in place.
- `CartridgeMetadata`: system family, mapper kind, cartridge RAM size/type, battery flag, optional peripherals and confidence/provenance.
- `MapperKind`: stable serializable identifier for ROM-only, Sega, Codemasters, Korean, MSX, Nemesis, Janggun and later variants.
- `MapperState`: an enum containing only each mapper's registers and bank-selection state. Use static dispatch; no heap allocation or trait-object dispatch per CPU access.
- `Cartridge`: owns the image, mapper state and cartridge RAM. Exposes read/write operations to the SMS memory/bus layer.
- `SmsMemory`: remains the owner of BIOS overlay, memory-control latch and console work RAM. It routes cartridge accesses and preserves the current RAM mirror behavior.
- `SaveRamStore`: frontend-owned persistence boundary. The core exposes typed save-memory bytes/dirty state; filesystem access stays outside the instruction path.

Do not blindly move the whole memory map into a new cartridge object. In particular, writes to `$FFFC-$FFFF` must reach the Sega mapper while retaining the documented work-RAM mirror side effect. Writes to the mirrored RAM addresses `$DFFC-$DFFF` must not accidentally program the mapper.

## 4. Deterministic mapper selection

Selection order:

1. **Explicit user override** for diagnosis and unsupported/modified dumps.
2. **Trusted exact database entry** keyed by canonical image size plus CRC-32, with SHA-256 retained as the stronger identity and for audit logs.
3. **Conservative size rule:** typical ROM-only images up to 48 KiB use ROM-only mapping; larger images default to the standard Sega mapper, matching the documented BIOS assumption.
4. Unknown or ambiguous images produce a visible diagnostic and allow a manual override. Never guess a special mapper from a title string.

Database rules:

- Keep mapper metadata separate from region/video/FM/peripheral metadata.
- Store source URL, source revision/date, exact size, CRC-32, optional SHA-256, mapper identifier, and confidence for every entry.
- A CRC match must also match image size. Do not treat CRC-32 as a cryptographic identity.
- Preserve the raw dump. Any copier/header removal must be based on a recognized, validated format; do not strip an arbitrary 512 bytes merely because a candidate file has that size.
- Include only confirmed SMS entries in the SMS database. Mark Game Gear-only, prototypes, uncertain releases and hacks separately; do not silently classify them as commercial SMS software.
- Keep the database deterministic and testable offline. No network lookup while loading a game.

## 5. Mapper implementation order

### Phase 0 — complete and test the standard Sega mapper

Acceptance requirements:

- Model the four write registers at `$FFFC-$FFFF`, including the three 16 KiB ROM slots and the fixed `$0000-$03FF` vector area.
- Preserve the work-RAM mirror side effect for writes to the register addresses; a write to another RAM mirror address must not alter mapper state.
- Implement cartridge SRAM in the slot(s) selected by `$FFFC`: bit 3 controls mapping at `$8000-$BFFF`, bit 4 controls mapping in the `$C000-$FFFF` RAM window, and bit 2 selects the SRAM bank. Verify interaction with the SMS memory-control port `$3E` to avoid bus contention.
- Support 8 KiB SRAM mirroring and 16/32 KiB SRAM banking only when the cartridge profile says the physical memory exists.
- Persist SRAM only for battery-backed profiles; non-battery RAM starts deterministically and is not written as a persistent save.
- Keep the ROM write-enable bit and bank-shift bits represented in the state model. Implement their effects only with a documented board profile and dedicated tests; do not infer that every board revision wires them.
- Distinguish documented reset behavior for 315-5235 from revisions whose mapper registers are undefined after reset. Until a revision is identified, make the deterministic emulator policy explicit and test it.
- Preserve power-of-two mirroring; define and test non-power-of-two behavior from hardware evidence rather than a generic modulo operation.

### Phase 1 — Codemasters

Implement the three bank registers at `$0000`, `$4000` and `$8000`, including their documented address mirrors, initial banks `0,1,0`, and the absence of Sega-style RAM readback. The first 1 KiB is not fixed under this mapper.

Support the optional 8 KiB RAM behavior only for a confirmed cartridge profile: the high bit written to `$4000` enables RAM at `$A000-$BFFF`, while the lower half of slot 2 remains ROM. Do not enable RAM for every Codemasters title.

Initial SMS regression set: Cosmic Spacehead, CJ Elephant Fugitive, Fantastic Dizzy and Micro Machines. Keep Game Gear and prototype entries out of the SMS acceptance set.

### Phase 2 — simple Korean families

Implement independently, with synthetic bank-pattern tests:

- Korean `$A000`: fixed first two 16 KiB banks and switchable slot 2.
- MSX: fixed first 16 KiB and four switchable 8 KiB windows controlled by `$0000-$0003`.
- Nemesis: MSX-style banking with the documented fixed final 8 KiB page at `$0000-$1FFF`.
- Janggun: four 8 KiB windows, 16 KiB overrides through `$FFFE/$FFFF`, bit-reversal behavior and the documented RAM mirror side effect.

### Phase 3 — Korean multicarts and uncommon mappers

Add only when a verified dump or a reproducible regression target exists:

- Korean MSX SMS 8000
- Korean 0000 XOR FF
- Korean SMS 32KB 2000
- Korean MSX 32KB 2000
- Korean 2000 XOR 1F
- Korean MSX 8KB 0300
- Korean FFFF HiCom
- Korean FFFE
- Korean BFFC
- Korean FFF3 FFFC
- Korean MD FFF5 / FFF0
- Jumbo / DahJee
- 4 PAK All Action
- Iratahack flash emulation

The 128 Hap investigation should be the first candidate for this phase if its local SHA-256, size, bank layout and mapper writes confirm the Korean 2000 XOR 1F variant. The PR #262 hypothesis is not proof.

## 6. Game-specific hardware that is not a mapper

Do not overload `MapperKind` with console features or peripherals.

| Feature | Hardware boundary | Required work / acceptance |
|---|---|---|
| YM2413 / OPLL FM | Console model or Mark III FM Unit capability; ports `$F0-$F2` | Implement chip state, register writes, audio timing/mix, presence detection and profile-specific PSG/FM control. Do not infer FM solely from ROM region or mapper. |
| Paddle / Sports Pad | Input peripheral | Add explicit device models and port protocols; sample host input at the frontend boundary, not per CPU instruction. |
| Light Phaser | Input + VDP raster/light sensing | Existing aim/trigger representation is insufficient: VDP must produce a raster-qualified light pulse/hit result. |
| SegaScope / 3-D glasses | Console/VDP presentation + shutter output | Existing left/right latch is only a partial implementation. Add correct two-eye rendering, frame alternation and synchronized host shutter state before claiming support. |
| Battery-backed SRAM | Cartridge hardware + save service | Implement SRAM mapping, dirty tracking, stable save identity and persistence; never include ROM bytes in saves/logs. |
| EEPROM 93C46 | Listed in the research inventory for Game Gear titles | Exclude from SMS scope unless a confirmed SMS cartridge is found. Do not add a generic EEPROM mapper speculatively. |

YM2413 compatibility tests should include games with known FM behavior and negative cases. SMS Power lists titles such as Wanted, American Pro Football and Walter Payton Football among cases that may fail with FM enabled; treat those as hardware-profile regressions to reproduce, not as a reason to globally disable FM. Aztec Adventure is catalogued with the standard Sega mapper and YM2413 capability: a scroll/parallax defect must be investigated in the VDP/video path, not “fixed” by changing its mapper.

## 7. Test and validation contract

Use synthetic images whose every 16 KiB or 8 KiB bank is filled with a unique byte pattern. Tests must assert the exact mapped byte after each register write.

Required test groups:

1. **Image identity:** exact size, CRC-32 and SHA-256; known/unknown entries; CRC collision with different size; altered dumps; explicit override; deterministic resolution.
2. **Sega mapper:** reset profile, all slots, fixed first 1 KiB, write-only registers plus RAM mirror, SRAM enable/disable, SRAM bank select, 8 KiB mirroring, work-RAM mapping, memory-control interaction and out-of-range banks.
3. **Codemasters:** register mirrors, initial bank values, slot-0 vector behavior, no mapper readback, optional RAM upper half and non-RAM title behavior.
4. **Korean/MSX/Nemesis/Janggun:** each register decode, bank width, fixed windows, address mirrors, data reversal and RAM side effects.
5. **Persistence:** battery-backed save/load round-trip, volatile RAM not persisted, corrupt/truncated save rejection, deterministic save identity and dirty tracking.
6. **Integration:** BIOS overlay and port `$3E` compose with every supported mapper; CPU writes reach the correct decoder; no duplicate Python hardware implementation.
7. **Regression:** preserve SG-1000 behavior; test all supported SMS ROM-only and standard Sega cases; keep game-specific investigations isolated until their first divergence is known.

Use legally obtained homebrew/synthetic test images and local user-owned ROMs. Never commit commercial ROMs, BIOS binaries, copyrighted dumps, or logs containing ROM contents. Hashes and sizes are acceptable diagnostics when they do not expose protected content.

Validation states must be recorded separately: `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING` and `TEMPORAL_PASS`. Mapper correctness does not imply cycle-exact timing; `TEMPORAL_PASS` requires a separately validated bus/event model.

## 8. Sources and licensing boundary

Primary hardware references:

- [SMS Power — Mappers](https://www.smspower.org/Development/Mappers)
- [SMS Power — SMSReader overview / cartridge banking](https://www.smspower.org/Development/SMSReader-Overview)
- [SMS Power — Codemasters cartridge barcodes and SMS releases](https://www.smspower.org/Development/Barcodes-Codemasters)
- [SMS Power — YM2413](https://www.smspower.org/Development/YM2413)
- [SMS Power — Audio Control Port](https://www.smspower.org/Development/AudioControlPort)
- [Charles MacDonald — SMS/GG hardware notes](https://github.com/drhelius/Gearsystem/blob/master/skills/gearsystem-debugging/references/smstech.md)

Independent implementation/reference database:

- [Gearsystem game database](https://github.com/drhelius/Gearsystem/blob/master/src/game_db.h)
- [Gearsystem Sega mapper reference](https://github.com/drhelius/Gearsystem/blob/master/src/SegaMemoryRule.cpp)
- [Gearsystem Codemasters mapper reference](https://github.com/drhelius/Gearsystem/blob/master/src/CodemastersMemoryRule.cpp)
- [Gearsystem mapper implementations](https://github.com/drhelius/Gearsystem/tree/master/src)
- [Gearsystem license](https://github.com/drhelius/Gearsystem/blob/master/LICENSE) — GPL-3.0-or-later.

Gearsystem is useful for cross-checking game database entries and observed mapper behavior. Its source is GPL-3.0-or-later; do not copy or port implementation code into this MIT-licensed project without a separate license/provenance review. Reimplement behavior independently from hardware documentation and tests.

## 9. Suggested execution sequence

1. Close the SMS BIOS branch integration and re-audit the final main SHA.
2. Implement Phase 0 Sega SRAM semantics and battery-save plumbing with synthetic tests.
3. Implement Phase 1 Codemasters.
4. Add exact database resolution and user mapper override.
5. Implement Phase 2 Korean/MSX/Nemesis/Janggun.
6. Use the verified 128 Hap dump to select the next Phase 3 variant.
7. Add non-mapper capabilities (FM, peripherals and SegaScope output) as separate, independently testable workstreams.

No gameplay claim is accepted until the exact candidate SHA passes the relevant workflows and a reproducible game or synthetic test demonstrates the behavior.
