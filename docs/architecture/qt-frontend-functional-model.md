# Modelo funcional do frontend Qt

**Estado:** DESIGN / MODELING — especificação inicial, sem implementação nesta proposta.  
**Escopo:** interface desktop Qt/PySide6 do Sega Emulator, inicialmente SG-1000 e Master System (SMS).  
**Runtime:** Rust permanece como único runtime de hardware. Python/PySide6 cuida da interface, preferências, carregamento de mídia e orquestração pela fronteira PyO3.

## 1. Objetivo e princípios
Este documento registra as decisões funcionais aprovadas para a interface Qt. É uma especificação de produto e arquitetura, não uma declaração de que todos os comportamentos já estejam implementados.

- Preservar a implementação funcional existente e evoluir incrementalmente.
- Não duplicar serviços, diálogos, configurações ou diagnósticos.
- Não implementar comportamento de hardware em Python.
- Não declarar suporte sem comprovação ponta a ponta em Rust, PyO3 e Qt.
- Separar preferências persistentes do aplicativo do estado volátil da máquina.
- SG-1000 e SMS terão identidade visual contextual própria, mantendo a aplicação como desktop moderno.
- Menus, toolbar e atalhos compartilham semântica e fonte de estado.

## 2. Estrutura da janela
Manter janela Qt convencional: barra de menus, barra de ferramentas logo abaixo, área central de apresentação/páginas existentes e barra de status inferior. A estrutura é consistente entre sistemas; tema, acentos e elementos visuais variam conforme SG-1000 ou SMS.

### 2.1 Barra de ferramentas
Manter ícones grandes, coloridos e acompanhados de rótulos, seguindo a direção visual aprovada na primeira proposta gráfica. As ações refletem o estado real da sessão e reutilizam as mesmas operações dos menus.

### 2.2 Barra de status
Manter a barra atual e a direção visual da primeira proposta gráfica. Não remover nem substituir informações existentes durante o refinamento. FPS de apresentação não é frequência física do monitor nem clock do console.

### 2.3 Tela inicial sem ROM
A tela inicial será preta, sem exigir grade de jogos. O usuário poderá abrir ROM ou solicitar boot pela BIOS, quando houver firmware válido e compatível com sistema e região. Boot pela BIOS sem cartucho é diferente de carregar uma ROM.

Se BIOS não estiver disponível ou for incompatível, informar a condição sem simular boot bem-sucedido. Ausência de BIOS não deve impedir boot direto por ROM quando suportado.

O frontend Grid é opcional em Opções. Desativar o Grid não desativa a janela Qt nem impede abertura convencional de ROMs.

## 3. Menus

### 3.1 Arquivo
- Abrir ROM: seletor filtrado pelo sistema selecionado; validar arquivo antes do carregamento.
- Lembrar último diretório separadamente por sistema.
- Histórico global das últimas 10 ROMs, mais recentes primeiro, identificando nome, caminho e sistema; separar visualmente sistemas diferentes.
- Reabrir última ROM, remover item individual e limpar histórico completo (com confirmação).
- Arquivo movido, removido ou inacessível é identificado como indisponível; nunca tentar carregar silenciosamente.
- Fechar ROM descarrega ROM e encerra sessão, mantendo Qt aberta.
- Sair encerra a aplicação.
- Fechar ROM não apaga arquivo físico; remover histórico não afeta biblioteca nem arquivo. Histórico persiste entre execuções.

### 3.2 Emulação
| Comando | Semântica |
|---|---|
| Play | Inicia ou retoma execução. |
| Pause | Suspende execução, mantendo sessão em memória. |
| Stop | Encerra a emulação, persiste dados pendentes e descarrega a ROM; mantém a aplicação Qt aberta e retorna à tela inicial do sistema selecionado. |
| Reiniciar emulador | Reinicializa o mecanismo de emulação. Escopo interno será definido na arquitetura do runtime; não equivale ao reset físico. |
| Reset do hardware | Simula o botão RESET do console real, respeitando sequência e efeitos específicos do hardware. Mantém ROM/sessão e não apaga dados persistentes. |
| Velocidade | Ajusta velocidade sem alterar clocks nominais do hardware. |

