# Roadmap de reorganização e charter dos agentes

**Data:** 2026-10-31  
**Baseline remoto confirmado:** `main` = `1edaa70af80dd4d8fb9b574609b456abfab12c0d`  
**Escopo:** reorganização Clean Architecture, runtime Rust, Z80 compartilhado, scheduler temporal e supervisão independente de lacunas.  
**Natureza:** plano de execução; não declara implementações ou testes concluídos.

## 1. Objetivo e regras de coordenação

Executar a reorganização por incrementos pequenos, auditáveis e reversíveis, mantendo o GitHub como fonte de verdade. O plano de arquitetura está em [shared-z80-runtime-reorganization.md](shared-z80-runtime-reorganization.md).

O projeto definiu limite de **três agentes de IA simultâneos**. A necessidade de três agentes de execução e um supervisor é atendida com quatro papéis nomeados, mas apenas três ativos ao mesmo tempo: o supervisor independente entra em janelas de auditoria e substitui temporariamente um agente de execução, que pausa seu trabalho. Não haverá quatro agentes simultâneos.

O coordenador do projeto mantém a visão de conjunto, consolida decisões e é o único responsável por integrar resultados. Agentes não fazem merge, não publicam em `main`, não alteram branch alheia e não modificam o mesmo conjunto de arquivos.

### 1.1 Papéis dos agentes

| ID | Papel | Responsabilidade exclusiva | Entregáveis |
|---|---|---|---|
| A1 | Arquiteto e gerente técnico | Clean Architecture, limites entre camadas, contratos, ADRs, dependências, roadmap e coordenação de protótipos | Diagramas de dependência, ADRs, contratos e plano de integração |
| A2 | Engenheiro de protótipo Z80 | Comparar executores SG-1000/SMS, projetar e prototipar o crate compartilhado, adaptadores e compatibilidade semântica | Inventário comparativo, protótipo em branch própria, testes diferenciais |
| A3 | Engenheiro temporal e migração Rust | Temporal Core, scheduler headless, micro-operações, fronteiras PyO3 e inventário de dependências Python de hardware | Protótipos temporais, traces, plano de migração de runtime |
| A4 | Supervisor independente de lacunas | Procurar lacunas semânticas, arquiteturais, temporais, de segurança, CI e cobertura; contestar premissas e evidências | Relatório de gaps, riscos, severidade, evidência e recomendação PASS/FAIL/BLOCKED |

A4 não implementa funcionalidades, não altera código e não é coautor dos protótipos que audita. Deve receber os critérios de aceitação e os SHAs/branches exatos, não apenas resumos dos agentes.

### 1.2 Concorrência e propriedade de arquivos

- A1 é proprietário dos documentos de arquitetura/ADRs.
- A2 é proprietário do protótipo Z80 e dos adaptadores durante a fase de extração.
- A3 é proprietário dos crates temporais e do inventário de fronteiras Python/Rust.
- A4 é somente leitura e registra achados em relatório/issue.
- A1 não altera código de A2/A3; solicita mudanças por revisão.
- A2 e A3 não editam os mesmos arquivos. Contratos compartilhados são propostos por A1 e aprovados antes da implementação.
- A4 é ativado em gates formais, com A3 ou outro agente de execução pausado para manter o limite de três.
- O coordenador consolida e publica. Nenhum agente tem autorização autônoma para merge.

## 2. Cronograma macro

Estimativas em semanas de trabalho técnico, não datas prometidas. O calendário pode variar conforme lacunas encontradas. Fases podem ter protótipos paralelos somente quando os contratos e a propriedade dos arquivos estiverem definidos.

