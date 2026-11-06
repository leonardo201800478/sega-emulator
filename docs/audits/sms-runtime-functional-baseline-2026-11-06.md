# Auditoria e plano de correção — baseline funcional A/V do SMS

**Data:** 2026-11-06  
**Repositório:** `leonardo201800478/sega-emulator`  
**Baseline inicial:** `main` — `8bd7f6c78cacd247736268d141e4cc7e30805ccd`  
**Main após integração:** `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c` (PRs #595, #596 e #597)  
**HEAD #595 validado:** `69ca40ab29c486f26852adb7536a04f95f9dd7ae`  
**HEAD #597 validado:** `30554cc6642dd49d06adf495dca3e80f4822b081`  
**Escopo:** estabelecer e validar o primeiro caminho funcional de imagem e áudio no runtime temporal SMS, sem alterar o runtime SG-1000.

## 1. Resumo executivo

A baseline contém um runtime SMS em Rust integrado ao frontend Qt/PyO3:

`MinimalSmsWindow → TemporalSMSMachine → SmsTemporalWorker → SmsTemporalMachine → SmsTemporalBus → SmsVdp + ScheduledPsg`.

O worker publica frames RGB888 em `FrameMailbox`, e uma sessão D3D12 nativa consome os frames. O mesmo worker drena amostras do PSG e as encaminha à sessão de áudio WASAPI/XAudio2 quando configurada.

**A existência desses caminhos não prova saída funcional de um jogo.** Antes desta tarefa, os testes de frame confirmavam avanço e dimensões, mas não exigiam pixels não pretos. Os testes de worker publicavam um frame gerado por ROM sintética sem configuração do VDP. Também não havia um teste end-to-end no core que exigisse amostras PSG não silenciosas. GPU física, dispositivo de áudio e gameplay em Windows permanecem sem comprovação nesta auditoria.

A prioridade foi alterada para: provar sinal gráfico e sonoro no core, provar transporte até os hosts e somente então iniciar análise de desempenho real.

## 2. Baseline e PRs abertas

| Item | Estado observado |
|---|---|
| `main` atual | `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c` |
| PR #595 — baseline A/V + IRQ | MERGED; CI #5178 SUCCESS no HEAD `69ca40ab29c486f26852adb7536a04f95f9dd7ae` |
| PR #596 — reconciliação documental | MERGED; CI documental SUCCESS; merge `3c0e0548f0f7923ec2e5a500b3a953528266fd2e` |
| PR #597 — worker → NativeAudioSession | MERGED; CI #5185 SUCCESS no HEAD `30554cc6642dd49d06adf495dca3e80f4822b081`; merge `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c` |
| PR #594 — ligação VDP IRQ → Z80 INT | CLOSED como SUPERSEDED; sem alteração exclusiva útil não portada |
| PR #593 — telemetria VDP/PSG | OPEN / DRAFT; CI CANCELLED no HEAD `ee5ba892`; custo de scan ainda não medido |
| Backend físico, GPU/áudio audíveis e gameplay real | NOT RUN |

A PR #594 identificou corretamente a ausência de atualização automática da entrada INT a partir de `SmsVdp::interrupt_pending()`. Porém, a proposta inicial chamava `set_int_line(None)` sempre que o VDP não estivesse pendente. Isso sobrescreve uma linha INT externa independente, quebrando testes de HALT/INT existentes. A correção não deve tratar a saída do VDP como proprietária exclusiva do pino INT.

A PR #593 adiciona telemetria por frame, mas seus scans de framebuffer/VRAM/CRAM introduzem trabalho adicional no worker. Essa telemetria pode ajudar a depurar a primeira ROM real, porém o overhead precisa ser medido ou a coleta deve ser desativável antes de usar o mesmo build para benchmark.

## 3. Correção de IRQ integrada pela PR #595

A branch de correção separa a fonte externa da saída IRQ do VDP:

- `external_int_line: Option<u8>` é preservada na máquina e no DTO de estado.
- A versão de estado da máquina SMS passa de 24 para 25.
- A linha efetiva do Z80 é a combinação das fontes: linha externa, se ativa; caso contrário, vetor `0xFF` quando o VDP sinaliza IRQ; caso contrário, linha desassertada.
- A atualização ocorre após o dispatch do Scheduler e os efeitos de I/O, antes do próximo ponto de amostragem do Z80.
- A restauração valida que o estado de INT da CPU corresponde às fontes restauradas.
- A API `set_int_line()` continua representando a fonte externa e não avança o tempo.
- A linha efetiva também controla o byte do ciclo Interrupt Acknowledge: fonte externa ativa fornece seu vetor; VDP-only fornece `$FF` (barramento não dirigido/pull-up); sem fonte ativa, o valor retorna a `$FF`. Isso evita reutilizar um vetor externo obsoleto após sua desativação.
- A restauração valida tanto a linha INT da CPU quanto o byte de acknowledge persistido no barramento.

Testes adicionados na branch:
1. VDP VBlank IRQ chega ao Z80 em uma ROM sintética com IM 1/HALT, a ISR lê `$BF` e incrementa RAM por frame.
2. A sincronização do VDP não apaga uma fonte externa INT ativa, inclusive após export/restore.
3. Uma ROM sintética configura VDP Mode 4, VRAM/name table e CRAM e exige pixels RGB não pretos.
4. A mesma ROM programa o SN76489 e exige amostras PCM não silenciosas.
5. O teste de interrupção usa R0 `$06` (Mode 4) e R1 `$E0` (16 KB, display ligado e frame IRQ ligado), verificando explicitamente os bits BLK/IE0.

**Validação integrada:** CI #5178 SUCCESS no HEAD exato `69ca40ab29c486f26852adb7536a04f95f9dd7ae`. A implementação e os testes descritos nesta seção estão integrados na `main` pelo merge SHA `733152f61b706b2243914ae017643cef949d4ae1`. Resultados de PR #594/#593 não foram transferidos; a #594 foi supersedida e a #593 continua Draft.

## 4. Gate A/V

| Gate | Critério | Estado |
|---|---|---|
| A/V-01 | Core VDP produz pixels RGB não pretos com ROM sintética | PASS — teste sintético Rust no CI #5178 |
| A/V-02 | PSG produz PCM não silencioso na timeline do Scheduler | PASS — teste sintético Rust no CI #5178 |
| A/V-03 | Worker publica frame produzido pelo VDP no mailbox RGB888 | PASS — pixel esperado RGB preservado no mailbox no CI #5178 |
| A/V-04a | Worker → `NativeAudioSession` → `PcmSink` em memória recebe PCM não silencioso antes do flush | PASS — teste de integração #597, CI #5185 |
| A/V-04b | `NativeAudioBackend` WASAPI/XAudio2 real aceita PCM sem erro/underrun persistente | NOT RUN; sink em memória não comprova backend real |
| A/V-05 | Janela D3D12 apresenta imagem em GPU física e áudio é audível | NOT RUN |
| A/V-06 | Jogo representativo ultrapassa boot e permite gameplay | NOT RUN |

A imagem gerada pelo core e o PCM não silencioso são condições necessárias, mas não suficientes, para provar que o D3D12 apresenta a imagem ou que o dispositivo reproduz som.

## 4A. Revisão crítica da hipótese de HALT/IRQ e pacing (2026-11-06)

A hipótese recebida do Gemini foi confrontada com o código da branch e com a referência técnica SMS Power. Resultado:

### Cabeamento VDP → Z80

- **Falha confirmada na baseline anterior à PR #595:** o VDP expunha `interrupt_pending()`, mas a integração automática com a linha INT do Z80 ainda não estava concluída.
- **Correção integrada pela PR #595:** a linha efetiva combina a origem INT externa e a saída IRQ de nível do VDP; a sincronização ocorre depois de cada evento do Scheduler e imediatamente antes de `SmsTemporalMachine::begin_instruction()`.
- O segundo ponto de sincronização protege a fronteira de instrução/HALT contra estado INT obsoleto. Não substitui o sincronismo na fronteira temporal de cada evento.
- **ACK/resume já existia:** `SmsTemporalMachine::step_frame()` identifica `Z80MicroStep::InterruptAcknowledge` e chama `resume_instruction(0)`. A API `step_instruction()` deliberadamente devolve o ACK ao chamador; não duplicar a retomada no worker nem esconder a fronteira pública.
- A ISR deve ler `$BF` para limpar o status de VBlank antes de retornar, conforme o comportamento documentado do VDP.

### Correção adicional encontrada na própria ROM de teste

A ROM de teste original gravava `$A0` em VDP R1 e comentava que habilitava display. A referência SMS Power define R1 bit 6 como display enable e bit 5 como frame IRQ enable. Logo, `$A0` habilitava o IRQ, mas mantinha a imagem em branco. O teste foi corrigido para `$E0` (16 KB, display ligado, frame IRQ habilitado) e passou a verificar explicitamente os dois bits.

Fonte primária: [SMS Power — VDP Registers](https://www.smspower.org/Development/VDPRegisters). A função local `SmsVdp::interrupt_pending()` usa R1 bit 5 para frame IRQ, consistente com a referência.

Foi adicionado o exemplo reproduzível `native/sms_core/examples/irq_wiring_repro.rs`, executável da raiz com:

```text
cargo run --release -p sega-sms-core --example irq_wiring_repro
```

O exemplo exige que a ISR incremente `$C000` exatamente 20 vezes em 20 frames; resultado de execução ainda depende do CI/execução local e permanece NOT RUN até essa evidência. O ACK é retomado pelo contrato já existente de `step_frame()` → `resume_instruction()`.

### HALT fast-forward

Não aplicar o salto temporal proposto sem modelagem. O Z80 em HALT continua produzindo ciclos M1/refresh e amostrando INT em fronteiras T4. Pular diretamente ao próximo evento só é correto se preservar:
- duração e índices de M-cycle/T-state;
- refresh register e ciclos de barramento observáveis;
- instante de amostragem e aceitação de INT/NMI;
- causalidade com VDP, PSG, mapper e Scheduler;
- export/restore no meio do intervalo e equivalência execução contínua/em blocos.

Primeiro medir quantos dispatches/instruções HALT são executados por frame e quanto custam. Depois, se o custo for material, modelar um estado idle suspensível do Z80 no Scheduler com testes diferenciais/temporais. Não substituir ciclos reais por avanço de tempo opaco.

### Pacing e timeBeginPeriod

O worker calcula o prazo de frame a partir das amostras PCM e usa `recv_timeout(frame_wait)`. Isso justifica medir overshoot, jitter, frame pacing e underruns no Windows, mas não prova que a resolução padrão do timer seja a causa.

A documentação oficial da Microsoft informa que `timeBeginPeriod(1)` pode melhorar a precisão de timeouts, mas:
- a partir do Windows 10 2004 a configuração é por processo, não global;
- no Windows 11 a resolução elevada não é garantida para processo com janela totalmente oculta/minimizada ou inaudível;
- há custo de escalonamento/energia;
- não melhora a precisão do high-resolution performance counter;
- exige pareamento estrito com `timeEndPeriod(1)`.

A própria Microsoft classifica Multimedia Timers como legado e recomenda MMCSS para novos cenários quando possível. Portanto, não adicionar FFI `timeBeginPeriod` como correção automática. Primeiro coletar histogramas de overshoot de `recv_timeout`, frametime, fila de áudio e underruns. Depois escolher mecanismo de espera apropriado e garantir aquisição/liberação RAII e testes Windows.

Referências: [Microsoft — timeBeginPeriod](https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timebeginperiod) e [Microsoft — Timer Resolution](https://learn.microsoft.com/en-us/windows/win32/multimedia/timer-resolution).

## 5. Plano de implementação incremental

1. Fechar a correção de INT e os testes de regressão HALT/INT, VBlank e Line IRQ, incluindo o exemplo `irq_wiring_repro`.
2. Passar os gates A/V-01/A/V-02 com valores esperados independentes.
3. Estender o teste do worker para publicar um frame não preto e confirmar a sequência do mailbox.
4. Adicionar teste de integração da sessão de áudio com PCM conhecido, sem depender de dispositivo físico; preservar teste manual de endpoint.
5. Executar CI Rust, Python e Windows x64 D3D12/PyO3 no HEAD exato.
6. Preparar build Windows para teste manual com ROM legal/homebrew e, depois, jogo representativo.
7. Só após A/V-01 a A/V-05, coletar FPS, frametime, áudio underruns/dropped blocks e custos do worker; medir overhead de telemetria separadamente.

## 6. Limites técnicos que permanecem

- O VDP ativo usa composição por scanline; não declarar fetch arbitration, WAIT físico, pixel pipeline ou efeitos intralinha concluídos.
- A saída de vídeo deve preservar geometria e conteúdo do frame; o host D3D12 não altera o tempo emulado.
- O PSG é clockado pela timeline e gera amostras digitais; isso não equivale a calibração analógica nem a validação acústica.
- Não atribuir a tela preta/ruído de um jogo específico à IRQ sem reproduzir o caso e identificar a primeira divergência.
- A investigação de Andre Agassi Tennis de 2026-10-29 permanece histórica e sem reprodução independente.
- O Z80 E7.7 continua como frente semântica independente; E8 não deve começar antes de E7 estar fechado.
- SG-1000 deve continuar passando regressões; nenhuma alteração deve duplicar CPU ou mover hardware para Python.

## 7. Documentação reconciliada

- [Roadmap mestre](../project-roadmap.md) — prioridade A/V atual antes de otimização.
- [Estado dos componentes](../component-status.md) — fotografia da main e limites de evidência.
- [Matriz VDP/PSG](../architecture/sms-vdp-psg-temporal-validation.md) — critérios A/V e temporais.
- [Consolidação histórica Scheduler/VDP/D3D12 de 2026-11-02](../architecture/scheduler-sms1-vdp-d3d12-consolidation.md) — preservar como fotografia anterior; não usar seus estados como fotografia da main atual.
- [Investigação histórica Andre Agassi Tennis](andre-agassi-tennis-sms-black-screen-investigation-2026-10-29.md).

## 8. Critérios para conclusão desta tarefa

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `python -m pytest`
- [ ] `python -m ruff check .`
- [ ] `python -m ruff format --check .`
- [ ] Windows x64 D3D12/PyO3 gates PASS no mesmo HEAD
- [ ] VDP IRQ não sobrescreve INT externo; snapshots preservam ambas as fontes
- [ ] Teste sintético produz pixels não pretos e PCM não silencioso
- [ ] Worker publica o frame correto e encaminha PCM
- [ ] Teste manual Windows confirma imagem e som com host real
- [ ] SG-1000 regressions PASS

A tarefa só pode ser declarada concluída após os gates aplicáveis e a validação manual A/V. `SKIPPED` e `NOT RUN` não são PASS.


## 9. Estado final após integração — 2026-11-06

**Main:** `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c` (merge da PR #597).

| Suíte / gate | Classificação | Evidência / limite |
|---|---|---|
| PR #595 — Rust fmt/Clippy/workspace | PASS | CI #5178 no HEAD `69ca40ab29c486f26852adb7536a04f95f9dd7ae` |
| PR #595 — Python lint/regressão | PASS | 332 passed, 9 skipped no runner Linux por ausência da extensão PyO3 nesse ambiente |
| PR #595 — Windows D3D12 e PyO3 | PASS | CI #5178; build Maturin, import e smoke SMS/SG-1000/input |
| PR #597 — Rust quality/unit | PASS | CI #5185; worker/audio test executado no job PyO3 Windows |
| PR #597 — Python lint/regressão | PASS | CI #5185 |
| PR #597 — Windows D3D12 e PyO3 | PASS | CI #5185; log confirma o teste de worker/PCM e 29 testes `audio_pipeline` |
| Scheduler deterministic stress | NOT RUN | SKIPPED nos workflows #5178/#5185 |
| Windows Qt smoke | NOT RUN | SKIPPED nos workflows #5178/#5185 |
| Windows PowerShell tooling | NOT RUN | SKIPPED nos workflows #5178/#5185 |
| A/V-01 — pixels no core | PASS | RGB `[255,255,255]` esperado na linha 32, CI #5178 |
| A/V-02 — PCM no core | PASS | Amostras não silenciosas do SN76489, CI #5178 |
| A/V-03 — worker/mailbox vídeo | PASS | Pixel RGB888 esperado preservado no mailbox, CI #5178 |
| A/V-04a — worker/NativeAudioSession/PcmSink | PASS | PCM não silencioso submetido antes do flush; 0 underruns/blocks dropped no `RecordingSink`, CI #5185 |
| A/V-04b — NativeAudioBackend/WASAPI/XAudio2 real | NOT RUN | Sem teste com backend nativo/endpoint real nesta etapa |
| GPU física / janela D3D12 | NOT RUN | CI de compilação/contratos não substitui teste em hardware |
| Endpoint WASAPI/XAudio2 audível | NOT RUN | Não há evidência de som audível em dispositivo real |
| Gameplay com ROM real/homebrew | NOT RUN | ROM sintética não comprova boot/gameplay de jogo |

### Revisão técnica estática

O diff da #595 e da #597 foi revisado. Não foi identificado bloqueador estático na composição IRQ/INT nem na generalização de `execute_frame<S: PcmSink>`. A PR #597 mantém o tipo de produção `MachineAudioSession` e injeta apenas um sink em memória nos testes. A revisão é estática e não substitui validação física nem prova de compatibilidade.

### Decisões de integração

- **PR #595:** MERGED em `733152f61b706b2243914ae017643cef949d4ae1`, usando expected HEAD `69ca40ab29c486f26852adb7536a04f95f9dd7ae`.
- **PR #596:** MERGED em `3c0e0548f0f7923ec2e5a500b3a953528266fd2e`; documentação reconciliada após #595.
- **PR #597:** MERGED em `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`, usando expected HEAD `30554cc6642dd49d06adf495dca3e80f4822b081`.
- **PR #594:** CLOSED como SUPERSEDED após comparação; sem alteração exclusiva útil não portada.
- **PR #593:** permanece OPEN / DRAFT; scans de framebuffer/VRAM/CRAM e amostras PSG precisam ser opt-in ou ter overhead medido em Release antes de benchmarks.

### Próxima etapa

Comprovar `NativeAudioBackend` WASAPI/XAudio2 aceitando PCM não silencioso e depois realizar teste de audição em dispositivo físico. Em paralelo, comprovar frame D3D12 em GPU física e executar ROM legal/homebrew para gameplay. Não declarar Master System funcional enquanto A/V-04b, A/V-05 e A/V-06 não forem comprovados. E7.7 permanece IN_PROGRESS e E8 BLOCKED; SG-1000 continua sujeito a regressão.
