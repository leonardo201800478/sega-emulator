# Inventário de migração da aplicação para Rust — M0.2

- **Baseline auditada:** `main` em `b82f748b83749eac2173ff86e535cc2a9e2ca862` (2026-11-06).
- **ADR normativa:** [ADR 0009 — Aplicação integralmente Rust com egui/eframe](../adr/0009-full-rust-application-egui-eframe.md).
- **Escopo:** código Python ativo, contratos de aplicação, persistência, assets, testes, fronteiras Rust e dependências.
- **Estado:** auditoria estática de primeiro passe; nenhum código de runtime foi removido ou alterado.

## 1. Resumo executivo

A árvore contém **40 módulos Python** em `src/sega_emulator/`. O entrypoint `__main__.py` inicia, por padrão, a janela SMS `MinimalSmsWindow`; essa janela usa PySide6 para apresentação e serviços Python para carregamento, preferências e biblioteca, enquanto o worker temporal, máquina, áudio e vídeo são Rust através de PyO3.

O frontend SG-1000 antigo não é atualmente um caminho operacional: `qt_frontend.run()` falha explicitamente conforme a decisão de vídeo nativo. A antiga `main_window.py` e diversos diálogos continuam presentes e testados, mas não são o entrypoint SMS atual. O inventário distingue código presente, código chamado no caminho padrão e código dormente; presença no pacote não prova uso em produção.

A aplicação final deverá executar como binário Rust sem interpretador Python, PySide6, NumPy, PyO3 ou Maturin. A árvore `legacy/` permanece histórica e inerte.

## 2. Grafo de execução observado

```text
src/sega_emulator/__main__.py
  ├── diagnostics.configure_session_logging
  ├── rom_loader.list_rom_members              (--list-roms)
  └── SMS (default)
      └── frontend/qt/minimal_sms_window.py     [PySide6]
          ├── frontend/qt/preferences.py        [QSettings adapter]
          ├── frontend/qt/rom_library_dialog.py  [ROM browser]
          │   ├── rom_library.py                [JSON index / scan / SHA-256]
          │   ├── rom_catalog.py                [No-Intro DAT]
          │   └── screenscraper.py              [optional HTTP/XML metadata]
          ├── frontend/qt/sms_settings_dialog.py
          ├── machine/sms.py                    [SMS ROM loader]
          └── machine/sms_temporal.py            [thin facade]
              └── sega_z80_core.NativeSMSTemporalMachine (PyO3)
                  └── SmsTemporalWorker
                      ├── sega_sms_core::SmsTemporalMachine
                      ├── sega_audio_runtime::NativeAudioSession
                      ├── native/audio_output (WASAPI/XAudio2)
                      └── sega_video_runtime::D3d12MailboxSession

SG-1000 selection
  └── frontend/qt_frontend.py                  [currently disabled / fails explicitly]
      └── machine/sg1000.py                    [thin PyO3 facade]

Controller configuration
  ├── frontend/controller_profiles.py          [JSON/profile validation]
  └── frontend/rust_controller_input.py        [PyO3 NativeInputManager facade]

Other application services
  ├── rom_loader.py / sms_rom_audit.py
  ├── machine/cartridge_profiles.py
  ├── machine/sms_firmware.py / sms_region.py
  ├── audio/output_config.py
  ├── audio/vgm_recorder.py
  └── diagnostics.py
```

### 2.1 Fronteira crítica a remover

`native/z80_core/src/sms_temporal_worker.rs` contém o worker de sessão, a construção da máquina SMS temporal, comandos de ciclo de vida, configuração de áudio, mailbox e integração D3D12. Ele está dentro do crate `native/z80_core`, cujo `lib.rs` é uma extensão PyO3.

A migração não deve duplicar esse worker nem reimplementar a máquina. A fatia de runtime deverá extrair/refatorar o worker para uma biblioteca Rust comum, independente de PyO3, e deixar o módulo PyO3 como adaptador transitório que consome a mesma biblioteca. A aplicação egui e o adaptador legado devem chamar a mesma implementação durante a transição. O ownership exclusivo da máquina e a thread dedicada permanecem no runtime Rust.

