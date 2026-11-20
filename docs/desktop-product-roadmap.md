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

**É proibido coletar, reter ou registrar diagnósticos sem autorização explícita do usuário.** A regra vale para Debug e Release e abrange:

- logs estruturados e saídas diagnósticas em console/terminal;
- traces de instruções, barramentos, Scheduler, vídeo ou áudio;
- snapshots de CPU/máquina e históricos de eventos;
- métricas de desempenho, frame pacing e saúde de áudio;
- relatórios e arquivos diagnósticos em background.

Mensagens operacionais necessárias para informar uma falha na própria interface não devem ser convertidas automaticamente em registros persistentes.

A única exceção automática é um **relatório mínimo de falha crítica de bootstrap** quando uma falha classificada impede a inicialização do emulador. Esse relatório é independente do logger e do consentimento, criado somente no caminho de falha, e contém apenas categoria estática e timestamp. Não pode conter payload de panic, backtrace, ROM/BIOS, memória, caminhos absolutos, identificadores pessoais ou conteúdo de sessão. Erros comuns de ROM, falhas de jogo após o boot e panics de runtime não geram relatório automático.

Requisitos do consentimento:

- uma única autorização global, apresentada no Debugger como Coleta de logs, cobre logs, métricas, snapshots, traces e históricos;
- a ação deve informar finalidade e escopo antes de iniciar;
- um controle secundário de trace pode restringir/ativar captura detalhada, mas só funciona enquanto a autorização global estiver ativa e nunca a substitui;
- desligado por padrão no início de cada sessão da aplicação; não persistir nem restaurar automaticamente;
- revogação imediata, sem reinício;
- build Release nunca implica consentimento;
- variável de ambiente, configuração herdada, abertura do Debugger ou carregamento de ROM nunca habilitam coleta.

Ao revogar, parar timers e publicação, desabilitar trace no worker, descartar eventos pendentes, fechar o sink, limpar filas, ring buffers, histórico do debugger e buffers de métricas do host. Uma nova autorização inicia uma sessão de coleta limpa. Não produzir snapshot JSON automático no encerramento; qualquer exportação exige ação explícita do usuário.

Quando desativado, o caminho de diagnóstico deve evitar formatação, alocação, serialização, I/O, amostragem e envio para filas. Todos os subsistemas devem usar o pipeline central de observabilidade; não manter sinks paralelos nem saídas diretas println!/eprintln! como mecanismo operacional de diagnóstico.

### 5.3 Impacto e armazenamento

Com consentimento ativo:

- um pipeline central de eventos estruturados, com níveis e targets filtráveis;
- captura em thread de background, com fila limitada e política explícita de descarte;
- nenhuma espera do Scheduler por consumidor, disco ou interface;
- limites configuráveis de tamanho por arquivo, quantidade de arquivos e quota total;
- rotação e retenção finitas;
- taxa limitada para eventos repetitivos;
- sem dumps integrais de ROM, RAM ou estados por padrão;
- sem trace por instrução/ciclo por padrão;
- indicador visível de coleta ativa e finalidade;
- exportação de trace/snapshot somente por ação explícita;
- encerramento imediato da coleta ao revogar consentimento.

A infraestrutura de observabilidade existente deve ser a única entrada de logs persistentes. Sua presença não prova que todos os produtores, buffers e snapshots respeitam consentimento nem que o overhead é desprezível.

Critérios de aceite: testes demonstram ausência de sinks/arquivos/amostras sem consentimento; revogação encerra trace e limpa buffers; limites e contadores de descarte são respeitados; fila cheia não bloqueia runtime; benchmark compara execução com observabilidade desligada e habilitada, registrando overhead no SHA avaliado.

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
