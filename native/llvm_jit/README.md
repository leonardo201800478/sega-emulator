# LLVM ORC JIT prototype

This directory contains an isolated LLVM ORC smoke test and the first Z80
basic-block translator slice. It is not yet connected to the SG-1000 machine
and does not claim a performance improvement.

## Current translated subset

- NOP: 4 nominal T-states.
- LD r,n: register destinations only, 7 nominal T-states.
- LD r,r: register-to-register forms only, 4 nominal T-states.
- HALT: 4 nominal T-states and terminates the block.

Any instruction involving (HL), any other unsupported opcode, an empty block,
or a truncated immediate returns a non-executable result. The caller must
execute from the original PC using the interpreter; partial translated blocks
are never returned as executable. The ABI is intentionally a small C++ state
structure, not yet the Rust/PyO3 CPU state.

## Build on Windows

Install an LLVM development distribution that includes CMake package files, a
C++20 compiler compatible with that LLVM build, CMake 3.25 or newer, and Ninja.

    cmake -S native/llvm_jit -B build/llvm-jit -G Ninja -DLLVM_DIR="C:\Program Files\LLVM\lib\cmake\llvm"
    cmake --build build/llvm-jit --config Release
    ctest --test-dir build/llvm-jit -C Release --output-on-failure

Adjust LLVM_DIR to the installation's actual CMake package directory.

## Correctness boundary

- The Python Z80 remains the functional reference; the Rust interpreter is a
  second independent comparison target when its branch is integrated.
- The current translator preserves nominal instruction costs for this subset,
  but it does not model per-T-state bus activity or cycle-level device
  interleaving.
- The caller must end blocks before control flow, interrupts, observable I/O,
  and memory operations not represented by the translator.
- No self-modifying code, cache invalidation, emulator integration, or benchmark
  claim is included in this first increment.