O crate `native/sms_core` já exporta `SmsTemporalMachine`, snapshots e operações temporais, mas não substitui sozinho o worker de aplicação/host. O crate `native/video_runtime` já oferece `D3d12MailboxSession::start_for_existing_window`; a janela externa deve permanecer viva até o encerramento da sessão.

## 3. Inventário de módulos Python ativos

| Área | Módulos | Responsabilidade observada | Destino Rust |
|---|---|---|---|
| Entry point | `__main__.py` | CLI, versão, logs, seleção SMS/SG-1000 e listagem de ZIP | `sega-app` |
| Preferências | `frontend/qt/preferences.py` | Defaults, validação, leitura/escrita QSettings, histórico | `sega-application` |
| Janela SMS | `frontend/qt/minimal_sms_window.py` | Abrir ROM, Play/Pause/Reset/Stop, áudio, viewport, telemetria | `sega-ui-egui` + `sega-application` |
| Biblioteca | `rom_library.py`, `frontend/qt/rom_library_dialog.py` | Varredura incremental, cache, hashes, duplicados, pesquisa, capas e seleção | `sega-media` + `sega-ui-egui` |
| Catálogo | `rom_catalog.py` | Leitura lazy de DAT, identificação SHA-1/MD5/CRC32 e metadados de disco | `sega-media` |
| Carregamento | `rom_loader.py`, `machine/sms.py` | Leitura e validação SG-1000/SMS, ZIP, limites, SHA-256 | `sega-media` |
| Auditoria ROM | `sms_rom_audit.py`, `frontend/qt/sms_rom_audit_dialog.py` | Relatórios de ROM SMS, CRC/hash e progresso/cancelamento | `sega-media` + UI |
| Firmware | `machine/sms_firmware.py`, `machine/sms_region.py` | Identificação de BIOS SMS, região e política de boot | `sega-media` + `sega-application` |
| Perfil cartucho | `machine/cartridge_profiles.py` | Perfil SG-1000 por hash/CRC e tamanho de RAM | `sega-media` / integração com core |
| Fachadas de máquina | `machine/sg1000.py`, `machine/sms.py`, `machine/sms_temporal.py` | Validação e chamadas coarse-grained via PyO3 | APIs Rust diretas; adaptador PyO3 removido no gate final |
| Input | `frontend/controller_profiles.py`, `frontend/rust_controller_input.py`, `frontend/qt/sms_input_settings.py` | Perfis, mapeamentos, captura e opções de Light Phaser | `sega-input-runtime` + `sega-application` |
| Áudio | `audio/output_config.py`, `audio/vgm_recorder.py` | Modelo de configuração e escrita VGM | `sega-audio-runtime` / `sega-media` |
| Diagnóstico | `diagnostics.py`, `frontend/qt/debug_logging.py` | Logging, JSON, arquivos e formatação de métricas | `sega-diagnostics` |
| Apresentação Qt | `frontend/qt/drivers_dialog.py`, `focus_pause.py`, `frame_pacer.py`, `localization.py`, `main_window.py`, `native_video_viewport.py`, `scraper_settings_dialog.py`, `screenscraper.py`, `settings_dialog.py`, `sms_settings_dialog.py`, `video_filters.py`, `video_widget.py`, `qt_frontend.py` | UI, localização, configurações, viewport, pacing e metadata | `sega-ui-egui` + serviços nativos; cada recurso deve ser classificado por uso efetivo antes da remoção |
| Perfis/ponte | `frontend/controller_profiles.py`, `frontend/rust_controller_input.py` | Persistência e ponte para input Rust | serviço Rust; remover PyO3 |

### 3.1 Módulos dormentes ou com uso parcial

