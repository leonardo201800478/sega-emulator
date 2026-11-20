# Roadmap de produto desktop — capacidades, UX, mídia e diagnóstico

**Estado:** documento normativo de planejamento; requisitos aprovados para orientar implementação, não declaração de recursos já concluídos.
**Data:** 2026-11-20
**Host-alvo:** Slint 1.18.x + WGPU 30.x/Vulkan + CPAL 0.18.2 + SDL3/input runtime.
**Escopo:** aplicação desktop, serviços de aplicação, persistência, biblioteca/carregamento de mídia, menus, CLI, diagnóstico e documentação de produto.

## 1. Autoridade e precedência

Este documento detalha a trilha transversal de produto desktop (R5) do [roadmap mestre](project-roadmap.md). Não substitui o roadmap de plataformas, ADRs, contratos do runtime nem especificações de hardware.

Precedência:

1. ADRs e invariantes de arquitetura/runtime/Scheduler.
2. [Roadmap mestre](project-roadmap.md) para ordem das plataformas e trilhas transversais.
3. Este documento para ordem e critérios de produto desktop.
4. Registro de capacidades e inventário de implementação, a serem criados nesta trilha.
5. Código, testes e evidências vinculados ao SHA efetivamente validado.

O Gear System é uma referência de clareza funcional: menus explícitos, opções identificáveis e funções organizadas. Não é um requisito de copiar sua aparência rústica, nem de priorizar seu debugger avançado. O objetivo é uma interface simples, organizada, consistente e que exponha somente capacidades reais do Sega Emulator.

## 2. Objetivo

Transformar o desktop em uma aplicação utilizável e previsível, cuja interface, CLI, documentação, seletores de arquivos e diagnósticos representem fielmente o que cada runtime implementa.

A aplicação deve:

- tornar as funções existentes descobríveis e acessíveis;
- não anunciar recursos apenas planejados, catalogados ou parcialmente integrados;
- persistir preferências e contexto operacional de forma confiável;
- selecionar mídias segundo as capacidades reais de cada sistema;
- não produzir logs, traces ou telemetria diagnóstica sem autorização explícita;
- manter o custo de observabilidade desprezível em relação ao orçamento de execução e nunca interferir na autoridade temporal do MachineScheduler.

## 3. Invariantes obrigatórios

### 3.1 Separação entre frontend e hardware

- Backend e hardware permanecem 100% Rust.
- Slint é responsável por GUI e interação; WGPU por apresentação; CPAL por saída física; SDL3/input runtime por gamepads.
- A GUI não implementa CPU, instruções, clocks, VDP, PSG/FM, barramentos, interrupções, temporização ou scheduler.
- O runtime mantém ownership da máquina e do MachineScheduler.
- A integração da GUI com a máquina permanece coarse-grained: carregar ROM, iniciar, pausar, parar, resetar, avançar quadro e consultar snapshots/estados imutáveis.
- Não adicionar callbacks de GUI por instrução ou por ciclo.
- Nenhuma gravação de arquivo, coleta diagnóstica, espera por dispositivo ou formatação de log pode bloquear o Scheduler ou o caminho crítico de áudio/vídeo.

### 3.2 Verdade funcional

Toda ação exposta deve apontar para uma capacidade identificada e testável. Uma opção visual sem serviço funcional não pode ser apresentada como implementada.

Estados permitidos no catálogo:

- `IMPLEMENTED`: funcional no caminho de produto e coberto por evidência.
- `PARTIAL`: existe, mas possui limitações ou integração incompleta claramente descritas.
- `PLANNED`: aprovado para desenvolvimento, ainda não disponível.
- `UNSUPPORTED`: não suportado pelo runtime atual.
- `NOT_VERIFIED`: evidência insuficiente para declarar estado.

A presença de código, DAT, extensão reconhecida por uma biblioteca de catálogo ou opção persistida não prova que a função esteja operacional.

## 4. Ordem normativa de desenvolvimento

A ordem abaixo organiza o trabalho de produto. Pode haver trabalho em paralelo quando explicitamente indicado, mas fases dependentes não podem ser puladas.

