# Controller Input Backends

## Estado atual

O caminho de input físico ativo é nativo em Rust:

1. Sdl3Backend carrega a ABI oficial SDL3 no Windows e enumera gamepads/joysticks.
2. A Gamepad API fornece controles padronizados; a Joystick API preserva eixos, botões e hats brutos.
3. NativeInputManager mantém handles, lê estados, processa hotplug, aplica perfis e produz ações semânticas.
4. RustControllerInput é uma ponte fina entre Qt e PyO3. Não faz polling, descoberta nem mapeamento físico.
5. Qt envia ações do teclado; Rust mescla teclado e controles físicos, aplica turbo e converte máscaras nos snapshots específicos de SG-1000/SMS.

## Runtime SDL3

- Instale o runtime oficial usando scripts/install_sdl3_runtime.ps1.
- O instalador fixa SDL 3.4.18 e valida o SHA-256 do ZIP oficial.
- O arquivo SDL3.dll local é ignorado pelo Git.
- Para outro local, defina SEGA_EMULATOR_SDL3_DLL com o caminho absoluto.
- Sem SDL3, os controles físicos ficam indisponíveis; o teclado continua funcional.

## Compatibilidade implementada e pendente

Implementado nesta fatia:
- Gamepads reconhecidos pelo mapeamento SDL3, com botões e eixos padronizados.
- Joysticks genéricos com botões, eixos e hats brutos.
- Hotplug/reconexão por enumeração periódica.
- Deadzone, perfis físicos e captura de botão processados em Rust.
- Leituras na fronteira de frame, sem polling dentro do core.

Pendente:
- GameInput e deduplicação entre GameInput/SDL3.
- Mais de um dispositivo por jogador e seleção manual de dispositivos.
- Classificação detalhada de volantes, pedais, câmbios e flight sticks.
- Rumble, force feedback, sensores e relatórios HID específicos.
- Backend nativo para Linux/macOS.
- Matriz física de compatibilidade e medições de latência.

## Fronteira arquitetural

- Qt/PySide6 é frontend: menus, perfis persistidos, captura de teclado, configuração e diagnóstico. Envia somente máscaras lógicas de teclado ao runtime Rust.
- Rust é responsável pela captura física, normalização, hotplug, mapeamento físico, mesclagem com teclado, turbo e conversão dos snapshots elétricos de cada core.
- Os cores recebem somente snapshots/ações lógicas; não conhecem SDL, Windows, Qt ou dispositivos físicos.
- Não há callbacks Python por instrução, memória, pixel ou amostra de áudio.
