# Meta de conclusão do Z80 — cobertura, validação e critérios de aceite

**Data da revisão:** 2026-11-04  
**Baseline de referência:** `main` — `5768705658522deb46a0fb0556b51923ae8166b8` (E7.1 HALT/M1 e auditorias E7.2; PR #493 corrigiu somente o host D3D12, sem alterar o Z80).  
**Evidência E4C:** HEAD validado `8f04b44e6c6ba85920a7438b74c9876586d5736c`; CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS; PR #475 integrada no merge SHA `f89a52346e91e88e82ab37a4afcfffff2d793407`.  
**Schema do microsequenciador na baseline main:** 28, após a integração da E5 pela PR #485.  
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

A baseline operacional consultada em `main` é `5768705658522deb46a0fb0556b51923ae8166b8`. A PR #493 alterou somente `native/video_runtime/src/d3d12.rs`; não alterou o runtime Z80. O CI pós-merge #41775300143 passou em Rust, Python e Windows x64; Scheduler deterministic stress foi SKIPPED. E1–E6 e E7.1 estão integradas nos escopos registrados. A saída de HALT por interrupção, acknowledge, reset especial e demais lacunas semânticas continuam pendentes.

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
| E7 | Fechamento semântico global BASE/CB/ED/DD/FD/DDCB/FDCB | E7.1 HALT/M1 integrada na main pela PR #490; E7.2 NMI/INT e saída de HALT pendentes |
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
- **Limite:** saída de HALT por NMI/INT, acknowledge, reset especial e validação independente ainda não estão implementados neste incremento. Não atribuir SHORT_PASS integral a HALT.
- Auditoria de código E7 corrigida para a baseline atual `5768705658522deb46a0fb0556b51923ae8166b8`: [auditoria E7 — semântica ED, microsequenciador, snapshots e interrupções](audits/z80-e7-code-validation-2026-11-04.md). A revisão encontrou 26 testes ED positivos no microsequenciador, aprovados no CI pós-merge #41775300143 (216 testes unitários totais no crate Z80). Esses testes incluem comparação com o executor atômico interno e validações de requests/WAIT/snapshots; não equivalem a oráculo externo nem a trace independente. E7 permanece INTEGRATION_PENDING porque E7.2 NMI/INT e a validação temporal independente seguem pendentes.

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

**Baseline operacional:** E6 integrada pela PR #486; merge SHA do código E6 `24a32e03e10c8e82022172faee69ec0c046f8cb0`. Documentação reconciliada pela PR #488, merge SHA `303a792b1caab1ba40dec9c9521208eb5aaa0b45`. CI #41759035927 (PR E6), #41759330209 (push após merge E6) e #41759833153 (push após reconciliação documental) concluídos com SUCCESS. E7 em andamento: E7.1 HALT/M1 integrada pela PR #490 (merge SHA `b1b5ced5c3d6053f8d59c8f4d932f40518db1064`); CI da PR #490: Rust fmt/Clippy/SMS/SG-1000/workspace PASS, Python pytest/Ruff PASS e Windows x64 D3D12/WASAPI/XAudio2 PASS. Scheduler deterministic stress: SKIPPED. CI pós-merge #41765421167: Rust/Python/Windows PASS; Scheduler stress SKIPPED. E7.2 NMI/INT e saída de HALT é o próximo incremento. Validação diferencial externa, ZEXDOC/ZEXALL, LONG_PASS e TEMPORAL_PASS continuam pendentes.
