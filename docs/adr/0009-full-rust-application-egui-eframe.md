# ADR 0009 — Aplicação integralmente Rust com egui/eframe

- **Status:** ACCEPTED — decisão de destino; migração incremental e ainda não concluída.
- **Data:** 2026-11-06.
- **Baseline:** `main` em `b82f748b83749eac2173ff86e535cc2a9e2ca862`.
- **Branch inicial:** `migration/rust-egui-application-phase0-20261106`.
- **Escopo:** substituir a aplicação ativa Python/PySide6 e a fronteira PyO3 por uma aplicação nativa Rust, mantendo os núcleos de hardware existentes como única implementação.

## 1. Contexto e decisão

A aplicação atual não é apenas uma biblioteca de jogos Python. A árvore ativa `src/sega_emulator/` contém entrypoint, frontend Qt, serviços de ROM/catalogação, firmware/região, perfis de cartucho e controles, configuração de áudio, gravação VGM e diagnóstico. O runtime de hardware já está majoritariamente em Rust, mas a aplicação ainda depende de Python, PySide6 e da extensão PyO3/Maturin.

**Decisão:** adotar `egui` com `eframe` como framework oficial da interface desktop e migrar a camada de aplicação para Rust. A aplicação distribuída não exigirá interpretador Python, PySide6, NumPy, PyO3 ou Maturin.

A migração é incremental. A interface Python existente permanece como baseline de comportamento e rollback até que cada fluxo tenha substituto Rust testado. Não remover, renomear em massa ou arquivar módulos ativos antes de concluir a matriz de consumidores, contratos e testes.

## 2. Relação com decisões anteriores

Esta ADR substitui somente as decisões de tecnologia de apresentação e linguagem da camada de aplicação que conflitem com o destino Rust:

- ADR 0008 continua válida quanto à propriedade Rust de CPU, barramentos, memória, mappers, I/O, VDP, PSG, scheduler, interrupções e estado de hardware.
- A restrição da ADR 0008 que mantém a aplicação Qt/Python e PyO3 como fronteira operacional deixa de ser a arquitetura-alvo.
- A ADR de reconstrução temporal SMS de 2026-11-05 continua válida quanto ao escopo funcional mínimo, comandos de sessão, ownership, execução fora da thread visual, determinismo, preservação SG-1000 e ausência de fallback silencioso. Referências a Qt/PySide6/PyO3 deverão ser reinterpretadas como egui/eframe e APIs Rust diretas.
- ADRs de hardware, scheduler, vídeo, áudio e input permanecem vigentes quando não contradizem esta decisão.

Não usar esta mudança como justificativa para reimplementar CPU, VDP, PSG, áudio ou scheduler dentro da camada de interface.

## 3. Auditoria do baseline

### 3.1 Código Python ativo identificado

| Área | Caminhos observados | Destino |
|---|---|---|
| Entrypoint e UI | `src/sega_emulator/__main__.py`, `frontend/qt/minimal_sms_window.py`, `frontend/qt_frontend.py`, `frontend/qt/**` | `sega-app` + `sega-ui-egui` |
| Catálogo e biblioteca | `rom_catalog.py`, `rom_library.py` | `sega-media` / `sega-application` |
| Carregamento e auditoria | `rom_loader.py`, `sms_rom_audit.py` | `sega-media` |
| Firmware e perfil de máquina | `machine/sms_firmware.py`, `sms_region.py`, `cartridge_profiles.py` | `sega-media` / `sega-application` |
| Fachadas de runtime | `machine/sg1000.py`, `sms.py`, `sms_temporal.py` | chamadas Rust diretas; eliminar adaptadores PyO3 após paridade |
| Input e perfis | `frontend/controller_profiles.py`, `rust_controller_input.py` | `sega-input-runtime` + serviço de perfis |
| Áudio da aplicação | `audio/output_config.py`, `audio/vgm_recorder.py` | `sega-audio-runtime` / `sega-application` |
| Diagnóstico | `diagnostics.py`, logging e painéis Qt | `sega-diagnostics` / UI |
| Testes e ferramentas | `tests/test_qt_*.py`, testes Python de serviços e ferramentas Python | migrar para testes Rust ou arquivar com justificativa; não manter runtime Python de produção |

O caminho de UI ativo no baseline foi refinado nesta auditoria: `__main__.py` inicia por padrão `MinimalSmsWindow` (PySide6), que contém viewport nativo D3D12, barra/menu, abertura de ROM, biblioteca, configurações, captura de controles e telemetria. A janela importa `TemporalSMSMachine` e a fachada ainda carrega `NativeSMSTemporalMachine` via PyO3. O caminho SG-1000 chama `frontend/qt_frontend.py`, cujo `run()` está deliberadamente desativado e falha explicitamente conforme ADR-0012; portanto, não classificar a antiga janela SG-1000 como frontend operacional. Os demais módulos Qt e testes permanecem no repositório, mas sua participação no fluxo de produção precisa ser classificada individualmente no grafo de imports.\n\nO código de hardware histórico em `legacy/python_reference/` é arquivo histórico e não deve ser reativado, empacotado ou usado como fallback.

