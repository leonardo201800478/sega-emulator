# Sega Desktop — R2 SMS / Slint / WGPU 30 / Vulkan

The Slint host presents real SMS VDP frames from the Rust runtime. R2.2 migrates the legacy menu groups and ports worker operations and UI settings that are safe before R3/R4.

## Menu migration

- Sistema: SMS region selection (USA, Brasil, Europa, Japão); other Sega systems remain visible but disabled until their runtimes are integrated.
- Arquivo: load raw `.sms` / `.bin` ROMs or a `.zip` containing exactly one supported ROM entry; reopen recent ROMs and exit. O desligamento POWER OFF mantém a ROM carregada; o comando de descarregar ROM não fica exposto neste menu.
- Vídeo: 4:3, 1:1 and 16:9 presentation modes preserve the selected ratio while the window is resized; unused viewport space is letterboxed. The source frame remains 256×192. The optional sprite-limit compatibility mode is persisted separately from hardware state.
- Áudio: a saída CPAL 0.18.2 e os controles de status/volume fazem parte da integração de software do host. Enumeração/seleção de endpoint e aceite físico prolongado continuam pendentes.
- Controles: teclado SMS ativo via eventos nativos do Slint (setas, Z/X, P e F5–F9); F11 alterna fullscreen. A primeira integração R4 de `sega-input-runtime`/SDL3 ao host Slint foi integrada pela PR #867. Os dois primeiros dispositivos elegíveis ocupam P1/P2 por ordem da sessão; D-pad ou analógico esquerdo controla direções; botões South/East controlam Botão 1/2; Start gera NMI por borda de subida; joystick genérico usa hat 0, eixos 0/1, botões 0/1 e botão 2 para NMI. Polling no host com intervalo nominal de 8 ms; o backend verifica hotplug a cada 250 ms. CI Windows SDL3, testes do runtime e build/test Release do desktop passaram no CI #7512. Perfis persistentes, remapeamento visual e aceite físico com controles ainda não foram concluídos.
- Opções: status-footer visibility. FPS size/position remain pending. Fullscreen is available from the UI and F11.
- DEBUG: request an immutable Z80/runtime snapshot and enable/disable bounded scheduler/bus trace capture. Full debugger viewport remains a follow-up.

## Menu interaction

- A barra de navegação usa itens compactos; cada seção abre um único painel contextual alinhado ao item selecionado, com reposicionamento horizontal para permanecer dentro da janela.
- Abrir ou alternar menus não pausa a emulação nem altera o layout da área de vídeo ou o estado de fullscreen.
- Clique fora do painel ou `Esc` fecha o menu. A tipografia e os controles de navegação usam escala reduzida.
- Validar visualmente cada submenu em janela normal, maximizada e fullscreen, incluindo os itens de ROM recente e os menus próximos à borda direita.

## Runtime contract

- SmsTemporalWorker owns the SMS machine and its execution thread.
- MachineScheduler remains the sole authority for emulated time.
- Start liga e executa; Pause congela preservando o estado; Stop executa POWER OFF (reinicializa a máquina sem descarregar a ROM); Reset reinicializa e retoma automaticamente; Step-frame avança um quadro enquanto pausado. Todas são operações coarse-grained do worker.
- Completed immutable VideoFrame values are published through FrameMailbox.
- The UI timer consumes only the newest pending frame; it never advances emulation. The SDL3 input timer polls only at the host boundary and submits complete controller snapshots through set_controller_at; it never reads or advances machine clocks.
- RGB888 is converted to tightly packed RGBA8 only at the presentation boundary.
- Slint/WGPU uses the renderer's own Vulkan device and queue.
- A saída física é fornecida pelo crate `sega-audio-output` baseado em CPAL 0.18.2. A geração digital do PSG continua pertencendo ao runtime SMS; o host não pode controlar o tempo do Scheduler. SDL3 gamepads são integrados ao host Slint na primeira fatia R4; aceite físico e recursos avançados de configuração permanecem pendentes.

## Run

From the repository root on Windows x64:

```powershell
cargo run --release --locked --manifest-path native/sega_desktop/Cargo.toml -- [path\to\game.sms] [usa|brazil|europe|japan]
```

