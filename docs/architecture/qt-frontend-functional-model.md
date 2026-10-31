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
| Stop | Interrompe, persiste dados pendentes e descarrega ROM; encerra sessão e retorna à tela inicial do sistema selecionado. |
| Reiniciar emulador | Reinicializa o mecanismo de emulação. Escopo interno será definido na arquitetura do runtime; não equivale ao reset físico. |
| Reset do hardware | Simula o botão RESET do console real, respeitando sequência e efeitos específicos do hardware. Mantém ROM/sessão e não apaga dados persistentes. |
| Velocidade | Ajusta velocidade sem alterar clocks nominais do hardware. |

Escalas inicialmente previstas: 0,25x, 0,50x, 0,75x, 1x, 1,25x, 1,5x, 2x, 4x e ilimitada/Fast Forward. Distinguir escala temporal do host dos clocks lógicos do console.

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
- Stop descarrega ROM; reset físico não.
- Reset físico específico por plataforma.
- Região contextual e distinta de padrão de vídeo/BIOS.
- Grid opcional; tela sem ROM preta.
- Debug atual reutilizado.
- Status bar atual preservada e refinada visualmente, não substituída.
- Menus, toolbar e atalhos compartilham semântica/estado.
- Testes futuros executados no SHA candidato e classificados pelos estados do projeto.

## 13. Pendências explícitas
- Contrato interno preciso de Reiniciar emulador, distinto do reset físico.
- Relação operacional entre Stop e Arquivo → Fechar ROM: mesma operação central em locais diferentes ou diferenças adicionais.
- Semântica exata de velocidades e pacing/fast-forward por backend.
- Schema/versionamento das preferências e migração.
- Inventário completo do que já está implementado versus planejado.
- Contratos de descoberta de monitores, modos, VRR, áudio e input por backend.
- Detalhes de Save State, módulo futuro.
- Boot sem cartucho por BIOS específico por sistema/região, condicionado à capacidade real do runtime.

**Classificação:** DESIGN / MODELING. Não representa implementação Qt, validação funcional, teste visual, build ou aprovação de gameplay.