### 3.2 Empacotamento e dependências Python

O `pyproject.toml` atual usa setuptools 84.2.1 e Python `>=3.15.2,<3.16`. Os grupos declarados incluem:

- `PySide6==6.12.0` e `numpy==2.5.4` para a aplicação Qt;
- `maturin==1.16.0` para construir a extensão;
- `pytest==9.2.0` e `ruff==0.17.3` para testes/lint;
- `uv==0.13.7` como versão de ferramenta.

Esses pacotes não serão removidos antes da migração dos consumidores correspondentes. O empacotamento final será Cargo + artefatos nativos/recursos versionados, com build Release reproduzível no Windows.

### 3.3 Fronteira Rust atual

No baseline:

- O workspace raiz usa resolver 3 e edition 2024; toolchain fixada em Rust 1.99.1.
- `native/z80_core` está excluído do workspace raiz e é a extensão PyO3 (`pyo3 = 0.30.1`), com lockfile próprio.
- `native/audio_output` também está excluído do workspace raiz e possui lockfile próprio.
- O adaptador `sega-z80-core` liga PyO3 a SG-1000, SMS, áudio, input e vídeo.
- `native/audio_output` já implementa saída WASAPI/XAudio2 em Rust; declara `wasapi = 0.26`, `windows = 0.62.2` e `rtrb = 0.3` (lockfile fixa a versão efetiva).
- `native/video_runtime` contém integração Windows/D3D12 via `windows = 0.62.2`.
- `native/audio_runtime`, `native/input_runtime`, `native/video_core`, `native/video_runtime`, `native/temporal_core` e `native/machine_scheduler` são componentes Rust já existentes e devem ser reutilizados, não duplicados.

A exclusão dos crates e seus lockfiles separados é um bloqueio arquitetural para a consolidação futura. Não mover `audio_output` ou `z80_core` para o workspace sem resolver dependências, features por plataforma, lockfile e CI de forma deliberada. `z80_core` deverá deixar de ser uma extensão Python e tornar-se uma biblioteca Rust comum quando todos os consumidores PyO3 forem migrados.

## 4. Dependências de interface escolhidas

Versão de referência auditada em 2026-11-06:

- `egui = 0.37.0`
- `eframe = 0.37.0`

`eframe` é o framework oficial de aplicação egui e oferece integração nativa com winit e renderer wgpu; também possui opção Glow. `egui` requer Rust 1.95.0 ou posterior, compatível com a toolchain do repositório (1.99.1). O backend wgpu tem suporte DX12 no Windows.

Fontes consultadas:
- https://docs.rs/crate/egui/0.37.0
- https://docs.rs/crate/eframe/0.37.0
- https://docs.rs/wgpu-hal/29.0.0
- https://docs.rs/wgpu/latest/wgpu/struct.Backends.html

**Política de dependências:** fixar versões exatas na primeira integração e atualizar somente com justificativa, changelog auditado, lockfile revisado e testes. Não adicionar bibliotecas de UI concorrentes.

**Risco conhecido:** egui está em desenvolvimento ativo e releases podem conter breaking changes; a aparência padrão não é nativamente igual à do Windows. A camada de apresentação deve encapsular egui para reduzir acoplamento e permitir atualização controlada. Acessibilidade, navegação por teclado, IME, localização, fontes e escala DPI serão critérios explícitos de validação, não presumidos.

## 5. Arquitetura de destino

```text
sega-app (binário Rust)
  |
  +-- sega-ui-egui (eframe / egui)
  |     |-- biblioteca e navegação
  |     |-- viewport de vídeo
  |     |-- menus, atalhos e diálogos
  |     |-- configuração, controles e diagnóstico
  |
  +-- sega-application (casos de uso / controller)
  |     |-- comandos tipados e snapshots
  |     |-- ciclo de vida de sessão
  |     |-- ROM, firmware e biblioteca
  |     |-- preferências e migrações
  |     |-- perfis, histórico e VGM
  |
  +-- host services (Rust)
  |     |-- sega-video-runtime / renderer
  |     |-- sega-audio-output (WASAPI / XAudio2)
  |     |-- sega-input-runtime / APIs nativas
  |     |-- filesystem, logging e diagnóstico
  |
  +-- máquinas e componentes compartilhados
        |-- sega-sms-core / sega-sg1000-core
        |-- sega-z80-cpu / bus / ALU
        |-- sega-video / SN76489 / audio runtime
        |-- MachineScheduler / temporal core
```

