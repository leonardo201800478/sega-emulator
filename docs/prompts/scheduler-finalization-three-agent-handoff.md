# Handoff Prompt — Finalização do Machine Scheduler com 3 Agentes

Você está assumindo a continuidade do projeto **Sega Emulator — Emulação de Alta Precisão**.

## 1. Papel e objetivo

Atue como Arquiteto de Software, engenheiro de emulação e coordenador técnico. Sua tarefa é finalizar a validação do **Machine Scheduler** já implementado na stack de PRs, sem reiniciar etapas concluídas, sem duplicar código e sem ampliar o escopo.

**Repositório oficial:** https://github.com/leonardo201800478/sega-emulator

**Documento técnico obrigatório:** `docs/architecture/machine-scheduler-technical-reference.md`

**Evidências obrigatórias:** `docs/evidence/adr-0013-snapshot-restore-validation.md`

**ADR principal:** `docs/architecture/adr/0013-deterministic-scheduler-snapshot-restore.md`

Leia os três documentos e confira o código real antes de propor mudanças.

## 2. Regra de continuidade: verificar o estado remoto primeiro

Não confie cegamente em SHAs, estados de PR ou conclusões deste prompt. A primeira operação técnica deve ser consultar GitHub:

1. SHA atual de `main`.
2. Estado, head, base, draft, mergeability, checks e comentários das PRs #343–#356 (ou da faixa que ainda existir).
3. Estado da PR #358 e checks do head atual; verificar também se #351, #355 e #356 já foram integralmente absorvidas e encerradas como superseded.
4. Branches da stack e relações de dependência.
5. Últimos commits e workflows associados aos SHAs candidatos.

Ponto de referência histórico, não presuma que ainda seja atual:
- #354 final head: `11378882ea73a7307430dcb52389709fdea2a509`, workflow `41370823497` SUCCESS nos seis gates.
- #358 inicial head: `ca349d304942d105fcef4bf133be8be2c79c8ed6`, workflow `41377566361` SUCCESS nos seis gates ordinários; job de stress SKIPPED.
- Consolidação documental posterior alterou o head de #358; o SHA e os checks atuais devem ser consultados novamente.
- `main` no último levantamento: `0da3098893cce4d80f7a22291d00488e4e7d0f9b`.

Se o estado remoto divergir, a fonte da verdade é GitHub. Preserve trabalho novo e histórico existente.

## 3. Estratégia de três agentes — divisão obrigatória

Use três agentes independentes, se o ambiente realmente disponibilizar execução multiagente. Se não disponibilizar, execute três trilhas de auditoria separadas e informe honestamente que não foram agentes autônomos. Não simule paralelismo nem invente resultados.

### AGENTE A — Auditoria semântica e invariantes

**Missão:** procurar defeitos de correção no código existente, sem começar refatorações amplas.

Auditar:
- `MachineTime`, `ClockRatio`, `ClockDomain`, fase racional e `ClockEdgeTime`.
- Overflow, underflow, conversões e invariância de avanço em blocos.
- Ordenação lexicográfica de eventos: timestamp, `HardwarePhase`, `PlatformPriority`, sequence.
- Watermark causal, agendamento retroativo e inserção no timestamp atual.
- Exaustão de sequence e invariantes do allocator.
- Delta-event budget, contagem, reset por timestamp e retomada após aumento do limite.
- Atomicidade de `dispatch_next`, `advance_idle_to`, restore de domínio e restore completo.
- Consistência cruzada entre `SchedulerState`, clock domains, queue, watermark e histórico de dispatch.
- Codec: framing, versões, CRC, tamanhos, alocação, tags, truncamento, trailing bytes e payload codec.
- Determinismo e estabilidade de replay.

**Entregável:** relatório de findings classificados Critical/High/Medium/Low, com caminho, linha, cenário reproduzível, impacto e correção mínima sugerida. Não alterar código antes de apresentar os findings ao coordenador.

### AGENTE B — Testes de estresse e propriedades

**Missão:** criar e executar campanhas de validação SHORT_PASS e LONG_PASS.

SHORT_PASS deve cobrir rapidamente:
- múltiplas razões racionais, incluindo 1/3, 3/2, 5/3 e 7/3;
- fases iniciais válidas e inválidas;
- avanços contínuos versus fragmentados;
- eventos em timestamps iguais e distintos;
- todas as dimensões de ordenação e empates de sequência;
- cadeias causais de eventos no mesmo timestamp;
- limites do delta budget, alteração e retomada;
- snapshots em pontos de fronteira e restauração independente;
- round-trip do codec, versões incompatíveis e entradas malformadas;
- overflow e rejeição sem mutação parcial.

LONG_PASS deve usar seeds fixas e variadas, sequências longas de operações, snapshots repetidos, múltiplos domínios, cargas de eventos, restaurações sucessivas e fuzz/property-style determinístico. Deve ter limites explícitos de tempo/memória e produzir logs reproduzíveis. Não criar testes aleatórios não determinísticos sem registrar seed e operação que falhou.

Comparar execuções contínuas e restauradas por resultados observáveis: eventos, chaves de ordenação, sequências, clock advances, erros e estado exportado final.

**Entregável:** plano, seeds, comandos, duração, contagens, logs, resultados PASS/FAIL/BLOCKED e bugs encontrados. Não declarar LONG_PASS com base apenas em unit tests rápidos.

### AGENTE C — CI, reprodutibilidade e fronteira de integração

**Missão:** validar que o Scheduler está pronto para ser consumido pela máquina sem acoplamento indevido.

Auditar:
- workspace Cargo e dependências entre `temporal_core` e `machine_scheduler`;
- ausência de lógica de dispositivo no crate genérico;
- APIs públicas, documentação e contratos de integração;
- que o Scheduler retorna dispatches e clock advances, sem callbacks de VDP/CPU/PSG;
- que a integração futura não duplica conversão racional nem move timing para Python;
- PR stack, SHA exato e checks associados.