- `frontend/qt/main_window.py`: janela antiga com callbacks e menus; permanece no pacote e possui testes, mas não é criada pelo caminho SMS padrão.
- `frontend/qt_frontend.py`: caminho SG-1000 que falha explicitamente no estado atual; não declarar SG-1000 frontend-validado por sua mera presença.
- `frontend/qt/video_widget.py` e `video_filters.py`: módulos pequenos/legados; validar consumidores e contratos antes de migrar ou arquivar.
- `audio/output_config.py`: modelo dataclass de configuração serializável; não confundir com o worker SMS ativo que atualmente recebe parâmetros de backend diretamente.
- `sms_input_settings.py`: grava opções de Light Phaser; o próprio código afirma que o runtime SMS ainda não as consome. Migrar o estado como preferência dormente, sem anunciar suporte funcional.
- `machine/sms.py`: contém fachada de máquina não temporal e loader; o caminho SMS padrão atual usa seu loader e a máquina temporal separadamente.

## 4. Contratos de mídia e catálogo

### 4.1 ROM loader

- SG-1000: `rom_loader.py` aceita `.sg`, `.rom`, `.bin`, ZIP com seleção explícita de membro quando houver ambiguidade, não extrai o arquivo e limita a ROM a 48 KiB neste estágio.
- SMS: `machine/sms.py::load_sms_rom` aceita `.sms`, `.rom`, `.bin` ou ZIP com exatamente um candidato e limite de 4 MiB.
- O entrypoint `--list-roms` usa o loader SG-1000 para enumerar membros de ZIP.
- A biblioteca é um indexador de candidatos, não um validador: inclui extensões de sistemas futuros e mídia de disco. Não transformar indexação em afirmação de compatibilidade de runtime.
- Preservar caminhos, membro ZIP, nome, tamanho, SHA-256 e mensagens de erro. Nenhuma extração automática insegura.

### 4.2 Catálogo DAT e mídia

`rom_catalog.py` carrega DATs XML sob demanda e identifica ROM por tamanho + SHA-1, depois MD5 e CRC32. Também classifica metadados de mídia `.iso`, `.cue`, `.chd` e referências CUE sem extrair ou alterar arquivos.

O repositório contém DATs de SG-1000/SC-3000, Master System/Mark III, Game Gear, Mega Drive/Genesis, Mega CD/Sega CD e 32X. A presença de DAT ou de um sistema em `RomLibrary.SUPPORTED_SYSTEMS` não significa que o emulador dessa plataforma esteja implementado.

### 4.3 Firmware SMS

- BIOS SMS reconhecida tem exatamente 8 KiB.
- Identificação usa SHA-256 confiável e/ou correspondência exata de hash com metadados DAT.
- Bytes de firmware não devem ser registrados em logs nem incluídos em relatórios.
- Região é resolvida conservadoramente por header de cartucho e DAT; não escolher BIOS incompatível por heurística.
- BIOS é opcional e nunca deve ser embutida/versionada.

## 5. Persistência: migração sem perda

### 5.1 QSettings

A aplicação usa organização/aplicativo `SegaEmulator/SegaEmulator`. O backend Rust deverá importar os valores existentes uma única vez, validar tipos/faixas, persistir versão de schema e manter cópia/backup do arquivo legado até a migração ser verificada.

Chaves observadas:

| Grupo | Chaves |
|---|---|
| Janela e UI | `window_width`, `window_height`, `video_scale`, `ui_language` |
| Histórico | `recent_roms` |
| Input/perfis | `controller_config`, `sms_light_phaser_enabled`, `sms_light_phaser_mouse_aim`, `sms_light_phaser_mouse_trigger`, `sms_light_phaser_right_stick_aim`, `sms_light_phaser_invert_y`, `sms_light_phaser_crosshair`, `sms_light_phaser_sensitivity` |
| Firmware/sistema | `firmware_directory`, `sms_bios_boot_enabled`, `sms_region_preference` |
| Áudio | `audio_enabled`, `audio_sample_rate`, `audio_buffer_size`, `audio_volume`, `audio_balance`, `audio_queue_blocks`, `sms_audio_backend` |
| Vídeo | `video_filter`, `integer_scale`, `sprite_limit_enabled`, `vsync_mode`, `vrr_mode`, `av_sync_mode`, `crt_scanline_intensity`, `crt_scanline_thickness`, `crt_phosphor_mask`, `crt_curvature`, `crt_signal_mode` |
| Biblioteca/capas | `rom_directories/{system}`, `box_art_directories/{system}`, `box_art_overrides/{system}/{sha256(path.casefold())}` |
| Metadata | `metadata_provider`, `metadata_language`, `auto_scrape_missing` |
| Região/UI | `sms_region_preference`, `ui_language` |

