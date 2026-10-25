# Experimental Rust Z80 execution core

This crate exposes \`RustZ80Cpu\` to Python through PyO3. It is a selectable,
incremental implementation, not yet a replacement for the Python reference.

## Build on Windows PowerShell

Activate the repository's Python 3.14 virtual environment, then run:

\`\`\`powershell
python -m pip install maturin
Set-Location native/z80_core
maturin develop --features python-extension
Set-Location ../..
\`\`\`

Choose the core from the Qt menu **Emulação → Núcleo da CPU**, or pass
\`--cpu-core python\` / \`--cpu-core rust\` to the command line.

## Current limitations

- The Python core remains the default and compatibility reference.
- Rust currently supports a limited subset of the base instruction set, common
  8-bit ALU operations, branches/calls/returns, stack operations, immediate I/O,
  basic IM selection, and initial INT/NMI handling.
- Unsupported instructions raise \`NotImplementedError\`. There is no silent
  fallback to Python, because that would make performance measurements invalid.
- Indexed DD/FD, CB operations, many ED instructions, full Q/P-latch behavior,
  and exhaustive interrupt edge cases remain incomplete.
- Bus and I/O accesses currently cross the Python/Rust boundary. Performance
  must be measured; the Rust backend is not assumed to be faster.

## Correctness gate

Run the Python/Rust instruction-sequence parity tests and the existing CPU
differential suites before expanding instruction coverage. A passing small
sequence is not proof of full Z80 compatibility or cycle accuracy.
