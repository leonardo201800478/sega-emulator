# Estratégia de conclusão DD/FD do Z80

**Status:** plano de execução para a próxima fatia de trabalho  
**Data da auditoria:** 2026-11-04  
**Baseline inicial E0:** `e4604f3152c09e8d16c37f94f94a4b344db33b62`  
**Baseline após E1 / PR #467:** `aa4a4fa7661c28d3b2e21ac6d6eb4379fd297f65`  
**Baseline após E2 / PR #468:** `abd154332b1dabfb68874f2b80f306da7e060552`  
**Baseline atual da main após PR #485:** `f4130104fa971289507c0b15ab2d8eabd1dd5b00`  
**HEAD E4C validado:** `8f04b44e6c6ba85920a7438b74c9876586d5736c`; CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 PASS.  
**PR de referência histórica:** [#447](https://github.com/leonardo201800478/sega-emulator/pull/447) — manter aberta, sem merge  
**PR integrada E4A:** [#471](https://github.com/leonardo201800478/sega-emulator/pull/471)  
**PR integrada E4C:** [#475](https://github.com/leonardo201800478/sega-emulator/pull/475)  
**PR E5 integrada:** [#485](https://github.com/leonardo201800478/sega-emulator/pull/485), HEAD `2cc6105a0c8c3589994870a6a67ceffb1e8c02f8`; CI #4245 SUCCESS (Rust, Python, Windows PASS; Scheduler SKIPPED); merge SHA `f4130104fa971289507c0b15ab2d8eabd1dd5b00`.  
**Escopo:** microsequenciador Z80 compartilhado; não confundir conclusão do núcleo com integração temporal completa ao SMS.

---

## 1. Decisão arquitetural

A implementação antiga da PR #447 **não será integrada, rebaseada nem usada como base de código**. Seu conteúdo será material de auditoria: ideias, casos de teste e comportamento a revalidar.

Ordem de execução:
1. Congelar e registrar o estado real da main.
2. Construir matriz semântica DD/FD/DDCB/FDCB, separada do inventário de dispatch.
3. Validar os oráculos e vetores externos.
4. Alterar o microsequenciador em fatias pequenas, compatíveis com o estado atual.
5. Provar snapshot/replay e comportamento temporal separadamente.
6. Integrar ao SMS somente após demonstrar o contrato do Z80 compartilhado.

**A PR #467 concluiu a primeira fatia documental/testes E1 sem alterar runtime; foi integrada na main após todos os gates obrigatórios passarem. PRs #469 (E3A1) e #470 (E3A2) também foram integradas; a main atual é `5b61b918`. A arquitetura permanece centralizada nos crates compartilhados. Não criar Z80 alternativo ou sequenciador específico do SMS.

## 2. Baseline E0 — fatos confirmados

### 2.1 Main e schema

| Item | Estado confirmado |
|---|---|
| SHA da main na auditoria inicial | `e4604f3152c09e8d16c37f94f94a4b344db33b62` |
| SHA da main após E1 (#467) | `aa4a4fa7661c28d3b2e21ac6d6eb4379fd297f65` |
| SHA da main após E2 (#468) | `abd154332b1dabfb68874f2b80f306da7e060552` |
| SHA da main após E3A1 (#469) | `ee9d39fc017b53493cc48d1343a7ca4b77581d3b` |
| SHA da main após E3A2 (#470) | `5b61b918055613902a70d60a2fdf645bb543f807` |
| SHA da main após E4A (#471) | `e9a3eff6c3733b4f11443954a647471c9d3908ff` |
| Candidato E4B / PR #472 | `245d42eb0337d41295329155de8e151feceb9022` |
| Schema inicial E0 | `Z80_MICRO_SEQUENCER_STATE_VERSION = 25` |
| Schema após E2 | `Z80_MICRO_SEQUENCER_STATE_VERSION = 26` |
| Schema atual no candidato E4A | `Z80_MICRO_SEQUENCER_STATE_VERSION = 27` |
| Codec | `native/z80_cpu/src/snapshot.rs`; não é save state integral da máquina SMS |
| Inventário BASE | PR #465 integrada; 251 encodings com handler e 1 lacuna explícita (HALT), excluídos CB/DD/ED/FD |
| Fonte da verdade | Código e testes da main no SHA acima |

O inventário BASE da #465 mede **dispatch/retirement do microsequenciador**. Não prova semântica completa, precisão de barramento ou cobertura DD/FD.

### 2.2 Prefixos no microsequenciador atual

Inspeção de `native/z80_cpu/src/sequencer.rs`:

- A FSM explícita agora mantém `None`, `CB`, `ED` ou `Index(IX/IY)` como estado de continuação serializável.
- DD/FD repetidos ou alternados atualizam o índice efetivo; o último prefixo vence.
- ED após DD/FD cancela a indexação e passa à família ED.
- O opcode após DD/FD é classificado como `UsesIndex`, `Ignored` ou `IndexedCb`; a execução das famílias permanece nas etapas E3-E7.
- DDCB/FDCB ainda não consome displacement/subopcode não-M1 no microsequenciador.
- Prefixos indexados seguidos de opcode terminal ainda resultam em `UnsupportedOpcode` até a implementação das respectivas famílias.
- O snapshot inclui o latch e valida a coerência fase/opcode; schema 26. O round-trip após DD está coberto.
- Não há limite artificial para cadeias DD/FD; teste com 257 prefixos alternados.
- HALT (0x76) permanece explicitamente não implementado no inventário BASE do microsequenciador.

Estado correto: **FSM DD/FD implementada e QUICK_PASS/SHORT_PASS na dimensão de transição/estado; sem conclusão semântica ou temporal das instruções indexadas**.

### 2.3 PR #447

- Estado: aberta; `mergeable=false`.
- Base antiga: `eba8faae3f87baed2e64dfbe96f2009474d9f212`.
- Head: `405944de94b7a90b921867a1ce8a827e658f08b9`.
- 68 commits; 4 arquivos; +1.185/−35.
- O HEAD tem workflow CI concluído com sucesso (run 41665270882): Rust quality/unit regression, Scheduler deterministic stress, Windows x64 D3D12/WASAPI/XAudio2 e Python lint/regression passaram.
- Esse CI demonstra que o HEAD antigo passou por aqueles jobs; **não** torna a branch compatível com a main atual, não substitui CI sobre um candidato reimplementado e não valida semântica completa.

Manter a PR sem alterações nesta etapa. Preservar seu histórico como referência, sem duplicar trabalho.

### 2.4 Oráculos e corpus existentes

- O projeto registra o manual ZiLOG **UM008011-0816 (2016)** como fonte primária para arquitetura, instruções, sinais e diagramas de temporização.
- Já existe manifesto/checksums e auditoria exploratória do corpus [SingleStepTests/Z80](https://github.com/SingleStepTests/z80), fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`.
- `docs/z80-gap-resolution-plan.md` registra auditoria histórica de 11 arquivos/11.000 vetores: 5.401 aprovados, zero divergências e 5.599 excluídos pelo campo interno `p`. Esses números pertencem àquela execução/caminho legado e **não** são resultado do microsequenciador Rust na main atual.
- O próprio projeto observa que SingleStepTests deriva do núcleo Z80 do ares e não é captura direta de silício. É um oráculo diferencial útil, não autoridade física independente.
- `raxoft/z80test` é candidato para validação independente de instruções documentadas, flags, SCF/CCF e MEMPTR; execução end-to-end depende de ambiente compatível.
- MAME, Gearsystem, ares e outros emuladores são referências comparativas, nunca oráculo definitivo isolado.

## 3. Três frentes especializadas

A conexão atual não disponibiliza criação/execução de agentes autônomos independentes. As três frentes abaixo são **papéis de auditoria separados**, com entregáveis próprios e revisão cruzada; não representam três processos externos iniciados.

### Agente 1 — Semântica de opcodes e substituição IX/IY

**Missão:** provar quais instruções são afetadas por DD/FD, quais ignoram o prefixo e como mapear HL/H/L/(HL) para IX/IY/IXH/IXL/IYH/IYL/(IX/IY+d).

**Responsabilidades**
- Construir matriz por família semântica, opcode, prefixo, bytes consumidos, comprimento e T-states nominais.
- Separar instruções documentadas, aliases/comportamentos não documentados, prefixos ignorados e encodings não atribuídos.
- Cobrir LD IX/IY,nn; INC/DEC IX/IY; ADD IX/IY,rr; LD (nn),IX/IY; LD IX/IY,(nn); PUSH/POP; EX (SP),IX/IY; JP (IX/IY); LD SP,IX/IY.
- Cobrir IXH/IXL/IYH/IYL em LD, INC/DEC e ALU, incluindo casos em que H/L continuam sendo os registradores normais quando o outro operando é (IX/IY+d).
- Cobrir (IX/IY+d): LD r,(IX/IY+d), LD (IX/IY+d),r, LD (IX/IY+d),n, ALU A,(IX/IY+d), INC/DEC (IX/IY+d).
- Cobrir DDCB/FDCB e separar BIT (somente leitura) de operações read-modify-write.
- Provar fallback de prefixo ignorado sem mascarar substituição indexada ausente.

**Entregáveis:** matriz semântica determinística; vetores discriminantes; divergências entre executor atômico, sequenciador e corpus externo. Nenhuma alteração de runtime antes do gate E1.

### Agente 2 — Estado arquitetural, flags e interrupções

**Missão:** verificar efeitos observáveis e latches ocultos que podem ser alterados por prefixos/instruções indexadas.

**Responsabilidades**
- Flags S/Z/Y/H/X/PV/N/C e política para comportamentos não documentados.
- WZ/MEMPTR e Q/previous-Q, sem inferir atualizações por analogia com HL.
- Registrador R, preservação do bit 7 e fetches M1 em cadeias DD/FD/CB.
- PC, SP, wraparound, bytes consumidos, efeitos em memória e I/O.
- INT IM0/IM1/IM2, NMI, EI delay e HALT nos limites de prefixos e instruções.
- Snapshot/restore de todos os latches necessários para retomar uma instrução indexada em qualquer safe point alcançável.
- Política de variante: separar Zilog NMOS, CMOS e clones quando houver diferenças documentadas ou evidência conflitante.

**Entregáveis:** matriz de estado inicial/final; tabela de atualização WZ/Q/R/PC/flags com fonte e confiança; casos de interrupção; matriz snapshot/replay e fuzz do codec; hipóteses não confirmadas explicitadas.

### Agente 3 — Temporização, barramento e integração

**Missão:** provar ordem dos ciclos, operações de barramento e retomada temporal, sem confundir T-states nominais com precisão cycle-accurate.

**Responsabilidades**
- Definir trace determinístico com índice T-state/M-cycle, tipo de ciclo, M1, endereço, dado, operação memória/I/O, acknowledge, WAIT e resultado.
- Validar ordem dos fetches DD/FD, CB, displacement e subopcode com manual e vetores adequados.
- Medir T-states nominais e separar ciclos internos de transações externas.
- Determinar por evidência quando WAIT pode ser amostrado; não assumir regra universal de “WAIT nunca em ciclo interno” sem conferir os diagramas aplicáveis.
- Verificar R/refresh e contabilização de cada M1.
- Garantir que restore reproduza requests, respostas, efeitos e tempos.
- Manter separada a integração a `SmsTemporalBus`, `MachineScheduler`, VDP, PSG, IRQ e portas.

**Entregáveis:** especificação de trace e fixtures; testes de ordem/WAIT; relatório de T-states versus tempos observados; plano de integração via `BusTransaction`/`BusOperationResult`; evidência para eventual `TEMPORAL_PASS`.

## 4. Coordenação

- Cada frente produz relatório separado; a coordenação central reconcilia conflitos antes de qualquer mudança de runtime.
- Agente 1 classifica semântica; Agente 2 valida efeitos arquiteturais; Agente 3 valida ordem e temporalidade. Uma conclusão só é aceita quando as dimensões aplicáveis concordam.
- `Z80::step()` é comparador adicional, nunca oráculo único: ambos os executores podem compartilhar o mesmo bug.
- Não duplicar inventários. Estender `docs/z80-opcode-matrix.md`, `docs/z80-coverage.md`, `docs/architecture/z80-coverage-gap-model.md` e o harness existente, mantendo separados executor atômico e microsequenciador.
- Não impor limite arquitetural arbitrário de prefixos repetidos. O runtime não deve rejeitar cadeia válida por limite inventado.
- Não promover tempos, flags X/Y, Q, WZ, interrupções ou WAIT de análises comunitárias a requisitos definitivos sem fonte, variante e teste discriminante.
- Não elevar IMPLEMENTED a QUICK_PASS/SHORT_PASS/LONG_PASS/TEMPORAL_PASS sem evidência específica registrada.

## 5. Fases e gates

### E0 — Baseline congelada (sem runtime)

- [x] SHA inicial E0 e schema 25 registrados.
- [x] SHA atual após E2 e schema 26 registrados.
- [x] Estado/base/head da PR #447 registrados.
- [x] CI do HEAD antigo #447 consultado; quatro jobs confirmados.
- [x] Inventário BASE #465 distinguido de cobertura semântica.
- [ ] Registrar workflow completo da main candidata quando houver SHA de PR.
- [ ] Consolidar lista exata de testes sequencer/atomic e caminhos ativos/legados do harness.

**Gate:** baseline versionada, sem suposições apresentadas como fatos.

### E1 — Matriz semântica e validação dos oráculos (primeira PR funcional)

- Estender a matriz existente para DD, FD, DDCB e FDCB, sem misturar executor atômico e microsequenciador.
- Cada célula registra: família/mnemonic, bytes, prefixo efetivo, classificação (documentada/alias/ignorada/não atribuída), reconhecimento em cada executor, semântica testada, flags/estado, T-states nominais, trace, snapshot/replay e fonte.
- Estados: `NOT_IMPLEMENTED`, `RECOGNIZED_ONLY`, `FUNCTIONAL_UNVERIFIED`, `DIFFERENTIAL_PASS`, `TEMPORAL_UNVERIFIED`, `TEMPORAL_PASS`, `KNOWN_DIVERGENCE`.
- Validar revisão, licença e hashes dos vetores antes de adicioná-los a qualquer gate.
- Reaproveitar o manifesto SingleStepTests; não baixar branch móvel `main` nem incorporar código externo sem análise de licença.
- Comparar estado completo, RAM e I/O quando suportados. Campos não suportados são reportados como não comparados, nunca omitidos.
- `Z80::step()` é comparador adicional, nunca oráculo único.
- Nenhuma mudança no sequencer/snapshot nesta fase.

**Gate E1:** 100% das células do escopo classificadas; cada estado funcional tem testes vinculados; divergências reproduzíveis e registradas.

### E2 — Modelo de prefixos e modo indexado

- [x] FSM integrada ao caminho OpcodeFetch com estados `None/Cb/Ed/Index(Ix|Iy)`.
- [x] DD/FD repetidos e alternados aplicam last-prefix-wins; ED cancela o modo indexado.
- [x] Classificador distingue `UsesIndex`, `Ignored` e `IndexedCb`; a execução geral de opcodes ignorados continua reservada a E6.
- [x] Sem limite artificial de prefixos; cadeia de 257 prefixos validada.
- [x] Snapshot contém o latch, valida fase/opcode e faz round-trip com request M1 pendente.
- [x] Schema incrementado uma única vez de 25 para 26; formato antigo não é reinterpretado.
- [x] Caminho NOP sem prefixo preserva 4 T-states.
- [x] PR #468 integrada na main em `abd154332b1dabfb68874f2b80f306da7e060552`.

**Gate E2:** PASS para FSM, seleção de prefixo, classificação e snapshot. CI completo do SHA candidato: [run 41721337030](https://github.com/leonardo201800478/sega-emulator/actions/runs/41721337030). Isso não declara FUNCTIONAL_PASS das instruções DD/FD nem TEMPORAL_PASS.

**Limites mantidos após E2:** famílias indexadas remanescentes, validação diferencial, traces por T-state e integração temporal. E4A/E4B/E4C, E5 e E6 estão implementadas nos escopos registrados; E6 cobre fallback BASE/ED sem refetch e preservação de M-cycles/snapshots.

### E3 — Operações 16-bit e substituição HL→IX/IY

**E3A1 — implementado e validado:**
- [x] LD IX/IY,nn.
- [x] INC/DEC IX/IY.
- [x] ADD IX/IY,BC/DE/IX|IY/SP.
- [x] JP (IX/IY) e LD SP,IX/IY.
- [x] Preservação do latch DD/FD em operand fetches e ciclos internos.
- [x] M-cycle index acumulado pelos prefixos repetidos/alternados.
- [x] WAIT em leituras imediatas e snapshot/restore nas fases de operandos e ciclos internos.
- [x] 10 testes direcionados E3A1; flags, WZ, wrap e tempos nominais cobertos.
- [x] CI completo no SHA candidato: [run 41723613218](https://github.com/leonardo201800478/sega-emulator/actions/runs/41723613218).

**E3A2 — implementado e integrado na main pela PR #470 (`5b61b918`):**
- [x] `LD (nn),IX/IY` e `LD IX/IY,(nn)`, incluindo ordem little-endian, WZ=nn+1 e wrap em FFFF→0000.
- [x] `PUSH IX/IY` e `POP IX/IY`, com decrementos/incrementos de SP nos pontos de barramento corretos.
- [x] `EX (SP),IX/IY`, com leituras low/high, escritas low/high, SP preservado e commit após os T-states internos finais.
- [x] WAIT em operações de leitura/escrita e snapshot/restore nos estados intermediários de todas as três famílias.
- [x] Prefixos DD/FD repetidos e alternados, last-prefix-wins e M-cycle index acumulado.
- [x] Wrap de nn, SP e SP+1; flags preservadas; PC e WZ cobertos.
- [x] 11 testes direcionados E3A2a/b/c.
- [x] CI completo do candidato antes do merge: [run 41727716404](https://github.com/leonardo201800478/sega-emulator/actions/runs/41727716404), 4/4 workflows PASS; PR #470 integrada na main `5b61b918`.

**Limite:** os resultados são QUICK_PASS/SHORT_PASS do microsequenciador para estas famílias; não representam oráculo físico independente, trace por T-state completo, ZEXDOC/ZEXALL ou TEMPORAL_PASS.

**Gate E3:** E3A1 + E3A2 implementadas e SHORT_PASS no candidato PR #470. E3 está funcionalmente fechada no escopo das operações de 16 bits IX/IY; validação diferencial independente, trace por T-state e integração temporal continuam pendentes. Não declarar TEMPORAL_PASS.

### E4 — Instruções indexadas com displacement

**E4A — LD indexado/displacement implementado e integrado na main pela PR #471 (`e9a3eff6`):**
- [x] `LD r,(IX+d)` / `LD r,(IY+d)`.
- [x] `LD (IX+d),r` / `LD (IY+d),r`.
- [x] `LD (IX+d),n` / `LD (IY+d),n`.
- [x] Displacement `i8` com wrap de 16 bits; PC e ordem dos operandos verificados.
- [x] Exceção H/L convencional coberta para load e store; flags preservadas.
- [x] Sequência nominal 5 M-cycles / 19 T-states (`4,4,3,5,3`) conforme ZiLOG UM008011-0816, com WAIT adicional.
- [x] Snapshot/restore nas fases de displacement, ciclos internos, immediate e transação de memória.
- [x] Prefixos DD/FD repetidos e alternados; last-prefix-wins; testes de borda `-128`, `+127`, zero e wrap de PC/endereço.
- [x] CI completo no candidato `88b3c6fa`: [run 41734982519](https://github.com/leonardo201800478/sega-emulator/actions/runs/41734982519), Rust/Python/Scheduler stress/Windows x64 — 4/4 PASS.

**Estado E4A:** IMPLEMENTED / QUICK_PASS / SHORT_PASS para o escopo de LD indexado no microsequenciador. Não declarar LONG_PASS, diferencial externo, ZEXDOC/ZEXALL ou TEMPORAL_PASS.

**E4B — ALU A,(IX/IY+d), implementado no candidato PR #472:**
- [x] ADD, ADC, SUB, SBC, AND, XOR, OR e CP.
- [x] Flags S/Z/H/PV/N/C e X/Y; vetores fixos para overflow, half-carry/borrow, carry/borrow-in, paridade e X/Y.
- [x] CP preserva A e copia X/Y do operando; as demais operações usam a semântica da ALU compartilhada.
- [x] Reutiliza `MemoryReadAlu`, sem duplicar aritmética ou flags.
- [x] WAIT em prefix M1, opcode M1, displacement e leitura efetiva; snapshot/restore nos safe points, inclusive displacement pendente.
- [x] IX/IY, prefixos DD/FD repetidos/alternados e last-prefix-wins.
- [x] CI completo no candidato E4B: [run 41738004725](https://github.com/leonardo201800478/sega-emulator/actions/runs/41738004725), 4/4 jobs PASS.

**Estado E4B:** IMPLEMENTED / QUICK_PASS / SHORT_PASS para as oito operações ALU indexadas. Não declarar LONG_PASS, diferencial externo, ZEXDOC/ZEXALL ou TEMPORAL_PASS.

**E4C — INC/DEC (IX/IY+d), implementado e integrado pela PR #475:**
- [x] INC e DEC com leitura, ciclo interno e escrita no caminho compartilhado `MemoryReadIncDec → MemoryIncDecInternal → MemoryWrite`.
- [x] Flags S/Z/H/PV/N atualizadas e C preservado; WAIT e snapshot nos safe points cobertos.
- [x] IX/IY, displacement zero/-128/+127, wrap, prefixos repetidos/alternados e last-prefix-wins.
- [x] Sequência ZiLOG 6 M-cycles / 23 T-states (`4,4,3,5,4,3`).
- [x] 5 testes E4C e CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS; merge SHA `f89a52346e91e88e82ab37a4afcfffff2d793407`.

**Estado E4C:** IMPLEMENTED / QUICK_PASS / SHORT_PASS no escopo testado. LONG_PASS, validação diferencial externa, ZEXDOC/ZEXALL e TEMPORAL_PASS permanecem NOT RUN.

**Gate E4:** E4A + E4B + E4C implementadas e aprovadas nos critérios dirigidos de cada fatia. A validação independente e temporal permanece como trabalho posterior; não confundir gate de implementação E4 com conformidade integral do Z80.

### E5 — Prefixos DDCB/FDCB — integrada pela PR #485

- [x] Fases serializáveis para displacement, opcode final non-M1, leitura efetiva e write-back opcional no microsequenciador compartilhado.
- [x] DD/FD e CB são M1; displacement é MemoryRead/3T; opcode final é MemoryRead non-M1/5T; leitura efetiva é MemoryRead/4T; escrita RMW é MemoryWrite/3T.
- [x] BIT é read-only: 5 M-cycles / 20T (`4,4,3,5,4`); rotações/shifts, RES e SET fazem RMW: 6 M-cycles / 23T (`4,4,3,5,4,3`).
- [x] Campo z copia o resultado para B/C/D/E/H/L/A nos grupos x=0,2,3; H/L são os registradores convencionais.
- [x] R conta apenas fetches M1; WZ recebe o endereço efetivo; WAIT e snapshot/restore são suportados nas transações pendentes.
- [x] Schema 28 e suíte `indexed_cb_e5.rs` cobrindo 256 subopcodes por índice, IX/IY, prefixos repetidos/alternados, wrap, BIT sem escrita, destinos e WAIT.
- [x] QUICK_PASS / SHORT_PASS no escopo implementado; `cargo fmt`, Clippy e `cargo test --workspace --locked` PASS no CI #4245, HEAD `2cc6105a0c8c3589994870a6a67ceffb1e8c02f8`; suíte E5 8/8 e suíte unitária Z80 216/216.
- [x] SMS/SG-1000 targeted tests PASS; Python pytest/Ruff command steps PASS; Windows D3D12/video compile e testes PASS.
- [x] Windows WASAPI/XAudio2 compile e audio-output tests PASS; CI #4245 concluído com Rust, Python e Windows PASS; Scheduler stress SKIPPED corretamente por não haver alteração no Scheduler.
- Limite ZiLOG: UM008011-0816 classifica S e P/V de BIT indexado como desconhecidos. O modelo do executor atômico é mantido, mas esses bits não estão comprovados contra hardware independente.

**Gate E5:** 256 subopcodes por índice, ordem de barramento e M/T-cycles conferidos, BIT sem write-back e safe points serializáveis. PR #485 integrada; CI #4245 SUCCESS no HEAD `2cc6105a0c8c3589994870a6a67ceffb1e8c02f8`; merge SHA `f4130104fa971289507c0b15ab2d8eabd1dd5b00`.

### E6 — Resolução geral de prefixos DD/FD/ED/CB

**E6.1 — Fallback BASE para opcodes já suportados: IMPLEMENTED.**
- Encaminhar `Ignored` ao dispatch BASE sem refetch; preservar o custo do M1 DD/FD já consumido.
- Preservar PC, R, M1, WAIT e snapshots dos fetches pendentes.
- Demonstrar last-prefix-wins em cadeias repetidas/alternadas e ausência de substituição de IX/IY/H/L nas instruções BASE ignoradas.
- Reutilizar o decoder ED para ED após DD/FD; preservar a rota DDCB/FDCB E5.
- Não adicionar handlers BASE ausentes: `UnsupportedOpcode` continua explícito para lacunas do decoder BASE.
- Testar instruções BASE/ED já suportadas em fases de fetch, operandos imediatos, leituras/escritas, ciclos internos, WAIT e snapshots.

**E6.2 — Fallback com operandos e fases suspensas: IMPLEMENTED.**
- Estender a resolução para as instruções BASE/ED já suportadas que possuem leituras, escritas e ciclos internos.
- Ajustar invariantes de snapshot e contagem M-cycle para prefixos ignorados, sem alterar semântica BASE.

**E6.3 — Fechamento combinatório: IMPLEMENTED.**
- Cobrir DD, FD, DD DD, FD FD, DD FD, FD DD, DD/FD+ED, DD/FD+CB e cadeias longas.
- Verificar bytes consumidos, PC, R, M1/non-M1, flags, WZ, M-cycles, T-states e ausência de fetch duplicado.
- Não impor limite artificial de prefixos repetidos.

**Gate E6: PASS no escopo declarado.** OpCodes `UsesIndex`, `Ignored` e `IndexedCb` seguem caminhos explícitos; fallback BASE não relê opcode; cadeias repetidas/alternadas e snapshots preservam prefixos já consumidos.

**Cobertura E6 validada:**
- DD/FD seguido de instrução BASE que não usa HL/IX/IY, incluindo EX DE,HL.
- DD/FD repetidos e alternados, last-prefix-wins e cadeia de 257 prefixos.
- DD/FD seguido de ED, cancelando seleção IX/IY e preservando M-cycles em fases ED retomáveis.
- DD/FD seguido de CB, mantendo a rota DDCB/FDCB implementada na E5.
- BASE imediato, ALU imediato, branch relativo, memória read/write e ED absolute transfer com snapshots/WAIT.
- PC, R, bytes consumidos, M1/non-M1, M-cycles, T-states e ausência de fetch duplicado.

**Gate E6: PASS** no escopo declarado; instruções BASE ausentes permanecem `UnsupportedOpcode` e são assunto da auditoria E7.

### E7 — Fechamento semântico do Z80

Auditar sistematicamente BASE, CB, ED, DD, FD, DDCB e FDCB. Fechar flags documentadas e não documentadas, Q/WZ/MEMPTR, R/I, HALT, INT/NMI, IM 0/1/2, EI delay, RETI/RETN, instruções de bloco, estados alternativos e condições de borda. Validar também snapshots e rejeição de estados inconsistentes em cada safe point.

**Gate E7:** matriz por instrução/opcode com semântica, bytes, estado, flags, barramento, M-cycles, T-states, snapshot e nível de evidência; nenhuma família marcada completa apenas por existir handler.

### E8 — Validação formal e diferencial

Executar ZEXDOC, ZEXALL e vetores independentes compatíveis com a revisão de CPU-alvo. Comparar registradores, flags, PC, SP, WZ, memória, I/O e tempos contra oráculos independentes. SingleStepTests e executores internos são referências diferenciais úteis, mas não capturas diretas de silício.

**Gate E8:** resultados reproduzíveis vinculados ao SHA; divergências classificadas por instrução e variante; nenhum resultado esperado adaptado ao comportamento atual do emulador.

### E9 — Validação temporal e integração

Demonstrar execução determinística por M-cycle/T-state, WAIT, interrupções, HALT, suspensão/retomada e snapshots. Integrar o mesmo Z80 compartilhado aos barramentos SMS e SG-1000, sem duplicar CPU ou deslocar timing para Python. Validar regressões BASE, Scheduler stress e invariância entre execuções equivalentes.

**Gate E9:** traces temporais independentes e integração sistêmica comprovada. Somente então conceder `TEMPORAL_PASS`.

## 6. Critérios transversais

**Semântica:** estado completo (registradores principais/alternativos, IX/IY, I/R, IFF, IM, flags, PC/SP, WZ/Q e latches modelados), bytes consumidos, wraparound, memória e I/O. Separar instruções documentadas de comportamentos não documentados/variantes.

**Temporalidade:** M-cycles/T-states, M1, refresh, ordem de transações, endereço/dados, WAIT e acknowledge. T-states nominais não provam precisão cycle-accurate. Sem trace validado, não conceder `TEMPORAL_PASS`.

**Snapshot:** estado de execução e transação pendente serializados deterministicamente; restore reproduz o mesmo trace. Schema versionado; não reutilizar v18/v19 da PR antiga.

**CI obrigatória no SHA candidato:**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```
Além disso, Scheduler deterministic stress e Windows x64 D3D12/WASAPI/XAudio2 quando workflows aplicáveis estiverem configurados. Nenhum merge com job obrigatório pendente, falho ou ignorado.

## 7. Riscos e decisões abertas

1. Variante-alvo: confirmar revisão de Z80 usada como referência do SMS e separar NMOS/CMOS/clones.
2. Q e WZ/MEMPTR: definir evidência/política por família; não extrapolar regras comunitárias.
3. Interrupções entre prefixos: validar INT e NMI separadamente, incluindo IM0/acknowledge.
4. WAIT: derivar amostragem e ciclos elegíveis dos diagramas; não assumir regra universal.
5. DDCB/FDCB: validar sequência de barramento e R com fonte técnica e vetores.
6. Oráculos: SingleStepTests deriva de um emulador; raxoft/z80test e hardware/documentação devem complementar.
7. Schema: baseline inicial 25; E2 avançou para 26; E4A para 27; E5 avançou para schema 28 para serializar fases DDCB/FDCB e requests pendentes. Nunca importar schema da PR #447.
8. Integração SMS: separada da conclusão funcional do conjunto de instruções.

## 8. Próxima ação

E6 foi integrada pela PR #486. SHA atual da `main`: `24a32e03e10c8e82022172faee69ec0c046f8cb0`. CI #41759035927 (PR) e #41759330209 (push main) concluídos com SUCCESS para Rust, Python e Windows x64. A próxima etapa é E7 — fechamento semântico integral do Z80.

**Estado atual DD/FD:** E2 FSM/snapshot, E3A1/E3A2/E4A/E4B/E4C, E5 e E6 IMPLEMENTED / QUICK_PASS / SHORT_PASS nos escopos validados. E6 integrada pela PR #486; `main` SHA `24a32e03e10c8e82022172faee69ec0c046f8cb0`; CI #41759035927 e #41759330209 Rust/Python/Windows PASS, Scheduler stress SKIPPED. Próxima etapa: E7 fechamento semântico; E8 formal/diferencial e E9 integração temporal permanecem pendentes. Não há LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS global.
