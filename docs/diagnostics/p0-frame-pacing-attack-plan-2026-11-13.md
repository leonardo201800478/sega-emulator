# P0 — Frame pacing: estado e plano de ataque

**Data:** 2026-11-13  
**Baseline:** `main` — `d093037bd5fbfef1278d4980f775efa2c549171c`  
**Issue:** #755  
**Status:** ABERTA — causa raiz não demonstrada; nenhuma correção definitiva aprovada.

## 1. Fechamento da frente SRAM

A frente SRAM sai do foco ativo e permanece apenas como regressão:
- PR #802: persistência de SRAM no Slint Desktop — merged.
- PR #803: override volátil explícito para ROMs desconhecidas de diagnóstico — merged.
- SMSmemtest v0.40: dez verificações de SRAM passaram (cinco padrões por banco, bancos 0 e 1), com override volátil de 32 KiB.
- Phantasy Star: usuário confirmou gravação e restauração após fechar e reabrir o emulador.
- PRs legadas #643 e #645 foram fechadas como supersedidas. A #645 também carregava uma otimização de preflight/commit de clocks: ela não foi aprovada nem declarada integrada. A lógica atual ainda calcula preview e depois chama `advance_to`, repetindo conversão. Se retomada, deve ser extraída e medida em PR independente; não presumir que seja causa de pacing.

## 2. O problema P0 permanece aberto

Não confundir:
1. **Tempo virtual:** Scheduler, Z80, VDP, PSG, T-states e frame lógico.
2. **Tempo de parede:** execução do worker, orçamento, espera, wake-up, comandos e custos host.
3. **Apresentação física:** renderização, present, DWM/VRR e scanout.

O HUD `FPS VDP` acompanha sequências concluídas pelo worker; não mede frames fisicamente exibidos. `request_redraw`, retirada do mailbox e duração de `write_texture` não comprovam present/scanout.

Há capturas históricas próximas de 57–59 FPS e outra execução de 60,012 s com 1.932 frames (~32,194 FPS), diagnósticos desligados; com diagnósticos ligados ficou ~32,213 FPS. Esses cenários são incompatíveis e não podem ser misturados sem reconciliar build SHA, runner, ROM, região, configuração e caminho de apresentação. Alguns valores históricos têm sufixos de unidade inconsistentes; conferir artefatos brutos antes de usá-los como baseline.

O runner existente `tests/sms/scripts/run_pacing_rom.py` executa o `SmsTemporalWorker` real e `recv_timeout`, mas desliga Slint/WGPU/Present físico e saída CPAL. Serve para isolar worker/wakeup, não para fechar apresentação física.

A hipótese de espera relativa interrompida por comandos é plausível e tem mecanismo de código observável, mas a contribuição ao sintoma ainda não foi quantificada. Oversleep do Windows, processamento, áudio, mailbox e apresentação também precisam ser medidos. Não alterar Scheduler, clocks, Z80, VDP, PSG ou WAIT states sem causalidade demonstrada.

## 3. ROM correta para frame pacing

SMSmemtest é uma suite funcional de RAM interna, VRAM e SRAM; tem fases interativas e muda de carga. **Não é benchmark de cadência.**

### ROM principal proposta: homebrew de pacing do projeto

Criar fonte própria e versionada, com binário gerado localmente/CI, sem ROM de terceiros:
- inicia automaticamente, sem menu, prompts, SRAM ou input obrigatório;
- perfil inicial USA/NTSC explícito;
- laço sincronizado por VBlank;
- produz conteúdo visual diferente a cada frame;
- cena fixa e repetível: fundo em barras/tiles, marcador móvel determinístico e conjunto fixo de sprites;
- modos compiláveis/selecionáveis: CPU-light, CPU-fixed, VDP/sprites e combinado;
- modo PSG determinístico opcional, mantendo o mesmo padrão de áudio em toda a captura;
- sem testes de memória, loops infinitos de falha ou trabalho variável;
- contador visual de frame/assinatura de cena observável read-only, usado apenas para detectar congelamento/repetição/salto de conteúdo;
- registrar SHA-256 da ROM, commit do fonte, compilador/devkitSMS, flags e região;
- definir licença própria permissiva e não incluir binários comerciais.

