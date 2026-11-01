# Escopo e decisões de arquitetura — periféricos e entrada

**Status:** decisão de escopo aprovada para orientar a arquitetura; implementação ainda não iniciada por este documento.  
**Base de referência:** Genesis Plus GX, consultado em 2026-10-31.  
**Runtime obrigatório:** Rust. Python permanece restrito ao frontend, configuração, ferramentas e testes.

Este documento registra as decisões do mantenedor para adaptar a cobertura de periféricos Sega às entradas modernas disponíveis no Windows. A compatibilidade do Genesis Plus GX é referência de comportamento/protocolo, não uma lista de funcionalidades que devam ser reproduzidas cegamente.

## 1. Princípios obrigatórios

1. Toda lógica de periférico emulado, protocolo, registradores, estado, temporização e interação com o scheduler pertence ao runtime Rust.
2. SDL3 e APIs nativas do Windows são fontes de eventos do host. Não são parte do hardware emulado.
3. O frontend normaliza e encaminha eventos de entrada em lotes/eventos com timestamp; não executa protocolos Sega.
4. O dispositivo emulado transforma eventos de host em sinais, leituras e transições de hardware. Não mapear diretamente controles modernos para valores de registradores sem passar pelo protocolo do periférico.
5. Recursos opcionais devem ser detectados em runtime. Não presumir que todos os controles expõem sensores, touchpad, force feedback ou microfone em todos os modos/conexões.
6. Não implementar periféricos descartados ou adaptadores multiplayer na primeira etapa.

## 2. Decisões por categoria

### 2.1 Joysticks e gamepads digitais — incluir

Considerar viáveis e prioritários:

- SG-1000 / Master System / Game Gear: controles digitais originais de 2 botões.
- Mega Drive: controles de 3 e 6 botões.
- Direcional, botões de ação, Start e demais sinais próprios do perfil.
- Hotplug, seleção de dispositivo, remapeamento e múltiplos controles no host.

**Botão Pause nos sistemas de 8 bits:** deve existir como ação de sistema separada do gamepad padrão. No Master System, o botão físico Pause do console gera uma borda de NMI no Z80, levando à rotina em 0x0066; não é um terceiro botão comum lido pela porta do joystick. No Game Gear, o botão Start é lido pela porta própria em modo GG, mas atua como Pause/NMI quando o aparelho opera em modo Master System. Portanto, o perfil de máquina deve decidir o destino correto do evento. Não inventar um Pause de console para SG-1000 sem evidência de hardware.

A interface poderá permitir mapear Pause para uma tecla ou botão de controle moderno, mas o evento resultante deve entrar na máquina Rust como entrada de sistema e respeitar borda, latch e semântica de NMI. Não pausar diretamente o loop do emulador: isso seria diferente do botão original.

### 2.2 Entradas analógicas e volantes — incluir

Os periféricos analógicos do GX devem ser adaptados a fontes modernas de eixos:

- Paddle Control.
- Sports Pad / trackball.
- XE-1AP.
- BH-400 / outros volantes e controles de direção, após confirmar o protocolo específico.

Fontes de host previstas:

- Sticks analógicos de gamepads.
- Eixos de volantes USB.
- Pedais e controles analógicos expostos pelo dispositivo.

**Hardware disponível para validação:** Logitech G923. Usá-lo para testar leitura de direção, pedais, faixa útil, centro, saturação, inversão, zona morta, curva e estabilidade. A leitura de eixo é requisito desta etapa; force feedback não está automaticamente incluído e deverá ser tratado como capacidade separada, somente se houver demanda e contrato técnico validado.

O perfil emulado define a conversão do eixo moderno para o intervalo/protocolo do periférico Sega. Não codificar calibração específica do G923 no core.

### 2.3 Pointers — incluir

Fontes de ponteiro:

- Mouse físico como fonte nativa.
- Stick analógico direito como ponteiro relativo ou absoluto configurável.
- Touchpad dos controles DualShock 4 e DualSense, quando exposto pelo driver.
- Eventos de toque e posição devem ser normalizados na fronteira do host.

O ponteiro alimentará dispositivos Sega apropriados, como Mouse, Graphic Board e tablet, conforme o protocolo de cada um. Um touchpad não deve ser tratado como se fosse automaticamente um mouse USB: manter distinção entre posição, delta, contato, pressão e clique.

**Hardware disponível para validação:** DualShock 4 e DualSense.

### 2.4 Lightguns — incluir com adaptação moderna

A Light Phaser, Menacer e Justifiers poderão ser operadas por fontes modernas:

- Mouse/cursor.
- Stick analógico (mira).
- Giroscópio de controles compatíveis.

**Hardware disponível para validação:**

- DualSense.
- DualShock 4.
- 8BitDo Ultimate 2, especialmente em modo Switch, cujo giroscópio será testado.

