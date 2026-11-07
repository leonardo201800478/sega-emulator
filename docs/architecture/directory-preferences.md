# Directory preferences and picker history

## Scope

The native egui application exposes six independent user-selected directories:

- Configuration
- ROMs
- BIOS
- Logs
- SRAM / saves
- Save states

No category is forced to share a parent directory. Selecting one path must not modify any other category. Changing a configured path does not move or delete existing files.

## Defaults

The initial layout is rooted at `Documents/Sega Emulator`:

```text
Sega Emulator/
  config/
  roms/
  bios/
  logs/
  saves/
  states/
```

The application checks the OneDrive and OneDriveCommercial environment paths for a redirected Documents folder before falling back to `%USERPROFILE%/Documents`. Each path can be changed independently through the general settings screen.

## Picker history

Each category stores its own last-visited directory. When a native picker opens, it uses:

1. Its last visited directory, if it still exists.
2. Its configured storage directory, if it exists.
3. Documents, then the current working directory as a final fallback.

The ROM file picker updates only the ROM history; it does not silently change the configured ROM storage path. Selecting a storage directory updates that category's configured path and its picker history. Configured paths and history are retained even while a removable drive or network location is unavailable; the picker falls back to the next valid location without erasing the stored preference.

## Persistence and bootstrap

A small bootstrap pointer remains under `%APPDATA%/Sega Emulator` so the application can find the selected configuration directory on startup. The directory settings file is stored inside the selected configuration directory. This allows the configuration directory itself to be changed without losing the bootstrap location.

Settings are written to a temporary file and renamed into place. The bootstrap pointer is updated only after the new settings file has been written. No ROM, BIOS, SRAM or save-state data is copied as a side effect of changing paths.

## Boundaries and follow-up

The settings service only owns path preferences and picker history. It does not yet redirect the ROM loader, BIOS resolver, logging backend, SRAM persistence or save-state serializer. Those consumers must be integrated separately before the configured paths can be considered operational for their respective data.
