# SMS1 VDP — scorecard de conclusão

**Sistema-alvo:** Sega Master System 1
**ASIC prioritário:** Sega 315-5124
**Backend:** Rust / `sega-video`
**Baseline integrada:** `main` após PR #841 (`c16e5133aab1aa668954737500383dfde44af6fe`)
**Baseline de comparação:** `202cdfc695d0c2346c86da4144b6e8a3316e0c4d`
**HEAD de validação da PR #841:** `82742d3c1b5fef3a45d0185529d5b9330f0d4180`
**Estado:** EM DESENVOLVIMENTO — NÃO LIBERADO PARA TESTE IN-GAME

Este scorecard registra somente capacidades presentes na `main` e evidências executadas no SHA correspondente. PR aberta, branch de trabalho ou teste ainda em execução não contam como implementação integrada.

## 1. Critérios funcionais

| Área | Estado | Evidência / limite |
|---|---|---|
| Perfil SMS1 / 315-5124 | IMPLEMENTED | Perfil explícito separado de SMS2, SG-1000 e Game Gear |
| DAC / paleta SMS1 | VALIDATED_FUNCTIONAL | PR #825; níveis digitais RGB específicos do SMS1. Não é calibração analógica completa |
| VRAM / CRAM / portas | IMPLEMENTED_FUNCTIONAL | Portas, command latch, read-ahead, wrap e endereçamento separados |
| Background Mode 4 | IMPLEMENTED_SCANLINE | Tiles, planos, paletas, prioridade, scroll e máscaras funcionais |
| Escritas CRAM intralinha | SELECTIVE_SPANS | PR #827; prefixo/sufixo por boundary de escrita. CRAM dot e latch físico da paleta não modelados |
| Escritas R7 intralinha | SELECTIVE_SPANS | PR #827; commit anterior à alteração do backdrop |
| Escritas VRAM intralinha | SELECTIVE_SPANS | PR #828; commit antes da escrita. Fetch-ahead de tiles/bitplanes/sprites não modelado |
| Registradores R1-R7 intralinha | SELECTIVE_SPANS_APPROXIMATE | PR #829 integrada; boundary de escrita é modelo coarse, não latch físico medido por registrador |
| R0 e transição Mode 4/legacy | SELECTIVE_SPANS_APPROXIMATE | PR #831 integrada; commit antes de R0 e renderer legacy/TMS por spans. Boundary coarse, não timestamp físico de latch |
| R8 horizontal scroll | IMPLEMENTED_APPROXIMATE | Latch em HPos 21; fase absoluta ainda sem validação física |
| R9 vertical scroll | IMPLEMENTED_APPROXIMATE | Valor pendente aplicado na fronteira de scanline conforme modelo atual |
| R10 / HINT counter | IMPLEMENTED_TSTATE_QUANTIZED | Decremento/reload no evento HPos 21, entregue no boundary T14; fase dot/fracionária não modelada |
| SAT / avaliação de sprites | IMPLEMENTED_SCANLINE | PR #832/#833: Mode 4 e legacy/TMS capturam metadados X/tile/pattern/color e controles R0/R6 durante avaliação por scanline; bytes de padrão continuam lidos na composição; timestamp físico não medido |
| Sprite overflow | IMPLEMENTED_TSTATE_QUANTIZED | Mode 4 e legacy/TMS: índice associado latched em HPos 24 / T16, usando o buffer do modo ativo |
| Sprite collision | IMPLEMENTED_PIXEL_COMMIT_APPROXIMATE | Sobreposição de pixels opacos publicada quando um commit de span cruza o pixel; leitura de status comita o prefixo até o feixe; não há pipeline de colisão por dot |
| HCounter / porta $7F | IMPLEMENTED_APPROXIMATE | Conversão 342 HPos / 228 T-states; latch por transição TH via software; leituras espelhadas preservam o valor capturado. Eventos TH externos atualizam $DD, mas ainda não capturam HCounter; sem validação física |
| Entradas externas TH1/TH2 | IMPLEMENTED_SCHEDULED_INPUT | Transições timestamped no Scheduler; níveis active-low independentes em $DD, precedência de saída $3F, snapshot/restore e trace. Phase 1 é desempate determinístico, não fase elétrica medida |
| HBlank | IMPLEMENTED_TSTATE_QUANTIZED | T213/HPos319 fecha os 256 pixels ativos; T212/HPos318 não fecha o último pixel. Teste de colisão no pixel 255 valida o boundary lógico, não a fase elétrica física |
| VBlank / IRQ | IMPLEMENTED_TSTATE_QUANTIZED | VINT IRQ pendente em HPos 20/T13; VBlank flag e frame-ready em HPos 24/T16 da primeira linha VBlank; acknowledge pelo status read |
| Scheduler / determinismo | IMPLEMENTED_INTEGRATED | VDP e entradas TH externas avançam pelo Scheduler; eventos TH têm timestamp absoluto, ordem de empate explícita, snapshot/restore e trace |
| Save State | IMPLEMENTED_WITH_VERSIONING | PR #841: coordinator v5, bus v20 e machine v33 preservam linhas TH externas e eventos pendentes; rejeição de versões anteriores |
| SMS2 / outros perfis | REGRESSION_GATED | Renderer e perfis não SMS1 preservados pelos testes direcionados; não generalizar spans SMS1 |

