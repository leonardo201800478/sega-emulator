# Plano de Execução — Integração Temporal do Master System 315-5124

**Status:** PLANEJADO — execução ainda não iniciada  
**Baseline remoto auditado:** `3a510ae68028228065f7e60350a821ef9f76c882`  
**Branch documental:** `docs/roadmap-sms1-temporal-integration`  
**Escopo:** Master System 1 (VDP 315-5124), preservando SG-1000 e os caminhos legados até haver evidência para removê-los.

> Este documento transforma a auditoria arquitetural em uma sequência executável. Ele não declara que os itens planejados estão implementados, testados ou aprovados. O estado de cada entrega deve ser atualizado somente com evidência no SHA candidato.

## 1. Objetivo final

Entregar uma máquina SMS1 em que o Master Clock Scheduler seja a única autoridade de tempo emulado e controle, de forma causal e determinística:

- Z80 interrompível e retomável em qualquer fronteira temporal suportada;
- transações reais de memória e I/O, com WAIT e arbitragem quando aplicáveis;
- VDP Sega 315-5124 ativo, com raster e pipeline temporal;
- SN76489 sincronizado ao mesmo relógio mestre;
- linha IRQ do VDP conectada ao mecanismo de interrupção do Z80;
- saída de frames do VDP para VideoFrame/FrameMailbox e consumidores de vídeo;
- snapshots coordenados, versionados e restaurados atomicamente;
- traces reproduzíveis e comparação com oráculos independentes.

O marco final é **TEMPORAL_PASS**. Contagem agregada de T-states, avanço por scanline ou execução por instrução não constituem, isoladamente, prova temporal.

## 2. Baseline e decisões de controle

### 2.1 Baseline conhecida

- Repositório: `leonardo201800478/sega-emulator`.
- SHA remoto auditado nesta rodada: `3a510ae68028228065f7e60350a821ef9f76c882`.
- PR #393: MERGED — H Counter latch.
- PR #394: MERGED — consolidação Scheduler/SMS1 VDP/D3D12.
- PRs documentais #365, #366, #367 e #378 permanecem como fontes documentais abertas; não devem ser integradas sem comparação de seus diffs contra a documentação consolidada e o código atual.

O SHA deve ser novamente consultado antes de cada implementação. Se `main` avançar, a branch de trabalho deve ser atualizada por integração segura; nunca por force-push ou reset destrutivo.

### 2.2 Fatos arquiteturais observados na auditoria

| Área | Situação observada | Consequência |
|---|---|---|
| Z80 | `Z80::step()` / `step_inner()` executam instruções inteiras; o barramento é síncrono | Construir executor interrompível sem duplicar decoder/ALU |
| Bus | `CpuBus` expõe leituras/escritas síncronas | Criar contrato de transação temporal |
| Scheduler | Clock ratios, fases, fila, ordenação, budget e snapshot já existem | Integrar; não reescrever isoladamente |
| SMS Machine | `SmsMachine` ainda compõe o VDP legado | Selecionar o 315-5124 explicitamente para perfil SMS1 |
| SMS1 VDP | Portas, VRAM/CRAM, composição Mode 4 e latch já existem | Evoluir para dispositivo temporal; não confundir composição com pipeline |
| PSG | `advance_clocks()` recebe quantidades agregadas | Agendar avanço e escritas com timestamps |
| IRQ | Cadeia atual usa o VDP legado | Propagar IRQ do novo VDP à CPU |
| Video Runtime | `VideoFrame` e `FrameMailbox` existem | Conectar frames produzidos pelo SMS1 VDP |
| Python/PyO3 | O frontend ainda solicita `run_frame()`; binding executa loop de instruções | Mover execução contínua para engine Rust; Python apenas controla/observa |
| CI | Gates básicos verdes no SHA auditado; stress Scheduler foi SKIPPED | Tornar os testes relevantes explícitos e registrar SKIPPED como não aprovado |

Esses pontos são o retrato da auditoria no SHA indicado, não uma afirmação sobre SHAs futuros.

### 2.3 Regras invariáveis

