# P0 — Plano e registro de validação do frame pacing

- Data: 2026-11-13
- Estado: **BLOQUEIO P0**
- HEAD de referência: `2942abfe0d4754dd857be0bc103d594abfd9b865`

## Estado do CI

### main — HEAD 2942abf

Workflows CI e SMS Debugger Runtime concluídos com sucesso.

Verificações relevantes aprovadas:
- SMS Runtime Release format, Clippy e testes.
- Rust workspace regression.
- Z80 focused validation.
- Windows x64 CPAL audio output.
- Windows x64 Slint + WGPU 30 + Vulkan POC.
- Documentation patch hygiene.

Verificações puladas no run associado ao HEAD:
- Scheduler deterministic stress.
- PSG/VDP/input core integration e algumas validações específicas por classificação de impacto.

Referências:
- https://github.com/leonardo201800478/sega-emulator/actions/runs/43359000469
- https://github.com/leonardo201800478/sega-emulator/actions/runs/43359000490

### PR #795

Permanece aberta e não integrada. A falha de Documentation patch hygiene decorre de whitespace final em dois documentos. O job agregador Rust quality falhou pela dependência documental; não há evidência de falha de teste Rust nessa PR documental. As alterações de documentação devem passar por nova execução de CI, sem merge automático.

## Integridade dos artefatos

Os percentis resumidos nesta investigação foram fornecidos para a janela final de 512 observações.

A árvore atual da main não contém os logs brutos do par de captura de 13/11 nem CSV de apresentação física. O arquivo histórico de captura de 12/11 não deve substituir a captura atual: foi produzido antes da integração H3 de 48 kHz da PR #774.

Preservar o arquivo histórico original. Não sobrescrever logs. A próxima captura deve manter, no mínimo:
- log bruto do worker com janela e configuração identificadas;
- metadados de ROM, região, build, commit, configuração de diagnóstico e modo de áudio;
- CSV/trace PresentMon correspondente;
- QPC start/end e frequência;
- separação clara entre vídeo-only e CPAL ativo.

## Experimentos isolados obrigatórios

### A — step_frame_until sem pacing

Executar a mesma ROM/estado por duração fixa, sem espera de parede. Medir throughput, ticks virtuais, instruções e estado final. Não modificar clocks ou semântica.

### B — recv_timeout com medição no ponto exato

Registrar o timeout efetivamente passado ao `recv_timeout`, timestamp QPC imediatamente antes/depois e deadline absoluto. Separar tempo pré-wait, tempo bloqueado e pós-wait.

### C — fase global versus reancoragem por frame

Comparar em branch experimental:
- deadline atual: `frame_started + frame_duration`;
- fase global: deadline anterior + duração virtual do frame.

Manter o mesmo mecanismo de espera. Verificar se lateness de um frame é recuperada no seguinte sem acelerar o hardware nem descartar frames virtuais.

### D — mecanismo de espera

Somente depois de B, comparar `recv_timeout` com timer de alta resolução Windows em espera one-shot. O comando do worker deve continuar interrompendo a espera. Medir energia, wake latency e comportamento de shutdown.

### E — CPAL ativo versus vídeo-only

Manter ROM, estado, build, duração e diagnóstico constantes. A diferença já observada não indica CPAL físico como causa primária.

### F — instrumentação ligada/desligada

O desktop atual chama `set_diagnostics_enabled(true)` ao criar/substituir o worker. Comparar com diagnóstico desabilitado usando medição externa independente, para quantificar overhead de mutex, timestamps e janelas de telemetria.

### G — Scheduler e alocações

Contabilizar despachos por frame e alocações no caminho `calculate_advances`. Fazer perfil CPU/allocator em Release. Não otimizar antes de demonstrar contribuição material.

### H — vídeo e apresentação

Executar com consumidor determinístico e sem consumidor, mantendo worker idêntico. Usar PresentMon para caracterizar produção, consumo, submit e apresentação física. Não usar V-Sync como clock da máquina.

## Testes de regressão antes de qualquer correção

- Determinismo e equivalência contínua versus em blocos.
- Estado final e ticks virtuais equivalentes.
- Pause/resume/reset/load e comandos durante pacing.
- Overrun e recuperação de deadline.
- PSG e contagem de amostras por frame contra o clock virtual.
- SMS e SG-1000.
- cargo fmt, Clippy `-D warnings`, crates afetados e workspace aplicável.
- Windows x64 Release e CI completo.
- Captura física pareada de pelo menos cinco minutos por cenário.

## Limitações atuais

Não há clone/worktree local disponível neste ambiente para certificar o estado da árvore de trabalho ou executar build local. A referência remota da main foi verificada. Nenhum arquivo de runtime foi alterado e nenhuma alteração foi publicada diretamente na main.

A validação física depende de execução no Windows x64 alvo e de artefatos brutos correspondentes.


## Evidência complementar da Library — 13/11

Foi encontrado `Texto colado(10).txt`, criado em 13/11 às 04:22 UTC, com logs R2/R3 de uma captura anterior/mais curta após a PR #793. Ele confirma taxa PCM de 48 kHz, `psg_samples_p50=801`, ticks/frame p50 59.736, worker interval p50 17.332 µs e timeout lateness p50 525 µs. Também registra zero command wakeups e zero p95 de command handler nessa janela.

O bloco R3_AUDIO_HEALTH correspondente registra 62.229 frames de underrun e 386 eventos, sem dropped frames, dropped blocks ou output_error. Isso deve ser correlacionado com a taxa de produção do worker; não deve ser confundido com a captura pareada final de cinco minutos. O arquivo permanece na Library e não foi alterado nem substituído. O CSV/trace PresentMon e os logs brutos da captura pareada final ainda precisam ser associados aos dados mais recentes.

## Experimento de deadline phase-locked

A PR draft #796 foi criada a partir do HEAD 2942abf. O modo experimental é opt-in por `SEGA_P0_PHASE_LOCKED_PACING=1`; o padrão permanece inalterado. Os testes Rust, Clippy e documentação passaram na execução atual. As validações Windows específicas foram puladas pela classificação de impacto; portanto, Windows x64 e captura física continuam pendentes.

PR: https://github.com/leonardo201800478/sega-emulator/pull/796