Escalas aprovadas (F05=B): 0,5x, 1x, 2x e ilimitada. Fast Forward (F06=A) executa o mais rápido possível, sem sincronização com o monitor. Distinguir escala temporal do host dos clocks lógicos do console. Distinguir escala temporal do host dos clocks lógicos do console.

Reset de hardware não reinicializa indiscriminadamente todos os componentes. Cada plataforma implementará comportamento sustentado por documentação/evidência. Reiniciar emulador permanece distinto; seu contrato interno está pendente.

### 3.3 Ajuda
Menu simples com Página do projeto no GitHub, abrindo https://github.com/leonardo201800478/sega-emulator no navegador padrão. Documentação, código, issues e PRs são acessados pelo repositório. Identificação e versão ficam em Sobre.

### 3.4 Opções
Organizar em quatro áreas:

**Interface**
- Habilitar/desabilitar frontend Grid.
- Tema claro/escuro e idioma.
- OSD ligado/desligado e duração das notificações.

**Firmwares e bibliotecas**
- Uma pasta global de firmwares para todos os sistemas.
- Auditoria de firmware por hash/conteúdo, sem confiar somente no nome.
- Diretório ROM/ISO individual por sistema.
- Incluir subdiretórios na varredura, opcionalmente.
- Auditoria da biblioteca reutilizando funcionalidade existente/em revisão, sem duplicação.
- Teste de acessibilidade/permissões dos diretórios.
- Quantidade de ROMs por sistema, revarredura manual e espaço ocupado por ROMs/firmwares.

**Drivers e APIs**
- Selecionar APIs de vídeo, áudio e entrada.
- Exibir apenas recursos compatíveis com API/dispositivo disponível e explicar indisponibilidades.
- Distinguir API selecionada da API/backend efetivamente ativo.
- Testar API quando aplicável; exibir dados técnicos dos dispositivos/backends.
- Se nova configuração falhar, restaurar configuração funcional anterior.
- Avisar quando reinicialização for necessária.
- Preservar configurações de APIs alternativas.

**Manutenção**
- Restaurar todas as configurações para Default, com confirmação.
- Restaurar somente uma categoria, com confirmação.
- Buscar atualizações do emulador.

Opções gerais não duplicam configurações específicas dos consoles.

### 3.5 Vídeo
As opções dependem da API efetiva e das capacidades reais do backend e monitor. Não presumir que uma API oferece todos os modos do dispositivo.

- Filtro: Bilinear, Nearest e None.
- Shader como camada adicional ao filtro, com packs específicos por API; implementação futura.
- VSync On/Off/Adaptativo quando suportado; VRR On/Off somente quando disponível e confirmado.
- Seleção de monitor; enumerar monitores conectados e identificar o principal.
- Detectar hotplug e atualizar lista.
- Consultar resoluções e frequências por monitor. Modos são combinações resolução + Hz, não listas fixas.
- Exibir frequência efetiva/ativa e consultar VRR quando a API permitir.
- Exclusive Fullscreen, janela sem bordas e janela normal respeitam monitor selecionado.
- Modo janela com escala até 5x.
- Formato: 1:1 (pixel perfect), 4:3, 16:9 e proporção original do console.
- Integer Scaling, overscan/área visível, posicionamento/enquadramento, escala personalizada e resolução de saída.
- Informações: resolução interna do VDP, resolução de apresentação/saída, FPS de apresentação e backend ativo.
- Captura de tela: formato e diretório.
- Restaurar somente configurações de vídeo.