O runtime Rust continuará simulando a interface da lightgun original. O frontend fornece a posição/atitude da mira e o estado do gatilho; o core correlaciona o disparo com o raster/beam do VDP e o instante em que o feixe atinge a posição. Não basta testar se o cursor está dentro da tela: o evento precisa respeitar a temporização de vídeo e o comportamento de detecção do periférico.

Prever calibração, sensibilidade, inversão de eixos, zona morta, ganho, suavização configurável e opção de recenter para gyro. A suavização deve ser explícita e configurável; não pode esconder latência ou alterar silenciosamente o timestamp original.

### 2.5 Adaptadores multiplayer — fora da primeira etapa

Não implementar inicialmente:

- Sega Team Player.
- EA 4-Way Play.
- Master Tap.
- Outros adaptadores/multitaps.

A arquitetura não deve impedir a inclusão futura, mas não haverá suporte funcional declarado até existir necessidade e testes dedicados.

### 2.6 Periféricos especiais

| Dispositivo | Decisão |
|---|---|
| Sega Activator | Descartado. Não reproduzir sensores infravermelhos/área física na primeira arquitetura. |
| Inovation Smash Controller | Fora de escopo. É um controle incomum com nove direções/entradas direcionais codificadas em quatro linhas; não confundir com Super Smash Bros. |
| Teclado | Usar o teclado nativo do host como fonte de teclas. Não implementar um periférico de teclado Sega separado nesta etapa. Se uma futura máquina SC-3000 exigir matriz/PPI, isso será um contrato de hardware específico, não apenas key mapping. |
| Rapid Fire Unit | Dispensável; não implementar inicialmente. |
| Microfone | Usar dispositivos de captura de áudio do host, incluindo microfone integrado ao controle quando o Windows o expuser como endpoint de captura. Não simular microfone como botão/gamepad. |

A captura de microfone é fluxo de áudio, não entrada digital de joystick. Deve integrar-se ao subsistema de áudio Rust. Em Windows, avaliar WASAPI capture para enumerar/abrir o endpoint correspondente. A existência de microfone no controle não garante que XInput ou SDL Gamepad o exponham.

## 3. SDL3, XInput e capacidades reais no Windows

SDL3 é a camada preferencial de descoberta/abstração de controles para a aplicação, desde que a capacidade seja consultada em runtime. XInput pode permanecer como caminho de compatibilidade para controles Xbox e dispositivos que se apresentem como XInput.

**Limitação essencial:** XInput padrão não fornece API de giroscópio, acelerômetro ou touchpad. As estruturas documentadas de XInput expõem botões, sticks, triggers, vibração e flags de capacidades, mas não IMU. Portanto, não usar XInput como requisito para gyro/lightgun ou touchpad.

SDL3 possui APIs específicas para consultar e habilitar sensores de gamepad, obter dados de gyro/acelerômetro e consultar touchpads/fingers. Isso não garante que qualquer dispositivo ou modo de conexão os exponha: verificar `SDL_GamepadHasSensor`, `SDL_GetGamepadSensorData`, `SDL_GetNumGamepadTouchpads` e `SDL_GetGamepadTouchpadFinger` no dispositivo real.

A documentação SDL3 consultada registra:

- Sensor API de gamepad desde SDL 3.2.0.
- Touchpad API de gamepad desde SDL 3.2.0.
- Hint dedicado HIDAPI para 8BitDo desde SDL 3.4.0.
- Driver HIDAPI 8BitDo com suporte a sensores em dispositivos/protocolos compatíveis.

Para DS4, DualSense e 8BitDo Ultimate 2, executar testes por combinação de dispositivo, firmware, cabo/Bluetooth e modo de operação. Registrar se o driver efetivo foi HIDAPI, XInput, Windows GameInput ou outro fallback. Não inferir que o modo Switch do 8BitDo funcionará apenas porque SDL tem um driver Switch/8BitDo: comprovar o modelo exato, gyro e frequência de atualização.

## 4. Matriz de hardware de validação

| Hardware | Finalidade | Estado |
|---|---|---|
| Logitech G923 | Eixos de volante e pedais; calibração analógica | Disponível para teste do mantenedor |
| DualShock 4 | Touchpad, sticks, gyro, botões e gatilho | Disponível para teste do mantenedor |
| DualSense | Touchpad, sticks, gyro, botões e gatilho | Disponível para teste do mantenedor |
| 8BitDo Ultimate 2 em modo Switch | Gyro e entradas; comparar latência/estabilidade | Disponível para teste do mantenedor |
| Mouse | Ponteiro e lightgun | Fonte nativa prevista; teste de integração pendente |
| Teclado do host | Entrada de teclas e mapeamentos | Fonte nativa prevista; teste de integração pendente |

Os dispositivos do mantenedor são alvos de validação manual futura. Nenhum teste físico é declarado concluído por este documento.

## 5. Contratos técnicos a implementar

A futura camada de entrada deve separar:

