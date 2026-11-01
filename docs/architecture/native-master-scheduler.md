# Master Scheduler Rust — timeline única de máquina, vídeo, áudio e entrada

## Objetivo e decisão arquitetural

Estabelecer um único proprietário temporal em Rust para a máquina emulada. CPU, barramento, VDP, PSG/FM e demais dispositivos digitais devem observar a mesma timeline e a mesma ordenação determinística de eventos. A entrada de gameplay deve chegar ao runtime como eventos normalizados e ordenados nessa timeline. A saída de vídeo e o consumo de áudio devem apresentar/consumir resultados produzidos pelo runtime, sem criar relógios independentes para avançar o hardware.

**Decisão proposta:** o Master Scheduler pertence ao runtime Rust e é a única autoridade para avançar o tempo emulado. Qt/PySide6 permanece como shell de janela, menus, configuração e diagnóstico. O host captura dispositivos e apresenta áudio/vídeo por adaptadores nativos, mas não executa lógica de hardware nem decide a progressão temporal da máquina.

Este documento define a arquitetura-alvo e os gates de migração. Não declara o scheduler implementado, não altera a fidelidade atual e não reivindica precisão ciclo a ciclo.

## Dependências e ordem obrigatória

Esta etapa depende da conclusão e integração dos ports existentes:

1. **PR #301 — vídeo nativo em Rust:** renderer, recursos, filtros, saída/apresentação, frame pacing e coordenação com o host. A saída normal de vídeo deve ser Rust-owned, sem cópia integral do framebuffer para Python/QImage por frame.
2. **PR #300 — input nativo em Rust:** captura/backend, enumeração, polling/eventos, hotplug, normalização, perfis e encaminhamento de gameplay. Nenhum polling/tradução de input por frame deve permanecer em Python.
3. **Áudio Rust já integrado:** reutilizar a sessão `NativeAudioSession<S: PcmSink>`, os adaptadores SG-1000/SMS e os sinks WASAPI/XAudio2 existentes. Não criar um segundo mixer, segundo pipeline PCM ou uma segunda execução da máquina.

As PRs #300 e #301 estão em Draft no momento da abertura desta proposta. Esta PR é deliberadamente uma etapa posterior: não substitui, não fecha e não declara concluídas essas dependências. Antes da implementação do scheduler, revalidar seus SHAs, contratos e testes no estado remoto atual.

## Modelo de responsabilidades

### Master Scheduler e máquina (Rust)

- Possui o estado temporal da máquina e define a política de avanço.
- Coordena CPU, barramento/memória, I/O, VDP, PSG/FM, interrupções, DMA e demais dispositivos existentes no perfil.
- Aplica escritas e leituras nos instantes definidos pelo contrato temporal; não posterga todos os efeitos até o fim do frame.
- Define a ordenação de eventos simultâneos de forma explícita, específica da plataforma e coberta por testes.
- Gera snapshots/telemetria e blocos de saída sem delegar execução de hardware ao Python.

### Entrada (captura nativa, gameplay em Rust)

- Backend nativo enumera dispositivos e captura teclado/gamepads/apontadores conforme o contrato aprovado na migração #300.
- Eventos físicos são normalizados na fronteira do host e inseridos em uma fila limitada e ordenada.
- O runtime aplica eventos em um ponto temporal determinístico: timestamp convertido para tick emulado ou, quando o hardware/contrato não justificar timestamp sub-frame, uma política explícita de amostragem em fronteira de frame.
- Perfil, mapeamento, ordem de jogadores, hotplug e desconexão não podem alterar retroativamente eventos já consumidos.
- A aquisição pode usar worker nativo; esse worker não modifica registradores da máquina nem avança o scheduler. Ele apenas publica eventos com ownership e sincronização definidos.

### Vídeo (produção Rust, apresentação host)

- VDP e renderer produzem estado/framebuffer no tempo da máquina.
- O caminho normal de renderização, recursos e apresentação deve ser Rust-owned conforme a #301.
- VSync/VRR e a fila de apresentação regulam a exibição física, não o relógio do hardware emulado.
- Qt fornece janela/surface handle e eventos de UI conforme contrato de lifetime/thread; não executa filtros nem copia o framebuffer integralmente por frame.
- Distinguir frame emulado, frame submetido, frame apresentado, refresh físico e latência medida.