The ROM argument is optional. Without it, use Arquivo > Procurar or enter an absolute path manually. The native Windows file-selection dialog filters `.sms`, `.bin` and `.zip`. A ZIP must contain exactly one non-directory `.sms` / `.bin` entry; archives are read in memory, with limits on archive size, entry count and decompressed ROM size. No archive-controlled path is extracted to disk. The selected region and sprite compatibility preference are applied to each newly loaded machine. Recent ROMs are persisted in the desktop settings directory. Clicking the video viewport resumes a paused machine; F11 toggles fullscreen.

## Validation

```powershell
cargo fmt --manifest-path native/sega_desktop/Cargo.toml -- --check
cargo clippy --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --locked --all-targets --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc
cargo build --release --locked --manifest-path native/sega_desktop/Cargo.toml --target x86_64-pc-windows-msvc

cargo fmt --manifest-path native/audio_output/Cargo.toml -- --check
cargo clippy --release --locked --all-targets --manifest-path native/audio_output/Cargo.toml --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --locked --all-targets --manifest-path native/audio_output/Cargo.toml --target x86_64-pc-windows-msvc
```

A successful build does not prove physical Vulkan presentation. Run on Windows x64 and verify real VDP output, menu panels opening without changing main-window geometry or fullscreen state, ROM replacement, pause/resume/POWER OFF/restart/reset-auto-resume/step-frame, aspect-ratio modes during continuous resize, video-click resume from pause, F11 fullscreen toggle, ZIP ROM selection and rejection cases, sprite-limit toggling, trace capture, and clean shutdown.

## R2.3 — persistent frontend preferences

- Persists presentation-only preferences in `%APPDATA%\\Sega Emulator\\slint_ui_settings.txt`: SMS region, aspect ratio, FPS visibility, footer visibility and FPS text size.
- Persists up to ten recent ROM paths in `%APPDATA%\\Sega Emulator\\recent_roms.txt`, newest first. Selecting a recent entry uses the same safe worker handoff as manual ROM loading.
- Recent ROM history and preferences are saved after UI changes and on application shutdown. They do not contain or serialize machine state.
- FPS text size offers small, medium and large values. HUD corner placement remains pending because the current Slint composition does not yet have a separately positioned overlay surface.
- The operational HUD/OSD message catalog and implementation sequence are tracked in [issue #1112](https://github.com/leonardo201800478/sega-emulator/issues/1112) and the [desktop product roadmap](../../docs/desktop-product-roadmap.md#9a-dp-4a--hudosd-sobreposta-ao-jogo). This is planned work, not an implemented overlay.

## Runtime video and save-state APIs (SMS1)

- The runtime exposes `SmsOverscanMode::Off` and `CropLeft8`. The latter crops the first eight pixels from each active line at the frame-publication boundary (248×192); it does not mutate the VDP's canonical 256×192 framebuffer or emulated timing. Default is Off.
- `SmsTemporalWorker::set_overscan_mode` controls the published frame crop. Persisting and exposing this option in Slint remains pending.
- The sprite-line compatibility switch is already wired to the worker and persisted in desktop preferences. HardwareLimit is the default; UnlockedCompatibility is an opt-in compatibility mode.
- `SmsTemporalWorker::save_state` and `load_state` provide version-validated in-memory snapshots/restores. The envelope binds the state to its format version, region, ROM bytes and VDP/PSG hardware profile. Restore is permitted only while paused. Persistent state files/slots and a portable ROM SHA-256/checksum container are not implemented yet; SRAM `.srm` files are separate.


## Windows x64 Release evidence (historical snapshot)

The local validation report for commit `118b96bf411248173b6215b7bf16aebdf3a66bff` records 26/26 desktop tests, 21/21 independent CPAL output tests, formatting, Clippy without warnings and a Windows x64 Release build as PASS. It explicitly does **not** claim physical Vulkan presentation, ROM gameplay, audible CPAL playback, endpoint lifecycle, hotplug or long-duration stability. These results are bound to that exact snapshot and are not a fresh validation of later `main` commits. See [the complete Windows x64 Release report](../../docs/testing/windows-x64-release-validation-2026-11-14.md).