Crates-alvo sugeridos (a criação será incremental e somente quando houver primeiro consumidor):

- `sega-application`: contratos, comandos, eventos, estados de sessão e casos de uso sem dependência de egui.
- `sega-media`: leitura/validação de ROM, ZIP, DAT, hashes, firmware, auditoria e biblioteca.
- `sega-ui-egui`: apresentação egui, sem regra de negócio nem acesso direto aos chips.
- `sega-app`: composição, configuração de host e entrypoint executável.
- `sega-diagnostics`: agregação de métricas e logs, se os módulos atuais não cobrirem o contrato.

Não criar crates vazios antecipadamente. O desenho final deve respeitar os crates compartilhados já existentes e evitar duplicidade por console.

### 5.1 Contrato de aplicação

A UI emite comandos tipados (ex.: `OpenMedia`, `Start`, `Pause`, `Stop`, `Reset`, `SelectSystem`, `UpdatePreferences`). Um controller/serviço valida pré-condições e encaminha comandos ao runtime Rust. A UI recebe snapshots/eventos imutáveis.

A thread egui não pode bloquear em I/O, varredura, hashing, scraping, carregamento de mídia ou execução da máquina. Use canais/buffers limitados e workers Rust dedicados para tarefas de aplicação. A UI consulta resultados sem espera bloqueante. Não criar uma thread por instrução nem paralelizar o scheduler de hardware.

### 5.2 Propriedade temporal e vídeo

O `MachineScheduler` permanece a única autoridade dos clocks/eventos emulados. O loop de repaint do eframe, VSync, timer do host e taxa do monitor não avançam nem corrigem clocks do console.

A integração de vídeo deve preservar a decisão já aceita na ADR-0012: o host de vídeo e o caminho de pixels permanecem Rust/D3D12, com HWND, swap chain, recursos, fences e apresentação sob ownership nativo. Esta ADR muda a tecnologia da interface de controle; não revoga a escolha do backend de vídeo.

`eframe` usa seu próprio renderer de UI (por padrão wgpu, com opção Glow). Isso não autoriza transportar o framebuffer para uma textura wgpu nem substituir o renderer D3D12 do emulador. A hipótese inicial a provar é uma janela eframe como shell de aplicação e a superfície D3D12 como viewport nativo filho, controlado por Rust e posicionado a partir do layout da UI. O protótipo deve confirmar se a API pública do eframe/winit permite obter o HWND pai de forma suportada, criar/gerir a janela filha e sincronizar resize/DPI/foco/teclado sem depender de APIs privadas.

Se a integração em janela única não for suportável sem hacks, bloquear a migração do viewport e avaliar uma integração customizada do egui/event loop com o host D3D12. Não aceitar duas janelas independentes como arquitetura final sem decisão explícita. Não criar um segundo caminho de apresentação de vídeo.

O protótipo deve verificar DX12, adapter, ownership do HWND, foco/captura de input, resize, DPI, minimize/restore, VSync, recuperação de device, shutdown e custo de UI. Nenhuma opção poderá alterar VDP, clocks, raster, pixels ou Scheduler. A decisão será fechada por prova funcional e medição, não apenas por compilação.

### 5.3 Áudio e input

Áudio continua integralmente em Rust: worker, mixer, resampler, fila, WASAPI/XAudio2, hotplug e métricas. A UI apenas configura e consulta snapshots; PCM não atravessa a camada de apresentação.

Input físico e periféricos lógicos permanecem separados. O host coleta teclado/gamepad/HID e converte para snapshots/eventos de input do runtime; a UI não injeta callbacks por instrução. Preservar hotplug, perfis e mapeamentos validados.

## 6. Plano incremental e gates

### M0 — Auditoria e decisão (esta entrega)
- [x] Auditar SHA remoto, manifestos principais e separação dos crates.
- [x] Identificar famílias de módulos Python ativos.
- [x] Identificar dependências de build/runtime e pontos de acoplamento.
- [x] Adotar egui/eframe e registrar riscos do renderer.
- [ ] Completar matriz de imports, consumidores, dados persistidos, recursos e cobertura por módulo.

**Gate:** arquitetura aprovada e baseline registrada; nenhuma remoção de código.

### M1 — Aplicação Rust mínima
- Criar binário `sega-app` e camada de aplicação independente da UI.
- Abrir janela eframe com navegação mínima, estado da sessão e painel de diagnóstico.
- Provar integração com o HWND do host D3D12 (parent/child, DPI, resize, foco e shutdown), sem mover pixels para o renderer wgpu.
- Adicionar smoke tests headless do controller e teste de inicialização/encerramento da janela no Windows.
- Não carregar ROM nem iniciar hardware até o contrato de sessão estar pronto.

