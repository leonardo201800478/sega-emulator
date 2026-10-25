#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <span>
#include <string>

#include "llvm/ExecutionEngine/Orc/LLJIT.h"

namespace sega::jit {

struct Z80JitState {
    std::uint8_t a = 0;
    std::uint8_t f = 0;
    std::uint8_t b = 0;
    std::uint8_t c = 0;
    std::uint8_t d = 0;
    std::uint8_t e = 0;
    std::uint8_t h = 0;
    std::uint8_t l = 0;
    std::uint8_t pc_low = 0;
    std::uint8_t pc_high = 0;
    std::uint8_t halted = 0;
};

using Z80JitEntry = std::uint32_t (*)(Z80JitState*);

struct Z80JitBlock {
    Z80JitEntry entry = nullptr;
    std::uint16_t start_pc = 0;
    std::size_t bytes_consumed = 0;
    std::size_t instructions = 0;
    std::uint32_t nominal_tstates = 0;
    std::string unsupported_reason;
};

class Z80BlockJit {
public:
    Z80BlockJit();
    Z80BlockJit(const Z80BlockJit&) = delete;
    Z80BlockJit& operator=(const Z80BlockJit&) = delete;
    ~Z80BlockJit();

    // Initial subset: NOP, register-only LD r,n / LD r,r, and HALT.
    // Unsupported blocks are returned as non-executable; no partial execution.
    Z80JitBlock compile(std::uint16_t start_pc, std::span<const std::uint8_t> code);

private:
    std::unique_ptr<llvm::orc::LLJIT> jit_;
    std::uint64_t next_block_id_ = 0;
};

}  // namespace sega::jit
