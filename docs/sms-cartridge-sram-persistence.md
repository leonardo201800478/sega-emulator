# SMS cartridge SRAM persistence (Slint Desktop)

**Status:** implementation and automated validation are in PR #802. Physical acceptance with Phantasy Star is still pending; this document does not claim that the in-game save/restore cycle has been physically demonstrated.

## Scope and ownership

The Slint desktop owns cartridge identification, profile selection, save-file lifecycle and host filesystem access. The temporal runtime receives only the validated SRAM capacity and an optional restored byte image.

The following remain unchanged:

- `MachineScheduler` and its temporal authority.
- Z80 execution and bus-cycle semantics.
- VDP implementation and timing.
- `SmsTemporalWorker` ownership of the running machine.

No filesystem operation runs in the CPU, mapper, bus, Scheduler or worker execution path. The worker exposes a coarse-grained command that returns an owned SRAM snapshot at a worker command boundary. The desktop performs persistence on a dedicated host I/O thread.

## Cartridge identification and hardware profile

The desktop uses `CartridgeIdentity` and `CartridgeCatalog::sms_default()`.

- Runtime profile matching is fingerprint-based: SHA-1 plus the No-Intro ROM size and CRC32 evidence.
- The SHA-256 of the loaded ROM bytes is the save-file identity.
- The ROM filename is never used as a hardware-profile fallback.
- A profile must be runtime-usable before its SRAM capacity is passed to the worker.
- An unknown or non-runtime-usable ROM does not receive an inferred SRAM mapping.

The catalog contains four verified Phantasy Star fingerprints with **8 KiB battery-backed SRAM**. The other historical Phantasy Star entries that are marked evidence-only are not automatically mapped.

For the Sega mapper, the memory implementation follows the project research in [SMS special mappers](hardware/sms-special-mappers-research.md):

- `$FFFC` bit 3 enables SRAM in `$8000-$BFFF`.
- `$FFFC` bit 2 selects the SRAM bank for that window.
- `$FFFC` bit 4 enables SRAM in `$C000-$FFFF` when the console's internal RAM window is disabled through the memory-control register.
- An 8 KiB SRAM image is mirrored across the 16 KiB slot-2 window.

The capacity is supplied by the verified profile, not inferred from a save file. The worker accepts only supported physical capacities (8, 16 or 32 KiB) and rejects an initial image whose length does not match the selected capacity.

## SRM identity and format

The save directory on Windows is:

```text
%APPDATA%\Sega Emulator\saves
```

Each file is named from the lowercase SHA-256 of the loaded ROM bytes:

```text
<rom-sha256>.srm
```

The existing `SaveRamStore` format is reused; no second save format is introduced. Its version-1 header is 116 bytes and records the format magic/version, RAM kind, ROM SHA-1 and SHA-256, ROM CRC32 and size, SRAM capacity, and payload CRC32 and SHA-256. The payload follows the header.

For the verified 8 KiB Phantasy Star profile, the expected file size is **8,308 bytes** (116-byte header + 8,192-byte SRAM payload).

Loading validates the profile, ROM identity, capacity, file length and payload checksums. A corrupt or mismatched file returns an error; it is not silently replaced with a zero-filled save. A missing file is a normal first-run condition.

Saving writes a temporary file in the destination directory, flushes the file, then renames it into place. A failed write cleans up the temporary file and does not replace the destination.

## Lifecycle

| Event | SRAM behavior |
|---|---|
| Initial ROM load | Identify profile, load and validate the matching SRM, then initialize the worker before execution |
| Reset | Rebuild the machine while copying SRAM into the replacement machine; no disk write is required |
| Stop / power-off | Stop the worker, export its SRAM snapshot and persist it |
| ROM replacement | Pause the previous worker, persist its SRAM, detach its video mailbox, then attach the new worker |
| Close ROM | Pause, snapshot and persist before dropping the worker |
| Normal application exit | Pause and persist before hiding the window; final runtime cleanup performs a final persistence attempt |

The desktop does not run periodic autosave at this stage. Saves occur on the lifecycle events above. An abrupt process termination or power loss before one of those events can therefore lose SRAM changes made since the last successful write.

## Automated validation

The implementation includes tests for:

- DAT parsing, ROM identity and runtime profile selection.
- Phantasy Star profile capacity and SRAM mapping.
- SRAM reads/writes, slot-2 banking and high-window mapping.
- Worker snapshot export and SRAM retention across reset and power-off.
- SRM round-trip, atomic replacement, identity mismatch, missing file, truncated/corrupt payload, wrong capacity, volatile/blocked profiles and filesystem failure.

The Windows CI job also runs the catalog, save-store, SMS core and SMS runtime tests for `x86_64-pc-windows-msvc`. This specifically exercises replacement of an existing SRM on Windows.

## Physical Windows acceptance procedure

Automated tests and a successful build do not prove that Phantasy Star persists its in-game save. The final acceptance test must be performed with the actual ROM and desktop executable:

1. Open the exact Phantasy Star ROM.
2. Create or update an in-game save.
3. Stop the machine or close the ROM normally.
4. Confirm that the corresponding SRM exists and has been modified.
5. Exit the emulator normally.
6. Start a second emulator process and load the same ROM.
7. Confirm that the previous in-game save is restored.
8. Repeat a save to the same SRM to exercise replacement of an existing file.
9. Re-run `artifacts/pacing-evidence/SMSmemtest.sms` and record the Cart SRAM Bank 0/1 results separately from the Phantasy Star test.

### PowerShell verification

For an uncompressed `.sms` ROM, the following derives the expected filename from the exact ROM bytes:

```powershell
$romPath = 'C:\ROMs\Phantasy Star.sms'
$saveDir = Join-Path $env:APPDATA 'Sega Emulator\saves'
$romSha256 = (Get-FileHash -LiteralPath $romPath -Algorithm SHA256).Hash.ToLowerInvariant()
$srmPath = Join-Path $saveDir "$romSha256.srm"

[pscustomobject]@{
    RomPath = $romPath
    RomSHA256 = $romSha256
    SrmPath = $srmPath
    SrmExists = Test-Path -LiteralPath $srmPath
}
```

After saving in the game and stopping/closing the ROM:

```powershell
Get-Item -LiteralPath $srmPath |
    Select-Object FullName, Length, LastWriteTimeUtc

Get-FileHash -LiteralPath $srmPath -Algorithm SHA256
```

For Phantasy Star, verify the file is 8,308 bytes. Save again in-game, stop normally, and run the hash command again; the timestamp should advance and the file should remain valid. Then restart the emulator and verify the in-game save, not merely the existence of the SRM.

If the ROM is loaded from an archive or transformed by the ROM loader, locate the corresponding file with:

```powershell
Get-ChildItem -LiteralPath $saveDir -Filter '*.srm' |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object FullName, Length, LastWriteTimeUtc
```

The desktop logs `SRAM_PROFILE` with the matched fingerprints, capacity and whether an SRM was restored, and `SRAM_SAVE` after a successful write. An `SRAM_PROFILE status=unknown` entry means the ROM did not match a verified runtime profile and no SRAM save is expected.