| Ordem | Entrega | Prioridade | Critério de saída |
|---|---|---|---|
| DP-0 | Auditoria funcional e política de diagnóstico | Bloqueante | Inventário completo de menus, ações, serviços, logs e sinks; política opt-in aplicada a todos os caminhos |
| DP-1 | Registro central de capacidades | Bloqueante | Fonte única de verdade com estado, sistema, dependências, configuração, UI, CLI, documentação e testes |
| DP-2 | Preferências e recuperação de estado da janela | Alta | Salvar/restaurar, defaults, diretórios por sistema/operação, geometria validada e recuperação de estado inválido |
| DP-3 | Catálogo de sistemas, formatos e carregamento | Alta | Matriz de mídia comprovada por loader/runtime; filtros gerados do catálogo; validação e rejeição segura |
| DP-4 | Organização funcional da interface | Alta | Menus e telas coerentes com capacidades reais; nenhuma ação visual sem implementação |
| DP-4A | HUD/OSD sobreposta ao jogo e mensagens operacionais | Alta | Overlay posicionado no viewport, catálogo tipado de eventos, fila limitada, prioridades/TTL e integração de lifecycle/input/ROM; Save State quando o serviço de slots existir |
| DP-5 | Diagnóstico básico opt-in e orçamento de desempenho | Alta | Snapshots coarse-grained, coleta desativada por padrão, limites e testes de overhead/armazenamento |
| DP-6 | CLI e documentação de capacidades | Média | `--help`, versão, comandos suportados e documentação gerados/validados a partir dos mesmos serviços |
| DP-7 | Auditoria final de integração e UX | Bloqueante para declarar R5 concluído | Testes de persistência, navegação, mídia, consentimento, recuperação, empacotamento e regressão aprovados |

**Regra de prioridade:** DP-0 deve começar imediatamente porque a geração não autorizada de logs é proibida. DP-1 estabelece a base comum. DP-2 e DP-3 vêm antes da finalização dos menus porque a interface depende de preferências e capacidades verdadeiras. DP-5 não autoriza debugger avançado. DP-6 não deve criar um segundo caminho de execução ou duplicar o runtime.

Esta trilha é transversal ao desenvolvimento dos sistemas. Ela não altera a ordem normativa SMS1/SMS2 → SG-1000 → Game Gear → expansões/acessórios 8-bit → Mega Drive → Sega CD → 32X, nem exige reescrever componentes de hardware validados.

## 5. DP-0 — auditoria funcional e política de diagnóstico

### 5.1 Inventário obrigatório

Auditar cada menu, submenu, janela, botão, atalho, seletor, opção persistida e comando. Para cada item registrar:

- identificador estável;
- categoria e nome apresentado;
- descrição e comportamento observável;
- estado funcional;
- sistemas e variantes compatíveis;
- serviço/API que executa a ação;
- configuração, valor padrão, limites e persistência;
- disponibilidade na GUI e na CLI;
- testes e documentação correspondentes;
- dependências e limitações conhecidas.

Categorias mínimas:

1. Sistemas e variantes.
2. Arquivos, biblioteca, ROMs, firmware/BIOS e saves.
3. Vídeo e apresentação.
4. Áudio e dispositivos de saída.
5. Controles, teclado e gamepads.
6. Opções e preferências gerais.
7. Diagnóstico básico.
8. Interface e janelas.
9. Ajuda, documentação e informações da versão.
10. CLI e comandos rápidos.

O inventário deve localizar também ações duplicadas, opções sem consumidor, preferências dormentes e caminhos que exibem sucesso antes da confirmação do serviço.

### 5.2 Proibição de coleta sem autorização

**É proibido gerar ou registrar logs, traces, telemetria diagnóstica ou relatórios automaticamente sem autorização explícita do usuário.** A proibição vale para Debug e Release e inclui:

- arquivos de log e arquivos rotativos;
- saída diagnóstica em console/terminal;
- traces de instruções, barramentos, Scheduler ou áudio;
- snapshots diagnósticos gravados automaticamente;
- métricas históricas e relatórios em background;
- captura automática de dados para crash/telemetria.

Mensagens operacionais necessárias para informar uma falha ao usuário na própria interface não devem ser convertidas automaticamente em registros persistentes.

Requisitos do consentimento:

- desligado por padrão em toda instalação e perfil;
- ação explícita e informada para habilitar;
- finalidade e escopo apresentados antes de iniciar;
- habilitação independente para logs e captura de diagnóstico quando os custos/volumes forem diferentes;
- revogação imediata, sem reinício;
- estado de consentimento persistido de forma explícita e auditável;
- build Release nunca implica consentimento;
- não iniciar coleta por flag implícita, configuração herdada, variável de ambiente ou fallback silencioso.

Quando desativado, o caminho de logging deve evitar formatação, alocação, serialização, I/O e envio para filas. A política deve abranger também bibliotecas e subsistemas que possam emitir logs próprios. Saídas diretas `println!`/`eprintln!` não podem permanecer como mecanismo operacional de diagnóstico.

### 5.3 Impacto e armazenamento

Com consentimento ativo:

- captura em thread de background, com fila limitada e política explícita de descarte;
- nenhuma espera do Scheduler por consumidor, disco ou interface;
- limites configuráveis de tamanho por arquivo, quantidade de arquivos e quota total;
- rotação e retenção finitas;
- taxa limitada para eventos repetitivos;
- sem dumps integrais de ROM, RAM ou estados por padrão;
- sem trace por instrução/ciclo, salvo ação específica e temporária do usuário;
- indicador visível de captura ativa, finalidade e destino;
- desligamento imediato ao revogar consentimento ou encerrar a sessão.

A infraestrutura de observabilidade já existente deve ser auditada antes de criar outra. Sua presença não prova que todos os sinks respeitam consentimento nem que o overhead é desprezível.

Critérios de aceite: testes demonstram zero arquivos/saídas diagnósticas sem consentimento; revogação interrompe novas gravações; limites são respeitados; fila cheia não bloqueia runtime; benchmark compara execução com observabilidade desligada e habilitada, registrando overhead no SHA avaliado.

## 6. DP-1 — registro central de capacidades

Criar um registro central de capacidades da aplicação, independente da camada visual. GUI, CLI, documentação, filtros de arquivos e telas de status devem consultar esse registro ou serviços derivados dele.

Cada capacidade deve declarar, no mínimo:

- `id` estável;
- categoria, nome e descrição;
- estado funcional;
- sistemas/variantes compatíveis;
- dependências de runtime e host;
- parâmetros, defaults e faixas válidas;
- persistência e escopo (global, sistema, operação ou sessão);
- disponibilidade GUI/CLI;
- motivo quando indisponível;
- testes/evidências e documentação.

Não duplicar listas de capacidades manualmente em múltiplos menus. Não inferir suporte a partir de nomes de sistemas, DATs, extensões, arquivos de firmware ou crates presentes.

## 7. DP-2 — preferências, diretórios e estado da janela

### 7.1 Ações de configuração

Toda seção configurável deve oferecer ações coerentes:

- **Salvar/Aplicar:** confirma as alterações e persiste somente após validação.
- **Restaurar padrões da seção:** restaura os defaults daquela categoria.
- **Restaurar tudo:** restaura as preferências do aplicativo, sem apagar ROMs, firmware, SRAM, save states, biblioteca ou outros dados do usuário.
- **Cancelar:** descarta alterações ainda não aplicadas, quando houver edição transacional.

A interface deve indicar se há alterações pendentes e comunicar erros de persistência sem perder silenciosamente a configuração anterior.

### 7.2 Estado da janela

Persistir:

- posição e dimensões;
- estado normal/maximizado e modo de tela, respeitando limitações do host;
- monitor/área de trabalho quando possível;
- layout e visibilidade de painéis;
- último sistema selecionado, quando válido.

