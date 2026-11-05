# E7.7 — DD/FD half-register LD matrix

**Data:** 2026-11-05  
**Baseline:** `1da784f556b9524f5a6f43ce8a7c525e68354f9e`  
**Escopo:** DD/FD-prefixed `LD r,r'` family, including IXH/IXL, IYH/IYL and indexed-memory H/L exceptions.

## 1. Evidência documental

UM008011-0816 documents IX/IY as 16-bit index registers and the indexed-addressing forms, but does not fully document the undocumented 8-bit aliases IXH/IXL and IYH/IYL.

The behavior is characterized in secondary technical references and reference implementations:
- *The Undocumented Z80 Documented*: https://z80.info/zip/z80-documented.pdf
- MAME Z80 decoder: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.lst
- redcode/Z80: https://github.com/redcode/Z80/blob/master/sources/Z80.c

The critical exception is the indexed-memory form: when one operand is `(IX+d)` or `(IY+d)`, the H/L operand in the same instruction refers to ordinary H/L, not IXH/IXL or IYH/IYL. For register-only DD/FD-prefixed `LD r,r'`, codes 4 and 5 select the corresponding index high/low byte.

## 2. Código integrado confrontado

Arquivo: `native/z80_cpu/src/cpu.rs`, indexed decoder/executor.

A implementação seleciona IXH/IXL ou IYH/IYL para operações registrador-registrador e usa H/L reais quando há um operando de memória indexada.

A exceção de memória é funcionalmente importante:
- `DD 66 d`: LD H,(IX+d), destino H normal;
- `DD 6E d`: LD L,(IX+d), destino L normal;
- `DD 74 d`: LD (IX+d),H, origem H normal;
- `DD 75 d`: LD (IX+d),L, origem L normal.

O mesmo vale para FD/IY.

## 3. Cobertura anterior

`native/z80_cpu/tests/indexed_semantic_baseline.rs` cobria:
- uma transferência B,IXH;
- uma exceção H,(IX+d);
- um caso BIT indexed com X/Y do endereço efetivo;
- um caso de deslocamento indexado.

Isso não era cobertura suficiente da matriz completa `LD r,r'`.

## 4. Incremento desta branch

Novo teste:
`native/sms_core/tests/z80_indexed_half_register_matrix.rs`

A matriz executa 126 casos:
- prefixos DD e FD;
- 63 opcodes LD r,r' por prefixo, excluindo 76 (HALT);
- IXH/IXL e IYH/IYL;
- todas as combinações de registradores e operandos de memória;
- exceções H/L em instruções com `(IX+d)`/`(IY+d)`;
- deslocamentos positivos e negativos;
- PC, R, flags, T-states e efeitos em memória.

O modelo de referência é separado do decoder de produção. A suíte não é um oráculo de silício; é uma matriz funcional de regressão derivada da semântica documentada por referências técnicas.

## 5. Limites desta entrega

Esta branch cobre somente a família `LD r,r'` DD/FD. Não fecha:
- ALU A,IXH/IXL/IYH/IYL;
- INC/DEC IXH/IXL/IYH/IYL;
- BIT/RES/SET indexed em toda a matriz;
- todas as combinações de prefixos repetidos;
- equivalência física em hardware Z80A.

Nenhuma mudança de runtime é proposta, pois a inspeção não identificou divergência de execução nesta família.

## 6. ADR-0016 e temporalidade

A matriz verifica os T-states nominais (8 para registrador-registrador; 19 para indexed-memory), mas não cria microsequenciamento universal. A granularidade adicional só será introduzida se um consumidor SMS demonstrar observabilidade de fases intermediárias.

## 7. Validação

- Testes locais: NOT RUN nesta sessão (sem rustc/cargo).
- CI remoto: aguardando execução no HEAD exato da PR.
- Nenhum resultado SKIPPED será classificado como PASS.
- E7.7 global permanece IN_PROGRESS; E8 permanece BLOCKED.
