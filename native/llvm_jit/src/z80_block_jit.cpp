#include "z80_block_jit.hpp"

#include <array>
#include <stdexcept>
#include <string>
#include <utility>

#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/TargetSelect.h"

namespace sega::jit {
namespace {

constexpr std::array<std::size_t, 8> kRegisterOffsets = {2, 3, 4, 5, 6, 7, 0, 0};
constexpr std::size_t kAOffset = 0;
constexpr std::size_t kPcLowOffset = 8;
constexpr std::size_t kPcHighOffset = 9;
constexpr std::size_t kHaltedOffset = 10;

llvm::Value* state_byte(llvm::IRBuilder<>& builder, llvm::Value* state,
                        llvm::Type* i8, std::size_t offset) {
    auto* index = llvm::ConstantInt::get(llvm::Type::getInt64Ty(builder.getContext()), offset);
    auto* address = builder.CreateInBoundsGEP(i8, state, index, "state_byte_ptr");
    return builder.CreateLoad(i8, address, "state_byte");
}

void store_state_byte(llvm::IRBuilder<>& builder, llvm::Value* state, llvm::Type* i8,
                      std::size_t offset, llvm::Value* value) {
    auto* index = llvm::ConstantInt::get(llvm::Type::getInt64Ty(builder.getContext()), offset);
    auto* address = builder.CreateInBoundsGEP(i8, state, index, "state_byte_ptr");
    builder.CreateStore(value, address);
}

std::size_t register_offset(std::uint8_t code) {
    return kRegisterOffsets.at(code);
}

}  // namespace

Z80BlockJit::Z80BlockJit() {
    llvm::InitializeNativeTarget();
    llvm::InitializeNativeTargetAsmPrinter();
    llvm::InitializeNativeTargetAsmParser();
    auto jit = llvm::orc::LLJITBuilder().create();
    if (!jit) {
        llvm::consumeError(jit.takeError());
        throw std::runtime_error("Could not create LLVM ORC LLJIT");
    }
    jit_ = std::move(*jit);
}

Z80BlockJit::~Z80BlockJit() = default;

Z80JitBlock Z80BlockJit::compile(std::uint16_t start_pc,
                                 std::span<const std::uint8_t> code) {
    using namespace llvm;
    using namespace llvm::orc;

    Z80JitBlock result;
    result.start_pc = start_pc;
    if (code.empty()) {
        result.unsupported_reason = "empty block";
        return result;
    }

    auto context = std::make_unique<LLVMContext>();
    auto module = std::make_unique<Module>("sega_z80_block", *context);
    module->setDataLayout(jit_->getDataLayout());
    module->setTargetTriple(jit_->getTargetTriple().str());

    auto* i8 = Type::getInt8Ty(*context);
    auto* i32 = Type::getInt32Ty(*context);
    auto* state_type = PointerType::getUnqual(*context);
    auto* function_type = FunctionType::get(i32, {state_type}, false);
    const auto symbol_name = "sega_z80_block_" + std::to_string(next_block_id_++);
    auto* function = Function::Create(function_type, Function::ExternalLinkage,
                                      symbol_name, module.get());
    auto* state = function->getArg(0);
    state->setName("state");
    auto* entry = BasicBlock::Create(*context, "entry", function);
    IRBuilder<> builder(entry);

    std::uint32_t cycles = 0;
    std::size_t offset = 0;
    std::size_t instructions = 0;
    bool terminated = false;
    while (offset < code.size()) {
        const auto opcode = code[offset];
        if (opcode == 0x00) {
            ++offset;
            ++instructions;
            cycles += 4;
            continue;
        }
        if (opcode == 0x76) {
            ++offset;
            ++instructions;
            cycles += 4;
            store_state_byte(builder, state, i8, kHaltedOffset, ConstantInt::get(i8, 1));
            terminated = true;
            break;
        }
        if ((opcode & 0xC7) == 0x06) {
            const auto destination = static_cast<std::uint8_t>((opcode >> 3) & 7);
            if (destination == 6) {
                result.unsupported_reason = "LD (HL),n requires a memory side exit";
                break;
            }
            if (offset + 1 >= code.size()) {
                result.unsupported_reason = "truncated LD r,n";
                break;
            }
            const auto destination_offset =
                destination == 7 ? kAOffset : register_offset(destination);
            store_state_byte(builder, state, i8, destination_offset,
                             ConstantInt::get(i8, code[offset + 1]));
            offset += 2;
            ++instructions;
            cycles += 7;
            continue;
        }
        if (opcode >= 0x40 && opcode <= 0x7F && opcode != 0x76) {
            const auto destination = static_cast<std::uint8_t>((opcode >> 3) & 7);
            const auto source = static_cast<std::uint8_t>(opcode & 7);
            if (destination == 6 || source == 6) {
                result.unsupported_reason = "LD involving (HL) requires a memory side exit";
                break;
            }
            const auto source_offset = source == 7 ? kAOffset : register_offset(source);
            const auto destination_offset =
                destination == 7 ? kAOffset : register_offset(destination);
            auto* value = state_byte(builder, state, i8, source_offset);
            store_state_byte(builder, state, i8, destination_offset, value);
            ++offset;
            ++instructions;
            cycles += 4;
            continue;
        }
        result.unsupported_reason = "unsupported opcode at block offset " +
                                    std::to_string(offset) + ": " +
                                    std::to_string(opcode);
        break;
    }

    if (instructions == 0 || (!result.unsupported_reason.empty() && !terminated)) {
        return result;
    }

    const auto next_pc = static_cast<std::uint16_t>(start_pc + offset);
    store_state_byte(builder, state, i8, kPcLowOffset, ConstantInt::get(i8, next_pc & 0xFF));
    store_state_byte(builder, state, i8, kPcHighOffset, ConstantInt::get(i8, next_pc >> 8));
    builder.CreateRet(ConstantInt::get(i32, cycles));

    if (verifyModule(*module, &errs())) {
        result.unsupported_reason = "LLVM verifier rejected translated block";
        return result;
    }
    ThreadSafeContext thread_safe_context(std::move(context));
    ThreadSafeModule thread_safe_module(std::move(module), std::move(thread_safe_context));
    if (auto error = jit_->addIRModule(std::move(thread_safe_module))) {
        consumeError(std::move(error));
        result.unsupported_reason = "LLVM ORC failed to add translated block";
        return result;
    }
    auto symbol = jit_->lookup(symbol_name);
    if (!symbol) {
        consumeError(symbol.takeError());
        result.unsupported_reason = "LLVM ORC failed to resolve translated block";
        return result;
    }

    result.entry = symbol->toPtr<Z80JitEntry>();
    result.bytes_consumed = offset;
    result.instructions = instructions;
    result.nominal_tstates = cycles;
    return result;
}

}  // namespace sega::jit