Pixel-perfect e proporção são controles distintos. Separar resolução nativa do VDP, resolução interna renderizada, saída do host, escala e modo do monitor. Não alterar clocks do console para acompanhar Hz do monitor. FPS apresentado não é frequência do monitor.

### 3.6 Som
- Enumerar dispositivos de saída, incluir Padrão do Windows, permitir seleção, mostrar dispositivo ativo e atualizar em hotplug.
- Consultar capacidades reais: frequências, profundidade de bits, canais, formatos PCM, modos compartilhado/exclusivo e buffer, quando expostos pela API.
- Distinguir frequência solicitada pelo emulador, aceita pelo backend e efetiva na saída.
- Indicar resampling/conversão quando ocorrer.
- Volume, balanço, amostras, buffer e latência.
- Perfis Alta fidelidade, Boa performance e Personalizado. Afetam saída host, não fidelidade do PSG Rust.
- Exibir latência em ms, underruns e falhas; oferecer teste de saída.
- Exibir frequência efetiva e configurar sincronização A/V.
- Restaurar somente configurações de som.

Distinguir frequência interna do PSG da frequência de saída host. Não alterar clocks internos para sincronizar com host. Métricas vêm do backend, não de estimativas.

### 3.7 Controles
Separar dispositivos físicos, periféricos lógicos e mapeamentos.

1. Tela inicial lista dispositivos físicos reconhecidos pela API: teclado, gamepad, joystick, volante, mouse e outros HID conforme suporte. Mostrar nome, tipo, estado e API; atualizar em hotplug.
2. Usuário seleciona periférico lógico do catálogo contextual SG-1000 ou SMS.
3. Abrir tela separada para mapear entradas físicas aos sinais do periférico lógico.

O mapeador oferece captura, limpeza, restauração de padrão, teste em tempo real, múltiplos dispositivos físicos por periférico lógico, perfis independentes, importar/exportar, deadzone, sensibilidade, inversão X/Y, teste visual, turbo/repetição quando aplicável e configuração individual dos jogadores 1 e 2.

Preservar perfis após desconexão e reconhecer reconexão. Light Phaser tem mira e gatilho próprios, não é gamepad convencional. Não anunciar periféricos sem suporte comprovado ponta a ponta.

Hotkeys ficam separadas dos perfis: Save State save/load, pause/resume, restart, fechar ROM, sair, screenshot, fullscreen, configurações e volume. São editáveis, detectam conflitos e restauram defaults. Evitar comandos duplicados se a mesma tecla estiver em controle e hotkey.

### 3.8 Região
Manter seletor de região atual. Cada sistema oferece somente opções regionais pertinentes e suportadas; não impor lista universal. Região/território, padrão de vídeo PAL/NTSC/PAL-M, perfil de hardware e família/identidade da BIOS são conceitos distintos.

### 3.9 Saves
- Diretório de saves nativos (SRAM e memórias equivalentes) separado do diretório Save States.
- Diretório Save States é módulo futuro.
- Até 10 slots numerados.
- Carregar/salvar em slot somente com jogo em andamento.
- Slot vazio não carrega; sobrescrever slot ocupado exige confirmação.
- Sem ROM, comandos de Save State desabilitados.
- Estado identifica sistema, versão do emulador e identidade da ROM; não carregar em ROM incompatível.
- Escrita robusta contra interrupção/corrupção.
- SRAM é persistência do jogo; Save State é snapshot do estado interno da máquina.

### 3.10 Sobre
Exibir nome Sega Emulator, versão obtida automaticamente dos metadados oficiais do build e identificação do projeto. Manter simples.

## 4. Seletor de sistema
Disponível somente quando o emulador estiver encerrado, sem ROM e sem sessão ativa.
- Durante execução ou pausa: desabilitado.
- Após Stop: habilitado.
- Selecionar SG-1000 ou SMS atualiza contexto, tema, biblioteca, catálogo de periféricos, opções e região.
- Não troca ROM nem inicia sessão automaticamente.

