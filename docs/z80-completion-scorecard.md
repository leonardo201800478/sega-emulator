# Meta de conclusão do Z80 — cobertura, validação e critérios de aceite

**Data da revisão:** 2026-11-04  
**Baseline de código:** `ea327025b312feb53f1e3e5326b5a5e7c40e72f1` (E7.2D integrada pela PR #500). **HEAD documental da main após PR #501:** `a324361574a45a80f8ae3aa70868630250e55d30`.  
**Evidência E4C:** HEAD validado `8f04b44e6c6ba85920a7438b74c9876586d5736c`; CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS; PR #475 integrada no merge SHA `f89a52346e91e88e82ab37a4afcfffff2d793407`.  
**Schema do microsequenciador na baseline main:** `Z80_MICRO_SEQUENCER_STATE_VERSION = 31` em `native/z80_cpu/src/lib.rs`; o schema foi incrementado para serializar as fases retomáveis de E7.2C.  
**Status deste documento:** referência de planejamento e critérios de aceite. Não substitui a matriz semântica por opcode nem os relatórios de execução dos testes.

---

## 1. Objetivo

Este documento estabelece uma meta verificável para concluir o núcleo Z80 compartilhado do Sega Emulator. Deve ser usado como referência de consulta durante implementação, revisão de PR, auditoria, planejamento de sprints e avaliação do percentual de conclusão.

A conclusão exige quatro dimensões independentes:

1. **Modelagem estrutural/semântica:** a instrução está classificada e possui comportamento de execução coerente com o Z80 clássico.
2. **Modelagem temporal retomável:** o microsequenciador representa os M-cycles, T-states, acessos ao barramento, WAIT e pontos de suspensão/restauração necessários.
3. **Validação funcional:** os resultados arquiteturais foram verificados com testes dirigidos, limites, combinações e regressões.
4. **Validação independente e sistêmica:** comparação com oráculos independentes e integração temporal demonstrada no barramento/Scheduler e nas máquinas que compartilham o Z80.

Nenhuma dimensão pode ser inferida automaticamente de outra. Um opcode reconhecido não está necessariamente correto; um teste unitário verde não prova conformidade integral; T-states nominais não provam a ordem dos eventos de barramento.

## 2. Estimativa inicial — fotografia de 2026-11-03

Os percentuais abaixo são **estimativas de engenharia**, usadas para dimensionar o trabalho. Não são métricas calculadas por contagem automatizada e não devem ser apresentados como certificação.

| Dimensão | Estimativa inicial | Interpretação |
|---|---:|---|
| Executor atômico / cobertura estrutural | **80–90%** | Grande parte das famílias possui caminho no executor atômico; ainda há lacunas semânticas e casos não documentados a decidir. |
| Microsequenciador retomável | **45–55%** | As operações IX/IY progrediram, mas várias famílias BASE/CB/ED e grupos DD/FD ainda precisam de execução temporal retomável ou fechamento de cobertura. |
| Validação funcional abrangente | **20–30%** | Há testes direcionados e regressões, mas não existe comprovação abrangente e independente de todo o conjunto documentado. |
| Validação diferencial externa completa | **0% nesta baseline** | A matriz DD/FD registra NOT_RUN para a comparação diferencial externa daquelas famílias. A execução de uma suíte parcial não deve ser generalizada para o núcleo inteiro. |
| TEMPORAL_PASS global | **0% nesta baseline** | Não há comprovação sistêmica por traces independentes de T-state para todo o Z80 integrado ao barramento/Scheduler. |

**Número de referência para comunicação:** aproximadamente **50% da modelagem temporal retomável** e **25% da validação funcional abrangente**. A modelagem no executor atômico é estimada em aproximadamente **85%**.

### 2.1 Como esses percentuais devem ser usados

- Servem para acompanhar tendência e priorização, não para decidir merge isoladamente.
- Não somar ou tirar média entre dimensões diferentes.
- Não contar ED-NOP, prefixo ignorado ou retorno sem exceção como instrução semanticamente validada.
- Não promover uma família por concordância entre dois executores internos que compartilham helpers ou lógica.
- A partir da criação da matriz quantitativa automatizada (seção 4), substituir progressivamente essas estimativas por valores reproduzíveis, vinculados ao SHA, versão do corpus e comandos executados.
- Se não houver evidência suficiente, registrar **UNKNOWN / NOT_RUN**, nunca estimar como PASS.

## 3. Baseline técnica e estado conhecido

O código ativo da main está no SHA `d64d86b5bb16305d6265a1522d095b68b8f4954a`, após merge da PR #504. E7.1 e E7.2A–E7.2D seguem integradas pelas PRs #490, #495, #496, #498 e #500. E7.2E recebeu a fatia de trace/WAIT da PR #504 e está em ampliação pela PR #505, que valida restore/continuação dos 70 tipos de fase gerados pelo corpus suportado. CI #41830953731 no HEAD `a21506ac4a2d8b66df94dfacdd7c749295e6bb5d`: Rust, Python e Windows x64 PASS; Scheduler deterministic stress SKIPPED. E7 global permanece INTEGRATION_PENDING por estados inválidos abrangentes, WAIT por todas as famílias, HALT T4, BUSREQ, RETI/daisy-chain, IRQ real VDP e validação independente.

| Fase | Escopo | Estado na fotografia |
|---|---|---|
| E1–E2 | Inventário, semântica DD/FD, FSM, last-prefix-wins e snapshots | Integradas anteriormente; SHORT_PASS no escopo registrado |
| E3A1 | Operações 16-bit IX/IY | Integrada na main |
| E3A2 | Transferências absolutas, PUSH/POP e EX (SP),IX/IY | Integrada na main |
| E4A | LD r,(IX/IY+d), LD (IX/IY+d),r e LD (IX/IY+d),n | Integrada na main; IMPLEMENTED / SHORT_PASS |
| E4B | ADD/ADC/SUB/SBC/AND/XOR/OR/CP (IX/IY+d) | Integrada na main pela PR #472; IMPLEMENTED / QUICK_PASS / SHORT_PASS; CI 4/4 PASS |
| E4C | INC/DEC (IX/IY+d) | Integrada na main pela PR #475; IMPLEMENTED / QUICK_PASS / SHORT_PASS; 5 testes dirigidos; CI 4/4 PASS |
| E5 | DDCB/FDCB completo | IMPLEMENTED / QUICK_PASS / SHORT_PASS; PR #485 merged; CI #4245 3 jobs PASS, Scheduler SKIPPED |
| E6 | Fallback geral e resolução de prefixos DD/FD/ED/CB | IMPLEMENTED / QUICK_PASS / SHORT_PASS; PR #486 merged; CI #41759035927 e push main #41759330209 PASS; Scheduler SKIPPED |
| E7 | Fechamento semântico global BASE/CB/ED/DD/FD/DDCB/FDCB | E7.1–E7.2D integradas; E7.2E com incremento #504 integrado e matriz de restore/continuação das 70 fases em PR #505 (CI Rust/Python/Windows PASS no HEAD registrado); matriz abrangente de estados inválidos, WAIT por famílias, HALT T4, BUSREQ, RETI/daisy-chain, IRQ real VDP e validação independente ainda pendentes. INTEGRATION_PENDING |
| E8 | Validação formal e diferencial (ZEXDOC/ZEXALL e corpora independentes) | Pendente |
| E9 | Integração temporal sistêmica com bus, WAIT, Scheduler, SMS e SG-1000 | Pendente |

Fontes normativas e de evidência do projeto:
- [Matriz semântica DD/FD](z80-dd-fd-semantic-matrix.md).
- [Matriz de cobertura histórica e notas de validação](z80-coverage.md) — observar as notas que identificam material histórico Python arquivado; não tratar a matriz antiga como cobertura normativa do runtime Rust.
- [Estratégia de resolução DD/FD](z80-dd-fd-resolution-strategy.md).
- [Manual ZiLOG Z80 UM008011](https://www.zilog.com/docs/z80/um0080.pdf).
- [SingleStepTests/Z80](https://github.com/SingleStepTests/z80), revisão fixada no projeto: `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`.
- [raxoft/z80test](https://github.com/raxoft/z80test).

## 4. Métrica oficial a construir

A partir desta revisão, os percentuais oficiais deverão vir de um inventário executável versionado. O inventário deve conter uma linha por codificação/família e distinguir o comportamento arquitetural da evidência disponível.

### 4.1 Espaços de opcode

O inventário classificatório existente trabalha com sete espaços de 256 células (BASE, CB, ED, DD, FD, DD-CB e FD-CB), totalizando **1.792 células classificatórias**. Esse total é um universo de classificação, não o número de instruções documentadas independentes: DD/FD contêm prefixos ignorados, substituições, aliases e formas não documentadas; DDCB/FDCB compartilham famílias e campos de operação.

A matriz quantitativa deve identificar explicitamente:
- Instrução documentada do Z80 clássico.
- Alias documentado ou codificação redundante.
- Opcode não documentado, com decisão de suporte separada.
- ED-NOP / byte sem operação atribuída.
- Prefixo ignorado que executa a instrução BASE.
- Prefixo repetido e regra last-prefix-wins.
- Extensão de clone ou derivado, fora do alvo clássico.
- Codificação indefinida ou fora do escopo.

Não usar 1.792 como denominador de “instruções implementadas” sem normalização por família e classificação arquitetural.

### 4.2 Dimensões por célula

Cada célula ou caso normalizado deverá registrar, no mínimo:

| Campo | Valores / significado |
|---|---|
| `classification` | DOCUMENTED, ALIAS, UNDOCUMENTED, PREFIX_FALLBACK, ED_NOP, CLONE_EXTENSION, UNDEFINED |
| `atomic_status` | IMPLEMENTED, PARTIAL, MISSING, NOT_APPLICABLE |
| `micro_status` | IMPLEMENTED, PARTIAL, MISSING, NOT_APPLICABLE |
| `semantic_tests` | número de casos executados e aprovados, por categoria |
| `differential_status` | PASS, FAIL, PARTIAL, NOT_RUN, BLOCKED |
| `timing_status` | NOMINAL_ONLY, BUS_TRACE_PARTIAL, BUS_TRACE_PASS, NOT_RUN |
| `wait_snapshot_status` | PASS, PARTIAL, NOT_APPLICABLE, NOT_RUN |
| `integration_status` | PASS, PARTIAL, FAIL, NOT_RUN |
| `evidence` | caminhos de testes, logs, workflow IDs, corpus/oráculo e SHA |
| `known_gaps` | lacunas explícitas e reproduzíveis |

### 4.3 Fórmulas

Calcular e publicar percentuais separados:

- **Cobertura atômica:** células documentadas elegíveis com handler implementado ÷ total de células documentadas elegíveis.
- **Cobertura microsequenciada:** células documentadas elegíveis com execução retomável ÷ total de células documentadas elegíveis.
- **Validação funcional:** células documentadas elegíveis que passaram nos critérios semânticos mínimos ÷ total de células documentadas elegíveis.
- **Validação diferencial:** células documentadas elegíveis comparadas e aprovadas por oráculo externo ÷ total de células documentadas elegíveis.
- **Validação temporal:** casos temporais elegíveis aprovados em traces de barramento independentes ÷ total de casos temporais definidos.

Publicar também os números absolutos (aprovados / elegíveis), não apenas o percentual. O denominador deve permanecer versionado e qualquer alteração de classificação deve ter justificativa revisada.

Uma célula só conta como **funcionalmente validada** quando seus critérios aplicáveis de resultado, flags, registradores, memória, PC/SP, WZ/MEMPTR, R, fluxo de controle e efeitos colaterais foram exercitados. Uma célula só conta como **temporalmente validada** quando a ordem e a duração dos eventos relevantes de barramento foram comparadas com uma especificação/oráculo independente; retornar o total nominal de T-states não é suficiente.

## 5. Critérios de aceite por nível de evidência

Os níveis abaixo são cumulativos no escopo declarado. O estado deve ser atribuído por família e nunca globalmente por inferência.

| Nível | Critério mínimo |
|---|---|
| IMPLEMENTED | Caminho de execução implementado e classificado; não implica correção. |
| QUICK_PASS | Testes unitários direcionados e regressões imediatas aprovados; sem falhas conhecidas no escopo. |
| SHORT_PASS | Matriz de casos do escopo, bordas relevantes, flags, fluxo, WAIT/snapshot quando aplicável e CI obrigatório aprovados. |
| LONG_PASS | Suíte ampliada e prolongada, corpus completo definido para a família, stress e regressão sem divergências; registrar duração, total de casos e SHA. |
| DIFFERENTIAL_PASS | Comparação reproduzível com oráculo externo identificado e fixado, sem divergências não justificadas no escopo. |
| TEMPORAL_PASS | Traces de barramento por M-cycle/T-state, WAIT, interrupções e retomada validados independentemente no escopo declarado. |
| INTEGRATION_PASS | Integração no sistema-alvo validada sem regressão, com bus/Scheduler e testes de máquina. |

**Proibição:** não atribuir LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS a partir apenas do CI convencional, de testes sintéticos próprios ou de comparação entre caminhos internos que compartilham implementação.

## 6. Plano obrigatório para chegar a 100%

### E4C — INC/DEC (IX/IY+d) — concluída no microsequenciador

- [x] INC/DEC indexados com read-modify-write compartilhado (`MemoryReadIncDec → MemoryIncDecInternal → MemoryWrite`), sem ALU/RMW duplicado.
- [x] IX/IY; flags S/Z/H/PV/N e Carry preservado; displacement 0/-128/+127; wrap de endereço efetivo e PC.
- [x] WAIT nos fetches M1, displacement, leitura efetiva e escrita; snapshot/restore nos safe points intermediários.
- [x] Prefixos repetidos/alternados e last-prefix-wins; 23 T-states / 6 M-cycles (`4,4,3,5,4,3`) conforme ZiLOG UM008011-0816.
- [x] 5 testes dirigidos `indexed_incdec_e4c.rs` e CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS no HEAD `8f04b44e6c6ba85920a7438b74c9876586d5736c`.
- Estado: IMPLEMENTED / QUICK_PASS / SHORT_PASS para o escopo implementado. Não promover LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS sem evidência independente.

### E5 — DDCB/FDCB — implementação PR #485

- [x] Caminho microsequenciado para os 256 subopcodes DDCB e 256 FDCB, usando o executor atômico existente como referência funcional, sem duplicar a semântica CB.
- [x] DD/FD e CB como M1; displacement como leitura de 3T; opcode final como leitura non-M1 de 5T; leitura efetiva de 4T; escrita RMW de 3T.
- [x] BIT encerra após leitura, sem write-back: 5 M-cycles / 20 T-states (`4,4,3,5,4`).
- [x] Rotações/shifts, RES e SET em read-modify-write: 6 M-cycles / 23 T-states (`4,4,3,5,4,3`).
- [x] Cópia opcional para B/C/D/E/H/L/A nos grupos x=0,2,3; H/L são os registradores convencionais, não IXH/IXL/IYH/IYL.
- [x] R incrementa somente nos M1; WZ mantém o endereço efetivo; IX/IY, last-prefix-wins, displacement assinado e wrap.
- [x] WAIT em cada transação e snapshot/restore nas fases com request pendente; schema 28.
- [x] Adicionada suíte `indexed_cb_e5.rs`: 256 subopcodes por índice, BIT read-only, RMW, cópia z, flags modeladas, prefixos repetidos/alternados, displacement 0/-128/+127, wrap de endereço/PC e round-trip de snapshots.
- [x] `cargo fmt --all -- --check`: PASS no CI run #4245, HEAD `2cc6105a0c8c3589994870a6a67ceffb1e8c02f8`.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS no mesmo workflow.
- [x] `cargo test --workspace --locked`: PASS; 216 testes unitários do crate Z80 e 8 testes da suíte indexed-CB passaram, além das demais suítes do workspace.
- [x] Testes direcionados SMS e SG-1000: PASS.
- [x] `python -m pytest`, `ruff check` e `ruff format --check`: job Python PASS no run #4245.
- [x] Windows x64 D3D12/video compile e video-runtime tests: PASS.
- [x] Windows WASAPI/XAudio2 compile e audio-output tests: PASS.
- [x] CI aplicável: 3/3 jobs PASS; Scheduler deterministic stress SKIPPED corretamente por não haver alteração no Scheduler.
- Estado: IMPLEMENTED / QUICK_PASS / SHORT_PASS para o escopo E5 integrado. CI #4245 no HEAD `2cc6105a0c8c3589994870a6a67ceffb1e8c02f8`: Rust, Python e Windows PASS; Scheduler stress SKIPPED por não haver alteração no Scheduler. Merge SHA `f4130104fa971289507c0b15ab2d8eabd1dd5b00`.

- Limite de fonte: ZiLOG UM008011-0816 marca S e P/V de BIT indexado como desconhecidos; o candidato mantém o modelo já existente no executor atômico. Isso não comprova esses bits contra silício.
- Não promover LONG_PASS, DIFFERENTIAL_PASS, ZEXDOC/ZEXALL ou TEMPORAL_PASS sem evidência independente.

### E6 — Resolução geral de prefixos

**E6.1 — Fallback BASE DD/FD: IMPLEMENTED.**
- Encaminhar opcodes `Ignored` já obtidos ao dispatch BASE, sem novo fetch.
- Preservar PC, R, M1, T-states acumulados, WAIT e snapshots dos fetches pendentes; manter o latch Index como contexto de timing/serialização, sem aplicar substituição IX/IY a opcodes `Ignored`.
- Demonstrar que H/L e IX/IY não são substituídos em instruções BASE ignoradas.
- Reutilizar o decoder ED quando ED segue DD/FD; manter DDCB/FDCB na rota E5.
- Limite deste incremento: somente instruções BASE/ED já suportadas pelo sequenciador; `UnsupportedOpcode` continua explícito para lacunas do decoder BASE.
- Aceite cumprido: sequências simples/repetidas/alternadas, WAIT aditivo, round-trip de snapshots e ausência de fetch duplicado.

**E6.2 — Operandos, fases suspensas e snapshots BASE/ED: IMPLEMENTED.**
- Estados suspensos com operandos imediatos, ALU imediato, branch relativo e memória read/write aceitam snapshot/restore e WAIT.
- Prefixos DD/FD anteriores a ED não perdem M-cycles ao entrar em ED absolute transfer, ED block/IO ou fases internas; contadores são preservados em transições internas.
- O latch Index é mantido como contexto de timing/serialização, enquanto a classificação Ignored evita semântica IX/IY no opcode BASE.

**E6.3 — Fechamento combinatório: IMPLEMENTED.**

- Cobrir DD, FD, DD DD, FD FD, DD FD, FD DD, DD/FD+ED, DD/FD+CB e prefixos redundantes/ignorados.
- Garantir last-prefix-wins, incremento de R, PC, bytes consumidos, M1 e custo temporal.
- Prefixos ignorados devem executar a instrução BASE correta sem fetch duplicado ou substituição indevida de HL/H/L.
- Não impor limite artificial de prefixos repetidos.
- Aceite cumprido: matriz de combinações, stress de cadeias longas, snapshot/restore e regressões sem fetch duplicado.

**Evidência E6 — PR #486, HEAD final `de0166a54bd6613cd7d9379e4e03ca8e81e238eb`, merge SHA `24a32e03e10c8e82022172faee69ec0c046f8cb0`:**
- Auditoria técnica detalhada: [E6 — Resolução geral dos prefixos Z80](audits/z80-prefix-resolution-e6-2026-11-04.md).
- CI [#41759035927](https://github.com/leonardo201800478/sega-emulator/actions/runs/41759035927) no HEAD final: Rust PASS (fmt, Clippy, workspace tests); Python PASS (pytest, Ruff check, Ruff format); Windows x64 PASS (D3D12, WASAPI/XAudio2 e testes); Scheduler stress SKIPPED por não haver alteração no Scheduler.
- CI de push na `main`, [#41759330209](https://github.com/leonardo201800478/sega-emulator/actions/runs/41759330209), SHA `24a32e03e10c8e82022172faee69ec0c046f8cb0`: SUCCESS; Rust, Python e Windows PASS; Scheduler SKIPPED.
- `native/z80_cpu/tests/prefix_fallback_e6.rs`: 9/9 testes PASS, incluindo BASE register, immediate, ALU, relative branch, memória read/write, ED absolute transfer, WAIT e snapshots.
- `native/z80_cpu/tests/prefix_fsm.rs`: last-prefix-wins, repetição/alternância e cadeia de 257 prefixos sem limite artificial.
- Regressão Z80 existente: 216/216 testes unitários PASS; E5 indexed-CB permanece coberta por 8/8 testes.
- E6 não reivindica ZEXDOC/ZEXALL, LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS. `UnsupportedOpcode` continua explícito para instruções BASE que o executor ainda não implementa.

### E7 — Fechamento semântico de todo o Z80 clássico

Auditar BASE, CB, ED, DD, FD, DDCB e FDCB, incluindo:
- Todas as instruções documentadas e aliases pertinentes.
- Flags S/Z/H/PV/N/C e bits X/Y quando o alvo e a evidência definirem comportamento.
- PC, SP, IX/IY, pares alternativos, WZ/MEMPTR, R e demais estado arquitetural modelado.
- HALT, NMI, INT, IM 0/1/2, EI delay, DI, RETI/RETN e interação com instruções em andamento.
- Instruções de bloco ED, repetição, término antecipado, wrap e casos BC=0000.
- I/O, endereços de porta, WAIT e efeitos de barramento.
- Save state/snapshot em todos os estados suspensíveis e validação contra estados inválidos/corrompidos.
- Decisão documentada para opcodes não documentados e diferenças entre Z80 clássico e clones.

Aceite: nenhuma instrução documentada elegível sem classificação e critério de teste; lacunas não documentadas explicitamente declaradas.

#### E7.1 — HALT e ciclos M1 de retenção

- Integrado na main pela PR #490: HALT entra após M1 normal; PC avança uma vez. CI #41762232781 passou nos gates Rust/Python/Windows; Scheduler stress SKIPPED. CI pós-merge #41765421167 no SHA `b1b5ced5c3d6053f8d59c8f4d932f40518db1064`: Rust/Python/Windows PASS; Scheduler stress SKIPPED.
- Durante HALT sem interrupção elegível, cada chamada emite M1 no endereço atual de PC; o byte é ignorado, PC não avança e R incrementa preservando o bit 7.
- WAIT e snapshot/restore do M1 pendente são cobertos; schema permanece 28.
- Validador temporal SMS ajustado para contabilizar M1s anteriores já concluídos, permitindo snapshot no M1 subsequente a prefixo ED.
- Inventário BASE candidato: 252 opcodes implementados / 0 rejeitados explicitamente.
- Evidência e limites: [auditoria E7.1 — HALT](audits/z80-e7-halt-m1-2026-11-04.md).
- **Limite:** o acknowledge e o dispatch NMI/INT estão implementados nas fases E7.2A/B/C, mas a saída de HALT ainda não possui amostragem T4 cycle-accurate. Reset especial e validação independente continuam pendentes. Não atribuir SHORT_PASS integral a HALT.
- Auditoria de código E7 na baseline histórica `5768705658522deb46a0fb0556b51923ae8166b8`: [auditoria E7 — semântica ED, microsequenciador, snapshots e interrupções](audits/z80-e7-code-validation-2026-11-04.md). A revisão encontrou 26 testes ED positivos no microsequenciador, aprovados no CI #41775300143 (216 testes unitários totais no crate Z80). Esses testes incluem comparação com o executor atômico interno e validações de requests/WAIT/snapshots; não equivalem a oráculo externo nem a trace independente. E7 permanece INTEGRATION_PENDING por HALT T4, BUSREQ, RETI/daisy-chain, snapshots completos e validação temporal/diferencial independente; E7.2D foi integrada pela PR #500.

#### E7.2C — Dispatch retomável IM0/IM1/IM2 — PR #498 (integrada na main)

- IM0 reutiliza o decoder microsequenciado existente para o byte fornecido no acknowledge, sem incrementar PC/R novamente; bytes seguintes de prefixo e operandos vêm da memória. Cobertos NOP, JP nn e DD 21 nn.
- IM1 empilha PC high/low em requests distintos, define PC=0038h e retorna 13 T-states. WZ permanece inalterado, sem inferir semântica MEMPTR não documentada.
- IM2 empilha PC, lê low/high da tabela em `(I << 8) | vector`, define PC/WZ para o destino e retorna 19 T-states. O vetor não é mascarado; dispositivo conforme deve fornecer vetor par.
- Modo IM é capturado na fase de acknowledge concluído. Fases de dispatch, stack push e leituras IM2 são retomáveis; snapshot schema 31, tags 65–69.
- `SmsTemporalMachine::resume_instruction()` retoma o dispatch após acknowledge; testes SMS exercitam IM0 RST, IM1 e IM2 pelo Scheduler.
- Testes Z80: NOP/JP/prefixo IM0; IM1 13T, ordem high/low, WAITs e snapshots; IM2 19T, leitura low/high, snapshots e preservação do vetor bruto.
- Evidência detalhada: [auditoria E7.2C — IM0/IM1/IM2](audits/z80-e7-2c-interrupt-modes-2026-11-04.md).
- CI [#41792143207](https://github.com/leonardo201800478/sega-emulator/actions/runs/41792143207), SHA de código `9cb576e957fdb37529d3c8dfb5d54fb22c2c907f`: Rust fmt/Clippy/SMS/SG-1000/workspace PASS; Python pytest/Ruff PASS; Windows x64 D3D12/video e WASAPI/XAudio2/audio compile/tests PASS. Scheduler deterministic stress SKIPPED.
- CI pós-merge [#41792924571](https://github.com/leonardo201800478/sega-emulator/actions/runs/41792924571), main SHA `a756e51cf6c5245357831896de293cd2e9d3a859`: Rust quality/unit regression, Python lint/regression e Windows x64 D3D12/WASAPI/XAudio2 PASS; Scheduler deterministic stress SKIPPED.
- Estado E7.2C: IMPLEMENTED / QUICK_PASS / SHORT_PASS para o escopo e cenários testados; cenários direcionados SMS pelo Scheduler PASS. PR #498 integrada no merge SHA `a756e51cf6c5245357831896de293cd2e9d3a859`. E7 completo continua INTEGRATION_PENDING; não declarar LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS.

#### E7.2D — Arbitragem EI/DI e atraso de uma instrução — PR #500 (integrada na main)

- EI habilita IFF1/IFF2 e define `ei_delay = 1`; INT pendente só é aceito quando IFF1 está ativo e o atraso chegou a zero.
- O atraso é consumido na aposentadoria da instrução seguinte, não na própria EI; um segundo EI reinicia o atraso.
- DI limpa IFF1/IFF2 e cancela o atraso; a linha INT permanece pendente, mas mascarada.
- NMI tem prioridade sobre INT e é aceito durante o atraso. O serviço NMI não consome o atraso; no cenário testado, RETN como primeira instrução do handler o consome, restaurando IFF1 a partir de IFF2 e permitindo INT no boundary seguinte.
- Snapshot binário round-trip com `ei_delay = 1` antes da instrução seguinte.
- Testes: `ei_defers_pending_int_until_the_following_instruction_and_survives_snapshot`, `di_cancels_ei_delay_and_keeps_pending_int_masked`, `a_second_ei_restarts_the_one_instruction_delay`, `nmi_is_accepted_during_ei_delay_and_retn_consumes_the_delay`.
- CI [#41793815959](https://github.com/leonardo201800478/sega-emulator/actions/runs/41793815959), SHA `c6f0930f6820e760d8b0d60ae7770da9cb22be32`: Rust fmt/Clippy/SMS/SG-1000/workspace PASS; Python pytest/Ruff PASS; Windows x64 D3D12/video e WASAPI/XAudio2/audio compile/tests PASS. Scheduler deterministic stress SKIPPED.
- CI pós-merge [#41794506556](https://github.com/leonardo201800478/sega-emulator/actions/runs/41794506556), main SHA `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`: Rust quality/unit regression, Python lint/regression e Windows x64 D3D12/WASAPI/XAudio2 PASS; Scheduler deterministic stress SKIPPED.
- **Limite:** `EI;HALT;INT` não é declarado coberto; a amostragem NMI/INT no T4 dos ciclos HALT continua pendente. BUSREQ, RETI/daisy-chain e validação diferencial/temporal independente também permanecem fora deste incremento.
- Estado dos cenários EI/DI testados: IMPLEMENTED / QUICK_PASS / SHORT_PASS; PR #500 integrada no merge SHA `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`. E7 global continua INTEGRATION_PENDING.

#### E7.2E — Snapshots, restauração e equivalência contínua/retomada — PR #504 (incremento integrado; PR #502 supersedida)

- HEAD consultado: `69ea4299b3858787f0f91682f3ea823371cde3b0`; base original `ea327025b312feb53f1e3e5326b5a5e7c40e72f1`; 11 commits à frente e 0 atrás da base original. Após o merge documental #501, a comparação remota registra a branch #502 divergente, 11 commits à frente e 4 atrás da main atual.
- Alterações candidatas: seis arquivos; teste temporal SMS de snapshots para NMI, IM0/IM1/IM2, EI delay e HALT/M1; round-trip de 70 variantes da fase; rejeição de tag inválida; validação de schema.
- Auditoria estrutural no código-base: 70 variantes do enum, 70 writer arms, 70 reader arms e 70 restore-validator arms. É cobertura estrutural, não validação de todas as combinações de estado.
- O teste temporal compara o resultado terminal, DTO completo exportado e tempo absoluto entre execução contínua e execução com restore nos safe points exercitados. WAIT=1 é usado em NMI/IM0/IM1/IM2 e HALT; EI usa WAIT=0.
- **Limites encontrados na revisão:** a matriz das 70 variantes testa tag/payload isolados, não restauração/continuação válida de cada fase completa; estados inválidos por fase ainda não estão sistematicamente cobertos; WAIT zero/múltiplo e limites de transação não estão abrangidos em todas as famílias; o cenário IM0 não fixa explicitamente o T-state terminal esperado; o teste de tag inválida deriva o offset pela composição dos campos anteriores do wire format; a versão do schema fica no cabeçalho e o CRC cobre apenas o corpo, portanto alterar a versão não exige recalcular CRC.
- **Limite de formato:** `SmsTemporalMachineState` continua DTO em memória com `export_state()/restore_state()`; não existe evidência de codec binário versionado para save state completo da máquina SMS. O codec binário validado é o do estado CPU Z80.
- CI para o HEAD histórico #502 (`69ea4299b3858787f0f91682f3ea823371cde3b0`): **NOT RUN / nenhum workflow associado**. Esse SHA não valida a continuação atual.
- PR #504, HEAD `b78d3354119c3f9af475561ff15db7673e93fb06`, workflow #41825655861: Rust fmt/Clippy/workspace **PASS**; Python pytest/Ruff **PASS**; Windows x64 D3D12/vídeo e WASAPI/XAudio2/áudio **PASS**; Scheduler deterministic stress **SKIPPED**.
- Execução local nesta sessão: BLOCKED por falha de resolução DNS de `github.com`; os resultados acima são do GitHub Actions no SHA exato.
- Estado: **IN_PROGRESS / INTEGRATION_PENDING**. O CI convencional desta fatia passou, mas não atribuir LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS à E7.2E; a cobertura complementar permanece pendente.

**Próximos critérios E7.2E:** (1) CI completo no SHA exato; (2) snapshot/restore/continuação por fase e por combinação de estado válida; (3) rejeição de estados incoerentes, truncados, com CRC/tags/schema inválidos; (4) WAIT 0, 1 e múltiplos em fronteiras de bus/internal/M1; (5) comparação de trace, CPU, memória, scheduler e tempo entre contínuo e retomada; (6) documentar separadamente CPU codec e DTO SMS.


#### E7.2E.1 — Restore/continuação por fase completa — PR #505 (em validação)

- Corpus: 256 opcodes BASE com flags 0x00/0xFF e 256 bytes em CB, ED, DD, FD, DDCB e FDCB; casos de interrupção NMI e IM0/IM1/IM2.
- Em cada boundary microsequenciado gerado, o teste codifica o DTO completo, decodifica, restaura e continua; compara microstep terminal, estado completo, sequência de requests/responses de barramento e conjunto de fases.
- O teste exige que o corpus alcance as 70 variantes de Z80MicroSequencerPhase. Casos que retornam UnsupportedOpcode são excluídos explicitamente; não contam como cobertura semântica.
- Defeitos encontrados e corrigidos no validador: MemoryWrite para LD (HL),r (70h–75h/77h); fase interna de RET condicional após prefixo DD/FD ignorado; fases internas e escritas de stack de RST/CALL após DD/FD ignorado.
- CI #41830953731 no HEAD a21506ac4a2d8b66df94dfacdd7c749295e6bb5d: Rust fmt/Clippy/SMS/SG-1000/workspace PASS; Python pytest/Ruff PASS; Windows x64 D3D12/vídeo e WASAPI/XAudio2/áudio PASS; Scheduler stress SKIPPED.
- Limites restantes: matriz de estados inválidos por fase/request/payload/índices; WAIT 0/1/múltiplo por famílias e fronteiras; T-states terminais IM0; codec binário persistente do SMS completo. E7.2E e E7 global permanecem INTEGRATION_PENDING.

#### E7.3 — Amostragem de INT/NMI no T4 de HALT

Pendente. Definir a borda exata de amostragem e demonstrar a saída de HALT em cada combinação de NMI/INT/IFF/EI delay, com PC/R, acknowledge e M-cycles coerentes. O caso `EI;HALT;INT` permanece explicitamente não aprovado.

#### E7.4 — BUSREQ e arbitragem de barramento

Pendente. Modelar BUSREQ/WAIT/contensão e retomada sem perder request, fase M1, estado CPU ou causalidade do Scheduler. Não simular BUSREQ como atraso genérico.

#### E7.5 — RETI e daisy-chain

Pendente. Distinguir RETI de RETN e representar a notificação de fim de serviço para periféricos/daisy-chain; validar sequência de barramento e efeitos de IFF.

#### E7.6 — Fonte IRQ real do VDP SMS

Pendente. Conectar IRQ do VDP ao pino INT pelo contrato de máquina, incluindo status/acknowledge do dispositivo e nível de linha; não usar vetor fixo artificial como substituto da fonte real.

#### E7.7 — Auditoria semântica integral

Pendente. Fechar a matriz BASE/CB/ED/DD/FD/DDCB/FDCB, flags documentadas e indeterminadas, Q/WZ, estados alternativos, instruções de bloco, I/O, aliases e opcodes não documentados; vincular cada célula a testes e evidências. A auditoria histórica de 26 testes ED no microsequenciador é QUICK_PASS apenas para os casos examinados, não para ED integral.

### E8 — Validação formal e diferencial

- Inspecionar os runners e manifests existentes antes de criar infraestrutura duplicada.
- Integrar ZEXDOC e ZEXALL e corpora independentes pertinentes, incluindo SingleStepTests/Z80 na revisão fixada e raxoft/z80test quando compatível.
- Comparar estado completo: registradores, flags, PC/SP, WZ/MEMPTR quando definido, memória, I/O e fluxo de controle.
- Para testes temporais, comparar sequência de eventos, endereço, operação, duração, M-cycles, T-states e WAIT.
- Não adaptar valores esperados para coincidir com o emulador; divergências devem ser classificadas e resolvidas contra manual e referências independentes.
- Publicar relatório por família: total, pass, fail, divergências, exclusões justificadas, versão do oráculo, commit/corpus e comando de reprodução.

Aceite: 100% dos testes aplicáveis do corpus selecionado aprovados ou exclusões justificadas e revisadas; nenhuma divergência silenciosa.

### E9 — Integração temporal sistêmica

- Conectar o microsequenciador ao barramento transacional e ao Scheduler, sem mover lógica de timing para Python.
- Validar WAIT states, contenção quando aplicável, INT/NMI, HALT, DMA/periféricos relevantes, reset e snapshots.
- Testar invariância de execução em diferentes tamanhos de chunk e retomada de snapshots em safe points.
- Validar o mesmo núcleo compartilhado em SMS e SG-1000, sem duplicação de CPU.
- Comparar traces de instrução e de máquina com referências independentes e testes de software/hardware disponíveis.

Aceite: TEMPORAL_PASS por cenário e plataforma somente com evidências reproduzíveis. O selo global depende de todos os cenários obrigatórios aprovados.

## 7. Gates de engenharia e CI

Toda PR que alterar o Z80 deve:
- Ser isolada em branch própria e revisada via PR; não fazer commit direto em `main`.
- Incluir testes focados e atualizar a matriz semântica, a matriz quantitativa e a auditoria da fatia.
- Preservar os contratos do runtime Rust compartilhado; Python permanece GUI/tooling.
- Não introduzir JIT, callbacks por instrução, frame skip ou hacks de timing.
- Não duplicar lógica de CPU por console nem duplicar helpers de ALU.
- Atualizar schema de snapshot somente quando necessário, com compatibilidade/validação documentada.
- Registrar SHA, workflow, logs e resultados.

Comandos obrigatórios no SHA candidato:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Também executar os perfis de Scheduler e os jobs Windows x64 relevantes (D3D12/vídeo e WASAPI/XAudio2/áudio). Registrar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN. CI verde não substitui validação semântica/diferencial/temporal.

## 8. Definição objetiva de “Z80 concluído”

O Z80 só poderá ser declarado **IMPLEMENTED / FUNCTIONALLY VALIDATED** quando:
- Todas as instruções documentadas elegíveis do Z80 clássico estiverem classificadas e implementadas no caminho de execução adotado.
- O microsequenciador suportar as instruções que exigem execução retomável e os prefixos sem fallback incorreto.
- Flags, estado arquitetural, memória, I/O, fluxo de controle e condições de borda estiverem cobertos por testes.
- As suítes formais e diferenciais definidas tiverem resultados publicados e reproduzíveis.
- Não houver divergências abertas sem justificativa técnica aprovada.

O Z80 só poderá ser declarado **TEMPORAL_PASS** quando, além disso:
- Os M-cycles/T-states e a ordem dos acessos de barramento estiverem validados por traces independentes.
- WAIT, interrupções, HALT, snapshots e retomada estiverem demonstrados em todos os cenários obrigatórios.
- A integração com Scheduler, bus e sistemas SMS/SG-1000 estiver aprovada.

**Meta final:** 100% das instruções documentadas elegíveis modeladas; 100% dos casos semânticos definidos aprovados; 100% dos cenários temporais obrigatórios aprovados. Qualquer comportamento não documentado, indeterminado ou dependente de revisão de silício deve permanecer explicitamente classificado, sem ser escondido no percentual.

## 9. Regra de atualização deste documento

Atualizar esta referência ao final de cada fase E4C–E9 e sempre que uma alteração mudar:
- O universo/denominador de instruções elegíveis.
- A classificação de uma família.
- O nível de evidência (QUICK_PASS, SHORT_PASS, LONG_PASS, DIFFERENTIAL_PASS, TEMPORAL_PASS).
- A baseline `main`, o SHA candidato ou o estado de uma PR.
- Os critérios de integração de SMS/SG-1000.

Toda atualização deve incluir data, SHA da baseline, SHA candidato, PR, workflows, números absolutos, percentuais por dimensão e lacunas remanescentes. Nunca apagar histórico de resultados: registrar a nova fotografia e preservar a anterior.

---

**Baseline operacional (2026-11-04):** main `9b606a6c785b9e7bb1999534e7e2292735254134` após merge da PR #503. E7.1, E7.2A, E7.2B, E7.2C e E7.2D integradas pelas PRs #490, #495, #496, #498 e #500. PR #501 integrada. PR #502 preservada como candidata histórica; PR #504 (`b78d3354119c3f9af475561ff15db7673e93fb06`) com CI #41825655861 PASS nos gates Rust/Python/Windows, Scheduler stress SKIPPED. E7.2E segue IN_PROGRESS. Próximo trabalho é concluir E7.2E e as fases E7.3–E7.7 antes de iniciar a validação E8. E7 global continua `INTEGRATION_PENDING`.


## Atualização operacional — 2026-11-04, após PR #505

Esta seção prevalece sobre fotografias operacionais anteriores deste documento.

- **Baseline main:** `644b2c22879d63447f63b730ed9415db11e81fab`, após merge da PR #505.
- **E7.2E:** PR #505 integrada; restore/continuação de estados completos para as 70 variantes de fase alcançadas pelo corpus suportado.
- **PR #506:** aberta, branch `test/z80-e7-2e-invalid-state-matrix-20261104`, HEAD `73ab196feb09aec2b6cdf21b73ff4b7d1cc0dde1`. Acrescenta matriz de rejeição de estados inválidos e correções no validador de snapshots.
- **CI #41860306541**, no HEAD exato da PR #506: Rust fmt/Clippy/workspace tests PASS; SMS core e SG-1000 core tests PASS; Python Ruff format/Ruff lint/pytest PASS; Windows x64 D3D12/vídeo e WASAPI/XAudio2/áudio compile/tests PASS. Scheduler deterministic stress SKIPPED (não foi alterado nesta PR).
- A matriz percorre 70 fases geradas pelo corpus suportado, mantém um fixture válido por fase e executa mutações de M-cycle, T-state, request ID, monotonicidade do ID, endereço, operação/payload, duração, request ausente, fase/request incompatível, WAIT sem request e opcode ausente quando aplicável. O teste exige pelo menos 210 mutações e verifica rejeição pelo encoder e por `restore_state()`.
- A matriz revelou validação insuficiente de M-cycle em snapshots de instruções imediatas com prefixo DD/FD ignorado. O validador agora relaciona a quantidade de M1s de prefixo ao mínimo de T-states acumulados, usando aritmética checked; verificações correlatas de fases imediatas foram endurecidas.
- **Limites:** um fixture por variante de fase não representa todas as combinações de opcode/prefixo/request; casos UnsupportedOpcode não entram no corpus suportado. A matriz WAIT=0/1/múltiplo por família de barramento, T-states terminais de IM0, codec binário persistente da máquina SMS completa e comprovação temporal sistêmica continuam pendentes.
- **Classificação:** PR #506 com CI convencional PASS no HEAD registrado; E7.2E permanece IN_PROGRESS / INTEGRATION_PENDING até fechar as lacunas remanescentes. E7 global permanece INTEGRATION_PENDING. Não atribuir LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS.



**Revalidação documental do HEAD final da PR #506:** HEAD `1bb057ec126eb07de485a16bcdd98c42552f5c93`, CI [#41860812506](https://github.com/leonardo201800478/sega-emulator/actions/runs/41860812506) SUCCESS em Rust, Python e Windows x64; Scheduler stress SKIPPED. A PR permanece aberta e E7.2E/E7 global permanecem INTEGRATION_PENDING.


---

## Fotografia operacional reconciliada — 2026-11-04 (main 644b2c22879d63447f63b730ed9415db11e81fab)

Esta atualização prevalece sobre fotografias históricas anteriores deste documento. A fonte da verdade é o estado remoto do GitHub.

### Baseline e E7

- `main`: `644b2c22879d63447f63b730ed9415db11e81fab`; CI pós-merge [#41831738966](https://github.com/leonardo201800478/sega-emulator/actions/runs/41831738966) SUCCESS.
- Integradas: E7.1 HALT/M1 (#490), E7.2A NMI (#495), E7.2B INT acknowledge (#496), E7.2C IM0/IM1/IM2 (#498), E7.2D EI/DI (#500), além das reconciliações documentais #501/#503 e restore por fases #505.
- PR #504 também está integrada; merge SHA `d64d86b5bb16305d6265a1522d095b68b8f4954a`.
- E7.2E permanece **IN_PROGRESS / INTEGRATION_PENDING**. PR #506 é a continuação ativa, baseada na main atual.
- PR #506 HEAD no início desta fotografia: `09c6dcb9a8f5a7ff7ca3f5934c1223515b5529a1`. CI [#41861829094](https://github.com/leonardo201800478/sega-emulator/actions/runs/41861829094) estava QUEUED nesse SHA. Portanto, o HEAD atual ainda não tem aprovação final.
- CI [#41860306541](https://github.com/leonardo201800478/sega-emulator/actions/runs/41860306541) passou no HEAD `73ab196feb09aec2b6cdf21b73ff4b7d1cc0dde1`; CI [#41860812506](https://github.com/leonardo201800478/sega-emulator/actions/runs/41860812506) passou no HEAD documental anterior `1bb057ec126eb07de485a16bcdd98c42552f5c93`. Esses sucessos não substituem o CI do HEAD atual.
- O run #41861604611 foi executado no SHA anterior `f2fc621...` e falhou em rustfmt por whitespace; o whitespace foi corrigido em `09c6dcb...`. A execução de Windows e Python daquele run não transforma o job Rust em PASS.
- Scheduler deterministic stress foi SKIPPED nos workflows referidos; não contar como PASS.

### Conteúdo ativo da PR #506

- Restore/continuação dos estados completos das 70 variantes de fase alcançadas pelo corpus suportado (PR #505, já integrada).
- Matriz de rejeição de estados inválidos: fixtures por fase, mutações de índices M/T, request ID, monotonicidade, endereço, operação/payload, duração, request ausente, incompatibilidade fase/request, WAIT sem request e opcode ausente.
- Correção do validador para fases imediatas com DD/FD ignorado, vinculando contagem M1 ao mínimo de T-states.
- Ampliação de testes WAIT=0/1/3 para M1, memória, I/O, ciclo interno e interrupções IM0/IM1/IM2; execução contínua versus retomada.
- IM0 tem asserção terminal explícita. O resultado dessa nova matriz só será considerado validado após CI verde no HEAD final.

### Próximas subfases

1. Fechar E7.2E e suas lacunas de restore/WAIT/trace.
2. E7.3: amostragem real de INT/NMI no T4 de HALT, incluindo `EI;HALT;INT`.
3. E7.4: BUSREQ e arbitragem real, suspensível/retomável.
4. E7.5: RETI e daisy-chain, distintos de RETN.
5. E7.6: IRQ real do VDP SMS, status e acknowledge.
6. E7.7: auditoria semântica integral BASE/CB/ED/DD/FD/DDCB/FDCB.
7. Somente após E7 concluída: E8 diferencial/formal; E9 comprovação temporal sistêmica.

Não declarar E7 concluída, LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS nesta fotografia.


---

## Fotografia operacional reconciliada — 2026-11-04 (main 644b2c22879d63447f63b730ed9415db11e81fab)

Esta atualização prevalece sobre fotografias históricas anteriores deste documento. A fonte da verdade é o estado remoto do GitHub.

### Baseline e E7

- `main`: `644b2c22879d63447f63b730ed9415db11e81fab`; CI pós-merge [#41831738966](https://github.com/leonardo201800478/sega-emulator/actions/runs/41831738966) SUCCESS.
- Integradas: E7.1 HALT/M1 (#490), E7.2A NMI (#495), E7.2B INT acknowledge (#496), E7.2C IM0/IM1/IM2 (#498), E7.2D EI/DI (#500), além das reconciliações documentais #501/#503 e restore por fases #505.
- PR #504 também está integrada; merge SHA `d64d86b5bb16305d6265a1522d095b68b8f4954a`.
- E7.2E permanece **IN_PROGRESS / INTEGRATION_PENDING**. PR #506 é a continuação ativa, baseada na main atual.
- PR #506 HEAD no início desta fotografia: `09c6dcb9a8f5a7ff7ca3f5934c1223515b5529a1`. CI [#41861829094](https://github.com/leonardo201800478/sega-emulator/actions/runs/41861829094) estava QUEUED nesse SHA. Portanto, o HEAD atual ainda não tem aprovação final.
- CI [#41860306541](https://github.com/leonardo201800478/sega-emulator/actions/runs/41860306541) passou no HEAD `73ab196feb09aec2b6cdf21b73ff4b7d1cc0dde1`; CI [#41860812506](https://github.com/leonardo201800478/sega-emulator/actions/runs/41860812506) passou no HEAD documental anterior `1bb057ec126eb07de485a16bcdd98c42552f5c93`. Esses sucessos não substituem o CI do HEAD atual.
- O run #41861604611 foi executado no SHA anterior `f2fc621...` e falhou em rustfmt por whitespace; o whitespace foi corrigido em `09c6dcb...`. A execução de Windows e Python daquele run não transforma o job Rust em PASS.
- Scheduler deterministic stress foi SKIPPED nos workflows referidos; não contar como PASS.

### Conteúdo ativo da PR #506

- Restore/continuação dos estados completos das 70 variantes de fase alcançadas pelo corpus suportado (PR #505, já integrada).
- Matriz de rejeição de estados inválidos: fixtures por fase, mutações de índices M/T, request ID, monotonicidade, endereço, operação/payload, duração, request ausente, incompatibilidade fase/request, WAIT sem request e opcode ausente.
- Correção do validador para fases imediatas com DD/FD ignorado, vinculando contagem M1 ao mínimo de T-states.
- Ampliação de testes WAIT=0/1/3 para M1, memória, I/O, ciclo interno e interrupções IM0/IM1/IM2; execução contínua versus retomada.
- IM0 tem asserção terminal explícita. O resultado dessa nova matriz só será considerado validado após CI verde no HEAD final.

### Próximas subfases

1. Fechar E7.2E e suas lacunas de restore/WAIT/trace.
2. E7.3: amostragem real de INT/NMI no T4 de HALT, incluindo `EI;HALT;INT`.
3. E7.4: BUSREQ e arbitragem real, suspensível/retomável.
4. E7.5: RETI e daisy-chain, distintos de RETN.
5. E7.6: IRQ real do VDP SMS, status e acknowledge.
6. E7.7: auditoria semântica integral BASE/CB/ED/DD/FD/DDCB/FDCB.
7. Somente após E7 concluída: E8 diferencial/formal; E9 comprovação temporal sistêmica.

Não declarar E7 concluída, LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS nesta fotografia.


### Estado quantitativo e limitações não alteradas

A cobertura estrutural de 70 variantes, 70 writer arms, 70 reader arms e 70 restore-validator arms é uma contagem de estrutura. Os testes de restore por fase cobrem os estados gerados pelo corpus, não todas as combinações válidas possíveis. Casos UnsupportedOpcode excluídos não contam como semântica aprovada. O DTO SMS in-memory não é codec binário persistente da máquina SMS completa. WAIT e traces independentes ainda não estão demonstrados em todos os limites. E7 global continua INTEGRATION_PENDING.


---

## Fotografia operacional reconciliada — 2026-11-04 (após PR #509)

Esta atualização prevalece sobre as fotografias históricas anteriores. Baseline verificada por comparação remota: `main` é idêntica ao merge commit da PR #509.

- **main:** `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`.
- **E7.2E:** PR #506 integrada no merge SHA `54f881872ec9f5a9923ebdb17643b37a19954ec5`. Workflow #41872451175: Rust quality/regression PASS; Python lint/regression PASS; Windows x64 D3D12 PASS; Scheduler deterministic stress PASS.
- **E7.3:** PR #509 integrada no merge SHA `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`. Workflow #41877136387: Rust quality/regression PASS; Python lint/regression PASS; Windows x64 D3D12 PASS; Scheduler deterministic stress PASS.
- **E7.3 limite:** CI verde demonstra os gates convencionais no HEAD validado, mas não substitui trace independente de barramento nem autoriza `TEMPORAL_PASS`.
- **E7.4:** próxima etapa ativa. A auditoria `docs/audits/z80-e7-4-busreq-arbitration-2026-11-04.md` identificou que o runtime ainda não expõe uma notificação genérica e verificável de conclusão de M-cycle. `tstate_index` é explicitamente parcial e não representa bordas elétricas; não inferir amostragem BUSREQ desses índices.
- **E7.4.1:** próximo incremento de código: explicitar e testar a fronteira M-cycle no contrato de transição do Z80 compartilhado, incluindo M1, ciclos de memória/I/O e ciclos internos. Somente depois integrar estado BUSREQ/BUSACK e arbitragem no perfil de máquina.
- **E7.5–E7.7:** pendentes.
- **E7 global:** `INTEGRATION_PENDING`; não declarar conclusão funcional/temporal.
- **E8:** bloqueada até fechar E7. Não iniciar ZEXDOC/ZEXALL como etapa de conclusão antes da auditoria semântica E7.7.
- **PRs documentais #507, #508 e #510:** permanecem abertas na base histórica `54f881...`; não considerar integradas nem atuais. A PR #508 é draft e contém a modelagem preliminar E7.3–E7.7. A auditoria E7.4 atual deve prevalecer quanto às lacunas de fronteira M-cycle.

Classificação de suites: os quatro jobs de cada workflow #41872451175 e #41877136387 foram concluídos com SUCCESS. Testes locais nesta sessão: `NOT RUN` (clone indisponível por falha de resolução DNS de github.com). Não atribuir `LONG_PASS`, `DIFFERENTIAL_PASS` ou `TEMPORAL_PASS` com base apenas nesses workflows.


## Atualização E7.4.1B — auditoria temporal WAIT/TW (2026-11-04)

Esta fotografia complementa a auditoria BUSREQ e prevalece para o estado da subfase E7.4.1B. A implementação de eventos de borda T-state na PR #514 passou os jobs Rust, Python e Windows no HEAD ed81de385f7f4a0e7ed50de434b1f4c143cdad9f (workflow 41896166347); Scheduler deterministic stress foi SKIPPED e não conta como PASS. Testes locais: NOT RUN.

A revisão direta do ZiLOG UM008011-0816 encontrou requisitos não representados pelo runtime candidato:
- WAIT é amostrado na borda de descida durante T2 e em cada TW subsequente; o evento atual só identifica conclusão por T-state/borda de subida e não registra a fase de amostragem.
- Cada ciclo I/O possui um TW automático entre T2 e T3; PortRead/PortWrite permanecem requests de 4T, mas o M-cycle atual os representa como quatro T-states nominais, sem fase TW automática explícita.
- O acknowledge INT possui dois TW automáticos; a rotina atual consome waits mandatórios antes de consultar WAIT externo, portanto não demonstra amostragem WAIT durante os TW mandatórios.
- A escala temporal atual ainda precisa demonstrar representação exata da meia-fase, especialmente quando master_ticks_per_tstate é ímpar. Não arredondar falling edge nem introduzir relógio paralelo.

A especificação foi retificada na PR #513, HEAD 143cdce6fde85621c81de0a1bc43350b180422cc, incluindo estado de fase de clock, TW automático, amostras WAIT, snapshots entre falling/rising edges e equivalência contínua/retomada.

**Classificação:** E7.4.1A (contrato isolado M-cycle) e E7.4.1B (progressão runtime por evento) possuem implementação candidata e CI convencional parcial PASS, mas E7.4.1B permanece **INCOMPLETE / BLOCKED** por fidelidade da fase WAIT/TW. Não avançar para BUSREQ/BUSACK até corrigir e validar:
1. falling-edge sampling e a resolução temporal exata;
2. TW automático de I/O e sua posição T2–T3;
3. WAIT amostrado nas fases T2/TW, inclusive TW obrigatório de I/O/INT;
4. snapshots em cada fase e equivalência de execução contínua/retomada;
5. regressão de M1, ciclos internos, HALT e interrupções.

E8 permanece bloqueada até fechamento de E7. Não atribuir TEMPORAL_PASS com base no CI convencional.


---

## Fotografia operacional reconciliada — 2026-11-04 (C1 / E7.4.1C)

Esta seção prevalece sobre fotografias históricas anteriores. Baseline remota após PR #517: `main` = `fcd91302cb64a31d63b75f5e1d3b6eab336668d1`.

- **E6:** integrada pela PR #486, merge SHA `24a32e03e10c8e82022172faee69ec0c046f8cb0`.
- **E7.3:** HALT T4 integrada pela PR #509, merge SHA `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`. CI de integração registrado como PASS nos quatro jobs; isso não constitui TEMPORAL_PASS global.
- **E7.4 documentação:** PR #511 integrada, merge SHA `5452ec9eb4307b997badff1858bfc60e7078a937`; ela audita BUSREQ/BUSACK, mas não implementa arbitragem no runtime.
- **E7.4.1A — M-cycle:** PR #512 permanece aberta e não integrada. CI no HEAD `19866f365b4873603a7dd230121a1127aeca9c2d`, workflow `41891089747`, 4/4 jobs PASS. A revisão semântica identificou lacunas WAIT/TW/falling-edge; CI verde não remove o bloqueio.
- **E7.4.1B — eventos T-state SMS:** PR #514 permanece aberta e não integrada. Workflow `41896166347`: Rust/Python/Windows PASS; Scheduler stress SKIPPED. O contrato atual não comprova amostragem WAIT na falling edge, TW automático tipado de I/O/INT nem amostragem externa durante TW obrigatório.
- **E7.4.1C — C1:** `ClockEdgeTime` calcula bordas racionais, mas ainda não integra a unidade temporal de `MachineScheduler`, `EventQueue`, watermarks, estados de clock domain e codec. Esses consumidores continuam em `MachineTime` inteiro; `HardwarePhase` não representa sub-tick.
- **Próxima implementação:** C1 no `sega-temporal-core` e `machine-scheduler`: instante racional canônico, aritmética/comparação exatas e verificadas, ordenação causal, watermarks, snapshot/restore, codec versionado e chunk invariance. Não arredondar falling edge nem criar scheduler paralelo SMS.
- **Sequência após C1:** C2 falling/rising edges; C3 TW automático e WAIT por origem; C4 integração Z80/SMS; C5 snapshots em subfases; C6 validação contínua/em blocos e traces. Só depois retomar BUSREQ/BUSACK.
- **E7.5–E7.7:** RETI/daisy-chain, IRQ real do VDP e auditoria semântica integral permanecem pendentes.
- **E8:** bloqueada até fechamento integral de E7. Não declarar LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS global com base em CI convencional.
- **Infraestrutura:** PR #516 integrou o gerenciador centralizado de toolchains (merge SHA `d06d4d203412d103a327a57604471074ef15faf6`); PR #517 integrou lockfiles independentes e correção Clippy (merge SHA `fcd91302cb64a31d63b75f5e1d3b6eab336668d1`). Validação local da #517: 20 PASS, 0 FAIL, 0 BLOCKED; quatro manifests fora dos workspaces ativos continuam NOT RUN. CI da #517 no HEAD `fc5f710f8c56698cb82dd4e49e881cea15e4639b`, workflow `41908119554`: 4/4 jobs PASS.

**Classificação global:** E7 permanece `INTEGRATION_PENDING`; C1 é o próximo bloqueio arquitetural. Esta fotografia documental não altera nem aprova código de hardware.



---

## Retomada E7 — auditoria independente de três agentes (2026-11-04)

Esta seção prevalece sobre fotografias operacionais anteriores deste scorecard.

### Baseline remota reconciliada

- **main atual:** `249d7a3da613dec456f22aa6a7ceaccb2418ee0b`.
- **PR #526:** aberta; branch `feat/temporal-core-machine-instant-c1-clippy`; HEAD verificado `1e5c0c23f959fb99f5cb1d8382b6c1d3dfdd3410`.
- **Workflow #41944516006** no HEAD #526:
  - Change impact classification: PASS.
  - Rust quality/regression: PASS.
  - Scheduler deterministic stress: PASS.
  - Windows Python 3.15 PyO3 extension build/import: PASS.
  - Refresh Cargo lockfiles (manual artifact): NOT RUN (job SKIPPED).
  - Windows PowerShell tooling syntax: NOT RUN (job SKIPPED).
  - Windows x64 D3D12 compile: NOT RUN (job SKIPPED).
  - Windows Python 3.15 Qt smoke test: NOT RUN (job SKIPPED).
  - Python lint and regression: NOT RUN (job SKIPPED).
- **PR #512:** aberta, não integrada; contrato M-cycle candidato.
- **PR #514:** aberta, não integrada; progressão T-state no SMS candidata.
- **PR #515:** encerrada; sua auditoria documental C1 continua disponível em `docs/audits/z80-e7-4-1c-rational-timeline-2026-11-04.md`.
- Nenhum resultado SKIPPED foi contado como PASS. O run #41944516006 não prova gates Python, Qt, D3D12, PowerShell ou refresh de lockfiles.

### Resultado dos três agentes

1. [Agente 1 — pesquisa comparativa de emuladores](audits/z80-e7-agent1-emulator-research.md): MAME fornece evidência de estados WAIT/BUSREQ/BUSACK e integração daisy-chain; chips/Z80 fornece arquitetura tick/pin; SingleStepTests e Fuse são corpora temporais diferenciais; raxoft/z80test fornece resultados de hardware real para comportamento arquitetural. ares e a implementação interna Gearsystem permanecem parcialmente NOT VERIFIED nesta execução.
2. [Agente 2 — conformidade documental](audits/z80-e7-agent2-documentation-conformance.md): confirma WAIT na falling edge de T2 e TW subsequentes, TW automático de I/O, dois TW do INT acknowledge, HALT T4, BUSREQ/BUSACK, EI delay, RETI/RETN e fonte IRQ/NMI do SMS.
3. [Agente 3 — projeto semântico](audits/z80-e7-agent3-semantic-design.md): mantém C1 como bloqueio; especifica a migração racional integral, fases elétricas, waits tipados, snapshots e critérios de aceitação.

### Bloqueio matemático novo identificado em C1

A implementação candidata da PR #526 pode retornar overflow intermediário ao construir o denominador comum em `u128`, mesmo quando a fração final reduzida cabe em `u128`. Contraexemplo reproduzível está no relatório do Agente 3: `1/(2g) + ((g-3)/2)/(3g) = 1/6`, com denominadores de entrada representáveis e LCM intermediário maior que `u128::MAX`.

**Ação exigida antes de aprovar C1:** implementar soma/subtração com intermediário suficientemente largo (ou algoritmo equivalente demonstravelmente exato), reduzir a fração antes de estreitar para os campos canônicos e adicionar o contraexemplo como teste de regressão.

### Lacunas de integração ainda abertas em PR #526

A leitura do HEAD candidato confirma que:
- `MachineScheduler.now` permanece `MachineTime`;
- `ClockDomain` e `ClockDomainSnapshot` ainda usam tempo inteiro;
- `ClockDomainState.current_time/registration_start` continuam inteiros;
- dispatch ainda expõe timestamp integral e contém `SubTickEventUnsupported`;
- os consumidores SMS/SG-1000 e os codecs de estado completo ainda não foram migrados integralmente para instante racional;
- falling/rising edges não estão representadas como eventos persistíveis.

Portanto, a PR #526 continua **C1 INCOMPLETE / NOT READY TO MERGE**, apesar dos três jobs de qualidade/stress e do build PyO3 terem passado no HEAD indicado.

### Próxima sequência autorizada tecnicamente

1. Completar a aritmética racional e o contraexemplo de redução.
2. Migrar ClockDomain, Scheduler.now, dispatch, watermarks, snapshots e codec para `MachineInstant`.
3. Migrar SMS e SG-1000 sem clocks paralelos ou truncamento.
4. Remover `SubTickEventUnsupported` somente após prova de avanço/dispatch racional ponta a ponta.
5. Fechar C1 com chunk invariance e restore.
6. Executar C2 (falling/rising) e C3 (WAIT/TW) antes de integrar o Z80/SMS.
7. Só após C1–C6 retomar BUSREQ/BUSACK; depois RETI/daisy-chain, IRQ VDP e auditoria semântica E7.7.

**Estado global:** E7 permanece `INTEGRATION_PENDING`; E8 permanece bloqueada. Não atribuir `LONG_PASS`, `DIFFERENTIAL_PASS` ou `TEMPORAL_PASS` com base nesta auditoria documental ou na CI convencional.
