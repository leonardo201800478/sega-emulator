# Plano: evolução da experiência Qt multissistema

- **Estado:** implementação incremental em andamento; biblioteca inicial, seletor de provedor e catálogo de idiomas já estão no branch.
- **Branch de trabalho:** `feat/qt-multisystem-experience`
- **Base auditada:** `cb4de9350233c7e98b9d5a72befc57000de6ba28`
- **Escopo:** frontend Qt/PySide6, integração de entrada, apresentação de vídeo e contratos de seleção/configuração por sistema.
- **Regra de segurança:** não alterar `main` diretamente; não apresentar como funcional hardware que não esteja implementado e integrado.

## Objetivo

Evoluir a experiência do Sega Emulator mantendo Rust como único runtime de hardware/emulação. Python/Qt será responsável pela navegação, apresentação, configuração, descoberta de dispositivos e adaptação de entradas. PyO3 permanece uma fronteira fina e explícita.

A implementação deverá ser incremental, testável e baseada em contratos existentes, sem duplicar o runtime de hardware nem reescrever componentes estáveis apenas por preferência.

## Escopo funcional

### 1. Identidade visual e temas

- Criar uma splashscreen própria do Sega Emulator.
- Criar seis assets de pixel art independentes para SG-1000, Master System, Game Gear, Mega Drive/Genesis, Sega CD e 32X.
- Incorporar os três conceitos visuais aprovados: Retro Neon, Retro Moderno e 16-bit Clássico.
- Disponibilizar modo claro e escuro para cada tema.
- Persistir tema e modo de aparência via configurações existentes.
- Centralizar tokens de cores, tipografia, bordas, estados e ícones; manter consistência entre splashscreen, seletor de sistemas, janela de emulação e diálogos.
- Não reutilizar logos, personagens ou assets de terceiros sem verificar direitos, procedência e licença.

### 2. Navegação e disponibilidade dos sistemas

Fluxo esperado: splashscreen -> seletor de sistema -> ambiente do emulador.

O seletor deve listar:
- SG-1000
- Master System
- Game Gear
- Mega Drive/Genesis
- Sega CD
- 32X

A lista visual não representa suporte técnico. Cada sistema deverá expor um estado explícito, por exemplo: disponível, parcial/em integração ou planejado. A abertura da tela de emulação só será habilitada quando houver núcleo, carregamento de ROM, entrada, vídeo e contratos suficientes para a experiência correspondente. Preservar o fluxo atual do SG-1000 durante a migração.

### 3. Preferências por sistema

- Separar preferências globais de preferências específicas por sistema.
- Isolar mapeamento de controles, região, escala/aspecto, áudio e opções específicas de hardware.
- Evitar vazamento de configurações de um console para outro.
- Migrar configurações existentes de forma compatível, com defaults seguros e recuperação de valores inválidos.
- Não exibir opções que não possam ser aplicadas pelo núcleo selecionado.

### 4. ROM, região e mapper

- Região padrão: Auto.
- A identificação de ROM deve separar sistema/formato, região declarada ou inferida, compatibilidade multirregional e confiança da identificação.
- Para ROM multirregional, usar a ordem solicitada: USA -> Europe -> Japan -> World, salvo decisão técnica documentada baseada no formato específico.
- A seleção manual de região sobrepõe Auto.
- **Mapper e região são conceitos diferentes:** o mapper define o mapeamento de memória/cartridge; não deve ser usado como sinônimo de detecção regional.
- Registrar incerteza quando os metadados ou evidências não permitirem identificar a região com segurança.
- Não inventar regras de cabeçalho/CRC; documentar formatos e fontes por sistema.

### 5. Perfis regionais e temporização

- Modelar os perfis de região por sistema e revisão de hardware, não por uma regra global PAL/NTSC.
- Documentar clock da CPU e do vídeo, cadência de quadros/linhas, área visível e sincronização audiovisual somente com fontes e evidências adequadas.
- Distinguir fatos documentados, observações, inferências e hipóteses.
- Aplicar mudanças de perfil no núcleo Rust por contrato explícito; a Qt não deve alterar clocks ou framebuffer emulados por conta própria.
- A mudança de região deve ter comportamento definido durante a execução (por exemplo, exigir reset quando necessário), sem corromper o estado da máquina.
- Não declarar precisão ciclo a ciclo com base apenas em ciclos totais por instrução.

### 6. Vídeo e apresentação