## 5. Confirmações e segurança
| Operação | Confirmação |
|---|---|
| Fechar ROM com execução ativa | Sim |
| Sair com ROM em execução ou pausada | Sim |
| Stop (ação explícita) | Não |
| Reiniciar emulador | Não |
| Reset do hardware | Não |
| Sobrescrever Save State | Sim |
| Restaurar todas as configurações | Sim |
| Restaurar uma categoria | Sim |
| Limpar histórico | Sim |
| Remover item individual do histórico | Não |
| Trocar sistema com sessão encerrada | Não |

Antes de descarregar ROM ou sair, persistir dados nativos pendentes, como SRAM. Em caso de falha, informar claramente; nunca afirmar que salvou sem confirmação da gravação. Save States devem ser escritos de forma robusta. Confirmação de sobrescrita identifica slot/arquivo.

Ao sair com ROM ativa, persistir automaticamente dados nativos antes de encerrar; avisar em caso de falha. Não sugerir que o estado interno da máquina será salvo.

## 6. Preferências versus estado da emulação
Salvar automaticamente somente configurações e preferências do aplicativo.

**Persistir:** último sistema selecionado; região preferida por sistema; configurações de vídeo, áudio e controles; APIs e dispositivos selecionados; tema, idioma e preferências da interface; diretórios ROM/firmware; histórico das últimas ROMs e caminhos.

**Não persistir como sessão:** registradores/estado da CPU e memória volátil; estado do VDP, PSG e scheduler; execução/pausa ou posição do jogo; estado interno de periféricos; snapshot automático da máquina.

Ao reabrir, restaurar preferências, sistema e região, apresentar tela inicial preta e aguardar ação explícita. Não carregar nem executar automaticamente ROM anterior. A última ROM permanece no histórico. Não criar retomada implícita.

Esta regra não impede SRAM nem Save States explícitos acionados pelo usuário; são funcionalidades distintas das preferências.

## 7. Diagnóstico geral
Manter a tela Debug atual como central de diagnóstico. Não criar tela concorrente nem duplicar recursos. Evoluções ampliam a tela existente com dados confiáveis e seletivos: versão/build, sistema/ROM, componentes, APIs/backends ativos, dispositivos, métricas, sincronização e erros. Logs não despejam conteúdo de ROM/BIOS; traces com bytes de instrução/fragmentos permanecem locais e fora do Git.

## 8. Estados e habilitação
Menus, toolbar e atalhos refletem uma fonte comum de estado:
- Sem ROM: execução, pausa, Stop e Save State indisponíveis; abrir ROM e boot BIOS disponíveis conforme suporte.
- ROM carregada/pronta: Play, Stop, reset e ações aplicáveis disponíveis.
- Em execução: Pause, Stop e resets disponíveis.
- Pausado: Play/retomar, Stop e resets disponíveis.
- Seletor de sistema: somente com sessão encerrada e ROM descarregada.
- Opções de áudio/vídeo dependem da disponibilidade e estado real do backend.

A UI não é fonte da verdade sobre hardware; aplicação/runtime reportam estado efetivo.

## 9. Fronteiras arquiteturais
- Rust: máquina, CPUs, barramento, memória, VDP, PSG, periféricos, interrupções, reset e temporização.
- PyO3: fronteira fina e tipada; sem callbacks Python por instrução, cópias grandes no caminho crítico ou ownership/sincronização implícitos.
- Python/Qt: menus, janelas, diálogos, preferências, descoberta de dispositivos host, apresentação, biblioteca e orquestração.
- APIs host: descobrir capacidades/dispositivos na fronteira do SO; nunca consultar host a cada instrução.
- Recursos disponíveis exigem suporte ponta a ponta e API efetiva.
- Troca de API preserva configurações alternativas e permite rollback.
- Varreduras/auditorias/testes demorados não bloqueiam thread de UI.
- Mudanças de apresentação não alteram clocks ou estados lógicos da máquina.