### Áudio (produção Rust, consumo host)

- PSG/FM e fontes futuras avançam apenas pelo tempo emulado definido pelo scheduler.
- A sessão Rust existente recebe amostras/blocos sem PCM atravessar PyO3.
- WASAPI/XAudio2 consomem a fila segundo o relógio do dispositivo; o worker de áudio não avança CPU, VDP ou PSG.
- Underrun, overflow, atraso e recuperação do dispositivo são telemetria/condições de saída; não podem corrigir silenciosamente o tempo lógico da máquina.

### Qt/PySide6

Qt permanece responsável por menus, janela, configuração de perfis, comandos coarse-grained (load/reset/pause/resume/stop), logs e snapshots. Não chama `run_frame()` em um timer para determinar a progressão normal da máquina depois da migração. Não faz polling de gameplay, mixagem, resampling, filtros ou apresentação de framebuffer no caminho normal.

## Contrato temporal

A implementação deve evoluir para um domínio inteiro de tempo (ticks/master-clock ou unidade racional equivalente) com conversões explícitas por plataforma e revisão de hardware. Evitar acumuladores de ponto flutuante como fonte de verdade temporal.

Cada evento agendado precisa ter, no mínimo:
- instante no domínio temporal da máquina;
- classe/prioridade de ordenação definida pela plataforma;
- sequência estável para desempate;
- origem e payload tipados;
- regra explícita para eventos simultâneos e eventos atrasados.

O scheduler não deve impor uma ordem universal falsa entre todos os consoles. A política de empate e as relações entre CPU, barramento, VDP, PSG e interrupções precisam ser sustentadas por documentação, medições ou comportamento de hardware observado.

A evolução será incremental:
1. avançar dispositivos por intervalos/timestamps coerentes, preservando os contratos atuais;
2. introduzir eventos de barramento e pontos de sincronização onde necessários;
3. refinar instruções para micro-operações/acessos temporais, waits e contenção;
4. somente declarar precisão ciclo a ciclo quando traces ordenados e testes por T-state sustentarem essa afirmação.

Contagem agregada de T-states por instrução, sincronização por frame ou timestamps de amostras não equivalem isoladamente a precisão ciclo a ciclo.

## Propriedade, concorrência e relógios

- Um único scheduler possui o avanço da máquina. Não executar CPU, VDP e PSG em workers independentes com relógios próprios.
- Concorrência é permitida nas fronteiras de host: captura de input, consumo de áudio e apresentação de vídeo, com filas e ownership explícitos.
- Workers de host não mutam estado de hardware. Comandos de controle entram por uma fila de comandos e são aplicados em safe points definidos.
- Buffers devem ter limites, política explícita de backpressure/overflow e shutdown sem deadlock/use-after-free.
- Pausa, reset, troca de ROM/sistema, perda de foco, device loss e encerramento devem definir o que acontece com eventos pendentes, áudio enfileirado e frames em voo.
- Não liberar GIL nem criar callbacks Python por instrução/frame. PyO3 permanece uma fronteira coarse-grained.

## Critérios de aceitação

### Arquitetura e integração

- [ ] PR #300 e #301 integradas na base candidata, ou seus contratos finais revalidados e dependências explicitamente satisfeitas.
- [ ] Scheduler Rust é o único proprietário do avanço temporal da máquina.
- [ ] CPU, barramento, VDP, PSG e interrupções consomem uma timeline comum e uma política de ordenação documentada.
- [ ] Input de gameplay é normalizado e consumido em Rust; nenhuma chamada XInput/SDL ou tradução/polling por frame permanece em Python.
- [ ] Qt não dirige a emulação por QTimer/run_frame no caminho normal.
- [ ] Áudio consome PCM gerado pelo tempo emulado; o worker WASAPI/XAudio2 não avança a máquina.
- [ ] Vídeo é produzido pelo runtime Rust e a apresentação do host não modifica a timeline.

