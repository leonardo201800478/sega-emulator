# Native Z80 migration slice

This crate is the first isolated Rust/PyO3 step for the Sega Emulator.

## Current scope

The extension exposes `Z80Subset` for a deliberately small, testable set of opcodes:
NOP, LD A,n, XOR A, INC A, JR e, HALT, and JP nn. Execution can be batched through
`run_steps`, avoiding a Python/Rust transition for every instruction.

**This is not a complete Z80 and must not be selected as the machine's production CPU.**
The Python implementation remains the reference and the only full execution core.

## Build and test on Windows PowerShell

From the repository root, with Rust stable/MSVC build tools installed:

```powershell
cargo fmt --manifest-path native/z80/Cargo.toml -- --check
cargo test --manifest-path native/z80/Cargo.toml
cargo clippy --manifest-path native/z80/Cargo.toml --all-targets -- -D warnings
py -3.14 -m venv .venv-native
.\.venv-native\Scripts\python.exe -m pip install --upgrade pip maturin pytest
Push-Location native/z80
..\.venv-native\Scripts\maturin.exe develop --release
Pop-Location
.\.venv-native\Scripts\python.exe -m pytest tests/test_native_z80.py -q
```

## Migration rules

- Add instructions only with tests against the Python reference or trusted Z80 vectors.
- Keep registers, flags, wrapping, and nominal T-state behavior explicit.
- Expand the native API at frame/batch granularity; do not call Python per opcode.
- Do not claim cycle-accurate bus behavior: this slice counts nominal instruction T-states only.
- Keep this crate isolated until parity and real-ROM benchmarks justify integration.