Disponibilizar em todos os sistemas suportados, conforme as capacidades reais do backend:
- Pixel Perfect 1:1: preservar correspondência entre pixels do framebuffer e pixels físicos quando possível.
- 4:3: preservar a proporção de apresentação, sem distorção.
- Fullscreen: modo de exibição separado da política de aspecto/escala; preservar aspecto por padrão.
- Considerar integer scaling como opção distinta.
- Se houver esticamento, oferecê-lo como preferência explícita separada, não como efeito implícito do fullscreen.
- Selecionar APIs/backends com base na implementação atual, compatibilidade e testes. Fallbacks devem ser visíveis em diagnósticos.
- Nenhuma opção de apresentação deve modificar clocks ou dimensões lógicas do hardware emulado.

### 7. Gamepads e entrada Qt

- Auditar e reutilizar o backend de entrada já existente, sem duplicar polling/discovery na Qt.
- Expor dispositivos detectados, conectar/desconectar (hotplug), estado do backend e mensagens de erro.
- Permitir mapeamento de botões, D-pad, hats/eixos quando suportados, deadzone e perfis por jogador/sistema.
- Manter teclado como alternativa e oferecer restauração de defaults.
- Normalizar eventos na fronteira com o host e entregar comandos lógicos ao núcleo por contrato explícito.
- Evitar polling por instrução da CPU, callbacks Python por instrução e trabalho de I/O no caminho crítico.
- Validar dispositivos reais no Windows; CI headless não substitui testes físicos.

### 8. Pausa automática e foco

Defaults recomendados:
- Pausar ao abrir menus e diálogos da aplicação: ativado.
- Pausar ao perder o foco da janela no Windows: ativado.
- Pausar ao minimizar: ativado.
- Retomar manualmente após pausa automática: padrão.

Requisitos:
- Pausa por menu, perda de foco e minimização devem ser eventos identificáveis e configuráveis.
- Menus de contexto, diálogos nativos e janelas auxiliares precisam ser considerados.
- Fechar um menu ou recuperar foco não pode retomar silenciosamente a emulação quando a retomada manual estiver configurada.
- Pausa explícita do usuário deve prevalecer; eventos de foco não devem desfazê-la.
- A pausa deve coordenar scheduler/timers, progressão do hardware e áudio sem acumular quadros nem reproduzir comandos atrasados.
- Usar uma máquina de estados/causas de pausa explícitas para impedir que um evento de foco desfaça outra causa de pausa.

## Sequência de implementação

1. **Inventário e contratos:** confirmar branch/SHA, árvore atual, entry points, temas existentes, preferências, backend de entrada, frame pacer, áudio, fullscreen, perfil da máquina e integração real dos seis sistemas.
2. **Especificação e testes de aceitação:** definir capacidades por sistema, estados do seletor, contratos de preferências, política de pausa e critérios de regressão antes de alterar o fluxo.
3. **Fundação visual:** tokens de tema, persistência de tema claro/escuro e assets originais.
4. **Navegação:** splashscreen e seletor de sistemas, preservando o ambiente de emulação atual.
5. **Entrada e pausa:** gamepads via backend existente e máquina de estados de pausa.
6. **Preferências por sistema e região:** identificação de ROM/região separada do mapper; integrar perfis somente quando o núcleo os suportar.
7. **Vídeo:** Pixel Perfect, 4:3, fullscreen e fallback, sem alterar a lógica de hardware.
8. **Integração incremental:** habilitar cada sistema apenas após seus contratos e testes estarem presentes.

## Critérios de aceitação e validação

### Testes Qt/Python
- Testar criação da aplicação em modo offscreen, navegação e persistência de temas.
- Testar todos os estados de disponibilidade do seletor e impedir acesso a sistemas não integrados.
- Testar migração/isolamento de preferências por sistema.
- Testar detecção de dispositivos, mapeamento, hotplug e defaults com testes simulados; planejar validação com hardware real no Windows.
- Testar perda/restauração de foco, menus, diálogos, minimização, pausa explícita e retomada manual/automática.
- Testar que eventos repetidos de foco não retomem o jogo indevidamente.
- Testar proporções, fullscreen, integer scaling e fallback de backend.
- Executar `python -m pytest`, `python -m ruff check .`, `python -m ruff format --check .`, `python -m compileall` e `python -m pip check`, quando aplicáveis.
- Executar o workflow Qt aplicável no SHA candidato exato; checks ausentes/ignorados/bloqueados não contam como aprovação.