## 10. Inventário inicial observado no código remoto
Inspeção pontual da main encontrou src/sega_emulator/frontend/qt/main_window.py com:
- EmulatorMainWindow(QMainWindow);
- superfície VideoWidget;
- abas centrais Emulador e Debug;
- Debug com ativação de logging, saída textual e limpeza;
- QStatusBar com rótulo de ROM;
- callbacks para abrir ROM, pause/resume, reset, controles, dispositivos, snapshot VDP, logs, estado da máquina e escala de vídeo;
- callbacks opcionais para troca de sistema, drivers, região, áudio/vídeo e gravação VGM.

O README descreve frontend Qt/PySide6 com menus, barra de ações, ROMs recentes, seleção de drivers, configurações e diagnóstico. Este inventário é pontual, não auditoria completa. Antes de implementar, reconciliar cada requisito com módulos, serviços, callbacks, settings e testes existentes.

## 11. Fases de modelagem e implementação
1. Inventário/reconciliação de menus, ações, callbacks, diálogos, serviços, settings, testes e fluxos existentes.
2. Contrato de estado da aplicação: sem ROM, carregada/pronta, execução, pausa e encerrada; habilitação derivada de fonte única.
3. Preferências: schema, defaults, migração, escopo global/por sistema/periférico e gravação atômica.
4. Shell da janela: menus, toolbar, área central, Debug e status bar preservando comportamentos.
5. Mídia e boot: abertura, validação, histórico, fechar, Stop e boot BIOS.
6. Vídeo/áudio: descoberta de capacidades, seleção, teste, backend efetivo, rollback e hotplug.
7. Controles: catálogo por sistema, descoberta host, mapeamento e perfis.
8. Segurança e saves: persistência SRAM, confirmações e Save States quando módulo estiver pronto.
9. Diagnóstico e acabamento: integrar dados técnicos existentes e validar UX/estados de erro.

Cada fase requer critérios de aceitação e testes direcionados. Widgets desenhados não significam funcionalidade concluída.

## 12. Critérios de aceitação
- Decisões aprovadas registradas sem contradizer contratos existentes.
- Distinguir comportamento definido, futuro e inventário observado.
- Nenhum suporte de hardware declarado apenas pela presença de controle na UI.
- Preferências persistem; estado interno da emulação não.
- Troca de sistema bloqueada durante ROM/sessão.
- Stop encerra a emulação e descarrega a ROM, sem fechar a aplicação Qt; Pause mantém ROM carregada.
- Reset físico específico por plataforma.
- Região contextual e distinta de padrão de vídeo/BIOS.
- Grid opcional; tela sem ROM preta.
- Debug atual reutilizado.
- Status bar atual preservada e refinada visualmente, não substituída.
- Menus, toolbar e atalhos compartilham semântica/estado.
- Testes futuros executados no SHA candidato e classificados pelos estados do projeto.

## 13. Pendências explícitas
- Contrato interno preciso de Reiniciar emulador, distinto do reset físico.
- Confirmar no inventário se Stop e Arquivo → Fechar ROM já encaminham à mesma operação central; a semântica funcional aprovada para ambos é encerrar a emulação e descarregar a ROM, mantendo a aplicação aberta.
- Semântica exata de velocidades e pacing/fast-forward por backend.
- Schema/versionamento das preferências e migração.
- Inventário completo do que já está implementado versus planejado.
- Contratos de descoberta de monitores, modos, VRR, áudio e input por backend.
- Detalhes de Save State, módulo futuro.
- Boot sem cartucho por BIOS específico por sistema/região, condicionado à capacidade real do runtime.

**Classificação:** DESIGN / MODELING. Não representa implementação Qt, validação funcional, teste visual, build ou aprovação de gameplay.


## 14. Registro de decisões funcionais — 2026-10-30

As escolhas abaixo foram fornecidas explicitamente pelo usuário. Elas substituem as alternativas anteriores correspondentes neste documento. Não representam implementação nem validação técnica.

