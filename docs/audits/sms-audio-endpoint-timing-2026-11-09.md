# Instrumentação temporal dos endpoints de áudio SMS

**Data:** 2026-11-09
**Baseline de implementação:** `ce4d14763fee258d5eea9aea109920eb2dac58cd`
**HEAD integrado na main:** `c67056e3c4c9bb7839d819345cb6e7b45cce66ae` (PR #733)
**Branch de origem:** `diagnostics/sms-audio-endpoint-timing-ci-20261109`
**Integração:** PR [#733](https://github.com/leonardo201800478/sega-emulator/pull/733), merge squash em `c67056e3c4c9bb7839d819345cb6e7b45cce66ae`
**Issues:** [#640](https://github.com/leonardo201800478/sega-emulator/issues/640), [#659](https://github.com/leonardo201800478/sega-emulator/issues/659)
**Natureza:** instrumentação diagnóstica; não corrige nem declara resolvidos os sintomas.

## 1. Objetivo

Correlacionar a cadência do produtor SMS com o consumo observado nos endpoints WASAPI Shared e XAudio2, sem modificar a linha temporal emulada, os clocks do PSG, a FIFO ou a política de buffers.

O objetivo imediato é distinguir:

1. falta de dados na FIFO;
2. atraso/jitter no atendimento dos eventos WASAPI;
3. buffers ainda em voo ou esgotamento da source voice XAudio2;
4. diferenças entre cadência do produtor e serviço do backend.

## 2. Evidência já disponível na baseline

A baseline já contém instrumentação que deve ser reutilizada:

### Produtor SMS

`SmsTemporalWorker::execute_frame` emite o evento TRACE `SMS frame pacing diagnostic`, contendo:

- `frame_number`;
- `machine_time_start_ticks` e `machine_time_end_ticks`;
- `machine_time_delta_ticks`;
- `instructions` e `psg_samples`;
- `step_frame_elapsed_ns`;
- `audio_submit_elapsed_ns`;
- `frame_processing_elapsed_ns`;
- `pacing_budget_ns` e `pacing_wait_ns`;
- `publish_audio`.

### FIFO

A fila SPSC expõe capacidade, ocupação observada, high-water, frames aceitos/consumidos, descartes, frames de silêncio/underrun e frames descartados em flush. Esses contadores são snapshots independentes e não formam uma transação global.

### WASAPI

O evento `WASAPI render buffer submitted` já registra frames submetidos, frames lidos da FIFO, frames substituídos por silêncio e ocupação FIFO após a leitura.

### XAudio2

O evento `XAudio2 source buffer submitted` já registra tamanho submetido e quantidade de buffers enfileirados. O callback `OnBufferEnd` incrementa um contador atômico sem executar logging ou trabalho pesado.

## 3. Incremento desta branch

### 3.1 WASAPI Shared

O evento `WASAPI render buffer submitted` passa a incluir:

| Campo | Unidade | Semântica |
|---|---|---|
| `endpoint_buffer_frames` | frames estéreo | Capacidade obtida na inicialização do endpoint |
| `endpoint_available_frames` | frames estéreo | Espaço disponível informado por `get_available_space_in_frames()` no evento |
| `endpoint_padding_frames` | frames estéreo | `endpoint_buffer_frames - endpoint_available_frames`; padding derivado |
| `event_interval_ns` | ns | Intervalo monotônico entre observações de eventos de renderização |

O intervalo inicial é zero por não existir observação anterior na mesma sessão de endpoint. A medição é feita após a consulta do espaço disponível.

A documentação da crate `wasapi` define `get_available_space_in_frames()` como capacidade menos padding corrente, e `get_current_padding()` como frames atualmente pendentes. Fonte: https://docs.rs/wasapi/latest/src/wasapi/api.rs.html. O campo calculado nesta branch é derivado do espaço livre e da capacidade já consultados; não adiciona uma chamada COM extra.

### 3.2 XAudio2

O novo evento TRACE `XAudio2 buffer completions observed` registra:

| Campo | Unidade | Semântica |
|---|---|---|
| `callback_completed_buffers` | buffers | Contador acumulado de callbacks `OnBufferEnd` |
| `callback_completion_delta` | buffers | Incremento observado desde a última leitura |
| `completion_observation_interval_ns` | ns | Intervalo monotônico entre observações do contador pelo worker |
| `buffers_queued` | buffers | `BuffersQueued` retornado por `GetState` |
| `in_flight_frames` | frames estéreo estimados | `BuffersQueued × frames_per_buffer` |
| `fifo_queued_frames` | frames estéreo | Ocupação FIFO observada no mesmo ponto |

**Limite de precisão:** `completion_observation_interval_ns` não é timestamp do callback nem intervalo exato entre conclusões físicas. O worker observa o contador durante seu loop de polling (atualmente 2 ms). O primeiro intervalo inclui o tempo desde a inicialização do worker até a primeira conclusão observada; os seguintes medem intervalos entre observações. O callback permanece restrito a incrementar um contador atômico.

A estimativa `in_flight_frames` assume buffers de tamanho fixo `frames_per_buffer`, coerente com o caminho atual de submissão XAudio2. Não deve ser rotulada como frames efetivamente reproduzidos pelo DAC.

## 4. Restrições preservadas

- MachineScheduler continua sendo a única autoridade do tempo emulado.
- Nenhuma chamada de backend avança, atrasa ou corrige o Scheduler.
- Nenhum clock de CPU/PSG/VDP foi alterado.
- Nenhum tamanho de buffer, prebuffer ou capacidade FIFO foi alterado.
- Nenhum algoritmo de PSG, FIR, mixer ou resampler foi alterado.
- Nenhum callback por instrução foi introduzido.
- O callback XAudio2 não faz logging, alocação, lock ou consulta COM.
- Nenhum PCM, ROM comercial ou dump protegido é versionado.

## 5. Interpretação dos dados

Os campos devem ser correlacionados com o evento `SMS frame pacing diagnostic` e com os contadores FIFO:

- Se `endpoint_available_frames` aumentar enquanto `fifo_queued_frames` cai a zero, investigar cadência/jitter do produtor versus demanda do endpoint.
- Se a FIFO permanecer abastecida, mas XAudio2 observar `buffers_queued = 0`, investigar a submissão e o ciclo de vida da source voice.
- Se ambos os backends receberem a mesma sequência PCM, mas o loopback diferir, concentrar a análise no formato, conversão do sistema e endpoint.
- Se a distorção já existir antes da FIFO, retornar à cadeia PSG → PCM16 e medir a primeira fronteira divergente.

Nenhum desses padrões isolados prova a causa raiz. São critérios de triagem para uma captura controlada.

## 6. Validação

| Atividade | Estado |
|---|---|
| Auditoria da baseline e dos pontos de instrumentação | PASS |
| Inspeção estática do patch | PASS |
| `cargo fmt --all -- --check` | NOT RUN — sem checkout local |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | NOT RUN — sem checkout local |
| `cargo test --workspace --locked` | NOT RUN — sem checkout local |
| CI Linux audio-output | PASS — workflow 42695223070, fmt/Clippy/testes |
| CI Windows WASAPI/XAudio2 | PASS — workflow 42695223070, fmt/Clippy/testes |
| Loopback WASAPI físico | BLOCKED — depende de Windows e endpoint real |
| Captura física XAudio2 | BLOCKED — depende de Windows e endpoint real |
| Windows egui fmt/Clippy/testes | PASS — workflow 42695223070 |
| D3D12/DXGI | NOT RUN — steps SKIPPED por classificação de impacto |
| Workspace, PSG, Scheduler, Z80, SDL3, PowerShell | NOT RUN — SKIPPED por classificação de impacto |
| Causa raiz #640/#659 | UNKNOWN |

A baseline `ce4d1476` tinha 15 checks: três PASS e doze SKIPPED. Workspace regression, PSG, Linux audio-output, Windows WASAPI/XAudio2 e Scheduler stress não foram executados nessa baseline.

**Revalidação CI:** workflow [42695223070 / CI 6156](https://github.com/leonardo201800478/sega-emulator/actions/runs/42695223070), executado no HEAD `0b78d93b59feba73f734f0eb2ddacb6a84eeafd3`, concluiu SUCCESS. A primeira execução Windows identificou E0793 porque o macro de tracing tomava referência a `XAUDIO2_VOICE_STATE::BuffersQueued`, campo de uma estrutura FFI empacotada. Corrigido em `9b50f0ae4e34f49a48ec03610bb3d566e269ad20`, copiando o campo para variável local alinhada antes do tracing e da comparação de starvation. Linux audio-output e Windows WASAPI/XAudio2 passaram fmt, Clippy e testes; Windows egui também passou fmt, Clippy e testes. D3D12/DXGI e as suítes fora do escopo foram SKIPPED e devem ser classificadas como NOT RUN.

## 7. Próxima etapa após CI

1. Executar a build candidata em Windows x64 com logging TRACE habilitado.
2. Reproduzir a mesma ROM/homebrew, região, trecho, backend, volume e duração.
3. Capturar logs do produtor e dos dois consumidores.
4. Capturar loopback do Windows para ambos os backends.
5. Comparar duração, frequência fundamental, gaps, RMS, pico e clipping.
6. Somente então definir uma correção mínima, acompanhada de teste que reproduza o defeito.

**Estado final desta entrega:** instrumentação integrada na `main` pela PR #733 (`c67056e3c4c9bb7839d819345cb6e7b45cce66ae`); CI aplicável PASS; D3D12/DXGI e regressões não relacionadas NOT RUN; loopback físico BLOCKED; nenhuma correção acústica declarada.