1. Runtime digital e avanço temporal exclusivamente em Rust.
2. Scheduler único por máquina; nenhum chip mantém scheduler independente.
3. CPU e barramento no hot path com generics/monomorfização; evitar dispatch dinâmico por operação.
4. Python/Qt é plano de controle, configuração, logs e telemetria somente leitura.
5. PyO3 não oferece stepping por instrução, callback por instrução ou avanço por frame como autoridade temporal.
6. Renderer e GPU consomem frames; nunca determinam o tempo emulado.
7. Sem JIT, frame-skip, paralelismo especulativo ou otimização sem baseline.
8. Separar PR estrutural de PR que altera semântica do hardware.
9. Preservar SG-1000 e VDP legado até regressões e evidências autorizarem remoção.
10. Não versionar ROM comercial, BIOS proprietária ou dumps protegidos.

## 3. Organização das trilhas

A plataforma não confirmou execução real de subagentes paralelos nesta auditoria. As três trilhas abaixo são responsabilidades especializadas coordenadas pelo responsável principal; não se deve alegar paralelismo inexistente.

### Trilha A — Hardware e temporalidade

Responsável por CPU Z80, Bus, Scheduler, VDP SMS1, PSG, IRQ, mappers, memória e composição da máquina.

Arquivos centrais observados:
- `native/sms_core/src/cpu.rs`
- `native/sms_core/src/bus.rs`
- `native/z80_bus/src/lib.rs`
- `native/sms_core/src/machine.rs`
- `native/sms_core/src/io.rs`
- `native/sms_core/src/vdp_sms1.rs`
- `native/sms_core/src/vdp.rs`
- `native/sms_core/src/psg.rs`
- `native/sms_core/src/memory.rs`
- `native/machine_scheduler/`
- `native/temporal_core/`

### Trilha B — Oráculos e auditoria independente

Responsável por manuais, proveniência, vetores, divergências, revisão semântica e parecer das PRs da Trilha A. Não altera runtime.

Fontes prioritárias:
- UM0080 e Z80 CPU Technical Manual anexados;
- documentação SMS1/315-5124 e manuais de serviço;
- SMS Power;
- esquemas e documentação técnica disponíveis;
- implementações independentes somente como referências secundárias, com revisão de proveniência.

Toda afirmação técnica deve registrar: comportamento, fonte, página/seção/endereço, revisão de hardware, confiança, divergências e itens UNKNOWN.

### Trilha C — Integração, testes e evidências

Responsável por harnesses, testes entre componentes, golden masters, replay, snapshots, CI, Windows x64/D3D12 e auditoria da fronteira Python/PyO3. Alterações semânticas de domínio dependem de aprovação do coordenador.

## 4. Etapas de execução

A ordem abaixo parte do estado atual do repositório. Não reinicia as fases de port estrutural P0–P8 do roadmap histórico quando seus resultados já existem. A auditoria P0 é mantida como gate recorrente de baseline. A prioridade presente é fechar a integração temporal SMS1.

### ETAPA 0 — Revalidar baseline e preparar evidências

**Objetivo:** assegurar que todos os ciclos partam do estado remoto correto e que regressões possam ser detectadas.

**Tarefas**
- Consultar SHA atual de `main`, commits posteriores, branches, PRs e workflows.
- Comparar alterações desde o SHA registrado neste documento.
- Atualizar inventário de crates, APIs, dependências, testes e workflows.
- Reconciliar #365, #366, #367 e #378 com #394; manter documentos úteis sem duplicar conteúdo.
- Confirmar os testes existentes e os que são ignorados por falta da extensão nativa.
- Preparar/confirmar harness de replay com entradas reproduzíveis e hashes de estado, vídeo e PCM.
- Capturar duas execuções idênticas dos cenários de referência SG-1000 e SMS1.
- Registrar ROM homebrew/legal por nome, hash e passos locais; nunca adicionar a ROM ao Git.

**Entregáveis**
- Inventário atualizado por SHA.
- Matriz de componentes e dependências.
- Manifesto de cenários e oráculos.
- Golden masters reproduzíveis.
- Registro de riscos e lacunas.

**Aceitação**
- Mesmo cenário e mesma entrada produzem hashes idênticos em duas execuções.
- Regressões SG-1000 e SMS existentes aprovadas.
- Lacunas classificadas como código ausente, integração pendente, teste ausente, evidência ausente ou bloqueio externo.

