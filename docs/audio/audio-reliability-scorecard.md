# Scorecard — confiabilidade de áudio (SMS / Hosts compartilhados)

**Baseline de referência:** `main` = `9731b37098e7937fa92fffc15007a7c79aac4e4c` (2026-11-08).
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
| AUDIO-06 | Métricas por backend | FIFO, endpoint starvation, WASAPI period/padding e buffers em voo separados | PARTIAL — FIFO/endpoint starvation/submit loss/flush expostos; WASAPI period e buffers em voo pendentes |
| AUDIO-07 | Lifecycle | Pause/Resume/Stop/Reset/Load/Close/StepFrame com epoch e snapshot coerentes | PARTIAL — snapshots após Start/Pause/Stop/Reset/Load; validação de endpoint pendente |
| AUDIO-08 | Comparação PCM comum | Mesma sequência PCM testada em WASAPI e XAudio2 | NOT RUN |
| AUDIO-09 | Validação física Windows | Loopback, waveform, taxa efetiva, pitch, clipping e gaps em ambos | NOT RUN |
| AUDIO-10 | Regressão sistêmica | fmt, Clippy, workspace, host Windows e SG-1000 no mesmo SHA | PARTIAL — fmt/Clippy/workspace/Linux audio/Windows WASAPI-XAudio2 PASS no SHA da PR #666; stress Scheduler SKIPPED |

## 2. Achados bloqueadores

| Achado | Prioridade | Estado |
|---|---:|---|
| Qualidade ruim nos dois backends apesar de poucos/nenhum underrun (#659) | P1 | ROOT CAUSE UNKNOWN |
| Underruns WASAPI relatados (#640) | P1 | ROOT CAUSE UNKNOWN |
| Starvation XAudio2 ausente do snapshot comum | P1 | CORRIGIDO NA PR #666; CI PASS; aguardando merge |
| XAudio2 pode aceitar PCM após falha terminal do worker | P1 | CORRIGIDO NA PR #666; CI PASS; aguardando merge |
| Snapshot de ocupação obsoleto após flush/transições | P1 | CORRIGIDO NA PR #666; CI PASS; aguardando merge |
| Falha de SubmitSourceBuffer pode perder PCM sem reporte | P2 | CONTABILIZADO NA PR #666; backend terminal; CI PASS; aguardando merge |
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
| Linux audio-output | SKIPPED no CI da baseline | Job executado e PASS no SHA candidato |
| Windows WASAPI/XAudio2 | SKIPPED no CI da baseline | Build/testes host executados no SHA candidato |
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
- Mudanças de código devem ser pequenas, em branch e PR; sem commit direto na main.

## 6. Gate para sair da prioridade de áudio

A trilha só pode ser considerada concluída após fechar AUDIO-00 a AUDIO-10, demonstrar a causa dos sintomas #640/#659, validar os dois backends em Windows e provar que não houve alteração indevida no tempo determinístico da máquina. A aprovação de áudio não concede automaticamente TEMPORAL_PASS ao SMS nem altera o scorecard Z80.

## 7. Incremento de correções — PR #666

**PR:** [#666 — harden host failure handling and PCM telemetry](https://github.com/leonardo201800478/sega-emulator/pull/666)
**HEAD validado:** `29916f7ddb27a069b385a57e53d04760de5e01ed`.
**CI:** workflow [42489225235](https://github.com/leonardo201800478/sega-emulator/actions/runs/42489225235) — SUCCESS.

- PASS: Rust workspace regression (fmt, Clippy e testes).
- PASS: Linux audio-output host validation.
- PASS: Windows WASAPI/XAudio2 (fmt, Clippy e testes automatizados dos backends).
- PASS: formatação e Clippy da aplicação Windows; documentação e classificação de impacto.
- SKIPPED/NOT RUN: stress do Scheduler, validação Z80, SDL3 e PowerShell; não houve alterações nessas áreas.
- Não houve captura loopback nem reprodução em endpoint físico. A PR permanece aberta e não está integrada na main.

A CI confirma compilação e comportamento automatizado, não qualidade audível. A issue #659 e a causa dos underruns #640 permanecem abertas. O próximo gate é AUDIO-08/AUDIO-09: comparar a mesma sequência PCM e validar WASAPI/XAudio2 fisicamente.
