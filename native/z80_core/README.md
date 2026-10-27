# Experimental Rust Z80 execution core

This crate exposes the native SG-1000 machine to the Qt frontend through PyO3. The Qt application always selects Rust; the Python reference implementation is retained only for differential tests during migration.

## Build on Windows PowerShell

Activate the repository's Python 3.14 virtual environment, then run from the repository root:

```powershell
python -m pip install maturin
Push-Location native/z80_core
maturin develop --features python-extension
Pop-Location
```

The Qt frontend uses the Rust machine unconditionally. There is no user-facing Python-backend selector.

## Coverage and limitations

- The implementation incorporates the broader Rust port and differential regression tests maintained in PR #89.
- Coverage includes primary, CB, indexed DD/FD, indexed CB and ED instruction families; block transfer, compare and I/O; interrupt handling; Q/P latches; WZ/MEMPTR updates; and timestamped I/O where the bus supports it.
- Assigned instructions that are not implemented raise `NotImplementedError`; execution never silently falls back to Python.
- The production Qt runtime is Rust-only. The Python reference remains a test oracle, not a selectable production backend. ROM compatibility must still be validated individually.
- Instruction T-states are not a claim of bus-cycle-accurate emulation. Undocumented behavior and interrupt edge cases still require independent verification.
- Bus and I/O operations cross the Python/Rust boundary. No performance gain is claimed without comparable benchmarks.

## Validation

The Windows workflow `.github/workflows/qt-rust-z80-core.yml` runs Rust formatting, Clippy, Rust unit tests, Maturin extension build, Ruff, targeted Qt/backend and Python/Rust parity tests, and the full pytest suite. Check the current run result before treating a revision as validated.

Targeted differential tests compare registers, flags, memory, I/O, cycle counts and auxiliary state with the Python reference. A passing differential suite is evidence for the tested cases, not proof of full Z80 compatibility or cycle accuracy. Manual validation with a legally obtained ROM and comparison against a pinned independent emulator remains recommended.
