# P0 — Frame pacing: estado e plano de ataque

- **Data:** 2026-11-13
- **Baseline:** `main` — `d093037bd5fbfef1278d4980f775efa2c549171c`
- **Issue:** #755
- **Status:** ABERTA — causa raiz não demonstrada; nenhuma correção definitiva aprovada.

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
Só aceitar correção se três repetições demonstrarem melhoria em cadência sustentada e jitter p50/p95/p99, sem deriva, sem alteração do tempo virtual/determinismo e com captura de apresentação física coerente. Caso contrário, manter P0 aberta e investigar a próxima fronteira instrumentada.

## 5. Critérios de encerramento
- causa raiz demonstrada por trace correlacionado e reprodução;
- baseline/correção comparáveis por hashes e ambiente;
- worker e apresentação física medidos separadamente;
- sem deriva, regressão temporal ou busy-spin permanente;
- melhora repetível em jitter e late frames p95/p99;
- CI e regressões aprovados;
- evidências, artefatos e decisão registrados.

**Conclusão em 2026-11-13: SRAM validada; P0 frame pacing não resolvida.**

## 6. Correção de passos defasados — atualização documental

Esta seção prevalece sobre instruções anteriores conflitantes em branches/PRs ainda abertas. Ela não apaga o histórico: identifica quais registros descrevem um estado anterior e qual procedimento deve ser seguido agora.

### 6.1 SRAM: investigação encerrada; não reabrir como bloqueio atual

**Passos obsoletos — não executar novamente como pendência:**
- Investigar novamente a ausência de backing store de SRAM para explicar a falha antiga do SMSmemtest.
- Tratar os bancos Cart SRAM 0/1 como `BLOCKED_CAPABILITY` no estado atual.
- Condicionar a validação funcional de SRAM à implementação futura do mapper, sem considerar o override diagnóstico já integrado.

**Estado atual e evidências:**
- PR #802 foi merged: perfil de cartucho verificado, persistência `.srm` e ciclo de vida de saves no Slint Desktop.
- PR #803 foi merged: `SEGA_SMS_DIAGNOSTIC_SRAM_BYTES=32768` permite SRAM volátil explícita para ROMs de diagnóstico sem perfil conhecido; não cria saves persistentes artificiais nem altera o comportamento padrão de ROMs desconhecidas.
- A validação posterior registrada neste plano confirmou SMSmemtest v0.40 com dez verificações SRAM aprovadas: cinco padrões em cada um dos bancos 0 e 1.
- A validação de persistência de jogo real foi confirmada com Phantasy Star: salvar, fechar o emulador e restaurar o save após reabertura.

**Como interpretar os documentos históricos:**
- `docs/validation/smsmemtest-oracle-audit-2026-11-13.md`, produzido antes da conclusão da PR #803 e da validação subsequente, registra corretamente a limitação daquele momento. Seus trechos `BLOCKED_CAPABILITY` são históricos e não representam o estado atual.
- A descrição original da PR #797 também registra esse bloqueio anterior. Não usar essa descrição isoladamente para decidir o estado atual da SRAM.
- SMSmemtest deve continuar como regressão funcional de memória. A conclusão dos testes SRAM não certifica, por si só, todos os mappers, perfis de cartucho ou todos os jogos.

### 6.2 SMSmemtest: não usar como benchmark primário de cadência

**Passos obsoletos — substituir:**
- Não usar a tela final/espera interativa do SMSmemtest como carga contínua representativa de CPU, VDP e sprites.
- Não interpretar o resultado de 60 s no worker como taxa de frames fisicamente apresentados.
- Não repetir a sequência de entrada `26,441` como se ela fosse um workload sustentado. Ela serve para reproduzir a navegação funcional específica dessa ROM.

**Procedimento atual:**
- SMSmemtest: executar para regressão funcional RAM/VRAM/SRAM e guardar os resultados do oráculo.
- ROM homebrew de pacing do projeto: benchmark primário, com cena determinística, mudança visual por frame, modos de carga controlados e hash/proveniência fixados.
- SMSTestSuite Drop Shadow: workload auxiliar de sprites somente depois de confirmação visual de que o menu selecionou o teste correto; respeitar a restrição de licença já documentada.
- O resultado hosted-Linux de aproximadamente 59,7 FPS em 60 s é apenas sanity check do worker/espera. Não é baseline de apresentação nem aceitação física.

### 6.3 Experimento de deadline antigo e PRs empilhadas

- A branch experimental da PR #796 é evidência histórica de experimento; sua base é antiga/divergente. Não mesclar a branch inteira nem tratá-la como implementação aprovada.
- Se algum experimento ainda for necessário, transplantar somente a alteração mínima relevante para uma branch baseada no `main` atual e repetir os testes de invariantes.
- PR #800 está empilhada sobre a linha experimental antiga (#796). Antes de qualquer merge, reavaliar a base/dependências e reaplicar somente o código ainda necessário sobre a linha atual; não presumir que a branch esteja pronta por ter testes unitários.
- PR #807 é a implementação atual em avaliação para phase-lock e notificação de frames. Continua draft até CI completo e validação física; não declarar sucesso de pacing apenas por compilação ou testes automatizados.

### 6.4 Critérios que continuam pendentes para fechar P0

1. Executar a ROM homebrew de pacing em Windows no build Slint + WGPU 30 + Vulkan.
2. Repetir capturas controladas, com ambiente, ROM SHA, emulator SHA, região, refresh e configuração documentados.
3. Correlacionar sequência produzida, publicação/consumo do mailbox e eventos PresentMon/ETW; distinguir chamadas Present de frames efetivamente exibidos.
4. Repetir com CPAL desligado e ligado, sem misturar os resultados.
5. Validar condições de janela em foco e fora de foco/oculta conforme a matriz experimental.
6. Demonstrar melhora repetível de cadência/jitter sem alterar ticks virtuais, estado final, samples PSG ou equivalência contínua-versus-blocos.
7. Manter P0 aberta até que a evidência física e os invariantes passem.

**Regra de manutenção:** quando uma PR posterior concluir uma etapa, atualizar este plano e marcar o procedimento antigo como histórico. Não reabrir SRAM sem regressão reproduzível; não iniciar E8/ZEXDOC/ZEXALL antes do fechamento demonstrável de E7.