**Saída:** BASELINE_READY.

### ETAPA 1 — Contratos temporais compartilhados

**Objetivo:** aprovar as interfaces que CPU, barramento e Scheduler usarão antes de implementar o executor interrompível.

**Tarefas**
- Especificar timestamp inteiro/racional e preservação de fase.
- Definir tipos de M-cycle, T-state, fase de barramento, direção, endereço, dados e tipo de acesso.
- Definir resultado de operação: concluída, aguardando WAIT, suspensa ou erro.
- Definir causalidade e regra contra eventos retroativos.
- Definir ordenação determinística para eventos simultâneos: timestamp, hardware phase, prioridade e sequência estável.
- Definir como o Scheduler solicita progresso e como um dispositivo devolve o próximo instante de evento.
- Definir contratos de reset, snapshot e restore para estado parcial.
- Registrar ADR dedicado se o contrato exigir alteração em crate compartilhado.

**Entregáveis**
- ADR do contrato temporal.
- Tipos/protocolos aprovados.
- Diagramas de sequência CPU–Bus–Scheduler.
- Vetores mínimos de ordenação e causalidade.

**Aceitação**
- Nenhum contrato depende de T-states agregados como execução.
- Eventos retroativos são rejeitados.
- Avanços equivalentes preservam estado e fase.
- O hot path não introduz dispatch dinâmico por operação.

**Dependência:** nenhuma implementação do microsequenciador deve cristalizar uma interface não aprovada.

### ETAPA 2 — Z80 interrompível (primeiro caminho crítico)

**Objetivo:** permitir que a CPU avance somente até a próxima operação temporal e retome exatamente do estado parcial.

**Tarefas**
- Inventariar decoder, ALU, flags, WZ/MEMPTR, Q/P, I/R, prefixos e interrupções existentes.
- Preservar o decoder e a semântica já validada; evitar um segundo executor paralelo.
- Introduzir estado explícito de execução: boundary, opcode fetch, prefix decode, operand access, internal cycle, I/O, refresh, interrupt acknowledge e completion.
- Representar micro-operações reais por família de instrução.
- Implementar M1, refresh, memory read/write, I/O read/write e ciclos internos conforme documentação.
- Implementar WAIT como suspensão no ciclo correspondente.
- Preservar estado parcial de BASE, CB, ED, DD/FD e DDCB/FDCB.
- Tratar HALT, EI delay, INT, NMI, IM0/1/2, RETI e RETN sem perder o estado de execução.
- Manter `step()` apenas como fachada de diagnóstico construída sobre o executor compartilhado.
- Não declarar cobertura completa por simplesmente executar todos os opcodes.

**Testes de aceitação**
- Suspender e retomar em cada tipo de ciclo.
- Comparar execução contínua com execução dividida em intervalos arbitrários.
- Testar WAIT em leitura, escrita e I/O.
- Testar prefixos e interrupções em fronteiras válidas.
- Comparar estado arquitetural com a implementação de referência anterior onde a semântica deve permanecer igual.
- Executar vetores independentes e registrar diferenças.

**Entregáveis**
- Microsequenciador.
- Estado serializável da instrução parcial.
- Trace de micro-operações.
- Suíte de testes unitários e de retomada.

**Aceitação:** nenhuma operação de barramento futura é executada antes de o Scheduler alcançar seu instante.

**Estado de saída:** Z80_STEP_READY; TEMPORAL_PASS ainda proibido.

### ETAPA 3 — SMS Bus temporal

**Objetivo:** fazer toda operação de CPU atravessar o contrato temporal de barramento.

**Tarefas**
- Adaptar o contrato síncrono atual sem removê-lo até migrar os consumidores.
- Implementar transação com timestamp, endereço, dados, direção, tipo, duração, fase e wait states.
- Integrar ROM, RAM, SRAM, VRAM, portas, mapper e registradores com semântica correta.
- Definir arbitragem e contenção somente onde houver evidência técnica.
- Garantir que a conclusão de uma transação gere o resultado esperado para a CPU.
- Definir comportamento de reset durante transações e snapshots de transação pendente.
- Impedir efeitos colaterais antes do instante de conclusão.

