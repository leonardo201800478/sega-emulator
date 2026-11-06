# Auditoria e plano de retirada dos testes Python — 2026-11-06

- **Baseline:** `main` em `edbec9272efe87a6a408bac74283d86a91d87b23`.
- **Branch desta auditoria:** `architecture/rust-only-test-migration-20261106`.
- **Escopo:** arquivos ativos sob `tests/`; testes sob `legacy/` não fazem parte da suíte ativa.
- **Resultado do inventário estático:** 42 arquivos Python e 272 funções `test_*` declaradas.
- **Execução local fornecida pelo usuário:** pytest coletou 352 casos, 351 PASS e 1 FAIL. O log não informa o SHA local; portanto, não é evidência de execução na baseline acima.
- **Estado:** inventário inicial concluído; equivalência individual com Rust ainda NÃO AUDITADA. Nenhum teste foi removido nesta etapa.

## 1. Decisão

O estado final do projeto não terá testes Python ativos. Todo comportamento essencial atualmente coberto por testes Python deverá ser validado por testes Rust no crate responsável pelo comportamento. Testes de interface serão migrados para a camada Rust da aplicação (egui/eframe), não para crates de hardware.

Não basta apagar os arquivos ou transferir asserts mecanicamente. A migração deve preservar o contrato testado, os limites, os casos negativos, os efeitos colaterais e as condições de erro. Teste sem equivalente comprovado não pode ser classificado como migrado.

A partir desta decisão:
- Não adicionar novos testes Python.
- Não ampliar a suíte pytest para cobrir funcionalidade nova.
- Não remover ainda arquivos que sejam a única especificação executável de comportamento ativo.
- Cada grupo só poderá ser removido após os critérios da seção 4.
- A remoção final de pytest, configuração e jobs de CI ocorre depois da migração da aplicação e da classificação de todos os arquivos.

## 2. Evidência e limites da contagem

A árvore GitHub da baseline contém 42 arquivos `.py` em `tests/`. A contagem de 272 é o número de funções declaradas com nome `test_*`; parametrização e fixtures fazem a coleta do pytest chegar a 352 casos na execução local fornecida. Esses números medem unidades diferentes.

O log local registra 351 aprovados e uma falha em `test_audio_output_failure_does_not_prevent_loading_rom`. A falha compara o texto esperado “Áudio WASAPI indisponível” com a mensagem atual “Áudio XAudio2 indisponível: WASAPI endpoint unavailable”. O comportamento central indicado pelo nome do teste — falha de configuração de áudio não impedir o carregamento da ROM — foi observado como verdadeiro no próprio log. A divergência é de expectativa textual; não será corrigida no frontend Qt, que está em retirada.

A presença de testes Rust nos arquivos listados abaixo comprova apenas que há código de teste no repositório. Sem execução no SHA desta auditoria, não constitui PASS nem demonstra equivalência integral com qualquer teste Python.

## 3. Inventário por arquivo

A coluna “destino” indica o crate/camada Rust onde o contrato deverá ser validado. “Pendente” significa que a equivalência ainda precisa ser confrontada caso a caso; não implica ausência total de testes Rust no domínio.

