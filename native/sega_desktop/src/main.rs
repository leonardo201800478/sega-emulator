#![deny(unsafe_code)]

#[cfg(windows)]
pub mod input_host;

#[cfg(windows)]
pub mod desktop_session;

#[cfg(windows)]
mod windows_host {
    use std::{
        cell::{Cell, RefCell},
        collections::HashSet,
        error::Error,
        fs,
        io::{Cursor, Read},
        path::{Path, PathBuf},
        process::Command,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use crate::desktop_session::{DesktopSession, DesktopSystem};
    use crate::input_host::Sdl3InputHost;
    use sega_media::{
        CartridgeCatalog, CartridgeHardwareProfile, CartridgeIdentity, SmsConsoleVariant,
        SmsFirmwareCatalog, SmsFirmwareId, SmsFirmwareKind, SmsFirmwareMappingStrategy,
        SmsFirmwareScanEntry, SmsFirmwareScanStatus, SmsFirmwareValidation, SmsRegion,
        SmsRegionPreference, resolve_sms_region,
    };
    use sega_save_runtime::SaveRamStore;
    use sega_sg1000_runtime::Sg1000TemporalWorker;
    use sega_sms_core::ControllerButtons;
    use sega_sms_runtime::{
        AnalogOutputConfig, AudioRendererMode, SmsTemporalWorker, SmsVdpRevision,
    };
    use sega_video_runtime::{FrameMailbox, PixelFormat, VideoFrame};
    use std::os::windows::process::CommandExt;
    use zip::ZipArchive;

    use slint::{
        GraphicsAPI, RenderingState, Timer, TimerMode,
        wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
    };

    const SMS_WIDTH: u32 = 256;
    const SMS_HEIGHT: u32 = 192;
    const SMS_EXTENDED_HEIGHTS: [u32; 2] = [224, 240];

    // Host diagnostics are session-only and remain disabled until the user opts in.
    fn host_diagnostics_enabled() -> bool {
        sega_observability::current().is_some_and(|logging| logging.is_enabled())
    }

    macro_rules! diagnostic_log {
        (error, $($argument:tt)*) => {
            if host_diagnostics_enabled() {
                sega_observability::tracing::error!(
                    target: "sega::host",
                    message = %format!($($argument)*)
                );
            }
        };
        (warn, $($argument:tt)*) => {
            if host_diagnostics_enabled() {
                sega_observability::tracing::warn!(
                    target: "sega::host",
                    message = %format!($($argument)*)
                );
            }
        };
        (debug, $($argument:tt)*) => {
            if host_diagnostics_enabled() {
                sega_observability::tracing::debug!(
                    target: "sega::host",
                    message = %format!($($argument)*)
                );
            }
        };
        (trace, $($argument:tt)*) => {
            if host_diagnostics_enabled() {
                sega_observability::tracing::trace!(
                    target: "sega::host",
                    message = %format!($($argument)*)
                );
            }
        };
        (info, $($argument:tt)*) => {
            if host_diagnostics_enabled() {
                sega_observability::tracing::info!(
                    target: "sega::host",
                    message = %format!($($argument)*)
                );
            }
        };
        ($($argument:tt)*) => {
            diagnostic_log!(info, $($argument)*);
        };
    }

    fn set_diagnostics_consent(
        consent: Option<&sega_observability::DiagnosticsConsent>,
    ) -> Result<(), String> {
        if let Some(consent) = consent {
            let logging = if let Some(logging) = sega_observability::current() {
                logging
            } else {
                let session = sega_observability::SessionMetadata::current(
                    env!("CARGO_PKG_VERSION"),
                    option_env!("SEGA_GIT_SHA").unwrap_or("unknown"),
                    if cfg!(debug_assertions) {
                        "debug"
                    } else {
                        "release"
                    },
                );
                sega_observability::init_after_explicit_consent(
                    consent,
                    sega_observability::LogConfig::default(),
                    session,
                )
                .map_err(|error| error.to_string())?
            };
            logging.enable(consent)?;
        } else if let Some(logging) = sega_observability::current() {
            logging.disable()?;
        }
        Ok(())
    }

    fn shutdown_diagnostics() {
        let Some(logging) = sega_observability::current() else {
            return;
        };
        if logging.is_enabled() {
            sega_observability::tracing::info!(
                target: "sega::app",
                event = "application_shutdown"
            );
        }
        // Shutdown drains only the explicitly enabled session; snapshots require an explicit export action.
        logging.shutdown();
    }

    /// Shared renderer resources plus the dimensions of the Slint-imported texture.
    /// This is host presentation state only; it never feeds back into the runtime.
    #[derive(Clone)]
    struct GpuFrameContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        texture_width: u32,
        texture_height: u32,
    }

    fn is_supported_sms_frame_extent(width: u32, height: u32) -> bool {
        width == SMS_WIDTH && (height == SMS_HEIGHT || SMS_EXTENDED_HEIGHTS.contains(&height))
    }
    const RENDERER_FALLBACK_POLL_INTERVAL: Duration = Duration::from_millis(250);
    const HOST_DIAGNOSTICS_INTERVAL: Duration = Duration::from_millis(250);
    const APP_DATA_DIRECTORY: &str = "Sega Emulator";
    const MAX_RECENT_ROMS: usize = 10;
    const MAX_ROM_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
    const MAX_ROM_BYTES: u64 = 16 * 1024 * 1024;
    const MAX_ZIP_ENTRIES: usize = 4_096;

    /// Modos de apresentação da janela; independentes do estado temporal do runtime.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DesktopDisplayMode {
        Windowed,
        FullscreenWithUi,
        FullscreenImmersive,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DisplayModeShortcut {
        F11,
        AltEnter,
    }

    /// Resolve os atalhos de apresentação sem tocar no worker ou no MachineScheduler.
    fn transition_display_mode(
        current: DesktopDisplayMode,
        shortcut: DisplayModeShortcut,
    ) -> DesktopDisplayMode {
        match shortcut {
            DisplayModeShortcut::F11 => match current {
                DesktopDisplayMode::Windowed | DesktopDisplayMode::FullscreenImmersive => {
                    DesktopDisplayMode::FullscreenWithUi
                }
                DesktopDisplayMode::FullscreenWithUi => DesktopDisplayMode::Windowed,
            },
            DisplayModeShortcut::AltEnter => match current {
                DesktopDisplayMode::FullscreenImmersive => DesktopDisplayMode::Windowed,
                DesktopDisplayMode::Windowed | DesktopDisplayMode::FullscreenWithUi => {
                    DesktopDisplayMode::FullscreenImmersive
                }
            },
        }
    }

