# Auditoria E7.7 — F-004: loads de pares de 16 bits

**Baseline main antes da PR:** `355a35662643ca357ca9360031e002964e5a5aef` (PR #725, reconciliação documental F-003).
**Família:** F-004 — `LD rr,nn` e transferências entre pares.
**Estado nesta branch:** IMPLEMENTED_PENDING_VALIDATION.
**Escopo de perfil:** Zilog Z80 NMOS; evidência de software e comparação entre implementações, sem alegação de medição física.

## 1. Escopo fechado para esta fatia

Incluídos nesta F-004:
- BASE `LD dd,nn`: opcodes `01`, `11`, `21`, `31` para BC, DE, HL e SP.
- BASE `LD (nn),HL`: opcode `22`.
- BASE `LD HL,(nn)`: opcode `2A`.

Famílias correlatas que não devem ser duplicadas:
- `DD/FD 21` (`LD IX/IY,nn`) e `DD/FD 22/2A` (`LD (nn),IX/IY`, `LD IX/IY,(nn)`): E3A1/E3A2 e auditorias DD/FD.
- `ED 43/53/63/73` e `ED 4B/5B/6B/7B` (`LD (nn),dd` / `LD dd,(nn)`): família ED/F-034.
- `LD SP,HL/IX/IY` e regras específicas do SP: F-005.
- `EX DE,HL`, bancos alternativos e `EX (SP),HL/IX/IY`: F-006.

Esta delimitação evita duplicar a cobertura E3/ED e preserva as dependências declaradas no checklist mestre.

## 2. Referência documental — ZiLOG UM008011-0816

- **p. 99 — LD dd,nn:** dd=BC/DE/HL/SP; primeiro byte imediato é low-order; 2 M-cycles, 10 T-states (4,3,3); nenhuma flag afetada.
- **p. 102 — LD HL,(nn):** lê `(nn)` para L e `(nn+1)` para H; 5 M-cycles, 16 T-states (4,3,3,3,3); nenhuma flag afetada.
- **p. 107 — LD (nn),HL:** escreve L em `nn` e H em `nn+1`; 5 M-cycles, 16 T-states (4,3,3,3,3); nenhuma flag afetada.

O manual especifica ordem little-endian e tempos nominais. Não especifica WZ/MEMPTR nem Q; esses detalhes não devem ser atribuídos à documentação primária.

## 3. Referências comparativas independentes

- MAME `src/devices/cpu/z80/z80.lst`: `0001`, `0011`, `0021`, `0031` usam `@arg16`; `0022` e `002a` executam o par de acessos de memória e definem `WZ = m_ea + 1`.
- ares `ares/component/processor/z80/instructions.cpp`: `instructionLD_rr_nn` limpa Q e carrega o par por `operands()` sem atribuir WZ; `instructionLD_inn_rr` e `instructionLD_rr_inn` usam WZ como endereço e terminam em `nn+1`.

As fórmulas de WZ são evidência comparativa de implementações, não uma afirmação contida no UM0080 nem prova física. Links:
- ZiLOG: https://www.zilog.com/docs/z80/um0080.pdf
- MAME: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.lst
- ares: https://github.com/ares-emulator/ares/blob/master/ares/component/processor/z80/instructions.cpp

## 4. Auditoria do código existente antes da expansão

- `native/z80_cpu/src/cpu.rs`: executor atômico implementa `01/11/21/31` e `22/2A`.
- `native/z80_cpu/src/sequencer.rs`: fases `ImmediateReadRegisterPairLow/High` e `BaseAbsoluteTransfer`; escrita do par após a leitura high; acessos absolutos em low/high.
- `native/z80_cpu/src/snapshot.rs`: já havia teste de round-trip de snapshot para as fases imediatas low/high e para estágios de transferência absoluta. O teste de snapshot existente não verificava sozinho todo o resultado arquitetural nem a equivalência de retomada.
- `native/sms_core/src/temporal_machine.rs`: já havia uma matriz pequena para os quatro pares comparada ao executor atômico, um caso WAIT apenas para HL e testes nominais de `22/2A`. Não havia matriz ampla de bytes/flags/endereço, wrap de PC, trace por ciclo e restore diferencial suficientes para fechar F-004.
- Nenhuma alteração de runtime é necessária se os novos testes passarem; qualquer divergência será classificada como runtime versus oracle antes de corrigir código.

## 5. Novas evidências nesta branch

### 5.1 BASE LD dd,nn

Arquivo: `native/z80_cpu/tests/base_register_pair_f004.rs`.

- 4 pares × (256 padrões de byte com cobertura de low/high e F + 7 valores-limite) = 1.052 vetores.
- BC, DE, HL e SP verificados individualmente; registradores não-alvo preservados.
- F preservado; Q zerado; previous-Q preservado; WZ preservado; refresh/R7 verificado.
- PC avança três bytes, incluindo fetch de operandos em FFFE→FFFF→0000.
- 10 T-states nominais; M1 + leitura low + leitura high.
- WAITs diferenciados 1T/2T/3T; 16T totais no microsequenciador.
- Snapshot encode/decode + restore antes e depois de cada resposta de barramento.
- Diferencial entre executor atômico e microsequenciador com oracle explícito para registradores, PC, flags, WZ e refresh.

### 5.2 BASE LD (nn),HL / LD HL,(nn)

Mesmo arquivo:
- 2 opcodes × 5 endereços (`0000`, `00FF`, `1234`, `FFFE`, `FFFF`) × 256 palavras = 2.560 vetores.
- Verifica low/high, wrap de endereço em FFFF→0000, PC e WZ=`nn+1`.
- Flags preservadas; Q zerado; previous-Q preservado; refresh/R7.
- 16 T-states nominais e cinco M-cycles.
- WAITs 1T/2T/3T/1T/2T; 25T no microsequenciador.
- Snapshot encode/decode + restore em todas as fronteiras de resposta.
- Casos adicionais com PC em FFFE verificam wrap do fetch de operandos para ambos os opcodes.

### 5.3 Integração SMS / Scheduler

Arquivo: `native/sms_core/tests/z80_f004_temporal.rs`.

- Os quatro `LD dd,nn` passam pelo scheduler com WAIT, snapshot durante M1 pendente, restore e retomada.
- `LD (nn),HL` escreve low/high em RAM; `LD HL,(nn)` lê low/high.
- Com um WAIT por request: 13T para `LD dd,nn`; 21T para as transferências absolutas HL.
- Compara estado serializado, resultado, WZ, F, Q/previous-Q, refresh, relógio do scheduler e trace de BusCompleted entre execução contínua e retomada.

**Contagem da matriz:** 3.612 vetores principais (1.052 + 2.560), mais seis casos de wrap de PC; integração temporal cobre os quatro opcodes imediatos e ambos os absolutos HL.

## 6. Critérios de aceite e estado

- Rustfmt: pendente no HEAD final da PR.
- Clippy com `-D warnings`: pendente no HEAD final da PR.
- Testes focados Z80/SMS/SG-1000: pendentes.
- Regressão integral do workspace: classificar conforme política de impacto; NOT RUN não é PASS.
- Documentação hygiene: pendente.
- Nenhuma alteração de runtime é autorizada sem falha reproduzível nos testes e confronto do oracle.
- F-004 permanece `IMPLEMENTED_PENDING_VALIDATION` até todos os gates aplicáveis passarem e a PR ser integrada por squash.
- E7 permanece IN_PROGRESS; E8/ZEXDOC/ZEXALL permanece BLOCKED.
