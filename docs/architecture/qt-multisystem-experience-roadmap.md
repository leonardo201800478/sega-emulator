# Plano: evolução da experiência Qt multissistema

- **Estado:** planejamento inicial; implementação ainda não iniciada.
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
