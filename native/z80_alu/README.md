# Rust Z80 ALU prototype

This crate is an isolated experiment; it is not linked to the emulator runtime.
The Python `Z80Cpu` remains the reference implementation.

## Implemented subset

- 8-bit `ADD A,r` arithmetic and documented/undocumented result flags.
- 8-bit `SUB r` arithmetic and documented/undocumented result flags.
- A stdin/stdout vector runner used by exhaustive Python differential tests.

Carry-in operations (`ADC`/`SBC`), instruction decoding, cycle accounting, memory
effects, interrupts, and integration with `Z80Cpu` are deliberately out of scope.
No performance gain is claimed at this stage.

## Validate on Windows PowerShell

```powershell
cargo fmt --manifest-path native/z80_alu/Cargo.toml --all -- --check
cargo clippy --manifest-path native/z80_alu/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path native/z80_alu/Cargo.toml
.\.venv\Scripts\python.exe -m pytest tests/integration/test_z80_alu_rust_parity.py -q
```

The differential test checks all 65,536 operand pairs for `ADD` and all 65,536
pairs for `SUB`, comparing both the result and flags with the Python implementation.
Passing this test is a correctness checkpoint for this subset, not proof of full
Z80 compatibility or cycle accuracy.