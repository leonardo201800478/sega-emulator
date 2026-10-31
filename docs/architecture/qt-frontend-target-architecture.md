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

Arquivos confirmados na main durante a inspeção:
- \`frontend/qt/main_window.py\`: janela, menus, toolbar, abas Emulador/Debug, status bar e callbacks.
- \`frontend/qt/video_widget.py\`: superfície Qt Quick, APIs gráficas Vulkan/D3D11/D3D12 e fallback raster; provider de imagem com cópia protegida por lock.
- \`frontend/qt/preferences.py\`: dataclass validada independente de PySide6, adaptador de armazenamento compatível com QSettings, defaults e migração defensiva.
- \`frontend/qt/drivers_dialog.py\`: seleção de APIs XInput/SDL3, WASAPI/XAudio2 e Vulkan/D3D11/D3D12/Raster; informa que mudança gráfica requer reinicialização.
- \`frontend/qt/settings_dialog.py\`: configura áudio/PSG e vídeo; já possui filtros, VSync/VRR, sync A/V, integer scaling e opção de limite de sprites.

Observações objetivas da inspeção:
- \`main_window.py\` ainda tem callbacks obrigatórios/opcionais numerosos e constrói menus diretamente no construtor. Isso sugere acoplamento de composição, não prova que a lógica de aplicação esteja toda dentro da janela.
- A janela atual inclui menu de região do Master System e ação “Mudar sistema/ROM…”. A regra nova de seleção de sistema apenas com sessão encerrada precisa ser conferida contra o callback real.
- \`preferences.py\` limita histórico a 12 caminhos simples, enquanto a decisão funcional define 10 entradas globais com sistema e caminho. Necessário migrar sem perder dados e sem confundir caminhos de sistemas diferentes.
- \`settings_dialog.py\` apresenta valores fixos para taxa/buffer de áudio e descreve estimativa nominal de fila, não medição ponta a ponta. A proposta funcional exige descoberta e exposição de capacidades/valores efetivos; isso é lacuna, não implementação concluída.
- \`drivers_dialog.py\` apresenta seleções de APIs, mas o próprio texto distingue escolha persistida e reinicialização. Falta confirmar no inventário o relatório unificado de backend efetivamente ativo, fallback e rollback.
- \`video_widget.py\` contém estado de apresentação e fallback; não presumir que já oferece seleção de monitor, modos de resolução/Hz ou VRR confirmado.

Os caminhos acima são os arquivos verificados. Nomes de serviços, entrypoints, testes e outros módulos não devem ser presumidos: completar a busca de referências/importadores e testes antes de definir extrações.

## 9. Matriz inicial de lacunas

| Domínio | Base observada | Trabalho de modelagem |
|---|---|---|
| Shell da janela | MainWindow já existe | Extrair catálogo de comandos/estado sem reescrita ampla |
| Preferências | FrontendPreferences e store existem | Namespaces por sistema/periférico, limite 10, schema/migração |
| Biblioteca | Há UI/scanner e auditoria em desenvolvimento | Localizar serviço existente e reutilizar |
| Vídeo | VideoWidget Qt Quick/raster e filtros | Descoberta monitor/modos, efetividade, proporção/escala |
| Áudio | WASAPI/XAudio2 e settings | Dispositivo host, formatos efetivos, underruns/latência medida |
| Input | XInput/SDL3 selecionáveis | Inventário de dispositivos, periféricos lógicos e mapeamentos |
| Sessão | callbacks pause/reset e fluxo de ROM existentes | Máquina de estados única e semântica Stop/fechar |
| Diagnóstico | Aba Debug e ações diagnósticas | Agregação sem duplicar logging |
| Saves | SRAM e fluxo de Save State ainda a reconciliar | Persistência antes de unload; slots futuros |

Esta matriz é provisória. Uma célula sem evidência não é classificada como ausente: precisa de busca no código e nos testes.

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

- **Stop versus Fechar ROM (F01=C):** são comandos distintos. Stop termina a execução, mantém mídia e sessão carregadas em estado retomável; Fechar ROM encerra a sessão e descarrega a mídia. A máquina de estados precisa representar READY com mídia carregada depois de Stop.
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

A máquina de estados da seção 3 deve acomodar Stop sem descarregar mídia. Portanto, READY não pode ser interpretado como “sem ROM”: é necessário distinguir sessão pronta com mídia carregada, BIOS-only e sessão encerrada. A representação recomendada é manter dimensões ortogonais:
- lifecycle da sessão: CLOSED, READY, RUNNING, PAUSED, STOPPING, ERROR;
- tipo/estado da mídia: NONE, CARTRIDGE_LOADED, BIOS_ONLY;
- estado de execução: READY/STOPPED, RUNNING ou PAUSED (ou enum equivalente claramente definido).

A transição Stop deve resultar em sessão pronta/parada com mídia preservada; Fechar ROM resulta em CLOSED + NONE. Troca de sistema com sessão ativa requer confirmação e só ocorre após encerramento seguro. O contrato de Restart Runtime deve preservar a identidade da mídia e não ser confundido com Reset físico.

### 12.2 Decisões ainda pendentes

- **F32:** estratégia de migração das preferências atuais.
- **F33:** manter Debug, evoluir para diagnóstico geral ou separar os dois.
- **F34:** nível de detalhe do relatório de backend.
- **F35:** restaurar configurações globalmente, por categoria ou ambos.

Não inferir nem aplicar as sugestões anteriores para essas flags. O inventário completo da Fase A deve determinar quais componentes e contratos já existem antes de definir extrações ou implementação.

**Estado:** modelagem atualizada com decisões funcionais parciais. Sem alterações em código executável e sem validação de runtime.