**Gate:** `cargo run --release` abre/fecha de forma limpa no Windows; UI não bloqueia; nenhum Python/PyO3 é necessário para esse binário.

### M2 — Integração vertical SG-1000/SMS
- Integrar máquina Rust por API Rust direta, sem PyO3.
- Migrar ciclo de vida coarse-grained, input, mailbox e apresentação D3D12 dentro da janela eframe, mantendo o renderer de vídeo Rust já aceito.
- Preservar SG-1000 e SMS; comparar resultados com a baseline atual e fixtures determinísticas.
- Provar execução contínua versus execução em blocos e encerramento sem deadlock.

**Gate:** smoke de boot, frame, input, pause/resume, reset e stop por sistema; sem regressão temporal.

### M3 — Serviços de mídia e preferências
- Migrar loader, ZIP, DAT/catalog, hashing, auditoria, firmware e região.
- Migrar biblioteca, índice, duplicados, cancelamento e progresso.
- Migrar preferências com versão de schema e importador dos valores QSettings existentes; não descartar chaves sem relatório.
- Migrar perfis de controles, histórico, VGM recorder, diagnóstico e logging.

**Gate:** testes diferenciais sobre fixtures não protegidas; dados e configurações legadas preservados; nenhuma operação longa na thread UI.

### M4 — Paridade funcional do frontend
- Recriar viewport, biblioteca, menus, atalhos, diálogos, controles, opções de vídeo/áudio e diagnóstico.
- Cobrir localização, fontes, DPI, acessibilidade e navegação por teclado.
- Verificar performance de UI e upload de vídeo separadamente do custo de hardware.

**Gate:** matriz funcional equivalente aprovada; screenshot/UX isolados não bastam.

### M5 — Remoção da ponte Python e consolidação
- Migrar consumidores restantes de `sega-z80-core`/PyO3 para Rust direto.
- Integrar crates excluídos ao workspace ou justificar formalmente a separação que restar.
- Consolidar lockfiles e dependências por plataforma.
- Remover PyO3, Maturin, setuptools, PySide6, NumPy e packaging Python do produto.
- Migrar ou arquivar ferramentas e testes Python ativos com substitutos Rust ou justificativa registrada.
- Manter `legacy/python_reference/` apenas como histórico inerte.

**Gate:** build/execução Release sem interpretador Python instalado; nenhum import/runtime PyO3; CI principal Rust; inventário de dependências sem pacotes Python necessários ao produto.

### M6 — Validação final e distribuição
- Build Release reproduzível em Windows x86_64.
- Testes Rust completos e integração de host.
- Validação manual de ROM homebrew/legal em SG-1000 e SMS, vídeo, áudio e controles.
- Testes de save state/replay/determinismo quando os caminhos forem afetados.
- Auditoria de licenças, assets, recursos e instalador.
- PRs pequenos; merge somente com autorização explícita e CI verde no SHA exato.

## 7. Critérios finais de aceitação

1. Aplicação e serviços de produto implementados em Rust.
2. Interface nativa egui/eframe; nenhum frontend Qt/PySide6 ativo.
3. Nenhuma dependência operacional de Python, PyO3 ou Maturin.
4. Binário Release inicializa sem Python instalado.
5. Hardware e timing continuam pertencendo aos crates Rust compartilhados.
6. Scheduler determinístico e equivalência contínua/blocos preservados.
7. SG-1000 não sofre regressão; SMS continua evoluindo sobre os módulos compartilhados.
8. Vídeo, áudio e input host integrados sem callbacks Python ou cópias PCM para a UI.
9. ROM, firmware, preferências, perfis e biblioteca preservados/migrados.
10. Testes e workflows classificados individualmente PASS, FAIL, BLOCKED ou NOT RUN; SKIPPED nunca é PASS.
11. Nenhuma exclusão de código legado ativo antes de paridade e aprovação do gate correspondente.
12. Toda mudança ocorre em branch e PR; proibidos commits diretos na main, force-push, reset destrutivo e merge não autorizado.

## 8. Limites desta ADR

Esta entrega é uma decisão arquitetural e auditoria inicial. Não afirma que o frontend Rust já compila, que o renderer D3D12 está integrado ao eframe, que a biblioteca de ROM foi migrada ou que Python pode ser removido hoje. A implementação funcional começa em M1 após completar o grafo de dependências e contratos de sessão.

**Baseline preservada:** `b82f748b83749eac2173ff86e535cc2a9e2ca862`.