## 2. Política de granularidade

- Scanline permanece a unidade predominante de processamento.
- Spans são commitados somente em boundaries de escrita com efeito visual observável e implementado.
- A janela ativa usa HPos 63 como início e HPos 319 como fim exclusivo, no modelo comparativo 228 T-states / 342 HPos.
- O Scheduler continua sendo a autoridade temporal; o VDP não possui relógio independente.
- Não declarar dot-accuracy, fetch timing ou latch físico sem evidência independente.
- MAME é referência comparativa, mas seu código 315-5124 declara mid-scanline changes como trabalho pendente; não é oráculo para a semântica CRAM/R7/VRAM.

## 3. Limitações que bloqueiam teste in-game

1. Fechar a temporalidade física de R0-R9 e latches específicos; o evento HINT/R10 está modelado em T14, mas ainda precisa de validação independente com a ROM e o barramento CPU.
2. Validar o comportamento de CRAM dot e o instante de latch da paleta.
3. Modelar ou justificar por evidência o fetch-ahead de tiles, bitplanes e padrões de sprites.
4. Modelar fetches de padrão por sprite/dot e separar seleção SAT de fetch real; Mode 4 e legacy/TMS já capturam metadados por scanline, mas bitplanes continuam consultados durante a composição.
5. Executar a ROM SmsVdpTest e validar os eventos HPos 20/T13 (VINT IRQ), HPos 21/T14 (HINT), HPos 23/T15 (VCounter) e HPos 24/T16 (VBlank flag/overflow) contra a implementação; os eventos seguem T-state boundaries, sem fase dot-clock.
6. Confirmar o instante de colisão pixel a pixel: o modelo publica a colisão no commit do pixel, mas não possui pipeline dot-level do ASIC.
7. Expandir a demonstração de equivalência contínua/em blocos/save-load para eventos subline, interrupções e transições de frame. A captura TH por software já possui teste de invariância para avanço contínuo versus blocos 3+5 T-states.
8. Executar regressões SMS1/SMS2 e build Windows x64 Release no candidato final.
9. Fechar a captura H-counter provocada por TH externo/Light Phaser. A linha externa já é agendada e lida em $DD, mas o latch físico e o atraso permanecem pendentes: SMS Power descreve latch quando TH está ativo-low; MAME arma em TH low e agenda captura no retorno a high com offset configurável. Não assumir o offset MAME de 19 HPos como universal. A integração host/SDL3 também permanece pendente.
10. Registrar os comportamentos ainda aproximados antes da primeira campanha in-game.

## 4. Evidências integradas

