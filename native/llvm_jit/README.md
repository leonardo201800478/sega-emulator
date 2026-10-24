# LLVM ORC JIT prototype

This directory is an isolated toolchain smoke test for LLVM ORC. It does **not yet execute Z80 instructions** and is not connected to the emulator machine.

## Purpose

Validate that the Windows build environment can create an LLVM ORC `LLJIT`, compile LLVM IR to host-native code, resolve a JIT symbol, and execute it. The next increment replaces the arithmetic smoke function with a Z80 basic-block translator.

## Build on Windows

Install an LLVM development distribution that includes CMake package files, a C++20 compiler compatible with that LLVM build, and CMake 3.25 or newer.

```powershell
cmake -S native/llvm_jit -B build/llvm-jit -G Ninja -DLLVM_DIR="C:\Program Files\LLVM\lib\cmake\llvm"
cmake --build build/llvm-jit --config Release
ctest --test-dir build/llvm-jit -C Release --output-on-failure
```

Adjust `LLVM_DIR` to the installation's actual CMake package directory. This smoke test must pass before implementing the Z80 translator.

## Safety and correctness boundary

- The Python Z80 remains the functional reference.
- This test only validates LLVM's JIT infrastructure; it is not evidence of emulator performance or Z80 correctness.
- The eventual translator must fall back to the interpreter for unsupported instructions and preserve observable memory/I/O behavior.
- Any reduced timing precision belongs to an explicit opt-in `fast` mode.
