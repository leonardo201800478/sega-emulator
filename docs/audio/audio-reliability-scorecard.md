# Scorecard — confiabilidade de áudio (SMS / Hosts compartilhados)

**Baseline de referência:** `main` = `c67056e3c4c9bb7839d819345cb6e7b45cce66ae` (2026-11-09).
**Estado geral: BLOCKED** — investigação de qualidade sonora e validação física pendentes.
**Escopo:** geração digital, conversão/resampling, transporte PCM, worker e saídas WASAPI/XAudio2.
**Autoridade temporal:** MachineScheduler; backend de áudio não pode modificar clocks da máquina.

Este scorecard acompanha o trabalho de áudio separado do scorecard Z80. Não altera a sequência E4C→E5→E6→E7→E8→E9 nem libera E8.

## 1. Matriz de trabalho

| ID | Entrega | Critério verificável | Estado |
|---|---|---|---|
| AUDIO-00 | Baseline e contrato de medição | SHA, dispositivos, taxa, cenário, unidades e semântica dos contadores documentados | PARTIAL — contrato documentado; endpoint físico NOT RUN |
| AUDIO-01 | Rastrear PSG → PCM | Mapa de funções/formatos/unidades e capturas nos limites do pipeline | PARTIAL — testes determinísticos de cadência/chunking; capturas de fronteira NOT RUN |
| AUDIO-02 | Taxa efetiva e cadência | Contagem de frames por segundo, intervalos e deriva medidos contra referência | PARTIAL — razão de amostras e invariância por chunk testadas; taxa efetiva do dispositivo NOT RUN |
| AUDIO-03 | Resampler e continuidade | Testes de fase, chunking, flush/epoch e round-trip de formato | PARTIAL — chunking em 22.05/44.1/48/96 kHz; validação física/epoch completa pendente |
| AUDIO-04 | FIFO e contabilização | Underflow em frames, silêncio, overflow, flush e perdas separados | PARTIAL — flush contado separadamente; CI Linux/Windows PASS |
| AUDIO-05 | Estado terminal dos backends | Falha XAudio2/WASAPI impede escrita falsa de sucesso e é exposta ao worker/UI | PARTIAL — escritas rejeitadas após estado terminal; SubmitSourceBuffer contabilizado; CI Windows PASS |
| AUDIO-06 | Métricas por backend | FIFO, endpoint starvation, WASAPI padding e buffers XAudio2 em voo separados | PARTIAL — instrumentação integrada na PR #733; Linux audio-output e Windows WASAPI/XAudio2 CI PASS; loopback físico pendente |
| AUDIO-07 | Lifecycle | Pause/Resume/Stop/Reset/Load/Close/StepFrame com epoch e snapshot coerentes | PARTIAL — snapshots após Start/Pause/Stop/Reset/Load; validação de endpoint pendente |
| AUDIO-08 | Comparação PCM comum | Mesma sequência PCM testada em WASAPI e XAudio2 | NOT RUN |
| AUDIO-09 | Validação física Windows | Loopback, waveform, taxa efetiva, pitch, clipping e gaps em ambos | NOT RUN |
| AUDIO-10 | Regressão sistêmica | fmt, Clippy, workspace, host Windows e SG-1000 no mesmo SHA | PARTIAL — #733 integrada; CI 6156 PASS para Linux audio-output, Windows WASAPI/XAudio2, Windows egui fmt/Clippy/testes, docs e impact; workspace, PSG, Scheduler, Z80, SDL3, PowerShell e D3D12/DXGI NOT RUN/SKIPPED |

## 2. Achados bloqueadores

