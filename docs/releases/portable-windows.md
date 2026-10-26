# Windows portable distribution

## Goal

Produce a Windows x64, one-folder application distribution and a ZIP archive for the GitHub release. Users should not need to install Python or Rust to launch the packaged application.

## Build pipeline

The workflow at .github/workflows/windows-portable.yml:

1. Uses Windows CI and Python 3.14 x64.
2. Installs the Qt frontend dependencies and builds/installs the optional PyO3 Z80 extension in Release mode.
3. Checks key dependency imports before packaging.
4. Uses PyInstaller in one-folder mode and collects SDL3, pygame, and audio dependencies.
5. Uses `PySide6-Essentials` rather than the full `PySide6` distribution. The current frontend imports QtCore, QtGui, and QtWidgets; it does not use Qt WebEngine. This avoids installing the Addons distribution that contributed the 203 MB `Qt6WebEngineCore.dll` to the previous package. PyInstaller still analyzes the actual Qt imports and includes their runtime dependencies.
6. Includes a portable readme and MIT license.
7. Creates SegaEmulator-Windows-x64-portable.zip and a package-size report (file count, uncompressed size, ZIP size, and the 25 largest files), then runs the packaged CLI entry point with --help as a non-interactive smoke test.
8. Uploads the ZIP as a CI artifact for pull requests and attaches it to an existing GitHub release when a v* tag build succeeds.

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
- The CI artifact includes `package-size-report.txt` so package growth and large bundled dependencies can be audited before tuning collection rules.
- The new package report confirms the WebEngine binary is absent and records the new ZIP/uncompressed sizes.
- A user validates launching the extracted application on a Windows machine outside the build environment before the package is advertised as broadly compatible.

## Known limitations

The non-interactive --help smoke test does not prove that the GUI, Qt Quick rendering backend, controller input, audio output, or ROM execution works. Switching to Qt Essentials is an evidence-based reduction, but it must be verified by the Windows build and import/packaging checks before it is accepted. The Windows CI runner's GUI startup test exited with Windows status 0xC0000005 (access violation), so GUI compatibility is explicitly not considered validated by CI and requires separate diagnosis/manual testing. A clean-machine manual test remains required. The emulator's documented timing and long-session audio/video synchronization limitations also apply to the portable build.
