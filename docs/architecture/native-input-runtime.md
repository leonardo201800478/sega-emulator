# Native Input Runtime

## Objetivo

O crate sega-input-runtime é a camada de entrada host-independent compartilhada por todos os cores Sega. Ele define os contratos entre dispositivos físicos, backends de plataforma, perfis do usuário e adaptadores específicos de cada console. Não implementa comportamento elétrico de hardware Sega.

O crate não depende de SG-1000, Master System, Game Gear, Mega Drive, Sega CD, 32X, Qt, PyO3 ou APIs do sistema operacional. Cores futuros devem consumir as mesmas leituras de dispositivos e estados lógicos e traduzi-los para seus próprios registradores e periféricos.

## Fluxo de dados

1. O backend enumera dispositivos físicos e obtém seus estados mais recentes.
2. A camada de identidade atribui um DeviceId válido durante a sessão e, quando disponível, uma chave física opaca comum entre backends para evitar duplicidade GameInput/SDL3.
3. A normalização preserva botões brutos, eixos, hats, classe, capacidades, sequência e timestamp. Eixos usam Q15 assinado; triggers usam 0 para liberado e 32767 para totalmente pressionado.
4. O perfil converte controles físicos em ActionId estáveis. Os valores analógicos permanecem disponíveis.
5. O roteador atribui um ou mais dispositivos aos jogadores lógicos. Não pressupõe dois jogadores e permite, por exemplo, volante e pedais separados.
6. O adaptador do console converte ações e canais analógicos no snapshot de entrada do hardware-alvo. SG-1000 e SMS preservam seus mapas elétricos e semântica active-low.

## Latência e scheduler

- Consultar o estado mais recente na última fronteira segura antes do frame ou fatia do scheduler.
- Nunca consultar APIs do host dentro de instruções Z80, operações de memória, callbacks de barramento, pixels ou amostras de áudio.
- Não criar thread de polling por padrão. Thread adicional exige profiling e modelo explícito de ownership, sincronização, wake-up e shutdown.
- Converter timestamps nativos para uma base monotônica comum em nanossegundos.
- Reutilizar buffers de polling. Estado físico fixo e vetores caller-owned evitam alocações por amostra.
- Preservar eventos e sequências para que o scheduler futuro possa ordenar transições.
- Timestamp de host não é automaticamente tempo emulado ciclo a ciclo; a conversão será especificada e validada no scheduler.

## Estratégia Windows

- Microsoft GameInput é o candidato preferencial no Windows por unificar XInput, DirectInput, Raw Input, HID, WinRT e classes de controladores. Oferece polling e notificações de eventos.
- SDL3 é fallback de compatibilidade e backend multiplataforma. Gamepad API oferece controles mapeados; Joystick API expõe botões, eixos e hats brutos.
- GameInput e SDL3 não são grupos separados de jogadores. Devem alimentar um único registro físico e deduplicar pela identidade canônica do dispositivo quando disponível.
- XInput ctypes e PySDL3 permanecem apenas durante a migração e serão removidos após paridade e validação física.
- GameInput para PC possui redistribuível próprio. O instalador deverá distribuí-lo se o backend for enviado; ausência/falha de inicialização deve ativar SDL3.
- GameInput usa interfaces COM C++. Antes do backend de produção, é obrigatório um spike isolado de ABI em Windows. Preferir bindings Rust gerados/auditados. Se um pequeno shim C ABI for inevitável, ele só poderá encapsular chamadas COM; normalização, perfis, roteamento, scheduler e emulação permanecem em Rust.

## Compatibilidade

A matriz física deverá incluir:
- Controles Xbox/XInput USB e Bluetooth.
- DualShock 4 e DualSense USB/Bluetooth.
- Nintendo Switch Pro e Joy-Con quando expostos pelo sistema.
- Gamepads DirectInput/HID genéricos e arcade sticks.
- Volantes, pedais, câmbios e flight sticks, preservando eixos e botões independentes.
- Dispositivos com mais de 32 botões, vários hats e mais de quatro eixos.
- Vários dispositivos simultâneos, inclusive descobertos por GameInput e SDL.
- Hotplug, reconexão, suspensão/retomada, remoção durante polling e mudanças de foco.

Nenhum backend é presumido universal. Capacidades ausentes devem ser reportadas, não simuladas.

## Threading e ownership

A primeira implementação é caller-polled e pertence à fronteira nativa da aplicação/runtime. Callbacks internos do sistema podem apenas enfileirar eventos compactos; nunca chamam cores ou Python. O runtime consome eventos em fronteiras explícitas. Overflow e desconexão devem liberar estados presos.

## Gates

- INPUT_CONTRACT_IMPLEMENTED: contratos host-independent e mapeamento determinístico existem.
- INPUT_BACKEND_WINDOWS_PASS: GameInput e SDL3 nativos compilam e passam integração Windows.
- INPUT_CORE_INTEGRATION_PASS: SG-1000 e SMS consomem input nativo sem polling/tradução Python por frame.
- INPUT_HARDWARE_COMPAT_PASS: matriz de dispositivos exercitada em hardware Windows.
- INPUT_NATIVE_PASS: gates anteriores mais medições de latência e regressões.

Medições devem separar captura dispositivo-runtime, aplicação runtime-frame e button-to-photon. Registrar modelo/firmware do controle, transporte USB/Bluetooth, Windows/build, refresh, FPS, aquecimento, repetições, mediana, p95 e variação. Marketing da API ou timestamps de software não provam redução de latência.