**Testes de aceitação**
- Leitura/escrita com timestamps conhecidos.
- WAIT suspende e retoma sem duplicar efeitos colaterais.
- Ordenação causal entre CPU, I/O e VDP.
- Replay idêntico de trace de barramento.
- Testes de mapa de memória, espelhamento e mappers.

**Entregáveis:** adaptador temporal SMS, trace de barramento e vetores de leitura/escrita.

**Estado de saída:** BUS_TEMPORAL_READY.

### ETAPA 4 — Integração da máquina ao Master Scheduler

**Objetivo:** tornar o Scheduler a única autoridade temporal da máquina SMS.

**Tarefas**
- Integrar `sega-machine-scheduler` e `sega-temporal-core` à composição SMS por dependências explícitas.
- Definir perfil regional e relações racionais dos domínios Z80, VDP e PSG.
- Registrar eventos de CPU, barramento, raster, I/O, PSG e IRQ.
- Definir avanço até timestamp e até próximo evento.
- Preservar fase entre chamadas e em avanços fracionados.
- Definir política para eventos simultâneos e delta-event budget.
- Remover qualquer avanço temporal independente dos chips.
- Manter API de granularidade grossa como fachada sobre `run_until`; stepping por instrução é ferramenta de debug.

**Testes de aceitação**
- Um avanço de N clocks equivale a N avanços de um clock.
- Dividir o avanço em blocos arbitrários não muda o estado final.
- Repetir o mesmo trace produz sequência idêntica de eventos.
- Nenhum evento retroativo.
- Reset reproduz o mesmo estado inicial e fase.

**Entregáveis:** SMS temporalmente dirigido pelo Scheduler e traces de integração.

**Estado de saída:** SMS_SCHEDULER_READY.

### ETAPA 5 — SMS1 VDP 315-5124 ativo e temporal

**Objetivo:** fazer o modelo SMS1 ser o VDP efetivo da máquina selecionada e evoluir de composição funcional para pipeline temporal.

**Tarefas em subfases**
1. **Seleção e reset:** conectar `vdp_revision`/perfil SMS1 à composição; manter VDP legado disponível.
2. **Raster:** definir geometria, linha, dot, blanking e contadores conforme documentação do 315-5124.
3. **H/V counters:** implementar fase, leitura, latch e comportamento de borda.
4. **Fetch:** implementar slots, endereçamento, read-ahead e acesso à VRAM no instante apropriado.
5. **Background:** pipeline de tiles, atributos, scroll e shifters.
6. **Sprites:** avaliação, fetch, limites, overflow e collision com fase temporal.
7. **Status e IRQ:** atualização de status e geração de interrupções no instante correto.
8. **TH/H Counter latch:** detectar transições reais de TH no I/O e capturar o contador no instante correto.
9. **Frame completion:** publicar frame somente quando o raster completar o quadro previsto.

**Regras**
- Não misturar SMS1 315-5124 com SMS2 315-5246 ou Game Gear.
- Não inferir comportamento elétrico ausente de documentação.
- Marcar UNKNOWN quando a evidência for insuficiente.
- Não chamar composição por scanline de pipeline temporal.

**Testes de aceitação**
- Cenas sintéticas por recurso e por borda.
- Contadores e status em timestamps conhecidos.
- Fetches e contenção verificados por trace.
- Sprites, scroll, collision e overflow comparados com oráculos.
- Regressão do VDP legado e SG-1000.
- Comparação de framebuffers com proveniência documentada.

**Entregáveis:** VDP SMS1 temporal, cenas sintéticas, trace de raster e matriz de evidências.

**Estado de saída:** SMS1_VDP_TEMPORAL_READY.

### ETAPA 6 — PSG, I/O e IRQ integrados

**Objetivo:** sincronizar áudio e interrupções com o mesmo relógio mestre.

**Tarefas**
- Avançar SN76489 por eventos/fases do Scheduler, não por soma tardia de clocks.
- Timestamp das escritas nos registradores e aplicação no instante correto.
- Preservar divisor, fase, contadores, latch, LFSR e estado de reamostragem.
- Conectar saída IRQ do VDP à linha INT da CPU.
- Implementar propagação, acknowledge e limpeza de IRQ com causalidade.
- Preservar NMI de entrada externa separadamente.
- Revisar reset e snapshot do PSG e I/O.