### Núcleo Rust e integração
- Executar `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` se mudanças alcançarem contratos ou crates Rust.
- Para alterações de gameplay/temporização, exigir testes pertinentes e evidência no emulador; marcar como pendente qualquer validação que dependa da máquina física.
- Classificar resultados como PASS, FAIL, BLOCKED ou NOT RUN. Não alegar precisão ciclo a ciclo sem evidência específica.

## Fora de escopo desta PR de planejamento

- Não implementar novos núcleos de Mega Drive, Sega CD ou 32X apenas para preencher o seletor.
- Não reescrever o núcleo Rust ou alterar semântica de hardware sem uma necessidade demonstrada.
- Não selecionar API gráfica ou biblioteca de gamepad antes do inventário das dependências e contratos existentes.
- Não incluir ROMs comerciais, BIOS protegidas, snapshots com dados de ROM ou logs locais no Git.

## Estado inicial

Esta PR registra o escopo e os critérios de aceitação. Ela não afirma que qualquer funcionalidade descrita já foi implementada ou testada. A implementação deverá ocorrer em commits pequenos nesta branch, com testes e evidências atualizados antes de solicitar revisão final.


## 9. Biblioteca de ROMs e scanner (requisitos confirmados)

Decisões confirmadas pelo usuário para a primeira experiência da biblioteca:

- **Entrada do aplicativo:** mostrar a splashscreen e depois o seletor de sistemas; não iniciar uma ROM automaticamente.
- **Navegação:** biblioteca de cada sistema em carrossel horizontal, com arte do jogo selecionado em destaque.
- **Scanner:** executar atualização incremental ao abrir a biblioteca e oferecer varredura completa manual.
- **Subpastas:** incluídas por padrão na descoberta recursiva.
- **Capas:** provedor externo opcional, configurável e desativável; a biblioteca deve continuar utilizável sem conexão ou provedor configurado.

### Diretórios por sistema

Cada sistema terá um ou mais diretórios ROM independentes e editáveis nas configurações. Sugerir diretórios iniciais separados (por exemplo, `ROMs/SG-1000/`, `ROMs/Master System/`, `ROMs/Game Gear/`, `ROMs/Mega Drive/`, `ROMs/Sega CD/` e `ROMs/32X/`), sem presumir que existam no computador. Não misturar resultados entre sistemas e não mover, renomear ou apagar arquivos de ROM.

### Contrato do scanner

- A atualização incremental identifica arquivos adicionados, removidos ou alterados e atualiza o índice local sem reler desnecessariamente toda a biblioteca.
- A varredura completa manual reconcilia o índice com os diretórios configurados e apresenta progresso, contagem e erros recuperáveis.
- A descoberta de arquivo não equivale a validação de ROM. Verificar formato/sistema apenas com regras implementadas e documentadas; indicar arquivos desconhecidos ou incompatíveis sem classificá-los falsamente como válidos.
- Duplicatas podem ser reportadas por hash de conteúdo, sem apagar arquivos.
- Metadados mínimos: caminho, tamanho, extensão, sistema atribuído e estado da última análise. Região/confiança só quando houver regra verificável para aquele formato.
- Erros de acesso, caminhos ausentes e arquivos ilegíveis devem ser visíveis e não interromper a varredura inteira quando for possível continuar.
- Guardar índice e cache em diretório de dados da aplicação, fora da árvore de ROMs; permitir reconstrução do índice.
- O scanner não deve bloquear a interface Qt. Executar trabalho de I/O em worker controlado, com cancelamento/encerramento seguro e atualizações de progresso na thread da interface.
- O provedor de capas é uma integração opcional separada: respeitar limites, licenças e termos do provedor, cache local e privacidade; nunca baixar ou enviar dados sem configuração/consentimento. A biblioteca não pode depender de capas remotas para abrir um jogo.
- Não incluir ROMs comerciais, BIOS, imagens de capas de terceiros sem licença, nem caminhos pessoais nos commits ou logs versionados.

### Fluxo da biblioteca e aceitação

1. Splashscreen -> seletor de sistemas -> biblioteca do sistema selecionado.
2. A biblioteca abre sem iniciar a emulação; uma seleção só inicia após ação explícita do usuário.
3. Carrossel navegável por teclado e gamepad, com foco visível, seleção estável e retorno consistente.
4. Abertura da biblioteca dispara atualização incremental; a varredura completa permanece uma ação manual explícita.
5. Usuário pode configurar múltiplos diretórios por sistema, habilitar/desabilitar caminhos e abrir o diretório no sistema operacional.
6. Testes cobrem subpastas, extensões desconhecidas, arquivos duplicados, remoção/alteração, caminhos inexistentes, permissões negadas, cancelamento, ausência de rede/provedor e biblioteca sem capas.
7. Scanner e provedor não carregam nem executam ROMs. O início do jogo usa o contrato existente de carregamento do núcleo Rust e fica desabilitado para sistemas sem integração funcional.
8. Validar persistência de diretórios e índice, atualização incremental idempotente e resposta da UI durante varreduras grandes; nenhuma validação é considerada executada até haver logs de teste no SHA candidato.