Executar, no SHA candidato final, os seis gates obrigatórios:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

**Entregável:** matriz de gates PASS/FAIL/BLOCKED/NOT RUN, SHA exato, workflow ID, logs e análise da fronteira arquitetural. Nunca transportar PASS de um SHA para outro.

## 4. Coordenador / agente principal

O coordenador deve:

1. Confirmar o estado remoto e ler a documentação.
2. Distribuir tarefas sem sobreposição de escrita.
3. Receber os relatórios dos três agentes antes de alterar o núcleo.
4. Consolidar findings e priorizar Critical/High.
5. Definir critérios de aceitação para cada correção.
6. Implementar correções pequenas, isoladas e testáveis.
7. Executar QUICK_PASS após cada fatia.
8. Executar SHORT_PASS e LONG_PASS em SHAs identificados.
9. Executar todos os seis gates no SHA final exato.
10. Atualizar ADR, documentação técnica e evidence ledger.
11. Abrir/atualizar PR Draft em branch própria, sem merge automático.
12. Entregar relatório final com SHAs, workflows, testes e pendências.

Se agentes diferentes encontrarem o mesmo problema, consolidar em um único finding. Apenas o coordenador deve integrar alterações conflitantes. Evitar múltiplos agentes editando o mesmo arquivo.

## 5. Regras arquiteturais imutáveis

- Backend de emulação em Rust; Python estritamente GUI/tooling.
- Runtime determinístico, cycle-accurate como objetivo; sem JIT.
- Sem floats no cálculo de tempo/ciclos.
- Sem hacks, frame skip, paralelismo especulativo ou otimização prematura.
- Scheduler genérico; não colocar nele semântica específica de SMS, VDP, CPU, PSG ou mapper.
- `HardwarePhase` é estrato ordinal de ordenação, não timestamp fracionário nem fase elétrica medida.
- `ClockEdgeTime` é cálculo exato de borda; não significa que a EventQueue atual despache eventos sub-tick.
- Não inferir clocks ou prioridades físicas sem documentação primária ou medição independente.
- Não usar callbacks por instrução entre Python e Rust.
- Não introduzir dependências sem justificativa, auditoria de versão e compatibilidade com `--locked`.

## 6. Fronteira acordada para dispositivos

Depois da finalização do Scheduler:

- **VDP:** receberá implementação completa em uma trilha própria, com fidelidade de hardware.
- **Z80, PSG, mappers e demais dispositivos:** inicialmente receberão somente a ligação ao controle de cadência/ciclos do Scheduler. O detalhamento interno de timing será aprimorado em etapas independentes.
- A camada de máquina/sistema registra domínios, agenda eventos documentados, consome `MachineDispatch` e coordena os dispositivos.
- Não antecipar implementação completa de VDP ou refatoração interna de CPU/PSG nesta tarefa de finalização do Scheduler.

## 7. Segurança Git e PR

- Nunca commitar diretamente em `main`.
- Nunca fazer force-push ou reset destrutivo.
- Nunca mergear PR nem marcar como pronta sem autorização explícita. PRs comprovadamente superseded podem ser fechadas após verificar que todo o conteúdo está presente na PR consolidada.
- Usar branch própria e PR Draft para mudanças não triviais.
- Respeitar a stack de PRs e não rebasear branches dependentes sem plano explícito.
- Antes de editar uma PR, verificar se seu head/base ainda correspondem ao estado auditado.
- Não descartar mudanças de terceiros.
- Não afirmar que um workflow passou sem consultar o resultado no SHA exato.
- Este handoff está sendo incorporado à PR #358. Atualize suas referências para o estado remoto atual e não mantenha instruções históricas contraditórias.

## 8. Estados e critérios de aceitação

Para cada componente, registrar:
- `IMPLEMENTED`
- `QUICK_PASS`
- `SHORT_PASS`
- `LONG_PASS`
- `INTEGRATION_PENDING`
- `TEMPORAL_PASS`

Para cada suíte:
- `PASS`
- `FAIL`
- `BLOCKED`
- `NOT RUN`

A conclusão desta tarefa exige:
- nenhum finding Critical/High sem resolução ou justificativa aceita;
- SHORT_PASS e LONG_PASS com evidência reproduzível;
- seis gates PASS no SHA final;
- ledger atualizado;
- PR aberta e revisável, sem merge.

Não atribuir `TEMPORAL_PASS` nesta fase apenas porque os testes de software passaram. Esse estado depende de integração temporal de dispositivos e oráculos independentes.

## 9. Escopo proibido nesta continuidade

Não:
- implementar o VDP completo;
- fechar lacunas de flags/instruções Z80;
- alterar mappers;
- adicionar áudio ou vídeo;
- implementar sub-tick event dispatch;
- inventar prioridades de hardware;
- mover lógica temporal para Python;
- refatorar módulos não relacionados;
- ampliar o Scheduler para bus arbitration.

Se um defeito fora do escopo bloquear a validação, documente-o e proponha uma PR separada.

## 10. Primeiro retorno esperado

Antes de codificar, apresente:
1. SHA atual de `main`.
2. Inventário atualizado das PRs e dependências.
3. Confirmação do SHA final validado de cada etapa E1–E7.
4. Status da PR #355 e do CI do seu head.
5. Plano de execução dos três agentes.
6. Critérios objetivos e orçamento de SHORT_PASS/LONG_PASS.
7. Riscos encontrados na leitura inicial.

Depois, aguarde a consolidação dos relatórios ou prossiga apenas com as verificações não destrutivas claramente autorizadas. Não faça merge.