### Determinismo e testes

- [ ] Relógio virtual permite executar a máquina sem depender de wall-clock, Qt, GPU ou dispositivo de áudio.
- [ ] Mesma configuração, estado inicial e sequência de eventos normalizados reproduzem a mesma sequência de eventos, estados e saídas digitais.
- [ ] Testes cobrem empate de eventos, wraparound temporal, fila cheia, evento atrasado, pausa/reset/load/stop e recuperação de dispositivo.
- [ ] Teste de replay reproduz sequência de input e checkpoints de CPU/memória/I/O/VDP/framebuffer/PSG.
- [ ] Testes de integração SG-1000 e SMS; regressão SG-1000 obrigatória.
- [ ] Testes headless não exigem timer Qt, janela, GPU ou dispositivo físico.

### Sincronização A/V e validação no host

- [ ] Relação entre ticks emulados, frames de vídeo e amostras de áudio documentada por perfil de máquina.
- [ ] Medir deriva A/V, ocupação de filas, underruns/overflows, tempo de produção, submissão/apresentação e latência ponta a ponta.
- [ ] Testar WASAPI e XAudio2, VSync/VRR e variações de refresh; separar medições lógicas de latência física.
- [ ] Testar resize, fullscreen, minimize/restore, foco, hotplug, device loss e shutdown no Windows.
- [ ] Benchmarks com baseline/candidato equivalentes, workload fixo, aquecimento, repetições, mediana e variação.

## Estados de validação

Registrar separadamente `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING` e `TEMPORAL_PASS`. Não promover estado por compilação isolada ou por ausência de falha.

- `TEMPORAL_PASS` exige evidência de ordenação temporal dos eventos relevantes; não é concedido por T-states agregados.
- `LONG_PASS` exige sessões longas e cenários de estresse repetíveis.
- Testes físicos de input, áudio e apresentação devem identificar máquina, Windows, driver, backend efetivo e método de medição.

## Fora do escopo desta etapa documental

- Implementar precisão ciclo a ciclo completa em todos os chips.
- Introduzir JIT, paralelismo da execução de hardware ou múltiplos schedulers.
- Escolher um backend gráfico universal sem benchmark e matriz de compatibilidade.
- Substituir WASAPI/XAudio2 ou adicionar outro framework de áudio sem justificativa medida.
- Expandir o escopo para Mega Drive, Sega CD ou 32X antes de validar o contrato no SG-1000/SMS.

## Referências internas

- [Arquitetura de emulação multi-sistema](../architecture.md)
- [Runtime de áudio nativo](native-audio-runtime.md)
- [Migração de áudio nativo](migrations/native-audio-runtime.md)
- [Migração de vídeo nativo — PR #301](migrations/native-video-runtime.md)
- [Migração de input nativo — PR #300](migrations/native-input-runtime.md)


## Atualização de implantação — Temporal Core e scheduler headless (2026-10-31)

A dependência rígida de concluir primeiro as PRs de vídeo/input não se aplica à fundação temporal headless. O clock virtual, domínios racionais, fila determinística e um futuro `MachineScheduler` podem ser desenvolvidos/testados sem os ports de host. As PRs de input e vídeo permanecem gates para a integração final da fronteira de host e apresentação/captura, não para T0/T1 headless.

A PR #315 propõe `sega-temporal-core` com `MachineTime`, `ClockRatio`, `ClockDomain`, prioridades e fila de eventos. Seu próprio escopo declara que ainda não implementa `MachineScheduler`, micro-operações Z80, bus cycles, WAIT, contenção, eventos raster nem integração PSG. Logo, a existência do crate não satisfaz a arquitetura completa deste documento e não autoriza TEMPORAL_PASS.

A regra vigente é: um scheduler por instância de máquina, com vários domínios de clock; nunca um scheduler autônomo por CPU/VDP/PSG. A integração deve aguardar auditoria dos contratos atuais e vetores/traces de regressão. Para o plano multi-plataforma e o motor Z80 único, consulte [reorganização arquitetural](migrations/shared-z80-runtime-reorganization.md).
