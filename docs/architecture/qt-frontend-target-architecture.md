# Arquitetura-alvo enxuta do frontend Qt

**Estado:** MODELING — proposta inicial para revisão.  
**Base observada:** main em 2026-10-30, commit \`2318fb35715c49c26e63f71debadacdb1816c79c\`.  
**Documento funcional associado:** [qt-frontend-functional-model.md](qt-frontend-functional-model.md).

## 1. Decisão arquitetural

A nova Qt não deve ser uma reescrita integral. Deve ser uma evolução modular do frontend existente, preservando serviços e contratos funcionais. A unidade central é uma **Application Controller** que recebe intenções da UI, coordena serviços e publica um snapshot imutável do estado observável da aplicação.

A UI não controla diretamente objetos de hardware nem decide por conta própria se uma operação foi concluída. Ela emite comandos; a camada de aplicação valida contexto, executa a operação apropriada e publica o resultado/novo estado.

## 2. Camadas mínimas

\`\`\`text
Qt Widgets / Qt Quick
        |
        v
Presentation (MainWindow, menus, toolbar, dialogs, status)
        |
        v
Application Controller
  |       |        |        |
  v       v        v        v
Session  Media  Preferences  Host capabilities
  |       |        |        |
  +-------+--------+--------+
                  |
                  v
             PyO3 facade
                  |
                  v
          Rust emulation runtime
\`\`\`

### 2.1 Presentation
Responsável por widgets, navegação, ações, acessibilidade e feedback. Não contém regras de hardware, descoberta de capacidades em loops de execução nem persistência dispersa em widgets.

Componentes visuais propostos:
- \`MainWindow\`: composição da janela e encaminhamento de ações.
- \`MenuBarBuilder\` e \`ToolbarBuilder\`: constroem ações a partir de um catálogo central de comandos.
- \`VideoSurface\`: apresentação de frames; não possui nem altera estado do VDP.
- \`StatusPresenter\`: converte snapshot de estado em indicadores.
- Diálogos/páginas por domínio: opções, drivers, controles, região, biblioteca e diagnóstico.

A separação acima é lógica; não exige criar um arquivo/classe para cada item antes de haver necessidade.

### 2.2 Application Controller
Ponto único de coordenação entre UI e serviços. Deve:
- receber comandos tipados (abrir, fechar, play, pause, stop, reset, trocar sistema, alterar preferências);
- validar pré-condições e conflitos;
- coordenar persistência antes de descarregar sessão;
- publicar eventos/resultados e snapshots de estado;
- centralizar política de confirmação e erros de operação.

Não deve virar um objeto monolítico com lógica de todos os domínios. Delegar operações aos serviços abaixo.

### 2.3 Serviços de aplicação
Começar somente com domínios comprovadamente necessários:

- **SessionService:** ciclo de vida da máquina/sessão, comandos de execução e transições de estado.
- **MediaService:** seleção, validação, identificação e carregamento de ROM/BIOS; histórico e último diretório por sistema.
- **PreferencesService:** leitura, validação, migração e gravação das preferências.
- **HostCapabilitiesService:** enumeração de monitores, modos, dispositivos de áudio/input e capacidades das APIs; atualiza em hotplug.
- **LibraryService:** varredura e auditoria da biblioteca, reutilizando os scanners existentes.
- **DiagnosticsService:** agrega eventos e dados já expostos pelos componentes; não cria uma segunda implementação de logging.

Não criar um serviço separado se o módulo atual já cumpre o contrato. A matriz de reconciliação é obrigatória antes de extrair ou mover código.

### 2.4 Fronteira nativa
PyO3 expõe operações de alto nível e snapshots/eventos necessários à aplicação. Rust continua proprietário do estado de hardware. A UI não chama métodos de CPU/VDP/PSG por instrução. Não transportar framebuffers grandes em cópias repetidas se o contrato atual já oferece caminho de apresentação mais eficiente.

## 3. Modelo de estado da aplicação

Separar três dimensões que não devem ser reduzidas a um único booleano:

1. **Sistema selecionado na UI:** contexto de navegação, catálogo e tema.
2. **Sessão da máquina:** existência e ciclo de vida da máquina/runtime.
3. **Mídia/boot:** ROM carregada, BIOS-only ou sem mídia.

Estados de sessão propostos:
- \`CLOSED\`: máquina/sessão encerrada.
- \`READY\`: sessão inicializada e pronta, sem execução.
- \`RUNNING\`: executando.
- \`PAUSED\`: pausada.
- \`STOPPING\`: persistência e descarregamento em andamento.
- \`ERROR\`: falha que impede continuar; requer recuperação/encerramento controlado.

Estado de mídia:
- \`NONE\`
- \`CARTRIDGE_LOADED\`
- \`BIOS_ONLY\`

O sistema selecionado não pode mudar enquanto sessão não estiver em \`CLOSED\` e mídia estiver em \`NONE\`. A tela preta inicial é uma apresentação visual, não um estado de máquina.

Transições essenciais:
- \`CLOSED -> READY\`: inicialização explícita de BIOS-only ou preparação de uma ROM.
- \`READY -> RUNNING\`: Play.
- \`RUNNING -> PAUSED\`: Pause.
- \`PAUSED -> RUNNING\`: Play/Resume.
- \`READY|RUNNING|PAUSED -> STOPPING -> CLOSED\`: Stop/fechar ROM, após persistência necessária.
- Reset de hardware permanece dentro da sessão e não descarrega mídia.
- Erros de persistência bloqueiam descarregamento silencioso; a política de recuperação deve preservar dados e informar o usuário.

O contrato final de \`READY\`, BIOS-only e falhas de boot será reconciliado com as capacidades atuais do runtime antes da implementação.

## 4. Catálogo de comandos e habilitação

Menus, toolbar e atalhos não implementam callbacks independentes. Todos apontam para identificadores de comando, por exemplo:
- \`media.open\`
- \`session.play\`
- \`session.pause\`
- \`session.stop\`
- \`session.reset_hardware\`
- \`session.restart_runtime\` (pendente de contrato)
- \`system.select\`
- \`settings.open\`
- \`diagnostics.open\`

Um único \`CommandState\` determina enabled/disabled, texto contextual, atalho e disponibilidade. O estado vem do Application Controller, não de inferência local dos widgets. A confirmação de ações destrutivas também é centralizada.

## 5. Preferências e persistência

Manter \`FrontendPreferences\` como modelo validado e sem dependência de PySide6. Não migrar tudo para um novo formato sem inventário de chaves e compatibilidade.

Princípios:
- SettingsStore é adaptador de armazenamento; widgets não escrevem diretamente em QSettings.
- Valores globais, por sistema e por periférico têm namespaces explícitos.
- Defaults e migrações são versionados.
- Gravações críticas são atômicas quando aplicável.
- Histórico guarda identidade do sistema e caminho, com limite funcional de 10 itens definido no modelo funcional; a constante atual de 12 precisa ser reconciliada.
- Configurações de APIs alternativas são preservadas.
- Preferências não armazenam estado interno da máquina.

## 6. Descoberta de capacidades do host

A descoberta ocorre na camada host, fora do caminho crítico de instruções. Resultados devem declarar:
- identificador estável do dispositivo;
- nome amigável e tipo;
- API que o enumerou;
- capacidades suportadas e limitações;
- estado conectado/ativo;
- instante/geração da enumeração, para invalidar escolhas após hotplug.

Distinguir **selecionado**, **disponível**, **inicializado** e **efetivamente ativo**. A UI nunca deve inferir que selecionar Vulkan, WASAPI ou XInput significa que o backend foi inicializado com sucesso.

## 7. Concorrência e responsividade

- A thread principal Qt é dona dos widgets.
- Varreduras, auditorias e operações de I/O potencialmente demoradas executam fora da thread de UI.
- Resultados retornam por signals/slots Qt com objetos de dados imutáveis ou cópias pequenas.
- Cancelamento precisa ter semântica explícita e estado visível.
- Não criar uma thread por tarefa nem paralelizar a execução determinística do hardware.
- Ownership e sincronização entre Qt, PyO3 e runtime devem ser documentados por contrato; não liberar GIL ou expor referências compartilhadas sem análise.

## 8. Inventário inicial e reconciliação obrigatória

Inventário remoto realizado na branch `main`, SHA-base `2318fb35715c49c26e63f71debadacdb1816c79c`. Os SHAs de cada arquivo foram consultados no GitHub durante esta rodada.

### 8.1 Composição e apresentação

- `frontend/qt_frontend.py` (SHA `81ce472fcb2edee432c7290a6defbffcbef1ede9`): entrypoint/orquestrador. A função `run()` coordena settings, criação de máquina SMS, runtime, áudio e janela. É o ponto prioritário para rastrear composição e ownership antes de extrair um Application Controller.
- `frontend/qt/main_window.py` (SHA `a1965d2f6d2e08a00e16adb9efa40a92ad1f038`): janela de 402 linhas; menus/toolbar, abas Emulador/Debug, status bar e callbacks injetados. A janela não importa diretamente a máquina, mas recebe numerosas operações por callback.
- `frontend/qt/video_widget.py` (SHA `d9a7f9e2249d0ddbd1de60460a3a87ad127aeaa6`): superfície Qt Quick, Vulkan/D3D11/D3D12 e fallback raster; `FramebufferImageProvider` usa lock e `QImage`. Confirmar cópias e custo no caminho de frame.
- `frontend/qt/localization.py`: localização compartilhada.
- `frontend/qt/video_filters.py`: filtros existentes.
- `frontend/qt/focus_pause.py` e `frontend/qt/frame_pacer.py`: pausa por foco e pacing já separados em módulos.

### 8.2 Preferências, configurações e drivers

- `frontend/qt/preferences.py` (SHA `196e3bae1a3f58a4077f9e0bdf4032393b1bc4e0`): dataclass `FrontendPreferences`, protocolo `SettingsStore`, validação, leitura/gravação e histórico atual de 12 caminhos simples.
- `frontend/qt/settings_dialog.py` (SHA `51e9879ebc0917d7c4e03698527760ec57c94348`): diálogo de áudio/PSG, vídeo, firmware SMS, filtros, VSync/VRR, sincronização A/V, integer scaling, CRT e limite de sprites.
- `frontend/qt/drivers_dialog.py` (SHA `024aa32b7eef5aa214736719ec780046c13a896e`): seleção XInput/SDL3, WASAPI/XAudio2 e Vulkan/D3D11/D3D12/Raster. Seleção persistida não prova inicialização nem backend ativo.
- `frontend/qt/sms_input_settings.py`: página de controles SMS; o próprio módulo documenta Light Phaser/mira e detecção por raster como pendentes no gameplay.
- F32=C exige migrador automático e relatório de incompatibilidades. Antes de implementá-lo, enumerar chaves antigas, tipos, defaults, escopos e versões. A migração deve preservar valores válidos e não descartar incompatibilidades silenciosamente.
- F35=C exige restauração global e por categoria, ambas confirmadas. As categorias devem corresponder ao schema efetivo, não a grupos arbitrários de widgets.

### 8.3 Biblioteca, mídia e identificação

- `frontend/qt/rom_library_dialog.py` (SHA `fa3335a1229c748971b54b7d9fbfbd2dccd0d0c0`) + `rom_library.py` (SHA `6843c2a8dab0d4ac3cebbb6edecc45d6465196b5`): UI de biblioteca e scanner existente. A varredura usa `QRunnable`/`QThreadPool`, sinais de progresso/conclusão, cancelamento por `Event`, índice persistente, hashes e agrupamento de duplicados. Reutilizar.
- `rom_loader.py` (SHA `3cee48dc1804abaa8128cabc1abac2df01716208`): carregamento/validação de ROM e membros ZIP.
- `rom_catalog.py` (SHA `f775b0a6f9cf521a7a271e4413bc8f528ce82714`): catálogo DAT, identificação por hashes e inspeção de mídia.
- `scraper_settings_dialog.py` e `screenscraper.py`: integração de metadados/capas já existente; não faz parte do núcleo de sessão.

### 8.4 Input, áudio e diagnóstico

- `frontend/controller_profiles.py` (SHA `90a0489486ecc816d1055922dd959fb2a687652c`): modelos/serialização de perfis de jogador e configuração de controles.
- `frontend/native_first_controller_input.py` (SHA `a90ed8ee53fc4a602426336aa677b7652bf0a580`): XInput nativo e fallback SDL3; snapshots normalizados na fronteira de input.
- `frontend/qt/audio_output.py` (SHA `ce007a1b75a5c65af8184a66848dfb995a09cf01`): facade de `PcmResampler` nativo Rust/PyO3.
- `frontend/qt/wasapi_audio.py` (SHA `5b043fcb0ac39c2df86b97be20fc10b5567167a2`): facade de saída de áudio Rust; expõe taxa, buffer, callbacks de underrun, blocos descartados e escrita PCM.
- `frontend/qt/debug_logging.py` (SHA `3e6db05e522b6d2bae65e87adeb386fef16174d0`): logger opt-in assíncrono, fila limitada e thread dedicada de escrita.
- F33=C determina separar Diagnóstico de Debug técnico. Diagnóstico deve agregar estado observável/capacidades/métricas/falhas; Debug mantém logs e traces. Reutilizar o logger existente; não criar segundo pipeline.
- F34=C exige relatório com backend solicitado, ativo, fallback, motivo de falha e capacidades detectadas. Distinguir solicitado, disponível, inicializado e efetivamente ativo.

### 8.5 Testes encontrados na main

Testes de frontend diretamente relacionados: `test_qt_main_window.py`, `test_qt_preferences.py`, `test_qt_drivers_dialog.py`, `test_qt_settings_dialog.py`, `test_qt_rom_library_dialog.py`, `test_qt_video_driver_selection.py`, `test_qt_frame_pacer.py`, `test_qt_audio_output.py`, `test_qt_sms_input_settings.py`, `test_qt_key_mapping.py`, `test_focus_pause.py`, `test_qt_box_art.py`, `test_qt_scraper_settings.py`, além de testes unitários de configuração/estado de áudio, logging, formatadores de diagnóstico, Vulkan e WASAPI. Também existem testes de perfis de controle, input native-first/SDL3 e integração de runtime Rust.

Workflows presentes na main incluem `qt-frontend-validation.yml`, `ui-controller-validation.yml`, `ci.yml`, além de workflows separados para SG-1000, SMS e Z80. A existência do workflow não é evidência de execução/aprovação nesta PR; filtros, SHA candidato e logs ainda precisam ser verificados quando houver mudança executável.

### 8.6 Lacunas de evidência

- O inventário de módulos confirma componentes reutilizáveis, mas ainda não rastreou cada callback da janela até o serviço, runtime Rust e fronteira PyO3.
- `qt_frontend.py` concentra a composição em `run()` e define closures locais para timer, input, frame presentation, ROM loading, troca de sistema, região e drivers. É uma concentração relevante de orquestração; não extrair classes antes de mapear dependências e ownership.
- `main_window.py` atualmente expõe Play/Pause por uma ação toggle e Reset, mas o menu Arquivo não possui Fechar ROM e o menu Emulação não possui Stop. Logo, Stop/Fechar ROM aprovados ainda não estão representados como comandos visíveis no shell atual; a ausência deve ser confirmada no fluxo de aplicação, não corrigida diretamente na janela sem contrato.
- Não foi concluída a matriz de todas as chaves QSettings, migrações existentes e namespaces por sistema/periférico.
- Não foi demonstrado que Stop e Arquivo → Fechar ROM já compartilham a mesma operação central.
- Não foi demonstrado relatório unificado de backend com fallback/motivo/capacidades, nem rollback completo após falha.
- Não foi demonstrada descoberta de monitor/modos/Hz/VRR efetivos.
- Métricas de áudio disponíveis no facade precisam ser reconciliadas com o backend real; estimativa não pode ser rotulada como medição ponta a ponta.
- O inventário de testes identifica cobertura existente, mas não prova cobertura de todas as transições funcionais aprovadas.

Os caminhos e SHAs acima são evidências do inventário consultado. Ausência de evidência em um ponto não deve ser descrita como ausência definitiva de implementação até rastrear referências, contratos e testes.

### 8.7 Rastreamento de sessão e preferências — aprofundamento

**Composição observada em `qt_frontend.py`:**
- `run()` instancia `QApplication`, `QSettings`, `FrontendPreferences`, `RomCatalog`, controlador de input, resampler, saída de áudio e `EmulatorMainWindow`.
- O estado de sessão permanece em variáveis locais/closures: `machine`, `loaded`, `active_system`, `pause_controller`, `timer`, `frame_pacer`, região SMS e contadores de frame.
- A janela recebe callbacks que fecham sobre essas variáveis. O entrypoint concentra carregamento, troca de sistema, reset, pacing, input, áudio, gravação VGM, configuração e diagnóstico.
- `show_frame()` executa `machine.run_frame()`, drena eventos PSG/PCM, escreve áudio e copia framebuffer para `QImage`. A fronteira de apresentação faz cópia explícita do framebuffer; o caminho e custo devem continuar separados do runtime.
- `reset_machine()` cria uma nova instância `SG1000Machine` ou `SMSMachine` com os dados da ROM carregada, zera métricas do frontend e retoma timer. Isso é a implementação observada de reset atual; não é evidência de um reset físico detalhado por componente nem de Restart Runtime preservando estado interno.
- `open_rom()` prepara candidato antes de substituir `machine/loaded`; se falha no carregamento, mostra aviso e mantém o fluxo anterior. Depois da preparação, troca máquina/mídia, atualiza histórico, UI e timer.
- `choose_system()` abre a biblioteca e chama `open_rom()` com a ROM/sistema escolhidos. Não foi observada confirmação explícita para encerrar uma sessão ativa antes da troca.
- Não foi localizado comando Stop/Fechar ROM nem uma rotina central de encerramento de sessão no trecho auditado. Pause apenas para o timer; reset reconstrói a máquina com ROM atual. Portanto, o comportamento aprovado de Stop ainda precisa de contrato e implementação futura.
- `stop_vgm_recording()` é uma dependência de encerramento: gravação ativa precisa ser finalizada ou o fechamento/troca deve ser cancelado. O mesmo padrão de falha transacional deve ser avaliado para SRAM pendente.

**Inventário parcial de chaves persistidas** (não inclui chaves de módulos ainda não rastreados):
- Preferências de `FrontendPreferences`: `window_width`, `window_height`, `video_scale`, `recent_roms`, `controller_config`, `firmware_directory`, `sms_bios_boot_enabled`, `audio_enabled`, `diagnostics_enabled`, `cpu_core`, `audio_sample_rate`, `audio_buffer_size`, `audio_volume`, `audio_balance`, `audio_queue_blocks`, `video_filter`, `integer_scale`, `sprite_limit_enabled`, `vsync_mode`, `vrr_mode`, `av_sync_mode`, `crt_scanline_intensity`, `crt_scanline_thickness`, `crt_phosphor_mask`, `crt_curvature`, `crt_signal_mode`.
- Chaves adicionais acessadas diretamente no entrypoint: `ui_language`, `video_driver`, `audio_driver`, `input_driver`, `window_state`, `last_directory`, `last_rom`, `sms_region_preference`.
- `recent_roms` é lido/escrito tanto pelo entrypoint quanto pelo modelo de preferências; atualmente o caminho direto limita a 10 itens, enquanto `FrontendPreferences.MAX_RECENT_ROMS` ainda é 12. A estrutura persiste caminhos sem sistema explícito; inferir sistema pela extensão não é confiável para extensões compartilhadas.
- `diagnostics_enabled` é sempre forçado a `False` na leitura/gravação de `FrontendPreferences`; a captura de Debug é opt-in e sessão-local. Não tratar a chave como preferência funcional ativa.
- `cpu_core` legado diferente de `rust` é sobrescrito durante `from_store()`; esta é uma migração pontual sem versão global do schema nem relatório de incompatibilidades.
- As preferências não contêm campos explícitos para monitor, modo de vídeo, VRR efetivo, device ID de áudio ou versão do schema. Isso não prova que nenhuma outra camada armazene tais dados; exige busca integral.

**Backends — comportamento observado:**
- Input: preferência `auto/xinput/sdl3`; validação de backend forçado antes de persistir. A disponibilidade é consultada no objeto de input existente.
- Vídeo: mudança de API pode exigir reinício; para certos casos, tenta `view.set_render_backend()` e rejeita aplicação quando indisponível. Não há no trecho um objeto de relatório uniforme com estado solicitado/disponível/inicializado/ativo.
- Áudio: fecha a saída anterior e tenta inicializar a nova. Se a inicialização falha, `_initialize_audio_backend()` retorna `None`, mas a preferência selecionada pode ser persistida e a mensagem indica indisponibilidade. Durante falha de escrita PCM, fecha a saída e mantém emulação sem áudio.
- `NativeAudioOutput` expõe sample rate, buffer, underrun callbacks e dropped blocks via extensão nativa. Não expõe ID/nome do dispositivo, latência física ponta a ponta nem motivo estruturado de fallback.
- A troca de drivers não é totalmente transacional: input/vídeo são validados em partes, depois as preferências são gravadas e áudio é reaberto. A ordem, rollback e coerência quando o backend de áudio falha precisam de contrato/testes.

**Critérios adicionais para a matriz:**
- Testar Pause → Play sem reconstruir máquina nem descarregar ROM.
- Testar Stop/Fechar ROM: persistência concluída antes do unload; falha em SRAM/VGM deve impedir perda silenciosa; aplicação continua aberta; mídia fica NONE.
- Testar Reset preservando identidade da ROM e região aplicada após reset; distinguir da reconstrução genérica atual.
- Testar troca de sistema com sessão ativa, cancelamento de confirmação e falha ao salvar dados pendentes.
- Testar migração QSettings idempotente, preservação de valores desconhecidos/incompatíveis em relatório e rollback seguro.
- Testar histórico com 10 itens, caminhos iguais em sistemas distintos, extensão ambígua, arquivos movidos/removidos e migração dos 12 itens legados.
- Testar backend solicitado indisponível, fallback automático, falha após inicialização, falha em escrita e rollback sem declarar preferência como backend ativo.

## 9. Matriz inicial de lacunas

| Domínio | Base observada | Decisão/necessidade | Lacuna e evidência de aceitação |
|---|---|---|---|
| Shell/comandos | MainWindow e entrypoint Qt existentes; callbacks injetados | Menus, toolbar e atalhos com semântica comum | Rastrear callbacks; teste por comando e estado, sem duplicar lógica |
| Sessão/mídia | Entry point compõe máquinas; loader e catálogo existem | Pause retém mídia; Stop/Fechar ROM descarregam; reset preserva mídia | Contrato único de sessão; testes de transições e falhas de persistência |
| Preferências | FrontendPreferences/SettingsStore existentes | F32=C migração automática com incompatibilidades reportadas | Inventário de chaves; fixtures de versões antigas; idempotência e preservação |
| Restauração | Defaults em diálogos/modelo já existem | F35=C restauração global e por categoria, confirmadas | Testes garantindo isolamento entre categorias e revalidação do schema |
| Diagnóstico | Aba Debug, logger assíncrono e formatadores existentes | F33=C Diagnóstico separado de Debug | Snapshot confiável separado de logs; não duplicar logger |
| Backends | DriversDialog e facades de áudio/vídeo/input | F34=C solicitado/ativo/fallback/motivo/capacidades | Testes com backend indisponível, fallback, erro e rollback |
| Biblioteca | RomLibrary e RomLibraryDialog com scan assíncrono | Reutilizar biblioteca existente | Testar cancelamento, atualização, inacessibilidade e duplicados |
| Vídeo | VideoWidget Qt Quick/raster e filtros | Proporção original/integer scaling; capacidades efetivas | Descoberta de monitor/modos/VRR e fallback com evidência host |
| Áudio | Facades nativas, buffer e contadores de underrun | Buffer/latência estimada/underruns | Separar estimativa de medição; testar dispositivo removido e erro |
| Input | Perfis, XInput/SDL3 e página SMS | Físico/lógico/mapeamento separados; hotplug preserva perfil | Testar reconexão e mapeamentos; não declarar Light Phaser funcional |
| Saves | Persistência SRAM e Save State precisam reconciliação adicional | SRAM automática; 10 slots como alvo futuro | Testar gravação segura; Save State depende de serialização determinística |

A matriz é uma linha de base do inventário remoto, não uma certificação de completude.

## 10. Ordem de trabalho e critérios de saída

**Fase A — inventário completo**
- Mapear entrypoint, composição de dependências, callbacks, serviços, settings, scanner, diálogos, backends, testes e workflows.
- Entregar mapa de dependências e matriz requisito → implementação → teste → lacuna.
- Não editar código.

**Fase B — contratos**
- Definir estados/transições, catálogo de comandos, resultados/erros e persistência.
- Definir fronteira de capacidades do host e snapshots de status.
- Revisar conflitos com os contratos PyO3/Rust existentes.

**Fase C — protótipo de shell**
- Somente após aprovação dos contratos.
- Alterar em fatias pequenas, preservando nomes públicos e testes úteis.
- Testar widgets/ações e regressões do fluxo de ROM.
- Não introduzir backend ou recurso host não comprovado.

**Fase D — integração por domínio**
- Biblioteca/mídia, sessão, vídeo, áudio, controles e diagnóstico, cada um com aceitação própria.

Condição para sair da modelagem: nenhum requisito aprovado sem destino arquitetural; toda lacuna tem responsável/camada e critério de aceitação; nenhuma funcionalidade é prometida sem contrato e teste possível; a proposta não duplica módulo existente.

## 11. Papéis de revisão desta rodada

- **Arquiteto:** camadas, ownership, estados e contratos.
- **Auditor Qt:** inventário real e reutilização de componentes.
- **Revisor de lacunas:** inconsistências, estados de erro, migração e critérios de aceitação.

Esta revisão foi executada como três frentes analíticas coordenadas nesta sessão, não como três processos autônomos independentes.

**Não implementado:** este documento não altera runtime, widgets ou serviços. Não constitui teste, build, validação visual ou aprovação de gameplay.


## 12. Decisões funcionais recebidas e impacto arquitetural — 2026-10-30

O modelo funcional registra 31 escolhas explícitas (F01–F31) e mantém F32–F35 pendentes. Este resumo destaca os impactos arquiteturais; a tabela completa e as regras de interpretação estão em [qt-frontend-functional-model.md](qt-frontend-functional-model.md), seção 14.

- **Stop versus Fechar ROM (F01=C):** são comandos distintos. Stop encerra a emulação, persiste dados pendentes e descarrega a mídia, mantendo a aplicação Qt aberta. Pause mantém a ROM e a sessão carregadas em estado suspenso; Play retoma a execução. A máquina de estados deve representar PAUSED com mídia carregada; Stop encerra a sessão e descarrega a mídia, mas não encerra o processo Qt.
- **Reiniciar runtime (F02=B):** operação separada de Reset físico. Preserva a mídia; exige contrato de reconstrução/reinicialização, persistência de SRAM pendente, recuperação de falhas e ownership PyO3/Rust.
- **Reset físico (F03=A) e região (F31=C):** reset mantém a sessão; alteração regional é aplicada somente após Reset. Validar se a infraestrutura atual consegue reconstruir componentes dependentes de região sem estado inconsistente.
- **Play/Pause (F04=A):** Play inicia ou retoma; Pause suspende.
- **Pacing (F05=B, F06=A, F07=C):** opções de velocidade 0,5x/1x/2x/ilimitada; Fast Forward é execução máxima sem sincronização com monitor; perfis de precisão, baixa latência e desempenho permanecem políticas do host. Não alterar clocks nominais do hardware.
- **Mídia e navegação (F08=A, F09=B, F10=D, F11=C):** histórico global de 10 itens com sistema+caminho; remover automaticamente itens indisponíveis; tela inicial configurável; troca de sistema exige confirmação quando houver sessão ativa.
- **Boot (F12=A, F13=C, F14=C):** permitir BIOS-only com firmware válido e suporte; seleção automática com override manual; boot sem BIOS quando o sistema/runtime suportar explicitamente.
- **Host capabilities (F15–F18, F20–F21):** seleção automática por padrão, controles manuais, hotplug e exposição do backend/valores efetivos. Capacidades precisam ser descobertas e reportadas; preferências selecionadas não provam inicialização.
- **Vídeo (F19=A):** manter proporção original e integer scaling como opções aprovadas; reconciliar opções já existentes sem descartar configurações silenciosamente.
- **Áudio (F22=B):** mostrar buffer, latência estimada e underruns; não rotular estimativa como medição ponta a ponta.
- **Input (F23=B, F24=C, F25=C, F26=C):** separar dispositivo físico, periférico lógico e mapeamento; hotplug preserva perfis; catálogo contextual por sistema, incluindo periféricos especiais previstos sem declarar suporte ainda não validado.
- **Persistência (F27=A, F28=A, F29=B):** SRAM com salvamento automático quando alterada e em encerramentos seguros; 10 slots de Save State como alvo, condicionados à serialização determinística; identificação por hash, sistema, região e versão do emulador.
- **Perfil regional (F30=B):** distinguir região comercial, padrão de vídeo e perfil de hardware; não foi escolhida revisão de hardware/BIOS como dimensão obrigatória de UI.

### 12.1 Ajustes obrigatórios no modelo de estado

A máquina de estados da seção 3 deve acomodar Stop descarrega mídia, mas não fecha a aplicação Qt. Portanto, READY não pode ser interpretado como “sem ROM”: é necessário distinguir sessão pronta com mídia carregada, BIOS-only e sessão encerrada. A representação recomendada é manter dimensões ortogonais:
- lifecycle da sessão: CLOSED, READY, RUNNING, PAUSED, STOPPING, ERROR;
- tipo/estado da mídia: NONE, CARTRIDGE_LOADED, BIOS_ONLY;
- estado de execução: READY/STOPPED, RUNNING ou PAUSED (ou enum equivalente claramente definido).

A transição Pause resulta em PAUSED + mídia carregada. Stop/Fechar ROM resulta em CLOSED + NONE após persistência segura, mantendo a aplicação Qt aberta. Troca de sistema com sessão ativa requer confirmação e só ocorre após encerramento seguro. O contrato de Restart Runtime deve preservar a identidade da mídia e não ser confundido com Reset físico.

### 12.2 Decisões F32–F35 agora aprovadas

- **F32=C — migração:** migrar automaticamente as configurações atuais e registrar incompatibilidades para revisão. Preservar valores válidos; incompatibilidades não devem ser descartadas silenciosamente. Inventariar chaves, tipos, defaults, escopos e versões antes de implementar.
- **F33=C — diagnóstico:** separar Diagnóstico (estado observável, capacidades, métricas e falhas) de Debug técnico (logs/traces). Reutilizar a infraestrutura de logging existente; não criar segundo pipeline.
- **F34=C — relatório de backend:** exibir backend solicitado, ativo, fallback aplicado, motivo de falha e capacidades detectadas. Distinguir solicitado, disponível, inicializado e efetivamente ativo.
- **F35=C — restauração:** oferecer restauração global e por categoria, ambas com confirmação; restaurar uma categoria sem alterar as demais.

**Estado:** 35/35 decisões funcionais registradas. Sem alterações em código executável e sem validação de runtime.

## 13. Correção de F01 — semântica de Stop e Pause

Correção explícita do usuário em 2026-10-30:

- **Pause:** mantém sessão e ROM carregadas, suspende a execução e permite retomar por Play.
- **Stop:** encerra a sessão de emulação e descarrega a ROM após persistência segura; não fecha a aplicação Qt.
- **Fechar ROM:** deve compartilhar a mesma semântica de encerramento/descarregamento de Stop, se o inventário confirmar que ambos podem encaminhar ao mesmo comando central.
- **Reset físico:** permanece dentro da sessão e preserva a mídia.

Consequência para o modelo: PAUSED é o estado com ROM carregada e execução suspensa; CLOSED + mídia NONE é o resultado de Stop. Não manter estado READY com cartucho carregado como consequência de Stop.
