# Plano de corte dos hosts anteriores — Slint / WGPU / CPAL

**Revisão:** 2026-11-12 — reconciliado com main `f5a0b691824322ce56c1a921329d3aa0957c4963`.
**Autoridade arquitetural:** ADR 0011 — Slint, WGPU e CPAL
**Objetivo:** manter a documentação operacional alinhada à arquitetura-alvo e retirar implementações transitórias sem quebrar o runtime ou apagar evidências históricas.

## 1. Arquitetura normativa

| Camada | Tecnologia / responsabilidade |
|---|---|
| GUI desktop | Slint 1.18.x |
| API de renderização | WGPU 30.x |
| Backend gráfico inicial | Vulkan, selecionado explicitamente por WGPU no Windows x64 |
| Saída física de áudio | CPAL 0.18.2 |
| Entrada/gamepads | SDL3 após auditoria |
| Hardware, CPU, VDP, PSG, barramentos e tempo | Runtime 100% Rust; MachineScheduler é autoridade temporal |

O frontend não implementa hardware, instruções, ciclos, temporização nem callbacks por instrução. A integração permanece coarse-grained. O runtime validado, SmsTemporalWorker, FrameMailbox e MachineScheduler não devem ser reescritos para facilitar o corte de hosts.

## 2. Estado comprovado das fases

| Fase | Estado | Evidência |
|---|---|---|
| R1 — POC Slint/WGPU/Vulkan | MERGED | PR #747, merge ef6398eeaf12edf663846a5bf8392dd0033b6ea1 |
| R2 — frames SMS no desktop Slint/WGPU | MERGED | PR #752, merge 5706e9125e345230b1b9ddda860654ce9d0789a3 |
| R2 — preferências persistentes | MERGED | PR #754, merge 22ea3a44cd66bcd7e5c4edbd98007ca59eef9858 |
| R3 — áudio físico CPAL | INTEGRADO EM SOFTWARE | PR #770 merged; H3 48 kHz PR #774 merged. Validação física prolongada, hotplug/reconexão, underruns e formatos adaptativos ainda pendentes. |
| R4 — input SDL3 | PENDING | Auditoria e integração de teclado/gamepads/hotplug. |
| R5 — consolidação e corte | PENDING | Paridade, regressão, remoção de consumidores e validação Release. |

Merge de R1/R2 não prova áudio físico, input SDL3, paridade integral de frontend nem encerramento da migração.

## 3. Inventário transitório no código

A auditoria da árvore main em 2026-11-12 encontrou:

- native/sega_app: shell anterior ainda presente e com integração ao runtime/host gráfico anterior.
- native/video_runtime: módulo de apresentação Windows ainda expõe implementação anterior; há consumidores em código do shell/worker.
- native/audio_output: a main já usa CPAL 0.18.2 após PR #770; preservar FIFO SPSC e o contrato de sink.
- native/sega_desktop / runtime SMS: o caminho CPAL está integrado por PR #770 e a linha PCM interna de 48 kHz por PR #774; faltam gates de operação física prolongada, hotplug e formatos adaptativos.
- .github/workflows/ci.yml: existem nomes de checks de compatibilidade exigidos pela proteção da branch; os jobs correspondentes ficam SKIPPED em eventos normais e não validam os hosts antigos.

**Conclusão:** a main já utiliza CPAL 0.18.2 e a linha PCM interna de 48 kHz após os merges #770 e #774. A retirada física dos hosts gráficos anteriores permanece condicionada ao inventário de consumidores, à paridade Slint/WGPU e aos gates R5. Não remover módulos ou dependências apenas para eliminar ocorrências textuais; preservar os contratos e evidências necessários.

## 4. Critérios obrigatórios para retirada física

### 4.1 Host gráfico anterior

Antes de excluir o módulo e suas dependências:

