# Auditoria E7.7 — F-003: LD A,(nn) / LD (nn),A

**Baseline main pré-PR:** `29462cf0a788d5905de7a5e9e0499a1f1361fec1`.
**PR:** #724 — `test(z80): audit absolute accumulator transfers (F-003)`; squash merge `7fde212aabc07e8c4b7df56938f8845f99d29499`.
**HEAD validado da PR:** `455ae4ea5ce68fdba53de1958fe0123afd39dc6d`.
**CI final pré-merge:** #6111 / run `42675073118` PASS no HEAD exato `455ae4ea5ce68fdba53de1958fe0123afd39dc6d`.
**Main pós-merge:** `7fde212aabc07e8c4b7df56938f8845f99d29499`.
**Histórico de CI:** #6107 / run `42672954884` falhou no agregador por orquestração: classificação SUCCESS, `EXPECT_Z80=true` e `EXPECT_DOCS=true`, mas jobs Z80 e docs retornaram `abandoned`. Esse run não executou gates de código e não representa falha funcional. O evento `synchronize` subsequente disparou corretamente os jobs.
**Gates no HEAD validado:** rustfmt PASS; Clippy focado PASS; testes Z80/SMS/SG-1000 PASS; SingleStepTests DD/FD 34.000/34.000 PASS; documentation patch hygiene PASS; agregador PASS. Regressão integral do workspace e jobs não relacionados NOT RUN/SKIPPED por política de impacto.

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
| Rustfmt Z80/SMS/SG-1000 | PASS | CI #6111 / run `42675073118`, HEAD `455ae4ea` |
| Clippy Z80/SMS/SG-1000 com `-D warnings` | PASS | CI #6111 / run `42675073118`, HEAD `455ae4ea` |
| Testes Z80/SMS/SG-1000 | PASS | CI #6111 / run `42675073118`, HEAD `455ae4ea`; testes F-003 atômicos e integração SMS temporal executados |
| SingleStepTests DD/FD | PASS | 34.000/34.000 vetores (2.000 + 32.000), CI #6111 |
| Documentation patch hygiene | PASS | CI #6111 / run `42675073118` |
| Agregador Rust quality/unit | PASS | CI #6111 / run `42675073118` |
| Regressão integral do workspace | NOT RUN | Não disparada pela política de impacto |
| Scheduler stress e hosts/subsistemas não relacionados | NOT RUN | Jobs SKIPPED por política de impacto; não são PASS |
| Validação física SMS | NOT RUN | Sem alegação de hardware |

A implementação não altera runtime. A matriz e a integração temporal F-003 passaram nos gates focados no HEAD indicado e a PR #724 foi integrada por squash na main `7fde212aabc07e8c4b7df56938f8845f99d29499`. F-003 está CLOSED somente para semântica de software do perfil de compatibilidade Zilog NMOS delimitado. O CI pós-merge permanece NOT VERIFIED: a consulta não retornou checks nem run push associado ao SHA de merge; não classificar como PASS. Sem validação física SMS; sem alegação de compatibilidade universal CMOS/clones. E7 permanece IN_PROGRESS e E8/ZEXDOC/ZEXALL BLOCKED.