**Testes de aceitação**
- Tons, ruído e LFSR em vetores determinísticos.
- Escritas próximas a bordas de clock.
- IRQ gerada e reconhecida em timestamps conhecidos.
- EI delay e HALT combinados com IRQ.
- Replay PCM e estado do PSG idênticos.

**Entregáveis:** integração PSG/IRQ, vetores de áudio e trace de interrupção.

**Estado de saída:** AUDIO_IRQ_TEMPORAL_READY.

### ETAPA 7 — Vídeo runtime e consumidores

**Objetivo:** publicar frames reais produzidos pelo SMS1 VDP sem permitir que a GPU controle a emulação.

**Tarefas**
- Conectar saída do VDP ao `VideoFrame` e `FrameMailbox`.
- Validar dimensões, stride, formato e sequência.
- Validar RGB565/RGBA8888 e conversão de pixels.
- Definir política para frame incompleto, reset e troca de mídia.
- Garantir publicação sem bloqueio do Scheduler pela GPU.
- Integrar D3D12 como consumidor; renderer não determina raster nem clock.

**Testes de aceitação**
- Frame sintético e frame de ROM homebrew.
- Sequência e hashes reproduzíveis.
- Validação de stride e limites de buffer.
- Consumidor lento não bloqueia o tempo emulado.
- Build Windows x64/D3D12 e smoke test quando o ambiente permitir.

**Entregáveis:** frames SMS1 reais no runtime e relatório de renderer.

**Estado de saída:** SMS1_VIDEO_READY.

### ETAPA 8 — Save states coordenados e replay

**Objetivo:** restaurar a máquina no mesmo instante lógico, inclusive quando há trabalho parcial pendente.

**Tarefas**
- Definir schema versionado para CPU, instrução parcial, Bus, Scheduler, fila de eventos, VDP, PSG, I/O, mapper e memórias.
- Derivar Serde quando compatível.
- Validar invariantes, referências e timestamps durante restore.
- Fazer restore atômico: falha não altera a máquina anterior.
- Definir compatibilidade/migração de versões.
- Incluir eventos pendentes e fases fracionárias.

**Testes de aceitação**
- Snapshot/restore em boundary de instrução.
- Snapshot durante micro-operação e transação pendente.
- Snapshot no meio de scanline e com eventos na fila.
- Replay após restore idêntico ao trace original.
- Restore inválido rejeitado sem mutação parcial.

**Entregáveis:** contrato de snapshot, testes de replay e matriz de compatibilidade.

**Estado de saída:** SMS1_STATE_READY.

### ETAPA 9 — Fronteira Engine, PyO3 e Python

**Objetivo:** retirar o Python do controle temporal e garantir autonomia do runtime Rust.

**Tarefas**
- Transferir execução contínua para thread/sessão Rust.
- Definir comandos assíncronos de alto nível: Load, Reset, Pause, Resume, Stop, SetOption e Shutdown.
- Remover avanço por QTimer/loop Python como autoridade.
- Reduzir PyO3 a controle e telemetria somente leitura.
- Não expor callbacks por instrução, pixel, amostra ou frame.
- Garantir que fechar/travar painel não interrompa emulação nem cause underrun.
- Revalidar GUI, logs, input e configuração sem lógica de hardware em Python.

**Testes de aceitação**
- Runtime Rust opera sem Python instalado.
- Painel conecta/desconecta sem interromper sessão.
- Comandos idempotentes e assíncronos.
- Teste de queda do controle.
- Import-linter e regras arquiteturais impedem dependências proibidas.
- Build e smoke test real da extensão PyO3.

**Entregáveis:** contrato EngineControl, teste de isolamento e relatório de fronteira.

**Estado de saída:** RUST_RUNTIME_ISOLATED.

### ETAPA 10 — Conformidade independente e fechamento semântico

**Objetivo:** distinguir funcionamento observado de conformidade semântica.

