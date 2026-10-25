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
  IM 0/1/2 interrupt handling and NMI entry, Q/P latches, ED 16-bit ADC/SBC, word transfers,
  IN/OUT (C), NEG, LD I/R and A,I/R, RRD/RLD, block transfer, block compare and
  block I/O families; accumulator rotates, DAA/CPL/SCF/CCF, EX AF,AF', EXX,
  EX (SP),HL and EX DE,HL. Unassigned ED bytes follow classic-Z80 8-T-state NOP
  behavior.
- Unsupported instructions raise \`NotImplementedError\`. There is no silent
  fallback to Python, because that would make performance measurements invalid.
- The base opcode map and CB/ED/indexed dispatch now have broad implementation
  coverage. Tests were added for all 256 primary bytes, all 256 CB second bytes,
  all DD/FD register/ALU forms, and all DD/FD-CB second bytes. These are test
  definitions, not yet confirmed passing results. Remaining gaps include some
  undocumented edge cases, interrupt timing/priority corner cases, and
  bus-cycle-accurate contention.
- Bus and I/O accesses currently cross the Python/Rust boundary. Performance
  must be measured; the Rust backend is not assumed to be faster.

## Correctness gate

Run the Python/Rust instruction-sequence parity tests and the existing CPU
differential suites before expanding instruction coverage. Targeted differential tests cover all 256 primary opcode bytes, all 256 CB
second-byte values, indexed register/ALU forms, all 256 DD/FD-CB second-byte values,
ED arithmetic and special-register operations, block transfer/compare/I/O, and
unassigned ED NOPs. CI status must be checked before treating these tests as passing.
Q/P latch behavior and execution of the interrupt-supplied opcode in IM 0 are
implemented against the Python reference and covered by targeted differential
tests; CI still needs to confirm them. The completion roadmap is tracked in
[issue #55](https://github.com/leonardo201800478/sega-emulator/issues/55). A passing
differential case is not proof of full Z80 compatibility or cycle accuracy.