| Fase | Janela estimada | Objetivo | Responsáveis | Gate de saída |
|---|---:|---|---|---|
| F0 — Baseline e inventário | Semana 1 | Auditar SHAs, PRs, crates, dependências, contratos, testes e workflows | A1 + A2 + A3; A4 revisa | Inventário reproduzível e gaps priorizados |
| F1 — Contratos e ADRs | Semanas 1–2 | Definir Clean Architecture, API do Z80, bus trait, profiles e modelo temporal | A1; contribuições A2/A3 | ADRs aprovados; sem ambiguidade de ownership |
| F2 — Protótipo do Z80 compartilhado | Semanas 2–4 | Extrair sem alterar semântica; adaptadores SG-1000/SMS | A2; A1 revisa | Mesmo crate usado pelos dois cores e regressões preservadas |
| F3 — Temporal Core e scheduler headless | Semanas 2–5 | Consolidar T0/T1, fila determinística, clock domains e scheduler de máquina | A3; A1 define contratos | Replay determinístico e scheduler sem dependência de Qt/SDL |
| F4 — Integração CPU/barramento | Semanas 4–7 | Introduzir M-cycles, eventos de barramento e execução retomável | A2 + A3, com fronteiras acordadas | Traces fixos e equivalência com modo por instrução |
| F5 — Perfis de plataforma | Semanas 6–8 | Adaptadores SMS/Game Gear e desenho do Z80 auxiliar Genesis | A2 + A1 | Sem condicionais de console no decoder; gaps por plataforma documentados |
| F6 — Migração de dependências | Semanas 5–9 | Remover hardware Python ativo, consolidar PyO3 e revisar input/vídeo/áudio | A3; A1 supervisiona | Caminho de produção sem runtime de hardware Python |
| F7 — Validação e estabilização | Semanas 9–10+ | Regressão, determinismo, stress, integração, build Release e docs finais | A2 + A3; A4 audita | Gates técnicos aprovados no SHA candidato |

**Nota de sequenciamento:** F2 e F3 podem avançar em paralelo após F1, pois o Z80 e o scheduler headless têm superfícies de trabalho distintas. F4 depende dos contratos de ambas. F5 não significa que Game Gear ou Mega Drive estarão funcionalmente implementados ao fim da janela; significa que seus perfis/adaptadores e limites estarão definidos e testados no escopo realmente implementado. Mega Drive completo é um marco posterior de máquina, incluindo 68000 e arbitragem, não um subproduto da extração Z80.

## 3. Plano detalhado por fase

### F0 — Baseline e inventário

- [ ] Confirmar HEAD remoto de `main` e SHAs de todas as PRs abertas.
- [ ] Registrar merge-base, commits exclusivos, arquivos alterados e checks por PR.
- [ ] Comparar CPU SG-1000/SMS: decoder, estado, flags, WZ/MEMPTR, R, interrupções, prefixos, HALT e barramento.
- [ ] Inventariar traits de memória/I/O, snapshots, traces, testes e fixtures.
- [ ] Mapear dependências Python que executam hardware e separar dependências legítimas de UI/tooling.
- [ ] Mapear dependências Cargo, features e crates excluídos do workspace.
- [ ] Identificar workflows com filtros de paths e checks que não executam em cada tipo de alteração.
- [ ] Classificar gaps por severidade, impacto, evidência e responsável.

**Aceitação:** baseline versionado; nenhuma decisão de extração baseada apenas em similaridade textual; todos os contratos públicos conhecidos inventariados.

### F1 — Clean Architecture e contratos

Definir as camadas:

1. **Domain:** estado e semântica de CPU/dispositivos, sem dependência de host.
2. **Application:** casos de uso de máquina, scheduler, stepping, reset, save-state e replay.
3. **Ports:** contratos de barramento, clock, interrupções, input, áudio e vídeo.
4. **Adapters:** SG-1000, SMS, Game Gear, Mega Drive, SDL3, Qt/PyO3 e APIs de host.
5. **Infrastructure:** backend de áudio/vídeo/input, logging, persistência e empacotamento.

Regras:

- Domain não importa Qt, SDL, PyO3, filesystem ou APIs do SO.
- Application coordena entidades e ports, não implementa drivers de host.
- Adapters implementam ports e não duplicam semântica de CPU.
- Dependências apontam para dentro; interfaces são definidas no lado consumidor.
- Nenhum callback Python por instrução, acesso de barramento ou amostra de áudio.
- Não criar abstrações genéricas sem consumidor concreto e teste.

**Aceitação:** ADRs revisados, grafo de dependência sem ciclos, ownership explícito e API pública mínima.

### F2 — Z80 compartilhado

