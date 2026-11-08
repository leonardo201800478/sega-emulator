# OUT-F — telemetria da saída PCM (primeiro incremento)

## Objetivo

Expor ao runtime e à interface diagnósticos não bloqueantes do transporte PCM estéreo por frames, sem alterar o relógio da máquina, o MachineScheduler, o PSG, o VDP ou o DSP determinístico.

## Métricas expostas

A estrutura AudioOutputMetrics representa um snapshot:

| Campo | Unidade | Semântica |
|---|---:|---|
| sample_rate_hz | frames/s | Taxa configurada na saída host |
| queue_capacity_frames | frames estéreo | Capacidade fixa da FIFO |
| queue_queued_frames | frames estéreo | Ocupação observada no instante da leitura |
| queue_high_water_frames | frames estéreo | Maior ocupação observada após publicação |
| accepted_frames | frames estéreo | Frames publicados com sucesso |
| consumed_frames | frames estéreo | Frames retirados pelo consumidor |
| dropped_frames | frames estéreo | Frames mais novos rejeitados por FIFO cheia |
| underrun_frames | frames estéreo | Frames de silêncio inseridos por falta de dados |
| underrun_events | eventos | Leituras que precisaram inserir ao menos um frame de silêncio |
| dropped_blocks | chamadas | Escritas PCM que rejeitaram ao menos um frame |
| flushed_frames | frames estéreo | Frames descartados da FIFO pelo consumidor ao invalidar uma epoch |
| endpoint_starvation_events | eventos | Transições para source voice sem buffers enfileirados no XAudio2; zero no WASAPI |
| submission_failed_frames | frames estéreo | Frames retirados da FIFO e contabilizados como perdidos quando SubmitSourceBuffer falha no XAudio2 |

Os contadores da FIFO são compartilhados entre produtor e consumidor por atomics. `flushed_frames` é distinto de overflow e consumo audível. `endpoint_starvation_events` não deve ser somado a `underrun_events`: são eventos de camadas e semânticas diferentes. `submission_failed_frames` representa PCM retirado da FIFO mas não aceito pela source voice. A ocupação é uma observação instantânea e pode mudar imediatamente após a leitura. Os campos independentes não formam uma transação atômica; o snapshot é diagnóstico, não um ponto de sincronização.

## Estimativa de backlog

queue_duration_micros() calcula:

queued_frames × 1.000.000 / sample_rate_hz

A estimativa representa somente o tempo equivalente aos frames atualmente enfileirados na FIFO SPSC. Não inclui buffers do endpoint, driver, mixer do sistema operacional, DAC ou dispositivo físico. Portanto, não deve ser apresentada como latência ponta a ponta.

A estimativa de high-water aplica a mesma conversão ao maior backlog observado.

## Integração

- WASAPI e XAudio2 expõem o mesmo contrato PcmSink::metrics() -> AudioOutputMetrics.
- O runtime SMS copia o snapshot para atomics de telemetria; a GUI apenas lê esses valores.
- A thread/callback de áudio não passa a executar lógica de interface, logging, alocação ou controle do Scheduler.
- A GUI mostra taxa configurada, ocupação/capacidade, backlog estimado, high-water, FIFO underrun em eventos/frames, starvation XAudio2, overflow, flush, perdas em SubmitSourceBuffer e frames aceitos/consumidos.

## Incremento de confiabilidade — 2026-11-08

- O consumidor registra `flushed_frames` ao descartar a fila durante uma transição de epoch; o snapshot é atualizado no worker após Start/Pause/Stop/Reset/Load ROM.
- XAudio2 propaga o contador de starvation da source voice separadamente dos underruns da FIFO.
- Falha em `SubmitSourceBuffer` marca o backend como terminal, contabiliza os frames retirados da FIFO e faz novas chamadas `write_pcm` retornarem erro em vez de continuar aceitando PCM sem consumidor ativo.
- A telemetria terminal é capturada antes do fechamento da sessão para não apagar o diagnóstico.

## Limites desta entrega

Este incremento fecha a exposição das métricas já existentes da FIFO e sua visualização. Ele **não** implementa ainda:

- latência medida do endpoint/driver;
- duração e distribuição temporal dos episódios de underrun;
- low-water mark;
- métricas de período/padding específicas de WASAPI;
- buffers em voo, amostras tocadas e callbacks específicos de XAudio2;
- tuning automático ou alteração dinâmica da capacidade/prebuffer.

Essas métricas exigem instrumentação específica por backend e validação no mesmo dispositivo/cenário. Não se deve ajustar buffer, prebuffer, número de blocos ou taxa de resampling antes de coletar uma linha de base comparável. O tuning futuro deverá ser determinístico, limitado por capacidades reportadas pelo endpoint e sem alterar clocks da máquina.

## Validação esperada

- Testes unitários verificam conversão de unidades e mapeamento completo de PcmFrameQueueMetrics para AudioOutputMetrics.
- CI deve compilar os dois backends e o frontend que consome o novo contrato.
- Testes in-game físicos permanecem separados: executar o mesmo cenário com WASAPI e XAudio2, registrar as métricas e comparar os resultados sem modificar a configuração entre medições.

## Reconciliação da base — 2026-11-08

A PR foi retargeteada para a `main` após a integração da correção de amostragem fracionária do SN76489 (PR #661, merge SHA `af24e77018257b77293a28e2f8e1ac12d0804aa4`).

A composição de merge gerada pelo GitHub inclui também a correção de ciclo de saída da PR #658. A inspeção do arquivo `native/audio_runtime/src/session.rs` na árvore de merge confirmou:
- `suspended` é marcado somente após `suspend_and_flush()` bem-sucedido;
- `resume()` limpa o estado após retomar o sink;
- `close()` descarta a cauda FIR quando o epoch foi invalidado por suspensão/flush ou por amostras de debugger descartadas.

Essa inspeção é uma verificação estrutural do merge, não substitui os testes executados sobre o merge resultante. O CI deve ser reexecutado após a atualização da base antes da integração.

A documentação geral da investigação foi integrada à `main` pela PR #663, merge SHA `d6d14cbcc78dd5d3b39cabfe7d03a857c59034ed`. A validação de higiene documental dessa integração passou no workflow #42481945972.