Na inicialização, validar a geometria contra monitores e área de trabalho disponíveis. Se o estado estiver fora da tela, inconsistente ou corrompido, iniciar com geometria segura e oferecer restauração do layout. Deve existir uma ação acessível para **Restaurar layout padrão**.

Não persistir estado transitório de hardware como se fosse preferência visual.

### 7.3 Diretórios por sistema e finalidade

Manter diretório padrão independente por sistema para ROMs, incluindo sistemas futuros somente quando o runtime e o carregador estiverem disponíveis.

Manter também a última pasta usada por finalidade distinta:

- abrir ROM;
- localizar firmware/BIOS;
- carregar save state;
- salvar/exportar save state;
- importar/exportar dados e relatórios.

O diretório padrão de ROM por sistema e a última pasta visitada no diálogo são conceitos diferentes e devem ter chaves distintas. Ao abrir ROM, usar o diretório configurado para o sistema selecionado; ao retornar ao diálogo, restaurar a última pasta válida daquela operação. Não substituir a preferência de diretório padrão por uma navegação temporária.

Persistência deve ser versionada, validada, atômica e recuperável após arquivo truncado/corrompido. Não sobrescrever configurações válidas com defaults por falha transitória de leitura.

## 8. DP-3 — catálogo de sistemas e formatos de mídia

### 8.1 Regra fundamental

Extensão de arquivo é um filtro de seleção, não prova de formato válido. A compatibilidade precisa ser definida por sistema, tipo de mídia, loader e estado real do runtime.

A matriz de formatos deve registrar:

- sistema e variante;
- mídia (cartucho, cartão, disco, firmware);
- extensões aceitas pelo loader;
- formatos compactados aceitos e limites;
- estrutura interna esperada;
- requisitos de firmware;
- limites de tamanho e regras de validação;
- suporte a múltiplos arquivos/faixas;
- status do loader e do runtime;
- testes de aceitação e rejeição.

Extensões são comparadas sem distinção de maiúsculas/minúsculas. O carregador deve validar conteúdo, tamanho, cabeçalho quando aplicável, integridade e perfil de hardware. Arquivos inválidos devem receber erro claro.

### 8.2 Baseline de compatibilidade confirmada no desktop atual

A documentação do desktop consultada registra atualmente:

| Sistema/caminho | Formatos documentados | Estado/limitação |
|---|---|---|
| Master System no desktop Slint | `.sms`, `.bin`, ZIP contendo exatamente um membro `.sms` ou `.bin` | Caminho de carregamento documentado; limites de tamanho, quantidade de entradas e leitura em memória aplicados |
| SG-1000 | Não declarar filtro operacional no desktop Slint até concluir integração do sistema e loader correspondente | Core/runtime e disponibilidade desktop são gates distintos |
| Game Gear | Não habilitar | Sistema futuro |
| Mega Drive/Genesis | Não habilitar | Sistema futuro |
| Mega CD/Sega CD | Não habilitar `.cue`, `.iso`, `.chd` como mídia executável | Sistema futuro; catálogo/metadados não significam runtime de disco |
| 32X | Não habilitar | Sistema futuro |

A documentação legada de migração descreve loaders Python SG-1000 e SMS com extensões próprias, mas isso não amplia automaticamente o contrato do carregador nativo Slint. A matriz deve ser atualizada somente após inspeção e testes do loader efetivamente usado.

### 8.3 Formatos futuros a pesquisar, não a habilitar automaticamente

A pesquisa de hardware e dumps deverá avaliar, por sistema:

- SG-1000: dumps de cartucho e variantes documentadas;
- Master System/Mark III: dumps de cartucho e Sega Card/My Card;
- Game Gear: dumps de cartucho;
- Mega Drive/Genesis: variantes de dump de cartucho, incluindo `.md`, `.gen`, `.bin` e formatos intercalados quando aplicável;
- Mega CD/Sega CD: CUE/BIN, ISO e CHD, com trilhas, áudio, subcanais e firmware conforme exigidos;
- 32X: dumps e organização de ROM conforme o hardware e o loader a implementar.

