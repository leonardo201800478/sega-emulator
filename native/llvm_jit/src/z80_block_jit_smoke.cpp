#include "z80_block_jit.hpp"

#include <array>
#include <cstdint>
#include <iostream>

int main() {
    sega::jit::Z80BlockJit jit;
    // LD B,12h; LD C,34h; LD B,C; NOP; HALT
    constexpr std::array<std::uint8_t, 7> code = {
        0x06, 0x12, 0x0E, 0x34, 0x41, 0x00, 0x76,
    };
    auto block = jit.compile(0x8000, code);
    if (block.entry == nullptr || block.bytes_consumed != code.size() ||
        block.instructions != 5 || block.nominal_tstates != 26) {
        std::cerr << "Z80 JIT block compilation failed: " << block.unsupported_reason << '\n';
        return 1;
    }

    sega::jit::Z80JitState state{};
    const auto cycles = block.entry(&state);
    if (cycles != 26 || state.b != 0x34 || state.c != 0x34 || state.halted != 1 ||
        state.pc_low != 0x07 || state.pc_high != 0x80) {
        std::cerr << "Z80 JIT block state mismatch\n";
        return 2;
    }

    constexpr std::array<std::uint8_t, 2> unsupported = {0xC3, 0x00};
    auto fallback = jit.compile(0x9000, unsupported);
    if (fallback.entry != nullptr || fallback.unsupported_reason.empty() ||
        fallback.bytes_consumed != 0) {
        std::cerr << "Unsupported opcode did not request a safe fallback\n";
        return 3;
    }

    constexpr std::array<std::uint8_t, 1> memory_opcode = {0x7E};
    auto memory_fallback = jit.compile(0xA000, memory_opcode);
    if (memory_fallback.entry != nullptr || memory_fallback.unsupported_reason.empty()) {
        std::cerr << "Memory access did not request a safe fallback\n";
        return 4;
    }

    std::cout << "Z80 LLVM ORC block translator smoke test passed\n";
    return 0;
}