O contador da ROM não substitui o frame_number do VDP nem o timestamp de apresentação. O runner mede o worker; PresentMon/ETW mede a fronteira host/present/display.

### ROMs auxiliares
- SMS Test Suite v0.38 — Drop Shadow: carga secundária útil de VDP/sprites após confirmação visual de que o teste foi selecionado. A auditoria não encontrou licença explícita; obter apenas da release oficial para uso local, sem redistribuir/commitar.
- SmsVdpTest: testes funcionais de VDP compatíveis com a granularidade implementada; não é o benchmark primário.
- SMSmemtest: somente regressão funcional de RAM/VRAM/SRAM.
- ZEXALL-SMS: não usar como ROM de pacing; permanece bloqueado até fechamento formal E7 do Z80.

## 4. Plano de ataque

### P0-A — Baseline reproduzível
1. Fixar commit, Release build, ROM SHA, região, resolução, refresh, VRR, VSync, driver, Windows build, CPAL e configuração do host.
2. Warm-up 10 s e captura mínima de 5 min; três repetições por cenário.
3. Preservar JSONL, ETW/PresentMon e metadados fora do Git.
4. Reconciliar a discrepância histórica ~32 versus ~57–59 FPS antes de escolher baseline.

### P0-B — Trace pareado por frame
Instrumentar com QPC/clock monotônico: início/fim de step_frame; processamento; orçamento e deadline; início/fim de espera; motivo do wake (timeout/comando); lateness; tipo/chegada/duração de comando; ticks virtuais; PSG samples; instruções; publicação, substituição e retirada do mailbox. A telemetria rolling atual não permite causalidade pareada. Usar ring buffer limitado e exportar em lote, sem callback por instrução nem logging síncrono no caminho crítico. Medir overhead da instrumentação.

### P0-C — A/B de deadline
Na mesma base atualizada:
- A: política atual;
- B: deadline phase-locked/absoluto.
Testar sem comandos e com wakeups controlados no meio da espera; separar comandos não-pausantes, Pause, Reset, Stop e LoadRom. Um deadline não pode sobreviver indevidamente a uma transição que interrompe/reinicia a execução. Exigir mesma sequência de ticks, estado final, samples PSG e chunk-invariance. A branch #796 está baseada em histórico antigo/divergente; transplantar somente o experimento, não mesclar sua branch antiga inteira.

### P0-D — Windows wait
Somente se o trace apontar lateness de wake-up, comparar espera atual com `CreateWaitableTimerExW` de alta resolução, preservando wake imediato por comando, fallback, RAII e sem busy-spin permanente. Medir lateness p50/p95/p99 e CPU usage no PC-alvo.

### P0-E — Apresentação física
Rodar a ROM própria pelo Slint + WGPU 30 + Vulkan, CPAL desligado/ligado. Capturar PresentMon/ETW e correlacionar worker sequence, mailbox publish/consume, render/present e displayed frame quando disponível. Separar janela visível, oculta/minimizada e cenários VSync/VRR. Não inferir present de request_redraw/write_texture.

### P0-F — Invariantes e regressões
Para cada variante, executar Scheduler monotonicidade/causalidade/chunk-invariance; Z80/VDP/PSG com ticks e estados iguais; PSG samples iguais; execução contínua equivalente a blocos; Pause/Resume/Reset/Stop/LoadRom/shutdown; regressões SMS/SG-1000; fmt, Clippy, testes workspace e CI Windows.

### P0-G — Aceitação
Só aceitar correção se três repetições demonstrarem melhora em cadência sustentada e jitter p50/p95/p99, sem deriva, sem alteração do tempo virtual/determinismo e com captura de apresentação física coerente. Caso contrário, manter P0 aberta e investigar a próxima fronteira instrumentada.

## 5. Critérios de encerramento
- causa raiz demonstrada por trace correlacionado e reprodução;
- baseline/correção comparáveis por hashes e ambiente;
- worker e apresentação física medidos separadamente;
- sem deriva, regressão temporal ou busy-spin permanente;
- melhora repetível em jitter e late frames p95/p99;
- CI e regressões aprovados;
- evidências, artefatos e decisão registrados.

**Conclusão em 2026-11-13: SRAM validada; P0 frame pacing não resolvida.**