Observações de compatibilidade:

1. `cpu_core` é forçado para `rust`; `diagnostics_enabled` é persistido como `false` no adaptador atual.
2. Credenciais de ScreenScraper não são persistidas nem enviadas pelo fluxo atual. Uma implementação futura deve usar o cofre de credenciais do SO, não texto simples no JSON/QSettings.
3. As opções Light Phaser são persistidas, mas não consumidas pelo runtime SMS atual.
4. O roadmap normativo exige histórico das **15 ROMs recentes**, enquanto `FrontendPreferences.MAX_RECENT_ROMS` limita o baseline atual a **12**. A migração deverá importar todas as entradas existentes e elevar o limite para 15, sem descartar as 12 atuais.
5. O menu de região normativo prevê Auto, Brasil, USA, Europa, Japão, Coreia e UNL; o caminho temporal atual valida apenas `usa`, `brazil`, `europe`, `japan`. A UI Rust não deve expor opções ainda sem política/runtime comprovados.

### 5.2 Índice persistente da biblioteca

- Caminho atual: `QStandardPaths.AppLocalDataLocation/library-index.json`; fallback `~/.sega-emulator/library-index.json`.
- Formato atual: JSON, `version = 1`, objeto `systems` com entradas por sistema.
- Campos por entrada: `system`, `path`, `name`, `size`, `mtime_ns`, `sha256`, `suffix`.
- Scan incremental reutiliza hash somente quando tamanho e mtime em nanossegundos coincidem; full scan recalcula hash.
- Cancelamento não apaga entradas ainda não visitadas; roots ausentes/inacessíveis protegem entradas anteriores.
- Persistência usa arquivo temporário e `os.replace`; Rust deve manter escrita atômica e recuperação de índice corrompido.
- Sistemas atualmente indexáveis: SG-1000, Master System, Game Gear, Mega Drive, Sega CD e 32X.

## 6. Recursos distribuídos

- Assets UI: `background.svg`, `splash_hero.svg`, ícones SVG por sistema, PNGs selecionados, `manifest.json`, `contact_sheet.png` e `README.txt` de assets.
- Dados: seis arquivos DAT versionados em `dats/`.
- O empacotamento final deve copiar os recursos necessários para o diretório de dados do produto e testar execução fora da raiz do repositório.
- Auditar licenças e notices de assets antes de empacotar; não adicionar ROMs comerciais ou BIOS proprietárias.

## 7. Dependências e build

### 7.1 Python

`pyproject.toml` declara setuptools 84.2.1; Python `>=3.15.2,<3.16`; extras `qt` com PySide6 6.12.0 e NumPy 2.5.4; extras `dev` com Maturin 1.16.0, pytest 9.2.0 e Ruff 0.17.3; uv 0.13.7.

- PySide6 é dependência operacional da UI ativa.
- Maturin/PyO3 constroem a ponte transitória.
- pytest/Ruff são tooling de desenvolvimento Python atual.
- A busca estática inicial não encontrou import de NumPy nos 40 módulos de produção; há referências no manifesto e em testes/documentação. Confirmar todos os consumidores antes de retirar a dependência.
- `tools/project_manager.py`, `tools/sms_boot_matrix.py`, `tools/validate_native_audio.py`, `tools/z80test/run_sms_z80test.py` e `tests/tooling/test_project_manager.py` continuam tooling Python fora do frontend. Para a meta de repositório 100% Rust, cada ferramenta deve ser portada para Rust ou formalmente classificada como utilitário externo não distribuído; não a confundir com runtime de produto.

### 7.2 Rust

- Toolchain fixada em Rust 1.99.1, edition 2024/resolver 3.
- Workspace raiz inclui SMS, SG-1000, Z80, PSG, áudio, input, scheduler, temporal core e vídeo.
- `native/z80_core` (PyO3) e `native/audio_output` estão excluídos do workspace raiz e possuem lockfiles próprios.
- `native/video_runtime` contém D3D12 e contrato `VideoFrame`/mailbox.
- `native/sms_core` exporta a máquina temporal, mas o worker atual está no crate PyO3.
- Consolidar crates/lockfiles somente após tornar a fronteira Rust comum e validar as features Windows.

