# PROMPT DE CONTINUIDADE — P0 FRAME PACING / AUDITORIA MULTIAGENTE RUST

Você está assumindo a investigação prioritária do **Sega Emulator — Emulação de Alta Precisão**.

Repositório: https://github.com/leonardo201800478/sega-emulator  
Issue principal: https://github.com/leonardo201800478/sega-emulator/issues/755  
PR de investigação: consulte a PR que adicionou este prompt.  
Baseline inicial: `744ebe8e55cbebe731e6393f992a2970d79a0aa1` (confirme o HEAD real da main antes de agir).  
Arquitetura: backend 100% Rust; Slint 1.18.x + WGPU 30.x + Vulkan + CPAL 0.18.2.

## 1. Mandato e bloqueio

Atue como Arquiteto de Software, Engenheiro de Emulação e líder de uma auditoria técnica multiagente especializada em Rust, concorrência, temporização e emulação cycle-accurate.

**Não avance nenhuma outra etapa do projeto até solucionar e validar a lentidão.** Não iniciar funcionalidades, novos sistemas, refactors amplos ou roadmap Z80 não relacionado. O escopo permitido é investigação P0, instrumentação não intrusiva, testes causais e correção mínima.

O sintoma apareceu após a introdução do MachineScheduler. Outros emuladores funcionam normalmente no mesmo equipamento e o Sega Emulator já executou em velocidade estável. O HUD variou aproximadamente entre 55 e 59 FPS. Capturas físicas pareadas indicaram ticks virtuais por frame estáveis (~59.735–59.736 em perfil NTSC), mas intervalo de worker acima do orçamento de ~16,687 ms. A captura vídeo-only não recuperou a cadência. Consulte o relatório P0 e os logs brutos; não trate hipótese como causa provada.

## 2. Criar uma equipe de agentes especialistas

Se o ambiente oferecer agentes/subagentes reais, crie agentes separados, atribua escopos sem sobreposição e exija relatórios independentes. Se não houver mecanismo de subagentes, simule as personas em trilhas independentes, identificando claramente que são análises do mesmo agente e não execuções paralelas reais.

### Agente A — Rust Runtime & Ownership Auditor
Especialista em Rust, ownership, borrowing, lifetimes, Send/Sync, canais, locks, atomics, drop/lifecycle, worker shutdown e concorrência. Auditar `SmsTemporalWorker`, comandos, filas, ownership da máquina e transições pause/resume/reset/load. Procurar bloqueios, starvation, contenda, operações síncronas e trabalho inesperado no caminho crítico.

### Agente B — Scheduler & Cycle-Accuracy Auditor
Especialista em emulação Z80/SMS, master clocks, divisores racionais, event scheduling, WAIT states, M-cycles/T-states e determinismo. Auditar MachineScheduler, domínios de clock, dispatch, avanço por frame, conversões e invariância contínuo-versus-blocos. Não modificar semântica para perseguir FPS.

### Agente C — Host Timing & Windows Concurrency Auditor
Especialista em Windows QPC, `Instant`, timer resolution, `recv_timeout`, waitable timers, thread scheduling, prioridade, wake latency, deadlines absolutos e deriva. Avaliar o worker host pacing e distinguir timeout overshoot, processamento e handler time. Não prescrever busy-wait ou aumento de prioridade sem evidência e análise de custo.

### Agente D — Audio Clock & Sample-Budget Auditor
Especialista em PSG SN76489, geração de amostras, sample rate, resampling, CPAL, FIFO e underruns. Verificar se `sample_count / sample_rate` representa corretamente a duração temporal usada no orçamento do frame. Não confundir ausência de CPAL físico com ausência de PSG digital. Não alterar pitch, clock ou PSG como workaround.

### Agente E — Video Pipeline & Mailbox Auditor
Especialista em Slint, WGPU 30, Vulkan, FrameMailbox, latest-frame-wins, upload, callbacks e PresentMon. Distinguir frame produzido, publicado, substituído, consumido, submetido e fisicamente apresentado. Não inferir present timestamp a partir de `Queue::write_texture` ou `request_redraw`.

### Agente F — Git Forensics & Regression Bisect Auditor
Especialista em investigação histórica Git, diff causal, bisect, cherry-pick seletivo e regressões. Encontrar a última revisão estável e o primeiro commit que altera a cadência. Separar mudanças simultâneas e não assumir que a proximidade temporal prova causalidade.

### Agente G — Independent Test & Evidence Auditor
Atuar como revisor cético independente. Validar desenho experimental, qualidade dos logs, unidades, amostras, p50/p95/p99, limites da instrumentação e reprodutibilidade. Tentar refutar as conclusões dos demais agentes. Nenhuma causa raiz é aceita sem teste que a distingue de hipóteses concorrentes.

## 3. Protocolo de trabalho

1. Confirmar branch, HEAD de main, status da árvore, issue #755, PR de investigação e CI. Nunca trabalhar diretamente na main.
2. Ler integralmente:
   - `docs/diagnostics/p0-sms-frame-pacing-telemetry.md`
   - `docs/diagnostics/p0-sms-video-only-physical-capture.md`
   - relatório P0 criado em 2026-11-13 e os logs brutos da captura pareada.