- PR #825 — DAC SMS1: níveis RGB e regressões de framebuffer.
- PR #826 — colisão de sprites: pixels opacos, limpeza por leitura de status e máscaras/deslocamentos.
- PR #827 — spans CRAM/R7: 79 testes `sega-video`, 14 testes SMS de integração, runtime Release e build Windows x64 Release aprovados no HEAD validado.
- PR #828 — spans VRAM: 81 testes `sega-video`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, runtime Release e build Windows x64 Release aprovados no HEAD validado.
- PR #829 — spans R1-R7: 83 testes `sega-video`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, runtime Release e build Windows x64 Release aprovados no HEAD validado.
- PR #830 — R0 em Mode 4: 84 testes `sega-video`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, runtime Release e build Windows x64 Release aprovados no HEAD validado.
- PR #831 — spans Mode 4/legacy e R0: 91 testes `sega-video`, 149 testes `sega-sms-core`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, validação Z80/DD-FD, runtime Release (45 + 1 testes) e build Windows x64 Release aprovados no HEAD `19c34080757f84aec5d7fb24a5dd3355badf5d41`.
- PR #832 — metadados SAT Mode 4: 92 testes `sega-video`, 131 testes `sega-sms-core`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, validação Z80/DD-FD, runtime Release e build Windows x64 Release aprovados no HEAD `1e0a1191f7bdc37c57520a3cc6b78e142206b1c6`.
- PR #833 — buffer SAT legacy/TMS: 93 testes `sega-video`, 131 testes `sega-sms-core`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, validação Z80/DD-FD, runtime Release e build Windows x64 Release aprovados no HEAD `ab37d5ae69a7e754fb46a0b7d98883ccf8089719`.
- PR #834 — boundaries HBlank/HINT/VBlank/overflow (primeira aproximação): 95 testes `sega-video`, 131 testes `sega-sms-core`, 14 testes SMS de integração, Rustfmt, Clippy, documentação, validação Z80/DD-FD, runtime Release e build Windows x64 Release aprovados no HEAD `c5e59b6a40e2158546c1d0a5bd857add34b42d08`.
- PR #836 — correção temporal baseada na SmsVdpTest: evidência histórica do HEAD `6449b7085c019a95d2cd4f0ca1c364a0bf929962`.
- PR #837 — atualização do scorecard temporal.
- PR #838 — leitura estável do H-counter por `$7F` e portas espelhadas; o primeiro workflow classificou incorretamente o caminho temporal e não executou os testes do SMS core.
- PR #839 — corrige a classificação de impacto temporal; adiciona invariância de captura TH para avanço contínuo versus blocos e boundary HBlank T212/T213 com colisão no último pixel. No HEAD `c7afd54c22d68dc3a7e9d94e26bbd780bd1564f0`: 98 testes `sega-video`, 131 testes `sega-sms-core`, 14 testes de integração SMS; Rustfmt, Clippy, Z80 focused, input focused, Scheduler stress, workspace regression, runtime Release (45 + 1 testes) e build Windows x64 Release aprovados.
- PR #841 — integra transições externas TH1/TH2 no Scheduler, níveis active-low em $DD, snapshot/restore, desempate phase 0/phase 1 e registro no trace. No HEAD `82742d3c1b5fef3a45d0185529d5b9330f0d4180`: 98 testes `sega-video`, 131 testes `sega-sms-core`, 14 testes de integração SMS; input/console, Z80, Scheduler stress, workspace regression, Clippy, Rustfmt, runtime Release (45 + 1 testes) e build Windows x64 Release aprovados.

Referência de timing independente: [SmsVdpTest](https://github.com/FluBBaOfWard/SmsVdpTest). A ROM ainda não foi executada pelo runtime deste projeto.

Os resultados acima pertencem aos respectivos commits e workflows; não são automaticamente atribuídos a commits futuros.

## 5. Critério de liberação

**SMS1 VDP permanece NÃO LIBERADO PARA TESTE IN-GAME.**

A aprovação dos spans CRAM/R7/VRAM e Mode 4/legacy comprova apenas esses incrementos. A liberação exige fechar os bloqueios da seção 3, executar os testes de integração ampliados e validar o candidato final na `main`.
