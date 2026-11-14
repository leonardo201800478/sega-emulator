# SMS1 VDP — scorecard de conclusão

**Sistema-alvo:** Sega Master System 1
**ASIC prioritário:** Sega 315-5124
**Backend:** Rust / `sega-video`
**Baseline integrada:** `main` após PR #833 (`a56ab308e3e6db2570216974bf50d54c3ad9c037`)
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
| R10 / HINT counter | IMPLEMENTED_SCANLINE | Contador e flag funcional; instante de HBlank ainda coarse |
| SAT / avaliação de sprites | IMPLEMENTED_SCANLINE | PR #832/#833: Mode 4 e legacy/TMS capturam metadados X/tile/pattern/color e controles R0/R6 durante avaliação por scanline; bytes de padrão continuam lidos na composição; timestamp físico não medido |
| Sprite overflow | IMPLEMENTED_APPROXIMATE | Mode 4 e legacy/TMS: índice associado e latch durante avaliação da linha; evento físico HPos ainda não temporizado |
| Sprite collision | IMPLEMENTED_APPROXIMATE | Sobreposição de pixels opacos; commit no fechamento da scanline, não no dot/HBlank físico |
| HBlank / contador horizontal | NOT_CLOSED | HPos modelado parcialmente; todos os eventos e limites físicos não validados |
| VBlank / IRQ | IMPLEMENTED_SCANLINE | Flags e integração funcional; instante sublinha e arbitragem completa ainda pendentes |
| Scheduler / determinismo | IMPLEMENTED_INTEGRATED | VDP avança pelo Scheduler; snapshots incluem o estado de rasterização parcial |
| Save State | IMPLEMENTED_WITH_VERSIONING | PR #833 atualiza snapshots temporais (bus v17, machine v30); teste de rejeição do formato anterior e clone/restore dos buffers Mode 4/legacy |
| SMS2 / outros perfis | REGRESSION_GATED | Renderer e perfis não SMS1 preservados pelos testes direcionados; não generalizar spans SMS1 |

## 2. Política de granularidade

- Scanline permanece a unidade predominante de processamento.
- Spans são commitados somente em boundaries de escrita com efeito visual observável e implementado.
- A janela ativa usa HPos 63 como início e HPos 319 como fim exclusivo, no modelo comparativo 228 T-states / 342 HPos.
- O Scheduler continua sendo a autoridade temporal; o VDP não possui relógio independente.
- Não declarar dot-accuracy, fetch timing ou latch físico sem evidência independente.
- MAME é referência comparativa, mas seu código 315-5124 declara mid-scanline changes como trabalho pendente; não é oráculo para a semântica CRAM/R7/VRAM.

## 3. Limitações que bloqueiam teste in-game

1. Fechar a temporalidade física dos registradores R0-R10 e os latches específicos; a composição parcial Mode 4/legacy está integrada, mas seus boundaries continuam aproximados.
2. Validar o comportamento de CRAM dot e o instante de latch da paleta.
3. Modelar ou justificar por evidência o fetch-ahead de tiles, bitplanes e padrões de sprites.
4. Modelar fetches de padrão por sprite/dot e separar seleção SAT de fetch real; Mode 4 e legacy/TMS já capturam metadados por scanline, mas bitplanes continuam consultados durante a composição.
5. Fechar o instante de HBlank, VBlank, HINT e a ordem dos flags em relação ao Scheduler/Z80.
6. Demonstrar equivalência determinística entre execução contínua, execução em blocos e save/load state.
7. Executar regressões SMS1/SMS2 e build Windows x64 Release no candidato final.
8. Registrar os comportamentos ainda aproximados antes da primeira campanha in-game.

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

Os resultados acima pertencem aos respectivos commits e workflows; não são automaticamente atribuídos a commits futuros.

## 5. Critério de liberação

**SMS1 VDP permanece NÃO LIBERADO PARA TESTE IN-GAME.**

A aprovação dos spans CRAM/R7/VRAM e Mode 4/legacy comprova apenas esses incrementos. A liberação exige fechar os bloqueios da seção 3, executar os testes de integração ampliados e validar o candidato final na `main`.
