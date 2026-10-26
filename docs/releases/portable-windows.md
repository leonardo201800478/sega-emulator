# Windows portable distribution

## Goal

Produce a Windows x64, one-folder application distribution and a ZIP archive for the GitHub release. Users should not need to install Python or Rust to launch the packaged application.

## Build pipeline

The workflow at .github/workflows/windows-portable.yml:

1. Uses Windows CI and Python 3.14 x64.
2. Installs the Qt frontend dependencies and builds/installs the optional PyO3 Z80 extension in Release mode.
3. Checks key dependency imports before packaging.
4. Uses PyInstaller in one-folder mode and collects Qt, SDL3, pygame, and audio dependencies.
5. Includes a portable readme and MIT license.
6. Creates SegaEmulator-Windows-x64-portable.zip, then starts the packaged GUI for a short smoke test.
7. Uploads the ZIP as a CI artifact for pull requests and attaches it to an existing GitHub release when a v* tag build succeeds.

## Using the package

Extract the complete ZIP to a writable folder and run SegaEmulator.exe. Do not move the executable out of its distribution directory. Use Arquivo → Abrir ROM to select a legally obtained SG-1000 ROM.

The archive does not contain commercial ROMs or BIOS files.

## Acceptance criteria

- The Windows job completes successfully on the exact candidate commit/tag.
- The PyO3 extension imports after a Release build.
- The application dependencies import in the packaging environment.
- PyInstaller produces dist/SegaEmulator/SegaEmulator.exe.
- The packaged executable exits successfully for the CLI --help smoke test.
- The ZIP contains the executable, runtime files, portable instructions, and license.
- A user validates launching the extracted application on a Windows machine outside the build environment before the package is advertised as broadly compatible.

## Known limitations

The --help smoke test does not prove that the GUI, Qt Quick rendering backend, controller input, audio output, or ROM execution works on every target PC. A clean-machine manual test remains required. The emulator's documented timing and long-session audio/video synchronization limitations also apply to the portable build.