| Achado | Prioridade | Estado |
|---|---:|---|
| Qualidade ruim nos dois backends apesar de poucos/nenhum underrun (#659) | P1 | ROOT CAUSE UNKNOWN |
| Underruns WASAPI relatados (#640) | P1 | ROOT CAUSE UNKNOWN |
| Starvation XAudio2 ausente do snapshot comum | P1 | CORRIGIDO NA PR #666; integrada em `a5bf770bcd8ff419d74a113d9982db38e6aae42c` |
| XAudio2 pode aceitar PCM após falha terminal do worker | P1 | CORRIGIDO NA PR #666; integrada em `a5bf770bcd8ff419d74a113d9982db38e6aae42c` |
| Snapshot de ocupação obsoleto após flush/transições | P1 | CORRIGIDO NA PR #666; integrada em `a5bf770bcd8ff419d74a113d9982db38e6aae42c` |
| Falha de SubmitSourceBuffer pode perder PCM sem reporte | P2 | CONTABILIZADO NA PR #666; backend terminal; integrada em `a5bf770bcd8ff419d74a113d9982db38e6aae42c` |
| Taxa configurada pode ser confundida com taxa efetiva | P2 | SEMANTIC GAP |
| FIFO consumida pode ser confundida com áudio reproduzido | P2 | SEMANTIC GAP |

## 3. Contrato de métricas

| Métrica | Significado correto | Não significa |
|---|---|---|
| accepted_frames | Frames publicados na FIFO | Frames ouvidos |
| consumed_frames | Frames removidos da FIFO/entregues ao próximo estágio | Reprodução confirmada |
| dropped_frames | Frames rejeitados por FIFO cheia | Perdas por flush/submit, a menos que explicitamente incluídas |
| underrun_frames | Frames de silêncio inseridos pelo consumidor da FIFO | Starvation XAudio2 automaticamente |
| underrun_events | Leituras/eventos com falta | Número de frames ausentes |
| endpoint_starvation_events | Eventos em que buffers de voz/endpoint esgotaram | Frames de silêncio efetivamente ouvidos |
| flushed_frames | Frames descartados intencionalmente ao invalidar epoch | Overflow |
| in_flight_frames | Frames entregues ao backend ainda não confirmados como concluídos | Ocupação da FIFO |
| configured_sample_rate_hz | Taxa solicitada/configurada | Taxa física efetiva do endpoint |
| endpoint_sample_rate_hz | Formato/taxa negociados, quando consultáveis | Taxa de conversão efetiva se o SO fizer SRC oculto |
| fifo_backlog_micros | Duração equivalente somente da FIFO | Latência ponta a ponta |

Os contadores precisam definir epoch, ponto de incremento e unidade. Snapshots independentes de atomics são diagnósticos e não constituem uma transação global.

## 4. Gates de validação

| Gate | Status atual | Condição para PASS |
|---|---|---|
| Inspeção estática da cadeia | PASS (escopo da auditoria) | Referências de arquivo/função e semântica registradas |
| Testes unitários do pipeline/resampler | NOT RUN | Execução no SHA candidato |
| Testes FIFO concorrentes e lifecycle | NOT RUN | Produtor/consumidor, flush e falha cobertos |
| Rust fmt | NOT RUN para esta trilha | `cargo fmt --all -- --check` |
| Rust Clippy | NOT RUN para esta trilha | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Rust workspace tests | NOT RUN para esta trilha | `cargo test --workspace --locked` |
| Linux audio-output | PASS no CI 6156 / HEAD `0b78d93b` | Job fmt, Clippy e testes executado |
| Windows WASAPI/XAudio2 | PASS no CI 6156 / HEAD `0b78d93b` | Job fmt, Clippy e testes executado |

| Loopback WASAPI | NOT RUN | Captura e análise documentadas |
| XAudio2 physical endpoint | NOT RUN | Captura e análise documentadas |
| Comparação mesma sequência PCM | NOT RUN | Vetor idêntico e resultados comparados |
| Regressão SG-1000 | NOT RUN nesta auditoria | Testes aplicáveis PASS no SHA candidato |

## 5. Política de promoção de estado

- `NOT RUN`: não executado.
- `BLOCKED`: não foi possível executar ou depende de pré-condição.
- `FAIL`: executado e falhou.
- `PASS`: executado com evidência reproduzível no SHA declarado.
- `SKIPPED` no CI deve ser registrado como `NOT RUN`, nunca como PASS.
- Evidência manual e CI são categorias separadas; uma não substitui a outra.
- Mudanças devem ser pequenas, compiláveis e validadas. A publicação direta na `main` está autorizada pelo mantenedor, mas as regras de proteção do repositório podem exigir PR; gates não podem ser ignorados.

## 6. Gate para sair da prioridade de áudio

A trilha só pode ser considerada concluída após fechar AUDIO-00 a AUDIO-10, demonstrar a causa dos sintomas #640/#659, validar os dois backends em Windows e provar que não houve alteração indevida no tempo determinístico da máquina. A aprovação de áudio não concede automaticamente TEMPORAL_PASS ao SMS nem altera o scorecard Z80.

## 7. Incremento de correções — PR #666

**PR:** [#666 — harden host failure handling and PCM telemetry](https://github.com/leonardo201800478/sega-emulator/pull/666)
**HEAD validado:** `29916f7ddb27a069b385a57e53d04760de5e01ed`.
**Merge:** `a5bf770bcd8ff419d74a113d9982db38e6aae42c` em 2026-11-08.
**CI do HEAD da PR:** workflow [42489225235](https://github.com/leonardo201800478/sega-emulator/actions/runs/42489225235) — SUCCESS.

- PASS: Rust workspace regression (fmt, Clippy e testes).
- PASS: Linux audio-output host validation.
- PASS: Windows WASAPI/XAudio2 (fmt, Clippy e testes automatizados dos backends).
- PASS: formatação e Clippy da aplicação Windows; documentação e classificação de impacto.
- SKIPPED/NOT RUN: stress do Scheduler, validação Z80, SDL3 e PowerShell; não houve alterações nessas áreas.
- Não houve captura loopback nem reprodução em endpoint físico. A PR #666 está integrada na `main`; sua integração não representa aprovação acústica nem encerra as issues #640/#659.

A CI confirma compilação e comportamento automatizado, não qualidade audível. A issue #659 e a causa dos underruns #640 permanecem abertas. O próximo gate é AUDIO-08/AUDIO-09: comparar a mesma sequência PCM e validar WASAPI/XAudio2 fisicamente.


## 8. Atualização da baseline e instrumentação de endpoint — 2026-11-09

**Baseline anterior:** `ce4d14763fee258d5eea9aea109920eb2dac58cd`.
**HEAD integrado na main:** `c67056e3c4c9bb7839d819345cb6e7b45cce66ae` (PR #733, squash).

A PR #661 (interpolação fracionária do SN76489) e a PR #666 (tratamento de falhas e telemetria PCM) estão integradas na `main`. A issue #640 e a issue #659 continuam abertas; não há causa raiz demonstrada nem validação física de loopback.

### Instrumentação já existente antes deste incremento

- `SmsTemporalWorker::execute_frame` emite `SMS frame pacing diagnostic`, com número do frame, timestamps lógicos da máquina, amostras PSG, tempos de execução/submissão e orçamento/espera de pacing.
- A FIFO expõe ocupação, high-water, frames aceitos/consumidos, descartes, silêncio e flush.
- WASAPI emite `WASAPI render buffer submitted`, com frames submetidos/consumidos, silêncio e ocupação FIFO.
- XAudio2 emite `XAudio2 source buffer submitted`, com quantidade de buffers submetidos.

### Incremento integrado — PR #733

A instrumentação descrita abaixo foi integrada na `main` pelo commit `c67056e3c4c9bb7839d819345cb6e7b45cce66ae`.

- WASAPI: adicionar capacidade do endpoint, espaço disponível, padding derivado e intervalo monotônico entre eventos de renderização.
- XAudio2: observar a variação do contador de callbacks `OnBufferEnd`, intervalo entre observações, buffers ainda enfileirados, frames estimados em voo e ocupação FIFO.
- Os intervalos XAudio2 são intervalos de **observação pelo worker**, não timestamps exatos de cada callback. O callback continua limitado a incrementar um contador atômico.
- Os valores são registros TRACE; não representam taxa física efetiva, latência DAC ou confirmação de que o áudio foi ouvido.
- Nenhum clock, Scheduler, tamanho de buffer, prebuffer, FIFO, algoritmo PSG ou conversão PCM é alterado.

### Validação deste incremento

**Revalidação CI:** workflow [42695223070 / CI 6156](https://github.com/leonardo201800478/sega-emulator/actions/runs/42695223070) no HEAD `0b78d93b59feba73f734f0eb2ddacb6a84eeafd3` — SUCCESS. A execução inicial falhou em Windows com E0793 ao referenciar diretamente `XAUDIO2_VOICE_STATE::BuffersQueued`; corrigido em `9b50f0ae4e34f49a48ec03610bb3d566e269ad20` copiando o campo para variável local alinhada.

| Suíte CI 6156 | Estado |
|---|---|
| Linux audio-output: fmt, Clippy, testes | PASS |
| Windows WASAPI/XAudio2: fmt, Clippy, testes | PASS |
| Windows egui: fmt, Clippy, testes | PASS |
| Documentação e classificação de impacto | PASS |
| D3D12/DXGI | NOT RUN — steps SKIPPED pela classificação de impacto |
| Workspace completo, PSG, Scheduler, Z80, SDL3, PowerShell | NOT RUN — SKIPPED pela classificação de impacto |
| Loopback físico / validação acústica | BLOCKED — endpoint Windows real necessário |

| Suíte | Estado |
|---|---|
| Inspeção estática dos pontos de instrumentação | PASS |
| `cargo fmt --all -- --check` | NOT RUN — sem checkout local nesta sessão |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | NOT RUN — sem checkout local nesta sessão |
| `cargo test --workspace --locked` | NOT RUN — sem checkout local nesta sessão |
| CI Windows WASAPI/XAudio2 no HEAD da branch | PASS — CI 6156 |
| Captura de loopback e comparação acústica | BLOCKED — depende de endpoint Windows físico |

A CI da baseline `ce4d1476` teve 15 checks: 3 PASS e 12 SKIPPED. Os jobs de workspace, PSG, Linux audio-output, Windows WASAPI/XAudio2 e Scheduler stress foram NOT RUN. Não promover esses resultados a PASS.

**Decisão:** PR #733 integrada. Manter #640 e #659 abertas. Não ajustar buffers ou clocks até analisar as novas séries temporais e executar captura física.