1. **HostInputBackend:** SDL3/XInput/Windows, hotplug, identificação e coleta de eventos.
2. **NormalizedInputEvent:** botões, eixos, ponteiro, toque, gyro/IMU, teclado e captura de áudio; valores com unidade, timestamp e origem.
3. **InputMapping/Profile:** mapeamento configurável de dispositivos do host para ações/fonte do periférico emulado.
4. **EmulatedPeripheral:** estado Rust, protocolo, registradores, sinais e temporização.
5. **MachineInputRouter:** roteamento por máquina/porta e eventos de sistema (ex.: Pause/NMI).
6. **VideoBeam interface:** consulta temporal do VDP para lightguns e dispositivos ópticos.
7. **AudioCapture interface:** fluxo de microfone Rust/WASAPI separado da API de gamepad.

Os timestamps de host não devem alterar o relógio emulado. O scheduler precisa converter eventos externos em entradas determinísticas em pontos definidos da timeline, registrando a ordem e evitando polling do sistema operacional por instrução.

## 6. Critérios de aceitação

- [ ] Joysticks 2/3/6 botões com protocolo e transições corretas.
- [ ] Pause 8-bit roteado como entrada de sistema/NMI, separado dos botões comuns.
- [ ] Perfis analógicos com calibração e saturação verificáveis.
- [ ] G923 reconhecido e eixos/pedais identificados sem depender de mapeamento fixo por índice.
- [ ] Mouse e touchpad convertidos em eventos de ponteiro com coordenadas e cliques.
- [ ] DS4/DualSense: capacidades reais de touchpad e gyro enumeradas, habilitadas e medidas.
- [ ] 8BitDo Ultimate 2: gyro validado especificamente em modo Switch e em cada conexão testada.
- [ ] Lightgun sincronizada com raster/beam; não apenas cursor e gatilho.
- [ ] Eventos externos ordenados e reproduzíveis em testes com timestamps fixos.
- [ ] Nenhuma lógica de periférico ou áudio implementada em Python.
- [ ] Adaptadores, Activator, Smash, Rapid Fire e emulação dedicada de teclado Sega permanecem explicitamente fora da primeira entrega.

## 7. Ordem de implementação

1. Inventariar os contratos de entrada atuais nos cores SG-1000/SMS e frontend; identificar duplicações e compatibilidade já existente.
2. Consolidar gamepads digitais e Pause/NMI dos sistemas 8-bit.
3. Criar a abstração de eventos normalizados e perfis de calibração.
4. Integrar analógicos e G923.
5. Integrar mouse, stick direito e touchpads DS4/DualSense.
6. Implementar lightguns com sincronização temporal do VDP; validar mouse, stick e gyro.
7. Integrar captura de microfone via áudio Rust/WASAPI quando existir consumidor de hardware que a exija.
8. Só reabrir itens descartados/adiados mediante evidência de necessidade.

Cada fatia exige critérios, testes Rust, testes de integração e validação física quando depender de hardware. Não declarar `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS` ou `TEMPORAL_PASS` sem a evidência correspondente.

## 8. Referências técnicas consultadas

### Genesis Plus GX — referência de protocolos
- https://github.com/ekeeke/Genesis-Plus-GX
- https://github.com/ekeeke/Genesis-Plus-GX/tree/master/core/input_hw

### SDL3 — APIs de gamepad, touchpad e sensores
- https://wiki.libsdl.org/SDL3/CategoryGamepad
- https://wiki.libsdl.org/SDL3/SDL_GetGamepadSensorData
- https://wiki.libsdl.org/SDL3/SDL_SetGamepadSensorEnabled
- https://wiki.libsdl.org/SDL3/SDL_GetNumGamepadTouchpads
- https://wiki.libsdl.org/SDL3/SDL_GetGamepadTouchpadFinger
- https://wiki.libsdl.org/SDL3/SDL_HINT_JOYSTICK_HIDAPI_8BITDO
- https://github.com/libsdl-org/SDL/blob/main/src/joystick/hidapi/SDL_hidapi_8bitdo.c

### XInput — capacidades e limitações
- https://learn.microsoft.com/en-us/windows/win32/api/xinput/ns-xinput-xinput_capabilities
- https://learn.microsoft.com/en-us/windows/win32/xinput/getting-started-with-xinput

### Master System / Game Gear — Pause
- https://www.smspower.org/Development/PauseButton
- https://www.smspower.org/Development/StartButton
- https://www.shadow-eclipse.net/tipus_manuals/consoles/sega/Sega%20Master%20System%203%20Manual.pdf

### Smash Controller
- https://gendev.spritesmind.net/forum/viewtopic.php?t=1083

---
**Nota de licença:** Genesis Plus GX deve ser usado como referência de comportamento e pesquisa. Não copiar/portar código. Verificar licenças e procedência de cada fonte antes de incorporar qualquer material ao Sega Emulator.