| Flag | Decisão | Consequência funcional registrada |
|---|---:|---|
| F01 | C | Stop encerra a emulação e descarrega a ROM, mas mantém a aplicação Qt aberta. Pause é que mantém a ROM carregada e suspensa, podendo ser retomada com Play. |
| F02 | B | Reiniciar emulador reinicia o runtime preservando a mídia carregada; contrato técnico ainda precisa ser definido. |
| F03 | A | Reset físico reinicia os componentes conforme o hardware, preservando a ROM/sessão. |
| F04 | A | Play inicia ou retoma; Pause suspende a execução. |
| F05 | B | Velocidades oferecidas: 0,5x, 1x, 2x e ilimitada. |
| F06 | A | Fast Forward executa o mais rápido possível, sem sincronização com o monitor. |
| F07 | C | Perfis de precisão, baixa latência e desempenho. |
| F08 | A | Histórico global limitado a 10 entradas, identificadas por sistema e caminho. |
| F09 | B | ROM removida/inacessível é removida automaticamente do histórico. |
| F10 | D | Tela inicial configurável. |
| F11 | C | Troca de sistema pergunta antes de encerrar a sessão atual. |
| F12 | A | Boot pela BIOS sem ROM permitido quando houver BIOS válida e suporte implementado. |
| F13 | C | BIOS automática por padrão, com seleção manual avançada. |
| F14 | C | Boot sem BIOS permitido quando tecnicamente suportado. |
| F15 | C | Monitor automático por padrão, com seleção manual e hotplug. |
| F16 | C | Resolução/frequência automáticas por padrão, com controles avançados. |
| F17 | C | VRR com detecção, configuração e relatório do estado efetivo. |
| F18 | C | API gráfica automática por padrão, seleção manual avançada e exibição do backend ativo. |
| F19 | A | Manter proporção original e integer scaling; não incluir os modos adicionais de esticamento e ajuste manual nesta decisão. |
| F20 | C | Áudio automático por padrão, seleção manual e hotplug. |
| F21 | C | Descobrir capacidades, permitir configuração manual e informar valores efetivos. |
| F22 | B | Diagnóstico de áudio exibe buffer, latência estimada e underruns; não prometer latência ponta a ponta medida. |
| F23 | B | Separar dispositivo físico, periférico lógico e mapeamento. |
| F24 | C | Hotplug automático preservando perfil de mapeamento. |
| F25 | C | Modelar Light Phaser, Paddle e demais periféricos previstos por sistema, sem antecipar suporte não validado. |
| F26 | C | Catálogo de periféricos por sistema, respeitando limites do hardware. |
| F27 | A | SRAM salva automaticamente quando houver alterações e em encerramentos seguros. |
| F28 | A | Planejar 10 slots de Save State por jogo. Implementação continua condicionada à serialização determinística e compatibilidade de estado. |
| F29 | B | Identificação dos saves por hash, sistema, região e versão do emulador. |
| F30 | B | Separar região comercial, padrão de vídeo e perfil de hardware; revisão de hardware/BIOS não foi escolhida como dimensão obrigatória. |
| F31 | C | Troca de região permitida somente após Reset. |
| F32 | C | Migrar automaticamente as configurações atuais e registrar incompatibilidades para revisão. |
| F33 | C | Separar Diagnóstico e Debug técnico, com responsabilidades distintas. |
| F34 | C | Exibir backend solicitado, backend ativo, fallback, motivo de falha e capacidades detectadas. |
| F35 | C | Oferecer restauração global e restauração por categoria, ambas com confirmação. |

### 14.1 Regras de interpretação e reconciliação

