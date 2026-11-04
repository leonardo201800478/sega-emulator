# Meta de conclusão do Z80 — cobertura, validação e critérios de aceite

**Data da revisão:** 2026-11-04  
**Baseline de referência:** `main` — `a0e58fd05c7f62c42f82b86d15d8532e06a9c59b` (após PR #476; código E4C integrado pela PR #475).  
**Evidência E4C:** HEAD validado `8f04b44e6c6ba85920a7438b74c9876586d5736c`; CI [41747460761](https://github.com/leonardo201800478/sega-emulator/actions/runs/41747460761), 4/4 jobs PASS; PR #475 integrada no merge SHA `f89a52346e91e88e82ab37a4afcfffff2d793407`.  
**Schema do microsequenciador na baseline main:** 27. O candidato E5 da PR #485 eleva para 28.  
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

A baseline consultada em `main` é `f89a52346e91e88e82ab37a4afcfffff2d793407`, após a integração da E4C pela PR #475. A auditoria `docs/audits/z80-indexed-incdec-e4c-2026-11-03.md` e o teste `native/z80_cpu/tests/indexed_incdec_e4c.rs` estão integrados. O HEAD da PR passou nos quatro grupos de CI.

| Fase | Escopo | Estado na fotografia |
|---|---|---|
| E1–E2 | Inventário, semântica DD/FD, FSM, last-prefix-wins e snapshots | Integradas anteriormente; SHORT_PASS no escopo registrado |
| E3A1 | Operações 16-bit IX/IY | Integrada na main |
| E3A2 | Transferências absolutas, PUSH/POP e EX (SP),IX/IY | Integrada na main |
| E4A | LD r,(IX/IY+d), LD (IX/IY+d),r e LD (IX/IY+d),n | Integrada na main; IMPLEMENTED / SHORT_PASS |
| E4B | ADD/ADC/SUB/SBC/AND/XOR/OR/CP (IX/IY+d) | Integrada na main pela PR #472; IMPLEMENTED / QUICK_PASS / SHORT_PASS; CI 4/4 PASS |
| E4C | INC/DEC (IX/IY+d) | Integrada na main pela PR #475; IMPLEMENTED / QUICK_PASS / SHORT_PASS; 5 testes dirigidos; CI 4/4 PASS |
| E5 | DDCB/FDCB completo | IMPLEMENTED / QUICK_PASS / SHORT_PASS; CI PR #485 3/3 PASS |
| E6 | Fallback geral e resolução de prefixos DD/FD/ED/CB | Pendente/incompleto no microsequenciador |
| E7 | Fechamento semântico global BASE/CB/ED/DD/FD/DDCB/FDCB | Pendente |
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
- [x] `cargo fmt --all -- --check`: PASS no CI run #4238, código SHA `2f1dff62ce23c8d6d321b26d6ffc1b4bd198cb5f`.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS no mesmo SHA.
- [x] `cargo test --workspace --locked`: PASS; 216 testes unitários do crate Z80 e 8 testes da nova suíte indexed-CB passaram, além das demais suítes do workspace.
- [x] Testes direcionados SMS e SG-1000: PASS.
- [x] `python -m pytest`, `ruff check` e `ruff format --check`: etapas de comando PASS; job concluiu PASS no run #4238.
- [x] Windows x64 D3D12/video compile e video-runtime tests: PASS.
- [x] Windows WASAPI/XAudio2 compile e audio-output tests: PASS.
- [x] CI aplicável: 3/3 jobs PASS; Scheduler deterministic stress SKIPPED corretamente por não haver alteração no Scheduler.
- Estado: IMPLEMENTED / QUICK_PASS / SHORT_PASS para cobertura funcional estrutural e microsequenciada no candidato. A aprovação de integração da PR aguarda o gate Windows de áudio e revisão.

- Limite de fonte: ZiLOG UM008011-0816 marca S e P/V de BIT indexado como desconhecidos; o candidato mantém o modelo já existente no executor atômico. Isso não comprova esses bits contra silício.
- Não promover LONG_PASS, DIFFERENTIAL_PASS, ZEXDOC/ZEXALL ou TEMPORAL_PASS sem evidência independente.

### E6 — Resolução de prefixos

- Cobrir DD, FD, DD DD, FD FD, DD FD, FD DD, DD/FD+ED, DD/FD+CB e prefixos redundantes/ignorados.
- Garantir last-prefix-wins, incremento de R, PC, bytes consumidos, M1 e custo temporal.
- Prefixos ignorados devem executar a instrução BASE correta sem fetch duplicado ou substituição indevida de HL/H/L.
- Não impor limite artificial de prefixos repetidos.
- Aceite: matriz combinatória, stress de cadeias longas, snapshot/restore e regressões sem duplicação de fetch.

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

**Próxima meta operacional na baseline atual:** iniciar E6 — fallback geral e resolução de prefixos DD/FD/ED/CB. E4A, E4B e E4C estão integradas na main; E5 DDCB/FDCB está implementada e QUICK_PASS/SHORT_PASS na PR #485, com CI 3/3 PASS no SHA de código 2f1dff62. Validação diferencial externa, ZEXDOC/ZEXALL, LONG_PASS e TEMPORAL_PASS continuam pendentes.
