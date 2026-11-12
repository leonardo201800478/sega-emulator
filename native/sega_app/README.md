> **LEGACY / NÃO É O DESKTOP-ALVO:** este aplicativo documenta o shell anterior e seus adaptadores de apresentação/áudio. A arquitetura ativa é `native/sega_desktop` (Slint 1.18 + WGPU 30/Vulkan). Não adicionar funcionalidades aqui; a remoção física aguarda os gates R3/R5 descritos em `docs/architecture/migrations/legacy-host-retirement.md`.

# Native egui application — SMS runtime integration

`sega-app` is the native Rust/egui frontend for the temporal Sega Master System runtime.

## Current vertical slice

- Opens the native Windows file picker for `.sms` and `.zip` ROMs, while keeping the path field editable and enabling Windows clipboard paste (`eframe/clipboard`). Raw `.bin` paths remain accepted for compatibility.
- Remembers the last selected ROM directory in `%APPDATA%\\Sega Emulator\\last_rom_directory.txt`.
- Loads raw `.sms` / `.bin` files or the only `.sms` entry inside a ZIP archive; archives with zero or multiple SMS entries are rejected explicitly.
- Selects the machine region (USA/NTSC, Brazil/PAL-M, Europe/PAL, Japan/NTSC).
- Creates the existing `SmsTemporalWorker`; it does not implement a second machine or scheduler.
- Exposes coarse-grained start, pause, stop, reset and single-frame commands.
- Connects completed RGB888 VDP frames through the external `FrameMailbox` to the D3D12 child-window presenter.
- Shows frame/instruction/video telemetry and runtime errors.
- Opens a separate, movable and resizable SMS Debugger native window attached to the same running machine worker.
- Auto-saves frontend preferences every five seconds and on shutdown to `%APPDATA%\\Sega Emulator\\ui_settings.txt`; restores the main/debugger window geometry and user choices on startup. Debugger breakpoints are associated with the ROM path and restored when that same ROM is loaded; the ROM itself is not auto-loaded.
- Stops and joins the machine worker, detaches its mailbox, then shuts down D3D12 before destroying the child HWND.

## Current limitations

- 7z archives, multi-ROM ZIP selection and library indexing are not implemented.
- The D3D12 viewport requires Windows x64 and a compatible graphics device.
- CI compilation and headless tests do not prove physical GPU presentation or gameplay compatibility.

## Build

```powershell
cargo fmt --manifest-path native/sega_app/Cargo.toml -- --check
cargo clippy --release --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo build --release --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
cargo test --release --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
```

## APIs nativas e hosts

- Áudio: CPAL 0.18.2; o projeto não mantém bindings próprios de WASAPI ou XAudio2. No Windows, o backend WASAPI é interno ao CPAL.
- Vídeo: D3D12/DXGI permanece exclusivamente neste shell legado até a consolidação R5.
- Entrada: SDL3 3.4.20, carregada dinamicamente pela ABI C.

O SDL3 é carregado dinamicamente, sem dependência Cargo `sdl3`. Para instalar
a DLL oficial x64 verificada por SHA-256, execute na raiz do repositório:

```powershell
./scripts/install_sdl3_runtime.ps1
```

O backend consulta `SDL_GetVersion` e rejeita DLLs anteriores a 3.4.20.
Versões estáveis posteriores são aceitas. A atualização da DLL não altera a
semântica do polling nem introduz callbacks no caminho da CPU/Scheduler.

## Ownership and timing

The frontend sends coarse-grained commands to the SMS worker. The worker remains the sole owner of mutable machine state and emulated time. Completed RGB888 frames cross into the presentation thread through an immutable single-slot mailbox; replacing a pending presentation frame never skips CPU, VDP, PSG, Scheduler or audio work. The D3D12 host owns its presentation session and closes its mailbox only after the SMS worker has detached.


## Aspect ratio

The video viewport defaults to the SMS 4:3 presentation ratio. The UI exposes
1:1, 4:3 and 16:9 display modes. The selected viewport is fitted inside the
available client area with a uniform scale; it is never stretched to fill a
window with a different ratio. The native child presentation window follows
the fitted viewport rectangle.


## Áudio e controles do SMS

- CPAL 0.18.2 é o único backend de saída física. O shell não seleciona nem instancia WASAPI/XAudio2 diretamente.
- A sessão DSP/resampler e o envio PCM permanecem em Rust. A factory do sink é executada pelo worker SMS; o frontend não transporta amostras PCM.
- O áudio pode ser habilitado/desabilitado e o volume ajustado entre 0 e 100. Underruns e blocos descartados são exibidos na telemetria.
- Se a abertura do dispositivo falhar, a falha é reportada pelo subsistema de áudio; não deve interromper o runtime de vídeo/máquina.
- Mapeamento inicial de teclado: setas direcionais, Z (Botão 1), X (Botão 2) e P (borda de Pause/NMI).
- Polling do teclado ocorre na fronteira do frame da UI e somente envia snapshot quando o estado muda. O Pause é enviado como NMI de borda, não como pausa do worker.

**Validação física:** evidência histórica de WASAPI/XAudio2 pertence à implementação anterior e não valida CPAL. A nova saída CPAL precisa de validação física Windows x64.

**Limites desta fatia:** remapeamento configurável, jogador 2, analógicos específicos de consoles Sega, volante/pedais, gyro, touchpad e lightguns ainda não estão ligados ao shell.


### Gamepads SDL3

O shell tenta carregar `SDL3.dll` dinamicamente e usa o backend compartilhado
`sega-input-runtime::Sdl3Backend`. É possível informar outro caminho com
`SEGA_EMULATOR_SDL3_DLL`. A enumeração e o polling são feitos na thread da UI,
uma vez por frame do host, nunca dentro da CPU ou de callbacks de barramento.

O mapeamento inicial de gamepad SDL3 usa D-pad ou hat, stick esquerdo,
botões padronizados South/East para os dois botões SMS e Start para Pause/NMI.
Teclado e gamepad são combinados por OR no snapshot do jogador 1. O backend
preserva capacidades e estados brutos; gyro, touchpad, eixos de volante,
perfis configuráveis e jogador 2 ainda não fazem parte desta fatia.


## Menus nativos sobre o viewport D3D12

A barra principal permanece na janela raiz. Os menus Arquivo, Vídeo, Som, Opções e Diagnostics são exibidos em viewports egui nativos opacos, limitados à área cliente e ancorados ao botão acionado. O HWND de vídeo D3D12 permanece no fim da ordem Z dos filhos, mantendo os painéis acima do jogo sem ocultar ou pausar a apresentação.

Os eventos de menu usam a infraestrutura compartilhada de observabilidade (`sega::ui::menu`); a coleta continua desativada por padrão e é ativada pelo próprio menu Diagnostics. Não há writer ou mecanismo de captura paralelo.