**Z80**
- Rodar variantes aplicáveis de z80test: z80full, z80flags, z80doc, z80docflags, z80ccf e z80memptr.
- Rodar ZEXDOC/ZEXALL quando ambiente e harness permitirem.
- Usar SingleStepTests quando o modelo de ciclo e a proveniência forem compatíveis.
- Registrar falhas conhecidas, limitações e variante exata.

**VDP**
- Cenas sintéticas independentes para raster, fetch, scroll, sprites, status e IRQ.
- Comparação de framebuffer e trace, não apenas screenshot visual.
- Separar evidência SMS1, SMS2 e Game Gear.

**Bus/IRQ/PSG**
- Vetores independentes de transações, WAIT, acknowledge, áudio e fases.

**Entregáveis**
- Manifesto de oráculos e hashes.
- Relatório de divergências.
- Primeiro ponto de divergência por trace.
- Parecer da trilha independente.

**Aceitação:** toda afirmação de precisão tem fonte, revisão de hardware, cenário reproduzível e grau de confiança.

### ETAPA 11 — Integração total, stress e TEMPORAL_PASS

**Objetivo:** provar a máquina SMS1 completa sob controle do Scheduler.

**Testes obrigatórios**
- Z80 ↔ Bus.
- Z80 ↔ Scheduler.
- Bus ↔ VDP.
- Scheduler ↔ VDP.
- VDP ↔ IRQ ↔ Z80.
- Bus ↔ PSG.
- Scheduler ↔ PSG.
- VDP ↔ VideoFrame.
- SMS completo.
- Reset, save/load, replay e troca de mídia.
- Avanços fracionados e chunking arbitrário.
- Stress determinístico e long-run.
- SG-1000 preservado.

**TEMPORAL_PASS exige cumulativamente**
1. Scheduler integrado como autoridade única.
2. Z80 interrompível e retomável.
3. Bus temporal com transações reais.
4. SMS1 315-5124 ativo no perfil SMS1.
5. PSG sincronizado.
6. IRQ integrada.
7. Frames reais produzidos pelo VDP e consumidos pelo runtime.
8. Snapshots coordenados e replay consistente.
9. Traces determinísticos comparados com evidência independente.
10. Gates de CI aprovados no mesmo SHA candidato.

**Saída:** TEMPORAL_PASS somente após revisão final e registro do SHA.

## 5. Gates de qualidade por SHA candidato