    /// Ações do teclado do host. Não representam ciclos nem instruções da máquina.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum KeyboardAction {
        Up,
        Down,
        Left,
        Right,
        Button1,
        Button2,
        Nmi,
        Start,
        Pause,
        Stop,
        Reset,
        StepFrame,
    }

    impl KeyboardAction {
        fn from_slint_key(key: &str) -> Option<Self> {
            match key {
                "up" => Some(Self::Up),
                "down" => Some(Self::Down),
                "left" => Some(Self::Left),
                "right" => Some(Self::Right),
                "button1" => Some(Self::Button1),
                "button2" => Some(Self::Button2),
                "nmi" => Some(Self::Nmi),
                "start" => Some(Self::Start),
                "pause" => Some(Self::Pause),
                "stop" => Some(Self::Stop),
                "reset" => Some(Self::Reset),
                "step-frame" => Some(Self::StepFrame),
                _ => None,
            }
        }

        fn is_controller_button(self) -> bool {
            matches!(
                self,
                Self::Up | Self::Down | Self::Left | Self::Right | Self::Button1 | Self::Button2
            )
        }
    }

    /// Mantém níveis de teclas pressionadas e compõe um snapshot completo do controle SMS.
    #[derive(Debug, Default)]
    struct KeyboardInputState {
        pressed: HashSet<KeyboardAction>,
    }

    impl KeyboardInputState {
        /// Atualiza uma tecla e retorna a ação somente quando seu estado realmente mudou.
        fn update(&mut self, key: &str, pressed: bool) -> Option<KeyboardAction> {
            let action = KeyboardAction::from_slint_key(key)?;
            let changed = if pressed {
                self.pressed.insert(action)
            } else {
                self.pressed.remove(&action)
            };
            changed.then_some(action)
        }

        /// Compõe os níveis simultâneos das direções e dos dois botões do Master System.
        fn controller_buttons(&self) -> ControllerButtons {
            ControllerButtons {
                up: self.pressed.contains(&KeyboardAction::Up),
                down: self.pressed.contains(&KeyboardAction::Down),
                left: self.pressed.contains(&KeyboardAction::Left),
                right: self.pressed.contains(&KeyboardAction::Right),
                button_1: self.pressed.contains(&KeyboardAction::Button1),
                button_2: self.pressed.contains(&KeyboardAction::Button2),
            }
        }

        /// Descarta todas as teclas, por exemplo quando a janela perde o foco.
        fn clear(&mut self) {
            self.pressed.clear();
        }
    }

    /// Combina fontes independentes de entrada sem permitir que uma libere a outra.
    fn merge_controller_buttons(
        keyboard: ControllerButtons,
        gamepad: ControllerButtons,
    ) -> ControllerButtons {
        ControllerButtons {
            up: keyboard.up || gamepad.up,
            down: keyboard.down || gamepad.down,
            left: keyboard.left || gamepad.left,
            right: keyboard.right || gamepad.right,
            button_1: keyboard.button_1 || gamepad.button_1,
            button_2: keyboard.button_2 || gamepad.button_2,
        }
    }

    /// Owns the SDL polling thread and its lightweight Slint snapshot consumer.
    /// Dropping this value stops and joins the SDL thread before the desktop host exits.
    struct Sdl3InputPolling {
        _timer: Timer,
        _thread: Sdl3PollingThread,
    }

    struct Sdl3PollingThread {
        stop: Arc<AtomicBool>,
        join: Option<thread::JoinHandle<()>>,
    }

    impl Drop for Sdl3PollingThread {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(join) = self.join.take()
                && join.join().is_err()
            {
                diagnostic_log!(error, "R4_SDL3 event=poll_thread_panicked");
            }
        }
    }

    #[derive(Default)]
    struct Sdl3InputSnapshot {
        players: [ControllerButtons; 2],
        pause_pressed: bool,
        status_update: Option<String>,
    }

    /// SDL3 handles are created, polled and destroyed on one dedicated host thread.
    /// Slint only consumes the newest coarse-grained snapshot; it never calls SDL.
    /// The 2 ms cadence is a host sampling target, not a hardware-clock guarantee.
    fn start_sdl3_input_polling(
        app: &MainWindow,
        worker_slot: Rc<RefCell<Option<DesktopSession>>>,
        keyboard_state: Rc<RefCell<KeyboardInputState>>,
        gamepad_state: Rc<RefCell<[ControllerButtons; 2]>>,
    ) -> Sdl3InputPolling {
        let snapshot = Arc::new(Mutex::new(Sdl3InputSnapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_snapshot = Arc::clone(&snapshot);
        let thread_stop = Arc::clone(&stop);

        let join = thread::Builder::new()
            .name("sega-sdl3-input".to_owned())
            .spawn(move || {
                let mut input_host = match Sdl3InputHost::new() {
                    Ok(host) => {
                        let status = host.status().to_owned();
                        thread_snapshot
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .status_update = Some(status);
                        host
                    }
                    Err(error) => {
                        let status = format!("SDL3 indisponível — teclado ativo: {error}");
                        thread_snapshot
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .status_update = Some(status);
                        diagnostic_log!(error, "R4_SDL3 event=initialization_failed error={error}");
                        return;
                    }
                };

                while !thread_stop.load(Ordering::Acquire) {
                    match input_host.poll() {
                        Ok((players, pause_pressed, status_update)) => {
                            let mut latest = thread_snapshot
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            latest.players = players;
                            latest.pause_pressed = pause_pressed;
                            if status_update.is_some() {
                                latest.status_update = status_update;
                            }
                        }
                        Err(error) => {
                            diagnostic_log!(warn, "R4_SDL3 event=poll_failed error={error}");
                            let mut latest = thread_snapshot
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            latest.players = [ControllerButtons::default(); 2];
                            latest.pause_pressed = false;
                            latest.status_update =
                                Some(format!("SDL3 interrompido — teclado ativo: {error}"));
                            break;
                        }
                    }
                    thread::sleep(Duration::from_millis(2));
                }
            });

        let thread_guard = match join {
            Ok(join) => Sdl3PollingThread {
                stop,
                join: Some(join),
            },
            Err(error) => {
                let status =
                    format!("SDL3 indisponível — teclado ativo: falha ao iniciar thread: {error}");
                app.set_input_status(status.into());
                diagnostic_log!(error, "R4_SDL3 event=thread_start_failed error={error}");
                Sdl3PollingThread { stop, join: None }
            }
        };

        let app_weak = app.as_weak();
        let timer_snapshot = Arc::clone(&snapshot);
        let mut last_pause_pressed = false;
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, Duration::from_millis(2), move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let (mut next_players, pause_pressed, status_update) = {
                let mut latest = timer_snapshot
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                (
                    latest.players,
                    latest.pause_pressed,
                    latest.status_update.take(),
                )
            };

            let gameplay_enabled = app.get_has_rom() && app.get_active_menu().as_str() == "none";
            if !gameplay_enabled {
                next_players = [ControllerButtons::default(); 2];
            }

            if gameplay_enabled
                && pause_pressed
                && !last_pause_pressed
                && let Some(worker) = worker_slot.borrow().as_ref()
                && let Err(error) = worker.request_nmi()
            {
                diagnostic_log!(warn, "R4_SDL3 action=pause_nmi error={error}");
            }
            last_pause_pressed = pause_pressed;

            let previous_players = *gamepad_state.borrow();
            if next_players != previous_players {
                *gamepad_state.borrow_mut() = next_players;

                if let Some(worker) = worker_slot.borrow().as_ref() {
                    if next_players[0] != previous_players[0] {
                        let combined = merge_controller_buttons(
                            keyboard_state.borrow().controller_buttons(),
                            next_players[0],
                        );
                        if let Err(error) = worker.set_controller_at(1, combined, Instant::now()) {
                            diagnostic_log!(
                                warn,
                                "R4_SDL3 player=1 action=controller_update error={error}"
                            );
                        }
                    }

                    if next_players[1] != previous_players[1]
                        && let Err(error) =
                            worker.set_controller_at(2, next_players[1], Instant::now())
                    {
                        diagnostic_log!(warn, "R4_SDL3 player=2 action=controller_update error={error}");
                    }
                }
            }

            if let Some(status) = status_update {
                app.set_input_status(status.into());
            }
        });

        Sdl3InputPolling {
            _timer: timer,
            _thread: thread_guard,
        }
    }

    slint::slint! {
        import { Button, CheckBox, ComboBox, LineEdit, ScrollView, Slider } from "std-widgets.slint";
        component CompactButton inherits Button {
            height: 28px;
        }
        component RecentRomEntry inherits Rectangle {
            in property <string> rom-path;
            callback activated();
            width: 100%;
            height: path_label.preferred-height + 12px;
            background: #263541;
            border-radius: 3px;
            path_label := Text {
                x: 6px; y: 6px;
                width: parent.width - 12px;
                text: root.rom-path;
                color: #f2f5f7;
                font-size: 11px;
                wrap: word-wrap;
            }
            TouchArea {
                width: 100%; height: 100%;
                clicked => { root.activated(); }
            }
        }
        export component MainWindow inherits Window {
            forward-focus: keyboard-focus;
            title: "Sega Emulator — Slint / WGPU 30 / Vulkan";
            default-font-size: 12px;
            preferred-width: 1120px; preferred-height: 860px;
            min-width: 960px; min-height: 720px;
            background: #101820;
            in-out property <image> gpu-texture;
            in-out property <image> play-icon;
            in-out property <image> pause-icon;
            in-out property <image> stop-icon;
            in-out property <image> reset-icon;
            in-out property <image> step-frame-icon;
            in-out property <string> gpu-status;
            in-out property <string> frame-status;
            in-out property <string> lifecycle-status;
            in-out property <string> rom-status;
            in-out property <string> active-menu: "none";
            in-out property <string> selected-region: "auto";
            in-out property <string> selected-system: "sms1";
            in-out property <string> firmware-directory: "";
            in-out property <string> firmware-scan-status: "Selecione a pasta de firmware e execute a varredura.";
            in-out property <string> firmware-scan-results: "Nenhuma varredura realizada.";
            in-out property <bool> firmware-boot-enabled: false;
            in-out property <[string]> firmware-brazil-options: [];
            in-out property <[string]> firmware-usa-options: [];
            in-out property <[string]> firmware-europe-options: [];
            in-out property <[string]> firmware-japan-options: [];
            in-out property <[string]> firmware-korea-options: [];
            in-out property <string> firmware-brazil-selected: "";
            in-out property <string> firmware-usa-selected: "";
            in-out property <string> firmware-europe-selected: "";
            in-out property <string> firmware-japan-selected: "";
            in-out property <string> firmware-korea-selected: "";
            in-out property <string> active-system: "sms1";
            in-out property <string> selected-aspect: "4:3";
            in-out property <string> recent-rom-path: "";
            in-out property <[string]> recent-roms: [];
            in-out property <string> fps-size: "medium";
            in-out property <length> fps-font-size: 18px;
            in-out property <string> rom-path-input: "";
            in-out property <string> debug-status: "Debugger ainda não consultado.";
            in-out property <bool> has-rom: false;
            in-out property <bool> emulation-paused: false;
            in-out property <bool> sprite-limit-enabled: true;
            in-out property <bool> show-fps: true;
            in-out property <bool> show-status-footer: true;
            in-out property <bool> trace-enabled: false;
            in-out property <bool> diagnostics-enabled: false;
            in-out property <float> audio-volume: 70;
            in-out property <bool> audio-muted: false;
            in-out property <string> audio-status: "Aguardando configuração da saída de áudio.";
            in-out property <string> input-status: "Inicializando SDL3...";
            in-out property <string> keyboard-up-key: Key.UpArrow;
            in-out property <string> keyboard-down-key: Key.DownArrow;
            in-out property <string> keyboard-left-key: Key.LeftArrow;
            in-out property <string> keyboard-right-key: Key.RightArrow;
            in-out property <string> keyboard-button1-key: Key.Z;
            in-out property <string> keyboard-button2-key: Key.X;
            in-out property <string> keyboard-up-label: "↑";
            in-out property <string> keyboard-down-label: "↓";
            in-out property <string> keyboard-left-label: "←";
            in-out property <string> keyboard-right-label: "→";
            in-out property <string> keyboard-button1-label: "Z";
            in-out property <string> keyboard-button2-label: "X";
            in-out property <string> keyboard-capture-action: "";
            in-out property <string> keyboard-capture-status: "Selecione uma ação para alterar sua tecla.";
            in-out property <string> audio-renderer-mode: "psg";
            in-out property <bool> audio-dsp-preset: false;
            in-out property <bool> audio-hpf-enabled: false;
            in-out property <float> audio-hpf-cutoff: 20;
            in-out property <bool> audio-lpf-enabled: false;
            in-out property <float> audio-lpf-cutoff: 18000;
            in-out property <bool> sg1000-audio-hpf-enabled: false;
            in-out property <float> sg1000-audio-hpf-cutoff: 20;
            in-out property <bool> sg1000-audio-lpf-enabled: false;
            in-out property <float> sg1000-audio-lpf-cutoff: 18000;
            in-out property <bool> fullscreen-enabled: false;
            in-out property <bool> immersive-fullscreen: false;
            in-out property <string> fps-status: "FPS: --";
            in-out property <length> video-width: 768px;
            in-out property <length> video-height: 576px;
            callback start-requested();
            callback pause-requested();
            callback stop-requested();
            callback reset-requested();
            callback step-frame-requested();
            callback load-rom-requested(string);
            callback browse-rom-requested();
            callback close-rom-requested();
            callback exit-requested();
            callback debug-snapshot-requested();
            callback trace-export-requested();
            callback trace-toggle-requested(bool);
            callback diagnostics-toggle-requested(bool);
            callback sprite-limit-toggle-requested(bool);
            callback display-mode-toggle-requested(string);
            callback video-area-clicked();
            callback preferences-changed();
            callback browse-firmware-directory-requested();
            callback scan-firmware-directory-requested(string);
            callback firmware-selection-changed(string, string);
            callback firmware-boot-toggle-requested(bool);
            callback audio-volume-changed(float);
            callback audio-mute-changed(bool);
            callback audio-renderer-changed(string);
            callback audio-dsp-changed(bool, bool, float, bool, float);
            callback sg1000-audio-dsp-changed(bool, float, bool, float);
            callback keyboard-key-pressed(string);
            callback keyboard-key-released(string);
            callback keyboard-focus-lost();
            callback keyboard-key-captured(string, string);
            keyboard-focus := FocusScope {
                width: 100%; height: 100%;
                key-pressed(event) => {
                    if (root.keyboard-capture-action != "") {
                        if (event.text == Key.Escape) {
                            root.keyboard-capture-action = "";
                            root.keyboard-capture-status = "Remapeamento cancelado."; accept
                        } else if (event.text == Key.P || event.text == Key.F5 || event.text == Key.F6 || event.text == Key.F7 || event.text == Key.F8 || event.text == Key.F9 || event.text == Key.F11 || event.text == Key.Return) {
                            root.keyboard-capture-action = "";
                            root.keyboard-capture-status = "Tecla reservada para atalhos do emulador."; accept
                        } else { root.keyboard-key-captured(root.keyboard-capture-action, event.text); accept }
                    }
                    else if (event.text == root.keyboard-up-key) { root.keyboard-key-pressed("up"); accept }
                    else if (event.text == root.keyboard-down-key) { root.keyboard-key-pressed("down"); accept }
                    else if (event.text == root.keyboard-left-key) { root.keyboard-key-pressed("left"); accept }
                    else if (event.text == root.keyboard-right-key) { root.keyboard-key-pressed("right"); accept }
                    else if (event.text == root.keyboard-button1-key) { root.keyboard-key-pressed("button1"); accept }
                    else if (event.text == root.keyboard-button2-key) { root.keyboard-key-pressed("button2"); accept }
                    else if (event.text == Key.P) { root.keyboard-key-pressed("nmi"); accept }
                    else if (event.text == Key.F5) { root.keyboard-key-pressed("start"); accept }
                    else if (event.text == Key.F6) { root.keyboard-key-pressed("pause"); accept }
                    else if (event.text == Key.F7) { root.keyboard-key-pressed("stop"); accept }
                    else if (event.text == Key.F8) { root.keyboard-key-pressed("reset"); accept }
                    else if (event.text == Key.F9) { root.keyboard-key-pressed("step-frame"); accept }
                    else { reject }
                }
                key-released(event) => {
                    if (root.keyboard-capture-action != "") { accept }
                    else if (event.text == root.keyboard-up-key) { root.keyboard-key-released("up"); accept }
                    else if (event.text == root.keyboard-down-key) { root.keyboard-key-released("down"); accept }
                    else if (event.text == root.keyboard-left-key) { root.keyboard-key-released("left"); accept }
                    else if (event.text == root.keyboard-right-key) { root.keyboard-key-released("right"); accept }
                    else if (event.text == root.keyboard-button1-key) { root.keyboard-key-released("button1"); accept }
                    else if (event.text == root.keyboard-button2-key) { root.keyboard-key-released("button2"); accept }
                    else if (event.text == Key.P) { root.keyboard-key-released("nmi"); accept }
                    else if (event.text == Key.F5) { root.keyboard-key-released("start"); accept }
                    else if (event.text == Key.F6) { root.keyboard-key-released("pause"); accept }
                    else if (event.text == Key.F7) { root.keyboard-key-released("stop"); accept }
                    else if (event.text == Key.F8) { root.keyboard-key-released("reset"); accept }
                    else if (event.text == Key.F9) { root.keyboard-key-released("step-frame"); accept }
                    else { reject }
                }
                focus-lost(_reason) => { root.keyboard-focus-lost(); }
                KeyBinding {
                    keys: @keys(Escape);
                    activated => { root.active-menu = "none"; }
                }
                KeyBinding {
                    keys: @keys(F11);
                    activated => { root.display-mode-toggle-requested("f11"); }
                }
                KeyBinding {
                    keys: @keys(Alt + Return);
                    activated => { root.display-mode-toggle-requested("alt-enter"); }
                }
                VerticalLayout {
                padding: root.immersive-fullscreen ? 0px : 12px;
                spacing: root.immersive-fullscreen ? 0px : 8px;
                if !root.immersive-fullscreen: Text { text: "SEGA EMULATOR"; color: #f2f5f7; font-size: 20px; }
                if !root.immersive-fullscreen: HorizontalLayout {
                    spacing: 3px;
                    z: 95;
                    CompactButton { text: "Sistema"; width: 66px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "system" ? "none" : "system"; } }
                    CompactButton { text: "Arquivo"; width: 62px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "file" ? "none" : "file"; } }
                    CompactButton { text: "Firmware"; width: 72px; enabled: root.selected-system != "sg1000"; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "firmware" ? "none" : "firmware"; if (root.firmware-directory != "") { root.scan-firmware-directory-requested(root.firmware-directory); } } }
                    CompactButton { text: "Vídeo"; width: 58px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "video" ? "none" : "video"; } }
                    CompactButton { text: "Áudio"; width: 58px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "audio" ? "none" : "audio"; } }
                    CompactButton { text: "Controles"; width: 76px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "controls" ? "none" : "controls"; } }
                    CompactButton { text: "Opções"; width: 66px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "options" ? "none" : "options"; } }
                    CompactButton { text: "DEBUG"; width: 58px; clicked => { root.keyboard-focus-lost(); root.active-menu = root.active-menu == "debug" ? "none" : "debug"; } }
                }
                if !root.immersive-fullscreen: HorizontalLayout {
                    spacing: 5px;
                    CompactButton { text: ""; icon: root.play-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Iniciar emulação"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.start-requested(); } }
                    CompactButton { text: ""; icon: root.pause-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Pausar emulação"; enabled: root.has-rom; clicked => { root.emulation-paused = true; root.pause-requested(); } }
                    CompactButton { text: ""; icon: root.stop-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Parar emulação"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.stop-requested(); } }
                    CompactButton { text: ""; icon: root.reset-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Resetar console"; enabled: root.has-rom; clicked => { root.emulation-paused = false; root.reset-requested(); } }
                    CompactButton { text: ""; icon: root.step-frame-icon; icon-size: 24px; colorize-icon: true; width: 44px; height: 38px; accessible-role: button; accessible-label: "Avançar um quadro"; enabled: root.has-rom; clicked => { root.emulation-paused = true; root.pause-requested(); root.step-frame-requested(); } }
                    Text { text: root.rom-status; color: #b9c5ce; font-size: 12px; vertical-alignment: center; }
                }
                if root.has-rom: Rectangle {
                    horizontal-stretch: 1; vertical-stretch: 1;
                    min-width: 256px; min-height: 192px;
                    background: #05080b;
                    Image {
                        source: root.gpu-texture;
                        width: min(parent.width, parent.height * (root.selected-aspect == "1:1" ? 1.0 : root.selected-aspect == "16:9" ? 16.0 / 9.0 : 4.0 / 3.0));
                        height: min(parent.height, parent.width / (root.selected-aspect == "1:1" ? 1.0 : root.selected-aspect == "16:9" ? 16.0 / 9.0 : 4.0 / 3.0));
                        x: (parent.width - self.width) / 2;
                        y: (parent.height - self.height) / 2;
                        image-fit: fill;
                    }
                    TouchArea {
                        width: 100%; height: 100%;
                        clicked => { root.video-area-clicked(); }
                    }
                    if root.immersive-fullscreen: Text {
                        x: 12px; y: 12px; z: 20;
                        text: root.fps-status;
                        color: #8bd5ca;
                        font-size: root.fps-font-size;
                    }
                }
                if !root.has-rom: Rectangle {
                    horizontal-stretch: 1; vertical-stretch: 1;
                    min-width: 256px; min-height: 192px;
                    background: #111722; border-radius: 14px;
                    VerticalLayout {
                        padding: 28px; spacing: 18px;
                        Text { text: "SEGA EMULATOR"; color: rgb(142, 171, 255); font-size: 13px; font-weight: 700; }
                        Text { text: "Sua biblioteca começa aqui"; color: #F1F5FA; font-size: 30px; font-weight: 700; }
                        Text { text: "Carregue uma ROM compatível para iniciar uma sessão de emulação."; color: #B4C0D0; font-size: 14px; }
                        HorizontalLayout {
                            spacing: 12px;
                            CompactButton {
                                text: "Abrir ROM";
                                height: 42px;
                                clicked => { root.browse-rom-requested(); }
                            }
                            CompactButton {
                                text: "Configurar firmware";
                                height: 42px;
                                clicked => {
                                    root.active-menu = "firmware";
                                    if (root.firmware-directory != "") {
                                        root.scan-firmware-directory-requested(root.firmware-directory);
                                    }
                                }
                            }
                            CompactButton {
                                text: "Configurações de vídeo";
                                height: 42px;
                                clicked => { root.active-menu = "video"; }
                            }
                        }
                        Rectangle {
                            background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                            height: 170px;
                            VerticalLayout {
                                padding: 16px; spacing: 8px;
                                Text { text: "ROMs recentes"; color: #F1F5FA; font-size: 17px; font-weight: 600; }
                                if root.recent-roms.length == 0: Text {
                                    text: "Nenhuma ROM recente. Use “Abrir ROM” para selecionar um arquivo .sms, .bin ou .zip.";
                                    color: #B4C0D0; font-size: 12px; wrap: word-wrap;
                                }
                                if root.recent-roms.length > 0: ScrollView {
                                    height: 112px; width: 100%;
                                    content-width: self.width;
                                    content-height: root.recent-roms.length * 42px;
                                    VerticalLayout {
                                        width: 100%; spacing: 4px;
                                        for rom in root.recent-roms: RecentRomEntry {
                                            rom-path: rom;
                                            activated => { root.load-rom-requested(rom); root.active-menu = "none"; }
                                        }
                                    }
                                }
                            }
                        }
                        Rectangle {
                            background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                            height: 92px;
                            VerticalLayout {
                                padding: 14px; spacing: 5px;
                                Text { text: "Sistema selecionado"; color: #B4C0D0; font-size: 11px; }
                                Text {
                                    text: root.selected-system == "sg1000" ? "Sega SG-1000 · TMS9918A" : root.selected-system == "sms2" ? "Sega Master System II · VDP 315-5246" : "Sega Master System · VDP 315-5124";
                                    color: #F1F5FA; font-size: 14px; font-weight: 600;
                                }
                                Text { text: root.selected-region == "auto" ? "Região: Automática" : root.selected-region == "brazil" ? "Região: Brasil / PAL-M" : root.selected-region == "europe" ? "Região: Europa / PAL" : root.selected-region == "japan" ? "Região: Japão / NTSC" : root.selected-region == "korea" ? "Região: Coreia / NTSC" : "Região: USA / NTSC"; color: rgb(142, 171, 255); font-size: 11px; }
                            }
                        }
                        Text { text: root.rom-status; color: #B4C0D0; font-size: 11px; wrap: word-wrap; }
                    }
                }
                if !root.immersive-fullscreen && root.show-fps: Text { text: root.fps-status; color: #8bd5ca; font-size: root.fps-font-size; }
                if !root.immersive-fullscreen && root.show-status-footer: Text { text: root.gpu-status; color: #8bd5ca; }
                if !root.immersive-fullscreen && root.show-status-footer: Text { text: root.frame-status; color: #b9c5ce; }
                if !root.immersive-fullscreen && root.show-status-footer: Text { text: root.lifecycle-status; color: #b9c5ce; }
                }
        if !root.immersive-fullscreen && root.active-menu != "none": TouchArea {
            x: 0px; y: 74px; width: 100%; height: parent.height - 74px; z: 90;
            clicked => { root.active-menu = "none"; }
        }
        if !root.immersive-fullscreen && root.active-menu != "none": Rectangle {
            x: (root.active-menu == "firmware" || root.active-menu == "video" || root.active-menu == "controls" || root.active-menu == "system" || root.active-menu == "file" || root.active-menu == "options" || root.active-menu == "debug") ? (parent.width - self.width) / 2 : min(max(root.active-menu == "audio" ? 207px : 8px, 8px), parent.width - self.width - 8px);
            y: 88px;
            width: min(parent.width - 32px, root.active-menu == "video" ? 1048px : root.active-menu == "firmware" ? 900px : root.active-menu == "controls" ? 680px : root.active-menu == "system" ? 760px : root.active-menu == "file" ? 720px : root.active-menu == "options" || root.active-menu == "debug" ? 640px : 480px);
            height: min(parent.height - 104px, root.active-menu == "firmware" || root.active-menu == "video" ? 680px : root.active-menu == "file" ? 560px : root.active-menu == "controls" ? 660px : root.active-menu == "options" ? 400px : root.active-menu == "system" ? 660px : root.active-menu == "debug" ? 480px : root.active-menu == "audio" ? 480px : 180px);
            z: 100;
            background: #1b2834; border-radius: 5px; border-width: 1px;
            border-color: #344154; min-height: 120px; max-height: 680px; max-width: 1048px;
            ScrollView {
                width: 100%; height: 100%;
                content-width: self.width;
                content-height: root.active-menu == "firmware" ? 980px : root.active-menu == "file" ? 540px + root.recent-roms.length * 40px : root.active-menu == "controls" ? 800px : root.active-menu == "system" ? 660px : root.active-menu == "video" ? 900px : root.active-menu == "options" ? 360px : root.active-menu == "debug" ? 440px : root.active-menu == "audio" ? 780px : 180px;
                VerticalLayout {
                    alignment: start;
                    width: 100%;
                    height: root.active-menu == "firmware" ? 980px : root.active-menu == "file" ? 540px + root.recent-roms.length * 40px : root.active-menu == "controls" ? 800px : root.active-menu == "system" ? 660px : root.active-menu == "video" ? 900px : root.active-menu == "options" ? 360px : root.active-menu == "debug" ? 440px : root.active-menu == "audio" ? 780px : 180px;
                    padding: 8px; spacing: 6px;
                        if root.active-menu == "system": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Sistema"; color: #F1F5FA; }
                            Text { font-size: 12px; text: "Modelo do console e região utilizados na próxima sessão."; color: #B4C0D0; }
                            Rectangle {
                                height: 164px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Modelo de hardware"; color: #F1F5FA; }
                                    Text { font-size: 12px; text: root.has-rom ? (root.active-system == "sg1000" ? "Em execução: SG-1000 · TMS9918A" : root.active-system == "sms2" ? "Em execução: Master System II · VDP 315-5246" : "Em execução: Master System · VDP 315-5124") : (root.selected-system == "sg1000" ? "Selecionado: SG-1000 · TMS9918A" : root.selected-system == "sms2" ? "Selecionado: Master System II · VDP 315-5246" : "Selecionado: Master System · VDP 315-5124"); color: rgb(142, 171, 255); }
                                    FlexboxLayout { flex-direction: row; flex-wrap: wrap; spacing: 6px;
                                        CompactButton { text: root.selected-system == "sms1" ? "✓ Master System / SMS1" : "Master System / SMS1"; enabled: !root.has-rom; clicked => { root.selected-system = "sms1"; root.preferences-changed(); if (root.firmware-directory != "") { root.scan-firmware-directory-requested(root.firmware-directory); } } }
                                        CompactButton { text: root.selected-system == "sms2" ? "✓ Master System II / SMS2" : "Master System II / SMS2"; enabled: !root.has-rom; clicked => { root.selected-system = "sms2"; root.preferences-changed(); if (root.firmware-directory != "") { root.scan-firmware-directory-requested(root.firmware-directory); } } }
                                        CompactButton { text: root.selected-system == "sg1000" ? "✓ SG-1000" : "SG-1000"; enabled: !root.has-rom; clicked => { root.selected-system = "sg1000"; root.preferences-changed(); } }
                                    }
                                    Text { font-size: 10px; text: root.has-rom ? "A ROM atual mantém o hardware já construído; a seleção vale para a próxima ROM." : "O hardware selecionado será construído ao carregar a ROM."; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            Rectangle {
                                height: 212px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Região e frequência"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: "Região e frequência são independentes do ASIC selecionado."; color: #B4C0D0; }
                                    FlexboxLayout { flex-direction: row; flex-wrap: wrap; spacing: 6px;
                                        CompactButton { text: root.selected-region == "auto" ? "✓ Automático" : "Automático"; clicked => { root.selected-region = "auto"; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-region == "brazil" ? "✓ Brasil / PAL-M" : "Brasil / PAL-M"; clicked => { root.selected-region = "brazil"; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-region == "europe" ? "✓ Europa / PAL" : "Europa / PAL"; clicked => { root.selected-region = "europe"; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-region == "japan" ? "✓ Japão / NTSC" : "Japão / NTSC"; clicked => { root.selected-region = "japan"; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-region == "korea" ? "✓ Coreia / NTSC" : "Coreia / NTSC"; clicked => { root.selected-region = "korea"; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-region == "usa" ? "✓ USA / NTSC" : "USA / NTSC"; clicked => { root.selected-region = "usa"; root.preferences-changed(); } }
                                    }
                                    Text { font-size: 10px; text: root.selected-region == "auto" ? "Automático: World → USA; múltiplas regiões: Brasil → USA → Europa → Japão → Coreia; sem evidência confiável: USA." : "A preferência regional será aplicada ao carregar a próxima ROM."; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            Rectangle {
                                height: 150px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Firmware e outros sistemas"; color: #F1F5FA; }
                                    CompactButton { text: "Gerenciar firmware SMS"; clicked => { root.active-menu = "firmware"; if (root.firmware-directory != "") { root.scan-firmware-directory-requested(root.firmware-directory); } } }
                                    Text { font-size: 10px; text: "Game Gear, Mega Drive, Sega CD e 32X ainda não estão disponíveis neste runtime."; color: #D6B879; wrap: word-wrap; }
                                }
                            }
                        }
                        if root.active-menu == "file": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Arquivo"; color: #F1F5FA; }
                            Text { font-size: 12px; text: "Abra uma ROM compatível ou retome um título recente."; color: #B4C0D0; }
                            Rectangle {
                                height: 132px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Carregar ROM"; color: #F1F5FA; }
                                    LineEdit { text <=> root.rom-path-input; placeholder-text: "C:\\ROMs\\game.sms"; }
                                    HorizontalLayout { spacing: 8px;
                                        CompactButton { text: "Procurar..."; clicked => { root.browse-rom-requested(); } }
                                        CompactButton { text: "Carregar ROM"; clicked => { root.load-rom-requested(root.rom-path-input); root.active-menu = "none"; } }
                                        CompactButton { text: "Sair"; clicked => { root.exit-requested(); } }
                                    }
                                }
                            }
                            Rectangle {
                                height: 360px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "ROMs recentes"; color: #F1F5FA; }
                                    if root.recent-roms.length == 0: Text { font-size: 12px; text: "Nenhuma ROM recente."; color: #B4C0D0; }
                                    if root.recent-roms.length > 0: ScrollView {
                                        height: 300px; width: 100%; content-width: self.width;
                                        content-height: root.recent-roms.length * 42px;
                                        VerticalLayout { width: 100%; spacing: 4px;
                                            for rom in root.recent-roms: RecentRomEntry { rom-path: rom; activated => { root.load-rom-requested(rom); root.active-menu = "none"; } }
                                        }
                                    }
                                }
                            }
                        }
                        if root.active-menu == "video": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Vídeo"; color: #F1F5FA; }
                            Text { font-size: 12px; text: "Apresentação da imagem e recursos visuais do sistema."; color: #B4C0D0; }
                            Rectangle {
                                height: 148px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Janela e escala"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: "Proporção da imagem"; color: #B4C0D0; }
                                    FlexboxLayout {
                                        flex-direction: row; flex-wrap: wrap; spacing: 6px;
                                        CompactButton { text: root.selected-aspect == "4:3" ? "✓ Original (4:3)" : "Original (4:3)"; clicked => { root.selected-aspect = "4:3"; root.video-width = 768px; root.video-height = 576px; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-aspect == "1:1" ? "✓ 1:1" : "1:1"; clicked => { root.selected-aspect = "1:1"; root.video-width = 576px; root.video-height = 576px; root.preferences-changed(); } }
                                        CompactButton { text: root.selected-aspect == "16:9" ? "✓ 16:9" : "16:9"; clicked => { root.selected-aspect = "16:9"; root.video-width = 768px; root.video-height = 432px; root.preferences-changed(); } }
                                    }
                                    Text { font-size: 10px; text: "A escala altera somente a área de apresentação; o frame publicado pelo VDP permanece em sua resolução nativa."; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            HorizontalLayout {
                                spacing: 12px;
                                Rectangle {
                                    horizontal-stretch: 1; height: 170px; background: #192231; border-radius: 10px;
                                    border-width: 1px; border-color: #344154;
                                    VerticalLayout {
                                        padding: 14px; spacing: 8px;
                                        Text { font-size: 16px; text: "Compatibilidade do VDP"; color: #F1F5FA; }
                                        CheckBox {
                                            text: "Limite de sprites por linha";
                                            checked: root.sprite-limit-enabled;
                                            toggled => { root.sprite-limit-enabled = self.checked; root.sprite-limit-toggle-requested(self.checked); root.preferences-changed(); }
                                        }
                                        Text { font-size: 10px; text: "Desativar o limite é um modo de compatibilidade. Overflow e temporização continuam pertencendo ao VDP emulado."; color: #B4C0D0; wrap: word-wrap; }
                                    }
                                }
                                Rectangle {
                                    horizontal-stretch: 1; height: 170px; background: #192231; border-radius: 10px;
                                    border-width: 1px; border-color: #344154;
                                    VerticalLayout {
                                        padding: 14px; spacing: 8px;
                                        Text { font-size: 16px; text: "Sincronização e telemetria"; color: #F1F5FA; }
                                        CheckBox { text: "Exibir FPS"; checked: root.show-fps; toggled => { root.show-fps = self.checked; root.preferences-changed(); } }
                                        CheckBox { text: "Exibir diagnóstico no rodapé"; checked: root.show-status-footer; toggled => { root.show-status-footer = self.checked; root.preferences-changed(); } }
                                        Text { font-size: 10px; text: "FPS e diagnóstico são métricas do host; não controlam o relógio do MachineScheduler."; color: #B4C0D0; wrap: word-wrap; }
                                    }
                                }
                            }
                            Rectangle {
                                height: 220px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Recursos de apresentação"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: "As opções abaixo pertencem ao desenho aprovado, mas dependem de integração e validação do backend gráfico."; color: #B4C0D0; wrap: word-wrap; }
                                    HorizontalLayout {
                                        Text { text: "VSync / Mailbox / VRR"; color: #B4C0D0; vertical-alignment: center; }
                                        Text { text: "Indisponível neste host"; color: #D6B879; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    HorizontalLayout {
                                        Text { text: "Overscan e corte regional"; color: #B4C0D0; vertical-alignment: center; }
                                        Text { text: "Não integrado"; color: #D6B879; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    HorizontalLayout {
                                        Text { text: "Shaders / CRT / NTSC"; color: #B4C0D0; vertical-alignment: center; }
                                        Text { text: "Não integrado"; color: #D6B879; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    HorizontalLayout {
                                        Text { text: "HDR / paper white"; color: #B4C0D0; vertical-alignment: center; }
                                        Text { text: "Não validado"; color: #D6B879; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    HorizontalLayout {
                                        Text { text: "SegaScope / 3D glasses"; color: #B4C0D0; vertical-alignment: center; }
                                        Text { text: "Somente sistemas compatíveis"; color: #D6B879; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                }
                            }
                        }
                        if root.active-menu == "audio": VerticalLayout {
                            spacing: 10px;
                            Text { font-size: 18px; text: "Áudio"; color: #f2f5f7; }
                            Text { font-size: 11px; text: "Configurações de áudio do sistema emulado"; color: #9baab6; }
                            Rectangle {
                                height: 132px; background: #243443; border-radius: 7px;
                                border-width: 1px; border-color: #405365;
                                VerticalLayout {
                                    padding: 10px; spacing: 7px;
                                    Text { font-size: 14px; text: "VOLUME GERAL"; color: #8bd5ca; }
                                    HorizontalLayout {
                                        Text { font-size: 12px; text: "Volume mestre"; color: #f2f5f7; }
                                        Text { font-size: 12px; text: "\\{round(root.audio-volume)}%"; color: #f2f5f7; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    Slider { minimum: 0; maximum: 100; step: 1; value <=> root.audio-volume; released(value) => { root.audio-volume-changed(value); } }
                                    HorizontalLayout {
                                        spacing: 6px;
                                        CompactButton { text: root.audio-muted ? "Ativar áudio" : "Silenciar"; clicked => { root.audio-muted = !root.audio-muted; root.audio-mute-changed(root.audio-muted); } }
                                        CompactButton { text: "50%"; clicked => { root.audio-volume = 50; root.audio-volume-changed(50); } }
                                        CompactButton { text: "70%"; clicked => { root.audio-volume = 70; root.audio-volume-changed(70); } }
                                    }
                                }
                            }
                            if (root.has-rom ? root.active-system : root.selected-system) != "sg1000": Rectangle {
                                height: 150px; background: #243443; border-radius: 7px;
                                border-width: 1px; border-color: #405365;
                                VerticalLayout {
                                    padding: 10px; spacing: 7px;
                                    Text { font-size: 14px; text: "RENDERIZAÇÃO PSG"; color: #8bd5ca; }
                                    Text { font-size: 11px; text: "Selecione o renderer digital do SN76489."; color: #b9c5ce; }
                                    HorizontalLayout {
                                        spacing: 6px;
                                        CompactButton {
                                            text: root.audio-renderer-mode == "psg" ? "✓ PSG padrão" : "PSG padrão";
                                            enabled: (root.has-rom ? root.active-system : root.selected-system) != "sg1000";
                                            clicked => { root.audio-renderer-mode = "psg"; root.audio-renderer-changed("psg"); }
                                        }
                                        CompactButton {
                                            text: root.audio-renderer-mode == "h2-experimental" ? "✓ H2 experimental" : "H2 experimental";
                                            enabled: (root.has-rom ? root.active-system : root.selected-system) != "sg1000";
                                            clicked => { root.audio-renderer-mode = "h2-experimental"; root.audio-renderer-changed("h2-experimental"); }
                                        }
                                    }
                                    Text {
                                        font-size: 11px;
                                        text: root.audio-renderer-mode == "h2-experimental"
                                            ? "H2 reconstrói transições do mesmo SN76489. Experimental; em caso de falha o worker retorna ao PSG."
                                            : "PSG utiliza o renderer SN76489 validado e permanece como padrão.";
                                        color: #d6b879; wrap: word-wrap;
                                    }
                                }
                            }
                            if (root.has-rom ? root.active-system : root.selected-system) == "sg1000": Rectangle {
                                height: 340px; background: #243443; border-radius: 7px;
                                border-width: 1px; border-color: #405365;
                                VerticalLayout {
                                    padding: 10px; spacing: 7px;
                                    Text { font-size: 14px; text: "DSP DE SAÍDA — SG-1000"; color: #8bd5ca; }
                                    Text { font-size: 10px; text: "Filtros pós-mixer opcionais. Desligados, o caminho digital permanece sem processamento analógico adicional."; color: #d6b879; wrap: word-wrap; }
                                    CheckBox {
                                        text: "Passa-altas / acoplamento AC"; checked <=> root.sg1000-audio-hpf-enabled;
                                        toggled => { root.sg1000-audio-dsp-changed(self.checked, root.sg1000-audio-hpf-cutoff, root.sg1000-audio-lpf-enabled, root.sg1000-audio-lpf-cutoff); }
                                    }
                                    HorizontalLayout {
                                        Text { font-size: 11px; text: "Corte HPF"; color: #b9c5ce; }
                                        Text { font-size: 11px; text: "\\{round(root.sg1000-audio-hpf-cutoff)} Hz"; color: #f2f5f7; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    Slider {
                                        minimum: 5; maximum: 500; step: 5; value <=> root.sg1000-audio-hpf-cutoff; enabled: root.sg1000-audio-hpf-enabled;
                                        released(value) => { root.sg1000-audio-dsp-changed(root.sg1000-audio-hpf-enabled, value, root.sg1000-audio-lpf-enabled, root.sg1000-audio-lpf-cutoff); }
                                    }
                                    CheckBox {
                                        text: "Passa-baixas"; checked <=> root.sg1000-audio-lpf-enabled;
                                        toggled => { root.sg1000-audio-dsp-changed(root.sg1000-audio-hpf-enabled, root.sg1000-audio-hpf-cutoff, self.checked, root.sg1000-audio-lpf-cutoff); }
                                    }
                                    HorizontalLayout {
                                        Text { font-size: 11px; text: "Corte LPF"; color: #b9c5ce; }
                                        Text { font-size: 11px; text: "\\{round(root.sg1000-audio-lpf-cutoff)} Hz"; color: #f2f5f7; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    Slider {
                                        minimum: 4000; maximum: 20000; step: 250; value <=> root.sg1000-audio-lpf-cutoff; enabled: root.sg1000-audio-lpf-enabled;
                                        released(value) => { root.sg1000-audio-dsp-changed(root.sg1000-audio-hpf-enabled, root.sg1000-audio-hpf-cutoff, root.sg1000-audio-lpf-enabled, value); }
                                    }
                                    CompactButton {
                                        text: "Desativar todos os filtros";
                                        clicked => {
                                            root.sg1000-audio-hpf-enabled = false;
                                            root.sg1000-audio-lpf-enabled = false;
                                            root.sg1000-audio-dsp-changed(false, root.sg1000-audio-hpf-cutoff, false, root.sg1000-audio-lpf-cutoff);
                                        }
                                    }
                                    Text { font-size: 10px; text: "Experimental: não há esquema/captura elétrica SG-1000 suficiente para afirmar uma resposta analógica específica da placa. Não altera PSG, Scheduler ou clocks."; color: #d6b879; wrap: word-wrap; }
                                }
                            }
                            if (root.has-rom ? root.active-system : root.selected-system) != "sg1000": Rectangle {
                                height: 340px; background: #243443; border-radius: 7px;
                                border-width: 1px; border-color: #405365;
                                VerticalLayout {
                                    padding: 10px; spacing: 7px;
                                    Text { font-size: 14px; text: "DSP / ESTÁGIO ANALÓGICO"; color: #8bd5ca; }
                                    CheckBox {
                                        text: "Preset SMS1 — experimental"; checked <=> root.audio-dsp-preset;
                                        toggled => {
                                            if (self.checked) {
                                                root.audio-hpf-enabled = true; root.audio-hpf-cutoff = 20;
                                                root.audio-lpf-enabled = true; root.audio-lpf-cutoff = 18000;
                                            } else { root.audio-hpf-enabled = false; root.audio-lpf-enabled = false; }
                                            root.audio-dsp-changed(self.checked, root.audio-hpf-enabled, root.audio-hpf-cutoff, root.audio-lpf-enabled, root.audio-lpf-cutoff);
                                        }
                                    }
                                    Text { font-size: 10px; text: "Aproximação ajustável; não representa calibração elétrica de uma revisão específica da placa."; color: #d6b879; wrap: word-wrap; }
                                    CheckBox {
                                        text: "Passa-altas / acoplamento AC"; checked <=> root.audio-hpf-enabled;
                                        toggled => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, self.checked, root.audio-hpf-cutoff, root.audio-lpf-enabled, root.audio-lpf-cutoff); }
                                    }
                                    HorizontalLayout {
                                        Text { font-size: 11px; text: "Corte HPF"; color: #b9c5ce; }
                                        Text { font-size: 11px; text: "\\{round(root.audio-hpf-cutoff)} Hz"; color: #f2f5f7; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    Slider {
                                        minimum: 5; maximum: 500; step: 5; value <=> root.audio-hpf-cutoff; enabled: root.audio-hpf-enabled;
                                        released(value) => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, root.audio-hpf-enabled, value, root.audio-lpf-enabled, root.audio-lpf-cutoff); }
                                    }
                                    CheckBox {
                                        text: "Passa-baixas"; checked <=> root.audio-lpf-enabled;
                                        toggled => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, root.audio-hpf-enabled, root.audio-hpf-cutoff, self.checked, root.audio-lpf-cutoff); }
                                    }
                                    HorizontalLayout {
                                        Text { font-size: 11px; text: "Corte LPF"; color: #b9c5ce; }
                                        Text { font-size: 11px; text: "\\{round(root.audio-lpf-cutoff)} Hz"; color: #f2f5f7; horizontal-stretch: 1; horizontal-alignment: right; }
                                    }
                                    Slider {
                                        minimum: 4000; maximum: 20000; step: 250; value <=> root.audio-lpf-cutoff; enabled: root.audio-lpf-enabled;
                                        released(value) => { root.audio-dsp-preset = false; root.audio-dsp-changed(false, root.audio-hpf-enabled, root.audio-hpf-cutoff, root.audio-lpf-enabled, value); }
                                    }
                                    CompactButton {
                                        text: "Desativar todos os filtros";
                                        clicked => { root.audio-dsp-preset = false; root.audio-hpf-enabled = false; root.audio-lpf-enabled = false; root.audio-dsp-changed(false, false, root.audio-hpf-cutoff, false, root.audio-lpf-cutoff); }
                                    }
                                }
                            }
                            Rectangle {
                                height: 62px; background: #1d2b37; border-radius: 7px;
                                border-width: 1px; border-color: #405365;
                                VerticalLayout {
                                    padding: 8px; spacing: 4px;
                                    Text { font-size: 11px; text: root.audio-status; color: #8bd5ca; wrap: word-wrap; }
                                    Text { font-size: 10px; text: "Filtros no worker, após o mixer e antes do reamostrador. Estado contínuo entre blocos; sem alteração no PSG, Scheduler ou clocks."; color: #9baab6; wrap: word-wrap; }
                                }
                            }
                        }
                        if root.active-menu == "controls": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Controles"; color: #F1F5FA; }
                            Text { font-size: 12px; text: "Configure teclado, jogadores e atalhos sem interferir no relógio do emulador."; color: #B4C0D0; }
                            Rectangle {
                                height: 370px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Remapeamento do teclado · P1"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: "Selecione uma ação e pressione a nova tecla. Esc cancela a captura."; color: #B4C0D0; wrap: word-wrap; }
                                    GridLayout {
                                        spacing: 6px;
                                        Row {
                                            Text { text: "Ação"; color: #B4C0D0; }
                                            Text { text: "Tecla atual"; color: #B4C0D0; }
                                            Text { text: "Atribuir"; color: #B4C0D0; }
                                        }
                                        Row { Text { text: "Cima"; color: #F1F5FA; } Text { text: root.keyboard-up-label; color: rgb(142, 171, 255); } CompactButton { text: root.keyboard-capture-action == "up" ? "Pressione..." : "Alterar"; clicked => { root.keyboard-capture-action = "up"; root.keyboard-capture-status = "Pressione a tecla para Cima..."; } } }
                                        Row { Text { text: "Baixo"; color: #F1F5FA; } Text { text: root.keyboard-down-label; color: rgb(142, 171, 255); } CompactButton { text: root.keyboard-capture-action == "down" ? "Pressione..." : "Alterar"; clicked => { root.keyboard-capture-action = "down"; root.keyboard-capture-status = "Pressione a tecla para Baixo..."; } } }
                                        Row { Text { text: "Esquerda"; color: #F1F5FA; } Text { text: root.keyboard-left-label; color: rgb(142, 171, 255); } CompactButton { text: root.keyboard-capture-action == "left" ? "Pressione..." : "Alterar"; clicked => { root.keyboard-capture-action = "left"; root.keyboard-capture-status = "Pressione a tecla para Esquerda..."; } } }
                                        Row { Text { text: "Direita"; color: #F1F5FA; } Text { text: root.keyboard-right-label; color: rgb(142, 171, 255); } CompactButton { text: root.keyboard-capture-action == "right" ? "Pressione..." : "Alterar"; clicked => { root.keyboard-capture-action = "right"; root.keyboard-capture-status = "Pressione a tecla para Direita..."; } } }
                                        Row { Text { text: "Botão 1"; color: #F1F5FA; } Text { text: root.keyboard-button1-label; color: rgb(142, 171, 255); } CompactButton { text: root.keyboard-capture-action == "button1" ? "Pressione..." : "Alterar"; clicked => { root.keyboard-capture-action = "button1"; root.keyboard-capture-status = "Pressione a tecla para Botão 1..."; } } }
                                        Row { Text { text: "Botão 2"; color: #F1F5FA; } Text { text: root.keyboard-button2-label; color: rgb(142, 171, 255); } CompactButton { text: root.keyboard-capture-action == "button2" ? "Pressione..." : "Alterar"; clicked => { root.keyboard-capture-action = "button2"; root.keyboard-capture-status = "Pressione a tecla para Botão 2..."; } } }
                                    }
                                    HorizontalLayout {
                                        spacing: 8px;
                                        CompactButton { text: "Restaurar padrão"; clicked => {
                                            root.keyboard-up-key = Key.UpArrow; root.keyboard-down-key = Key.DownArrow;
                                            root.keyboard-left-key = Key.LeftArrow; root.keyboard-right-key = Key.RightArrow;
                                            root.keyboard-button1-key = Key.Z; root.keyboard-button2-key = Key.X;
                                            root.keyboard-up-label = "↑"; root.keyboard-down-label = "↓";
                                            root.keyboard-left-label = "←"; root.keyboard-right-label = "→";
                                            root.keyboard-button1-label = "Z"; root.keyboard-button2-label = "X";
                                            root.keyboard-capture-action = ""; root.keyboard-capture-status = "Mapeamento padrão restaurado.";
                                            root.preferences-changed();
                                        } }
                                    }
                                    Text { font-size: 11px; text: root.keyboard-capture-status; color: rgb(142, 171, 255); wrap: word-wrap; }
                                }
                            }
                            Rectangle {
                                height: 160px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Atalhos do emulador"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: "F5 iniciar · F6 pausar · F7 parar · F8 resetar · F9 avançar quadro"; color: #B4C0D0; }
                                    Text { font-size: 11px; text: "F11 tela cheia com menus · Alt+Enter tela cheia imersiva · Esc fecha o menu"; color: #B4C0D0; }
                                    Text { font-size: 11px; text: "P solicita NMI por borda. No Master System, Pause é uma NMI de hardware; não é o comando Pausar do host."; color: #D6B879; wrap: word-wrap; }
                                }
                            }
                            Rectangle {
                                height: 170px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Gamepads e joysticks · P1 / P2"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: root.input-status; color: rgb(142, 171, 255); wrap: word-wrap; }
                                    Text { font-size: 11px; text: "Atribuição atual: os dois primeiros dispositivos compatíveis da sessão são associados a P1 e P2. D-pad/analógico esquerdo, South/East e Start usam o perfil padrão SDL3."; color: #B4C0D0; wrap: word-wrap; }
                                    Text { font-size: 11px; text: "Perfis independentes por dispositivo, captura de botões/eixos/hats e hotkeys por combinação exigem identidade persistente e editor de perfil; não são apresentados como ativos nesta etapa."; color: #D6B879; wrap: word-wrap; }
                                }
                            }
                        }
                        if root.active-menu == "options": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Opções"; color: #F1F5FA; }
                            Text { font-size: 12px; text: "Elementos de interface e apresentação do host."; color: #B4C0D0; }
                            Rectangle { height: 176px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "HUD e telemetria"; color: #F1F5FA; }
                                    CheckBox { text: "Exibir FPS"; checked: root.show-fps; toggled => { root.show-fps = self.checked; root.preferences-changed(); } }
                                    CheckBox { text: "Exibir status no rodapé"; checked: root.show-status-footer; toggled => { root.show-status-footer = self.checked; root.preferences-changed(); } }
                                    FlexboxLayout { flex-direction: row; flex-wrap: wrap; spacing: 6px;
                                        CompactButton { text: root.fps-size == "small" ? "✓ Pequeno" : "Pequeno"; clicked => { root.fps-size = "small"; root.fps-font-size = 13px; root.preferences-changed(); } }
                                        CompactButton { text: root.fps-size == "medium" ? "✓ Médio" : "Médio"; clicked => { root.fps-size = "medium"; root.fps-font-size = 18px; root.preferences-changed(); } }
                                        CompactButton { text: root.fps-size == "large" ? "✓ Grande" : "Grande"; clicked => { root.fps-size = "large"; root.fps-font-size = 24px; root.preferences-changed(); } }
                                    }
                                    Text { font-size: 10px; text: "FPS mede uploads de textura bem-sucedidos; não representa apresentação física do monitor."; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            Rectangle { height: 128px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Modo de exibição"; color: #F1F5FA; }
                                    HorizontalLayout { spacing: 8px;
                                        CompactButton { text: root.fullscreen-enabled && !root.immersive-fullscreen ? "✓ Tela cheia com menus" : "Tela cheia com menus (F11)"; clicked => { root.display-mode-toggle-requested("f11"); } }
                                        CompactButton { text: root.immersive-fullscreen ? "✓ Tela cheia imersiva" : "Tela cheia imersiva (Alt+Enter)"; clicked => { root.display-mode-toggle-requested("alt-enter"); } }
                                    }
                                    Text { font-size: 10px; text: "A posição do HUD permanece inferior nesta versão."; color: #B4C0D0; }
                                }
                            }
                        }
                        if root.active-menu == "firmware": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Firmware"; color: #F1F5FA; }
                            Text {
                                font-size: 12px;
                                text: root.selected-system == "sms2"
                                    ? "Gerencie imagens catalogadas para Master System II / Samsung SMS2."
                                    : "Gerencie imagens catalogadas para Master System I / SMS1.";
                                color: #B4C0D0; wrap: word-wrap;
                            }
                            Rectangle {
                                height: 164px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "1. Diretório de firmware"; color: #F1F5FA; }
                                    HorizontalLayout {
                                        spacing: 8px;
                                        LineEdit { horizontal-stretch: 1; text <=> root.firmware-directory; placeholder-text: "Pasta que contém BIOS e firmwares"; }
                                        CompactButton { text: "Selecionar pasta"; clicked => { root.browse-firmware-directory-requested(); } }
                                        CompactButton { text: "Escanear"; enabled: root.firmware-directory != ""; clicked => { root.scan-firmware-directory-requested(root.firmware-directory); } }
                                    }
                                    Text { font-size: 11px; text: root.firmware-scan-status; color: rgb(142, 171, 255); wrap: word-wrap; }
                                    Text { font-size: 10px; text: root.firmware-scan-results; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            Rectangle {
                                height: 420px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "2. Seleção regional"; color: #F1F5FA; }
                                    Text { font-size: 11px; text: "Cada região mantém uma seleção independente. Somente imagens válidas e compatíveis com o modelo selecionado ficam disponíveis."; color: #B4C0D0; wrap: word-wrap; }
                                    VerticalLayout {
                                        spacing: 6px;
                                        HorizontalLayout {
                                            spacing: 8px;
                                            Text { width: 100px; text: "Brasil"; color: #F1F5FA; vertical-alignment: center; }
                                            ComboBox { horizontal-stretch: 1; model: root.firmware-brazil-options; current-value: root.firmware-brazil-selected; enabled: root.firmware-brazil-options.length > 0; selected(value) => { root.firmware-brazil-selected = value; root.firmware-selection-changed("brazil", value); root.preferences-changed(); } }
                                        }
                                        HorizontalLayout {
                                            spacing: 8px;
                                            Text { width: 100px; text: "USA"; color: #F1F5FA; vertical-alignment: center; }
                                            ComboBox { horizontal-stretch: 1; model: root.firmware-usa-options; current-value: root.firmware-usa-selected; enabled: root.firmware-usa-options.length > 0; selected(value) => { root.firmware-usa-selected = value; root.firmware-selection-changed("usa", value); root.preferences-changed(); } }
                                        }
                                        HorizontalLayout {
                                            spacing: 8px;
                                            Text { width: 100px; text: "Europa"; color: #F1F5FA; vertical-alignment: center; }
                                            ComboBox { horizontal-stretch: 1; model: root.firmware-europe-options; current-value: root.firmware-europe-selected; enabled: root.firmware-europe-options.length > 0; selected(value) => { root.firmware-europe-selected = value; root.firmware-selection-changed("europe", value); root.preferences-changed(); } }
                                        }
                                        HorizontalLayout {
                                            spacing: 8px;
                                            Text { width: 100px; text: "Japão"; color: #F1F5FA; vertical-alignment: center; }
                                            ComboBox { horizontal-stretch: 1; model: root.firmware-japan-options; current-value: root.firmware-japan-selected; enabled: root.firmware-japan-options.length > 0; selected(value) => { root.firmware-japan-selected = value; root.firmware-selection-changed("japan", value); root.preferences-changed(); } }
                                        }
                                        HorizontalLayout {
                                            spacing: 8px;
                                            Text { width: 100px; text: "Coreia"; color: #F1F5FA; vertical-alignment: center; }
                                            ComboBox { horizontal-stretch: 1; model: root.firmware-korea-options; current-value: root.firmware-korea-selected; enabled: root.firmware-korea-options.length > 0; selected(value) => { root.firmware-korea-selected = value; root.firmware-selection-changed("korea", value); root.preferences-changed(); } }
                                        }
                                    }
                                    Text { font-size: 10px; text: "Samsung Gam*Boy II é uma variante própria de SMS2. Identificação e seleção não comprovam o boot em hardware."; color: #D6B879; wrap: word-wrap; }
                                }
                            }
                            Rectangle {
                                height: 150px; background: #192231; border-radius: 10px;
                                border-width: 1px; border-color: #344154;
                                VerticalLayout {
                                    padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "3. Inicialização"; color: #F1F5FA; }
                                    CheckBox {
                                        text: "Habilitar boot pela BIOS / firmware";
                                        checked <=> root.firmware-boot-enabled;
                                        toggled => { root.firmware-boot-toggle-requested(self.checked); root.preferences-changed(); }
                                    }
                                    Text { font-size: 10px; text: "Com cartucho, somente BIOS convencional de 8 KiB identificada e revalidada será executada antes da ROM. Firmwares com jogo interno e boot sem cartucho continuam pendentes de seus mapas específicos no runtime."; color: #D6B879; wrap: word-wrap; }
                                }
                            }
                        }
                        if root.active-menu == "debug": VerticalLayout {
                            spacing: 12px;
                            Text { font-size: 24px; text: "Diagnóstico"; color: #F1F5FA; }
                            Text { font-size: 12px; text: "Snapshots de CPU e ferramentas de diagnóstico do runtime ativo."; color: #B4C0D0; }
                            Rectangle { height: 220px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 8px;
                                    Text { font-size: 16px; text: "Runtime e CPU"; color: #F1F5FA; }
                                    CompactButton { text: "Capturar snapshot da CPU"; enabled: root.has-rom && root.diagnostics-enabled; clicked => { root.debug-snapshot-requested(); } }
                                    CompactButton { text: root.trace-enabled ? "Desativar trace" : "Ativar trace"; enabled: root.has-rom && root.active-system != "sg1000" && root.diagnostics-enabled; clicked => { root.trace-enabled = !root.trace-enabled; root.trace-toggle-requested(root.trace-enabled); } }
                                    CompactButton { text: "Exportar trace para arquivo"; enabled: root.has-rom && root.trace-enabled && root.diagnostics-enabled && root.active-system != "sg1000"; clicked => { root.trace-export-requested(); } }
                                    Text { font-size: 11px; text: root.debug-status; color: rgb(142, 171, 255); wrap: word-wrap; }
                                    Text { font-size: 10px; text: "A GUI solicita snapshots e traces; não executa instruções nem controla ciclos."; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            Rectangle { height: 112px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 6px;
                                    Text { font-size: 16px; text: "Coleta de logs, métricas e traces"; color: #F1F5FA; }
                                    CheckBox {
                                        text: "Autorizar coleta diagnóstica nesta sessão";
                                        checked <=> root.diagnostics-enabled;
                                        toggled => { root.diagnostics-toggle-requested(self.checked); }
                                    }
                                    Text { font-size: 10px; text: "Desativado por padrão. A autorização desta sessão abrange logs, métricas, snapshots e traces. Desativar interrompe a coleta e limpa os buffers; exportar trace exige ação explícita. A autorização não é persistida."; color: #B4C0D0; wrap: word-wrap; }
                                }
                            }
                            Rectangle { height: 84px; background: #192231; border-radius: 10px; border-width: 1px; border-color: #344154;
                                VerticalLayout { padding: 14px; spacing: 6px;
                                    Text { font-size: 12px; text: "Breakpoints, inspeção de memória e depuração interativa ainda não foram integrados."; color: #D6B879; wrap: word-wrap; }
                                }
                            }
                        }
                    }
                }
                }
            }
        }
    }

    /// Preferências persistidas exclusivamente para a camada de apresentação.
    #[derive(Debug, Clone)]
    struct DesktopPreferences {
        region: String,
        selected_system: String,
        firmware_directory: String,
        firmware_boot_enabled: bool,
        firmware_brazil_selected: String,
        firmware_usa_selected: String,
        firmware_europe_selected: String,
        firmware_japan_selected: String,
        firmware_korea_selected: String,
        aspect: String,
        show_fps: bool,
        show_status_footer: bool,
        fps_size: String,
        sprite_limit_enabled: bool,
        audio_volume: u8,
        audio_muted: bool,
        audio_renderer_mode: String,
        audio_dsp_preset: bool,
        audio_hpf_enabled: bool,
        audio_hpf_cutoff_hz: u16,
        audio_lpf_enabled: bool,
        audio_lpf_cutoff_hz: u16,
        sg1000_audio_hpf_enabled: bool,
        sg1000_audio_hpf_cutoff_hz: u16,
        sg1000_audio_lpf_enabled: bool,
        sg1000_audio_lpf_cutoff_hz: u16,
        keyboard_up_key: String,
        keyboard_down_key: String,
        keyboard_left_key: String,
        keyboard_right_key: String,
        keyboard_button1_key: String,
        keyboard_button2_key: String,
    }

    impl Default for DesktopPreferences {
        fn default() -> Self {
            Self {
                region: "auto".to_owned(),
                selected_system: "sms1".to_owned(),
                firmware_directory: String::new(),
                firmware_boot_enabled: false,
                firmware_brazil_selected: String::new(),
                firmware_usa_selected: String::new(),
                firmware_europe_selected: String::new(),
                firmware_japan_selected: String::new(),
                firmware_korea_selected: String::new(),
                aspect: "4:3".to_owned(),
                show_fps: true,
                show_status_footer: true,
                fps_size: "medium".to_owned(),
                sprite_limit_enabled: true,
                audio_volume: 70,
                audio_muted: false,
                audio_renderer_mode: "psg".to_owned(),
                audio_dsp_preset: false,
                audio_hpf_enabled: false,
                audio_hpf_cutoff_hz: 20,
                audio_lpf_enabled: false,
                audio_lpf_cutoff_hz: 18_000,
                sg1000_audio_hpf_enabled: false,
                sg1000_audio_hpf_cutoff_hz: 20,
                sg1000_audio_lpf_enabled: false,
                sg1000_audio_lpf_cutoff_hz: 18_000,
                keyboard_up_key: String::new(),
                keyboard_down_key: String::new(),
                keyboard_left_key: String::new(),
                keyboard_right_key: String::new(),
                keyboard_button1_key: String::new(),
                keyboard_button2_key: String::new(),
            }
        }
    }

    /// Resolve o diretório de dados por usuário no Windows sem criar diretórios durante leitura.
    fn app_data_directory() -> Option<PathBuf> {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_DATA_DIRECTORY))
    }

    /// Cria o armazenamento SRM dentro do diretório de dados do usuário.
    fn cartridge_save_store() -> Result<Arc<SaveRamStore>, String> {
        let root = app_data_directory()
            .ok_or_else(|| "APPDATA não está definido para armazenar SRAM".to_owned())?
            .join("saves");
        Ok(Arc::new(SaveRamStore::new(root)))
    }

    /// Carrega preferências com fallback individual para valores seguros e conhecidos.
    fn load_desktop_preferences() -> DesktopPreferences {
        let mut preferences = DesktopPreferences::default();
        let Some(directory) = app_data_directory() else {
            return preferences;
        };
        let Ok(contents) = fs::read_to_string(directory.join("slint_ui_settings.txt")) else {
            return preferences;
        };

        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "region"
                    if ["auto", "brazil", "europe", "japan", "korea", "usa"]
                        .contains(&value.trim()) =>
                {
                    preferences.region = value.trim().to_owned();
                }
                "selected_system" if ["sms1", "sms2", "sg1000"].contains(&value.trim()) => {
                    preferences.selected_system = value.trim().to_owned();
                }
                // Backward compatibility with settings written before multi-system selection.
                "sms_system" if ["sms1", "sms2"].contains(&value.trim()) => {
                    preferences.selected_system = value.trim().to_owned();
                }
                "firmware_directory" => {
                    preferences.firmware_directory = value.trim().to_owned();
                }
                "firmware_boot_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.firmware_boot_enabled = value;
                    }
                }
                "firmware_brazil_selected" => {
                    preferences.firmware_brazil_selected = value.trim().to_owned();
                }
                "firmware_usa_selected" => {
                    preferences.firmware_usa_selected = value.trim().to_owned();
                }
                "firmware_europe_selected" => {
                    preferences.firmware_europe_selected = value.trim().to_owned();
                }
                "firmware_japan_selected" => {
                    preferences.firmware_japan_selected = value.trim().to_owned();
                }
                "firmware_korea_selected" => {
                    preferences.firmware_korea_selected = value.trim().to_owned();
                }
                "aspect" if ["4:3", "1:1", "16:9"].contains(&value.trim()) => {
                    preferences.aspect = value.trim().to_owned();
                }
                "show_fps" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.show_fps = value;
                    }
                }
                "show_status_footer" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.show_status_footer = value;
                    }
                }
                "fps_size" if ["small", "medium", "large"].contains(&value.trim()) => {
                    preferences.fps_size = value.trim().to_owned();
                }
                "sprite_limit_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.sprite_limit_enabled = value;
                    }
                }
                "audio_volume" => {
                    if let Ok(value) = value.trim().parse::<u8>()
                        && value <= 100
                    {
                        preferences.audio_volume = value;
                    }
                }
                "audio_muted" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_muted = value;
                    }
                }
                "audio_renderer_mode" if ["psg", "h2-experimental"].contains(&value.trim()) => {
                    preferences.audio_renderer_mode = value.trim().to_owned();
                }
                "audio_dsp_preset" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_dsp_preset = value;
                    }
                }
                "audio_hpf_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_hpf_enabled = value;
                    }
                }
                "audio_hpf_cutoff_hz" => {
                    if let Ok(value) = value.trim().parse::<u16>()
                        && (5..=500).contains(&value)
                    {
                        preferences.audio_hpf_cutoff_hz = value;
                    }
                }
                "audio_lpf_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.audio_lpf_enabled = value;
                    }
                }
                "audio_lpf_cutoff_hz" => {
                    if let Ok(value) = value.trim().parse::<u16>()
                        && (4_000..=20_000).contains(&value)
                    {
                        preferences.audio_lpf_cutoff_hz = value;
                    }
                }
                "sg1000_audio_hpf_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.sg1000_audio_hpf_enabled = value;
                    }
                }
                "sg1000_audio_hpf_cutoff_hz" => {
                    if let Ok(value) = value.trim().parse::<u16>()
                        && (5..=500).contains(&value)
                    {
                        preferences.sg1000_audio_hpf_cutoff_hz = value;
                    }
                }
                "sg1000_audio_lpf_enabled" => {
                    if let Ok(value) = value.trim().parse() {
                        preferences.sg1000_audio_lpf_enabled = value;
                    }
                }
                "sg1000_audio_lpf_cutoff_hz" => {
                    if let Ok(value) = value.trim().parse::<u16>()
                        && (4_000..=20_000).contains(&value)
                    {
                        preferences.sg1000_audio_lpf_cutoff_hz = value;
                    }
                }
                "keyboard_up_key" => {
                    preferences.keyboard_up_key = decode_key_token(value.trim()).unwrap_or_default()
                }
                "keyboard_down_key" => {
                    preferences.keyboard_down_key =
                        decode_key_token(value.trim()).unwrap_or_default()
                }
                "keyboard_left_key" => {
                    preferences.keyboard_left_key =
                        decode_key_token(value.trim()).unwrap_or_default()
                }
                "keyboard_right_key" => {
                    preferences.keyboard_right_key =
                        decode_key_token(value.trim()).unwrap_or_default()
                }
                "keyboard_button1_key" => {
                    preferences.keyboard_button1_key =
                        decode_key_token(value.trim()).unwrap_or_default()
                }
                "keyboard_button2_key" => {
                    preferences.keyboard_button2_key =
                        decode_key_token(value.trim()).unwrap_or_default()
                }
                _ => {}
            }
        }
        preferences
    }

    /// Salva as preferências em arquivo simples, sem serializar estado do hardware.
    fn save_desktop_preferences(preferences: &DesktopPreferences) -> Result<(), String> {
        let directory =
            app_data_directory().ok_or_else(|| "APPDATA não está definido".to_owned())?;
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let contents = format!(
            "region={}\nselected_system={}\nfirmware_directory={}\nfirmware_boot_enabled={}\nfirmware_brazil_selected={}\nfirmware_usa_selected={}\nfirmware_europe_selected={}\nfirmware_japan_selected={}\nfirmware_korea_selected={}\naspect={}\nshow_fps={}\nshow_status_footer={}\nfps_size={}\nsprite_limit_enabled={}\naudio_volume={}\naudio_muted={}\naudio_renderer_mode={}\naudio_dsp_preset={}\naudio_hpf_enabled={}\naudio_hpf_cutoff_hz={}\naudio_lpf_enabled={}\naudio_lpf_cutoff_hz={}\nsg1000_audio_hpf_enabled={}\nsg1000_audio_hpf_cutoff_hz={}\nsg1000_audio_lpf_enabled={}\nsg1000_audio_lpf_cutoff_hz={}\nkeyboard_up_key={}\nkeyboard_down_key={}\nkeyboard_left_key={}\nkeyboard_right_key={}\nkeyboard_button1_key={}\nkeyboard_button2_key={}\n",
            preferences.region,
            preferences.selected_system,
            preferences.firmware_directory,
            preferences.firmware_boot_enabled,
            preferences.firmware_brazil_selected,
            preferences.firmware_usa_selected,
            preferences.firmware_europe_selected,
            preferences.firmware_japan_selected,
            preferences.firmware_korea_selected,
            preferences.aspect,
            preferences.show_fps,
            preferences.show_status_footer,
            preferences.fps_size,
            preferences.sprite_limit_enabled,
            preferences.audio_volume,
            preferences.audio_muted,
            preferences.audio_renderer_mode,
            preferences.audio_dsp_preset,
            preferences.audio_hpf_enabled,
            preferences.audio_hpf_cutoff_hz,
            preferences.audio_lpf_enabled,
            preferences.audio_lpf_cutoff_hz,
            preferences.sg1000_audio_hpf_enabled,
            preferences.sg1000_audio_hpf_cutoff_hz,
            preferences.sg1000_audio_lpf_enabled,
            preferences.sg1000_audio_lpf_cutoff_hz,
            encode_key_token(&preferences.keyboard_up_key),
            encode_key_token(&preferences.keyboard_down_key),
            encode_key_token(&preferences.keyboard_left_key),
            encode_key_token(&preferences.keyboard_right_key),
            encode_key_token(&preferences.keyboard_button1_key),
            encode_key_token(&preferences.keyboard_button2_key)
        );
        fs::write(directory.join("slint_ui_settings.txt"), contents)
            .map_err(|error| error.to_string())
    }

    /// Encodes each key character as a hexadecimal Unicode code point for safe text-file persistence.
    fn encode_key_token(value: &str) -> String {
        value
            .chars()
            .map(|character| format!("{:X}", character as u32))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Decodes a persisted key token, rejecting invalid Unicode values and empty entries.
    fn decode_key_token(value: &str) -> Option<String> {
        let decoded = value
            .split(',')
            .map(|part| u32::from_str_radix(part, 16).ok().and_then(char::from_u32))
            .collect::<Option<String>>()?;
        (!decoded.is_empty()).then_some(decoded)
    }

    /// Returns whether a character belongs to a Unicode private-use area.
    /// Slint uses private-use code points to represent non-printable keys.
    fn is_private_use_character(character: char) -> bool {
        matches!(
            character as u32,
            0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD
        )
    }

    /// Returns a readable key label without exposing control or private-use characters.
    fn keyboard_key_label(value: &str) -> String {
        let mut chars = value.chars();
        let Some(character) = chars.next() else {
            return "Não atribuída".to_owned();
        };
        if chars.next().is_some() || character.is_control() || is_private_use_character(character) {
            "Tecla especial".to_owned()
        } else {
            character.to_uppercase().collect()
        }
    }

    /// Carrega até dez ROMs recentes, removendo entradas duplicadas e vazias.
    fn load_recent_roms() -> Vec<PathBuf> {
        let Some(directory) = app_data_directory() else {
            return Vec::new();
        };
        let Ok(contents) = fs::read_to_string(directory.join("recent_roms.txt")) else {
            return Vec::new();
        };
        let mut recent = Vec::new();
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            let path = PathBuf::from(line);
            if !recent.contains(&path) {
                recent.push(path);
            }
            if recent.len() == MAX_RECENT_ROMS {
                break;
            }
        }
        recent
    }

    /// Persiste a lista recente, mantendo a ROM mais nova no início.
    fn save_recent_roms(recent: &[PathBuf]) -> Result<(), String> {
        let directory =
            app_data_directory().ok_or_else(|| "APPDATA não está definido".to_owned())?;
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let contents = recent
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(directory.join("recent_roms.txt"), contents).map_err(|error| error.to_string())
    }

    /// Move uma ROM para o início da lista e limita o histórico a dez entradas.
    fn record_recent_rom(recent: &mut Vec<PathBuf>, path: PathBuf) {
        recent.retain(|entry| entry != &path);
        recent.insert(0, path);
        recent.truncate(MAX_RECENT_ROMS);
        if let Err(error) = save_recent_roms(recent) {
            diagnostic_log!(warn, "R2_PREFERENCES recent_roms_save_failed error={error}");
        }
    }

    /// Converte a lista de caminhos em um modelo imutável para o menu Slint.
    fn recent_roms_model(recent: &[PathBuf]) -> Vec<slint::SharedString> {
        recent
            .iter()
            .map(|path| path.display().to_string().into())
            .collect()
    }

    /// Inicializa o renderer Slint sobre WGPU 30, restringindo a instância ao backend Vulkan.
    fn select_vulkan_backend() -> Result<(), Box<dyn Error>> {
        let mut settings = WGPUSettings::default();
        settings.backends = wgpu::Backends::VULKAN;
        settings.power_preference = wgpu::PowerPreference::HighPerformance;

        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("femtovg-wgpu".into())
            .require_wgpu_30(WGPUConfiguration::Automatic(settings))
            .select()?;

        Ok(())
    }

    /// Retorna a extensão 2D correspondente ao frame efetivamente publicado.
    fn texture_extent(width: u32, height: u32) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        }
    }

    /// Descreve o layout tightly-packed RGBA8 para as dimensões do frame.
    fn texture_data_layout(width: u32, height: u32) -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        }
    }

    /// Converte um frame imutável RGB888 em RGBA8, respeitando o stride de cada linha.
    fn convert_rgb888_to_rgba8(frame: &VideoFrame) -> Result<Vec<u8>, String> {
        if frame.pixel_format() != PixelFormat::Rgb888 {
            return Err(format!(
                "formato de frame incompatível: esperado RGB888, recebido {:?}",
                frame.pixel_format()
            ));
        }

        let width = usize::try_from(frame.width())
            .map_err(|_| "largura do frame não cabe em usize".to_owned())?;
        let height = usize::try_from(frame.height())
            .map_err(|_| "altura do frame não cabe em usize".to_owned())?;
        let output_len = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| "dimensões do frame excedem o limite de memória".to_owned())?;
        let expected_row_bytes = width
            .checked_mul(3)
            .ok_or_else(|| "largura do frame excede o limite de memória".to_owned())?;

        let mut rgba = Vec::with_capacity(output_len);
        for y in 0..frame.height() {
            let row = frame
                .row(y)
                .ok_or_else(|| format!("linha {y} ausente no frame"))?;
            if row.len() != expected_row_bytes {
                return Err(format!(
                    "linha {y} possui {} bytes; esperado {expected_row_bytes}",
                    row.len()
                ));
            }
            for rgb in row.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }

        if rgba.len() != output_len {
            return Err(format!(
                "conversão produziu {} bytes; esperado {output_len}",
                rgba.len()
            ));
        }
        Ok(rgba)
    }

    /// Cria uma textura importável pelo Slint, usando o device e queue do próprio renderer.
    fn create_imported_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) -> Result<slint::Image, Box<dyn Error>> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sega-r2-sms-vdp-frame"),
            size: texture_extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let initial_pixels = vec![0_u8; (width * height * 4) as usize];
        queue.write_texture(
            texture.as_image_copy(),
            &initial_pixels,
            texture_data_layout(width, height),
            texture_extent(width, height),
        );

        Ok(slint::Image::try_from(texture)?)
    }

    /// Registra o device/queue WGPU do Slint e publica a textura externa na mesma cena.
    fn attach_gpu_texture(
        app: &MainWindow,
        gpu_context_slot: Arc<Mutex<Option<GpuFrameContext>>>,
        mailbox: FrameMailbox,
        setup_counter: Rc<Cell<u64>>,
        teardown_counter: Rc<Cell<u64>>,
    ) -> Result<(), Box<dyn Error>> {
        let app_weak = app.as_weak();

        app.window().set_rendering_notifier(move |state, graphics_api| {
            match (state, graphics_api) {
                (
                    RenderingState::RenderingSetup,
                    GraphicsAPI::WGPU30 { device, queue, .. },
                ) => {
                    let setup_count = setup_counter.get().wrapping_add(1);
                    setup_counter.set(setup_count);

                    let adapter_info = device.adapter_info();
                    diagnostic_log!(
                        "R2_LIFECYCLE event=RenderingSetup count={setup_count} backend={:?} adapter={}",
                        adapter_info.backend, adapter_info.name
                    );

                    if adapter_info.backend != wgpu::Backend::Vulkan {
                        let app_weak = app_weak.clone();
                        let status = format!(
                            "FALHA: backend esperado Vulkan; recebido {:?} ({})",
                            adapter_info.backend, adapter_info.name
                        );
                        Timer::single_shot(Duration::ZERO, move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_gpu_status(status.into());
                            }
                        });
                        return;
                    }

                    match create_imported_texture(device, queue, SMS_WIDTH, SMS_HEIGHT) {
                        Ok(image) => {
                            *gpu_context_slot.lock().unwrap() = Some(GpuFrameContext {
                                device: device.clone(),
                                queue: queue.clone(),
                                texture_width: SMS_WIDTH,
                                texture_height: SMS_HEIGHT,
                            });

                            let app_weak = app_weak.clone();
                            let mailbox = mailbox.clone();
                            let status = format!(
                                "PASS API: Vulkan | adapter: {} | Slint/WGPU 30 compartilham device/queue",
                                adapter_info.name
                            );
                            let lifecycle_status = format!(
                                "Renderer setup: {setup_count} | teardown: {}",
                                teardown_counter.get()
                            );

                            // Rendering notifiers run while the Winit adapter is borrowed.
                            // Defer Slint property mutations until the notifier has returned.
                            Timer::single_shot(Duration::ZERO, move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_gpu_texture(image);
                                    app.set_gpu_status(status.into());
                                    app.set_lifecycle_status(lifecycle_status.into());
                                    if let Err(error) = mailbox.notify_if_pending() {
                                        diagnostic_log!(
                                            error,
                                            "R2_FRAME_NOTIFY renderer_recovery_failed error={error}"
                                        );
                                    }
                                }
                            });
                        }
                        Err(error) => {
                            diagnostic_log!(error, "R2_LIFECYCLE event=TextureImportFailed error={error}");
                            let app_weak = app_weak.clone();
                            let status = format!("FALHA ao importar textura WGPU: {error}");
                            Timer::single_shot(Duration::ZERO, move || {
                                if let Some(app) = app_weak.upgrade() {
                                    app.set_gpu_status(status.into());
                                }
                            });
                        }
                    }
                }
                (RenderingState::RenderingTeardown, _) => {
                    let teardown_count = teardown_counter.get().wrapping_add(1);
                    teardown_counter.set(teardown_count);
                    diagnostic_log!(
                        "R2_LIFECYCLE event=RenderingTeardown count={teardown_count}"
                    );

                    *gpu_context_slot.lock().unwrap() = None;

                    let app_weak = app_weak.clone();
                    let lifecycle_status = format!(
                        "Renderer setup: {} | teardown: {teardown_count}",
                        setup_counter.get()
                    );

                    // Do not mutate Slint properties from inside RenderingTeardown:
                    // Winit still holds an internal RefCell borrow at this point.
                    Timer::single_shot(Duration::ZERO, move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_gpu_texture(slint::Image::default());
                            app.set_lifecycle_status(lifecycle_status.into());
                        }
                    });
                }
                (RenderingState::RenderingSetup, _) => {
                    diagnostic_log!(
                        error,
                        "R2_LIFECYCLE event=RenderingSetupFailed reason=GraphicsAPI_not_WGPU30"
                    );
                    let app_weak = app_weak.clone();
                    Timer::single_shot(Duration::ZERO, move || {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_gpu_status(
                                "FALHA: o renderer Slint não forneceu GraphicsAPI::WGPU30.".into(),
                            );
                        }
                    });
                }                _ => {}
            }
        })?;

        Ok(())
    }

    const HOST_PACING_WINDOW_CAPACITY: usize = 512;

    /// Fixed-capacity rolling distribution. Samples are appended without allocation;
    /// sorting happens only when the one-second host telemetry record is emitted.
    #[derive(Clone)]
    struct HostDurationWindow {
        values: [u64; HOST_PACING_WINDOW_CAPACITY],
        len: usize,
        next: usize,
    }

    impl Default for HostDurationWindow {
        fn default() -> Self {
            Self {
                values: [0; HOST_PACING_WINDOW_CAPACITY],
                len: 0,
                next: 0,
            }
        }
    }

    impl HostDurationWindow {
        fn push(&mut self, value_ns: u64) {
            self.values[self.next] = value_ns;
            self.next = (self.next + 1) % HOST_PACING_WINDOW_CAPACITY;
            self.len = (self.len + 1).min(HOST_PACING_WINDOW_CAPACITY);
        }

        fn percentile_ns(&self, percentile: usize) -> u64 {
            if self.len == 0 {
                return 0;
            }
            let mut sorted = self.values;
            sorted[..self.len].sort_unstable();
            let rank = (self.len * percentile).div_ceil(100).max(1) - 1;
            sorted[rank.min(self.len - 1)]
        }

        const fn len(&self) -> usize {
            self.len
        }
    }

    /// Cumulative snapshot from the host PCM queue. Window rates are derived from deltas.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct AudioOutputSnapshot {
        sample_rate_hz: u32,
        queue_capacity_frames: u64,
        queue_queued_frames: u64,
        queue_high_water_frames: u64,
        accepted_frames: u64,
        consumed_frames: u64,
        dropped_frames: u64,
        dropped_blocks: u64,
        underrun_frames: u64,
        underrun_events: u64,
        flushed_frames: u64,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct AudioOutputDelta {
        accepted_frames: u64,
        consumed_frames: u64,
        dropped_frames: u64,
        dropped_blocks: u64,
        underrun_frames: u64,
        underrun_events: u64,
        flushed_frames: u64,
        counter_reset: bool,
    }

    impl AudioOutputSnapshot {
        /// Calculate monotonic counter deltas; if a reset/configuration change is observed,
        /// treat the current snapshot as the beginning of a new measurement epoch.
        fn delta_since(self, previous: Self) -> AudioOutputDelta {
            let counter_reset = self.sample_rate_hz != previous.sample_rate_hz
                || self.queue_capacity_frames != previous.queue_capacity_frames
                || self.accepted_frames < previous.accepted_frames
                || self.consumed_frames < previous.consumed_frames
                || self.dropped_frames < previous.dropped_frames
                || self.dropped_blocks < previous.dropped_blocks
                || self.underrun_frames < previous.underrun_frames
                || self.underrun_events < previous.underrun_events
                || self.flushed_frames < previous.flushed_frames;
            let delta = |current: u64, prior: u64| {
                if counter_reset {
                    current
                } else {
                    current.saturating_sub(prior)
                }
            };

            AudioOutputDelta {
                accepted_frames: delta(self.accepted_frames, previous.accepted_frames),
                consumed_frames: delta(self.consumed_frames, previous.consumed_frames),
                dropped_frames: delta(self.dropped_frames, previous.dropped_frames),
                dropped_blocks: delta(self.dropped_blocks, previous.dropped_blocks),
                underrun_frames: delta(self.underrun_frames, previous.underrun_frames),
                underrun_events: delta(self.underrun_events, previous.underrun_events),
                flushed_frames: delta(self.flushed_frames, previous.flushed_frames),
                counter_reset,
            }
        }
    }

    impl AudioOutputDelta {
        /// Every requested stereo frame was either consumed from the FIFO or zero-filled.
        fn requested_frames(self) -> u64 {
            self.consumed_frames.saturating_add(self.underrun_frames)
        }

        /// Percentage of requested output frames replaced with digital silence.
        fn zero_fill_percent(self) -> f64 {
            let requested = self.requested_frames();
            if requested == 0 {
                0.0
            } else {
                self.underrun_frames as f64 * 100.0 / requested as f64
            }
        }

        /// A first sample or reset epoch is not a valid measurement window.
        fn zero_fill_percent_for_window(self, rate_valid: bool) -> f64 {
            if rate_valid {
                self.zero_fill_percent()
            } else {
                0.0
            }
        }
    }

    /// Bounded host-side measurements for mailbox consumption and WGPU upload.
    ///
    /// This deliberately stops at queue.write_texture: Slint owns the Winit surface
    /// and physical present operation, so this host cannot claim a GPU-present timestamp.
    struct HostFramePacingWindow {
        started: Instant,
        qpc_clock: Option<sega_video_runtime::diagnostics::QpcClock>,
        last_wakeup: Option<Instant>,
        last_consumed: Option<Instant>,
        last_sequence: u64,
        last_fps_uploaded: u64,
        last_fps_sample: Instant,
        uploaded: u64,
        last_audio_snapshot: Option<AudioOutputSnapshot>,
        last_audio_snapshot_at: Option<Instant>,
        wakeups: u64,
        empty_wakeups: u64,
        consumed: u64,
        sequence_gaps: u64,
        wake_interval_sum_ns: u128,
        wake_interval_max_ns: u64,
        consumed_interval_sum_ns: u128,
        consumed_interval_max_ns: u64,
        mailbox_residence_count: u64,
        mailbox_residence_sum_ns: u128,
        mailbox_residence_max_ns: u64,
        conversion_count: u64,
        conversion_sum_ns: u128,
        conversion_max_ns: u64,
        queue_write_count: u64,
        queue_write_sum_ns: u128,
        queue_write_max_ns: u64,
        callback_count: u64,
        callback_sum_ns: u128,
        callback_max_ns: u64,
        mailbox_take_count: u64,
        mailbox_take_sum_ns: u128,
        mailbox_take_max_ns: u64,
        wake_interval_distribution: HostDurationWindow,
        consumed_interval_distribution: HostDurationWindow,
        mailbox_residence_distribution: HostDurationWindow,
        conversion_distribution: HostDurationWindow,
        queue_write_distribution: HostDurationWindow,
        callback_distribution: HostDurationWindow,
        mailbox_take_distribution: HostDurationWindow,
    }

    impl HostFramePacingWindow {
        fn new() -> Self {
            Self {
                started: Instant::now(),
                qpc_clock: sega_video_runtime::diagnostics::QpcClock::new(),
                last_wakeup: None,
                last_consumed: None,
                last_sequence: 0,
                last_fps_uploaded: 0,
                last_fps_sample: Instant::now(),
                uploaded: 0,
                last_audio_snapshot: None,
                last_audio_snapshot_at: None,
                wakeups: 0,
                empty_wakeups: 0,
                consumed: 0,
                sequence_gaps: 0,
                wake_interval_sum_ns: 0,
                wake_interval_max_ns: 0,
                consumed_interval_sum_ns: 0,
                consumed_interval_max_ns: 0,
                mailbox_residence_count: 0,
                mailbox_residence_sum_ns: 0,
                mailbox_residence_max_ns: 0,
                conversion_count: 0,
                conversion_sum_ns: 0,
                conversion_max_ns: 0,
                queue_write_count: 0,
                queue_write_sum_ns: 0,
                queue_write_max_ns: 0,
                callback_count: 0,
                callback_sum_ns: 0,
                callback_max_ns: 0,
                mailbox_take_count: 0,
                mailbox_take_sum_ns: 0,
                mailbox_take_max_ns: 0,
                wake_interval_distribution: HostDurationWindow::default(),
                consumed_interval_distribution: HostDurationWindow::default(),
                mailbox_residence_distribution: HostDurationWindow::default(),
                conversion_distribution: HostDurationWindow::default(),
                queue_write_distribution: HostDurationWindow::default(),
                callback_distribution: HostDurationWindow::default(),
                mailbox_take_distribution: HostDurationWindow::default(),
            }
        }

        fn record_wakeup(&mut self, now: Instant) {
            self.wakeups = self.wakeups.saturating_add(1);
            if let Some(previous) = self.last_wakeup {
                let elapsed = now
                    .saturating_duration_since(previous)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                self.wake_interval_sum_ns = self
                    .wake_interval_sum_ns
                    .saturating_add(u128::from(elapsed));
                self.wake_interval_max_ns = self.wake_interval_max_ns.max(elapsed);
                self.wake_interval_distribution.push(elapsed);
            }
            self.last_wakeup = Some(now);
        }

        fn record_empty_wakeup(&mut self) {
            self.empty_wakeups = self.empty_wakeups.saturating_add(1);
        }

        fn record_frame(&mut self, now: Instant, sequence: u64, published_at: Instant) {
            self.consumed = self.consumed.saturating_add(1);
            if let Some(previous) = self.last_consumed {
                let elapsed = now
                    .saturating_duration_since(previous)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64;
                self.consumed_interval_sum_ns = self
                    .consumed_interval_sum_ns
                    .saturating_add(u128::from(elapsed));
                self.consumed_interval_max_ns = self.consumed_interval_max_ns.max(elapsed);
                self.consumed_interval_distribution.push(elapsed);
            }
            if self.last_sequence > 0 && sequence > self.last_sequence.saturating_add(1) {
                self.sequence_gaps = self
                    .sequence_gaps
                    .saturating_add(sequence - self.last_sequence - 1);
            }
            let residence = now
                .saturating_duration_since(published_at)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            self.mailbox_residence_count = self.mailbox_residence_count.saturating_add(1);
            self.mailbox_residence_sum_ns = self
                .mailbox_residence_sum_ns
                .saturating_add(u128::from(residence));
            self.mailbox_residence_max_ns = self.mailbox_residence_max_ns.max(residence);
            self.mailbox_residence_distribution.push(residence);
            self.last_sequence = sequence;
            self.last_consumed = Some(now);
        }

        /// Count a successful WGPU queue upload; this is not a physical present.
        fn record_upload(&mut self, now: Instant) -> Option<f64> {
            self.uploaded = self.uploaded.saturating_add(1);
            let elapsed = now.saturating_duration_since(self.last_fps_sample);
            if elapsed < Duration::from_millis(500) {
                return None;
            }

            let uploaded = self.uploaded.saturating_sub(self.last_fps_uploaded);
            self.last_fps_uploaded = self.uploaded;
            self.last_fps_sample = now;
            Some(uploaded as f64 / elapsed.as_secs_f64())
        }

        fn record_conversion(&mut self, elapsed: Duration) {
            self.conversion_count = self.conversion_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.conversion_sum_ns = self.conversion_sum_ns.saturating_add(u128::from(nanos));
            self.conversion_max_ns = self.conversion_max_ns.max(nanos);
            self.conversion_distribution.push(nanos);
        }

        fn record_queue_write(&mut self, elapsed: Duration) {
            self.queue_write_count = self.queue_write_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.queue_write_sum_ns = self.queue_write_sum_ns.saturating_add(u128::from(nanos));
            self.queue_write_max_ns = self.queue_write_max_ns.max(nanos);
            self.queue_write_distribution.push(nanos);
        }

        fn record_callback(&mut self, elapsed: Duration) {
            self.callback_count = self.callback_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.callback_sum_ns = self.callback_sum_ns.saturating_add(u128::from(nanos));
            self.callback_max_ns = self.callback_max_ns.max(nanos);
            self.callback_distribution.push(nanos);
        }

        fn record_mailbox_take(&mut self, elapsed: Duration) {
            self.mailbox_take_count = self.mailbox_take_count.saturating_add(1);
            let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
            self.mailbox_take_sum_ns = self.mailbox_take_sum_ns.saturating_add(u128::from(nanos));
            self.mailbox_take_max_ns = self.mailbox_take_max_ns.max(nanos);
            self.mailbox_take_distribution.push(nanos);
        }

        fn report_if_due(&mut self, worker_slot: &Rc<RefCell<Option<DesktopSession>>>) {
            let elapsed = self.started.elapsed();
            if elapsed < Duration::from_secs(1) {
                return;
            }

            let (runtime, replaced_frames, machine_clock_hz) = {
                let guard = worker_slot.borrow();
                guard.as_ref().map_or(
                    (sega_sms_runtime::SmsFramePacingMetrics::default(), 0, 0),
                    |worker| {
                        (
                            worker.sms_frame_pacing_metrics(),
                            worker.video_replaced_frames(),
                            worker.machine_clock_hz(),
                        )
                    },
                )
            };
            let audio_output = {
                let guard = worker_slot.borrow();
                guard.as_ref().map(|worker| {
                    let metrics = worker.audio_output_metrics();
                    (
                        AudioOutputSnapshot {
                            sample_rate_hz: metrics.sample_rate_hz,
                            queue_capacity_frames: metrics.queue_capacity_frames,
                            queue_queued_frames: metrics.queue_queued_frames,
                            queue_high_water_frames: metrics.queue_high_water_frames,
                            accepted_frames: metrics.accepted_frames,
                            consumed_frames: metrics.consumed_frames,
                            dropped_frames: metrics.dropped_frames,
                            dropped_blocks: metrics.dropped_blocks,
                            underrun_frames: metrics.underrun_frames,
                            underrun_events: metrics.underrun_events,
                            flushed_frames: metrics.flushed_frames,
                        },
                        worker.audio_output_error(),
                    )
                })
            };
            let audio_window = if let Some((audio, _)) = audio_output.as_ref() {
                let now = Instant::now();
                let previous = self.last_audio_snapshot.zip(self.last_audio_snapshot_at);
                let window = previous.map(|(previous, previous_at)| {
                    (
                        (*audio).delta_since(previous),
                        now.saturating_duration_since(previous_at),
                    )
                });
                self.last_audio_snapshot = Some(*audio);
                self.last_audio_snapshot_at = Some(now);
                window
            } else {
                self.last_audio_snapshot = None;
                self.last_audio_snapshot_at = None;
                None
            };
            // QPC shares the timestamp domain used by PresentMon --qpc_time.
            // This marker is diagnostic only and never participates in frame pacing.
            let (qpc_ticks, qpc_frequency, qpc_available) = self
                .qpc_clock
                .as_ref()
                .and_then(|clock| clock.now().map(|ticks| (ticks, clock.frequency())))
                .map_or((0, 0, false), |(ticks, frequency)| (ticks, frequency, true));
            let average_us = |total_ns: u128, count: u64| -> u64 {
                if count == 0 {
                    0
                } else {
                    (total_ns / u128::from(count) / 1_000).min(u128::from(u64::MAX)) as u64
                }
            };
            diagnostic_log!(
                debug,
                "R2_FRAME_PACING_HOST qpc_available={} qpc_ticks={} qpc_frequency={} window_ms={} wakeups={} empty_wakeups={} consumed={} uploaded={} sequence_gaps={} wake_interval_avg_us={} wake_interval_max_us={} wake_interval_samples={} wake_interval_p50_us={} wake_interval_p95_us={} wake_interval_p99_us={} wake_interval_jitter_p95_us={} consumed_interval_avg_us={} consumed_interval_max_us={} consumed_interval_samples={} consumed_interval_p50_us={} consumed_interval_p95_us={} consumed_interval_p99_us={} consumed_interval_jitter_p95_us={} mailbox_residence_avg_us={} mailbox_residence_max_us={} mailbox_residence_samples={} mailbox_residence_p50_us={} mailbox_residence_p95_us={} mailbox_residence_p99_us={} convert_avg_us={} convert_max_us={} convert_samples={} convert_p50_us={} convert_p95_us={} convert_p99_us={} queue_write_avg_us={} queue_write_max_us={} queue_write_samples={} queue_write_p50_us={} queue_write_p95_us={} queue_write_p99_us={} callback_avg_us={} callback_max_us={} callback_samples={} callback_p50_us={} callback_p95_us={} callback_p99_us={} mailbox_take_avg_us={} mailbox_take_max_us={} mailbox_take_samples={} mailbox_take_p50_us={} mailbox_take_p95_us={} mailbox_take_p99_us={} mailbox_replaced_total={} worker_samples={} worker_total_samples={} worker_frame_interval_p50_us={} worker_frame_interval_p95_us={} worker_frame_interval_p99_us={} worker_frame_interval_jitter_p95_us={} machine_ticks_per_frame_min={} machine_ticks_per_frame_p50={} machine_ticks_per_frame_max={} worker_step_p50_us={} worker_step_p95_us={} worker_step_p99_us={} video_publish_p95_us={} audio_submit_p95_us={} frame_processing_p50_us={} frame_processing_p95_us={} frame_processing_p99_us={} pacing_wait_budget_p50_us={} pacing_wait_budget_p95_us={} pacing_wait_budget_p99_us={} pacing_wait_elapsed_p50_us={} pacing_wait_elapsed_p95_us={} pacing_wait_elapsed_p99_us={} frame_budget_p50_us={} frame_budget_p95_us={} frame_budget_p99_us={} psg_samples_p50={} psg_samples_p95={} psg_samples_p99={} pacing_timeout_lateness_p50_us={} pacing_timeout_lateness_p95_us={} pacing_timeout_lateness_p99_us={} pacing_timeout_count_total={} pacing_command_wakeups_total={} pacing_command_remaining_p95_us={} command_handler_p95_us={} machine_clock_hz={} physical_present_timestamp=unavailable_slint_owned",
                u8::from(qpc_available),
                qpc_ticks,
                qpc_frequency,
                elapsed.as_millis(),
                self.wakeups,
                self.empty_wakeups,
                self.consumed,
                self.uploaded,
                self.sequence_gaps,
                average_us(self.wake_interval_sum_ns, self.wakeups.saturating_sub(1)),
                self.wake_interval_max_ns / 1_000,
                self.wake_interval_distribution.len(),
                self.wake_interval_distribution.percentile_ns(50) / 1_000,
                self.wake_interval_distribution.percentile_ns(95) / 1_000,
                self.wake_interval_distribution.percentile_ns(99) / 1_000,
                self.wake_interval_distribution
                    .percentile_ns(95)
                    .saturating_sub(self.wake_interval_distribution.percentile_ns(50))
                    / 1_000,
                average_us(
                    self.consumed_interval_sum_ns,
                    self.consumed.saturating_sub(1)
                ),
                self.consumed_interval_max_ns / 1_000,
                self.consumed_interval_distribution.len(),
                self.consumed_interval_distribution.percentile_ns(50) / 1_000,
                self.consumed_interval_distribution.percentile_ns(95) / 1_000,
                self.consumed_interval_distribution.percentile_ns(99) / 1_000,
                self.consumed_interval_distribution
                    .percentile_ns(95)
                    .saturating_sub(self.consumed_interval_distribution.percentile_ns(50))
                    / 1_000,
                average_us(self.mailbox_residence_sum_ns, self.mailbox_residence_count),
                self.mailbox_residence_max_ns / 1_000,
                self.mailbox_residence_distribution.len(),
                self.mailbox_residence_distribution.percentile_ns(50) / 1_000,
                self.mailbox_residence_distribution.percentile_ns(95) / 1_000,
                self.mailbox_residence_distribution.percentile_ns(99) / 1_000,
                average_us(self.conversion_sum_ns, self.conversion_count),
                self.conversion_max_ns / 1_000,
                self.conversion_distribution.len(),
                self.conversion_distribution.percentile_ns(50) / 1_000,
                self.conversion_distribution.percentile_ns(95) / 1_000,
                self.conversion_distribution.percentile_ns(99) / 1_000,
                average_us(self.queue_write_sum_ns, self.queue_write_count),
                self.queue_write_max_ns / 1_000,
                self.queue_write_distribution.len(),
                self.queue_write_distribution.percentile_ns(50) / 1_000,
                self.queue_write_distribution.percentile_ns(95) / 1_000,
                self.queue_write_distribution.percentile_ns(99) / 1_000,
                average_us(self.callback_sum_ns, self.callback_count),
                self.callback_max_ns / 1_000,
                self.callback_distribution.len(),
                self.callback_distribution.percentile_ns(50) / 1_000,
                self.callback_distribution.percentile_ns(95) / 1_000,
                self.callback_distribution.percentile_ns(99) / 1_000,
                average_us(self.mailbox_take_sum_ns, self.mailbox_take_count),
                self.mailbox_take_max_ns / 1_000,
                self.mailbox_take_distribution.len(),
                self.mailbox_take_distribution.percentile_ns(50) / 1_000,
                self.mailbox_take_distribution.percentile_ns(95) / 1_000,
                self.mailbox_take_distribution.percentile_ns(99) / 1_000,
                replaced_frames,
                runtime.sample_count,
                runtime.total_samples,
                runtime.frame_interval_p50_ns / 1_000,
                runtime.frame_interval_p95_ns / 1_000,
                runtime.frame_interval_p99_ns / 1_000,
                runtime
                    .frame_interval_p95_ns
                    .saturating_sub(runtime.frame_interval_p50_ns)
                    / 1_000,
                runtime.machine_ticks_per_frame_min,
                runtime.machine_ticks_per_frame_p50,
                runtime.machine_ticks_per_frame_max,
                runtime.step_frame_p50_ns / 1_000,
                runtime.step_frame_p95_ns / 1_000,
                runtime.step_frame_p99_ns / 1_000,
                runtime.video_publish_p95_ns / 1_000,
                runtime.audio_submit_p95_ns / 1_000,
                runtime.frame_processing_p50_ns / 1_000,
                runtime.frame_processing_p95_ns / 1_000,
                runtime.frame_processing_p99_ns / 1_000,
                runtime.pacing_wait_budget_p50_ns / 1_000,
                runtime.pacing_wait_budget_p95_ns / 1_000,
                runtime.pacing_wait_budget_p99_ns / 1_000,
                runtime.pacing_wait_elapsed_p50_ns / 1_000,
                runtime.pacing_wait_elapsed_p95_ns / 1_000,
                runtime.pacing_wait_elapsed_p99_ns / 1_000,
                runtime.frame_budget_p50_ns / 1_000,
                runtime.frame_budget_p95_ns / 1_000,
                runtime.frame_budget_p99_ns / 1_000,
                runtime.audio_samples_p50,
                runtime.audio_samples_p95,
                runtime.audio_samples_p99,
                runtime.pacing_timeout_lateness_p50_ns / 1_000,
                runtime.pacing_timeout_lateness_p95_ns / 1_000,
                runtime.pacing_timeout_lateness_p99_ns / 1_000,
                runtime.pacing_timeout_count_total,
                runtime.pacing_command_wakeups_total,
                runtime.pacing_command_remaining_p95_ns / 1_000,
                runtime.command_handler_p95_ns / 1_000,
                machine_clock_hz,
            );

            if let Some((audio, output_error)) = audio_output {
                let (delta, window_ms, window_valid) = audio_window.map_or(
                    (AudioOutputDelta::default(), 0_u64, false),
                    |(delta, elapsed)| {
                        (
                            delta,
                            elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
                            true,
                        )
                    },
                );
                let requested_frames = delta.requested_frames();
                let requested_frames_total =
                    audio.consumed_frames.saturating_add(audio.underrun_frames);
                let zero_fill_percent_total = if requested_frames_total == 0 {
                    0.0
                } else {
                    audio.underrun_frames as f64 * 100.0 / requested_frames_total as f64
                };
                let rate_valid = window_valid && !delta.counter_reset && window_ms > 0;
                let accepted_frames_per_second = if rate_valid {
                    delta.accepted_frames as f64 * 1_000.0 / window_ms as f64
                } else {
                    0.0
                };
                let requested_frames_per_second = if rate_valid {
                    requested_frames as f64 * 1_000.0 / window_ms as f64
                } else {
                    0.0
                };
                diagnostic_log!(
                    debug,
                    "R3_AUDIO_HEALTH configured={} sample_rate_hz={} queue_capacity_frames={} queue_queued_frames={} queue_high_water_frames={} accepted_frames_total={} consumed_frames_total={} requested_frames_total={} dropped_frames_total={} dropped_blocks_total={} underrun_frames_total={} underrun_events_total={} flushed_frames_total={} zero_fill_percent_total={:.3} window_valid={} window_ms={} counter_reset={} rate_valid={} accepted_frames_delta={} consumed_frames_delta={} requested_frames_delta={} dropped_frames_delta={} dropped_blocks_delta={} underrun_frames_delta={} underrun_events_delta={} flushed_frames_delta={} accepted_frames_per_second={:.1} requested_frames_per_second={:.1} zero_fill_percent_window={:.3} output_error={:?}",
                    u8::from(audio.sample_rate_hz > 0),
                    audio.sample_rate_hz,
                    audio.queue_capacity_frames,
                    audio.queue_queued_frames,
                    audio.queue_high_water_frames,
                    audio.accepted_frames,
                    audio.consumed_frames,
                    requested_frames_total,
                    audio.dropped_frames,
                    audio.dropped_blocks,
                    audio.underrun_frames,
                    audio.underrun_events,
                    audio.flushed_frames,
                    zero_fill_percent_total,
                    u8::from(window_valid),
                    window_ms,
                    u8::from(delta.counter_reset),
                    u8::from(rate_valid),
                    delta.accepted_frames,
                    delta.consumed_frames,
                    requested_frames,
                    delta.dropped_frames,
                    delta.dropped_blocks,
                    delta.underrun_frames,
                    delta.underrun_events,
                    delta.flushed_frames,
                    accepted_frames_per_second,
                    requested_frames_per_second,
                    delta.zero_fill_percent_for_window(rate_valid),
                    output_error,
                );
            }

            self.started = Instant::now();
            self.last_wakeup = None;
            self.last_consumed = None;
            self.last_fps_uploaded = 0;
            self.last_fps_sample = self.started;
            self.uploaded = 0;
            self.wakeups = 0;
            self.empty_wakeups = 0;
            self.consumed = 0;
            self.sequence_gaps = 0;
            self.wake_interval_sum_ns = 0;
            self.wake_interval_max_ns = 0;
            self.consumed_interval_sum_ns = 0;
            self.consumed_interval_max_ns = 0;
            self.mailbox_residence_count = 0;
            self.mailbox_residence_sum_ns = 0;
            self.mailbox_residence_max_ns = 0;
            self.conversion_count = 0;
            self.conversion_sum_ns = 0;
            self.conversion_max_ns = 0;
            self.queue_write_count = 0;
            self.queue_write_sum_ns = 0;
            self.queue_write_max_ns = 0;
            self.callback_count = 0;
            self.callback_sum_ns = 0;
            self.callback_max_ns = 0;
            self.mailbox_take_count = 0;
            self.mailbox_take_sum_ns = 0;
            self.mailbox_take_max_ns = 0;
        }
    }

    /// Records the complete UI-thread callback duration, including early-return paths.
    struct HostFrameCallbackGuard {
        host_pacing: Arc<Mutex<HostFramePacingWindow>>,
        started: Instant,
    }

    impl Drop for HostFrameCallbackGuard {
        fn drop(&mut self) {
            if host_diagnostics_enabled() {
                self.host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_callback(self.started.elapsed());
            }
        }
    }

    /// Consume and upload one latest frame after the mailbox signals publication.
    fn present_latest_frame(
        app_weak: &slint::Weak<MainWindow>,
        gpu_context_slot: &Arc<Mutex<Option<GpuFrameContext>>>,
        mailbox: &FrameMailbox,
        host_pacing: &Arc<Mutex<HostFramePacingWindow>>,
    ) {
        let callback_started = host_diagnostics_enabled().then(Instant::now);
        let _callback_guard = callback_started.map(|started| HostFrameCallbackGuard {
            host_pacing: Arc::clone(host_pacing),
            started,
        });
        if let Some(callback_started) = callback_started {
            host_pacing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .record_wakeup(callback_started);
        }

        let Some(app) = app_weak.upgrade() else {
            return;
        };
        let context = gpu_context_slot
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().cloned());
        let Some(context) = context else {
            return;
        };
        let device = context.device.clone();
        let queue = context.queue.clone();

        let mailbox_take_started = host_diagnostics_enabled().then(Instant::now);
        let take_result = mailbox.take_latest_with_timestamp();
        if let Some(started) = mailbox_take_started {
            host_pacing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .record_mailbox_take(started.elapsed());
        }
        let (frame, published_at) = match take_result {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                if host_diagnostics_enabled() {
                    host_pacing
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .record_empty_wakeup();
                }
                return;
            }
            Err(error) => {
                app.set_frame_status(format!("Erro no mailbox: {error}").into());
                return;
            }
        };

        if !is_supported_sms_frame_extent(frame.width(), frame.height()) {
            app.set_frame_status(
                format!(
                    "Frame ignorado: {}x{}; suportado: 256x192, 256x224 ou 256x240",
                    frame.width(),
                    frame.height()
                )
                .into(),
            );
            return;
        }

        if context.texture_width != frame.width() || context.texture_height != frame.height() {
            match create_imported_texture(&device, &queue, frame.width(), frame.height()) {
                Ok(image) => {
                    let mut slot = match gpu_context_slot.lock() {
                        Ok(slot) => slot,
                        Err(_) => {
                            app.set_frame_status(
                                "Contexto WGPU indisponível durante resize".into(),
                            );
                            return;
                        }
                    };
                    let Some(current) = slot.as_mut() else {
                        return;
                    };
                    if current.device != device {
                        return;
                    }
                    current.texture_width = frame.width();
                    current.texture_height = frame.height();
                    drop(slot);
                    app.set_gpu_texture(image);
                }
                Err(error) => {
                    app.set_frame_status(
                        format!("Falha ao redimensionar textura WGPU: {error}").into(),
                    );
                    return;
                }
            }
        }

        let Some(texture) = app.get_gpu_texture().to_wgpu_30_texture() else {
            app.set_frame_status("Textura WGPU indisponível após atualização de dimensões".into());
            return;
        };

        if host_diagnostics_enabled() {
            host_pacing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .record_frame(Instant::now(), frame.sequence(), published_at);
        }

        let conversion_started = host_diagnostics_enabled().then(Instant::now);
        match convert_rgb888_to_rgba8(&frame) {
            Ok(pixels) => {
                if let Some(started) = conversion_started {
                    host_pacing
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .record_conversion(started.elapsed());
                }
                let queue_write_started = host_diagnostics_enabled().then(Instant::now);
                queue.write_texture(
                    texture.as_image_copy(),
                    &pixels,
                    texture_data_layout(frame.width(), frame.height()),
                    texture_extent(frame.width(), frame.height()),
                );
                if let Some(started) = queue_write_started {
                    host_pacing
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .record_queue_write(started.elapsed());
                }
                let upload_fps = host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record_upload(Instant::now());
                if let Some(fps) = upload_fps {
                    app.set_fps_status(format!("FPS UI upload: {fps:.1}").into());
                }
                app.set_frame_status(
                    format!(
                        "VDP frame: {} | {}x{} RGB888 → RGBA8",
                        frame.sequence(),
                        frame.width(),
                        frame.height()
                    )
                    .into(),
                );
                app.window().request_redraw();
            }
            Err(error) => {
                if let Some(started) = conversion_started {
                    host_pacing
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .record_conversion(started.elapsed());
                }
                app.set_frame_status(format!("Falha ao converter frame: {error}").into());
            }
        }
    }

    /// Wake Slint only when a new frame is published; the atomic gate bounds queued callbacks.
    fn start_frame_presentation(
        app: &MainWindow,
        gpu_context_slot: Arc<Mutex<Option<GpuFrameContext>>>,
        mailbox: FrameMailbox,
        host_pacing: Arc<Mutex<HostFramePacingWindow>>,
    ) -> Result<(), sega_video_runtime::FrameMailboxError> {
        let app_weak = app.as_weak();
        let notification_pending = Arc::new(AtomicBool::new(false));
        let notifier: Arc<dyn Fn(FrameMailbox) + Send + Sync> =
            Arc::new(move |published_mailbox| {
                if notification_pending.swap(true, Ordering::AcqRel) {
                    return;
                }

                let callback_pending = Arc::clone(&notification_pending);
                let callback_app = app_weak.clone();
                let callback_gpu_context = Arc::clone(&gpu_context_slot);
                let callback_mailbox = published_mailbox;
                let callback_metrics = Arc::clone(&host_pacing);
                let result = slint::invoke_from_event_loop(move || {
                    // Clear before taking the frame. A concurrent publication either
                    // becomes the frame consumed below or queues one harmless follow-up.
                    callback_pending.store(false, Ordering::Release);
                    present_latest_frame(
                        &callback_app,
                        &callback_gpu_context,
                        &callback_mailbox,
                        &callback_metrics,
                    );
                });
                if let Err(error) = result {
                    notification_pending.store(false, Ordering::Release);
                    diagnostic_log!(warn, "R2_FRAME_NOTIFY enqueue_failed error={error}");
                }
            });

        mailbox.set_frame_available_notifier(Some(Arc::clone(&notifier)))?;
        mailbox.notify_if_pending()?;
        Ok(())
    }

    fn start_renderer_fallback_polling(
        app: &MainWindow,
        worker_slot: Rc<RefCell<Option<DesktopSession>>>,
    ) -> Timer {
        let timer = Timer::default();
        let app_weak = app.as_weak();
        timer.start(
            TimerMode::Repeated,
            RENDERER_FALLBACK_POLL_INTERVAL,
            move || {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                let renderer_fallback = worker_slot
                    .borrow()
                    .as_ref()
                    .and_then(|worker| worker.runtime_error())
                    .filter(|error| error.starts_with("H2 renderer"));
                if let Some(error) = renderer_fallback
                    && app.get_audio_renderer_mode().as_str() != "psg"
                {
                    app.set_audio_renderer_mode("psg".into());
                    app.set_audio_status(error.into());
                    app.invoke_preferences_changed();
                }
            },
        );
        timer
    }

    /// The diagnostic timer may run only after an explicit consent change succeeds.
    fn should_run_diagnostics_timer(
        consent_requested: bool,
        consent_change_succeeded: bool,
    ) -> bool {
        consent_requested && consent_change_succeeded
    }

    /// Creates the host diagnostics timer stopped; consent controls its lifecycle.
    fn create_frame_diagnostics_timer(
        app: &MainWindow,
        host_pacing: Arc<Mutex<HostFramePacingWindow>>,
        worker_slot: Rc<RefCell<Option<DesktopSession>>>,
    ) -> Timer {
        let timer = Timer::default();
        let app_weak = app.as_weak();
        timer.start(TimerMode::Repeated, HOST_DIAGNOSTICS_INTERVAL, move || {
            if app_weak.upgrade().is_none() || !host_diagnostics_enabled() {
                return;
            }
            host_pacing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .report_if_due(&worker_slot);
        });
        timer.stop();
        timer
    }

    /// Abre o diálogo nativo de seleção de arquivos do Windows para ROMs SMS.
    ///
    /// O PowerShell 5.1 e o Windows Forms fazem parte do Windows; o diálogo usa
    /// o seletor comum do Shell. O processo auxiliar não recebe entrada do usuário.
    fn pick_rom_file() -> Result<Option<PathBuf>, String> {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const SCRIPT: &str = r#"
            Add-Type -AssemblyName System.Windows.Forms
            [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
            $dialog = New-Object System.Windows.Forms.OpenFileDialog
            $dialog.Title = 'Selecionar ROM Sega 8-bit'
            $dialog.Filter = 'ROM Sega 8-bit (*.sms;*.sg;*.bin;*.zip)|*.sms;*.sg;*.bin;*.zip|Todos os arquivos (*.*)|*.*'
            $dialog.Multiselect = $false
            $dialog.CheckFileExists = $true
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::Out.Write($dialog.FileName)
            }
        "#;

        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-STA",
                "-WindowStyle",
                "Hidden",
                "-Command",
                SCRIPT,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| format!("não foi possível iniciar o seletor do Windows: {error}"))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }

        let selected = String::from_utf8_lossy(&output.stdout);
        let selected = selected.trim_end_matches(['\r', '\n']);
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PathBuf::from(selected)))
        }
    }

    /// Abre o seletor nativo de diretório do Windows para a pasta de firmware SMS.
    /// Cancelar retorna None e não modifica as preferências.
    fn pick_firmware_directory() -> Result<Option<PathBuf>, String> {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const SCRIPT: &str = r#"
            Add-Type -AssemblyName System.Windows.Forms
            [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
            $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
            $dialog.Description = 'Selecione a pasta que contém BIOS e firmwares do Sega Master System'
            $dialog.ShowNewFolderButton = $false
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::Out.Write($dialog.SelectedPath)
            }
        "#;

        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-STA",
                "-WindowStyle",
                "Hidden",
                "-Command",
                SCRIPT,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| format!("não foi possível iniciar o seletor de diretório: {error}"))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        let selected = String::from_utf8_lossy(&output.stdout);
        let selected = selected.trim_end_matches(['\r', '\n']);
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PathBuf::from(selected)))
        }
    }

    #[derive(Debug)]
    struct FirmwareScanView {
        status: String,
        results: String,
        brazil: Vec<String>,
        usa: Vec<String>,
        europe: Vec<String>,
        japan: Vec<String>,
        korea: Vec<String>,
    }

    fn firmware_id_key(id: SmsFirmwareId) -> &'static str {
        match id {
            SmsFirmwareId::JapanV21 => "JapanV21",
            SmsFirmwareId::ExportUsEuV13 => "ExportUsEuV13",
            SmsFirmwareId::AlexKiddUsEuBrazilV1990 => "AlexKiddUsEuBrazilV1990",
            SmsFirmwareId::EuropeV20 => "EuropeV20",
            SmsFirmwareId::HangOnV34 => "HangOnV34",
            SmsFirmwareId::HangOnSafariHuntV24 => "HangOnSafariHuntV24",
            SmsFirmwareId::MissileDefense3dV44 => "MissileDefense3dV44",
            SmsFirmwareId::SamsungGamBoyIiKoreaAlexKidd => "SamsungGamBoyIiKoreaAlexKidd",
            SmsFirmwareId::SonicEurope => "SonicEurope",
        }
    }

    fn firmware_id_from_key(key: &str) -> Option<SmsFirmwareId> {
        match key.trim() {
            "JapanV21" => Some(SmsFirmwareId::JapanV21),
            "ExportUsEuV13" => Some(SmsFirmwareId::ExportUsEuV13),
            "AlexKiddUsEuBrazilV1990" => Some(SmsFirmwareId::AlexKiddUsEuBrazilV1990),
            "EuropeV20" => Some(SmsFirmwareId::EuropeV20),
            "HangOnV34" => Some(SmsFirmwareId::HangOnV34),
            "HangOnSafariHuntV24" => Some(SmsFirmwareId::HangOnSafariHuntV24),
            "MissileDefense3dV44" => Some(SmsFirmwareId::MissileDefense3dV44),
            "SamsungGamBoyIiKoreaAlexKidd" => Some(SmsFirmwareId::SamsungGamBoyIiKoreaAlexKidd),
            "SonicEurope" => Some(SmsFirmwareId::SonicEurope),
            _ => None,
        }
    }

    fn firmware_console_compatible(console: SmsConsoleVariant, system: &str) -> bool {
        match system {
            "sms1" => matches!(
                console,
                SmsConsoleVariant::Sms1 | SmsConsoleVariant::Sms1AndSms2
            ),
            "sms2" => matches!(
                console,
                SmsConsoleVariant::Sms2
                    | SmsConsoleVariant::SamsungGamBoyIi
                    | SmsConsoleVariant::Sms1AndSms2
            ),
            _ => false,
        }
    }

    fn firmware_options_for_region(
        catalog: &SmsFirmwareCatalog,
        entries: &[SmsFirmwareScanEntry],
        system: &str,
        region: SmsRegion,
    ) -> Vec<String> {
        let mut options = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::Identified))
            .filter_map(|entry| entry.firmware_id)
            .filter_map(|id| catalog.descriptor(id).map(|descriptor| (id, descriptor)))
            .filter(|(_, descriptor)| firmware_console_compatible(descriptor.console, system))
            .filter(|(_, descriptor)| descriptor.supported_regions.contains(&region))
            .map(|(id, descriptor)| {
                format!(
                    "{} — {} ({} KiB)",
                    firmware_id_key(id),
                    descriptor.name,
                    descriptor.size_bytes / 1024
                )
            })
            .collect::<Vec<_>>();
        options.sort();
        options.dedup();
        options
    }

    fn scan_sms_firmware_directory(
        directory: &Path,
        system: &str,
    ) -> Result<FirmwareScanView, String> {
        let catalog = SmsFirmwareCatalog::sms_default();
        let entries = catalog
            .scan_directory(directory)
            .map_err(|error| format!("não foi possível escanear o diretório: {error}"))?;

        let identified = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::Identified))
            .count();
        let duplicates = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::DuplicateHash))
            .count();
        let unknown = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::UnknownContent))
            .count();
        let unsupported = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::UnsupportedSize))
            .count();
        let read_errors = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::ReadError(_)))
            .count();

        let mut lines = entries
            .iter()
            .filter(|entry| matches!(&entry.status, SmsFirmwareScanStatus::Identified))
            .map(|entry| {
                let filename = entry
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().replace(['\r', '\n'], " "))
                    .unwrap_or_else(|| "(arquivo sem nome)".to_owned());
                let size_kib = entry.size_bytes / 1024;
                let digest = entry
                    .sha256
                    .as_deref()
                    .map(|hash| hash.chars().take(12).collect::<String>())
                    .unwrap_or_else(|| "não calculado".to_owned());
                let descriptor = entry.firmware_id.and_then(|id| catalog.descriptor(id));
                let name = descriptor
                    .map(|item| item.name)
                    .unwrap_or("Firmware não catalogado");
                let validation = descriptor
                    .map(|item| match item.validation {
                        SmsFirmwareValidation::IdentifiedOnly => "IDENTIFICADO — BOOT NÃO VALIDADO",
                        SmsFirmwareValidation::MappingDocumentedBootPending => {
                            "MAPEAMENTO DOCUMENTADO — BOOT PENDENTE"
                        }
                        SmsFirmwareValidation::MappingUnresolvedBootPending => {
                            "MAPEAMENTO PENDENTE — BOOT NÃO VALIDADO"
                        }
                        SmsFirmwareValidation::BootConfirmed => "BOOT VALIDADO",
                    })
                    .unwrap_or("CONTEÚDO NÃO CATALOGADO");
                format!("{name} — {filename} | {size_kib} KiB | SHA-256 {digest} | {validation}")
            })
            .collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push("Nenhum firmware válido foi identificado.".to_owned());
        }
        if duplicates > 0 {
            lines.push(format!("{duplicates} arquivo(s) duplicado(s) ignorado(s)."));
        }
        if unknown > 0 {
            lines.push(format!("{unknown} arquivo(s) com conteúdo desconhecido."));
        }
        if unsupported > 0 {
            lines.push(format!(
                "{unsupported} arquivo(s) com tamanho não catalogado."
            ));
        }
        if read_errors > 0 {
            lines.push(format!(
                "{read_errors} erro(s) de leitura ou acesso a subdiretório."
            ));
        }

        Ok(FirmwareScanView {
            status: format!(
                "Varredura concluída: {} arquivos; {identified} firmware(s) identificados; {duplicates} duplicatas; {unknown} desconhecidos; {unsupported} tamanhos não suportados; {read_errors} erros.",
                entries.len()
            ),
            results: lines.join("\n"),
            brazil: firmware_options_for_region(&catalog, &entries, system, SmsRegion::Brazil),
            usa: firmware_options_for_region(&catalog, &entries, system, SmsRegion::Usa),
            europe: firmware_options_for_region(&catalog, &entries, system, SmsRegion::Europe),
            japan: firmware_options_for_region(&catalog, &entries, system, SmsRegion::Japan),
            korea: firmware_options_for_region(&catalog, &entries, system, SmsRegion::Korea),
        })
    }

    fn slint_string_model(values: Vec<String>) -> slint::ModelRc<slint::SharedString> {
        let values = values
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>();
        slint::ModelRc::new(slint::VecModel::from(values))
    }

    /// Abre o diálogo nativo do Windows para escolher explicitamente o destino do trace.
    ///
    /// O cancelamento retorna None e não cria nem altera qualquer arquivo.
    fn pick_trace_save_file() -> Result<Option<PathBuf>, String> {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const SCRIPT: &str = r#"
            Add-Type -AssemblyName System.Windows.Forms
            [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
            $dialog = New-Object System.Windows.Forms.SaveFileDialog
            $dialog.Title = 'Exportar diagnóstico do Sega Master System'
            $dialog.Filter = 'Trace SMS (*.txt)|*.txt|Todos os arquivos (*.*)|*.*'
            $dialog.DefaultExt = 'txt'
            $dialog.AddExtension = $true
            $dialog.OverwritePrompt = $true
            $dialog.FileName = 'sms-bus-trace.txt'
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::Out.Write($dialog.FileName)
            }
        "#;

        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-STA",
                "-WindowStyle",
                "Hidden",
                "-Command",
                SCRIPT,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| {
                format!("não foi possível iniciar o seletor de destino do Windows: {error}")
            })?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }

        let selected = String::from_utf8_lossy(&output.stdout);
        let selected = selected.trim_end_matches(['\r', '\n']);
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PathBuf::from(selected)))
        }
    }

    /// Read a raw Sega 8-bit ROM or a ZIP containing exactly one supported ROM entry.
    ///
    /// ZIP data is decompressed in memory only. No archive path is ever written to disk,
    /// and entry count, archive size and uncompressed ROM size are bounded.
    fn read_rom_image(path: &PathBuf) -> Result<Vec<u8>, String> {
        let is_zip = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
        let max_size = if is_zip {
            MAX_ROM_ARCHIVE_BYTES as u64
        } else {
            MAX_ROM_BYTES
        };
        let metadata = fs::metadata(path)
            .map_err(|error| format!("não foi possível consultar o arquivo ROM: {error}"))?;
        if metadata.len() > max_size {
            return Err(format!("arquivo ROM excede o limite de {max_size} bytes"));
        }

        let bytes =
            fs::read(path).map_err(|error| format!("não foi possível ler a ROM: {error}"))?;
        if is_zip {
            extract_rom_from_zip(&bytes)
        } else {
            Ok(bytes)
        }
    }

    /// Extract a single .sms/.sg/.bin entry without writing archive-controlled paths.
    fn extract_rom_from_zip(bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() > MAX_ROM_ARCHIVE_BYTES {
            return Err(format!(
                "arquivo ZIP excede o limite de {MAX_ROM_ARCHIVE_BYTES} bytes"
            ));
        }

        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| format!("arquivo ZIP inválido: {error}"))?;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err(format!(
                "arquivo ZIP contém mais de {MAX_ZIP_ENTRIES} entradas"
            ));
        }

        let mut candidates = Vec::new();
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|error| format!("não foi possível inspecionar entrada ZIP: {error}"))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|error| format!("não foi possível ler o nome da entrada ZIP: {error}"))?
                .into_owned();
            let is_rom = std::path::Path::new(&name)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("sms")
                        || extension.eq_ignore_ascii_case("sg")
                        || extension.eq_ignore_ascii_case("bin")
                });
            if is_rom {
                candidates.push((index, name, entry.size()));
            }
        }

        if candidates.is_empty() {
            return Err("ZIP não contém uma ROM .sms, .sg ou .bin.".to_owned());
        }
        if candidates.len() != 1 {
            return Err(
                "ZIP contém múltiplas ROMs; mantenha somente uma entrada .sms, .sg ou .bin."
                    .to_owned(),
            );
        }

        let (index, name, expected_size) = candidates.pop().expect("one candidate was checked");
        if expected_size == 0 || expected_size > MAX_ROM_BYTES {
            return Err(format!(
                "ROM compactada '{name}' tem tamanho inválido ou excede {MAX_ROM_BYTES} bytes"
            ));
        }

        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("não foi possível abrir ROM '{name}': {error}"))?;
        let mut rom = Vec::with_capacity(expected_size as usize);
        entry
            .by_ref()
            .take(MAX_ROM_BYTES + 1)
            .read_to_end(&mut rom)
            .map_err(|error| format!("falha ao descompactar ROM '{name}': {error}"))?;
        if rom.len() as u64 != expected_size {
            return Err(format!(
                "tamanho descompactado inconsistente para ROM '{name}'"
            ));
        }
        Ok(rom)
    }

    /// Returns whether physical audio must be bypassed for a video-only diagnostic capture.
    fn video_only_capture_requested(value: Option<&str>) -> bool {
        value.is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    }

    fn video_only_capture_enabled() -> bool {
        video_only_capture_requested(std::env::var("SEGA_EMULATOR_VIDEO_ONLY").ok().as_deref())
    }

    /// Converte volume mestre e mute no nível efetivo enviado ao DSP de saída.
    fn effective_audio_volume(volume: u8, muted: bool) -> u8 {
        if muted { 0 } else { volume.min(100) }
    }

    /// Converte as preferências da GUI em configuração DSP validada pelo runtime.
    fn audio_dsp_config(
        preset: bool,
        hpf_enabled: bool,
        hpf_cutoff: f64,
        lpf_enabled: bool,
        lpf_cutoff: f64,
    ) -> AnalogOutputConfig {
        if preset {
            return AnalogOutputConfig::sms1_experimental();
        }
        AnalogOutputConfig {
            enabled: hpf_enabled || lpf_enabled,
            high_pass_hz: hpf_enabled.then_some(hpf_cutoff),
            low_pass_hz: lpf_enabled.then_some(lpf_cutoff),
        }
    }

    /// Cria a sessão CPAL com volume e configuração DSP persistidos.
    fn configure_sms_audio(
        worker: &SmsTemporalWorker,
        volume: u8,
        dsp_config: AnalogOutputConfig,
        renderer_mode: &str,
    ) {
        let renderer = if renderer_mode == "h2-experimental" {
            AudioRendererMode::H2Experimental
        } else {
            AudioRendererMode::Psg
        };
        if let Err(error) = worker.set_audio_renderer(renderer) {
            diagnostic_log!(
                warn,
                "R3_AUDIO renderer selection failed; PSG remains the fallback: {error}"
            );
        }
        if video_only_capture_enabled() {
            diagnostic_log!(
                "R3_AUDIO skipped: SEGA_EMULATOR_VIDEO_ONLY enabled; digital PSG generation remains active"
            );
            return;
        }
        const BUFFER_FRAMES: u32 = 512;
        const QUEUE_BLOCKS: usize = 8;
        let result = worker.configure_audio_output(
            move || -> Result<Box<dyn sega_sms_runtime::AudioSession>, String> {
                let sink = sega_audio_output::NativeCpalOutput::new(BUFFER_FRAMES, QUEUE_BLOCKS)?;
                diagnostic_log!(
                    "R3_AUDIO CPAL output opened: sample_rate={} Hz, channels={}, format={:?}, default_config_fallback={}, queue_capacity={} frames",
                    sink.sample_rate_hz(),
                    sink.channels(),
                    sink.sample_format(),
                    sink.used_default_config_fallback(),
                    BUFFER_FRAMES as usize * QUEUE_BLOCKS
                );
                let mut session = sink.into_audio_session(volume)?;
                session.set_analog_output_config(dsp_config)?;
                Ok(Box::new(session))
            },
        );
        if let Err(error) = result {
            diagnostic_log!(
                warn,
                "R3_AUDIO CPAL unavailable; continuing without physical audio: {error}"
            );
        }
    }

    const DIAGNOSTIC_SRAM_CAPACITY_ENV: &str = "SEGA_SMS_DIAGNOSTIC_SRAM_BYTES";

    /// Analisa a capacidade SRAM volátil solicitada explicitamente para diagnóstico.
    /// A opção nunca cria contexto de persistência nem altera perfis do catálogo.
    fn parse_diagnostic_sram_capacity(value: Option<&str>) -> Result<Option<usize>, String> {
        let Some(value) = value else {
            return Ok(None);
        };
        let capacity = value.trim().parse::<usize>().map_err(|_| {
            format!("{DIAGNOSTIC_SRAM_CAPACITY_ENV} deve ser 8192, 16384 ou 32768 bytes")
        })?;
        if !matches!(capacity, 0x2000 | 0x4000 | 0x8000) {
            return Err(format!(
                "{DIAGNOSTIC_SRAM_CAPACITY_ENV} deve ser 8192, 16384 ou 32768 bytes; recebido {capacity}"
            ));
        }
        Ok(Some(capacity))
    }

    /// Resolve um perfil apenas pela identidade binária e carrega o SRM, se existir.
    fn prepare_cartridge(
        rom: &[u8],
        store: Arc<SaveRamStore>,
    ) -> Result<PreparedCartridge, String> {
        let identity = CartridgeIdentity::from_bytes(rom);
        let catalog = CartridgeCatalog::sms_default()
            .map_err(|error| format!("falha ao carregar catálogo SMS: {error}"))?;
        let profile = catalog
            .lookup_runtime_profile(&identity)
            .map_err(|error| format!("falha ao identificar cartucho SMS: {error}"))?
            .cloned();

        let Some(profile) = profile else {
            let diagnostic_capacity = parse_diagnostic_sram_capacity(
                std::env::var(DIAGNOSTIC_SRAM_CAPACITY_ENV).ok().as_deref(),
            )?;
            if let Some(capacity) = diagnostic_capacity {
                diagnostic_log!(
                    "SRAM_PROFILE status=diagnostic-override capacity={} persistent=false restored=false",
                    capacity
                );
                return Ok(PreparedCartridge {
                    sram_capacity: Some(capacity),
                    initial_sram: None,
                    save_context: None,
                });
            }
            diagnostic_log!("SRAM_PROFILE status=unknown");
            return Ok(PreparedCartridge {
                sram_capacity: None,
                initial_sram: None,
                save_context: None,
            });
        };

        let initial_sram = store
            .load(&identity, &profile)
            .map_err(|error| format!("não foi possível validar/restaurar o SRM: {error}"))?;
        let save_context = profile
            .requires_persistent_store()
            .then(|| CartridgeSaveContext {
                identity: identity.clone(),
                profile: profile.clone(),
                store,
            });
        diagnostic_log!(
            "SRAM_PROFILE status=verified capacity={} persistent={} restored={}",
            profile.ram_capacity_bytes().unwrap_or_default(),
            profile.requires_persistent_store(),
            initial_sram.is_some()
        );
        Ok(PreparedCartridge {
            sram_capacity: profile.ram_capacity_bytes(),
            initial_sram,
            save_context,
        })
    }

    /// Cria o worker já configurado com o perfil físico e o snapshot SRM restaurado.
    fn create_sms_worker(
        rom: Vec<u8>,
        settings: &SmsWorkerSettings,
        save_store: Arc<SaveRamStore>,
    ) -> Result<(SmsTemporalWorker, Option<CartridgeSaveContext>), String> {
        let prepared = prepare_cartridge(&rom, save_store)?;
        let bios = load_selected_sms_bios(&rom, settings)?;
        let worker = match bios {
            Some(bios) => SmsTemporalWorker::new_with_bios_and_region_preference_and_hardware_profile_and_cartridge_sram_size(
                rom,
                bios,
                &settings.region,
                settings.vdp_revision,
                prepared.sram_capacity,
                prepared.initial_sram,
            )?,
            None => SmsTemporalWorker::new_with_region_preference_and_hardware_profile_and_cartridge_sram_size(
                rom,
                &settings.region,
                settings.vdp_revision,
                prepared.sram_capacity,
                prepared.initial_sram,
            )?,
        };
        configure_sms_audio(
            &worker,
            settings.audio_volume,
            settings.dsp_config,
            &settings.renderer_mode,
        );
        // Detailed diagnostics are session opt-in. The worker and machine start
        // with tracing/telemetry disabled; the user enables them from DEBUG.
        Ok((worker, prepared.save_context))
    }

    /// Configure the shared CPAL/PCM pipeline on the SG-1000 machine-owning thread.
    fn configure_sg1000_audio(
        worker: &Sg1000TemporalWorker,
        volume: u8,
        dsp_config: AnalogOutputConfig,
    ) {
        if video_only_capture_enabled() {
            diagnostic_log!("R3_AUDIO skipped for SG-1000: SEGA_EMULATOR_VIDEO_ONLY enabled");
            return;
        }
        const BUFFER_FRAMES: u32 = 512;
        const QUEUE_BLOCKS: usize = 8;
        let result = worker.configure_audio_output(
            move || -> Result<Box<dyn sega_sms_runtime::AudioSession>, String> {
                let sink = sega_audio_output::NativeCpalOutput::new(BUFFER_FRAMES, QUEUE_BLOCKS)?;
                diagnostic_log!(
                    "R3_AUDIO SG-1000 CPAL output opened: sample_rate={} Hz, channels={}, format={:?}, default_config_fallback={}, queue_capacity={} frames",
                    sink.sample_rate_hz(),
                    sink.channels(),
                    sink.sample_format(),
                    sink.used_default_config_fallback(),
                    BUFFER_FRAMES as usize * QUEUE_BLOCKS
                );
                let mut session = sink.into_audio_session(volume)?;
                session.set_analog_output_config(dsp_config)?;
                Ok(Box::new(session))
            },
        );
        if let Err(error) = result {
            diagnostic_log!(
                warn,
                "R3_AUDIO SG-1000 CPAL unavailable; continuing without physical audio: {error}"
            );
        }
    }

    /// Build exactly the runtime selected by the persisted/explicit desktop system id.
    /// SMS cartridge catalog/SRAM/BIOS preparation is never applied to SG-1000.
    fn create_desktop_session(
        rom: Vec<u8>,
        settings: &SmsWorkerSettings,
        save_store: Arc<SaveRamStore>,
    ) -> Result<(DesktopSession, Option<CartridgeSaveContext>), String> {
        let system = DesktopSystem::from_id(&settings.sms_system)?;
        match system {
            DesktopSystem::Sg1000 => {
                let worker = Sg1000TemporalWorker::new_for_cartridge(rom)?;
                configure_sg1000_audio(&worker, settings.audio_volume, settings.sg1000_dsp_config);
                Ok((DesktopSession::Sg1000(worker), None))
            }
            DesktopSystem::Sms1 | DesktopSystem::Sms2 => {
                let (worker, save_context) = create_sms_worker(rom, settings, save_store)?;
                Ok((DesktopSession::Sms { worker, system }, save_context))
            }
        }
    }

    /// Persistência executada no host, após snapshot imutável obtido no worker.
    /// O Scheduler e o caminho da CPU nunca executam operações de filesystem.
    fn persist_worker_sram(
        worker: &SmsTemporalWorker,
        context: Option<&CartridgeSaveContext>,
    ) -> Result<(), String> {
        let Some(context) = context else {
            return Ok(());
        };
        let snapshot = worker
            .export_cartridge_sram()?
            .ok_or_else(|| "perfil persistente não expôs SRAM ao worker".to_owned())?;
        let store = Arc::clone(&context.store);
        let identity = context.identity.clone();
        let profile = context.profile.clone();
        let save = thread::Builder::new()
            .name("sms-sram-save".to_owned())
            .spawn(move || {
                store
                    .save(&identity, &profile, &snapshot)
                    .map_err(|error| error.to_string())
            })
            .map_err(|error| format!("não foi possível iniciar gravação SRM: {error}"))?;
        save.join()
            .map_err(|_| "thread de gravação SRM encerrou inesperadamente".to_owned())??;
        diagnostic_log!(
            "SRAM_SAVE status=written bytes={}",
            context.profile.ram_capacity_bytes().unwrap_or_default()
        );
        Ok(())
    }

    /// Persists SRAM only for an SMS session; SG-1000 has no SMS cartridge SRAM contract.
    fn persist_desktop_session_sram(
        session: &DesktopSession,
        context: Option<&CartridgeSaveContext>,
    ) -> Result<(), String> {
        match session {
            DesktopSession::Sms { worker, .. } => persist_worker_sram(worker, context),
            DesktopSession::Sg1000(_) if context.is_none() => Ok(()),
            DesktopSession::Sg1000(_) => {
                Err("SG-1000 session cannot own an SMS SRAM persistence context".to_owned())
            }
        }
    }

    /// Substitui a máquina sem permitir dois produtores simultâneos no FrameMailbox.
    fn replace_desktop_session(
        path: &PathBuf,
        worker_slot: &Rc<RefCell<Option<DesktopSession>>>,
        save_context_slot: &Rc<RefCell<Option<CartridgeSaveContext>>>,
        save_store: Arc<SaveRamStore>,
        mailbox: &FrameMailbox,
        settings: &SmsWorkerSettings,
        diagnostics_consent: Option<sega_observability::DiagnosticsConsent>,
    ) -> Result<(), String> {
        let rom = read_rom_image(path)?;
        let (next, next_save_context) = create_desktop_session(rom, settings, save_store)?;
        if let Some(consent) = diagnostics_consent.as_ref() {
            next.enable_sms_diagnostics(consent)?;
        }

        let previous = worker_slot.borrow_mut().take();
        if let Some(previous) = previous {
            let was_running = previous.is_running();
            if let Err(error) = previous.pause() {
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }
            let previous_context = save_context_slot.borrow().clone();
            if let Err(error) = persist_desktop_session_sram(&previous, previous_context.as_ref()) {
                if was_running {
                    let _ = previous.start();
                }
                *worker_slot.borrow_mut() = Some(previous);
                return Err(format!(
                    "ROM anterior preservada; falha ao gravar SRAM: {error}"
                ));
            }
            if let Err(error) = previous.detach_video_mailbox() {
                if was_running {
                    let _ = previous.start();
                }
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }
            if let Err(error) = next.attach_video_mailbox(mailbox.clone()) {
                let _ = previous.attach_video_mailbox(mailbox.clone());
                if was_running {
                    let _ = previous.start();
                }
                *worker_slot.borrow_mut() = Some(previous);
                return Err(error);
            }

            *worker_slot.borrow_mut() = Some(next);
            *save_context_slot.borrow_mut() = next_save_context;
            drop(previous);
        } else {
            next.attach_video_mailbox(mailbox.clone())?;
            *worker_slot.borrow_mut() = Some(next);
            *save_context_slot.borrow_mut() = next_save_context;
        }

        Ok(())
    }

    /// Contexto host-side que associa um worker ao arquivo SRM da ROM exata.
    #[derive(Clone)]
    struct CartridgeSaveContext {
        identity: CartridgeIdentity,
        profile: CartridgeHardwareProfile,
        store: Arc<SaveRamStore>,
    }

    struct PreparedCartridge {
        sram_capacity: Option<usize>,
        initial_sram: Option<Vec<u8>>,
        save_context: Option<CartridgeSaveContext>,
    }

    struct SmsWorkerSettings {
        region: String,
        sms_system: String,
        firmware_directory: String,
        firmware_boot_enabled: bool,
        firmware_brazil_selected: String,
        firmware_usa_selected: String,
        firmware_europe_selected: String,
        firmware_japan_selected: String,
        firmware_korea_selected: String,
        vdp_revision: SmsVdpRevision,
        audio_volume: u8,
        dsp_config: AnalogOutputConfig,
        sg1000_dsp_config: AnalogOutputConfig,
        renderer_mode: String,
    }

    /// Load only a catalog-identified conventional 8 KiB BIOS for the resolved
    /// cartridge region. Built-in-game firmware needs its own internal-ROM map.
    fn load_selected_sms_bios(
        rom: &[u8],
        settings: &SmsWorkerSettings,
    ) -> Result<Option<Vec<u8>>, String> {
        if !settings.firmware_boot_enabled {
            return Ok(None);
        }
        if settings.firmware_directory.trim().is_empty() {
            return Err(
                "boot pela BIOS está habilitado, mas o diretório de firmware não foi configurado"
                    .to_owned(),
            );
        }

        let preference = SmsRegionPreference::parse(&settings.region).map_err(str::to_owned)?;
        let region = match preference {
            SmsRegionPreference::Manual(region) => region,
            SmsRegionPreference::Automatic => match CartridgeCatalog::sms_default() {
                Ok(cartridge_catalog) => {
                    resolve_sms_region(preference, rom, &cartridge_catalog).region
                }
                Err(error) => {
                    diagnostic_log!(
                        warn,
                        "SMS_REGION preference=auto catalog_error={error}; fallback=usa"
                    );
                    SmsRegion::Usa
                }
            },
        };
        let selected = match region {
            SmsRegion::Brazil => &settings.firmware_brazil_selected,
            SmsRegion::Usa => &settings.firmware_usa_selected,
            SmsRegion::Europe => &settings.firmware_europe_selected,
            SmsRegion::Japan => &settings.firmware_japan_selected,
            SmsRegion::Korea => &settings.firmware_korea_selected,
        };
        let key = selected
            .split_once(" — ")
            .map_or(selected.as_str(), |(key, _)| key)
            .trim();
        let id = firmware_id_from_key(key).ok_or_else(|| {
            format!(
                "boot pela BIOS está habilitado, mas não há firmware selecionado para {}",
                region.label()
            )
        })?;

        let catalog = SmsFirmwareCatalog::sms_default();
        let descriptor = catalog
            .descriptor(id)
            .ok_or_else(|| format!("firmware {key} não existe no catálogo"))?;
        if descriptor.kind != SmsFirmwareKind::ConventionalBios
            || descriptor.mapping != SmsFirmwareMappingStrategy::ConventionalBios8KiB
            || descriptor.size_bytes != 0x2000
        {
            return Err(format!(
                "{} é firmware com jogo interno, não uma BIOS convencional de 8 KiB; seu mapeamento ainda não está integrado ao runtime",
                descriptor.name
            ));
        }
        if !firmware_console_compatible(descriptor.console, &settings.sms_system)
            || !descriptor.supported_regions.contains(&region)
        {
            return Err(format!(
                "{} não é compatível com {} / {}",
                descriptor.name,
                settings.sms_system,
                region.label()
            ));
        }

        let entries = catalog
            .scan_directory(&settings.firmware_directory)
            .map_err(|error| {
                format!("não foi possível revalidar o diretório de firmware: {error}")
            })?;
        let entry = entries
            .iter()
            .find(|entry| {
                entry.firmware_id == Some(id)
                    && matches!(
                        &entry.status,
                        SmsFirmwareScanStatus::Identified | SmsFirmwareScanStatus::DuplicateHash
                    )
            })
            .ok_or_else(|| {
                format!(
                    "{} não foi encontrado como firmware identificado; execute novamente a varredura",
                    descriptor.name
                )
            })?;
        let bytes = catalog
            .load_validated(&entry.path, id)
            .map_err(|error| format!("não foi possível validar {}: {error}", descriptor.name))?;
        diagnostic_log!(
            "SMS_BIOS status=validated id={} region={} bytes={}",
            firmware_id_key(id),
            region.key(),
            bytes.len()
        );
        Ok(Some(bytes))
    }

    /// Convert the user-selected console model into its explicit VDP ASIC revision.
    fn sms_vdp_revision_for_system(system: &str) -> Result<SmsVdpRevision, String> {
        match system {
            "sms1" => Ok(SmsVdpRevision::Sega3155124),
            "sms2" => Ok(SmsVdpRevision::Sega3155246),
            _ => Err(format!("unsupported SMS hardware selection: {system}")),
        }
    }

    /// Material Icons are decoded to Slint SVG images once during application startup.
    struct TransportIcons {
        play: slint::Image,
        pause: slint::Image,
        stop: slint::Image,
        reset: slint::Image,
        step_frame: slint::Image,
    }

    fn load_transport_icons() -> Result<TransportIcons, slint::LoadImageError> {
        use md_icons_classic::filled;

        Ok(TransportIcons {
            play: slint::Image::load_from_svg_data(filled::ICON_PLAY_ARROW.as_bytes())?,
            pause: slint::Image::load_from_svg_data(filled::ICON_PAUSE.as_bytes())?,
            stop: slint::Image::load_from_svg_data(filled::ICON_STOP.as_bytes())?,
            reset: slint::Image::load_from_svg_data(filled::ICON_REPLAY.as_bytes())?,
            step_frame: slint::Image::load_from_svg_data(filled::ICON_SKIP_NEXT.as_bytes())?,
        })
    }

    /// Executa o host SMS; a GUI envia somente comandos coarse-grained ao worker.
    pub fn run() -> Result<(), Box<dyn Error>> {
        let mut args = std::env::args_os().skip(1);
        let rom_path = args.next().map(PathBuf::from);
        let preferences = load_desktop_preferences();
        let region = args
            .next()
            .and_then(|value| value.into_string().ok())
            .unwrap_or_else(|| preferences.region.clone());
        let recent_roms = Rc::new(RefCell::new(load_recent_roms()));

        select_vulkan_backend()?;
        let mailbox = FrameMailbox::new();
        let save_store = cartridge_save_store()?;
        let worker_settings = SmsWorkerSettings {
            region: region.clone(),
            sms_system: preferences.selected_system.clone(),
            firmware_directory: preferences.firmware_directory.clone(),
            firmware_boot_enabled: preferences.firmware_boot_enabled,
            firmware_brazil_selected: preferences.firmware_brazil_selected.clone(),
            firmware_usa_selected: preferences.firmware_usa_selected.clone(),
            firmware_europe_selected: preferences.firmware_europe_selected.clone(),
            firmware_japan_selected: preferences.firmware_japan_selected.clone(),
            firmware_korea_selected: preferences.firmware_korea_selected.clone(),
            // SG-1000 never consumes the SMS-only VDP field in SmsWorkerSettings.
            vdp_revision: sms_vdp_revision_for_system(&preferences.selected_system)
                .unwrap_or(SmsVdpRevision::Sega3155124),
            audio_volume: effective_audio_volume(preferences.audio_volume, preferences.audio_muted),
            dsp_config: audio_dsp_config(
                preferences.audio_dsp_preset,
                preferences.audio_hpf_enabled,
                f64::from(preferences.audio_hpf_cutoff_hz),
                preferences.audio_lpf_enabled,
                f64::from(preferences.audio_lpf_cutoff_hz),
            ),
            sg1000_dsp_config: audio_dsp_config(
                false,
                preferences.sg1000_audio_hpf_enabled,
                f64::from(preferences.sg1000_audio_hpf_cutoff_hz),
                preferences.sg1000_audio_lpf_enabled,
                f64::from(preferences.sg1000_audio_lpf_cutoff_hz),
            ),
            renderer_mode: preferences.audio_renderer_mode.clone(),
        };
        let mut initial_save_context = None;
        let worker = if let Some(path) = rom_path.as_ref() {
            let (sms_worker, context) = create_desktop_session(
                read_rom_image(path)?,
                &worker_settings,
                Arc::clone(&save_store),
            )?;
            initial_save_context = context;
            let worker = sms_worker;
            worker.attach_video_mailbox(mailbox.clone())?;
            worker.set_sprite_limit_enabled(preferences.sprite_limit_enabled)?;
            Some(worker)
        } else {
            None
        };
        if let Some(path) = rom_path.as_ref() {
            record_recent_rom(&mut recent_roms.borrow_mut(), path.clone());
        }
        let worker = Rc::new(RefCell::new(worker));
        let diagnostics_consent_slot =
            Rc::new(RefCell::new(None::<sega_observability::DiagnosticsConsent>));
        let save_context = Rc::new(RefCell::new(initial_save_context));
        let recent_rom = Rc::new(RefCell::new(rom_path.clone()));

        let app = MainWindow::new()?;
        let transport_icons = load_transport_icons()?;
        app.set_play_icon(transport_icons.play);
        app.set_pause_icon(transport_icons.pause);
        app.set_stop_icon(transport_icons.stop);
        app.set_reset_icon(transport_icons.reset);
        app.set_step_frame_icon(transport_icons.step_frame);
        app.set_gpu_status("Aguardando inicialização do renderer WGPU 30...".into());
        app.set_frame_status("Aguardando o primeiro frame completo do VDP.".into());
        app.set_lifecycle_status("Renderer setup: 0 | teardown: 0".into());
        app.set_selected_region(region.clone().into());
        app.set_selected_system(preferences.selected_system.clone().into());
        app.set_firmware_directory(preferences.firmware_directory.clone().into());
        app.set_firmware_boot_enabled(preferences.firmware_boot_enabled);
        app.set_firmware_brazil_selected(preferences.firmware_brazil_selected.clone().into());
        app.set_firmware_usa_selected(preferences.firmware_usa_selected.clone().into());
        app.set_firmware_europe_selected(preferences.firmware_europe_selected.clone().into());
        app.set_firmware_japan_selected(preferences.firmware_japan_selected.clone().into());
        app.set_firmware_korea_selected(preferences.firmware_korea_selected.clone().into());
        for (key, setter) in [
            (&preferences.keyboard_up_key, 0_u8),
            (&preferences.keyboard_down_key, 1),
            (&preferences.keyboard_left_key, 2),
            (&preferences.keyboard_right_key, 3),
            (&preferences.keyboard_button1_key, 4),
            (&preferences.keyboard_button2_key, 5),
        ] {
            if !key.is_empty() {
                let value: slint::SharedString = key.clone().into();
                match setter {
                    0 => {
                        app.set_keyboard_up_key(value);
                        app.set_keyboard_up_label(keyboard_key_label(key).into());
                    }
                    1 => {
                        app.set_keyboard_down_key(value);
                        app.set_keyboard_down_label(keyboard_key_label(key).into());
                    }
                    2 => {
                        app.set_keyboard_left_key(value);
                        app.set_keyboard_left_label(keyboard_key_label(key).into());
                    }
                    3 => {
                        app.set_keyboard_right_key(value);
                        app.set_keyboard_right_label(keyboard_key_label(key).into());
                    }
                    4 => {
                        app.set_keyboard_button1_key(value);
                        app.set_keyboard_button1_label(keyboard_key_label(key).into());
                    }
                    5 => {
                        app.set_keyboard_button2_key(value);
                        app.set_keyboard_button2_label(keyboard_key_label(key).into());
                    }
                    _ => unreachable!(),
                }
            }
        }
        if rom_path.is_some() {
            app.set_active_system(preferences.selected_system.clone().into());
        }
        app.set_show_fps(preferences.show_fps);
        app.set_show_status_footer(preferences.show_status_footer);
        app.set_sprite_limit_enabled(preferences.sprite_limit_enabled);
        app.set_audio_volume(f32::from(preferences.audio_volume));
        app.set_audio_muted(preferences.audio_muted);
        app.set_audio_renderer_mode(preferences.audio_renderer_mode.clone().into());
        app.set_audio_dsp_preset(preferences.audio_dsp_preset);
        app.set_audio_hpf_enabled(preferences.audio_hpf_enabled);
        app.set_audio_hpf_cutoff(f32::from(preferences.audio_hpf_cutoff_hz));
        app.set_audio_lpf_enabled(preferences.audio_lpf_enabled);
        app.set_audio_lpf_cutoff(f32::from(preferences.audio_lpf_cutoff_hz));
        app.set_sg1000_audio_hpf_enabled(preferences.sg1000_audio_hpf_enabled);
        app.set_sg1000_audio_hpf_cutoff(f32::from(preferences.sg1000_audio_hpf_cutoff_hz));
        app.set_sg1000_audio_lpf_enabled(preferences.sg1000_audio_lpf_enabled);
        app.set_sg1000_audio_lpf_cutoff(f32::from(preferences.sg1000_audio_lpf_cutoff_hz));
        app.set_fps_size(preferences.fps_size.clone().into());
        app.set_fps_font_size(match preferences.fps_size.as_str() {
            "small" => 13.0,
            "large" => 24.0,
            _ => 18.0,
        });
        app.set_recent_roms(slint::ModelRc::new(slint::VecModel::from(
            recent_roms_model(&recent_roms.borrow()),
        )));
        match preferences.aspect.as_str() {
            "1:1" => {
                app.set_selected_aspect("1:1".into());
                app.set_video_width(576.0);
                app.set_video_height(576.0);
            }
            "16:9" => {
                app.set_selected_aspect("16:9".into());
                app.set_video_width(768.0);
                app.set_video_height(432.0);
            }
            _ => {
                app.set_selected_aspect("4:3".into());
                app.set_video_width(768.0);
                app.set_video_height(576.0);
            }
        }
        app.set_has_rom(worker.borrow().is_some());
        app.set_audio_status(if video_only_capture_enabled() {
            "Saída física desativada pelo modo de captura somente vídeo.".into()
        } else if worker
            .borrow()
            .as_ref()
            .is_some_and(|worker| worker.audio_output_metrics().sample_rate_hz > 0)
        {
            "CPAL ativo — volume aplicado no DSP de saída.".into()
        } else if worker.borrow().is_some() {
            "Saída CPAL indisponível; consulte R3_AUDIO nos logs.".into()
        } else {
            "Carregue uma ROM para inicializar a saída CPAL.".into()
        });
        if let Some(path) = rom_path.as_ref() {
            app.set_rom_status(format!("ROM: {} | região: {region}", path.display()).into());
            app.set_recent_rom_path(path.display().to_string().into());
            app.set_rom_path_input(path.display().to_string().into());
        } else {
            app.set_rom_status(
                "Nenhuma ROM carregada — informe o caminho em Arquivo > Carregar ROM.".into(),
            );
        }

        if let Some(initial_worker) = worker.borrow().as_ref() {
            initial_worker.start()?;
        }

        let start_worker = Rc::clone(&worker);
        let start_app = app.as_weak();
        app.on_start_requested(move || {
            if let Some(worker) = start_worker.borrow().as_ref()
                && let Err(error) = worker.start()
            {
                diagnostic_log!(warn, "R2_CONTROL action=start error={error}");
            } else if let Some(app) = start_app.upgrade() {
                app.set_emulation_paused(false);
            }
        });

        let pause_worker = Rc::clone(&worker);
        let pause_app = app.as_weak();
        app.on_pause_requested(move || {
            if let Some(worker) = pause_worker.borrow().as_ref()
                && let Err(error) = worker.pause()
            {
                diagnostic_log!(warn, "R2_CONTROL action=pause error={error}");
            } else if let Some(app) = pause_app.upgrade() {
                app.set_emulation_paused(true);
            }
        });

        // STOP encerra a sessão atual (persistindo SRAM, destacando o mailbox e
        // destruindo o worker) para permitir selecionar outro sistema/ROM sem fechar
        // a janela. PAUSE continua sendo a operação que apenas suspende a máquina.
        let stop_app = app.as_weak();
        app.on_stop_requested(move || {
            if let Some(app) = stop_app.upgrade() {
                app.invoke_close_rom_requested();
                if !app.get_has_rom() {
                    app.set_emulation_paused(false);
                    app.set_rom_status(
                        "Emulação encerrada. Selecione o sistema e carregue outra ROM.".into(),
                    );
                }
            }
        });

        let reset_worker = Rc::clone(&worker);
        let reset_app = app.as_weak();
        app.on_reset_requested(move || {
            let result = reset_worker.borrow().as_ref().map_or(Ok(()), |worker| {
                worker.reset().and_then(|()| worker.start())
            });
            if let Some(app) = reset_app.upgrade() {
                match result {
                    Ok(()) => {
                        app.set_emulation_paused(false);
                        app.set_rom_status("Console reiniciado; execução retomada.".into());
                    }
                    Err(error) => {
                        app.set_rom_status(format!("Falha ao reiniciar o console: {error}").into());
                        diagnostic_log!(warn, "R2_CONTROL action=reset error={error}");
                    }
                }
            }
        });

        let step_worker = Rc::clone(&worker);
        app.on_step_frame_requested(move || {
            if let Some(worker) = step_worker.borrow().as_ref()
                && let Err(error) = worker.step_frame(100_000)
            {
                diagnostic_log!(warn, "R2_CONTROL action=step_frame error={error}");
            }
        });

        let keyboard_state = Rc::new(RefCell::new(KeyboardInputState::default()));
        let gamepad_state = Rc::new(RefCell::new([ControllerButtons::default(); 2]));

        let keyboard_capture_app = app.as_weak();
        app.on_keyboard_key_captured(
            move |action: slint::SharedString, key: slint::SharedString| {
                let Some(app) = keyboard_capture_app.upgrade() else {
                    return;
                };
                let key = key.to_string();
                if key.is_empty() || key.chars().any(char::is_control) {
                    app.set_keyboard_capture_status("Tecla inválida. Tente outra tecla.".into());
                    app.set_keyboard_capture_action("".into());
                    return;
                }
                let current = [
                    app.get_keyboard_up_key().to_string(),
                    app.get_keyboard_down_key().to_string(),
                    app.get_keyboard_left_key().to_string(),
                    app.get_keyboard_right_key().to_string(),
                    app.get_keyboard_button1_key().to_string(),
                    app.get_keyboard_button2_key().to_string(),
                ];
                let action_index = match action.as_str() {
                    "up" => 0,
                    "down" => 1,
                    "left" => 2,
                    "right" => 3,
                    "button1" => 4,
                    "button2" => 5,
                    _ => 6,
                };
                if action_index == 6
                    || current
                        .iter()
                        .enumerate()
                        .any(|(index, assigned)| index != action_index && assigned == &key)
                {
                    app.set_keyboard_capture_status("Tecla já atribuída ou ação inválida.".into());
                    app.set_keyboard_capture_action("".into());
                    return;
                }
                let label = keyboard_key_label(&key);
                let shared: slint::SharedString = key.into();
                match action.as_str() {
                    "up" => {
                        app.set_keyboard_up_key(shared);
                        app.set_keyboard_up_label(label.into());
                    }
                    "down" => {
                        app.set_keyboard_down_key(shared);
                        app.set_keyboard_down_label(label.into());
                    }
                    "left" => {
                        app.set_keyboard_left_key(shared);
                        app.set_keyboard_left_label(label.into());
                    }
                    "right" => {
                        app.set_keyboard_right_key(shared);
                        app.set_keyboard_right_label(label.into());
                    }
                    "button1" => {
                        app.set_keyboard_button1_key(shared);
                        app.set_keyboard_button1_label(label.into());
                    }
                    "button2" => {
                        app.set_keyboard_button2_key(shared);
                        app.set_keyboard_button2_label(label.into());
                    }
                    _ => {
                        app.set_keyboard_capture_status(
                            "Ação de remapeamento desconhecida.".into(),
                        );
                        app.set_keyboard_capture_action("".into());
                        return;
                    }
                }
                app.set_keyboard_capture_action("".into());
                app.set_keyboard_capture_status(
                    "Tecla atualizada; preferências persistidas.".into(),
                );
                app.invoke_preferences_changed();
            },
        );

        let keyboard_press_state = Rc::clone(&keyboard_state);
        let keyboard_press_worker = Rc::clone(&worker);
        let keyboard_press_gamepad = Rc::clone(&gamepad_state);
        let keyboard_press_app = app.as_weak();
        app.on_keyboard_key_pressed(move |key: slint::SharedString| {
            let Some(app) = keyboard_press_app.upgrade() else {
                return;
            };
            if app.get_active_menu().as_str() != "none" || !app.get_has_rom() {
                return;
            }

            let action = keyboard_press_state.borrow_mut().update(key.as_str(), true);
            let Some(action) = action else {
                return;
            };

            if action.is_controller_button() {
                let buttons = merge_controller_buttons(
                    keyboard_press_state.borrow().controller_buttons(),
                    keyboard_press_gamepad.borrow()[0],
                );
                if let Some(worker) = keyboard_press_worker.borrow().as_ref()
                    && let Err(error) = worker.set_controller_at(1, buttons, Instant::now())
                {
                    diagnostic_log!(
                        warn,
                        "SLINT_KEYBOARD action=keyboard_controller_press error={error}"
                    );
                }
            }

            match action {
                KeyboardAction::Nmi => {
                    if let Some(worker) = keyboard_press_worker.borrow().as_ref()
                        && let Err(error) = worker.request_nmi()
                    {
                        diagnostic_log!(warn, "SLINT_KEYBOARD action=keyboard_nmi error={error}");
                    }
                }
                KeyboardAction::Start => app.invoke_start_requested(),
                KeyboardAction::Pause => app.invoke_pause_requested(),
                KeyboardAction::Stop => app.invoke_stop_requested(),
                KeyboardAction::Reset => app.invoke_reset_requested(),
                KeyboardAction::StepFrame => {
                    app.set_emulation_paused(true);
                    app.invoke_pause_requested();
                    app.invoke_step_frame_requested();
                }
                _ => {}
            }
        });

        let keyboard_release_state = Rc::clone(&keyboard_state);
        let keyboard_release_worker = Rc::clone(&worker);
        let keyboard_release_gamepad = Rc::clone(&gamepad_state);
        app.on_keyboard_key_released(move |key: slint::SharedString| {
            let action = keyboard_release_state
                .borrow_mut()
                .update(key.as_str(), false);
            if action.is_some_and(KeyboardAction::is_controller_button) {
                let buttons = merge_controller_buttons(
                    keyboard_release_state.borrow().controller_buttons(),
                    keyboard_release_gamepad.borrow()[0],
                );
                if let Some(worker) = keyboard_release_worker.borrow().as_ref()
                    && let Err(error) = worker.set_controller_at(1, buttons, Instant::now())
                {
                    diagnostic_log!(
                        warn,
                        "SLINT_KEYBOARD action=keyboard_controller_release error={error}"
                    );
                }
            }
        });

        let keyboard_focus_state = Rc::clone(&keyboard_state);
        let keyboard_focus_worker = Rc::clone(&worker);
        let keyboard_focus_gamepad = Rc::clone(&gamepad_state);
        app.on_keyboard_focus_lost(move || {
            let previous_buttons = {
                let mut state = keyboard_focus_state.borrow_mut();
                let previous = state.controller_buttons();
                state.clear();
                previous
            };
            let combined =
                merge_controller_buttons(previous_buttons, keyboard_focus_gamepad.borrow()[0]);
            if let Some(worker) = keyboard_focus_worker.borrow().as_ref()
                && let Err(error) = worker.set_controller_at(1, combined, Instant::now())
            {
                diagnostic_log!(warn, "SLINT_KEYBOARD action=keyboard_focus_release error={error}");
            }
        });

        let open_app = app.as_weak();
        let open_worker = Rc::clone(&worker);
        let open_diagnostics_consent = Rc::clone(&diagnostics_consent_slot);
        let open_save_context = Rc::clone(&save_context);
        let open_save_store = Arc::clone(&save_store);
        let open_recent = Rc::clone(&recent_rom);
        let open_recent_list = Rc::clone(&recent_roms);
        let open_mailbox = mailbox.clone();
        let open_gamepad_state = Rc::clone(&gamepad_state);
        app.on_load_rom_requested(move |path: slint::SharedString| {
            let path = PathBuf::from(path.as_str());
            if path.as_os_str().is_empty() {
                if let Some(app) = open_app.upgrade() {
                    app.set_rom_status(
                        "Informe o caminho completo da ROM (.sms, .sg, .bin ou .zip).".into(),
                    );
                }
                return;
            }

            let (
                region,
                system,
                sprite_limit_enabled,
                audio_volume,
                dsp_config,
                sg1000_dsp_config,
                renderer_mode,
                firmware_directory,
                firmware_boot_enabled,
                firmware_brazil_selected,
                firmware_usa_selected,
                firmware_europe_selected,
                firmware_japan_selected,
                firmware_korea_selected,
            ) = open_app
                .upgrade()
                .map(|app| {
                    (
                        app.get_selected_region().to_string(),
                        app.get_selected_system().to_string(),
                        app.get_sprite_limit_enabled(),
                        effective_audio_volume(
                            app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
                            app.get_audio_muted(),
                        ),
                        audio_dsp_config(
                            app.get_audio_dsp_preset(),
                            app.get_audio_hpf_enabled(),
                            f64::from(app.get_audio_hpf_cutoff().round() as u16),
                            app.get_audio_lpf_enabled(),
                            f64::from(app.get_audio_lpf_cutoff().round() as u16),
                        ),
                        audio_dsp_config(
                            false,
                            app.get_sg1000_audio_hpf_enabled(),
                            f64::from(app.get_sg1000_audio_hpf_cutoff().round() as u16),
                            app.get_sg1000_audio_lpf_enabled(),
                            f64::from(app.get_sg1000_audio_lpf_cutoff().round() as u16),
                        ),
                        app.get_audio_renderer_mode().to_string(),
                        app.get_firmware_directory().to_string(),
                        app.get_firmware_boot_enabled(),
                        app.get_firmware_brazil_selected().to_string(),
                        app.get_firmware_usa_selected().to_string(),
                        app.get_firmware_europe_selected().to_string(),
                        app.get_firmware_japan_selected().to_string(),
                        app.get_firmware_korea_selected().to_string(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        "usa".to_owned(),
                        "sms1".to_owned(),
                        true,
                        70,
                        AnalogOutputConfig::default(),
                        AnalogOutputConfig::default(),
                        "psg".to_owned(),
                        String::new(),
                        false,
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    )
                });
            let vdp_revision = match DesktopSystem::from_id(&system) {
                Ok(DesktopSystem::Sg1000) => SmsVdpRevision::Sega3155124,
                Ok(DesktopSystem::Sms1 | DesktopSystem::Sms2) => {
                    match sms_vdp_revision_for_system(&system) {
                        Ok(revision) => revision,
                        Err(error) => {
                            if let Some(app) = open_app.upgrade() {
                                app.set_rom_status(
                                    format!("Sistema selecionado inválido: {error}").into(),
                                );
                            }
                            return;
                        }
                    }
                }
                Err(error) => {
                    if let Some(app) = open_app.upgrade() {
                        app.set_rom_status(format!("Sistema selecionado inválido: {error}").into());
                    }
                    return;
                }
            };
            let open_settings = SmsWorkerSettings {
                region,
                sms_system: system.clone(),
                firmware_directory,
                firmware_boot_enabled,
                firmware_brazil_selected,
                firmware_usa_selected,
                firmware_europe_selected,
                firmware_japan_selected,
                firmware_korea_selected,
                vdp_revision,
                audio_volume,
                dsp_config,
                sg1000_dsp_config,
                renderer_mode,
            };

            match replace_desktop_session(
                &path,
                &open_worker,
                &open_save_context,
                Arc::clone(&open_save_store),
                &open_mailbox,
                &open_settings,
                open_diagnostics_consent.borrow().as_ref().copied(),
            ) {
                Ok(()) => {
                    *open_gamepad_state.borrow_mut() = [ControllerButtons::default(); 2];
                    let start_result = open_worker.borrow().as_ref().map(|worker| {
                        worker
                            .set_sprite_limit_enabled(sprite_limit_enabled)
                            .and_then(|()| worker.reset())
                            .and_then(|()| worker.start())
                    });
                    *open_recent.borrow_mut() = Some(path.clone());
                    record_recent_rom(&mut open_recent_list.borrow_mut(), path.clone());
                    if let Some(app) = open_app.upgrade() {
                        app.set_recent_roms(slint::ModelRc::new(slint::VecModel::from(
                            recent_roms_model(&open_recent_list.borrow()),
                        )));
                        app.set_has_rom(true);
                        app.set_active_system(system.to_owned().into());
                        app.set_trace_enabled(false);
                        app.set_audio_status(if video_only_capture_enabled() {
                            "Saída física desativada pelo modo de captura somente vídeo.".into()
                        } else if open_worker
                            .borrow()
                            .as_ref()
                            .is_some_and(|worker| worker.audio_output_metrics().sample_rate_hz > 0)
                        {
                            "CPAL ativo — volume aplicado no DSP de saída.".into()
                        } else {
                            "Saída CPAL indisponível; ative Coleta de logs para detalhes.".into()
                        });
                        app.set_emulation_paused(false);
                        app.set_rom_status(
                            match start_result {
                                Some(Ok(())) => {
                                    format!("ROM: {} | execução iniciada", path.display())
                                }
                                Some(Err(error)) => {
                                    format!("ROM: {} | falha ao iniciar: {error}", path.display())
                                }
                                None => {
                                    "ROM carregada, mas o worker não está disponível.".to_owned()
                                }
                            }
                            .into(),
                        );
                        app.set_recent_rom_path(path.display().to_string().into());
                        app.set_rom_path_input(path.display().to_string().into());
                        app.set_debug_status(
                            "ROM carregada; ative Coleta de logs para capturar snapshots e traces."
                                .into(),
                        );
                    }
                }
                Err(error) => {
                    if let Some(app) = open_app.upgrade() {
                        app.set_rom_status(format!("Falha ao carregar ROM: {error}").into());
                    }
                }
            }
        });

        let browse_app = app.as_weak();
        app.on_browse_rom_requested(move || match pick_rom_file() {
            Ok(Some(path)) => {
                if let Some(app) = browse_app.upgrade() {
                    let selected_path = path.to_string_lossy().into_owned();
                    app.set_rom_path_input(selected_path.clone().into());
                    app.set_active_menu("none".into());
                    app.invoke_load_rom_requested(selected_path.into());
                }
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(app) = browse_app.upgrade() {
                    app.set_rom_status(format!("Seletor de arquivos indisponível: {error}").into());
                }
            }
        });

        let firmware_picker_app = app.as_weak();
        app.on_browse_firmware_directory_requested(move || match pick_firmware_directory() {
            Ok(Some(path)) => {
                if let Some(app) = firmware_picker_app.upgrade() {
                    let directory = path.to_string_lossy().into_owned();
                    app.set_firmware_directory(directory.clone().into());
                    app.set_firmware_scan_status(
                        "Pasta selecionada; iniciando varredura...".into(),
                    );
                    app.invoke_preferences_changed();
                    app.invoke_scan_firmware_directory_requested(directory.into());
                }
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(app) = firmware_picker_app.upgrade() {
                    app.set_firmware_scan_status(
                        format!("Seletor de diretório indisponível: {error}").into(),
                    );
                }
            }
        });

        let firmware_scan_app = app.as_weak();
        let firmware_scan_generation = Arc::new(AtomicU64::new(0));
        app.on_scan_firmware_directory_requested(move |directory: slint::SharedString| {
            let Some(app) = firmware_scan_app.upgrade() else {
                return;
            };
            let path = PathBuf::from(directory.as_str());
            let system = app.get_selected_system().to_string();
            if system == "sg1000" {
                app.set_firmware_scan_status(
                    "O SG-1000 não utiliza a BIOS convencional do Master System.".into(),
                );
                app.set_firmware_scan_results(
                    "Selecione SMS1 ou SMS2 para consultar o catálogo de firmware.".into(),
                );
                return;
            }
            // Every request supersedes older scans, including an invalid directory request.
            let generation = firmware_scan_generation.fetch_add(1, Ordering::AcqRel) + 1;
            let generation_state = Arc::clone(&firmware_scan_generation);
            if !path.is_dir() {
                app.set_firmware_scan_status(
                    "O diretório informado não existe ou não está acessível.".into(),
                );
                return;
            }
            let selected = [
                app.get_firmware_brazil_selected().to_string(),
                app.get_firmware_usa_selected().to_string(),
                app.get_firmware_europe_selected().to_string(),
                app.get_firmware_japan_selected().to_string(),
                app.get_firmware_korea_selected().to_string(),
            ];
            app.set_firmware_scan_status("Escaneando arquivos de firmware...".into());
            app.set_firmware_scan_results(
                "A leitura ocorre fora da thread da interface e fora do Scheduler.".into(),
            );

            let weak = firmware_scan_app.clone();
            let spawn = thread::Builder::new()
                .name("sms-firmware-directory-scan".to_owned())
                .spawn(move || {
                    let result = scan_sms_firmware_directory(&path, &system);
                    let _ = slint::invoke_from_event_loop(move || {
                        if generation_state.load(Ordering::Acquire) != generation {
                            return;
                        }
                        let Some(app) = weak.upgrade() else {
                            return;
                        };
                        match result {
                            Ok(view) => {
                                let keep_selection = |options: &[String], current: &str| {
                                    if options.is_empty()
                                        || options.iter().any(|option| option == current)
                                    {
                                        current.to_owned()
                                    } else {
                                        String::new()
                                    }
                                };
                                let brazil = keep_selection(&view.brazil, &selected[0]);
                                let usa = keep_selection(&view.usa, &selected[1]);
                                let europe = keep_selection(&view.europe, &selected[2]);
                                let japan = keep_selection(&view.japan, &selected[3]);
                                let korea = keep_selection(&view.korea, &selected[4]);

                                app.set_firmware_brazil_options(slint_string_model(view.brazil));
                                app.set_firmware_usa_options(slint_string_model(view.usa));
                                app.set_firmware_europe_options(slint_string_model(view.europe));
                                app.set_firmware_japan_options(slint_string_model(view.japan));
                                app.set_firmware_korea_options(slint_string_model(view.korea));
                                app.set_firmware_brazil_selected(brazil.into());
                                app.set_firmware_usa_selected(usa.into());
                                app.set_firmware_europe_selected(europe.into());
                                app.set_firmware_japan_selected(japan.into());
                                app.set_firmware_korea_selected(korea.into());
                                app.set_firmware_scan_status(view.status.into());
                                app.set_firmware_scan_results(view.results.into());
                                app.invoke_preferences_changed();
                            }
                            Err(error) => {
                                app.set_firmware_scan_status(
                                    format!("Falha na varredura: {error}").into(),
                                );
                                app.set_firmware_scan_results(
                                    "Nenhum resultado válido foi produzido.".into(),
                                );
                            }
                        }
                    });
                });
            if let Err(error) = spawn {
                app.set_firmware_scan_status(
                    format!("Não foi possível iniciar a varredura: {error}").into(),
                );
            }
        });

        app.on_firmware_selection_changed(move |region, firmware| {
            diagnostic_log!(
                "SMS_FIRMWARE_SELECTION region={} selection={}",
                region,
                firmware
            );
        });

        let firmware_toggle_app = app.as_weak();
        app.on_firmware_boot_toggle_requested(move |enabled| {
            if let Some(app) = firmware_toggle_app.upgrade() {
                if enabled {
                    app.set_firmware_scan_status(
                        "Preferência salva para a próxima abertura da ROM. Com cartucho, selecione uma BIOS convencional de 8 KiB compatível; firmware com jogo interno e boot sem cartucho continuam pendentes.".into(),
                    );
                } else {
                    app.set_firmware_scan_status(
                        "Boot direto por cartucho selecionado; a BIOS não será carregada na próxima abertura.".into(),
                    );
                }
            }
        });

        let close_app = app.as_weak();
        let close_worker = Rc::clone(&worker);
        let close_save_context = Rc::clone(&save_context);
        app.on_close_rom_requested(move || {
            let previous = close_worker.borrow_mut().take();
            if let Some(previous) = previous {
                let was_running = previous.is_running();
                if let Err(error) = previous.pause() {
                    if was_running {
                        let _ = previous.start();
                    }
                    *close_worker.borrow_mut() = Some(previous);
                    if let Some(app) = close_app.upgrade() {
                        app.set_rom_status(
                            format!("Falha ao pausar antes de salvar SRAM: {error}").into(),
                        );
                    }
                    return;
                }
                if let Err(error) =
                    persist_desktop_session_sram(&previous, close_save_context.borrow().as_ref())
                {
                    if was_running {
                        let _ = previous.start();
                    }
                    *close_worker.borrow_mut() = Some(previous);
                    if let Some(app) = close_app.upgrade() {
                        app.set_rom_status(
                            format!("ROM mantida; falha ao gravar SRAM: {error}").into(),
                        );
                    }
                    return;
                }
                let _ = previous.stop();
                let _ = previous.detach_video_mailbox();
                drop(previous);
                *close_save_context.borrow_mut() = None;
            }
            if let Some(app) = close_app.upgrade() {
                app.set_has_rom(false);
                app.set_emulation_paused(false);
                app.set_audio_status("Sem ROM carregada; volume e mute permanecem salvos.".into());
                app.set_rom_status("Nenhuma ROM carregada.".into());
                app.set_debug_status("Máquina fechada.".into());
            }
        });

        let exit_recent = Rc::clone(&recent_roms);
        let exit_worker = Rc::clone(&worker);
        let exit_save_context = Rc::clone(&save_context);
        let exit_prefs_app = app.as_weak();
        app.on_exit_requested(move || {
            if let Some(active_worker) = exit_worker.borrow().as_ref() {
                let was_running = active_worker.is_running();
                if let Err(error) = active_worker.pause() {
                    if was_running {
                        let _ = active_worker.start();
                    }
                    if let Some(app) = exit_prefs_app.upgrade() {
                        app.set_rom_status(
                            format!("Saída cancelada; falha ao pausar a máquina: {error}").into(),
                        );
                    }
                    return;
                }
                if let Err(error) =
                    persist_desktop_session_sram(active_worker, exit_save_context.borrow().as_ref())
                {
                    if was_running {
                        let _ = active_worker.start();
                    }
                    if let Some(app) = exit_prefs_app.upgrade() {
                        app.set_rom_status(
                            format!("Saída cancelada; falha ao gravar SRAM: {error}").into(),
                        );
                    }
                    return;
                }
            }
            let _ = save_recent_roms(&exit_recent.borrow());
            if let Some(app) = exit_prefs_app.upgrade() {
                let prefs = DesktopPreferences {
                    region: app.get_selected_region().to_string(),
                    selected_system: app.get_selected_system().to_string(),
                    firmware_directory: app.get_firmware_directory().to_string(),
                    firmware_boot_enabled: app.get_firmware_boot_enabled(),
                    firmware_brazil_selected: app.get_firmware_brazil_selected().to_string(),
                    firmware_usa_selected: app.get_firmware_usa_selected().to_string(),
                    firmware_europe_selected: app.get_firmware_europe_selected().to_string(),
                    firmware_japan_selected: app.get_firmware_japan_selected().to_string(),
                    firmware_korea_selected: app.get_firmware_korea_selected().to_string(),
                    aspect: app.get_selected_aspect().to_string(),
                    show_fps: app.get_show_fps(),
                    show_status_footer: app.get_show_status_footer(),
                    fps_size: app.get_fps_size().to_string(),
                    sprite_limit_enabled: app.get_sprite_limit_enabled(),
                    audio_volume: app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
                    audio_muted: app.get_audio_muted(),
                    audio_renderer_mode: app.get_audio_renderer_mode().to_string(),
                    audio_dsp_preset: app.get_audio_dsp_preset(),
                    audio_hpf_enabled: app.get_audio_hpf_enabled(),
                    audio_hpf_cutoff_hz: app.get_audio_hpf_cutoff().round().clamp(5.0, 500.0)
                        as u16,
                    audio_lpf_enabled: app.get_audio_lpf_enabled(),
                    audio_lpf_cutoff_hz: app.get_audio_lpf_cutoff().round().clamp(4_000.0, 20_000.0)
                        as u16,
                    sg1000_audio_hpf_enabled: app.get_sg1000_audio_hpf_enabled(),
                    sg1000_audio_hpf_cutoff_hz: app
                        .get_sg1000_audio_hpf_cutoff()
                        .round()
                        .clamp(5.0, 500.0) as u16,
                    sg1000_audio_lpf_enabled: app.get_sg1000_audio_lpf_enabled(),
                    sg1000_audio_lpf_cutoff_hz: app
                        .get_sg1000_audio_lpf_cutoff()
                        .round()
                        .clamp(4_000.0, 20_000.0)
                        as u16,
                    keyboard_up_key: app.get_keyboard_up_key().to_string(),
                    keyboard_down_key: app.get_keyboard_down_key().to_string(),
                    keyboard_left_key: app.get_keyboard_left_key().to_string(),
                    keyboard_right_key: app.get_keyboard_right_key().to_string(),
                    keyboard_button1_key: app.get_keyboard_button1_key().to_string(),
                    keyboard_button2_key: app.get_keyboard_button2_key().to_string(),
                };
                let _ = save_desktop_preferences(&prefs);
                let _ = app.window().hide();
            }
        });

        let debug_app = app.as_weak();
        let debug_worker = Rc::clone(&worker);
        app.on_debug_snapshot_requested(move || {
            let worker_guard = debug_worker.borrow();
            let Some(worker) = worker_guard.as_ref() else {
                return;
            };
            let result = worker.debug_summary(host_diagnostics_enabled());
            if let Some(app) = debug_app.upgrade() {
                app.set_debug_status(
                    result
                        .unwrap_or_else(|error| format!("Falha no snapshot: {error}"))
                        .into(),
                );
            }
        });

        let trace_export_app = app.as_weak();
        let trace_export_worker = Rc::clone(&worker);
        app.on_trace_export_requested(move || {
            let result = (|| -> Result<Option<PathBuf>, String> {
                let snapshot = {
                    let worker_guard = trace_export_worker.borrow();
                    let session = worker_guard
                        .as_ref()
                        .ok_or_else(|| "Nenhuma sessão ativa".to_owned())?;
                    let DesktopSession::Sms { worker, .. } = session else {
                        return Err("Exportação de trace está disponível somente no Master System.".to_owned());
                    };
                    worker.debug_snapshot()?
                };
                if !snapshot.trace_enabled {
                    return Err("Ative o trace antes de exportar".to_owned());
                }

                let mut report = format!(
                    "SEGA EMULATOR — SMS BUS TRACE\nformat_version=1\nframe={}\nmachine_time_ticks={}\ntrace_enabled={}\ntrace_events={}\ntrace_dropped={}\nPC={:04X} SP={:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} IX={:04X} IY={:04X} I={:02X} R={:02X} IFF1={} IFF2={} IM={} HALT={}\nVDP_STATUS={:02X} VDP_SCANLINE={} VDP_CYCLES_INTO_LINE={}\nMAPPER_BANKS={:02X},{:02X},{:02X} MAPPER_CONTROL={:02X} MEMORY_CONTROL={:02X}\n\nEVENTS (oldest first)\n",
                    snapshot.frame_number, snapshot.machine_time_ticks, snapshot.trace_enabled,
                    snapshot.trace_events.len(), snapshot.trace_dropped,
                    snapshot.pc, snapshot.sp, snapshot.af, snapshot.bc, snapshot.de, snapshot.hl,
                    snapshot.ix, snapshot.iy, snapshot.i, snapshot.r, snapshot.iff1, snapshot.iff2,
                    snapshot.interrupt_mode, snapshot.halted, snapshot.vdp_status,
                    snapshot.vdp_scanline, snapshot.vdp_cycles_into_line,
                    snapshot.mapper_banks[0], snapshot.mapper_banks[1], snapshot.mapper_banks[2],
                    snapshot.mapper_control, snapshot.memory_control
                );
                for (index, event) in snapshot.trace_events.iter().enumerate() {
                    report.push_str(&format!("{index:04} {event:?}\n"));
                }

                let Some(path) = pick_trace_save_file()? else {
                    return Ok(None);
                };
                fs::write(&path, report)
                    .map_err(|error| format!("não foi possível gravar o trace: {error}"))?;
                Ok(Some(path))
            })();
            if let Some(app) = trace_export_app.upgrade() {
                let status = match result {
                    Ok(Some(path)) => format!("Trace exportado: {}", path.display()),
                    Ok(None) => "Exportação do trace cancelada; nenhum arquivo foi gravado.".to_owned(),
                    Err(error) => format!("Falha ao exportar trace: {error}"),
                };
                app.set_debug_status(status.into());
            }
        });

        let preferences_app = app.as_weak();
        app.on_preferences_changed(move || {
            let Some(app) = preferences_app.upgrade() else {
                return;
            };
            let prefs = DesktopPreferences {
                region: app.get_selected_region().to_string(),
                selected_system: app.get_selected_system().to_string(),
                firmware_directory: app.get_firmware_directory().to_string(),
                firmware_boot_enabled: app.get_firmware_boot_enabled(),
                firmware_brazil_selected: app.get_firmware_brazil_selected().to_string(),
                firmware_usa_selected: app.get_firmware_usa_selected().to_string(),
                firmware_europe_selected: app.get_firmware_europe_selected().to_string(),
                firmware_japan_selected: app.get_firmware_japan_selected().to_string(),
                firmware_korea_selected: app.get_firmware_korea_selected().to_string(),
                aspect: app.get_selected_aspect().to_string(),
                show_fps: app.get_show_fps(),
                show_status_footer: app.get_show_status_footer(),
                fps_size: app.get_fps_size().to_string(),
                sprite_limit_enabled: app.get_sprite_limit_enabled(),
                audio_volume: app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
                audio_muted: app.get_audio_muted(),
                audio_renderer_mode: app.get_audio_renderer_mode().to_string(),
                audio_dsp_preset: app.get_audio_dsp_preset(),
                audio_hpf_enabled: app.get_audio_hpf_enabled(),
                audio_hpf_cutoff_hz: app.get_audio_hpf_cutoff().round().clamp(5.0, 500.0) as u16,
                audio_lpf_enabled: app.get_audio_lpf_enabled(),
                audio_lpf_cutoff_hz: app.get_audio_lpf_cutoff().round().clamp(4_000.0, 20_000.0)
                    as u16,
                sg1000_audio_hpf_enabled: app.get_sg1000_audio_hpf_enabled(),
                sg1000_audio_hpf_cutoff_hz: app
                    .get_sg1000_audio_hpf_cutoff()
                    .round()
                    .clamp(5.0, 500.0) as u16,
                sg1000_audio_lpf_enabled: app.get_sg1000_audio_lpf_enabled(),
                sg1000_audio_lpf_cutoff_hz: app
                    .get_sg1000_audio_lpf_cutoff()
                    .round()
                    .clamp(4_000.0, 20_000.0) as u16,
                keyboard_up_key: app.get_keyboard_up_key().to_string(),
                keyboard_down_key: app.get_keyboard_down_key().to_string(),
                keyboard_left_key: app.get_keyboard_left_key().to_string(),
                keyboard_right_key: app.get_keyboard_right_key().to_string(),
                keyboard_button1_key: app.get_keyboard_button1_key().to_string(),
                keyboard_button2_key: app.get_keyboard_button2_key().to_string(),
            };
            if let Err(error) = save_desktop_preferences(&prefs) {
                diagnostic_log!(warn, "R2_PREFERENCES save_failed error={error}");
            }
        });

        let audio_volume_worker = Rc::clone(&worker);
        let audio_volume_app = app.as_weak();
        app.on_audio_volume_changed(move |value| {
            let volume = value.round().clamp(0.0, 100.0) as u8;
            let Some(app) = audio_volume_app.upgrade() else {
                return;
            };
            let effective = effective_audio_volume(volume, app.get_audio_muted());
            let result = audio_volume_worker
                .borrow()
                .as_ref()
                .map(|worker| worker.set_audio_volume(effective));
            match result {
                Some(Ok(())) => app.set_audio_status(
                    format!(
                        "Volume mestre aplicado: {volume}%{}.",
                        if effective == 0 { " (silenciado)" } else { "" }
                    )
                    .into(),
                ),
                Some(Err(error)) => {
                    app.set_audio_status(format!("Falha ao aplicar volume: {error}").into())
                }
                None => app.set_audio_status(
                    "Volume salvo; será aplicado ao carregar uma ROM SMS.".into(),
                ),
            }
            app.invoke_preferences_changed();
        });

        let audio_mute_worker = Rc::clone(&worker);
        let audio_mute_app = app.as_weak();
        app.on_audio_mute_changed(move |muted| {
            let Some(app) = audio_mute_app.upgrade() else {
                return;
            };
            let volume = app.get_audio_volume().round().clamp(0.0, 100.0) as u8;
            let effective = effective_audio_volume(volume, muted);
            let result = audio_mute_worker
                .borrow()
                .as_ref()
                .map(|worker| worker.set_audio_volume(effective));
            match result {
                Some(Ok(())) => app.set_audio_status(if muted {
                    "Áudio silenciado; volume escolhido preservado.".into()
                } else {
                    format!("Áudio reativado em {volume}%.").into()
                }),
                Some(Err(error)) => {
                    app.set_audio_status(format!("Falha ao alterar áudio: {error}").into())
                }
                None => app.set_audio_status(
                    "Preferência salva; será aplicada ao carregar uma ROM.".into(),
                ),
            }
            app.invoke_preferences_changed();
        });

        let audio_dsp_worker = Rc::clone(&worker);
        let audio_dsp_app = app.as_weak();
        app.on_audio_dsp_changed(
            move |preset, hpf_enabled, hpf_cutoff, lpf_enabled, lpf_cutoff| {
                let Some(app) = audio_dsp_app.upgrade() else {
                    return;
                };
                let config = audio_dsp_config(
                    preset,
                    hpf_enabled,
                    f64::from(hpf_cutoff.round() as u16),
                    lpf_enabled,
                    f64::from(lpf_cutoff.round() as u16),
                );
                let result = audio_dsp_worker
                    .borrow()
                    .as_ref()
                    .map(|worker| worker.set_audio_dsp_config(config));
                match result {
                    Some(Ok(())) => app.set_audio_status(if config.enabled {
                        "DSP aplicado no worker; filtros ativos na saída PCM.".into()
                    } else {
                        "DSP analógico adicional desativado; saída original preservada.".into()
                    }),
                    Some(Err(error)) => {
                        app.set_audio_status(format!("Falha ao aplicar DSP: {error}").into())
                    }
                    None => app.set_audio_status(
                        "Preferências DSP salvas; serão aplicadas ao carregar uma ROM.".into(),
                    ),
                }
                app.invoke_preferences_changed();
            },
        );

        let sg1000_audio_dsp_worker = Rc::clone(&worker);
        let sg1000_audio_dsp_app = app.as_weak();
        app.on_sg1000_audio_dsp_changed(move |hpf_enabled, hpf_cutoff, lpf_enabled, lpf_cutoff| {
            let Some(app) = sg1000_audio_dsp_app.upgrade() else {
                return;
            };
            let config = audio_dsp_config(
                false,
                hpf_enabled,
                f64::from(hpf_cutoff.round() as u16),
                lpf_enabled,
                f64::from(lpf_cutoff.round() as u16),
            );
            let result = sg1000_audio_dsp_worker
                .borrow()
                .as_ref()
                .map(|session| session.set_audio_dsp_config(config));
            match result {
                Some(Ok(())) => app.set_audio_status(if config.enabled {
                    "DSP experimental aplicado à saída do SG-1000.".into()
                } else {
                    "DSP adicional do SG-1000 desativado; caminho digital preservado.".into()
                }),
                Some(Err(error)) => {
                    app.set_audio_status(format!("Falha ao aplicar DSP SG-1000: {error}").into())
                }
                None => app.set_audio_status(
                    "Preferências DSP do SG-1000 salvas; serão aplicadas ao carregar uma ROM."
                        .into(),
                ),
            }
            app.invoke_preferences_changed();
        });

        let audio_renderer_worker = Rc::clone(&worker);
        let audio_renderer_app = app.as_weak();
        app.on_audio_renderer_changed(move |value| {
            let mode = if value.as_str() == "h2-experimental" {
                AudioRendererMode::H2Experimental
            } else {
                AudioRendererMode::Psg
            };
            let Some(app) = audio_renderer_app.upgrade() else {
                return;
            };
            let result = audio_renderer_worker
                .borrow()
                .as_ref()
                .map(|worker| worker.set_sms_audio_renderer(mode));
            match result {
                Some(Ok(())) => app.set_audio_status(match mode {
                    AudioRendererMode::Psg => "Renderer PSG selecionado.".into(),
                    AudioRendererMode::H2Experimental => "H2 experimental selecionado; fallback automático para PSG em caso de falha.".into(),
                }),
                Some(Err(error)) => {
                    app.set_audio_renderer_mode("psg".into());
                    app.set_audio_status(format!("H2 indisponível; PSG preservado: {error}").into());
                }
                None => app.set_audio_status(
                    "Preferência do renderer salva; será aplicada ao iniciar uma ROM.".into(),
                ),
            }
            app.invoke_preferences_changed();
        });

        let display_mode = Rc::new(Cell::new(DesktopDisplayMode::Windowed));
        let display_mode_app = app.as_weak();
        let display_mode_state = Rc::clone(&display_mode);
        app.on_display_mode_toggle_requested(move |shortcut| {
            let shortcut = match shortcut.as_str() {
                "f11" => DisplayModeShortcut::F11,
                "alt-enter" => DisplayModeShortcut::AltEnter,
                _ => return,
            };
            let next = transition_display_mode(display_mode_state.get(), shortcut);
            display_mode_state.set(next);
            let fullscreen = next != DesktopDisplayMode::Windowed;
            let immersive = next == DesktopDisplayMode::FullscreenImmersive;
            if let Some(app) = display_mode_app.upgrade() {
                app.set_fullscreen_enabled(fullscreen);
                app.set_immersive_fullscreen(immersive);
                app.set_active_menu("none".into());
                app.window().set_fullscreen(fullscreen);
            }
        });

        let video_click_worker = Rc::clone(&worker);
        let video_click_app = app.as_weak();
        app.on_video_area_clicked(move || {
            let paused = video_click_app
                .upgrade()
                .is_some_and(|app| app.get_has_rom() && app.get_emulation_paused());
            if paused && let Some(worker) = video_click_worker.borrow().as_ref() {
                match worker.start() {
                    Ok(()) => {
                        if let Some(app) = video_click_app.upgrade() {
                            app.set_emulation_paused(false);
                        }
                    }
                    Err(error) => {
                        if let Some(app) = video_click_app.upgrade() {
                            app.set_rom_status(
                                format!("Falha ao retomar a emulação: {error}").into(),
                            );
                        }
                    }
                }
            }
        });

        let sprite_worker = Rc::clone(&worker);
        let sprite_app = app.as_weak();
        app.on_sprite_limit_toggle_requested(move |enabled| {
            if let Some(worker) = sprite_worker.borrow().as_ref()
                && let Err(error) = worker.set_sprite_limit_enabled(enabled)
            {
                diagnostic_log!(warn, "R2_VIDEO action=sprite_limit_toggle error={error}");
                if let Some(app) = sprite_app.upgrade() {
                    app.set_sprite_limit_enabled(!enabled);
                    app.set_rom_status(
                        format!("Falha ao alterar limite de sprites: {error}").into(),
                    );
                }
            }
        });

        let trace_worker = Rc::clone(&worker);
        let trace_app = app.as_weak();
        app.on_trace_toggle_requested(move |enabled| {
            let result = if enabled && !host_diagnostics_enabled() {
                Err("Habilite Coleta de logs antes de ativar o trace".to_owned())
            } else {
                trace_worker.borrow().as_ref().map_or_else(
                    || Err("SMS worker indisponível".to_owned()),
                    |worker| worker.set_sms_trace_enabled(enabled),
                )
            };
            if let Err(error) = result
                && let Some(app) = trace_app.upgrade()
            {
                app.set_trace_enabled(false);
                app.set_debug_status(format!("Falha ao alterar trace: {error}").into());
            }
        });

        let gpu_context_slot = Arc::new(Mutex::new(None));
        let setup_counter = Rc::new(Cell::new(0_u64));
        let teardown_counter = Rc::new(Cell::new(0_u64));
        attach_gpu_texture(
            &app,
            Arc::clone(&gpu_context_slot),
            mailbox.clone(),
            setup_counter,
            teardown_counter,
        )?;
        let host_pacing = Arc::new(Mutex::new(HostFramePacingWindow::new()));
        let diagnostics_host_pacing = Arc::clone(&host_pacing);
        let diagnostics_worker = Rc::clone(&worker);
        let diagnostics_consent_control = Rc::clone(&diagnostics_consent_slot);
        let diagnostics_app = app.as_weak();
        let diagnostics_timer = Rc::new(create_frame_diagnostics_timer(
            &app,
            Arc::clone(&host_pacing),
            Rc::clone(&worker),
        ));
        let diagnostics_timer_control = Rc::clone(&diagnostics_timer);
        app.on_diagnostics_toggle_requested(move |enabled| {
            if !enabled {
                diagnostics_timer_control.stop();
                *diagnostics_host_pacing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                    HostFramePacingWindow::new();
                if let Some(app) = diagnostics_app.upgrade() {
                    app.set_trace_enabled(false);
                }
                *diagnostics_consent_control.borrow_mut() = None;
                // Close the central gate first; no diagnostic producer may publish
                // while the worker is being asked to release its in-memory buffers.
                let consent_result = set_diagnostics_consent(None);
                let worker_result = diagnostics_worker
                    .borrow()
                    .as_ref()
                    .map_or(Ok(()), DesktopSession::disable_sms_diagnostics);
                if let Some(app) = diagnostics_app.upgrade() {
                    app.set_debug_status(match (worker_result, consent_result) {
                        (Ok(()), Ok(())) => "Coleta revogada; timer, trace, métricas e buffers diagnósticos foram encerrados.".to_owned(),
                        (Err(worker_error), _) => format!("Coleta revogada; falha ao encerrar o trace do worker: {worker_error}"),
                        (_, Err(logging_error)) => format!("Coleta interrompida; falha ao fechar o logger: {logging_error}"),
                    }.into());
                }
                return;
            }

            let consent = sega_observability::DiagnosticsConsent::from_explicit_user_action();
            let result = set_diagnostics_consent(Some(&consent)).and_then(|()| {
                diagnostics_worker
                    .borrow()
                    .as_ref()
                    .map_or(Ok(()), |worker| worker.enable_sms_diagnostics(&consent))
            });
            match result {
                Ok(()) => {
                    *diagnostics_consent_control.borrow_mut() = Some(consent);
                    *diagnostics_host_pacing
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                        HostFramePacingWindow::new();
                    if should_run_diagnostics_timer(enabled, true) {
                        diagnostics_timer_control.restart();
                    }
                    if let Some(app) = diagnostics_app.upgrade() {
                        app.set_debug_status(
                            "Coleta autorizada nesta sessão: logs, métricas e traces permanecem sob o mesmo consentimento.".into(),
                        );
                    }
                }
                Err(error) => {
                    diagnostics_timer_control.stop();
                    *diagnostics_consent_control.borrow_mut() = None;
                    let _ = set_diagnostics_consent(None);
                    if let Some(worker) = diagnostics_worker.borrow().as_ref() {
                        let _ = worker.disable_sms_diagnostics();
                    }
                    if let Some(app) = diagnostics_app.upgrade() {
                        app.set_diagnostics_enabled(false);
                        app.set_trace_enabled(false);
                        app.set_debug_status(
                            format!("Não foi possível ativar a coleta diagnóstica: {error}").into(),
                        );
                    }
                }
            }
        });
        let _renderer_fallback_timer = start_renderer_fallback_polling(&app, Rc::clone(&worker));
        let _frame_diagnostics_timer = diagnostics_timer;
        let _sdl3_input_timer = start_sdl3_input_polling(
            &app,
            Rc::clone(&worker),
            Rc::clone(&keyboard_state),
            Rc::clone(&gamepad_state),
        );
        start_frame_presentation(
            &app,
            Arc::clone(&gpu_context_slot),
            mailbox.clone(),
            host_pacing,
        )?;

        let run_result = app.run();
        let final_preferences = DesktopPreferences {
            region: app.get_selected_region().to_string(),
            selected_system: app.get_selected_system().to_string(),
            firmware_directory: app.get_firmware_directory().to_string(),
            firmware_boot_enabled: app.get_firmware_boot_enabled(),
            firmware_brazil_selected: app.get_firmware_brazil_selected().to_string(),
            firmware_usa_selected: app.get_firmware_usa_selected().to_string(),
            firmware_europe_selected: app.get_firmware_europe_selected().to_string(),
            firmware_japan_selected: app.get_firmware_japan_selected().to_string(),
            firmware_korea_selected: app.get_firmware_korea_selected().to_string(),
            aspect: app.get_selected_aspect().to_string(),
            show_fps: app.get_show_fps(),
            show_status_footer: app.get_show_status_footer(),
            fps_size: app.get_fps_size().to_string(),
            sprite_limit_enabled: app.get_sprite_limit_enabled(),
            audio_volume: app.get_audio_volume().round().clamp(0.0, 100.0) as u8,
            audio_muted: app.get_audio_muted(),
            audio_renderer_mode: app.get_audio_renderer_mode().to_string(),
            audio_dsp_preset: app.get_audio_dsp_preset(),
            audio_hpf_enabled: app.get_audio_hpf_enabled(),
            audio_hpf_cutoff_hz: app.get_audio_hpf_cutoff().round().clamp(5.0, 500.0) as u16,
            audio_lpf_enabled: app.get_audio_lpf_enabled(),
            audio_lpf_cutoff_hz: app.get_audio_lpf_cutoff().round().clamp(4_000.0, 20_000.0) as u16,
            sg1000_audio_hpf_enabled: app.get_sg1000_audio_hpf_enabled(),
            sg1000_audio_hpf_cutoff_hz: app.get_sg1000_audio_hpf_cutoff().round().clamp(5.0, 500.0)
                as u16,
            sg1000_audio_lpf_enabled: app.get_sg1000_audio_lpf_enabled(),
            sg1000_audio_lpf_cutoff_hz: app
                .get_sg1000_audio_lpf_cutoff()
                .round()
                .clamp(4_000.0, 20_000.0) as u16,
            keyboard_up_key: app.get_keyboard_up_key().to_string(),
            keyboard_down_key: app.get_keyboard_down_key().to_string(),
            keyboard_left_key: app.get_keyboard_left_key().to_string(),
            keyboard_right_key: app.get_keyboard_right_key().to_string(),
            keyboard_button1_key: app.get_keyboard_button1_key().to_string(),
            keyboard_button2_key: app.get_keyboard_button2_key().to_string(),
        };
        let _ = save_desktop_preferences(&final_preferences);
        let _ = save_recent_roms(&recent_roms.borrow());

        let mut final_save_error = None;
        if let Some(active_worker) = worker.borrow_mut().take() {
            if let Err(error) = active_worker.pause() {
                diagnostic_log!(error, "SRAM_SAVE pause_before_exit_failed error={error}");
            }
            if let Err(error) =
                persist_desktop_session_sram(&active_worker, save_context.borrow().as_ref())
            {
                final_save_error = Some(error);
            }
            let _ = active_worker.stop();
            let _ = active_worker.detach_video_mailbox();
            drop(active_worker);
        }
        *save_context.borrow_mut() = None;
        let _ = mailbox.close();
        app.set_gpu_texture(slint::Image::default());
        if let Some(error) = final_save_error.as_ref() {
            diagnostic_log!(error, "SRAM_SAVE final_save_failed error={error}");
        }
        if let Err(error) = &run_result {
            diagnostic_log!(error, "SLINT event_loop_failed error={error}");
        }
        shutdown_diagnostics();
        if let Some(error) = final_save_error {
            return Err(error.into());
        }
        run_result?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn diagnostics_timer_requires_successful_explicit_consent() {
            assert!(!should_run_diagnostics_timer(false, true));
            assert!(!should_run_diagnostics_timer(true, false));
            assert!(should_run_diagnostics_timer(true, true));
        }

        #[test]
        fn host_diagnostics_are_disabled_by_default() {
            assert!(!super::host_diagnostics_enabled());
        }

        use super::*;

        #[test]
        fn keyboard_key_token_round_trips_unicode() {
            for key in ["w", "Z", "↑"] {
                let encoded = encode_key_token(key);
                assert_eq!(decode_key_token(&encoded).as_deref(), Some(key));
            }
            assert_eq!(decode_key_token("not-hex"), None);
            assert_eq!(decode_key_token(""), None);
        }

        #[test]
        fn keyboard_key_labels_are_readable_for_printable_keys() {
            assert_eq!(keyboard_key_label("w"), "W");
            assert_eq!(keyboard_key_label("z"), "Z");
            assert_eq!(keyboard_key_label(""), "Não atribuída");
        }

        #[test]
        fn keyboard_key_labels_hide_unicode_private_use_codes_for_special_keys() {
            for key in ['\u{E000}', '\u{F700}', '\u{F0000}', '\u{100000}'] {
                assert_eq!(keyboard_key_label(&key.to_string()), "Tecla especial");
            }
        }

        #[test]
        fn diagnostic_sram_capacity_accepts_only_supported_volatile_sizes() {
            assert_eq!(parse_diagnostic_sram_capacity(None).unwrap(), None);
            assert_eq!(
                parse_diagnostic_sram_capacity(Some("8192")).unwrap(),
                Some(0x2000)
            );
            assert_eq!(
                parse_diagnostic_sram_capacity(Some("16384")).unwrap(),
                Some(0x4000)
            );
            assert_eq!(
                parse_diagnostic_sram_capacity(Some("32768")).unwrap(),
                Some(0x8000)
            );
            assert!(parse_diagnostic_sram_capacity(Some("4096")).is_err());
            assert!(parse_diagnostic_sram_capacity(Some("invalid")).is_err());
        }

        #[test]
        fn master_volume_is_bounded_and_mute_is_independent() {
            assert_eq!(effective_audio_volume(70, false), 70);
            assert_eq!(effective_audio_volume(70, true), 0);
            assert_eq!(effective_audio_volume(0, false), 0);
            assert_eq!(effective_audio_volume(100, false), 100);
            assert_eq!(effective_audio_volume(255, false), 100);
        }

        #[test]
        fn default_audio_preferences_start_at_a_moderate_volume() {
            let preferences = DesktopPreferences::default();
            assert_eq!(preferences.audio_volume, 70);
            assert!(!preferences.audio_muted);
        }

        #[test]
        fn sg1000_audio_dsp_preferences_default_to_unfiltered_digital_output() {
            let preferences = DesktopPreferences::default();
            let config = audio_dsp_config(
                false,
                preferences.sg1000_audio_hpf_enabled,
                f64::from(preferences.sg1000_audio_hpf_cutoff_hz),
                preferences.sg1000_audio_lpf_enabled,
                f64::from(preferences.sg1000_audio_lpf_cutoff_hz),
            );

            assert!(!config.enabled);
            assert_eq!(config.high_pass_hz, None);
            assert_eq!(config.low_pass_hz, None);
        }

        #[test]
        fn sg1000_dsp_configuration_is_independent_from_sms1_experimental_preset() {
            let preferences = DesktopPreferences {
                sg1000_audio_hpf_enabled: true,
                sg1000_audio_hpf_cutoff_hz: 35,
                sg1000_audio_lpf_enabled: true,
                sg1000_audio_lpf_cutoff_hz: 12_000,
                ..DesktopPreferences::default()
            };

            let config = audio_dsp_config(
                false,
                preferences.sg1000_audio_hpf_enabled,
                f64::from(preferences.sg1000_audio_hpf_cutoff_hz),
                preferences.sg1000_audio_lpf_enabled,
                f64::from(preferences.sg1000_audio_lpf_cutoff_hz),
            );

            assert!(config.enabled);
            assert_eq!(config.high_pass_hz, Some(35.0));
            assert_eq!(config.low_pass_hz, Some(12_000.0));
            assert!(!preferences.audio_dsp_preset);
            assert!(!preferences.audio_hpf_enabled);
            assert!(!preferences.audio_lpf_enabled);
        }

        #[test]
        fn fullscreen_shortcuts_transition_between_three_presentation_modes() {
            use DesktopDisplayMode::{FullscreenImmersive, FullscreenWithUi, Windowed};
            use DisplayModeShortcut::{AltEnter, F11};

            assert_eq!(transition_display_mode(Windowed, F11), FullscreenWithUi);
            assert_eq!(transition_display_mode(FullscreenWithUi, F11), Windowed);
            assert_eq!(
                transition_display_mode(Windowed, AltEnter),
                FullscreenImmersive
            );
            assert_eq!(
                transition_display_mode(FullscreenWithUi, AltEnter),
                FullscreenImmersive
            );
            assert_eq!(
                transition_display_mode(FullscreenImmersive, AltEnter),
                Windowed
            );
            assert_eq!(
                transition_display_mode(FullscreenImmersive, F11),
                FullscreenWithUi
            );
        }

        #[test]
        fn fullscreen_mode_transitions_are_independent_of_emulation_state() {
            // The transition function only receives presentation state and shortcut;
            // no worker, scheduler, CPU or VDP state is accessible here.
            let mode = transition_display_mode(
                DesktopDisplayMode::Windowed,
                DisplayModeShortcut::AltEnter,
            );
            assert_eq!(mode, DesktopDisplayMode::FullscreenImmersive);
        }

        #[test]
        fn keyboard_mapping_recognizes_gameplay_and_transport_actions() {
            assert_eq!(
                KeyboardAction::from_slint_key("up"),
                Some(KeyboardAction::Up)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("button1"),
                Some(KeyboardAction::Button1)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("nmi"),
                Some(KeyboardAction::Nmi)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("start"),
                Some(KeyboardAction::Start)
            );
            assert_eq!(
                KeyboardAction::from_slint_key("step-frame"),
                Some(KeyboardAction::StepFrame)
            );
            assert_eq!(KeyboardAction::from_slint_key("unknown"), None);
        }

        #[test]
        fn keyboard_state_composes_simultaneous_directions_and_buttons() {
            let mut state = KeyboardInputState::default();
            assert_eq!(state.update("up", true), Some(KeyboardAction::Up));
            assert_eq!(state.update("right", true), Some(KeyboardAction::Right));
            assert_eq!(state.update("button1", true), Some(KeyboardAction::Button1));
            assert_eq!(state.update("button2", true), Some(KeyboardAction::Button2));

            let buttons = state.controller_buttons();
            assert!(buttons.up && buttons.right);
            assert!(buttons.button_1 && buttons.button_2);
            assert!(!buttons.down && !buttons.left);
        }

        #[test]
        fn keyboard_state_ignores_repeats_and_releases_only_the_matching_key() {
            let mut state = KeyboardInputState::default();
            assert_eq!(state.update("up", true), Some(KeyboardAction::Up));
            assert_eq!(state.update("up", true), None);
            assert_eq!(state.update("right", true), Some(KeyboardAction::Right));
            assert_eq!(state.update("up", false), Some(KeyboardAction::Up));

            let buttons = state.controller_buttons();
            assert!(!buttons.up);
            assert!(buttons.right);
            assert_eq!(state.update("right", false), Some(KeyboardAction::Right));
            assert_eq!(state.controller_buttons(), ControllerButtons::default());
        }

        #[test]
        fn keyboard_transport_actions_are_edge_triggered_until_key_release() {
            let mut state = KeyboardInputState::default();
            assert_eq!(state.update("start", true), Some(KeyboardAction::Start));
            assert_eq!(state.update("start", true), None);
            assert_eq!(state.update("start", false), Some(KeyboardAction::Start));
            assert_eq!(state.update("start", true), Some(KeyboardAction::Start));
        }

        #[test]
        fn keyboard_focus_loss_releases_all_held_controller_buttons() {
            let mut state = KeyboardInputState::default();
            state.update("left", true);
            state.update("button1", true);
            assert_ne!(state.controller_buttons(), ControllerButtons::default());

            state.clear();

            assert_eq!(state.controller_buttons(), ControllerButtons::default());
        }

        #[test]
        fn video_only_capture_flag_accepts_only_explicit_true_values() {
            assert!(video_only_capture_requested(Some("1")));
            assert!(video_only_capture_requested(Some("true")));
            assert!(video_only_capture_requested(Some("TRUE")));
            assert!(!video_only_capture_requested(None));
            assert!(!video_only_capture_requested(Some("0")));
            assert!(!video_only_capture_requested(Some("false")));
            assert!(!video_only_capture_requested(Some("")));
        }

        #[test]
        fn host_duration_window_reports_nearest_rank_percentiles() {
            let mut samples = HostDurationWindow::default();
            for value in [10, 20, 30, 40, 50] {
                samples.push(value);
            }

            assert_eq!(samples.len(), 5);
            assert_eq!(samples.percentile_ns(50), 30);
            assert_eq!(samples.percentile_ns(95), 50);
            assert_eq!(samples.percentile_ns(99), 50);
        }

        #[test]
        fn host_duration_window_retains_only_the_latest_512_samples() {
            let mut samples = HostDurationWindow::default();
            for value in 1..=513 {
                samples.push(value);
            }

            assert_eq!(samples.len(), HOST_PACING_WINDOW_CAPACITY);
            assert_eq!(samples.percentile_ns(50), 257);
            assert_eq!(samples.percentile_ns(95), 488);
            assert_eq!(samples.percentile_ns(99), 508);
        }

        #[test]
        fn audio_output_delta_uses_consumed_plus_zero_filled_frames_as_output_total() {
            let previous = AudioOutputSnapshot {
                sample_rate_hz: 48_000,
                queue_capacity_frames: 4_096,
                accepted_frames: 10_000,
                consumed_frames: 9_000,
                underrun_frames: 1_000,
                ..AudioOutputSnapshot::default()
            };
            let current = AudioOutputSnapshot {
                sample_rate_hz: 48_000,
                queue_capacity_frames: 4_096,
                accepted_frames: 58_000,
                consumed_frames: 54_000,
                underrun_frames: 5_000,
                underrun_events: 20,
                ..AudioOutputSnapshot::default()
            };

            let delta = current.delta_since(previous);
            assert!(!delta.counter_reset);
            assert_eq!(delta.accepted_frames, 48_000);
            assert_eq!(delta.consumed_frames, 45_000);
            assert_eq!(delta.underrun_frames, 4_000);
            assert_eq!(delta.requested_frames(), 49_000);
            assert!((delta.zero_fill_percent() - (4_000.0 / 49_000.0 * 100.0)).abs() < 0.0001);
        }

        #[test]
        fn audio_output_delta_zero_fill_window_is_zero_until_rate_is_valid() {
            let delta = AudioOutputDelta {
                consumed_frames: 75,
                underrun_frames: 25,
                ..AudioOutputDelta::default()
            };

            assert_eq!(delta.zero_fill_percent_for_window(false), 0.0);
            assert_eq!(delta.zero_fill_percent_for_window(true), 25.0);
        }

        #[test]
        fn audio_output_delta_starts_a_new_window_when_counters_reset() {
            let previous = AudioOutputSnapshot {
                sample_rate_hz: 48_000,
                queue_capacity_frames: 4_096,
                accepted_frames: 100_000,
                consumed_frames: 99_000,
                underrun_frames: 1_000,
                ..AudioOutputSnapshot::default()
            };
            let current = AudioOutputSnapshot {
                sample_rate_hz: 48_000,
                queue_capacity_frames: 4_096,
                accepted_frames: 480,
                consumed_frames: 450,
                underrun_frames: 30,
                underrun_events: 2,
                ..AudioOutputSnapshot::default()
            };

            let delta = current.delta_since(previous);
            assert!(delta.counter_reset);
            assert_eq!(delta.accepted_frames, 480);
            assert_eq!(delta.consumed_frames, 450);
            assert_eq!(delta.underrun_frames, 30);
            assert_eq!(delta.requested_frames(), 480);
        }

        #[test]
        fn host_frame_pacing_records_wakeup_and_consumption_distributions() {
            let mut window = HostFramePacingWindow::new();
            let start = Instant::now();
            for (index, micros) in [10, 20, 30, 40, 50].into_iter().enumerate() {
                window.record_wakeup(start + Duration::from_micros((index as u64) * micros));
            }
            assert_eq!(window.wake_interval_distribution.len(), 4);
            assert!(window.wake_interval_distribution.percentile_ns(95) > 0);

            window.record_frame(start + Duration::from_millis(1), 1, start);
            window.record_frame(
                start + Duration::from_millis(2),
                2,
                start + Duration::from_millis(1),
            );
            assert_eq!(window.consumed_interval_distribution.len(), 1);
            assert_eq!(
                window.consumed_interval_distribution.percentile_ns(50),
                1_000_000
            );
            assert_eq!(window.mailbox_residence_distribution.len(), 2);
        }

        #[test]
        fn host_frame_pacing_fps_counts_successful_uploads_not_skipped_sequences() {
            let mut window = HostFramePacingWindow::new();
            let start = Instant::now();
            window.last_fps_sample = start;
            window.last_fps_uploaded = 0;

            for (elapsed_ms, sequence) in [(150, 1), (300, 3), (450, 5), (600, 8)] {
                let now = start + Duration::from_millis(elapsed_ms);
                window.record_frame(now, sequence, now);
                let fps = window.record_upload(now);
                if elapsed_ms < 600 {
                    assert_eq!(fps, None);
                } else {
                    // Four successful queue uploads occurred in 600 ms.
                    // Sequence gaps do not inflate this rate. This is not
                    // a physical present/scanout measurement.
                    assert!((fps.unwrap() - (4.0 / 0.6)).abs() < 0.001);
                }
            }

            assert_eq!(window.consumed, 4);
            assert_eq!(window.uploaded, 4);
            assert_eq!(window.sequence_gaps, 4);
        }

        #[test]
        fn host_frame_pacing_does_not_count_consumed_frames_as_gpu_uploads() {
            let mut window = HostFramePacingWindow::new();
            let start = Instant::now();
            window.record_frame(start, 1, start);
            window.record_frame(start + Duration::from_millis(100), 2, start);
            assert_eq!(window.consumed, 2);
            assert_eq!(window.uploaded, 0);
            let fps = window
                .record_upload(start + Duration::from_millis(600))
                .unwrap();
            assert!((fps - (1.0 / 0.6)).abs() < 0.001);
        }

        #[test]
        fn host_frame_pacing_window_records_callback_and_mailbox_take_costs() {
            let mut window = HostFramePacingWindow::new();
            window.record_callback(Duration::from_micros(120));
            window.record_mailbox_take(Duration::from_micros(8));

            assert_eq!(window.callback_count, 1);
            assert_eq!(window.callback_sum_ns, 120_000);
            assert_eq!(window.callback_max_ns, 120_000);
            assert_eq!(window.mailbox_take_count, 1);
            assert_eq!(window.mailbox_take_sum_ns, 8_000);
            assert_eq!(window.mailbox_take_max_ns, 8_000);
        }

        #[test]
        fn classic_material_transport_icons_load_as_slint_svg_images() {
            let icons = load_transport_icons().unwrap();
            for icon in [
                icons.play,
                icons.pause,
                icons.stop,
                icons.reset,
                icons.step_frame,
            ] {
                assert!(icon.size().width > 0);
                assert!(icon.size().height > 0);
            }
        }

        #[test]
        fn rgb888_conversion_respects_row_stride_and_sets_opaque_alpha() {
            let frame = VideoFrame::new(
                7,
                2,
                2,
                PixelFormat::Rgb888,
                8,
                vec![
                    255, 0, 0, 0, 255, 0, 90, 91, 0, 0, 255, 255, 255, 255, 92, 93,
                ],
            )
            .unwrap();

            assert_eq!(
                convert_rgb888_to_rgba8(&frame).unwrap(),
                vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                ]
            );
        }

        fn zip_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
            use std::io::Write;
            use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, contents) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(contents).unwrap();
            }
            writer.finish().unwrap().into_inner()
        }

        #[test]
        fn zip_rom_loader_reads_single_sms_entry_in_memory() {
            let bytes = zip_archive(&[("games/sonic.sms", b"SMS-ROM")]);
            assert_eq!(extract_rom_from_zip(&bytes).unwrap(), b"SMS-ROM");
        }
        #[test]
        fn zip_rom_loader_accepts_sg1000_entry() {
            let bytes = zip_archive(&[("games/space-mountain.sg", b"SG1000-ROM")]);
            assert_eq!(extract_rom_from_zip(&bytes).unwrap(), b"SG1000-ROM");
        }

        #[test]
        fn zip_rom_loader_rejects_archives_without_roms_or_with_multiple_roms() {
            let no_rom = zip_archive(&[("readme.txt", b"not a rom")]);
            assert!(extract_rom_from_zip(&no_rom).is_err());

            let multiple = zip_archive(&[("sonic.sms", b"first"), ("alex-kidd.bin", b"second")]);
            assert!(extract_rom_from_zip(&multiple).is_err());
        }

        #[test]
        fn zip_rom_loader_rejects_invalid_archive_bytes() {
            assert!(extract_rom_from_zip(b"not a ZIP archive").is_err());
        }

        #[test]
        fn conventional_bios_loader_rejects_builtin_game_firmware() {
            let settings = SmsWorkerSettings {
                region: "usa".to_owned(),
                sms_system: "sms1".to_owned(),
                firmware_directory: "unused-firmware-directory".to_owned(),
                firmware_boot_enabled: true,
                firmware_brazil_selected: String::new(),
                firmware_usa_selected: "HangOnV34 — Hang On USA/Europe v3.4 (128 KiB)".to_owned(),
                firmware_europe_selected: String::new(),
                firmware_japan_selected: String::new(),
                firmware_korea_selected: String::new(),
                vdp_revision: SmsVdpRevision::Sega3155124,
                audio_volume: 70,
                dsp_config: AnalogOutputConfig::default(),
                sg1000_dsp_config: AnalogOutputConfig::default(),
                renderer_mode: "psg".to_owned(),
            };

            let error = load_selected_sms_bios(&[0; 0x4000], &settings).unwrap_err();
            assert!(error.contains("não uma BIOS convencional de 8 KiB"));
        }

        #[test]
        fn conventional_bios_loader_rejects_region_and_console_mismatches() {
            let mut settings = SmsWorkerSettings {
                region: "brazil".to_owned(),
                sms_system: "sms1".to_owned(),
                firmware_directory: "unused-firmware-directory".to_owned(),
                firmware_boot_enabled: true,
                firmware_brazil_selected: "EuropeV20 — Master System Europe v2.0 (8 KiB)"
                    .to_owned(),
                firmware_usa_selected: String::new(),
                firmware_europe_selected: String::new(),
                firmware_japan_selected: String::new(),
                firmware_korea_selected: String::new(),
                vdp_revision: SmsVdpRevision::Sega3155124,
                audio_volume: 70,
                dsp_config: AnalogOutputConfig::default(),
                sg1000_dsp_config: AnalogOutputConfig::default(),
                renderer_mode: "psg".to_owned(),
            };

            let region_error = load_selected_sms_bios(&[0; 0x4000], &settings).unwrap_err();
            assert!(region_error.contains("não é compatível"));

            settings.region = "europe".to_owned();
            settings.sms_system = "sms2".to_owned();
            settings.firmware_brazil_selected.clear();
            settings.firmware_europe_selected =
                "EuropeV20 — Master System Europe v2.0 (8 KiB)".to_owned();

            let console_error = load_selected_sms_bios(&[0; 0x4000], &settings).unwrap_err();
            assert!(console_error.contains("não é compatível"));
        }

        #[test]
        fn disabled_bios_preference_keeps_direct_cartridge_boot() {
            let settings = SmsWorkerSettings {
                region: "usa".to_owned(),
                sms_system: "sms1".to_owned(),
                firmware_directory: String::new(),
                firmware_boot_enabled: false,
                firmware_brazil_selected: String::new(),
                firmware_usa_selected: String::new(),
                firmware_europe_selected: String::new(),
                firmware_japan_selected: String::new(),
                firmware_korea_selected: String::new(),
                vdp_revision: SmsVdpRevision::Sega3155124,
                audio_volume: 70,
                dsp_config: AnalogOutputConfig::default(),
                sg1000_dsp_config: AnalogOutputConfig::default(),
                renderer_mode: "psg".to_owned(),
            };

            assert!(
                load_selected_sms_bios(&[0; 0x4000], &settings)
                    .unwrap()
                    .is_none()
            );
        }

        #[test]
        fn firmware_region_options_expose_sms2_regional_firmware_and_keep_variants_separate() {
            let catalog = SmsFirmwareCatalog::sms_default();
            let entries = [
                SmsFirmwareScanEntry {
                    path: PathBuf::from("samsung-gamboy.sms"),
                    size_bytes: 128 * 1024,
                    sha256: None,
                    firmware_id: Some(SmsFirmwareId::SamsungGamBoyIiKoreaAlexKidd),
                    status: SmsFirmwareScanStatus::Identified,
                },
                SmsFirmwareScanEntry {
                    path: PathBuf::from("sonic-europe.sms"),
                    size_bytes: 256 * 1024,
                    sha256: None,
                    firmware_id: Some(SmsFirmwareId::SonicEurope),
                    status: SmsFirmwareScanStatus::Identified,
                },
                SmsFirmwareScanEntry {
                    path: PathBuf::from("alex-kidd-international.sms"),
                    size_bytes: 128 * 1024,
                    sha256: None,
                    firmware_id: Some(SmsFirmwareId::AlexKiddUsEuBrazilV1990),
                    status: SmsFirmwareScanStatus::Identified,
                },
            ];

            let sms2_korea =
                firmware_options_for_region(&catalog, &entries, "sms2", SmsRegion::Korea);
            assert_eq!(sms2_korea.len(), 1);
            assert!(sms2_korea[0].contains("SamsungGamBoyIiKoreaAlexKidd"));

            let sms2_europe =
                firmware_options_for_region(&catalog, &entries, "sms2", SmsRegion::Europe);
            assert_eq!(sms2_europe.len(), 2);
            assert!(sms2_europe.iter().any(|item| item.contains("SonicEurope")));
            assert!(
                sms2_europe
                    .iter()
                    .any(|item| item.contains("AlexKiddUsEuBrazilV1990"))
            );

            let sms2_brazil =
                firmware_options_for_region(&catalog, &entries, "sms2", SmsRegion::Brazil);
            assert_eq!(sms2_brazil.len(), 1);
            assert!(sms2_brazil[0].contains("AlexKiddUsEuBrazilV1990"));

            let sms2_usa = firmware_options_for_region(&catalog, &entries, "sms2", SmsRegion::Usa);
            assert_eq!(sms2_usa.len(), 1);
            assert!(sms2_usa[0].contains("AlexKiddUsEuBrazilV1990"));

            assert!(
                firmware_options_for_region(&catalog, &entries, "sms1", SmsRegion::Korea)
                    .is_empty()
            );
            let sms1_brazil =
                firmware_options_for_region(&catalog, &entries, "sms1", SmsRegion::Brazil);
            assert_eq!(sms1_brazil.len(), 1);
            assert!(sms1_brazil[0].contains("AlexKiddUsEuBrazilV1990"));
        }

        #[test]
        fn selected_sms_system_maps_to_the_matching_vdp_asic() {
            assert_eq!(
                sms_vdp_revision_for_system("sms1").unwrap(),
                SmsVdpRevision::Sega3155124
            );
            assert_eq!(
                sms_vdp_revision_for_system("sms2").unwrap(),
                SmsVdpRevision::Sega3155246
            );
            assert!(sms_vdp_revision_for_system("sg1000").is_err());
        }

        #[test]
        fn sms_frame_extent_accepts_only_documented_sms_vertical_modes() {
            assert!(is_supported_sms_frame_extent(256, 192));
            assert!(is_supported_sms_frame_extent(256, 224));
            assert!(is_supported_sms_frame_extent(256, 240));
            assert!(!is_supported_sms_frame_extent(256, 193));
            assert!(!is_supported_sms_frame_extent(256, 223));
            assert!(!is_supported_sms_frame_extent(256, 241));
            assert!(!is_supported_sms_frame_extent(255, 224));
        }

        #[test]
        fn wgpu_texture_extent_and_upload_layout_follow_dynamic_height() {
            for height in [192, 224, 240] {
                let extent = texture_extent(256, height);
                assert_eq!(extent.width, 256);
                assert_eq!(extent.height, height);
                assert_eq!(extent.depth_or_array_layers, 1);

                let layout = texture_data_layout(256, height);
                assert_eq!(layout.bytes_per_row, Some(1024));
                assert_eq!(layout.rows_per_image, Some(height));
            }
        }

        #[test]
        fn rgb888_conversion_preserves_all_extended_scanlines() {
            for height in [224_u32, 240_u32] {
                let width = 2_u32;
                let stride = 8_usize;
                let mut source = vec![0_u8; stride * height as usize];
                for y in 0..height as usize {
                    let row = y * stride;
                    source[row] = y as u8;
                    source[row + 1] = 0xA5;
                    source[row + 2] = 0x5A;
                    source[row + 3] = 0x3C;
                    source[row + 4] = 0xC3;
                    source[row + 5] = 0x69;
                    source[row + 6] = 0xDE;
                    source[row + 7] = 0xAD;
                }

                let frame =
                    VideoFrame::new(9, width, height, PixelFormat::Rgb888, stride, source).unwrap();
                let rgba = convert_rgb888_to_rgba8(&frame).unwrap();

                assert_eq!(rgba.len(), width as usize * height as usize * 4);
                let last_row = (height as usize - 1) * width as usize * 4;
                assert_eq!(
                    &rgba[last_row..last_row + 8],
                    &[height as u8 - 1, 0xA5, 0x5A, 255, 0x3C, 0xC3, 0x69, 255]
                );
            }
        }

        #[test]
        fn rgb888_conversion_rejects_other_pixel_formats() {
            let frame =
                VideoFrame::new(1, 1, 1, PixelFormat::Rgba8888, 4, vec![1, 2, 3, 4]).unwrap();
            assert!(
                convert_rgb888_to_rgba8(&frame)
                    .unwrap_err()
                    .contains("esperado RGB888")
            );
        }

        #[test]
        fn sms_worker_publishes_complete_vdp_frames_to_desktop_mailbox() {
            let worker = SmsTemporalWorker::new(vec![0; 0x4000], "usa").unwrap();
            let mailbox = FrameMailbox::new();
            worker.attach_video_mailbox(mailbox.clone()).unwrap();
            worker.step_frame(100_000).unwrap();

            let frame = mailbox.take_latest().unwrap().unwrap();
            assert_eq!(frame.sequence(), 1);
            assert_eq!(frame.width(), SMS_WIDTH);
            assert_eq!(frame.height(), SMS_HEIGHT);
            assert_eq!(frame.pixel_format(), PixelFormat::Rgb888);
            assert_eq!(frame.pixels().len(), (SMS_WIDTH * SMS_HEIGHT * 3) as usize);
            assert!(worker.machine_time_ticks() > 0);
            worker.detach_video_mailbox().unwrap();
        }
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows_host::run()
}

#[cfg(not(windows))]
fn main() {
    println!("sega-desktop R2 currently targets Windows x64 with Vulkan.");
}
