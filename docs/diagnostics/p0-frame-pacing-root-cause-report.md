# P0 — Relatório preliminar de causa raiz do frame pacing

- Data: 2026-11-13
- Estado: **ABERTO — causa raiz não confirmada**
- Bloqueio: **MANTIDO**
- Base auditada: `2942abfe0d4754dd857be0bc103d594abfd9b865`
- Issue: #755
- PR de investigação: #795

## Resumo executivo

O runtime mantém progressão virtual por frame próxima do esperado, mas a cadência de parede do worker é aproximadamente 4% inferior ao orçamento regional.

A evidência atual prioriza o caminho de espera do worker e a política de deadline por frame. Não há base para atribuir a regressão ao CPAL: o problema foi registrado antes da integração física de áudio e a captura vídeo-only não recuperou a cadência.

Nenhuma causa raiz foi declarada. Nenhuma alteração de runtime foi aplicada.

## Evidências de referência

Valores fornecidos para a janela final de 512 observações:

| Métrica | CPAL ativo | Vídeo-only |
|---|---:|---:|
| worker_frame_interval_p50 | 17.324 µs | 17.415 µs |
| worker_frame_interval_p95 | 19.600 µs | 20.404 µs |
| worker_frame_interval_p99 | 21.179 µs | 22.621 µs |
| worker_step_p50 | 11.108 µs | 13.644 µs |
| frame_processing_p50 | 11.140 µs | 13.664 µs |
| pacing_wait_budget_p50 | 5.542 µs | 3.022 µs |
| pacing_wait_elapsed_p50 | 6.130 µs | 3.664 µs |
| pacing_timeout_lateness_p50 | 449 µs | 373 µs |
| mailbox_replaced_total | 1.313 | 2.440 |

Orçamento nominal aproximado: 16.687 µs. Ticks virtuais por frame permanecem na faixa 59.717–59.755, conforme a captura fornecida.

Esses percentis são estatísticas independentes; não se deve somar ou subtrair percentis como se fossem observações pareadas.

## Fatos de código confirmados

### Pacing do worker

Em `rust/runtime/sms-runtime/src/worker.rs`:

1. `execute_frame_with_renderer` registra `frame_started = Instant::now()`.
2. A duração é derivada de `sample_count / SMS_INTERNAL_AUDIO_SAMPLE_RATE`.
3. O deadline é construído como `frame_started + frame_duration`.
4. `worker_loop` calcula o restante do deadline e chama `commands.recv_timeout(timeout)`.
5. Após o timeout, o deadline é limpo.
6. O próximo frame cria um novo `frame_started`.

A política é absoluta dentro do frame corrente, mas não mantém uma época global de cadência entre frames. Um despertar tardio não é compensado pelo deadline do frame seguinte.

A política equivalente já existia no worker temporal original da PR #571: `frame_wait = frame_duration - frame_started.elapsed()`, seguido de `recv_timeout(frame_wait)`.

### Lacuna de teste

O teste `frame_pacing_deadline_preserves_remaining_budget_across_commands` verifica que handlers não reiniciam o deadline do frame corrente. Não foi encontrado teste que injete lateness e verifique recuperação de fase entre frames sucessivos.

### Semântica da telemetria

`pacing_wait_budget` é registrado antes de `record_frame_pacing_sample` e `drain_machine_trace`. O argumento real de `recv_timeout` é recalculado depois no `worker_loop`. A telemetria ainda não registra diretamente esse argumento no instante da chamada.

`worker_frame_interval` é capturado na conclusão da etapa da máquina, antes de publicar vídeo e submeter áudio.

A métrica `pacing_timeout_lateness`, calculada diretamente como retorno da espera menos deadline, é a evidência direta mais útil sobre despertar tardio.

### Custo potencial do Scheduler

`MachineScheduler::dispatch_next` chama `calculate_advances`, que cria um novo `Vec<ClockAdvance>` com capacidade igual ao número de domínios. O SMS registra três domínios: Z80, PSG e VDP.

É um custo estrutural a medir por contagem de despachos e perfil de alocação. Ainda não há evidência de que explique o déficit sistemático de cadência.

## Matriz de hipóteses

| Hipótese | Estado | Evidência necessária |
|---|---|---|
| Progressão virtual incorreta | Sem evidência atual | Comparação de estado/ticks e determinismo |
| Deadline relativo por frame | Forte candidata | A/B com época global e lateness controlada |
| Wake lateness do Windows | Forte candidata | Medir timeout solicitado no ponto da chamada e retorno QPC |
| Custo do Scheduler | Aberta | Despachos/frame, alocações e perfil CPU |
| Conversão PSG/amostras | Aberta | Amostras por frame contra ticks e taxa PCM efetiva |
| Command handlers | Aberta | `command_handler_p95`, wakeups e eventos TRACE pós-deadline |
| Slint/FrameMailbox | Não explica sozinho o intervalo do worker | Execução sem consumidor e medição PresentMon |
| CPAL físico | Não é causa primária indicada | A/B pareado já não mostrou recuperação vídeo-only |
| Instrumentação | Aberta | Captura com diagnósticos ligados/desligados |

## Conclusão provisória

A primeira candidata histórica é a política de pacing introduzida com o worker temporal da PR #571, não a criação isolada do crate MachineScheduler em 01/11.

O mecanismo mais suspeito é a reancoragem do deadline em cada frame, combinada com a latência real de espera do Windows. A hipótese ainda exige experimento controlado.

**Não aplicar patch de pacing, timer de alta resolução ou otimização de Scheduler antes de concluir a comparação A/B.**

## Critério de liberação

P0 só pode ser liberada após causa vinculada a código/commit, determinismo preservado, testes de execução contínua/em blocos, regressões SMS/SG-1000, CI aplicável e captura física pareada de pelo menos cinco minutos por cenário, com apresentação física caracterizada.