### Rust
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`

### Python
- `python -m pytest`
- `python -m ruff check .`
- `python -m ruff format --check .`

### Windows x64 / D3D12 / PyO3
- Build do alvo Windows MSVC.
- Testes do renderer.
- Build/teste de extensão nativa.
- Smoke test da sessão SMS1.
- GPU física somente quando disponível; build headless não equivale a validação de GPU física.

### Testes especializados
- Scheduler stress short/long.
- Z80 external conformance.
- Golden masters e replay.
- Long-run determinístico.
- Testes de restore atômico.

Cada gate recebe exatamente um estado:
- **PASS:** executado no SHA candidato e aprovado.
- **FAIL:** executado e falhou.
- **BLOCKED:** impedimento externo comprovado.
- **NOT RUN:** não executado.
- **SKIPPED:** registrar como ignorado; nunca converter em PASS.

Um gate ausente, ignorado ou executado em SHA diferente não aprova o candidato.

## 6. Política de PR e integração

- Nunca commitar diretamente em `main`.
- Cada fatia não trivial tem branch e PR dedicadas.
- PR pequena, com uma responsabilidade principal e critérios de aceitação explícitos.
- Mudança de contrato compartilhado exige PR dedicada e aviso às trilhas.
- Revisão independente da semântica antes da integração.
- Rodar testes direcionados antes da suíte ampla.
- Inspecionar logs de cada workflow.
- Não fazer merge sem aprovação explícita do usuário.
- Não usar force-push, reset destrutivo ou remoção prematura de branches.
- Verificar SHA remoto antes de cada ciclo.
- PR documental declara explicitamente que não altera runtime nem comprova gameplay.

## 7. Matriz de trabalho e rastreabilidade

| ID | Entrega | Etapa | Responsável | Teste de saída | Estado inicial |
|---|---|---:|---|---|---|
| SMS-00 | Baseline, branches, PRs e golden masters | 0 | C | Replay duplo e inventário por SHA | AUDIT PARCIAL |
| SMS-01 | Contrato temporal compartilhado | 1 | A + B | Vetores de timestamp/ordenação | NOT STARTED |
| SMS-02 | Z80 microsequencer | 2 | A; revisão B | Suspensão/retomada e vetores Z80 | INTEGRATION_PENDING |
| SMS-03 | Bus temporal | 3 | A; revisão B | Transações/WAIT/replay | INTEGRATION_PENDING |
| SMS-04 | SMS Scheduler integration | 4 | A + C | Chunking e traces | INTEGRATION_PENDING |
| SMS-05 | SMS1 raster/pipeline | 5 | A + B | Cenas sintéticas e traces | INTEGRATION_PENDING |
| SMS-06 | PSG/IRQ/TH | 6 | A + B | Vetores de áudio/IRQ | INTEGRATION_PENDING |
| SMS-07 | Frame runtime/D3D12 | 7 | A + C | Frame hash e smoke | INTEGRATION_PENDING |
| SMS-08 | Snapshot/replay atômico | 8 | A + C | Restore em estados parciais | INTEGRATION_PENDING |
| SMS-09 | Engine/PyO3/Python | 9 | C | Headless e teste de queda | INTEGRATION_PENDING |
| SMS-10 | Oráculos independentes | 10 | B + C | Relatório de conformidade | NOT STARTED |
| SMS-11 | Integração total | 11 | A + B + C | Gates e TEMPORAL_PASS | NOT STARTED |

O campo PR correspondente deve ser preenchido somente após criação real da PR. Não inventar números ou associar PR antiga a entrega nova sem comparar o diff.

## 8. Riscos e decisões ainda abertas

| Risco/decisão | Tratamento |
|---|---|
| Semântica temporal de operação ainda não comprovada | Marcar UNKNOWN e criar vetor/experimento |
| Diferenças entre SMS1 e SMS2 | Perfil explícito; nunca compartilhar constantes sem evidência |
| WAIT/arbitragem sem fonte suficiente | Não inventar atraso; documentar e bloquear o comportamento dependente |
| Scheduler exigir reescrita excessiva de chips | Revisar contrato via ADR antes de propagar adaptação |
| Snapshot em micro-operação | Incluir estado parcial e eventos pendentes |
| Extensão PyO3 ausente no CI | Adicionar build/teste explícito; pytest com importorskip não é prova |
| Golden master preservar bug | Declarar baseline como preservação; correção semântica exige oráculo e PR separada |
| Ambiente Windows/GPU indisponível | BLOCKED, nunca PASS por build headless |

Decisões de renderizador, pacer, processo PyO3/IPC e demais itens do roadmap geral permanecem sujeitas às ADRs existentes; não devem deslocar o caminho crítico SMS1 sem dependência demonstrada.

## 9. Registro de evidências

Criar/atualizar um registro por SHA candidato, preferencialmente em `docs/evidence/`, contendo:

- SHA e branch.
- PRs incluídas.
- Ferramenta, versão e comando.
- Ambiente (OS, target, toolchain, GPU quando aplicável).
- Resultado PASS/FAIL/BLOCKED/NOT RUN/SKIPPED.
- Logs ou artefatos de teste.
- Hashes de golden masters e traces.
- Proveniência e revisão do oráculo.
- Limitações conhecidas.
- Estado de cada componente: IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING ou TEMPORAL_PASS.

## 10. Próximo ciclo recomendado

A próxima execução deve começar pela **ETAPA 0**, atualizando o SHA remoto e verificando se o inventário desta auditoria continua válido. Em seguida, iniciar a **ETAPA 1** com uma PR dedicada ao contrato temporal. Após sua aprovação, implementar a **ETAPA 2 — Z80 interrompível**, que é o primeiro bloqueio estrutural do caminho crítico.

Não começar pipeline do VDP, PSG temporal ou IRQ antes de o contrato de CPU/Bus/Scheduler estar definido. Não remover o VDP legado. Não alterar semântica e arquitetura na mesma PR.

---

## Histórico

| Data | SHA | Evento |
|---|---|---|
| 2026-11-02 | `3a510ae68028228065f7e60350a821ef9f76c882` | Baseline da auditoria e criação deste plano |