1. confirmar que native/sega_desktop é o único desktop mantido;
2. pesquisar todos os usos de tipos, módulos, features e exemplos do host anterior;
3. mover qualquer contrato genérico reutilizável para uma crate neutra, se necessário, sem duplicar VideoFrame/mailbox;
4. remover consumidores antigos em passos pequenos;
5. retirar exemplos, dependências Windows, features e jobs de CI que não tenham mais consumidor;
6. executar workspace fmt, Clippy, testes e build Release Windows x64;
7. comprovar frame real, resize, fullscreen, shutdown, replay determinístico e campanha de apresentação no caminho Slint/WGPU;
8. atualizar esta documentação com SHA, PR e workflows finais.

### 4.2 Saída física de áudio anterior

A PR #770 foi integrada na main e substituiu os adaptadores próprios WASAPI/XAudio2 por CPAL 0.18.2, preservando FIFO SPSC e contrato de sink. A PR #774 consolidou a fonte PCM interna de 48 kHz e taxas explícitas. Isso fecha a integração de software, não a validação operacional completa.

R3 só deve ser declarada tecnicamente concluída após:
1. enumeração de dispositivos e formatos suportados;
2. seleção/abertura de stream e reprodução PCM real;
3. conversão de formato sem alterar a linha temporal do Scheduler;
4. lifecycle start/pause/stop/reset/close;
5. device removal/reconnection e erros;
6. métricas de underrun/overflow e ausência de bloqueio indevido no callback;
7. execução física prolongada em Windows Release, com logs e captura reproduzível;
8. negociação adaptativa de PCM16 e formatos superiores efetivamente suportados, sem impor 24 bits.


### 4.3 CI e proteção de branch

Os checks antigos podem continuar aparecendo como SKIPPED por exigência de proteção de branch. Não renomear/remover seus identificadores até confirmar e atualizar a regra de proteção no GitHub. Eles não podem ser apresentados como validação funcional. Após atualização da proteção, remover os shims e validar todos os checks obrigatórios no HEAD candidato.

## 5. Política documental

- README, AGENTS, roadmap, status de componentes e ADRs normativas devem descrever apenas Slint/WGPU/Vulkan, CPAL e SDL3 como arquitetura-alvo.
- ADRs e auditorias antigas permanecem preservadas como histórico, marcadas como superseded/archived quando necessário.
- Resultados passados não devem ser reescritos para fingir que foram executados sobre a arquitetura atual.
- Ocorrências em logs, relatórios históricos, hashes, evidências e changelogs são referências históricas, não dependências ativas.
- A busca final deve classificar cada ocorrência como: código ativo, CI/proteção, documentação normativa, evidência histórica ou referência externa.

## 6. Invariantes que o corte não pode violar

- MachineScheduler continua autoridade única do tempo emulado.
- Renderização, áudio físico e input não avançam nem corrigem ciclos do hardware.
- Não alterar CPU Z80, VDP, PSG, barramentos, WAIT states ou clocks regionais como efeito colateral da migração.
- Preservar SmsTemporalWorker, FrameMailbox, ownership e operações coarse-grained.
- Execução contínua e em blocos deve continuar equivalente; Save States e replays devem permanecer determinísticos.
- Nenhum corte é considerado concluído apenas porque compila: exigir testes semânticos, regressão, build Release e validação física aplicável.

## 7. Próximas ações

1. Fechar R3: CPAL end-to-end no desktop Slint.
2. Fechar R4: SDL3 após auditoria.
3. R5: tornar Slint o único frontend desktop mantido e remover consumidores anteriores.
4. Remover dependências, exemplos, CI transitória e documentação operacional antiga, preservando evidências históricas.
5. Publicar auditoria final de corte com lista de arquivos removidos, testes, SHA e workflows.


## 8. Atualização de estado — 2026-11-12

- R1 POC #747, R2 frames #752, preferências #754, CPAL #770 e H3 #774 estão integrados.
- R2 permanece em aceite parcial: issues #755 (P0 frame pacing) e #756 (viewport/ZIP/sprite-limit/menu/resume/fullscreen) continuam abertas.
- PR #757 (viewport), #758 (preservar sprite policy no reset) e #746 (mailbox portátil) precisam ser revalidadas/portadas para a main atual; não usar seus históricos divergentes como merge integral.
- Ver [Auditoria geral e plano de continuidade 2026-11-12](../../project-status-2026-11-12.md) para inventário de todas as PRs/issues abertas e sequência priorizada.