| Arquivo Python | Funções | Contrato principal | Destino / situação |
|---|---:|---|---|
| `tests/test_controller_profiles.py` | 5 | Perfis, JSON, schema, ações e ordem | Input/aplicação Rust — PENDENTE |
| `tests/test_emulation_pause.py` | 5 | Pausa manual, retomada e foco | Aplicação/UI Rust — PENDENTE |
| `tests/test_focus_pause.py` | 3 | Política de pausa por perda/retorno de foco | Aplicação/UI Rust — PENDENTE |
| `tests/test_frame_pacer.py` | 4 | Cadência, atraso e reset do pacing | Host/aplicação Rust — PENDENTE |
| `tests/test_native_input_runtime.py` | 1 | Contrato público do input nativo | `native/input_runtime` — PENDENTE |
| `tests/test_qt_box_art.py` | 8 | Descoberta, associação e preferência de artwork | Mídia/biblioteca Rust + UI — PENDENTE |
| `tests/test_qt_drivers_dialog.py` | 2 | Seleção e defaults de drivers | Aplicação/UI Rust — PENDENTE |
| `tests/test_qt_frame_pacer.py` | 4 | Cadência visual sem catch-up burst | Host/aplicação Rust — PENDENTE |
| `tests/test_qt_key_mapping.py` | 2 | Conversão de teclas para nomes portáveis | Input Rust — PENDENTE |
| `tests/test_qt_main_window.py` | 12 | Menus, atalhos, região, áudio/vídeo, VGM, fullscreen e localização | `native/sega_app` — PENDENTE |
| `tests/test_qt_minimal_sms.py` | 15 | Sessão SMS, ROM, áudio, viewport, telemetria e shutdown | Aplicação Rust + integração SMS/hosts — PENDENTE |
| `tests/test_qt_preferences.py` | 13 | Defaults, validação, persistência e migração de preferências | Aplicação Rust — PENDENTE |
| `tests/test_qt_rom_library_dialog.py` | 9 | Biblioteca, pesquisa, grid e abertura de ROM | Mídia/aplicação/UI Rust — PENDENTE |
| `tests/test_qt_scraper_settings.py` | 5 | Localização e preferências de metadata | Aplicação Rust ou remoção de recurso — PENDENTE |
| `tests/test_qt_settings_dialog.py` | 4 | Configuração de áudio/vídeo e scan de firmware | Aplicação/mídia Rust — PENDENTE |
| `tests/test_qt_sms_input_settings.py` | 3 | Preferências de controle e Light Phaser | Input/aplicação Rust; distinguir preferência de hardware implementado — PENDENTE |
| `tests/test_qt_sms_rom_audit_dialog.py` | 1 | Controles da auditoria de ROM | Mídia/aplicação Rust — PENDENTE |
| `tests/test_qt_sms_settings_dialog.py` | 2 | Validação de áudio/vídeo/região | Aplicação Rust — PENDENTE |
| `tests/test_qt_video_driver_selection.py` | 2 | Bloqueio do renderer Qt legado e ausência de cópia de pixels | UI/host Rust — PENDENTE |
| `tests/test_rom_catalog.py` | 9 | DAT, hashes, identificação e formatos de disco | Mídia Rust — PENDENTE |
| `tests/test_rom_library.py` | 8 | Scan incremental, cache, duplicatas e recuperação | Mídia Rust — PENDENTE |
| `tests/test_rust_controller_input.py` | 5 | Adapter PyO3, perfis e dispositivos | Input Rust direto — PENDENTE |
| `tests/test_rust_runtime_selection.py` | 4 | Seleção e diagnóstico do runtime nativo | Aplicação Rust — PENDENTE |
| `tests/test_sg1000_native_machine.py` | 5 | Integração nativa SG-1000, joystick e PSG | `native/sg1000_core` — PENDENTE |
| `tests/test_smoke.py` | 4 | Entry point, versão e bloqueio do frontend legado | `native/sega_app`/CLI Rust — PENDENTE |
| `tests/test_sms_firmware.py` | 39 | Scanner, hashes, DAT, região e política de boot BIOS | Mídia/aplicação Rust — PENDENTE |
| `tests/test_sms_native_audio.py` | 9 | Perfil de clock, PSG, NMI e adapter SMS | SMS/áudio Rust — PENDENTE |
| `tests/test_sms_region_policy.py` | 17 | Resolução de região por preferência, header e DAT | Mídia/aplicação Rust — PENDENTE |
| `tests/test_sms_rom_audit.py` | 7 | Limites de leitura, ZIP e conclusões de auditoria | Mídia Rust — PENDENTE |
| `tests/test_sms_rom_loader.py` | 6 | Limites, ZIP e validação de ROM SMS | Mídia Rust — PENDENTE |
| `tests/test_sms_temporal_machine.py` | 6 | Fachada coarse-grained, sessão e telemetria | API de aplicação/SMS Rust — PENDENTE |
| `tests/test_vgm_machine_integration.py` | 1 | Escritas PSG com timestamp | Áudio/mídia Rust — PENDENTE |
| `tests/test_vgm_recorder.py` | 4 | Formato VGM, waits, ordenação e persistência | Áudio/mídia Rust — PENDENTE |
| `tests/test_video_filters.py` | 1 | Rejeição do filtro de pixels Python legado | UI/host Rust — PENDENTE |
| `tests/tooling/test_project_manager.py` | 10 | CLI de auditoria, locks, status e execução de processos | Ferramenta Rust ou retirada do tooling Python — PENDENTE |
| `tests/unit/test_audio_output_config.py` | 14 | Configuração, validação e round-trip de áudio | `native/audio_runtime`/aplicação — PENDENTE |
| `tests/unit/test_audio_status.py` | 3 | Estado de áudio habilitado/desabilitado/falha | Áudio/aplicação Rust — PENDENTE |
| `tests/unit/test_cartridge_profiles.py` | 7 | CRC/SHA e perfis de cartucho | Mídia/SMS/SG-1000 Rust — PENDENTE |
| `tests/unit/test_debug_logging.py` | 3 | Logging assíncrono, capacidade e default | Diagnóstico Rust — PENDENTE |
| `tests/unit/test_diagnostics_formatters.py` | 2 | Identidade de backend/áudio em logs | Diagnóstico Rust — PENDENTE |
| `tests/unit/test_rom_loader_diagnostics.py` | 7 | Loader, ZIP, hash e logs JSONL | Mídia/diagnóstico Rust — PENDENTE |
| `tests/unit/test_vulkan_video_widget.py` | 1 | Bloqueio de widget Qt legado | UI/host Rust — PENDENTE |

