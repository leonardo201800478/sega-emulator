# Rust Z80 ALU library and vector runner

This crate provides a shared, hardware-neutral subset of Z80 8-bit arithmetic and logic operations plus the `z80-alu-vectors` stdin/stdout runner used for differential tests.

It is a member of the root Rust workspace (`sega-z80-alu`). It is directly used by:

- `native/sg1000_core`, as a dependency of the standalone SG-1000 Rust core.
- `native/z80_core`, as a dependency of the optional PyO3 Z80 execution extension selected by the Qt frontend.

It is therefore **not an isolated experiment and is linked to existing Rust execution paths**. This does not make it a complete Z80 CPU implementation. Instruction decoding, full execution, memory effects, interrupts, cycle accounting and bus timing are outside this crate's scope. The Rust extension remains experimental and the main workspace's standalone machine crates remain incremental.

## Implemented subset

- 8-bit `ADD A,r`, `SUB r`, `ADC A,r` and `SBC A,r`, including carry/borrow input.
- `AND`, `OR`, `XOR` and `CP` 8-bit operations.
- Result and documented/undocumented flags represented by the implemented functions.
- A stdin/stdout vector runner used by exhaustive Python differential tests.

No performance gain is claimed without comparable benchmarks.

## Validate on Windows PowerShell

From the repository root:

```powershell
cargo fmt --manifest-path native/z80_alu/Cargo.toml --all -- --check
cargo clippy -p sega-z80-alu --all-targets --locked -- -D warnings
cargo test -p sega-z80-alu --locked
.\.venv\Scripts\python.exe -m pytest tests/integration/test_z80_alu_rust_parity.py -q
```

Or run the full root workspace checks:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The differential test checks all 65,536 operand pairs for each of ten operation variants (655,360 vectors total), comparing result/register-preservation and flags with the Python implementation. This is a correctness checkpoint for the implemented ALU subset, not proof of full Z80 compatibility or cycle accuracy.

See [Native Rust crate architecture](../../docs/architecture/native-rust-crates.md) for workspace membership, dependency roles and CI boundaries.
