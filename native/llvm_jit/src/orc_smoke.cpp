#include <cstdint>
#include <iostream>
#include <memory>

#include "llvm/ExecutionEngine/Orc/LLJIT.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/Error.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"

int main() {
    using namespace llvm;
    using namespace llvm::orc;

    InitializeNativeTarget();
    InitializeNativeTargetAsmPrinter();
    InitializeNativeTargetAsmParser();

    auto jit_expected = LLJITBuilder().create();
    if (!jit_expected) {
        logAllUnhandledErrors(jit_expected.takeError(), errs(), "LLJIT creation failed: ");
        return 1;
    }
    auto jit = std::move(*jit_expected);

    auto tsc = std::make_unique<LLVMContext>();
    auto tsm = std::make_unique<Module>("sega_z80_jit_smoke", *tsc);
    tsm->setDataLayout(jit->getDataLayout());
    tsm->setTargetTriple(jit->getTargetTriple().str());
    auto *t_i64 = Type::getInt64Ty(*tsc);
    auto *t_i8 = Type::getInt8Ty(*tsc);
    auto *t_ft = FunctionType::get(t_i64, {t_i64}, false);
    auto *t_fn = Function::Create(t_ft, Function::ExternalLinkage, "sega_jit_smoke", tsm.get());
    auto t_arg = t_fn->arg_begin();
    t_arg->setName("input");
    auto *t_entry = BasicBlock::Create(*tsc, "entry", t_fn);
    IRBuilder<> t_builder(t_entry);
    auto *t_inc = t_builder.CreateAdd(&*t_arg, ConstantInt::get(t_i64, 1));
    auto *t_low = t_builder.CreateTrunc(t_inc, t_i8);
    auto *t_ret = t_builder.CreateZExt(t_low, t_i64);
    t_builder.CreateRet(t_ret);

    ThreadSafeContext thread_safe_context(std::move(tsc));
    auto thread_safe_module_wrapper =
        ThreadSafeModule(std::move(tsm), std::move(thread_safe_context));
    if (auto error = jit->addIRModule(std::move(thread_safe_module_wrapper))) {
        logAllUnhandledErrors(std::move(error), errs(), "Adding IR module failed: ");
        return 1;
    }

    auto symbol = jit->lookup("sega_jit_smoke");
    if (!symbol) {
        logAllUnhandledErrors(symbol.takeError(), errs(), "JIT symbol lookup failed: ");
        return 1;
    }

    using JitFunction = std::uint64_t (*)(std::uint64_t);
    auto entry_point = symbol->toPtr<JitFunction>();
    for (std::uint64_t value : {0ULL, 1ULL, 0x7EULL, 0xFFULL, 0x1234ULL}) {
        const auto expected = (value + 1) & 0xFF;
        const auto actual = entry_point(value);
        if (actual != expected) {
            std::cerr << "Mismatch for input " << value << ": expected " << expected
                      << ", got " << actual << '\n';
            return 2;
        }
    }

    std::cout << "LLVM ORC smoke test passed on " << jit->getTargetTriple().str() << '\n';
    return 0;
}
