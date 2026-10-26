# Qt frontend: implementação e relatório de validação da PR #98

**Data da auditoria:** 2026-10-26  
**Repositório:** `leonardo201800478/sega-emulator`  
**PR:** [#98 — redesign do frontend e configurações avançadas de áudio/vídeo](https://github.com/leonardo201800478/sega-emulator/pull/98)  
**Branch:** `feat/qt-modern-settings`  
**Commit validado:** `a21c9abf8e38424ecee4ebd41e8fd364c36b0302`  
**Base:** `main`, commit `008b85914fe8d0c5d77a170892e4cc2ac472a957`

## 1. Objetivo e limites

Modernizar a interface Qt 6/PySide6 sem criar uma máquina de emulação paralela. O frontend continua integrando `SG1000Machine` e os backends Python/Rust existentes. Esta PR modifica apresentação, saída de áudio e preferências; não declara precisão de ciclo do Z80 nem fidelidade analógica do PSG.

A mudança anterior [PR #97](https://github.com/leonardo201800478/sega-emulator/pull/97) removeu o adaptador de diagnóstico isolado e adotou o runtime SG-1000 integrado como caminho padrão, mantendo a opção de backend existente e o fallback Python. O commit de migração de preferências antigas é `008b85914fe8d0c5d77a170892e4cc2ac472a957`.

## 2. Código desta etapa

| Arquivo | Responsabilidade |
|---|---|
| `.github/workflows/qt-frontend-validation.yml` | Formatação, lint, testes focados de Qt/áudio e regressão do frontend. |
| `src/sega_emulator/frontend/qt/main_window.py` | Janela redesenhada, menus, barra de ações, tela cheia e entrada para configurações A/V. |
| `src/sega_emulator/frontend/qt/settings_dialog.py` | Diálogo de preferências de áudio e vídeo com restauração de padrões. |
| `src/sega_emulator/frontend/qt/preferences.py` | Defaults, validação defensiva e persistência em `QSettings`. |
| `src/sega_emulator/frontend/qt/audio_output.py` | Conversão/re-amostragem de amostras mono do PSG para PCM de saída. |
| `src/sega_emulator/frontend/qt/video_widget.py` | Apresentação do framebuffer, proporção 4:3, escala inteira e filtro selecionável. |
| `src/sega_emulator/frontend/qt_frontend.py` | Integração das preferências com runtime, mixer, volume, vídeo e persistência. |
| `tests/test_qt_preferences.py` | Defaults, valores inválidos e persistência das preferências. |
| `tests/test_qt_audio_output.py` | PCM mono int16, limites, amostras vazias e taxas de saída inválidas. |
| `tests/test_qt_settings_dialog.py` | Leitura/edição do diálogo e restauração dos padrões. |
| `tests/test_qt_main_window.py` | Menus, escalas e ação de configurações. |
| `README.md` | Estado e instruções do frontend atualizados. |

### Configurações disponíveis

- **Áudio:** saída ativada/desativada; volume de 0–100%; taxa do mixer 22.050, 44.100 ou 48.000 Hz; buffer de 256, 512, 1.024 ou 2.048 amostras.
- **Vídeo:** filtro nearest-neighbor ou bilinear; preferência por escala inteira; proporção 4:3.
- **Preferências:** valores são persistidos; valores fora do conjunto suportado são normalizados para padrões seguros.
- **Compatibilidade:** `_initialize_pygame_backend()` mantém argumento opcional para preservar chamadas legadas e testes de integração.
- **Relógios:** re-amostragem e filtros alteram somente saída/apresentação, não os clocks lógicos da máquina.

## 3. Testes executados no GitHub Actions

Todos os resultados abaixo pertencem ao commit listado no cabeçalho, salvo indicação em contrário.

| Execução | Verificação | Resultado observado |
|---|---|---|
| [Qt Frontend Rebuild Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799900) | Ruff format + Ruff check e testes específicos Qt/áudio | **28 passed** nos quatro módulos focados; workflow concluído com sucesso. |
| Mesmo workflow Qt | Testes de controles | **25 passed**. |
| Mesmo workflow Qt | Suíte completa, com PySide6 instalado | **677 passed, 1.664 skipped**, em 4,32 s. |
| [CI](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799736) | Qualidade e regressão | Concluído com sucesso. |
| [Agente 1 — Functional Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799769) | Suíte geral em Python 3.14.8 | **646 passed, 1.667 skipped**, em 7,25 s. |
| [Agente 2 — Integrity and Determinism](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799745) | Repetibilidade, imports e integridade do pacote | **646 passed, 1.667 skipped**, em 7,09 s; workflow concluído com sucesso. |
| [Agente 3 — Performance and Coverage Audit](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799771) | Regressão cronometrada e inventário de opcodes Z80 | Concluído com sucesso; a suíte reportou **646 passed, 1.667 skipped**, em 8,12 s. |
| [UI and Controller Input Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799844) | Lint, formatação, compilação Python e testes de controle em Windows | **43 passed**; workflow concluído com sucesso. |
| [Performance Diagnostic](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799770) | Perfil de runtime com ROM de referência `super-uwol.sg` | Baseline sem instrumentação: **72,744 FPS**; quadro instrumentado mediano/P95: **20,875/25,238 ms**. É diagnóstico do núcleo Python, não benchmark da nova GUI. |
| [Z80 SingleStepTests Differential Audit](https://github.com/leonardo201800478/sega-emulator/actions/runs/40258799762) | Auditoria diferencial do Z80 | Workflow concluído com sucesso. |

### O que os resultados demonstram

1. O código Python alterado passa por formatação e lint nos workflows relevantes.
2. O diálogo, as preferências e a conversão PCM passam nos testes unitários focados.
3. A janela e a entrada de controles foram exercitadas em CI; os testes de controles passaram no runner Windows.
4. A suíte geral passou no workflow Qt com PySide6 instalado.
5. Os três agentes terminaram com sucesso no commit validado.

### Limitações dos testes

- **1.664 testes ignorados** na suíte completa do workflow Qt, majoritariamente porque o módulo compilado `sega_z80_core` não estava instalado naquele job. A suíte geral dos três agentes também ignorou testes de Qt e Rust por ausência dessas dependências no ambiente específico dos agentes.
- O sucesso do workflow Qt não é um teste de escuta humana nem valida áudio em hardware físico: o CI usa backend de áudio dummy.
- Não foi realizado teste manual interativo em Windows nesta execução.
- O benchmark de desempenho é da máquina de referência em Python e não mede input lag, VSync ou desempenho gráfico da janela Qt.
- Não foi alegada precisão ciclo a ciclo do Z80 nem fidelidade analógica do PSG.

## 4. Correções feitas durante a validação

- Preservado o contrato legado de `_initialize_pygame_backend()` ao tornar as preferências opcionais, sem impedir que o frontend passe a configuração explicitamente.
- Normalizada a formatação de arquivos e testes afetados para que Ruff passe.
- Atualizado o workflow Qt para incluir os novos módulos de teste de áudio e diálogo.
- Corrigido o tratamento de Esc para sair da tela cheia sem encerrar a sessão do emulador.

As execuções intermediárias tiveram falhas durante a evolução da branch; os resultados finais acima são das execuções posteriores ao ajuste. As execuções canceladas/falhas de commits intermediários não são contabilizadas como sucesso final.

## 5. Próximas validações recomendadas

Antes de integrar a PR, ainda é útil:
1. confirmar a conclusão dos workflows pendentes específicos do runtime Rust;
2. executar um teste manual curto no Windows com uma ROM homebrew/legalmente obtida;
3. verificar som em dispositivo real com 44,1/48 kHz e buffers 256/512/1.024, observando estalos e latência;
4. comparar captura do framebuffer antes/depois de trocar filtros e escala, confirmando que apenas a apresentação muda.

## 6. Estado da publicação

Na geração deste relatório, a PR #98 permanece aberta e não integrada à `main`. Este documento registra os resultados observados; não declara a PR pronta para merge automático.