### Ordem de implementação atualizada

1. Inventariar os contratos existentes de descoberta/carregamento de ROM, preferências Qt, threads e integração Rust.
2. Definir modelo de dados e testes para diretórios por sistema e índice local.
3. Implementar scanner recursivo com atualização incremental, progresso, cancelamento e varredura completa manual.
4. Integrar splashscreen, seletor e carrossel sem iniciar automaticamente a emulação.
5. Acrescentar capas locais/importação e só depois um adaptador de provedor externo opcional.
6. Validar regressões Qt/Python e workflows no SHA exato; registrar limitações de sistemas ainda não integrados.


## 10. Implementação inicial em andamento

- **Implementado no branch (ainda aguardando CI):** índice local de ROMs por sistema em `src/sega_emulator/rom_library.py`, com descoberta recursiva, hashes SHA-256, detecção de duplicatas, atualização incremental, varredura completa, progresso, cancelamento e recuperação do índice.
- **Implementado no branch (ainda aguardando CI):** diálogo Qt de inicialização com splash textual provisória, seletor de sistemas com disponibilidade explícita e biblioteca em carrossel horizontal; a biblioteca é aberta antes do emulador quando não há ROM passada pela linha de comando.
- **Limite intencional:** apenas SG-1000 pode iniciar a emulação nesta tela. Os outros cinco sistemas aparecem desabilitados até haver integração funcional com o núcleo e o carregador.
- **Ainda não implementado:** arte final/splash em imagem, capas de jogos/provedor externo, retorno à biblioteca depois de fechar uma sessão, tema claro/escuro completo e testes de interação visual no Windows.
- **Validação:** os workflows de CI foram disparados no SHA candidato e seus resultados precisam ser conferidos antes de classificar esta fatia como aprovada. Não há PASS local declarado.


## Fatia implementada: preferências de scraper e idiomas

- O diálogo de configuração permite selecionar **ScreenScraper**, **TheGamesDB**, **IGDB** ou nenhum provedor.
- A seleção do provedor, a consulta automática opcional, o idioma da interface e o idioma preferido dos metadados são persistidos em `QSettings`.
- O fluxo inicial da biblioteca e o diálogo de scraper têm catálogo inicial em português (Brasil), inglês, espanhol, italiano e francês. A janela principal e os demais diálogos ainda precisam ser internacionalizados; portanto, não declarar a tradução completa do aplicativo.
- Os links apontam para os sites/documentação oficiais de cada serviço. O link do ScreenScraper usa o domínio oficial francês; não se inventa um parâmetro de idioma que a documentação não confirme.
- **ScreenScraper:** a documentação oficial informa que a integração de aplicativos exige credenciais de desenvolvedor emitidas após contato com a equipe; as credenciais do usuário (SSID/senha) são adicionais. A API documenta parâmetros `devid`, `devpassword`, `softname`, `ssid` e `sspassword`. Fontes: [WebAPI oficial](https://www.screenscraper.fr/webapi2.php) e [portal oficial](https://www.screenscraper.fr/).
- Os campos de credenciais são somente uma interface preliminar. Nesta fatia, segredos **não são enviados nem persistidos**. Antes da consulta real, implementar adaptadores, limites de taxa/cache, erros e armazenamento seguro via cofre de credenciais do sistema operacional. Não embutir segredos de desenvolvedor no repositório.
- TheGamesDB e IGDB também têm somente seleção e links nesta fatia; os adaptadores de API ainda não foram implementados. A opção de provedor não deve ser descrita como scraping remoto funcional até que isso seja concluído.
- O idioma selecionado passa a valer ao reabrir a biblioteca/aplicativo. Tradução completa da janela de emulação, configurações e mensagens de runtime é uma etapa pendente.

### Validação específica pendente

- Testes Qt offscreen para a lista de provedores, persistência de idioma/preferências e ausência de persistência de segredos.
- CI precisa executar os testes novos no SHA candidato. Ainda não há evidência de teste visual no Windows nem chamadas reais às APIs.
