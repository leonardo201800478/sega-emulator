# Auditoria E7.7 — F-003: LD A,(nn) / LD (nn),A

**Baseline main pré-PR:** `29462cf0a788d5905de7a5e9e0499a1f1361fec1`.
**PR:** #724 — `test(z80): audit absolute accumulator transfers (F-003)`.
**HEAD atual da candidata:** `cb1583823511cdd9db00bdf5367f98f1e1b4a8d5`.
**Validação CI:** BLOCKED/NOT RUN no HEAD atual. Run #6099 falhou em rustfmt no HEAD anterior; formatação corrigida. Runs #6101–#6103 não produziram Z80 focused validation PASS no HEAD atual. Não houve integração.

## 1. Escopo e referência primária

- `0x3A`: `LD A,(nn)`, ZiLOG UM008011-0816 p. 90.
- `0x32`: `LD (nn),A`, ZiLOG UM008011-0816 p. 93.
- Ambas: quatro M-cycles e 13 T-states nominais. O manual é autoridade para a operação e a temporização documentadas.

O manual não define WZ/MEMPTR como estado arquitetural observável. Não atribuir as fórmulas de WZ a ZiLOG.

## 2. Evidência comparativa de WZ/MEMPTR

- Load: `WZ = (nn + 1) mod 65536`.
- Store: `WZ = (A << 8) | ((nn + 1) & 0x00FF)`.

As fórmulas são confrontadas com MAME `src/devices/cpu/z80/z80.lst` e ares `ares/component/processor/z80/instructions.cpp` (`instructionLD_a_inn` e `instructionLD_inn_a`). São comportamento de compatibilidade comparativo NMOS, não prova física nem requisito expresso no UM0080.

## 3. Matriz candidata

Arquivo: `native/z80_cpu/tests/base_absolute_accumulator_f003.rs`.

- 2 opcodes × cinco endereços (`0000`, `00FF`, `1234`, `FFFE`, `FFFF`) × 256 valores = 2.560 vetores.
- Os valores percorrem todos os 256 bytes e F percorre todos os 256 padrões por transformação bijetiva.
- Verifica A, F preservado, PC, memória, WZ, Q zerado, previous-Q preservado e refresh/R7.
- O fetch de operandos é exercitado com PC=`FFFE`, cobrindo `FFFF` e wrap para `0000`.
- Comparação executor atômico × microsequenciador e oracle separado para WZ.
- Ordem esperada: M1 opcode, leitura do byte low, leitura do byte high, leitura/escrita no endereço absoluto.
- 13T nominais; WAITs injetados 1T, 2T, 3T e 1T nos quatro requests, totalizando 20T no teste microsequenciado.
- Snapshot encode/decode + restore antes e depois de cada resposta de barramento.

## 4. Integração temporal SMS candidata

Arquivo: `native/sms_core/tests/z80_f003_temporal.rs`.

- `LD A,(0000)` verifica leitura de ROM no endereço absoluto e WZ=0001.
- `LD (C000),A` verifica escrita em RAM e WZ com byte alto A.
- WAIT 1T por request; total de 17T (13T nominais + quatro WAITs, incluindo M1).
- Snapshot durante a primeira continuação M1 pendente; restore, retomada e comparação de estado/trace com execução contínua.
- Verifica PC, A, F, WZ, Q/previous-Q, refresh e transações BusCompleted.

## 5. Estado de validação e limites

| Suíte | Estado | Evidência |
|---|---|---|
| Rustfmt do HEAD anterior | FAIL | CI #6099 |
| Rustfmt após correção | NOT RUN | Correção aplicada no HEAD atual |
| Clippy Z80/SMS/SG-1000 no HEAD atual | NOT RUN | CI focado não executado |
| Testes Z80/SMS/SG-1000 no HEAD atual | NOT RUN | CI focado não executado |
| SingleStepTests DD/FD | NOT RUN | Não aplicável ao escopo F-003; sem execução nesta candidata |
| Regressão integral do workspace | NOT RUN | Não disparada |
| Validação física SMS | NOT RUN | Sem alegação de hardware |

A candidata não altera runtime. F-003 permanece AUDIT_PARTIAL até a CI focada passar no HEAD exato e qualquer divergência ser corrigida. Sem validação física SMS; sem alegação de compatibilidade universal CMOS/clones. E7 permanece IN_PROGRESS e E8/ZEXDOC/ZEXALL BLOCKED.