ZIP, 7z e outros contêineres devem ser avaliados separadamente dos formatos de dump. Suporte a ZIP não implica suporte a 7z. CUE/ISO/CHD não devem ser tratados como ROM de cartucho. Arquivos BIN podem representar mídias diferentes e precisam de identificação contextual.

Nenhum formato futuro será apresentado como suportado até existir loader funcional, política de segurança, integração com runtime e testes positivos/negativos. ZIP deve continuar sem extração insegura; validar número de entradas, tamanho comprimido/descomprimido, expansão, ambiguidades e limites.

## 9. DP-4 — organização funcional da interface

A estrutura inicial de menus será:

| Menu | Funções |
|---|---|
| Arquivo | Abrir ROM, biblioteca, recentes, firmware/BIOS, saves e sair |
| Sistema | Sistema/variante, região, informações da máquina, reset e operações de sessão |
| Vídeo | Proporção, escala, sincronização e opções realmente implementadas |
| Áudio | Dispositivo, volume, mute e parâmetros suportados pelo runtime |
| Controles | Teclado, gamepads, mapeamento e perfis implementados |
| Opções | Preferências gerais, tema, persistência e restauração |
| Diagnóstico | Estado resumido do runtime e ativação explícita de captura |
| Ajuda | Ajuda contextual, documentação, compatibilidade, versão e créditos |

Regras:

- menus e janelas devem ser explícitos, consistentes e orientados às tarefas;
- recursos não disponíveis podem aparecer como planejados/desabilitados somente quando isso ajudar o usuário, com explicação do motivo;
- não apresentar controles decorativos ou opções sem efeito;
- não misturar opções de hardware com opções de apresentação;
- ações que alterem a sessão devem informar seu resultado real;
- navegação entre telas não pode alterar estado de emulação, proporção, fullscreen ou foco de forma acidental;
- layout deve permanecer utilizável em janela normal, maximizada e fullscreen;
- restaurar layout deve ser possível sem apagar preferências não relacionadas.

## 9A. DP-4A — HUD/OSD sobreposta ao jogo

