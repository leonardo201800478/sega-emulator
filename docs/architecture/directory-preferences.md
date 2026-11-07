# Directory preferences and picker history

## Scope

The native egui application exposes thirteen independent user-selected directories:

- Configuration
- ROMs
- BIOS
- Logs
- SRAM / saves
- Save states
- Screenshots / captures
- Vulkan shader packs
- DirectX 11/12 shader packs
- Shared Slang shader assets
- Shader presets
- Shader textures / LUTs
- Compiled shader cache

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
  screenshots/
  shaders/
    vulkan/
    directx/
    shared/
    presets/
    textures/
    cache/
```

The application checks the OneDrive and OneDriveCommercial environment paths for a redirected Documents folder before falling back to `%USERPROFILE%/Documents`. Each path can be changed independently through the general settings screen.

## Shader organization

The shader storage is separated by graphics backend, with additional shared-resource directories:

- `shaders/vulkan`: Vulkan-oriented shader packs and presets.
- `shaders/directx`: Direct3D 11/12-oriented shader packs and presets.
- `shaders/shared`: reusable Slang shader sources and common assets. Libretro documents Slang as compatible with Vulkan and Direct3D 10/11/12; shared Slang assets should be preferred when a single source supports both backends.
- `shaders/presets`: user presets and shader-chain descriptions (for example, `.slangp`).
- `shaders/textures`: LUTs and external texture assets referenced by presets.

The folder split is an organization and selection boundary, not a claim that Vulkan and Direct3D require different shader languages. The compiled shader cache is separate from source packs and presets because compiled artifacts may depend on backend, GPU, driver and shader compiler versions; cache invalidation must be handled by the renderer rather than by deleting user shader sources. Slang is the preferred cross-backend format where supported; GLSL is OpenGL-specific and legacy Cg is deprecated. The current native video runtime exposes D3D12 presentation only; this preference layer does not implement a Vulkan renderer, shader compiler, shader-chain loader, or runtime shader application.

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

The settings service only owns path preferences and picker history. It does not yet redirect the ROM loader, BIOS resolver, logging backend, SRAM persistence, save-state serializer, screenshot capture pipeline, shader discovery, preset loader, shader compiler, compiled-shader cache manager or texture resolver. Those consumers must be integrated separately before the configured paths can be considered operational for their respective data.