- [ ] Selecionar implementação canônica após comparação semântica, não por preferência.
- [ ] Criar crate `native/z80_cpu` / `sega-z80-cpu`, distinto da extensão PyO3 `native/z80_core`.
- [ ] Extrair decoder, estado, ALU e semântica sem alterar comportamento.
- [ ] Definir Bus port neutro para memória e I/O.
- [ ] Converter SG-1000 e SMS em adaptadores finos.
- [ ] Preservar fixtures, testes, matrizes e proveniência.
- [ ] Executar vetores de ambos os cores contra o mesmo motor.
- [ ] Remover cópias antigas somente após uso confirmado do crate compartilhado e regressões aprovadas.

**Aceitação:** exatamente uma implementação de produção do decoder/semântica Z80; ambos os cores usam o crate; nenhuma regressão não explicada.

### F3 — Temporal Core e scheduler

- [ ] Auditar a PR #315 e seu SHA atual antes de integrar.
- [ ] Validar MachineTime, ClockRatio, ClockDomain e EventQueue.
- [ ] Definir política estável de desempate e delta-cycle.
- [ ] Implementar MachineScheduler headless por máquina.
- [ ] Testar clocks racionais, overflow, ordenação, eventos simultâneos e replay.
- [ ] Integrar dispositivos sintéticos antes de hardware real.
- [ ] Manter o scheduler independente de pacing Qt e dispositivos host.

**Aceitação:** replay idêntico produz sequência idêntica; nenhum relógio do host avança o hardware; eventos simultâneos têm ordem documentada e testada.

### F4 — Execução temporal Z80

- [ ] Modelar M1, refresh, memória, I/O, ciclos internos e acknowledge.
- [ ] Representar WAIT e arbitragem como eventos/condições explícitas.
- [ ] Criar API de step/resume em fronteiras seguras.
- [ ] Garantir que modo por instrução e modo temporal compartilhem semântica.
- [ ] Comparar traces com vetores independentes e documentação ZiLOG.
- [ ] Validar INT/NMI, EI delay, HALT, IM0/1/2 e BUSREQ/BUSACK.

**Aceitação:** nenhuma afirmação de ciclo a ciclo baseada somente em T-states agregados; traces têm oráculo, limites e primeira divergência investigável.

### F5 — Perfis de plataforma

- [ ] SG-1000: manter adaptador e regressões existentes.
- [ ] SMS: manter slots, portas, VDP e particularidades de máquina fora do decoder.
- [ ] Game Gear: documentar diferenças de mapa, portas, VDP e áudio antes de habilitar o perfil.
- [ ] Genesis: definir Z80 auxiliar, ownership/request/grant, RAM e integração com 68000.
- [ ] Não inserir frequências, prioridades ou tempos sem fonte verificável.

**Aceitação:** cada diferença pertence à CPU silicon profile, ao bus adapter ou ao scheduler da máquina, com justificativa e testes.

### F6 — Runtime Rust e dependências

- [ ] Inventariar imports Python em runtime, entry points, plugins e scripts.
- [ ] Classificar cada dependência como UI, tooling, test-only ou hardware runtime.
- [ ] Migrar hardware ativo para Rust com fronteiras coarse-grained.
- [ ] Reconciliar #300/#313 para evitar dois runtimes de input.
- [ ] Reconciliar #301 com a arquitetura de vídeo e lifecycle Qt.
- [ ] Desacoplar scheduler de pacing Qt e saída de áudio.
- [ ] Manter Python/PySide6 para UI, ROM selection/loading, configuração, logs e ferramentas.
- [ ] Remover pacotes Python somente com prova de ausência de import em produção e teste do pacote instalado.

**Aceitação:** nenhum hardware emulado é executado por Python; a UI não bloqueia a máquina; ownership, GIL e sincronização estão explícitos.

### F7 — Validação e estabilização

Executar no SHA candidato exato:

**Rust**
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`

**Python**
- `python -m pytest`
- `python -m ruff check .`
- `python -m ruff format --check .`
- `python -m compileall`
- `python -m pip check`

**Integração**
- Build Release/Maturin e import da extensão recém-construída.
- Testes de fronteira PyO3.
- Testes de replay determinístico.
- Testes de integração SG-1000/SMS.
- Testes no emulador quando dependentes de hardware/ambiente.
- Workflows e filtros de paths no SHA exato.

Cada verificação é PASS, FAIL, BLOCKED ou NOT RUN. Se lint impedir pytest, pytest fica NOT RUN. Não transferir resultados de um SHA para outro.

## 4. Governança das PRs existentes

| PR | Ação de planejamento |
|---|---|
| #315 Temporal Core | Preservar como candidata T0; auditar código/checks atuais e reconciliar com o scheduler alvo |
| #314 Z80 boundary test | Preservar como vetor de regressão; atualizar base e revalidar após extração |
| #313 Input Rust/SDL3 | Principal candidata de input; revisar escopo e dependências antes de rebase |
| #300 Input legado | Comparar com #313; evitar manter dois caminhos de runtime |
| #301 Vídeo Rust | Reavaliar base antiga, ownership de recursos e integração Qt |
| #298 Modelo Qt | Preservar decisões funcionais; reconciliar documentação sem descartar conteúdo |
| #297 Auditoria ROM SMS | Preservar como ferramenta de diagnóstico; atualizar base e validar imports |

Nenhuma PR será fechada, rebaseada, alterada ou integrada automaticamente por este plano. Cada uma requer revisão de commits, arquivos, conflitos e checks no novo SHA.

## 5. Gates independentes do supervisor A4

A4 deve auditar em cada marco:

1. **Arquitetura:** dependências apontam para dentro, ausência de ciclos e de lógica de hardware no frontend.
2. **Semântica Z80:** opcodes/prefixos, flags, WZ, R, interrupções, reset, HALT e casos-limite.
3. **Temporalidade:** M-cycles, T-states, refresh, WAIT, arbitragem, ordem de eventos e relógios racionais.
4. **Plataformas:** diferenças SG-1000/SMS/Game Gear/Genesis atribuídas à camada correta.
5. **Migração:** ausência de segundo runtime Python, callbacks/cópias indevidas e ownership implícito.
6. **Testes:** cobertura real, oráculos independentes, proveniência e determinismo.
7. **CI:** workflows aplicáveis, filtros, artefatos, SHA e logs.
8. **Documentação:** alegações correspondem ao código e aos checks; estados não são promovidos sem evidência.

O relatório deve conter: achado, severidade (BLOCKER/HIGH/MEDIUM/LOW), componente, evidência reproduzível, impacto, critério de correção e estado. A4 não corrige o próprio achado; devolve ao agente responsável.

## 6. Definition of Done global

A reorganização só é considerada concluída quando:

- [ ] Um único crate Z80 executa SG-1000 e SMS.
- [ ] Game Gear tem perfil/adaptador testado no escopo declarado.
- [ ] Genesis tem arquitetura de máquina definida sem confundir Z80 auxiliar e 68000 principal.
- [ ] MachineScheduler é proprietário da timeline de cada máquina.
- [ ] CPU e dispositivos avançam por eventos temporais verificáveis.
- [ ] Runtime de hardware é 100% Rust.
- [ ] Python permanece somente nas fronteiras aprovadas de frontend/ferramentas.
- [ ] PyO3/Maturin tem ownership e chamadas coarse-grained documentados.
- [ ] Testes, CI e evidências de gameplay estão vinculados ao SHA exato.
- [ ] A4 não mantém blockers sem resolução ou aceitação formal de risco.
- [ ] README, arquitetura, ADRs, matriz de cobertura e roadmap refletem o código real.

Estados do projeto continuam separados: `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING`, `TEMPORAL_PASS`. O cronograma não promove nenhum estado automaticamente.

## 7. Próxima iteração imediata

1. A1 abre o inventário Z0 e os ADRs de Clean Architecture.
2. A2 produz relatório comparativo de SG-1000/SMS e proposta de API pública para `sega-z80-cpu`, sem editar o runtime ainda.
3. A3 audita #315 e desenha a API headless de MachineScheduler; em paralelo, mapeia dependências Python de hardware.
4. Pausar A3 e ativar A4 para revisão independente do inventário, contratos propostos e riscos.
5. Corrigir os blockers, aprovar F1 e só então abrir os protótipos executáveis em branches isoladas.

**Primeiro marco de entrega:** inventário Z0 + ADRs F1 + relatório A4. O marco deve encerrar com decisão documentada sobre implementação canônica Z80, API do barramento, ownership do scheduler e lista priorizada de gaps. Nenhuma alteração de runtime entra em `main` antes desse gate.
