# E7.7 — DD/FD half-register ALU and INC/DEC matrix

**Data:** 2026-11-05  
**Baseline:** `1da784f556b9524f5a6f43ce8a7c525e68354f9e`  
**Branch:** `test/z80-indexed-half-register-alu-matrix-20261105`  
**Escopo:** ALU A,IXH/IXL/IYH/IYL and INC/DEC IXH/IXL/IYH/IYL.

## 1. Evidência documental

The ZiLOG UM0080 user manual describes IX/IY indexed addressing and the documented indexed-memory ALU forms. It does not provide a complete documented contract for IXH/IXL/IYH/IYL as independent 8-bit operands.

The half-register encodings are therefore treated as undocumented Z80 behavior, not as a new documented SMS-specific feature. Secondary instruction references enumerate:
- DD/FD-prefixed ALU source aliases IXH/IXL and IYH/IYL for ADD, ADC, SUB, SBC, AND, XOR, OR and CP;
- DD/FD-prefixed INC/DEC aliases for all four index halves;
- 8T nominal duration for register-only prefixed forms (the 4T base instruction plus the 4T prefix).

References:
- ZiLOG UM008011-0816: https://www.zilog.com/docs/z80/um0080.pdf
- MSX Wiki, Z80 opcodes / DD-prefixed opcode table: https://www.msx.org/wiki/Z80_Opcodes
- Z80 instruction reference, INC: https://asm-editor.specy.app/documentation/z80/instruction/inc
- Z80 instruction reference, ADD: https://asm-editor.specy.app/documentation/z80/instruction/add
- Z80 instruction reference, ADC: https://asm-editor.specy.app/documentation/z80/instruction/adc
- *The Undocumented Z80 Documented*: https://datasheets.chipdb.org/Zilog/Z80/z80-documented_0.4.pdf
- MAME Z80 decoder: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.lst
- redcode/Z80: https://github.com/redcode/Z80/blob/master/sources/Z80.c

The sources above characterize opcode aliases and nominal behavior; they are not a substitute for an independently captured Zilog NMOS silicon trace. CMOS/clone equivalence is not claimed.

## 2. Production implementation inspected

File: `native/z80_cpu/src/cpu.rs`.

The atomic DD/FD decoder:
- maps source register codes 4 and 5 to the active index high/low byte for register-only ALU operations;
- routes DD/FD 24/25/2C/2D to the shared INC/DEC flag helper and writes the result back to the active index half;
- preserves the other byte of IX/IY;
- uses the shared ALU helpers for result and flag semantics.

No runtime defect was identified in this inspection. This PR adds regression coverage only.

## 3. Added differential regression

Test: `native/sms_core/tests/z80_indexed_half_register_alu.rs`.

### ALU matrix

- Both DD and FD prefixes.
- IXH, IXL, IYH and IYL as source.
- All eight ALU operations: ADD, ADC, SUB, SBC, AND, XOR, OR and CP.
- Thirteen edge-oriented accumulator values and thirteen source values.
- Both carry-in states.
- 10,816 cases total.
- Compares indexed execution against the corresponding unprefixed BASE H/L operation for A and F.
- Checks index-half preservation, PC, R refresh count and nominal T-states (8T indexed versus 4T BASE).

### INC/DEC matrix

- Both DD and FD prefixes.
- IXH, IXL, IYH and IYL.
- INC and DEC.
- All 256 input byte values and both carry states.
- 4,096 cases total.
- Compares flags against the corresponding BASE H/L operation.
- Checks the resulting index byte, preservation of the other index byte and ordinary H/L, PC, R and nominal T-states (8T indexed versus 4T BASE).

Total: 14,912 deterministic cases.

The BASE H/L executor is used as a differential reference for the shared arithmetic/flag behavior. This specifically verifies DD/FD operand selection and writeback; it is not an independent arithmetic oracle because the BASE and indexed paths share the ALU/INC/DEC helpers. No silicon-level claim is made.

## 4. ADR-0016

These are register-internal operations with no intermediate externally observable state demonstrated for SMS. The atomic executor is therefore the appropriate implementation path. The tests verify the nominal extra prefix cost without adding microsequencer phases, WAIT states or scheduler events.

## 5. Validation status

- Local Rust tests: NOT RUN in this session; the execution environment has no Rust toolchain.
- CI: must be evaluated on the final PR HEAD.
- Scheduler stress, Qt, PowerShell and lock-refresh jobs must retain their actual PASS/SKIPPED/NOT RUN classification.
- No runtime files are changed.
- E7.7 remains IN_PROGRESS until the remaining semantic families and evidence are reconciled; E8 remains BLOCKED.
