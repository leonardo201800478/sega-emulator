# Estratégia de conclusão DD/FD do Z80

**Status:** plano de execução para a próxima fatia de trabalho  
**Data da auditoria:** 2026-11-03  
**Baseline inicial E0:** `e4604f3152c09e8d16c37f94f94a4b344db33b62`  
**Baseline após E1 / PR #467:** `aa4a4fa7661c28d3b2e21ac6d6eb4379fd297f65`  
**Baseline após E2 / PR #468:** `abd154332b1dabfb68874f2b80f306da7e060552`  
**Baseline atual após E3A1 / PR #469:** `ee9d39fc017b53493cc48d1343a7ca4b77581d3b`  
**PR de referência:** [#447](https://github.com/leonardo201800478/sega-emulator/pull/447) — manter aberta, sem merge  
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

**A PR #467 concluiu a primeira fatia documental/testes E1 sem alterar runtime; foi integrada na main após todos os gates obrigatórios passarem.** A arquitetura permanece centralizada nos crates compartilhados. Não criar Z80 alternativo ou sequenciador específico do SMS.

## 2. Baseline E0 — fatos confirmados

### 2.1 Main e schema

| Item | Estado confirmado |
|---|---|
| SHA da main na auditoria inicial | `e4604f3152c09e8d16c37f94f94a4b344db33b62` |
| SHA da main após E1 (#467) | `aa4a4fa7661c28d3b2e21ac6d6eb4379fd297f65` |
| SHA da main após E2 (#468) | `abd154332b1dabfb68874f2b80f306da7e060552` |
| SHA da main após E3A1 (#469) | `ee9d39fc017b53493cc48d1343a7ca4b77581d3b` |
| Schema inicial E0 | `Z80_MICRO_SEQUENCER_STATE_VERSION = 25` |
| Schema atual após E2 | `Z80_MICRO_SEQUENCER_STATE_VERSION = 26` |
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
- [x] Classificador distingue `UsesIndex`, `Ignored` e `IndexedCb`; a execução de opcodes ignorados continua reservada a E4.
- [x] Sem limite artificial de prefixos; cadeia de 257 prefixos validada.
- [x] Snapshot contém o latch, valida fase/opcode e faz round-trip com request M1 pendente.
- [x] Schema incrementado uma única vez de 25 para 26; formato antigo não é reinterpretado.
- [x] Caminho NOP sem prefixo preserva 4 T-states.
- [x] PR #468 integrada na main em `abd154332b1dabfb68874f2b80f306da7e060552`.

**Gate E2:** PASS para FSM, seleção de prefixo, classificação e snapshot. CI completo do SHA candidato: [run 41721337030](https://github.com/leonardo201800478/sega-emulator/actions/runs/41721337030). Isso não declara FUNCTIONAL_PASS das instruções DD/FD nem TEMPORAL_PASS.

**Limites mantidos:** execução de instruções indexadas, semântica de prefixos ignorados, displacement, DDCB/FDCB, traces por T-state e integração temporal permanecem nas etapas E3-E9.

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

**E3A2 — pendente:** LD (nn),IX/IY; LD IX/IY,(nn); PUSH/POP IX/IY; EX (SP),IX/IY.

**Gate E3:** E3A1 QUICK_PASS/SHORT_PASS; E3 integral ainda não concluído. Não declara oráculo independente nem TEMPORAL_PASS; os traces por T-state e commit semântico temporal continuam pendentes.

### E4 — Prefixos não afetados

Cobrir instruções que ignoram DD/FD e preservar custo dos prefixos. Distinguir instruções HL/H/L/(HL) afetadas das que não usam esses recursos. Validar EX DE,HL, EXX, ED e demais famílias.

**Gate E4:** nenhum opcode indexado cai silenciosamente no caminho base; cada fallback classificado e testado.

### E5 — IXH/IXL/IYH/IYL

Implementar LD r,r', LD r,n, INC/DEC e ALU para registradores de metade. Tratar separadamente casos com (IX/IY+d), nos quais H/L pode referir-se aos registradores normais. Usar tabela independente para flags; helper ALU do projeto não é oráculo independente.

**Gate E5:** vetores de borda e varreduras exaustivas aplicáveis; diferenças de variante documentadas.

### E6 — Endereçamento indexado com displacement

Ler displacement como byte assinado e calcular EA com wrap de 16 bits. Implementar LD, ALU, INC/DEC e operações de memória indexadas. Verificar EA, WZ/MEMPTR e ordem de leituras/escritas. Derivar ciclos internos e WAIT dos diagramas/traces, sem premissas não verificadas.

**Gate E6:** displacement 0x00–0xFF, bordas EA 0x0000/0xFFFF e modos de acesso relevantes.

### E7 — DDCB/FDCB

Implementar DD/FD-CB, displacement e subopcode como estados distintos. Confirmar M1/non-M1, R, ordem do barramento e T-states por fontes; valores citados com frequência (23T RMW e 20T BIT) são hipóteses de teste até confirmação. BIT é read-only; rotações/shifts/RES/SET exigem read-modify-write. Validar cópia do resultado para registrador conforme campo z e flags X/Y conforme política de variante.

**Gate E7:** 256 subopcodes por prefixo; testes de memória/registradores, estado final, flags, R, T-states e trace.

### E8 — Snapshot, interrupções e replay

Snapshot em cada safe point alcançável, inclusive transação pendente e WAIT quando aplicável. Restore reproduz estado, trace, requests, respostas, escritas e resultado. Injetar INT/NMI em fronteiras relevantes; aceitar interrupção somente nos pontos confirmados pela documentação/testes da variante. Fuzz do codec rejeita estados inconsistentes sem panic. Snapshot integral SMS é gate separado.

**Gate E8:** replay determinístico e fuzz sem panic, com cobertura de cada fase/latch.

### E9 — Fechamento e integração

Fechar cada célula com evidência; executar testes formais disponíveis (ZEXDOC/ZEXALL quando runner compatível); rodar todos os gates CI do SHA candidato. Integrar o Z80 ao `SmsTemporalBus` via contratos compartilhados e depois VDP/PSG/Scheduler/IRQ/ports em tarefas separadas. `TEMPORAL_PASS` exige traces temporais independentes e integração demonstrada.

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
7. Schema: baseline inicial 25; E2 avançou para 26 com um latch de prefixo explícito. Próximas alterações devem considerar snapshots pendentes; nunca importar schema da PR #447.
8. Integração SMS: separada da conclusão funcional do conjunto de instruções.

## 8. Próxima ação

Prosseguir para E3A2: transferências de/para memória absoluta, PUSH/POP IX/IY e EX (SP),IX/IY. Preservar a FSM E2 e os contratos de barramento; acrescentar fases retomáveis, snapshot, WAIT e testes por família.

**Estado atual DD/FD:** E2 aprovada na dimensão de FSM/snapshot; E3A1 implementada e QUICK_PASS/SHORT_PASS; E3A2, E4-E9 pendentes. DDCB/FDCB ainda sem displacement/subopcode no microsequenciador; sem LONG_PASS ou TEMPORAL_PASS global.