## 8. Cobertura de testes existente

Os testes Python estão organizados em famílias, não constituem prova de fidelidade do hardware:

- UI Qt: `test_qt_main_window`, `test_qt_minimal_sms`, dialogs de ROM, settings, drivers, input, scraper e audit.
- ROM/media: `test_rom_catalog`, `test_rom_library`, `test_sms_rom_loader`, `test_sms_rom_audit`, `test_sms_firmware`, `test_sms_region_policy`.
- Runtime/ponte: `test_sms_temporal_machine`, `test_sms_native_audio`, `test_sg1000_native_machine`, `test_rust_runtime_selection`, `test_rust_controller_input`, `test_native_input_runtime`.
- Host: `test_frame_pacer`, `test_qt_frame_pacer`, `test_video_filters`, `test_vulkan_video_widget`, `test_focus_pause`, `test_emulation_pause`.
- Áudio/diagnóstico: `test_vgm_recorder`, `test_vgm_machine_integration`, `test_audio_output_config`, `test_audio_status`, `test_debug_logging`, `test_diagnostics_formatters`, `test_rom_loader_diagnostics`.
- Tooling: `tests/tooling/test_project_manager.py`.

Cada família deverá receber equivalente Rust ou evidência de que o contrato é coberto por testes Rust já existentes. Testes Qt não podem ser simplesmente apagados: os casos de uso e asserções devem ser transferidos para testes de controller/serviço e testes de integração da UI egui.

## 9. Pendências e critérios para fechar M0.2

- [x] Inventariar os 40 módulos Python de produção e suas famílias de imports.
- [x] Mapear o caminho padrão SMS e a fronteira PyO3.
- [x] Identificar a localização do worker temporal, áudio e D3D12.
- [x] Enumerar grupos de persistência e índice da biblioteca.
- [x] Identificar assets, DATs, tooling e famílias de testes.
- [x] Registrar incompatibilidades entre baseline e contrato normativo (12 vs 15 ROMs; regiões; Light Phaser).
- [ ] Confirmar por busca completa os consumidores residuais de NumPy, PySide6 e PyO3 em tools/workflows.
- [ ] Executar inventário de todos os imports/recursos gerados com ferramentas no checkout Windows e validar caminhos de empacotamento.
- [ ] Fechar API de aplicação/worker Rust comum sem PyO3.
- [ ] Provar viewport D3D12 filho hospedado no shell eframe.

**Gate M0.2:** o inventário está suficientemente completo para iniciar o shell eframe sem tocar no hardware; os itens não concluídos acima bloqueiam remoção de dependências, port integral do runtime ou declaração de migração concluída.

## 10. Fontes do baseline

Todos os caminhos abaixo foram lidos na `main` do SHA registrado no cabeçalho:

- `src/sega_emulator/__main__.py`
- `src/sega_emulator/frontend/qt/minimal_sms_window.py`
- `src/sega_emulator/frontend/qt/preferences.py`
- `src/sega_emulator/frontend/qt/rom_library_dialog.py`
- `src/sega_emulator/rom_library.py`
- `src/sega_emulator/rom_catalog.py`
- `src/sega_emulator/rom_loader.py`
- `src/sega_emulator/machine/sms.py`
- `src/sega_emulator/machine/sms_temporal.py`
- `src/sega_emulator/machine/sms_firmware.py`
- `src/sega_emulator/frontend/qt/sms_input_settings.py`
- `native/z80_core/src/lib.rs`
- `native/z80_core/src/sms_temporal_worker.rs`
- `native/sms_core/src/lib.rs`
- `native/sms_core/src/temporal_machine.rs`
- `native/video_runtime/src/d3d12/mailbox_session.rs`
- `native/audio_output/src/lib.rs`
- `pyproject.toml`, `Cargo.toml`, `rust-toolchain.toml`