**Total:** 42 arquivos; 272 funções `test_*` declaradas.

## 4. Critérios de migração e remoção

Um arquivo Python só pode ser removido quando:

1. Cada função de teste tiver uma decisão rastreável: migrada, substituída por teste de UI Rust ou obsoleta com justificativa.
2. Cada comportamento essencial tiver teste Rust no crate responsável, sem chamar Python ou PyO3.
3. Casos positivos, negativos, limites, serialização, erros e efeitos colaterais relevantes forem preservados.
4. Testes de integração entre crates cobrirem as fronteiras que antes eram verificadas pelo adapter Python.
5. Os gates Rust forem executados no SHA candidato:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`
   - `cargo test --workspace --locked`
   - workspaces independentes (`native/audio_output`, `native/z80_core` enquanto existir e `native/sega_app`) e targets Windows aplicáveis.
6. Testes físicos/host que não podem ser executados em CI forem classificados NOT RUN, nunca PASS.

A remoção dos arquivos deve ocorrer por domínio e em PRs pequenas. Não apagar os 42 arquivos em lote.

## 5. Sequência de execução

1. **Inventário (esta etapa):** registrar os 42 arquivos e seus contratos, sem remoção.
2. **Núcleos e integrações:** conferir testes Rust já existentes; fechar lacunas de CPU, SMS, SG-1000, vídeo, áudio, Scheduler e input. Os testes Python de adapter devem migrar para chamadas diretas às APIs Rust.
3. **Serviços de aplicação:** migrar ROM/DAT/ZIP, firmware, região, perfis de cartucho, biblioteca, VGM, preferências, logging e diagnóstico para crates Rust, preservando vetores e fixtures legais.
4. **Interface:** implementar cada fluxo no `native/sega_app`; substituir testes Qt por testes egui headless e testes Windows de integração para comportamento dependente de HWND, dispositivo ou sistema operacional.
5. **Retirada:** após equivalência, remover os arquivos Python por domínio, diretório `tests/`, configuração pytest e dependência `pytest`.
6. **CI final:** retirar dispatch/job/filters de pytest, Qt e PyO3. Remover Ruff e o restante do empacotamento Python somente quando não houver código Python ativo que o projeto decida manter. A meta final da aplicação Rust elimina também PyO3/Maturin e dependências de execução Python.

## 6. Estado atual

- Inventário de arquivos: **PASS** (consulta estática da árvore GitHub na baseline indicada).
- Contagem de funções declaradas: **PASS** (extração estática dos 42 arquivos).
- Equivalência individual Python ↔ Rust: **NOT RUN / NOT AUDITED**.
- Execução de testes Rust nesta etapa: **NOT RUN**.
- Testes físicos Windows/GPU/áudio: **NOT RUN**.
- Remoção de arquivos Python: **NOT STARTED**.

Não declarar cobertura migrada com base apenas na existência de `#[test]` em um crate ou em CI de SHA diferente.

## 7. Referências

- [ADR 0009 — aplicação integralmente Rust](../adr/0009-full-rust-application-egui-eframe.md)
- [Inventário de migração da aplicação](rust-application-migration-inventory.md)
- [Status da migração da aplicação](rust-application-migration-status-2026-11-06.md)
- [CI validation policy](../ci-validation-policy.md)