- F01: Pause mantém ROM/sessão carregadas e suspende a execução; Stop e Arquivo → Fechar ROM encerram a sessão, persistem dados nativos pendentes e descarregam a mídia, mantendo a aplicação Qt aberta. A auditoria deve verificar se ambos encaminham ao mesmo comando central, sem duplicar lógica.
- F02 não pode ser implementada como destruição/recriação arbitrária do runtime sem contrato. Definir preservação de mídia, estado persistente, SRAM pendente, recursos host e falhas de reinicialização.
- F03 permanece dependente da semântica de reset específica de cada plataforma; não significa zerar indiscriminadamente todos os componentes.
- F05 e F06 substituem a lista anterior de velocidades e definem Fast Forward como execução máxima sem sincronização com monitor. O comportamento de áudio nesse modo precisa ser definido pelo perfil/contrato de pacing, sem alterar clocks nominais do hardware.
- F07 perfis de sincronização são políticas do host; não podem comprometer determinismo ou modificar temporização lógica do console.
- F09 remove automaticamente entradas indisponíveis do histórico; a remoção não apaga arquivos físicos nem conteúdo da biblioteca.
- F10 exige preferência configurável para a tela inicial. Os valores possíveis e o comportamento de primeira execução precisam ser detalhados no contrato de preferências.
- F11 permite solicitar troca de sistema com sessão ativa, mas exige confirmação e conclusão segura do encerramento/descarregamento antes da troca. Não permite troca implícita.
- F12 e F14 permitem boot BIOS-only e boot sem BIOS quando suportados. A UI deve consultar capacidade real por sistema/região e não inferir suporte apenas pela presença de firmware.
- F19 limita a decisão funcional a proporção original e integer scaling. A compatibilidade com opções de proporção já existentes precisa ser reconciliada no inventário, sem descartar configurações silenciosamente.
- F22 limita a telemetria aprovada a buffer, latência estimada e underruns. Não exibir estimativa como medição física.
- F28 define a experiência-alvo de 10 slots, mas não autoriza implementação antes da serialização determinística, identidade de estado e testes de compatibilidade.
- F30 não exige uma dimensão separada de revisão de hardware/BIOS na interface; manter distinções internas quando tecnicamente necessárias.
- F31 significa que a alteração regional é aplicada após Reset. O inventário deve confirmar se o reset atual pode reconstruir os componentes regionais de forma correta; se não, apresentar a operação como indisponível até haver suporte seguro.
- F32–F35 foram respondidas como C e estão registradas na tabela: migração automática com relatório de incompatibilidades; separação entre Diagnóstico e Debug técnico; relatório de backend com solicitado/ativo/fallback/motivo/capacidades; restauração global e por categoria, sempre confirmada.

### 14.2 Pendências técnicas que as escolhas não resolvem

As decisões funcionais não provam disponibilidade técnica. Permanecem necessárias: inventário completo dos callbacks e serviços; contratos de sessão/runtime/PyO3; capacidades reais de BIOS-only por sistema; descoberta host de monitores, modos, VRR e dispositivos; política de pacing e áudio em Fast Forward; migração do histórico e preferências; e critérios/testes para SRAM e Save States.

**Estado do registro:** 35/35 flags respondidas. Nenhum código executável foi alterado por este registro.


## 15. Correção de decisão — F01 (2026-10-30)

O usuário corrigiu a interpretação anterior de F01=C. A semântica aprovada passa a ser:

- **Pause:** interrompe temporariamente a execução, mantendo ROM e sessão carregadas em estado PAUSED; Play retoma.
- **Stop:** encerra a emulação, persiste dados nativos pendentes e descarrega a ROM; a aplicação Qt permanece aberta e retorna à tela inicial.
- **Arquivo → Fechar ROM:** deve produzir o mesmo resultado funcional de Stop. A auditoria deverá verificar se ambos reutilizam a mesma operação de aplicação, sem duplicar lógica.
- **Reset físico:** não descarrega ROM nem encerra a sessão.

A correção substitui todas as passagens anteriores que diziam que Stop preservava a ROM. O código ainda não foi alterado nem validado.
