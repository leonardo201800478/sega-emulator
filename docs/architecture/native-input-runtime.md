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

- A primeira implementação física é Sdl3Backend, em Rust, carregando a ABI C do SDL3 dinamicamente no Windows. Ela usa a Gamepad API para controles mapeados e a Joystick API para eixos, botões e hats brutos.
- O SDL3 é inicializado e consultado na thread da Qt, na fronteira de input antes do avanço do frame. O backend mantém handles, atualiza estados e detecta hotplug sem chamadas por instrução.
- Instale o runtime oficial com scripts/install_sdl3_runtime.ps1. O script fixa SDL 3.4.18 e valida SHA-256 antes de copiar SDL3.dll para a raiz local; o binário não é versionado. SEGA_EMULATOR_SDL3_DLL permite indicar outro caminho.
- NativeInputManager expõe leituras semânticas pela extensão PyO3. RustControllerInput é apenas uma ponte de UI: envia perfis e recebe ações; não enumera, consulta ou mapeia dispositivos.
- O backend SDL3 atual atende inicialmente Windows. GameInput permanece candidato a backend complementar/futuro, sujeito a spike de ABI e validação; não está implementado nesta etapa.
- Atribuição automática atual suporta dois slots de jogador e um dispositivo por slot. Multi-dispositivo por jogador, deduplicação GameInput/SDL3, rumble/force feedback, sensores e classificação detalhada de volantes/pedais permanecem pendentes.
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


## Implementação física atual — 2026-10-31

- native/input_runtime/src/sdl3_backend.rs implementa o backend SDL3 nativo em Rust para Windows, sem PySDL3 ou callbacks Python.
- native/z80_core/src/input.rs implementa NativeInputManager, que normaliza leituras, aplica perfis físicos e entrega ações semânticas pela fronteira PyO3.
- src/sega_emulator/frontend/rust_controller_input.py é somente um adaptador de UI/configuração. O Qt não faz polling físico nem converte botões SDL.
- A UI continua capturando teclado por eventos Qt e combina ações de teclado com as ações já normalizadas do backend nativo.
- A implementação ainda não prova menor latência física: timestamps e medições button-to-photon não foram validados com controles reais.

### Limitações explícitas desta fatia

- SDL3 é carregado dinamicamente por SDL3.dll; sem esse runtime, o backend fica indisponível e o teclado continua operacional.
- O backend é Windows-only nesta etapa.
- Atribuição automática é limitada a dois dispositivos, um por jogador.
- Controles gamepad mapeados expõem 21 botões padrão e 6 eixos padrão. Joysticks genéricos expõem até 96 botões brutos, 16 eixos brutos e 8 hats.
- Eixos brutos são preservados; a semântica de volante/pedais ainda exige perfil manual.
- GameInput, deduplicação entre backends, force feedback, rumble e sensores não estão implementados.

- A ponte PyO3 atual entrega máscaras de ações digitais para SG-1000/SMS. Os canais analógicos continuam preservados em DeviceState no runtime Rust, mas ainda não são encaminhados aos cores; controles analógicos de cores futuros exigirão um snapshot nativo dedicado, sem round-trip por Python.
