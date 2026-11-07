# Estado do projeto — 2026-11-07

**Repositório:** https://github.com/leonardo201800478/sega-emulator  
**Branch de referência:** `main`  
**SHA auditado:** `3ad1781f84b7824b5aa889a583ae6d4545987ec1`  
**Critério:** somente código integrado na `main` é tratado como estado atual do produto. PRs abertas são descritas separadamente.

## 1. Resumo executivo

O produto permanece Rust-only, com aplicação desktop egui/eframe e SMS temporal como primeiro runtime. O shell nativo está integrado, mas a integração vertical completa entre GUI, runtime SMS, vídeo, áudio e input ainda está distribuída em PRs abertas e não deve ser declarada concluída.

O PR #615 integrou uma melhoria de fidelidade do SN76489 (filtro de saída e reset do ruído). O PR #617 integrou esclarecimentos arquiteturais/documentais. A main consultada é `3ad1781f84b7824b5aa889a583ae6d4545987ec1`.

O teste longo de estresse do MachineScheduler foi executado com sucesso no Windows local, em uma branch de integração. Os arquivos de teste e implementação do Scheduler usados nessa execução foram comparados por blob SHA com a main e são idênticos. O resultado é evidência válida para esses mesmos fontes na main, mas não comprova a integração temporal sistêmica do SMS.

## 2. Arquitetura vigente

- Produto integralmente em Rust.
- GUI desktop em egui/eframe; apresentação e comandos de alto nível apenas.
- SMS temporal como primeiro runtime ativo.
- Z80 compartilhado, sem cópia por console.
- MachineScheduler como autoridade de ordenação temporal.
- Áudio, vídeo e input em runtimes/hosts Rust.
- Comunicação UI/runtime por comandos coarse-grained; sem callbacks por instrução.
- Sem Python, PySide6/Qt, PyO3 ou Maturin no produto.
- Sem JIT, frame skipping, hacks temporais ou otimizações especulativas.
- A precisão temporal é proporcional ao comportamento observável, conforme ADR-0016; não declarar precisão universal de pinos/ciclos sem evidência.

## 3. PRs relevantes consultadas