**Prioridade:** Alta. **Issue:** [#1112 — HUD sobreposta ao jogo e catálogo de mensagens operacionais](https://github.com/leonardo201800478/sega-emulator/issues/1112).

A HUD/OSD é infraestrutura do produto desktop e deve ser implementada antes da integração RetroAchievements. O serviço será genérico: RetroAchievements poderá publicar eventos no futuro, mas não será dependência da HUD.

### Baseline e lacunas conhecidas

O README do desktop registra FPS visibility/text size persistidos, porém o posicionamento por canto permanece pendente porque a composição Slint atual não dispõe de uma superfície de overlay posicionável separadamente. Save/Load State existem como APIs in-memory no worker SMS, mas arquivos persistentes e slots na GUI ainda não estão implementados. Esses fatos devem ser revalidados na main antes de codificar.

### Catálogo de mensagens

| Domínio | Eventos/mensagens | Condição para exibição |
|---|---|---|
| ROM/mídia | Carregada, rejeitada, formato inválido, incompatível, identificação incompleta | Somente após resultado real do loader/validador |
| Sistema/região | Sistema e variante; região selecionada e região efetivamente aplicada | Não confundir preferência selecionada com configuração ativa da máquina |
| Controles | Dispositivo conectado/desconectado, P1/P2 atribuído/liberado, dispositivo substituído | Eventos reais de hotplug e confirmação do mapeamento |
| Save State | Salvando, salvo, falha, carregando, carregado, falha, slot selecionado | Slot/arquivo só quando o serviço de persistência e slots existir; sucesso após confirmação |
| Sessão | Reset, pausa, retomada, step-frame, Stop/POWER OFF, ROM descarregada | Mensagens semanticamente distintas; evitar notificações redundantes |
| Áudio | Inicialização, dispositivo indisponível/desconectado, recuperação | Somente quando o estado do endpoint for conhecido |
| Desempenho | FPS de origem e FPS de apresentação | Métricas distintas, com definição e janela de amostragem explícitas |
| Futuro | Achievements, progresso, leaderboard e Rich Presence | Consumidor futuro via adaptador de eventos; sem acoplamento ao OSD |

### Indicadores persistentes

- Estado da sessão e identificação do sistema/jogo, quando disponíveis.
- Região efetivamente aplicada.
- FPS de origem do emulador e FPS de apresentação/renderização, identificados separadamente.
- Slot ativo, somente quando existir serviço real de slots.
- Estado resumido de controles/áudio apenas quando sustentado por dados reais.

### Preferências de apresentação

- Quatro âncoras (superior esquerdo/direito e inferior esquerdo/direito), margem e escala/tamanho.
- Visibilidade independente de indicadores persistentes e mensagens temporárias.
- Duração por categoria, com opção de manter avisos críticos até dispensar.
- Limite de mensagens simultâneas, prioridade, deduplicação, agrupamento e política explícita de overflow.
- Ícones/textos acessíveis; informação não pode depender exclusivamente de cor.
- Overlay não captura input do jogo nem altera foco, pausa, fullscreen ou proporção.

### Arquitetura obrigatória

1. Runtime/serviços emitem eventos tipados que descrevem fatos; não enviam strings de UI nem inferem sucesso a partir do clique.
2. O serviço OSD no host transforma eventos em mensagens com IDs estáveis, parâmetros, prioridade, categoria, TTL e política de deduplicação.
3. Slint compõe a camada visual sobre o viewport de vídeo. Primeiro auditar a composição atual e provar que a solução não cria segundo renderer/device/queue, não duplica o frame e não altera a proporção.
4. OSD consome eventos de lifecycle e snapshots imutáveis/coarse-grained; nenhum callback por instrução/ciclo.
5. Sem I/O, serialização, logging persistente, bloqueios, espera ou alocação ilimitada no Scheduler/worker de emulação. A exibição operacional não habilita coleta diagnóstica DP-0.
6. Mensagens de conclusão só são emitidas após confirmação do serviço; falha, cancelamento e sucesso são estados distintos.

### Fatiamento e gates

- **OSD-0 — auditoria:** mapear composição Slint/WGPU, eventos existentes, contratos e lacunas; especificar catálogo e modelo de mensagem.
- **OSD-1 — overlay visual:** posicionamento, escala, visibilidade, TTL e fila limitada; testes unitários e UI sem depender do runtime.
- **OSD-2 — sessão/mídia:** ROM, região efetivamente aplicada, reset, pausa, retomada, Stop/POWER OFF e descarregamento.
- **OSD-3 — dispositivos:** hotplug SDL3, atribuição P1/P2 e estados de áudio suportados.
- **OSD-4 — Save State:** slots e mensagens de save/load somente após existir serviço persistente validado; não simular funcionalidade.
- **OSD-5 — indicadores:** FPS de origem/apresentação e estado da sessão.
- **OSD-6 — aceite:** resize, fullscreen, escala, acessibilidade, flood/duplicação, overflow e orçamento de desempenho.

Critérios de aceite: overlay visível em janela, maximizada e fullscreen; sem mudança do frame ou proporção; mensagens corretas após confirmação; hotplug sem flood; TTL/prioridade/deduplicação/overflow testados; FPS corretamente rotulados; nenhuma interferência no Scheduler/áudio/input; nenhum sink diagnóstico habilitado; evidências vinculadas ao SHA. `SKIPPED` e `NOT RUN` não são PASS.

**Fora de escopo:** implementação do serviço persistente de Save State/slots, debugger avançado e integração RetroAchievements. A HUD apenas define o ponto futuro de integração para esses eventos.

## 10. DP-5 — diagnóstico básico, não debugger avançado

O escopo inicial é diagnóstico de desenvolvimento e operação, ativado somente quando solicitado explicitamente.

Consultas coarse-grained permitidas, quando implementadas:

- identificação de sistema, variante e ROM carregada;
- estado de ciclo de vida da sessão;
- estado/resumo do Scheduler sem trace por ciclo;
- snapshot imutável básico da CPU;
- estado de publicação de frames e parâmetros de vídeo;
- estado do PSG/YM2413 e saída de áudio, sem dump contínuo;
- estado de interrupções e input;
- erros de inicialização e disponibilidade dos subsistemas.

A coleta deve ser pontual ou limitada no tempo, com botão explícito de iniciar/parar e indicação visível. Dados são apresentados na UI; exportação/gravação exige ação e consentimento específicos.

Fora do escopo inicial: debugger visual completo, execução passo a passo por instrução, disassembler interativo, memory viewer/search, watchpoints, trace contínuo de barramento, inspeção de cada operador e edição de memória. Esses itens não são prioridade e não bloqueiam o roadmap de hardware.

## 11. DP-6 — CLI e serviços comuns

A CLI deve usar os mesmos serviços de aplicação e o mesmo registro de capacidades da GUI. Não criar uma segunda implementação de carregamento ou runtime.

Primeiro escopo:

- `--help` e `--version`;
- listar sistemas e capacidades disponíveis;
- inspecionar e validar uma ROM sem iniciar emulação;
- listar/validar firmware quando suportado;
- mostrar/validar configuração;
- consultar status de áudio/input/vídeo;
- consultar estado do diagnóstico e consentimento sem habilitar coleta.

Comandos que iniciem emulação headless, gravem estados, capturem traces ou exportem dados só podem ser adicionados quando houver contrato funcional, política de autorização e testes. `--help` deve descrever somente comandos realmente implementados. Não assumir que o launcher GUI já oferece uma CLI completa.

## 12. DP-7 — validação e critérios de conclusão

R5 só pode ser declarado concluído quando:

1. Todos os menus, ações e opções têm entrada no inventário de capacidades.
2. Nenhuma opção sem suporte é anunciada como funcional.
3. Preferências globais, por sistema e por operação são persistidas e validadas.
4. Restaurar padrões por seção e global funciona sem apagar dados do usuário.
5. Janela recupera geometria inválida e permite restaurar layout.
6. Diretórios padrão por sistema e últimas pastas por finalidade funcionam independentemente.
7. Filtros de arquivos são derivados da matriz de loaders reais.
8. Testes cobrem formatos válidos, inválidos, ambíguos, compactados e limites de segurança.
9. Não há logs, traces ou telemetria registrados sem consentimento explícito em Debug ou Release.
10. Revogação interrompe a coleta; limites de retenção e overhead são medidos.
11. Diagnóstico básico não bloqueia nem altera a temporização do runtime.
12. CLI e documentação correspondem ao registro de capacidades.
13. Testes de UI cobrem navegação, janela normal/maximizada/fullscreen, restauração e falhas de persistência.
14. Cada resultado é vinculado ao SHA validado; `SKIPPED` e `NOT RUN` não são PASS.

## 13. Situação inicial conhecida e lacunas

A documentação atual do desktop registra: Slint/WGPU/Vulkan, carregamento SMS por diálogo Windows, ZIP com um único candidato, histórico de até dez ROMs, preferências limitadas de apresentação, CPAL integrado em software, primeira integração SDL3, e captura limitada de snapshot/trace de Scheduler/bus.

Também registra como pendentes: enumeração/seleção e aceite físico prolongado de áudio; perfis persistentes e remapeamento visual de controles; opções de HUD/overlay; save states persistentes em arquivos/slots; consolidação de biblioteca e configurações. A existência de APIs in-memory de save/load state não significa que haja gerenciamento de arquivos de save state na GUI.

Este resumo é ponto de partida da auditoria, não substitui o inventário DP-0. O estado deve ser revalidado contra a main e os testes no momento de cada implementação.

## 14. Referências internas

- [Roadmap mestre](project-roadmap.md)
- [Escopo do projeto](project-scope.md)
- [Status dos componentes](component-status.md)
- [README do desktop Slint](../native/sega_desktop/README.md)
- [Inventário de migração da aplicação Rust](architecture/rust-application-migration-inventory.md)
- [Política de CI e validação](ci-validation-policy.md)