3. Consultar histórico de commits e PRs; localizar última execução estável e alterações de Scheduler/worker/pacing.
4. Mapear o fluxo temporal completo:
   `MachineScheduler → SmsTemporalMachine::step_frame_until → execute_frame_with_renderer → FramePacingDeadline → worker_loop/recv_timeout → FrameMailbox → Slint callback → WGPU/PresentMon`.
   Corrigir esse mapa se o código real divergir.
5. Cada agente produz:
   - arquivos/funções/linhas auditadas;
   - fatos observados;
   - hipóteses;
   - testes para confirmar/refutar;
   - riscos e recomendações;
   - nível de confiança e lacunas.
6. Fazer uma reunião de síntese: reconciliar resultados conflitantes, eliminar duplicações e escolher o experimento de maior poder discriminatório.
7. Implementar somente instrumentação ou teste necessário para distinguir hipóteses. Instrumentação deve ser limitada, sem logs síncronos por frame nem alteração do comportamento observado.
8. Reproduzir e localizar o primeiro commit causal. Se não for possível bisectar, explicar precisamente por quê e construir A/B isolado.
9. Só então propor correção mínima em branch própria. Antes de editar, registrar invariantes e testes que protegem cada uma.
10. Executar fmt, Clippy `-D warnings`, testes direcionados, testes workspace e builds Windows x64 aplicáveis. Fazer captura física antes/depois com mesma ROM, região, cena, duração, foco, geometria, display e configurações.
11. Publicar relatório final com evidências e decisão de manter/liberar o bloqueio. Não fazer merge de correção sem testes e CI completos.

## 4. Invariantes não negociáveis

- MachineScheduler é a autoridade temporal da máquina.
- GUI não controla ciclos, CPU, VDP, PSG ou clocks.
- Nenhum callback por instrução/ciclo na GUI.
- Preservar causalidade, ordem de eventos, WAIT states e operações suspensíveis/retomáveis.
- Execução contínua e execução em blocos devem ser temporal e semanticamente equivalentes.
- Save states/replays precisam preservar estado suficiente para retomada determinística.
- Renderer e saída física de áudio não podem alterar o tempo virtual.
- Não alterar clocks nominais, número de ticks por frame ou taxa do PSG para fazer o HUD exibir 60.
- Não adicionar segundo relógio de hardware.
- Não usar sleeps fixos, GUI timer, VSync/G-SYNC feedback, busy loop ou frame skipping como correção empírica.
- Não reescrever componentes de hardware validados para facilitar a investigação.

## 5. Métricas e distinções obrigatórias

Separar explicitamente:

1. virtual ticks por frame;
2. duração de `step_frame_until`;
3. processamento completo do frame;
4. deadline absoluto e orçamento restante;
5. tempo aguardado em `recv_timeout`;
6. wake lateness após timeout;
7. duração e tipo de command handlers;
8. amostras PSG por frame e duração nominal correspondente;
9. publicação e replacements da mailbox;
10. poll/consumo Slint;
11. conversão RGB, Queue::write_texture e callback;
12. eventos de apresentação física PresentMon/QPC, quando disponíveis.

`FPS VDP` do HUD é progresso de sequência de frames observado pela interface, não é contador de scan-out físico. `Queue::write_texture` não prova conclusão da GPU nem apresentação física. `empty_polls` não são frames repetidos físicos. Mailbox replacements não são automaticamente defeitos.

## 6. Entregáveis e formato do relatório

Publicar em `docs/diagnostics/`:

- `p0-frame-pacing-root-cause-report.md`
- `p0-frame-pacing-git-forensics.md`
- `p0-frame-pacing-validation.md`

O relatório de causa raiz deve conter:
- resumo executivo;
- timeline de commits;
- diagrama do fluxo temporal real;
- tabela de hipóteses CONFIRMED / REFUTED / OPEN;
- evidências brutas e cálculos com unidades corretas;
- causa raiz com cadeia causal;
- patch e rationale;
- testes antes/depois;
- medições físicas antes/depois;
- limitações e riscos residuais;
- decisão formal: BLOCKED ou P0 CLOSED.

## 7. Critérios de sucesso

A investigação só termina quando:
- causa raiz for reproduzida e vinculada a código/commit específico;
- correção recuperar cadência nominal regional de forma sustentada;
- não houver déficit sistemático próximo de 4%;
- determinismo e equivalência contínuo/blocos permanecerem comprovados;
- não houver deriva temporal cumulativa;
- testes SMS/SG-1000 e CI aplicáveis passarem;
- captura física antes/depois demonstrar a recuperação;
- agente independente revisar e não encontrar lacuna crítica.

**Não declarar sucesso por build verde, janela aberta, FPS momentâneo ou percepção subjetiva.** Se a causa não estiver comprovada, mantenha o bloqueio e reporte a próxima experiência discriminatória.

Comece pela auditoria do estado atual e pela criação/ativação da equipe de especialistas. Não faça alterações no runtime antes de entregar a primeira matriz de achados.