| PR | Estado observado | Conteúdo e consequência |
|---|---|---|
| [#615](https://github.com/leonardo201800478/sega-emulator/pull/615) | MERGED | Melhoria de filtragem de saída SN76489 e fidelidade do reset do ruído. Merge commit `3ca0f648c841bfe7c85a301fb561273e5c62f676`, ancestral da main auditada. |
| [#617](https://github.com/leonardo201800478/sega-emulator/pull/617) | MERGED | Clarificação da arquitetura Rust/egui-only. Main auditada em `3ad1781f84b7824b5aa889a583ae6d4545987ec1`. |
| [#613](https://github.com/leonardo201800478/sega-emulator/pull/613) | OPEN | Integração do runtime SMS temporal, carregamento de ROM e viewport D3D12 no shell egui. Não contar como integrada até merge. |
| [#614](https://github.com/leonardo201800478/sega-emulator/pull/614) | OPEN | Áudio WASAPI/XAudio2 e controles teclado/SDL3 sobre a #613. Base é a branch da #613; PR empilhada. Testes físicos de áudio/gamepad ainda NOT RUN segundo a descrição da PR. |
| [#616](https://github.com/leonardo201800478/sega-emulator/pull/616) | OPEN | Reconciliação documental da auditoria de boot/cartuchos SMS. Não é código integrado na main. |

Os estados acima são a fotografia da consulta de 2026-11-07. Reconsultar GitHub antes de qualquer integração ou publicação posterior.

## 4. Evidência — stress longo do MachineScheduler

**Registro detalhado:** [Scheduler long stress — 2026-11-07](evidence/scheduler-stress-long-2026-11-07.md).

- Ambiente: Windows local, PowerShell.
- Branch executada: `feat/sega-app-audio-keyboard-controls-20261106`.
- SHA executado: `f118a4d3a2547b95c679c4133b94e22d2b3a03be`.
- Perfil: `SCHEDULER_STRESS_PROFILE=long`.
- Resultado: 2 testes PASS, 0 FAIL, 1 teste filtrado.
- Carga: 320.000 operações e 4.000.000 dispatches, em 16 seeds.
- Duração reportada: 405,68 segundos.

Comparação de conteúdo no GitHub:
- `native/machine_scheduler/tests/stress_replay.rs`: blob `228b379745054ea8a8b184cda95c33191781d237` em main e no SHA executado.
- `native/machine_scheduler/Cargo.toml`: blob `33f853d9fa64f7719d94d2d90462858557c3b59d` em ambos.
- `native/machine_scheduler/src/lib.rs`: blob `ba3f16500081007b05f44d9ee83997aad03a7ef4` em ambos.

Portanto, o resultado é transferível para os mesmos fontes do Scheduler na main. Ele não é uma execução do workspace inteiro na main, nem substitui gates de CI, testes de host ou validação física.

### Histórico de execução no Codespaces

- Perfil curto: PASS, 8.000 operações e 80.000 dispatches.
- Perfil longo: execução interrompida manualmente após exceder o tempo esperado sob quota de CPU. Classificação BLOCKED/INTERRUPTED; não é FAIL e não é PASS.
- O Codespace foi parado para liberar recursos, sem ser excluído. A campanha longa foi concluída posteriormente no Windows local.

## 5. Estado das validações

| Gate / suíte | Status | Observação |
|---|---|---|
| Scheduler stress curto no Codespaces | PASS | 4 seeds; 8k operações e 80k dispatches |
| Scheduler stress longo no Windows local | PASS | 16 seeds; 320k operações e 4M dispatches |
| Stress longo no Codespaces | BLOCKED | Interrompido; não usar como resultado final |
| `cargo fmt --all -- --check` | NOT RUN nesta campanha local longa | Não foi parte do comando informado |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | NOT RUN nesta campanha local longa | Não foi parte do comando informado |
| `cargo test --workspace --locked` | NOT RUN nesta campanha local longa | A campanha executou apenas o alvo de stress |
| ZEXDOC / ZEXALL | NOT RUN | E8 continua bloqueada até fechamento E7 e harness Rust completo |
| WASAPI/XAudio2 físico | NOT RUN | Exige endpoint físico e evidência separada |
| Gamepad físico / hotplug | NOT RUN | Exige dispositivos reais |
| GPU / viewport físico | NOT RUN | Exige validação Windows/GPU |
| Gameplay SMS com ROM homebrew | NOT RUN nesta evidência | Scheduler stress não executa jogo nem valida o sistema completo |

## 6. Estado do Z80

A execução do stress do Scheduler não altera o scorecard do Z80.

- E4C, E5 e E6 permanecem conforme evidências integradas registradas no scorecard.
- E7.7 permanece IN_PROGRESS até fechamento semântico auditável.
- E8 permanece BLOCKED; não iniciar ZEXDOC/ZEXALL antes do fechamento E7.
- A matriz DD/FD e o scorecard continuam sendo as referências obrigatórias para a próxima tarefa de Z80.

## 7. Próximas prioridades

1. Revisar e validar as PRs #613 e #614 como uma integração empilhada, sem assumir que estão na main.
2. Executar os gates obrigatórios no HEAD final de cada PR e registrar resultados por SHA.
3. Realizar testes físicos de vídeo, áudio e input no Windows.
4. Validar a fatia SMS com ROM sintética/homebrew e confirmar lifecycle/encerramento.
5. Retomar a auditoria semântica Z80 E7 quando a integração prioritária do SMS estiver estabilizada; somente então liberar E8.
6. Manter o scorecard, matriz DD/FD, roadmap e ledger de evidências sincronizados com o código integrado.

## 8. Limitações desta fotografia

Esta atualização consolida informações do GitHub e a saída de terminal fornecida pelo usuário. A execução local não foi realizada pelo CI nem observada diretamente pelo executor; os resultados são registrados como evidência reportada pelo usuário, com SHA e blobs verificados no GitHub. Não há declaração de merge, validação física ou conclusão funcional do Master System.
