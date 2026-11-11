use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::debugger_window::{
    DebuggerWindowState, parse_debug_address, parse_saved_position, parse_saved_root_size,
    parse_saved_size, render_debugger_contents,
};
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rfd::FileDialog;
use sega_audio_output::{NativeWasapiOutput, NativeXAudio2Output};
use sega_input_runtime::{
    ActionBinding, ActionId, ActionState, AxisDirection, DeviceClass, DeviceDescriptor, DeviceId,
    DeviceState, InputBackend, InputBuffers, InputProfile, PhysicalControl,
    sdl3_backend::Sdl3Backend,
};
use sega_observability::{
    LogConfig, SessionMetadata, current as current_logging, init as init_logging,
};
use sega_sms_core::ControllerButtons;
use sega_sms_runtime::SmsTemporalWorker;
use sega_video_runtime::d3d12::{D3d12ChildWindow, D3d12MailboxSession};
use zip::ZipArchive;

const MAX_ROM_SIZE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ROM_SIZE_BYTES_U64: u64 = 16 * 1024 * 1024;
const APP_DATA_DIRECTORY: &str = "Sega Emulator";
const LAST_ROM_DIRECTORY_FILE: &str = "last_rom_directory.txt";
const RECENT_ROM_LIST_FILE: &str = "recent_roms.txt";
const UI_SETTINGS_FILE: &str = "ui_settings.txt";
const MAX_RECENT_ROMS: usize = 10;
const MENU_BAR_HEIGHT: f32 = 32.0;

fn record_ui_diagnostic(event: &'static str, details: impl std::fmt::Display) {
    if current_logging().is_some_and(sega_observability::LoggingHandle::is_enabled) {
        sega_observability::tracing::debug!(target: "sega::ui::menu", event, details = %details);
    }
}

/// Native eframe application shell connected to the Rust SMS temporal runtime.
///
/// The independent flags represent orthogonal UI preferences and lifecycle states;
/// consolidating them would obscure rather than clarify their ownership.
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent UI preferences and lifecycle flags are intentionally orthogonal."
)]
pub struct SegaApp {
    rom_path: String,
    region: String,
    last_rom_directory: Option<PathBuf>,
    rom_dialog_requested: bool,
    recent_roms: Vec<PathBuf>,
    recent_rom_requested: Option<PathBuf>,
    footer_mode: FooterMode,
    aspect_ratio: AspectRatioMode,
    audio_state: AudioState,
    audio_backend: AudioBackend,
    audio_volume: u8,
    last_controller: Option<ControllerButtons>,
    pause_key_down: bool,
    input_backend: Option<Result<Sdl3Backend, String>>,
    input_buffers: InputBuffers,
    input_devices: Vec<DeviceDescriptor>,
    selected_gamepad: Option<DeviceId>,
    keyboard_bindings: KeyboardBindings,
    pending_key_binding: Option<KeyboardAction>,
    gamepad_bindings: GamepadBindings,
    gamepad_profile: InputProfile,
    pending_gamepad_binding: Option<GamepadAction>,
    input_mapping_error: Option<String>,
    active_menu: Option<TopLevelMenu>,
    menu_anchor: egui::Pos2,
    menu_popup_size: egui::Vec2,
    menu_last_focused: Option<bool>,
    native_viewport: NativeViewportState,
    diagnostics_enabled: bool,
    diagnostics_error: Option<String>,
    fps_overlay_enabled: bool,
    fps_overlay_was_active: bool,
    fps_overlay_size: FpsOverlaySize,
    fps_counter: FpsCounter,
    theme_initialized: bool,
    debugger_window_open: Arc<AtomicBool>,
    debugger_window: Arc<Mutex<DebuggerWindowState>>,
    last_settings_save: Instant,
    main_window_position: Option<egui::Pos2>,
    main_window_size: Option<egui::Vec2>,
    main_window_maximized: bool,
    main_window_fullscreen: bool,
    main_window_geometry_restored: bool,
}

impl Default for SegaApp {
    fn default() -> Self {
        let gamepad_bindings = load_gamepad_bindings();
        let gamepad_profile = gamepad_bindings.to_profile();
        let mut app = Self {
            rom_path: String::new(),
            region: "usa".to_owned(),
            last_rom_directory: load_last_rom_directory(),
            rom_dialog_requested: false,
            recent_roms: load_recent_roms(),
            recent_rom_requested: None,
            footer_mode: FooterMode::Visible,
            aspect_ratio: AspectRatioMode::FourThree,
            audio_state: AudioState::Enabled,
            audio_backend: AudioBackend::Wasapi,
            audio_volume: 100,
            last_controller: None,
            pause_key_down: false,
            input_backend: None,
            input_buffers: InputBuffers::default(),
            input_devices: Vec::new(),
            selected_gamepad: None,
            keyboard_bindings: load_keyboard_bindings(),
            pending_key_binding: None,
            gamepad_bindings,
            gamepad_profile,
            pending_gamepad_binding: None,
            input_mapping_error: None,
            active_menu: None,
            menu_anchor: egui::Pos2::ZERO,
            menu_popup_size: egui::vec2(360.0, 280.0),
            menu_last_focused: None,
            native_viewport: NativeViewportState::default(),
            diagnostics_enabled: false,
            diagnostics_error: None,
            fps_overlay_enabled: true,
            fps_overlay_was_active: false,
            fps_overlay_size: FpsOverlaySize::Medium,
            fps_counter: FpsCounter::default(),
            theme_initialized: false,
            debugger_window_open: Arc::new(AtomicBool::new(false)),
            debugger_window: Arc::new(Mutex::new(DebuggerWindowState::default())),
            last_settings_save: Instant::now(),
            main_window_position: None,
            main_window_size: None,
            main_window_maximized: false,
            main_window_fullscreen: false,
            main_window_geometry_restored: false,
        };
        app.restore_ui_settings();
        if app.diagnostics_enabled {
            app.set_diagnostics_enabled(true);
        }
        app
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AspectRatioMode {
    OneToOne,
    FourThree,
    SixteenNine,
}

impl AspectRatioMode {
    const fn value(self) -> f32 {
        match self {
            Self::OneToOne => 1.0,
            Self::FourThree => 4.0 / 3.0,
            Self::SixteenNine => 16.0 / 9.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopLevelMenu {
    System,
    File,
    Video,
    Audio,
    Controls,
    Options,
    Debug,
}

fn close_menu_if_current(
    active_menu: &mut Option<TopLevelMenu>,
    rendered_menu: TopLevelMenu,
    should_close: bool,
) {
    if should_close && *active_menu == Some(rendered_menu) {
        *active_menu = None;
    }
}

impl TopLevelMenu {
    const fn title(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::File => "Arquivo",
            Self::Video => "Vídeo",
            Self::Audio => "Áudio",
            Self::Controls => "Controles",
            Self::Options => "Opções",
            Self::Debug => "DEBUG",
        }
    }

    fn minimum_size(self) -> egui::Vec2 {
        match self {
            Self::System => egui::vec2(340.0, 500.0),
            Self::File => egui::vec2(420.0, 280.0),
            Self::Video => egui::vec2(320.0, 220.0),
            Self::Audio => egui::vec2(340.0, 280.0),
            Self::Controls => egui::vec2(460.0, 420.0),
            Self::Options => egui::vec2(440.0, 380.0),
            Self::Debug => egui::vec2(640.0, 540.0),
        }
    }
}

const FPS_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug)]
struct FpsCounter {
    last_sequence: u64,
    last_sample: Instant,
    fps: f32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self {
            last_sequence: 0,
            last_sample: Instant::now(),
            fps: 0.0,
        }
    }
}

impl FpsCounter {
    fn reset(&mut self, sequence: u64, now: Instant) {
        self.last_sequence = sequence;
        self.last_sample = now;
        self.fps = 0.0;
    }

    fn update(&mut self, sequence: u64, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last_sample);
        if elapsed < FPS_SAMPLE_INTERVAL {
            return false;
        }
        let frames = u16::try_from(sequence.saturating_sub(self.last_sequence)).unwrap_or(u16::MAX);
        self.fps = f32::from(frames) / elapsed.as_secs_f32();
        self.last_sequence = sequence;
        self.last_sample = now;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FpsOverlaySize {
    Small,
    Medium,
    Large,
}

impl FpsOverlaySize {
    const fn dimensions(self) -> egui::Vec2 {
        match self {
            Self::Small => egui::vec2(88.0, 26.0),
            Self::Medium => egui::vec2(104.0, 30.0),
            Self::Large => egui::vec2(128.0, 36.0),
        }
    }
    const fn font_size(self) -> f32 {
        match self {
            Self::Small => 14.0,
            Self::Medium => 18.0,
            Self::Large => 22.0,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Small => "Pequeno",
            Self::Medium => "Médio",
            Self::Large => "Grande",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioState {
    Enabled,
    Disabled,
}

impl AudioState {
    const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioBackend {
    Wasapi,
    XAudio2,
}

#[derive(Debug, Clone, Copy)]
enum AudioCommand {
    Configure,
    SetVolume(u8),
    Disable,
}

#[derive(Debug, Clone, Copy)]
enum UiCommand {
    Start,
    Pause,
    Stop,
    Reset,
    StepFrame,
    CloseMachine,
}

#[derive(Default)]
struct MenuPopupActions {
    command: Option<UiCommand>,
    audio_command: Option<AudioCommand>,
    fullscreen_requested: Option<bool>,
    exit_requested: bool,
    input_updated: bool,
    root_fullscreen: bool,
    diagnostics_command: Option<bool>,
}

/// Owns the UI-thread HWND child, SMS worker and D3D12 presentation session.
///
/// Shutdown order is deliberate: stop the machine, detach its borrowed mailbox,
/// join the machine worker, close/join D3D12 presentation, then destroy the HWND.
#[derive(Default)]
struct NativeViewportState {
    parent_hwnd: Option<usize>,
    session: Option<D3d12MailboxSession>,
    child: Option<D3d12ChildWindow>,
    worker: Option<SmsTemporalWorker>,
    last_bounds: Option<(i32, i32, i32, i32)>,
    ready: bool,
    video_attached: bool,
    start_requested: bool,
    error: Option<String>,
}

impl NativeViewportState {
    fn status(&self) -> String {
        if let Some(error) = self.error.as_ref() {
            error.clone()
        } else if self.child.is_none() {
            "Aguardando janela nativa".to_owned()
        } else if !self.ready {
            "Inicializando D3D12 em worker dedicado".to_owned()
        } else if let Some(worker) = self.worker.as_ref() {
            if worker.is_running() {
                "SMS em execução — vídeo D3D12 ativo".to_owned()
            } else if let Some(error) = worker.runtime_error() {
                format!("Runtime SMS pausado: {error}")
            } else {
                "SMS carregado e pausado — vídeo D3D12 ativo".to_owned()
            }
        } else {
            "D3D12 pronto — aguardando ROM SMS".to_owned()
        }
    }

    fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.stop();
            if self.video_attached {
                let _ = worker.detach_video_mailbox();
            }
            drop(worker);
        }
        self.video_attached = false;

        if let Some(session) = self.session.take() {
            // The presentation worker is joined while its borrowed HWND is alive.
            let _ = session.shutdown();
        }
        self.child = None;
        self.parent_hwnd = None;
        self.last_bounds = None;
        self.ready = false;
        self.start_requested = false;
    }

    fn replace_worker(&mut self, worker: SmsTemporalWorker) {
        if let Some(previous) = self.worker.take() {
            let _ = previous.stop();
            if self.video_attached {
                let _ = previous.detach_video_mailbox();
            }
            drop(previous);
        }
        self.video_attached = false;
        self.worker = Some(worker);
        self.start_requested = true;
        self.error = None;
    }

    fn close_worker(&mut self) {
        self.start_requested = false;
        if let Some(worker) = self.worker.take() {
            let _ = worker.stop();
            if self.video_attached {
                let _ = worker.detach_video_mailbox();
            }
            drop(worker);
        }
        self.video_attached = false;
        self.error = None;
    }
}

impl Drop for NativeViewportState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Reveal the fullscreen menu while the pointer is at the top edge or a menu is open.
fn should_reveal_fullscreen_menu(
    fullscreen: bool,
    menu_open: bool,
    pointer_y: Option<f32>,
    root_top: f32,
) -> bool {
    !fullscreen || menu_open || pointer_y.is_some_and(|y| y <= root_top + 88.0)
}

fn saved_ui_setting<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents.lines().find_map(|line| {
        let (candidate, value) = line.split_once('=')?;
        (candidate == key).then_some(value)
    })
}

impl SegaApp {
    /// Restore the user's UI preferences from the per-user settings file.
    fn restore_ui_settings(&mut self) {
        let Some(directory) = rom_settings_directory() else {
            return;
        };
        let Ok(contents) = fs::read_to_string(directory.join(UI_SETTINGS_FILE)) else {
            return;
        };

        self.restore_debugger_settings(&contents);
        self.restore_audio_settings(&contents);
        self.restore_display_settings(&contents);
    }

    fn restore_debugger_settings(&mut self, contents: &str) {
        if let Some(rom_path) = saved_ui_setting(contents, "rom_path") {
            rom_path.clone_into(&mut self.rom_path);
        }

        let Ok(mut debugger_window) = self.debugger_window.lock() else {
            return;
        };
        if let Some(memory_address) = saved_ui_setting(contents, "debugger_memory_address") {
            memory_address.clone_into(&mut debugger_window.memory_address);
        }
        if let Some(breakpoint_input) = saved_ui_setting(contents, "debugger_breakpoint_input") {
            breakpoint_input.clone_into(&mut debugger_window.breakpoint_input);
        }
        if let Some(rom_path) = saved_ui_setting(contents, "debugger_breakpoints_rom") {
            rom_path.clone_into(&mut debugger_window.breakpoint_rom_path);
        }
        if let Some(breakpoints) = saved_ui_setting(contents, "debugger_breakpoints") {
            debugger_window.breakpoints = breakpoints
                .split(',')
                .filter_map(parse_debug_address)
                .take(64)
                .collect();
        }
        if !debugger_window
            .breakpoint_rom_path
            .eq_ignore_ascii_case(&self.rom_path)
        {
            debugger_window.breakpoints.clear();
        }
        if let Some(enabled) = saved_ui_setting(contents, "debugger_trace_enabled")
            .and_then(|value| value.parse().ok())
        {
            debugger_window.trace_capture_enabled = enabled;
        }
        if let Some(open) =
            saved_ui_setting(contents, "debugger_open").and_then(|value| value.parse().ok())
        {
            self.debugger_window_open.store(open, Ordering::Release);
        }
        if let Some(position) =
            saved_ui_setting(contents, "debugger_position").and_then(parse_saved_position)
        {
            debugger_window.window_position = Some(position);
        }
        if let Some(size) = saved_ui_setting(contents, "debugger_size").and_then(parse_saved_size) {
            debugger_window.window_size = Some(size);
        }
    }

    fn restore_audio_settings(&mut self, contents: &str) {
        if let Some(region) = saved_ui_setting(contents, "region")
            .filter(|value| matches!(*value, "usa" | "brazil" | "europe" | "japan"))
        {
            region.clone_into(&mut self.region);
        }
        if let Some(mode) = saved_ui_setting(contents, "footer") {
            self.footer_mode = if mode == "hidden" {
                FooterMode::Hidden
            } else {
                FooterMode::Visible
            };
        }
        if let Some(mode) = saved_ui_setting(contents, "audio_state") {
            self.audio_state = if mode == "disabled" {
                AudioState::Disabled
            } else {
                AudioState::Enabled
            };
        }
        if let Some(backend) = saved_ui_setting(contents, "audio_backend") {
            self.audio_backend = if backend == "xaudio2" {
                AudioBackend::XAudio2
            } else {
                AudioBackend::Wasapi
            };
        }
        if let Some(volume) =
            saved_ui_setting(contents, "audio_volume").and_then(|value| value.parse::<u8>().ok())
        {
            self.audio_volume = volume.min(100);
        }
        if let Some(enabled) =
            saved_ui_setting(contents, "diagnostics_enabled").and_then(|value| value.parse().ok())
        {
            self.diagnostics_enabled = enabled;
        }
    }

    fn restore_display_settings(&mut self, contents: &str) {
        if let Some(mode) = saved_ui_setting(contents, "aspect_ratio") {
            self.aspect_ratio = match mode {
                "1:1" => AspectRatioMode::OneToOne,
                "16:9" => AspectRatioMode::SixteenNine,
                _ => AspectRatioMode::FourThree,
            };
        }
        if let Some(enabled) =
            saved_ui_setting(contents, "fps_overlay_enabled").and_then(|value| value.parse().ok())
        {
            self.fps_overlay_enabled = enabled;
        }
        if let Some(size) = saved_ui_setting(contents, "fps_overlay_size") {
            self.fps_overlay_size = match size {
                "small" => FpsOverlaySize::Small,
                "large" => FpsOverlaySize::Large,
                _ => FpsOverlaySize::Medium,
            };
        }
        if let Some(position) =
            saved_ui_setting(contents, "main_window_position").and_then(parse_saved_position)
        {
            self.main_window_position = Some(position);
        }
        if let Some(size) =
            saved_ui_setting(contents, "main_window_size").and_then(parse_saved_root_size)
        {
            self.main_window_size = Some(size);
        }
        if let Some(maximized) =
            saved_ui_setting(contents, "main_window_maximized").and_then(|value| value.parse().ok())
        {
            self.main_window_maximized = maximized;
        }
        if let Some(fullscreen) = saved_ui_setting(contents, "main_window_fullscreen")
            .and_then(|value| value.parse().ok())
        {
            self.main_window_fullscreen = fullscreen;
        }
    }

    /// Persist the complete set of user-configurable UI preferences.
    fn save_ui_settings(&self) -> std::io::Result<()> {
        let directory = rom_settings_directory()
            .ok_or_else(|| std::io::Error::other("APPDATA não está definido"))?;
        fs::create_dir_all(&directory)?;
        let debugger_window = self
            .debugger_window
            .lock()
            .map_err(|_| std::io::Error::other("debugger settings lock poisoned"))?;

        let aspect_ratio = match self.aspect_ratio {
            AspectRatioMode::OneToOne => "1:1",
            AspectRatioMode::FourThree => "4:3",
            AspectRatioMode::SixteenNine => "16:9",
        };
        let footer = if self.footer_mode.is_visible() {
            "visible"
        } else {
            "hidden"
        };
        let audio_state = if self.audio_state.is_enabled() {
            "enabled"
        } else {
            "disabled"
        };
        let audio_backend = match self.audio_backend {
            AudioBackend::Wasapi => "wasapi",
            AudioBackend::XAudio2 => "xaudio2",
        };
        let fps_size = match self.fps_overlay_size {
            FpsOverlaySize::Small => "small",
            FpsOverlaySize::Medium => "medium",
            FpsOverlaySize::Large => "large",
        };
        let debugger_position = debugger_window
            .window_position
            .map_or_else(String::new, |position| {
                format!("{:.1},{:.1}", position.x, position.y)
            });
        let debugger_size = debugger_window
            .window_size
            .map_or_else(String::new, |size| format!("{:.1},{:.1}", size.x, size.y));
        let main_position = self
            .main_window_position
            .map_or_else(String::new, |position| {
                format!("{:.1},{:.1}", position.x, position.y)
            });
        let main_size = self
            .main_window_size
            .map_or_else(String::new, |size| format!("{:.1},{:.1}", size.x, size.y));
        let breakpoints = debugger_window
            .breakpoints
            .iter()
            .map(|address| format!("{address:04X}"))
            .collect::<Vec<_>>()
            .join(",");

        let contents = format!(
            "rom_path={}\ndebugger_breakpoints_rom={}\ndebugger_memory_address={}\ndebugger_breakpoint_input={}\ndebugger_breakpoints={}\ndebugger_trace_enabled={}\nregion={}\naspect_ratio={}\nfooter={}\naudio_state={}\naudio_backend={}\naudio_volume={}\ndiagnostics_enabled={}\nfps_overlay_enabled={}\nfps_overlay_size={}\ndebugger_open={}\ndebugger_position={}\ndebugger_size={}\nmain_window_position={}\nmain_window_size={}\nmain_window_maximized={}\nmain_window_fullscreen={}\n",
            self.rom_path,
            debugger_window.breakpoint_rom_path,
            debugger_window.memory_address,
            debugger_window.breakpoint_input,
            breakpoints,
            debugger_window.trace_capture_enabled,
            self.region,
            aspect_ratio,
            footer,
            audio_state,
            audio_backend,
            self.audio_volume,
            self.diagnostics_enabled,
            self.fps_overlay_enabled,
            fps_size,
            self.debugger_window_open.load(Ordering::Acquire),
            debugger_position,
            debugger_size,
            main_position,
            main_size,
            self.main_window_maximized,
            self.main_window_fullscreen,
        );
        fs::write(directory.join(UI_SETTINGS_FILE), contents)
    }

    /// Restore root-window geometry once and periodically auto-save all UI preferences.
    fn maintain_window_settings(&mut self, context: &egui::Context) {
        if !self.main_window_geometry_restored {
            if self.main_window_fullscreen {
                context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            } else if self.main_window_maximized {
                context.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            } else {
                if let Some(position) = self.main_window_position {
                    context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
                }
                if let Some(size) = self.main_window_size {
                    context.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                }
            }
            self.main_window_geometry_restored = true;
            return;
        }

        if let Some(position) =
            context.input(|input| input.viewport().outer_rect.map(|rect| rect.min))
        {
            self.main_window_position = Some(position);
        }
        if let Some(size) =
            context.input(|input| input.viewport().inner_rect.map(|rect| rect.size()))
        {
            self.main_window_size = Some(size);
        }
        if let Some(maximized) = context.input(|input| input.viewport().maximized) {
            self.main_window_maximized = maximized;
        }
        if let Some(fullscreen) = context.input(|input| input.viewport().fullscreen) {
            self.main_window_fullscreen = fullscreen;
        }

        if self.last_settings_save.elapsed() >= Duration::from_secs(5) {
            if let Err(error) = self.save_ui_settings() {
                eprintln!("Unable to auto-save UI settings: {error}");
            }
            self.last_settings_save = Instant::now();
        }
    }

    /// Render the debugger in a deferred native viewport to avoid coupling its repaint rate to the game.
    fn render_debugger_window(&mut self, context: &egui::Context) {
        if !self.debugger_window_open.load(Ordering::Acquire) {
            return;
        }

        let Ok(state) = self.debugger_window.lock() else {
            self.debugger_window_open.store(false, Ordering::Release);
            return;
        };
        let window_position = state.window_position;
        let window_size = state.window_size;
        let window_initialized = state.window_initialized;
        drop(state);

        let worker = self
            .native_viewport
            .worker
            .as_ref()
            .map(SmsTemporalWorker::debug_handle);
        let state = Arc::clone(&self.debugger_window);
        let open = Arc::clone(&self.debugger_window_open);
        let mut builder = egui::ViewportBuilder::default()
            .with_title("Sega Emulator — SMS Debugger")
            .with_min_inner_size([760.0, 560.0])
            .with_resizable(true)
            .with_decorations(true)
            .with_taskbar(true);
        if !window_initialized {
            builder = builder.with_inner_size(window_size.unwrap_or(egui::vec2(980.0, 760.0)));
            if let Some(position) = window_position {
                builder = builder.with_position(position);
            }
        }

        context.show_viewport_deferred(
            egui::ViewportId::from_hash_of("sega-sms-debugger-window"),
            builder,
            move |debugger_ui, _viewport_class| {
                let Ok(mut state) = state.lock() else {
                    open.store(false, Ordering::Release);
                    return;
                };
                if debugger_ui.input(|input| input.viewport().close_requested()) {
                    state.window_initialized = false;
                    open.store(false, Ordering::Release);
                    return;
                }

                state.window_initialized = true;
                render_debugger_contents(debugger_ui, worker.as_ref(), &mut state);
                if let Some(position) =
                    debugger_ui.input(|input| input.viewport().outer_rect.map(|rect| rect.min))
                {
                    state.window_position = Some(position);
                }
                if let Some(size) =
                    debugger_ui.input(|input| input.viewport().inner_rect.map(|rect| rect.size()))
                {
                    state.window_size = Some(size);
                }
            },
        );
    }

    /// Render the complete shell in the root window for native and headless use.
    pub fn render(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let mut viewport_rect = egui::Rect::NOTHING;
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(17, 21, 29))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.vertical(|ui| {
                    let fullscreen = ui
                        .ctx()
                        .input(|input| input.viewport().fullscreen.unwrap_or(false));
                    let root_top = ui.min_rect().top();
                    let pointer_y = ui
                        .ctx()
                        .input(|input| input.pointer.hover_pos().map(|p| p.y));
                    let reveal_menu = should_reveal_fullscreen_menu(
                        fullscreen,
                        self.active_menu.is_some(),
                        pointer_y,
                        root_top,
                    );
                    if reveal_menu {
                        self.render_header_controls(ui);
                    }
                    viewport_rect = self.render_game_area(ui);
                });
            });
        viewport_rect
    }

    /// Render top-level menu buttons and apply coarse-grained UI commands.
    fn render_header_controls(&mut self, ui: &mut egui::Ui) {
        let mut command = None;
        let mut audio_command = None;
        let mut diagnostics_command = None;
        let mut fullscreen_requested = None;
        let mut exit_requested = false;
        let root_fullscreen = ui
            .ctx()
            .input(|input| input.viewport().fullscreen.unwrap_or(false));
        {
            let mut header_state = HeaderState {
                browse_requested: &mut self.rom_dialog_requested,
                recent_roms: &self.recent_roms,
                recent_rom_requested: &mut self.recent_rom_requested,
                footer_mode: &mut self.footer_mode,
                region: &mut self.region,
                aspect_ratio: &mut self.aspect_ratio,
                audio_state: &mut self.audio_state,
                audio_backend: &mut self.audio_backend,
                audio_volume: &mut self.audio_volume,
                audio_command: &mut audio_command,
                viewport: &self.native_viewport,
                machine_command: &mut command,
                devices: &self.input_devices,
                selected_gamepad: &mut self.selected_gamepad,
                keyboard_bindings: &self.keyboard_bindings,
                pending_key_binding: &mut self.pending_key_binding,
                gamepad_bindings: &self.gamepad_bindings,
                pending_gamepad_binding: &mut self.pending_gamepad_binding,
                input_mapping_error: self.input_mapping_error.as_deref(),
                input_error: self
                    .input_backend
                    .as_ref()
                    .and_then(|state| state.as_ref().err())
                    .map(String::as_str),
                active_menu: &mut self.active_menu,
                menu_anchor: &mut self.menu_anchor,
                menu_popup_size: &mut self.menu_popup_size,
                root_fullscreen,
                fullscreen_requested: &mut fullscreen_requested,
                exit_requested: &mut exit_requested,
                diagnostics_enabled: self.diagnostics_enabled,
                diagnostics_command: &mut diagnostics_command,
                diagnostics_error: self.diagnostics_error.as_deref(),
                debugger_window_open: self.debugger_window_open.as_ref(),
                fps_overlay_enabled: &mut self.fps_overlay_enabled,
                fps_overlay_size: &mut self.fps_overlay_size,
            };
            render_header(ui, &mut header_state);
        }

        if let Some(path) = self.recent_rom_requested.take() {
            self.rom_path = path.to_string_lossy().into_owned();
            self.load_rom_from_path();
        }
        if let Some(command) = command {
            self.apply_command(command);
        }
        if let Some(audio_command) = audio_command {
            self.apply_audio_command(audio_command);
        }
        if let Some(enabled) = diagnostics_command {
            self.set_diagnostics_enabled(enabled);
        }
        if let Some(fullscreen) = fullscreen_requested {
            ui.ctx().send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::Fullscreen(fullscreen),
            );
        }
        if exit_requested {
            ui.ctx()
                .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
        }
    }

    /// Render the selected menu in an opaque native child viewport above D3D12.
    fn render_menu_popup(&mut self, context: &egui::Context, root_bounds: egui::Rect) -> bool {
        let Some(menu) = self.active_menu else {
            return false;
        };

        let popup_size = menu_popup_size(menu, self.menu_popup_size, root_bounds);
        let position = menu_popup_position(root_bounds, self.menu_anchor, popup_size);
        let builder = menu_popup_viewport_builder(menu.title(), position, popup_size);
        let mut actions = MenuPopupActions {
            root_fullscreen: context.input(|input| input.viewport().fullscreen.unwrap_or(false)),
            ..MenuPopupActions::default()
        };
        let mut measured_size = popup_size;
        let mut popup_focused = false;

        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of("sega-menu-popup"),
            builder,
            |menu_ui, _class| {
                popup_focused = menu_ui.input(|input| input.viewport().focused.unwrap_or(false));
                measured_size =
                    self.render_menu_popup_contents(menu_ui, context, menu, &mut actions);
            },
        );

        if self.menu_last_focused != Some(popup_focused) {
            record_ui_diagnostic(
                "menu.viewport.focus_changed",
                if popup_focused {
                    "focused"
                } else {
                    "unfocused"
                },
            );
            self.menu_last_focused = Some(popup_focused);
        }
        self.menu_popup_size = menu_popup_size(menu, measured_size, root_bounds);

        if let Some(path) = self.recent_rom_requested.take() {
            self.rom_path = path.to_string_lossy().into_owned();
            self.load_rom_from_path();
            self.active_menu = None;
        }
        if let Some(command) = actions.command {
            self.apply_command(command);
            self.active_menu = None;
        }
        if let Some(audio_command) = actions.audio_command {
            self.apply_audio_command(audio_command);
        }
        if let Some(enabled) = actions.diagnostics_command {
            self.set_diagnostics_enabled(enabled);
        }
        if let Some(fullscreen) = actions.fullscreen_requested {
            context.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::Fullscreen(fullscreen),
            );
        }
        if actions.exit_requested {
            context.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
        }
        if self.rom_dialog_requested {
            record_ui_diagnostic("menu.popup.closed", menu.title());
            self.active_menu = None;
        }
        actions.input_updated
    }

    /// Render menu controls inside the dedicated native popup viewport.
    fn render_menu_popup_contents(
        &mut self,
        menu_ui: &mut egui::Ui,
        root_context: &egui::Context,
        menu: TopLevelMenu,
        actions: &mut MenuPopupActions,
    ) -> egui::Vec2 {
        let mut measured_size = egui::Vec2::ZERO;
        egui::CentralPanel::default()
            .frame(egui::Frame::popup(menu_ui.style()))
            .show(menu_ui, |menu_ui| {
                let scroll = egui::ScrollArea::both()
                    .max_width(menu.minimum_size().x)
                    .auto_shrink([true, true])
                    .show(menu_ui, |menu_ui| {
                        menu_ui.vertical(|menu_ui| {
                            menu_ui.horizontal(|menu_ui| {
                                menu_ui.strong(menu.title());
                                if menu_ui.small_button("×").clicked() {
                                    record_ui_diagnostic("menu.popup.closed", menu.title());
                                    self.active_menu = None;
                                }
                            });
                            menu_ui.separator();

                            let (should_close, focused) = {
                                let mut header_state = HeaderState {
                                    browse_requested: &mut self.rom_dialog_requested,
                                    recent_roms: &self.recent_roms,
                                    recent_rom_requested: &mut self.recent_rom_requested,
                                    footer_mode: &mut self.footer_mode,
                                    region: &mut self.region,
                                    aspect_ratio: &mut self.aspect_ratio,
                                    audio_state: &mut self.audio_state,
                                    audio_backend: &mut self.audio_backend,
                                    audio_volume: &mut self.audio_volume,
                                    audio_command: &mut actions.audio_command,
                                    viewport: &self.native_viewport,
                                    machine_command: &mut actions.command,
                                    devices: &self.input_devices,
                                    selected_gamepad: &mut self.selected_gamepad,
                                    input_error: self
                                        .input_backend
                                        .as_ref()
                                        .and_then(|state| state.as_ref().err())
                                        .map(String::as_str),
                                    keyboard_bindings: &self.keyboard_bindings,
                                    pending_key_binding: &mut self.pending_key_binding,
                                    gamepad_bindings: &self.gamepad_bindings,
                                    pending_gamepad_binding: &mut self.pending_gamepad_binding,
                                    input_mapping_error: self.input_mapping_error.as_deref(),
                                    active_menu: &mut self.active_menu,
                                    menu_anchor: &mut self.menu_anchor,
                                    menu_popup_size: &mut self.menu_popup_size,
                                    root_fullscreen: actions.root_fullscreen,
                                    fullscreen_requested: &mut actions.fullscreen_requested,
                                    exit_requested: &mut actions.exit_requested,
                                    diagnostics_enabled: self.diagnostics_enabled,
                                    diagnostics_command: &mut actions.diagnostics_command,
                                    diagnostics_error: self.diagnostics_error.as_deref(),
                                    debugger_window_open: self.debugger_window_open.as_ref(),
                                    fps_overlay_enabled: &mut self.fps_overlay_enabled,
                                    fps_overlay_size: &mut self.fps_overlay_size,
                                };
                                render_menu_content(menu_ui, menu, &mut header_state);

                                let close_requested = menu_ui.input(|input| {
                                    input.key_pressed(egui::Key::Escape)
                                        || input.viewport().close_requested()
                                });
                                let action_selected = header_state.machine_command.is_some()
                                    || *header_state.browse_requested
                                    || header_state.recent_rom_requested.is_some()
                                    || *header_state.exit_requested;
                                if close_requested
                                    || action_selected
                                    || header_state.diagnostics_command.is_some()
                                {
                                    record_ui_diagnostic(
                                        "menu.popup.close_requested",
                                        menu.title(),
                                    );
                                }
                                (
                                    close_requested || action_selected,
                                    menu_ui
                                        .input(|input| input.viewport().focused.unwrap_or(false)),
                                )
                            };

                            close_menu_if_current(&mut self.active_menu, menu, should_close);
                            if focused {
                                self.update_keyboard_input(menu_ui.ctx(), root_context);
                                actions.input_updated = true;
                            }
                        })
                    });
                measured_size = scroll.inner.response.rect.size() + egui::vec2(24.0, 24.0);
            });
        measured_size
    }

    /// Render the game viewport and optional status footer in the root window.
    fn render_game_area(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let show_status = self.footer_mode.is_visible();
        let viewport_rect = render_video_viewport(ui, self.aspect_ratio, show_status);
        if show_status {
            render_footer(
                ui,
                &self.native_viewport,
                &self.rom_path,
                self.fps_overlay_enabled.then_some(self.fps_counter.fps),
                self.fps_overlay_size.font_size(),
            );
        }
        viewport_rect
    }

    /// Open the native Windows file picker, parented to the main application window.
    fn open_rom_dialog(&mut self, frame: &eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            self.native_viewport.error = Some(
                "A janela principal ainda não está disponível para abrir o seletor de ROM"
                    .to_owned(),
            );
            return;
        };

        let mut dialog = FileDialog::new()
            .set_title("Selecionar ROM do Sega Master System")
            .add_filter("ROM do Master System", &["sms", "zip", "bin"]);
        if let Some(directory) = self.last_rom_directory.as_ref() {
            dialog = dialog.set_directory(directory);
        }

        if let Some(path) = dialog.set_parent(window).pick_file() {
            self.rom_path = path.to_string_lossy().into_owned();
            if let Some(directory) = path.parent().filter(|path| path.is_dir()) {
                self.last_rom_directory = Some(directory.to_path_buf());
                if let Err(error) = save_last_rom_directory(directory) {
                    self.native_viewport.error = Some(format!(
                        "ROM selecionada, mas não foi possível salvar o último diretório: {error}"
                    ));
                    return;
                }
            }
            self.native_viewport.error = None;
            self.load_rom_from_path();
        }
    }

    fn load_rom_from_path(&mut self) {
        let path = Path::new(self.rom_path.trim());
        if path.as_os_str().is_empty() {
            self.native_viewport.error =
                Some("Selecione ou informe o caminho de uma ROM .sms ou .zip".to_owned());
            return;
        }

        let rom = match read_rom_file(path) {
            Ok(rom) => rom,
            Err(error) => {
                self.native_viewport.error = Some(error);
                return;
            }
        };

        let region = if self.region.is_empty() {
            "usa"
        } else {
            &self.region
        };
        let (restored_breakpoints, trace_capture_enabled) = self
            .debugger_window
            .lock()
            .map(|mut state| {
                if !state
                    .breakpoint_rom_path
                    .eq_ignore_ascii_case(&self.rom_path)
                {
                    state.breakpoints.clear();
                    state.breakpoint_rom_path.clone_from(&self.rom_path);
                }
                (state.breakpoints.clone(), state.trace_capture_enabled)
            })
            .unwrap_or_default();

        match SmsTemporalWorker::new(rom, region) {
            Ok(worker) => {
                self.native_viewport.replace_worker(worker);
                if let Some(worker) = self.native_viewport.worker.as_ref() {
                    for address in &restored_breakpoints {
                        if let Err(error) = worker.set_breakpoint(*address, true) {
                            self.native_viewport.error = Some(format!(
                                "Falha ao restaurar breakpoint 0x{address:04X}: {error}"
                            ));
                            break;
                        }
                    }
                    if trace_capture_enabled && let Err(error) = worker.set_trace_enabled(true) {
                        self.native_viewport.error =
                            Some(format!("Falha ao restaurar trace do debugger: {error}"));
                    }
                }
                if self.diagnostics_enabled
                    && let Some(worker) = self.native_viewport.worker.as_ref()
                {
                    worker.set_diagnostics_enabled(true);
                }
                self.fps_counter.reset(0, Instant::now());
                remember_recent_rom(&mut self.recent_roms, path);
                self.last_controller = None;
                self.pause_key_down = false;
                if self.audio_state.is_enabled() {
                    self.apply_audio_command(AudioCommand::Configure);
                }
            }
            Err(error) => {
                self.native_viewport.error = Some(format!("Falha ao criar runtime SMS: {error}"));
            }
        }
    }

    fn set_diagnostics_enabled(&mut self, enabled: bool) {
        if enabled {
            let logging = if let Some(logging) = current_logging() {
                Ok(logging)
            } else {
                let mut config = LogConfig::default();
                if std::env::var_os("SEGA_LOG_FILTER").is_none() {
                    // Menu events are emitted at DEBUG and must be included explicitly.
                    "info,sega::audio::pacing=trace,sega::ui::menu=debug"
                        .clone_into(&mut config.filter);
                }
                let session = SessionMetadata::current(
                    env!("CARGO_PKG_VERSION"),
                    option_env!("SEGA_GIT_SHA").unwrap_or("unknown"),
                    option_env!("SEGA_BUILD_PROFILE").unwrap_or("unknown"),
                );
                init_logging(config, session).map_err(|error| error.to_string())
            };
            match logging.and_then(|logging| {
                logging.set_enabled(true)?;
                Ok(logging)
            }) {
                Ok(logging) => {
                    sega_observability::tracing::info!(
                        target: "sega::app",
                        event = "diagnostics_enabled",
                        session_id = %logging.session().session_id,
                    );
                    if let Some(worker) = self.native_viewport.worker.as_ref() {
                        worker.set_diagnostics_enabled(true);
                    }
                    self.diagnostics_enabled = true;
                    self.diagnostics_error = None;
                }
                Err(error) => {
                    self.diagnostics_enabled = false;
                    self.diagnostics_error = Some(format!(
                        "Não foi possível ativar a coleta de diagnóstico: {error}"
                    ));
                }
            }
        } else {
            if let Some(logging) = current_logging() {
                let _ = logging.set_enabled(false);
            }
            if let Some(worker) = self.native_viewport.worker.as_ref() {
                worker.set_diagnostics_enabled(false);
            }
            self.diagnostics_enabled = false;
            self.diagnostics_error = None;
        }
    }

    fn apply_audio_command(&mut self, command: AudioCommand) {
        let Some(worker) = self.native_viewport.worker.as_ref() else {
            return;
        };
        let result = match command {
            AudioCommand::Configure => {
                configure_audio_output(worker, self.audio_backend, self.audio_volume)
            }
            AudioCommand::SetVolume(volume) => worker.set_audio_volume(volume),
            AudioCommand::Disable => worker.close_audio_output(),
        };
        if let Err(error) = result {
            self.native_viewport.error = Some(format!("Configuração de áudio falhou: {error}"));
        } else {
            self.native_viewport.error = None;
        }
    }

    fn apply_command(&mut self, command: UiCommand) {
        if matches!(command, UiCommand::CloseMachine) {
            self.native_viewport.close_worker();
            return;
        }

        let Some(worker) = self.native_viewport.worker.as_ref() else {
            return;
        };

        let result = match command {
            UiCommand::Start => {
                if self.native_viewport.video_attached {
                    worker.start()
                } else {
                    self.native_viewport.start_requested = true;
                    Ok(())
                }
            }
            UiCommand::Pause => worker.pause(),
            UiCommand::Stop => {
                self.native_viewport.start_requested = false;
                worker.stop()
            }
            UiCommand::Reset => worker.reset(),
            UiCommand::StepFrame => worker.step_frame(100_000).map(|_| ()),
            UiCommand::CloseMachine => unreachable!("handled before borrowing the worker"),
        };

        if let Err(error) = result {
            self.native_viewport.error = Some(format!("Comando SMS falhou: {error}"));
        } else if !matches!(command, UiCommand::Start) {
            self.native_viewport.error = None;
        }
    }

    fn ensure_input_backend(&mut self) {
        if self.input_backend.is_some() {
            return;
        }
        let backend_result = match Sdl3Backend::new() {
            Ok(mut backend) => match backend.enumerate(&mut self.input_devices) {
                Ok(()) => Ok(backend),
                Err(error) => {
                    backend.shutdown();
                    Err(error)
                }
            },
            Err(error) => Err(error),
        };
        match backend_result {
            Ok(backend) => {
                self.selected_gamepad = self
                    .input_devices
                    .iter()
                    .find(|device| device.class == DeviceClass::Gamepad)
                    .map(|device| device.id);
                self.input_backend = Some(Ok(backend));
            }
            Err(error) => self.input_backend = Some(Err(error.to_string())),
        }
    }

    fn update_gamepad_input(&mut self) -> (ControllerButtons, bool) {
        self.ensure_input_backend();
        let Some(Ok(backend)) = self.input_backend.as_mut() else {
            return (ControllerButtons::default(), false);
        };
        self.input_buffers.clear();
        let poll_result = backend.poll_into(&mut self.input_buffers);
        if let Err(error) = poll_result {
            let error = error.to_string();
            if let Some(Ok(mut backend)) = self.input_backend.take() {
                backend.shutdown();
            }
            self.input_backend = Some(Err(error));
            return (ControllerButtons::default(), false);
        }
        if !self.input_buffers.events.is_empty() {
            let enumerate_result = backend.enumerate(&mut self.input_devices);
            if let Err(error) = enumerate_result {
                let error = error.to_string();
                if let Some(Ok(mut backend)) = self.input_backend.take() {
                    backend.shutdown();
                }
                self.input_backend = Some(Err(error));
                return (ControllerButtons::default(), false);
            }
        }
        if self.selected_gamepad.is_none_or(|selected| {
            !self
                .input_devices
                .iter()
                .any(|device| device.id == selected)
        }) {
            self.selected_gamepad = self
                .input_devices
                .iter()
                .find(|device| device.class == DeviceClass::Gamepad)
                .map(|device| device.id);
        }
        let Some(reading) = self
            .input_buffers
            .readings
            .iter()
            .find(|reading| Some(reading.device_id) == self.selected_gamepad)
            .copied()
        else {
            return (ControllerButtons::default(), false);
        };

        if let Some(action) = self.pending_gamepad_binding {
            let button_count = self
                .input_devices
                .iter()
                .find(|device| Some(device.id) == self.selected_gamepad)
                .map_or(0, |device| device.capabilities.button_count)
                .min(u16::try_from(sega_input_runtime::MAX_BUTTONS).unwrap_or(u16::MAX));
            if let Some(button) =
                (0..button_count).find(|index| reading.state.buttons.is_pressed(*index as usize))
            {
                let button =
                    u8::try_from(button).expect("gamepad button index is bounded by MAX_BUTTONS");
                self.gamepad_bindings.set(action, button);
                self.gamepad_profile = self.gamepad_bindings.to_profile();
                self.pending_gamepad_binding = None;
                self.input_mapping_error = save_gamepad_bindings(self.gamepad_bindings)
                    .err()
                    .map(|error| format!("Falha ao salvar mapeamento do controle: {error}"));
            }
            return (ControllerButtons::default(), false);
        }

        map_sdl3_gamepad(reading.state, &self.gamepad_profile)
    }

    /// Poll gameplay input and dispatch edge-triggered emulator shortcuts.
    ///
    /// F5 starts, F6 pauses, F7 stops, F8 resets, F9 advances one frame and F11
    /// toggles borderless fullscreen. Commands remain coarse-grained UI requests.
    fn update_keyboard_input(
        &mut self,
        input_context: &egui::Context,
        root_context: &egui::Context,
    ) {
        if let Some(action) = self.pending_key_binding {
            let captured = input_context.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        ..
                    } => Some(*key),
                    _ => None,
                })
            });
            if let Some(key) = captured {
                self.keyboard_bindings.set(action, key);
                self.pending_key_binding = None;
                self.input_mapping_error = save_keyboard_bindings(&self.keyboard_bindings)
                    .err()
                    .map(|error| format!("Falha ao salvar mapeamento do teclado: {error}"));
            }
            return;
        }

        let bindings = self.keyboard_bindings;
        let (keyboard_buttons, keyboard_pause, shortcut, toggle_fullscreen) =
            input_context.input(|input| {
                let shortcut = if input.key_pressed(bindings.start) {
                    Some(UiCommand::Start)
                } else if input.key_pressed(bindings.pause) {
                    Some(UiCommand::Pause)
                } else if input.key_pressed(bindings.stop) {
                    Some(UiCommand::Stop)
                } else if input.key_pressed(bindings.reset) {
                    Some(UiCommand::Reset)
                } else if input.key_pressed(bindings.step_frame) {
                    Some(UiCommand::StepFrame)
                } else {
                    None
                };
                (
                    ControllerButtons {
                        up: input.key_down(bindings.up),
                        down: input.key_down(bindings.down),
                        left: input.key_down(bindings.left),
                        right: input.key_down(bindings.right),
                        button_1: input.key_down(bindings.button_1),
                        button_2: input.key_down(bindings.button_2),
                    },
                    input.key_down(bindings.nmi),
                    shortcut,
                    input.key_pressed(bindings.toggle_fullscreen),
                )
            });
        let (gamepad_buttons, gamepad_pause) = self.update_gamepad_input();
        let buttons = ControllerButtons {
            up: keyboard_buttons.up || gamepad_buttons.up,
            down: keyboard_buttons.down || gamepad_buttons.down,
            left: keyboard_buttons.left || gamepad_buttons.left,
            right: keyboard_buttons.right || gamepad_buttons.right,
            button_1: keyboard_buttons.button_1 || gamepad_buttons.button_1,
            button_2: keyboard_buttons.button_2 || gamepad_buttons.button_2,
        };
        let pause_down = keyboard_pause || gamepad_pause;
        if let Some(worker) = self.native_viewport.worker.as_ref() {
            if self.last_controller != Some(buttons)
                && worker.set_controller_at(1, buttons, Instant::now()).is_ok()
            {
                self.last_controller = Some(buttons);
            }
            if pause_down && !self.pause_key_down {
                let _ = worker.request_nmi();
            }
        }
        self.pause_key_down = pause_down;

        if let Some(command) = shortcut {
            self.apply_command(command);
        }
        if toggle_fullscreen {
            let fullscreen =
                root_context.input(|input| !input.viewport().fullscreen.unwrap_or(false));
            root_context.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::Fullscreen(fullscreen),
            );
        }
    }

    /// Update the presented-frame counter used by the bottom status footer.
    fn update_fps_counter(&mut self) {
        if !self.fps_overlay_enabled || !self.native_viewport.video_attached {
            self.fps_overlay_was_active = false;
            return;
        }
        let now = Instant::now();
        let presented_sequence = self.native_viewport.session.as_ref()
            .map_or(0, D3d12MailboxSession::last_presented_sequence);
        if !self.fps_overlay_was_active {
            self.fps_counter.reset(presented_sequence, now);
            self.fps_overlay_was_active = true;
        } else {
            self.fps_counter.update(presented_sequence, now);
        }
    }

    fn update_native_viewport(
        &mut self,
        frame: &eframe::Frame,
        context: &egui::Context,
        viewport_rect: egui::Rect,
    ) {
        let Some(parent_hwnd) =
            resolve_parent_hwnd(frame, context, &mut self.native_viewport.error)
        else {
            return;
        };

        if !self.ensure_native_child(parent_hwnd) {
            return;
        }

        self.update_child_bounds(context, viewport_rect);
        self.poll_native_startup(context);
        self.connect_worker_video(context);
        self.monitor_native_state(context);
    }

    fn ensure_native_child(&mut self, parent_hwnd: usize) -> bool {
        if self.native_viewport.parent_hwnd == Some(parent_hwnd) {
            return true;
        }

        self.native_viewport.shutdown();
        self.native_viewport.error = None;

        let child = match D3d12ChildWindow::create(parent_hwnd) {
            Ok(child) => {
                record_ui_diagnostic("native.d3d12_child.created", "");
                child
            }
            Err(error) => {
                self.native_viewport.error =
                    Some(format!("Falha ao criar HWND filho D3D12: {error}"));
                return false;
            }
        };

        let session =
            match D3d12MailboxSession::start_for_existing_window_async(256, 192, child.handle()) {
                Ok(session) => session,
                Err(error) => {
                    self.native_viewport.error =
                        Some(format!("Falha ao iniciar worker D3D12: {error}"));
                    drop(child);
                    return false;
                }
            };

        self.native_viewport.parent_hwnd = Some(parent_hwnd);
        self.native_viewport.child = Some(child);
        self.native_viewport.session = Some(session);
        true
    }

    fn update_child_bounds(&mut self, context: &egui::Context, viewport_rect: egui::Rect) {
        let scale = context.pixels_per_point();
        let bounds = (
            to_win32_pixel(viewport_rect.min.x * scale),
            to_win32_pixel(viewport_rect.min.y * scale),
            to_win32_pixel(viewport_rect.width() * scale),
            to_win32_pixel(viewport_rect.height() * scale),
        );

        if bounds.2 > 0
            && bounds.3 > 0
            && self.native_viewport.last_bounds != Some(bounds)
            && let Some(child) = self.native_viewport.child.as_ref()
        {
            match child.set_bounds(bounds.0, bounds.1, bounds.2, bounds.3) {
                Ok(()) => {
                    self.native_viewport.last_bounds = Some(bounds);
                    record_ui_diagnostic("native.d3d12_child.bounds", "");
                }
                Err(error) => {
                    record_ui_diagnostic("native.d3d12_child.bounds_error", "");
                    self.native_viewport.error =
                        Some(format!("Falha ao redimensionar viewport D3D12: {error}"));
                    context.request_repaint();
                }
            }
        }
    }

    fn poll_native_startup(&mut self, context: &egui::Context) {
        let startup = self
            .native_viewport
            .session
            .as_mut()
            .map(D3d12MailboxSession::poll_startup);
        match startup {
            Some(Ok(true)) => {
                self.native_viewport.ready = true;
                context.request_repaint();
            }
            Some(Ok(false)) => context.request_repaint_after(Duration::from_millis(16)),
            Some(Err(error)) => {
                self.native_viewport.shutdown();
                self.native_viewport.error = Some(format!("Inicialização D3D12 falhou: {error}"));
                context.request_repaint();
            }
            None => {}
        }
    }

    fn connect_worker_video(&mut self, context: &egui::Context) {
        if !self.native_viewport.ready || self.native_viewport.video_attached {
            return;
        }
        let mailbox = self
            .native_viewport
            .session
            .as_ref()
            .map(D3d12MailboxSession::mailbox);
        let Some(mailbox) = mailbox else {
            return;
        };
        let Some(worker) = self.native_viewport.worker.as_ref() else {
            return;
        };

        match worker.attach_video_mailbox(mailbox) {
            Ok(()) => {
                self.native_viewport.video_attached = true;
                if self.native_viewport.start_requested {
                    match worker.start() {
                        Ok(()) => self.native_viewport.start_requested = false,
                        Err(error) => {
                            self.native_viewport.start_requested = false;
                            self.native_viewport.error =
                                Some(format!("Falha ao iniciar SMS: {error}"));
                        }
                    }
                }
                context.request_repaint();
            }
            Err(error) => {
                self.native_viewport.error = Some(format!("Falha ao conectar vídeo SMS: {error}"));
            }
        }
    }

    fn monitor_native_state(&mut self, context: &egui::Context) {
        if self.native_viewport.ready
            && self
                .native_viewport
                .session
                .as_ref()
                .is_some_and(|session| !session.is_running())
        {
            self.native_viewport.shutdown();
            self.native_viewport.error =
                Some("O worker de apresentação D3D12 foi encerrado".to_owned());
            context.request_repaint();
        } else {
            let gamepad_connected = self
                .input_devices
                .iter()
                .any(|device| device.class == DeviceClass::Gamepad);
            let input_poll_interval = if gamepad_connected {
                Duration::from_millis(16)
            } else {
                Duration::from_millis(100)
            };
            context.request_repaint_after(input_poll_interval);
        }
    }
}

impl Drop for SegaApp {
    fn drop(&mut self) {
        // Stop producers before shutting down the observability worker. The
        // viewport owns the SMS and D3D12 workers; they may emit final events
        // while being stopped and joined.
        if let Err(error) = self.save_ui_settings() {
            eprintln!("Unable to save UI settings: {error}");
        }
        self.native_viewport.shutdown();

        // SDL3 is also an owned native resource. Shut it down explicitly while
        // diagnostics are still available instead of relying on field-drop
        // order, which runs only after this Drop implementation returns.
        if let Some(Ok(backend)) = self.input_backend.as_mut() {
            backend.shutdown();
        }
        self.input_backend = None;

        if let Some(logging) = current_logging() {
            let diagnostics_were_enabled = logging.is_enabled();

            if diagnostics_were_enabled {
                sega_observability::tracing::info!(
                    target: "sega::app",
                    event = "application_shutdown",
                    session_id = %logging.session().session_id,
                );

                // Drain queued records before serializing the final snapshot.
                logging.shutdown();
                if let Err(error) = logging.write_diagnostics_json() {
                    eprintln!("Unable to write diagnostics snapshot: {error}");
                }
            } else {
                logging.shutdown();
            }
        }
    }
}

impl eframe::App for SegaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.maintain_window_settings(&context);
        if !self.theme_initialized {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(17, 21, 29);
            visuals.window_fill = egui::Color32::from_rgb(25, 30, 40);
            context.set_visuals(visuals);
            self.theme_initialized = true;
        }
        let viewport_rect = self.render(ui);

        if self.active_menu.is_some() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            if let Some(menu) = self.active_menu {
                record_ui_diagnostic("menu.popup.closed", menu.title());
            }
            self.active_menu = None;
            self.menu_last_focused = None;
        }
        let input_updated =
            if let Some(root_bounds) = context.input(|input| input.viewport().inner_rect) {
                self.render_menu_popup(&context, root_bounds)
            } else {
                context.request_repaint_after(Duration::from_millis(16));
                false
            };

        if !input_updated {
            self.update_keyboard_input(ui.ctx(), ui.ctx());
        }
        if self.rom_dialog_requested {
            self.rom_dialog_requested = false;
            self.open_rom_dialog(frame);
        }
        self.update_native_viewport(frame, ui.ctx(), viewport_rect);
        self.render_debugger_window(&context);
        self.update_fps_counter();
        if self.fps_overlay_enabled && self.native_viewport.video_attached {
            context.request_repaint_after(FPS_SAMPLE_INTERVAL);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

/// Read a raw SMS image or extract the single SMS image contained in a ZIP archive.
fn read_rom_file(path: &Path) -> Result<Vec<u8>, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    if extension.eq_ignore_ascii_case("sms") || extension.eq_ignore_ascii_case("bin") {
        let rom = fs::read(path).map_err(|error| format!("Não foi possível ler a ROM: {error}"))?;
        if rom.is_empty() {
            return Err("O arquivo da ROM está vazio".to_owned());
        }
        if rom.len() > MAX_ROM_SIZE_BYTES {
            return Err("A ROM excede o limite de 16 MiB aceito pelo carregador".to_owned());
        }
        return Ok(rom);
    }

    if extension.eq_ignore_ascii_case("zip") {
        let file = fs::File::open(path)
            .map_err(|error| format!("Não foi possível abrir o arquivo ZIP: {error}"))?;
        return read_sms_from_zip(file);
    }

    Err(
        "Formato não suportado. Selecione um arquivo .sms ou .zip (ou informe um .bin manualmente)"
            .to_owned(),
    )
}

/// Extract the only non-directory .sms entry from a ZIP archive.
///
/// Requiring exactly one candidate avoids silently booting the wrong game when
/// an archive contains multiple ROM images. The uncompressed-size cap limits
/// memory consumption when opening malformed or oversized archives.
fn read_sms_from_zip<R: Read + Seek>(reader: R) -> Result<Vec<u8>, String> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|error| format!("Arquivo ZIP inválido ou não suportado: {error}"))?;
    let mut sms_entries = Vec::new();

    for index in 0..archive.len() {
        let is_sms = {
            let entry = archive
                .by_index(index)
                .map_err(|error| format!("Não foi possível inspecionar o ZIP: {error}"))?;
            let entry_name = entry.name().map_err(|error| {
                format!("Não foi possível ler o nome de uma entrada ZIP: {error}")
            })?;

            !entry.is_dir()
                && Path::new(entry_name.as_ref())
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("sms"))
        };
        if is_sms {
            sms_entries.push(index);
        }
    }

    let index = match sms_entries.as_slice() {
        [] => return Err("O ZIP não contém nenhum arquivo .sms".to_owned()),
        [index] => *index,
        _ => {
            return Err(
                "O ZIP contém mais de um arquivo .sms; mantenha apenas a ROM desejada".to_owned(),
            );
        }
    };

    let entry = archive
        .by_index(index)
        .map_err(|error| format!("Não foi possível abrir a ROM dentro do ZIP: {error}"))?;
    if entry.size() == 0 {
        return Err("A ROM dentro do ZIP está vazia".to_owned());
    }
    if entry.size() > MAX_ROM_SIZE_BYTES_U64 {
        return Err("A ROM dentro do ZIP excede o limite de 16 MiB".to_owned());
    }

    let capacity = usize::try_from(entry.size())
        .map_err(|_| "O tamanho da ROM não cabe na memória deste processo".to_owned())?;
    let mut rom = Vec::with_capacity(capacity);
    entry
        .take(MAX_ROM_SIZE_BYTES_U64 + 1)
        .read_to_end(&mut rom)
        .map_err(|error| format!("Falha ao descompactar a ROM: {error}"))?;
    if rom.is_empty() {
        return Err("A ROM dentro do ZIP está vazia".to_owned());
    }
    if rom.len() > MAX_ROM_SIZE_BYTES {
        return Err("A ROM descompactada excede o limite de 16 MiB".to_owned());
    }

    Ok(rom)
}

/// Return the per-user settings directory used to persist the last ROM folder.
fn rom_settings_directory() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|directory| directory.join(APP_DATA_DIRECTORY))
}

/// Restore the last directory used by the ROM picker, if it still exists.
fn load_last_rom_directory() -> Option<PathBuf> {
    let settings_file = rom_settings_directory()?.join(LAST_ROM_DIRECTORY_FILE);
    let directory = PathBuf::from(fs::read_to_string(settings_file).ok()?.trim());
    directory.is_dir().then_some(directory)
}

/// Restore recent ROM paths, retaining existing files and the newest ten entries.
fn load_recent_roms() -> Vec<PathBuf> {
    let Some(directory) = rom_settings_directory() else {
        return Vec::new();
    };
    let content = fs::read_to_string(directory.join(RECENT_ROM_LIST_FILE)).unwrap_or_default();
    content
        .lines()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .take(MAX_RECENT_ROMS)
        .collect()
}

/// Persist recent ROM paths without copying or modifying ROM data.
fn save_recent_roms(recent_roms: &[PathBuf]) -> std::io::Result<()> {
    let directory = rom_settings_directory()
        .ok_or_else(|| std::io::Error::other("APPDATA não está definido"))?;
    fs::create_dir_all(&directory)?;
    let contents = recent_roms
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(directory.join(RECENT_ROM_LIST_FILE), contents)
}

/// Move a successfully loaded ROM to the front of the bounded recent-file list.
fn update_recent_rom_list(recent_roms: &mut Vec<PathBuf>, path: &Path) {
    let normalized = path.to_string_lossy();
    recent_roms.retain(|recent| !recent.to_string_lossy().eq_ignore_ascii_case(&normalized));
    recent_roms.insert(0, path.to_path_buf());
    recent_roms.truncate(MAX_RECENT_ROMS);
}

/// Persist a successfully loaded ROM in the bounded recent-file list.
fn remember_recent_rom(recent_roms: &mut Vec<PathBuf>, path: &Path) {
    update_recent_rom_list(recent_roms, path);
    if let Err(error) = save_recent_roms(recent_roms) {
        eprintln!("Não foi possível salvar a lista de ROMs recentes: {error}");
    }
}

/// Persist only the last selected directory; the ROM itself is not copied.
fn save_last_rom_directory(directory: &Path) -> std::io::Result<()> {
    let settings_directory = rom_settings_directory()
        .ok_or_else(|| std::io::Error::other("APPDATA não está definido"))?;
    fs::create_dir_all(&settings_directory)?;
    fs::write(
        settings_directory.join(LAST_ROM_DIRECTORY_FILE),
        directory.to_string_lossy().as_bytes(),
    )
}

const KEYBOARD_BINDINGS_FILE: &str = "keyboard_bindings.txt";
const GAMEPAD_BINDINGS_FILE: &str = "gamepad_bindings.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Fullscreen,
}

#[derive(Debug, Clone, Copy)]
struct KeyboardBindings {
    up: egui::Key,
    down: egui::Key,
    left: egui::Key,
    right: egui::Key,
    button_1: egui::Key,
    button_2: egui::Key,
    nmi: egui::Key,
    start: egui::Key,
    pause: egui::Key,
    stop: egui::Key,
    reset: egui::Key,
    step_frame: egui::Key,
    toggle_fullscreen: egui::Key,
}

impl Default for KeyboardBindings {
    fn default() -> Self {
        Self {
            up: egui::Key::ArrowUp,
            down: egui::Key::ArrowDown,
            left: egui::Key::ArrowLeft,
            right: egui::Key::ArrowRight,
            button_1: egui::Key::Z,
            button_2: egui::Key::X,
            nmi: egui::Key::P,
            start: egui::Key::F5,
            pause: egui::Key::F6,
            stop: egui::Key::F7,
            reset: egui::Key::F8,
            step_frame: egui::Key::F9,
            toggle_fullscreen: egui::Key::F11,
        }
    }
}

impl KeyboardBindings {
    fn key(self, action: KeyboardAction) -> egui::Key {
        match action {
            KeyboardAction::Up => self.up,
            KeyboardAction::Down => self.down,
            KeyboardAction::Left => self.left,
            KeyboardAction::Right => self.right,
            KeyboardAction::Button1 => self.button_1,
            KeyboardAction::Button2 => self.button_2,
            KeyboardAction::Nmi => self.nmi,
            KeyboardAction::Start => self.start,
            KeyboardAction::Pause => self.pause,
            KeyboardAction::Stop => self.stop,
            KeyboardAction::Reset => self.reset,
            KeyboardAction::StepFrame => self.step_frame,
            KeyboardAction::Fullscreen => self.toggle_fullscreen,
        }
    }

    fn set(&mut self, action: KeyboardAction, key: egui::Key) {
        match action {
            KeyboardAction::Up => self.up = key,
            KeyboardAction::Down => self.down = key,
            KeyboardAction::Left => self.left = key,
            KeyboardAction::Right => self.right = key,
            KeyboardAction::Button1 => self.button_1 = key,
            KeyboardAction::Button2 => self.button_2 = key,
            KeyboardAction::Nmi => self.nmi = key,
            KeyboardAction::Start => self.start = key,
            KeyboardAction::Pause => self.pause = key,
            KeyboardAction::Stop => self.stop = key,
            KeyboardAction::Reset => self.reset = key,
            KeyboardAction::StepFrame => self.step_frame = key,
            KeyboardAction::Fullscreen => self.toggle_fullscreen = key,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GamepadAction {
    Up,
    Down,
    Left,
    Right,
    Button1,
    Button2,
    Pause,
}

#[derive(Debug, Clone, Copy)]
struct GamepadBindings {
    up: u8,
    down: u8,
    left: u8,
    right: u8,
    button_1: u8,
    button_2: u8,
    pause: u8,
}

impl Default for GamepadBindings {
    fn default() -> Self {
        Self {
            up: 11,
            down: 12,
            left: 13,
            right: 14,
            button_1: 0,
            button_2: 1,
            pause: 6,
        }
    }
}

impl GamepadBindings {
    fn set(&mut self, action: GamepadAction, button: u8) {
        match action {
            GamepadAction::Up => self.up = button,
            GamepadAction::Down => self.down = button,
            GamepadAction::Left => self.left = button,
            GamepadAction::Right => self.right = button,
            GamepadAction::Button1 => self.button_1 = button,
            GamepadAction::Button2 => self.button_2 = button,
            GamepadAction::Pause => self.pause = button,
        }
    }

    fn to_profile(self) -> InputProfile {
        let action = |index| ActionId::new(index).expect("built-in gamepad action is in range");
        let button = |action_id, index| ActionBinding {
            action: action(action_id),
            control: PhysicalControl::Button(index),
        };
        let axis = |action_id, index, direction| ActionBinding {
            action: action(action_id),
            control: PhysicalControl::Axis {
                index,
                direction,
                threshold: 16_000,
            },
        };
        let hat = |action_id, direction_mask| ActionBinding {
            action: action(action_id),
            control: PhysicalControl::Hat {
                index: 0,
                direction_mask,
            },
        };

        InputProfile::new(
            vec![
                button(0, self.up),
                axis(0, 1, AxisDirection::Negative),
                hat(0, 0x01),
                button(1, self.down),
                axis(1, 1, AxisDirection::Positive),
                hat(1, 0x04),
                button(2, self.left),
                axis(2, 0, AxisDirection::Negative),
                hat(2, 0x08),
                button(3, self.right),
                axis(3, 0, AxisDirection::Positive),
                hat(3, 0x02),
                button(4, self.button_1),
                button(5, self.button_2),
                button(6, self.pause),
            ],
            0,
        )
        .expect("built-in gamepad profile is valid")
    }
}

fn key_from_name(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name)
}

fn load_keyboard_bindings() -> KeyboardBindings {
    let mut bindings = KeyboardBindings::default();
    let Some(directory) = rom_settings_directory() else {
        return bindings;
    };
    let Ok(contents) = fs::read_to_string(directory.join(KEYBOARD_BINDINGS_FILE)) else {
        return bindings;
    };

    for line in contents.lines() {
        let Some((action, key)) = line.split_once('=') else {
            continue;
        };
        let Some(key) = key_from_name(key.trim()) else {
            continue;
        };
        let action = match action.trim() {
            "up" => KeyboardAction::Up,
            "down" => KeyboardAction::Down,
            "left" => KeyboardAction::Left,
            "right" => KeyboardAction::Right,
            "button1" => KeyboardAction::Button1,
            "button2" => KeyboardAction::Button2,
            "nmi" => KeyboardAction::Nmi,
            "start" => KeyboardAction::Start,
            "pause" => KeyboardAction::Pause,
            "stop" => KeyboardAction::Stop,
            "reset" => KeyboardAction::Reset,
            "step_frame" => KeyboardAction::StepFrame,
            "fullscreen" => KeyboardAction::Fullscreen,
            _ => continue,
        };
        bindings.set(action, key);
    }
    bindings
}

fn save_keyboard_bindings(bindings: &KeyboardBindings) -> std::io::Result<()> {
    let directory = rom_settings_directory()
        .ok_or_else(|| std::io::Error::other("APPDATA não está definido"))?;
    fs::create_dir_all(&directory)?;
    let rows = [
        ("up", bindings.up),
        ("down", bindings.down),
        ("left", bindings.left),
        ("right", bindings.right),
        ("button1", bindings.button_1),
        ("button2", bindings.button_2),
        ("nmi", bindings.nmi),
        ("start", bindings.start),
        ("pause", bindings.pause),
        ("stop", bindings.stop),
        ("reset", bindings.reset),
        ("step_frame", bindings.step_frame),
        ("fullscreen", bindings.toggle_fullscreen),
    ];
    let contents = rows
        .iter()
        .map(|(name, key)| format!("{name}={}", key.name()))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(directory.join(KEYBOARD_BINDINGS_FILE), contents)
}

fn load_gamepad_bindings() -> GamepadBindings {
    let mut bindings = GamepadBindings::default();
    let Some(directory) = rom_settings_directory() else {
        return bindings;
    };
    let Ok(contents) = fs::read_to_string(directory.join(GAMEPAD_BINDINGS_FILE)) else {
        return bindings;
    };

    for line in contents.lines() {
        let Some((action, value)) = line.split_once('=') else {
            continue;
        };
        let Ok(button) = value.trim().parse::<u8>() else {
            continue;
        };
        if button as usize >= sega_input_runtime::MAX_BUTTONS {
            continue;
        }
        let action = match action.trim() {
            "up" => GamepadAction::Up,
            "down" => GamepadAction::Down,
            "left" => GamepadAction::Left,
            "right" => GamepadAction::Right,
            "button1" => GamepadAction::Button1,
            "button2" => GamepadAction::Button2,
            "pause" => GamepadAction::Pause,
            _ => continue,
        };
        bindings.set(action, button);
    }
    bindings
}

fn save_gamepad_bindings(bindings: GamepadBindings) -> std::io::Result<()> {
    let directory = rom_settings_directory()
        .ok_or_else(|| std::io::Error::other("APPDATA não está definido"))?;
    fs::create_dir_all(&directory)?;
    let rows = [
        ("up", bindings.up),
        ("down", bindings.down),
        ("left", bindings.left),
        ("right", bindings.right),
        ("button1", bindings.button_1),
        ("button2", bindings.button_2),
        ("pause", bindings.pause),
    ];
    let contents = rows
        .iter()
        .map(|(name, button)| format!("{name}={button}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(directory.join(GAMEPAD_BINDINGS_FILE), contents)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FooterMode {
    Visible,
    Hidden,
}

impl FooterMode {
    const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// Borrowed presentation state shared by the compact top-level menu renderers.
struct HeaderState<'a> {
    browse_requested: &'a mut bool,
    recent_roms: &'a [PathBuf],
    recent_rom_requested: &'a mut Option<PathBuf>,
    footer_mode: &'a mut FooterMode,
    region: &'a mut String,
    aspect_ratio: &'a mut AspectRatioMode,
    audio_state: &'a mut AudioState,
    audio_backend: &'a mut AudioBackend,
    audio_volume: &'a mut u8,
    audio_command: &'a mut Option<AudioCommand>,
    viewport: &'a NativeViewportState,
    machine_command: &'a mut Option<UiCommand>,
    devices: &'a [DeviceDescriptor],
    selected_gamepad: &'a mut Option<DeviceId>,
    input_error: Option<&'a str>,
    keyboard_bindings: &'a KeyboardBindings,
    pending_key_binding: &'a mut Option<KeyboardAction>,
    gamepad_bindings: &'a GamepadBindings,
    pending_gamepad_binding: &'a mut Option<GamepadAction>,
    input_mapping_error: Option<&'a str>,
    active_menu: &'a mut Option<TopLevelMenu>,
    menu_anchor: &'a mut egui::Pos2,
    menu_popup_size: &'a mut egui::Vec2,
    root_fullscreen: bool,
    fullscreen_requested: &'a mut Option<bool>,
    exit_requested: &'a mut bool,
    diagnostics_enabled: bool,
    diagnostics_command: &'a mut Option<bool>,
    diagnostics_error: Option<&'a str>,
    debugger_window_open: &'a AtomicBool,
    fps_overlay_enabled: &'a mut bool,
    fps_overlay_size: &'a mut FpsOverlaySize,
}

/// Render the menu bar. Dropdown content is hosted by a native child viewport.
fn render_header(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.strong("SEGA EMULATOR");
        top_level_menu_button(ui, "Sistema", TopLevelMenu::System, state);
        top_level_menu_button(ui, "Arquivo", TopLevelMenu::File, state);
        top_level_menu_button(ui, "Vídeo", TopLevelMenu::Video, state);
        top_level_menu_button(ui, "Áudio", TopLevelMenu::Audio, state);
        top_level_menu_button(ui, "Controles", TopLevelMenu::Controls, state);
        top_level_menu_button(ui, "Opções", TopLevelMenu::Options, state);
        top_level_menu_button(ui, "DEBUG", TopLevelMenu::Debug, state);
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        render_machine_controls(ui, state.viewport, state.machine_command);
    });
    ui.separator();
}

fn top_level_menu_button(
    ui: &mut egui::Ui,
    label: &str,
    menu: TopLevelMenu,
    state: &mut HeaderState<'_>,
) {
    let active = *state.active_menu == Some(menu);
    let fill = if active {
        egui::Color32::from_rgb(47, 61, 82)
    } else {
        egui::Color32::TRANSPARENT
    };
    let response = ui.add(egui::Button::new(label).fill(fill));
    // Use egui's completed click gesture. Toggling on both press and release
    // can open and immediately close the native popup.
    let switch_by_hover =
        state.active_menu.is_some() && *state.active_menu != Some(menu) && response.hovered();
    if response.clicked() || switch_by_hover {
        let previous = *state.active_menu;
        record_ui_diagnostic("menu.button.activated", menu.title());
        if response.clicked() && previous == Some(menu) {
            *state.active_menu = None;
            record_ui_diagnostic("menu.state.changed", format!("{} -> closed", menu.title()));
        } else {
            *state.active_menu = Some(menu);
            let previous_title = previous.map_or("closed", TopLevelMenu::title);
            record_ui_diagnostic(
                "menu.state.changed",
                format!("{previous_title} -> {}", menu.title()),
            );
            *state.menu_anchor = response.rect.left_bottom();
            *state.menu_popup_size = menu.minimum_size();
        }
    }
}

fn render_menu_content(ui: &mut egui::Ui, menu: TopLevelMenu, state: &mut HeaderState<'_>) {
    match menu {
        TopLevelMenu::System => render_system_menu(ui, state),
        TopLevelMenu::File => render_file_menu(ui, state),
        TopLevelMenu::Video => render_video_menu(ui, state),
        TopLevelMenu::Audio => render_audio_menu(ui, state),
        TopLevelMenu::Controls => render_controls_menu(ui, state),
        TopLevelMenu::Options => render_options_menu(ui, state),
        TopLevelMenu::Debug => render_runtime_status(
            ui,
            state.viewport,
            state.diagnostics_enabled,
            state.diagnostics_command,
            state.diagnostics_error,
            state.debugger_window_open,
        ),
    }
}

fn menu_popup_size(
    menu: TopLevelMenu,
    measured_size: egui::Vec2,
    root_bounds: egui::Rect,
) -> egui::Vec2 {
    let preferred_size = menu.minimum_size();
    let maximum_height = (root_bounds.height() - MENU_BAR_HEIGHT).max(1.0);

    // Keep width tied to the menu's preferred size. The ScrollArea response can
    // otherwise report the full available width when auto_shrink is disabled,
    // feeding the root viewport width back into the next popup window size.
    egui::vec2(
        preferred_size.x.min(root_bounds.width()).max(1.0),
        measured_size.y.max(preferred_size.y).min(maximum_height),
    )
}

fn menu_popup_position(
    root_bounds: egui::Rect,
    anchor: egui::Pos2,
    size: egui::Vec2,
) -> egui::Pos2 {
    let mut position = root_bounds.min + anchor.to_vec2();
    position.x = position
        .x
        .min((root_bounds.right() - size.x).max(root_bounds.left()));
    position.y = position
        .y
        .min((root_bounds.bottom() - size.y).max(root_bounds.top() + MENU_BAR_HEIGHT))
        .max(root_bounds.top() + MENU_BAR_HEIGHT);
    position
}

fn menu_popup_viewport_builder(
    title: &str,
    position: egui::Pos2,
    size: egui::Vec2,
) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(title)
        .with_position(position)
        .with_inner_size(size)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
}

/// Render ROM loading/history and application lifecycle actions.
fn render_file_menu(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    if ui.button("Abrir ROM…").clicked() {
        *state.browse_requested = true;
        *state.active_menu = None;
        ui.close();
    }
    ui.menu_button("Recentes", |ui| {
        if state.recent_roms.is_empty() {
            ui.label("Nenhuma ROM recente");
        } else {
            for path in state.recent_roms {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("ROM sem nome");
                if ui
                    .button(name)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    *state.recent_rom_requested = Some(path.clone());
                    *state.active_menu = None;
                    ui.close();
                }
            }
        }
    });
    ui.separator();
    add_machine_button(
        ui,
        "Fechar ROM",
        "Fechar a ROM atualmente carregada",
        state.viewport.worker.is_some(),
        UiCommand::CloseMachine,
        state.machine_command,
    );
    ui.separator();
    if ui.button("Sair").clicked() {
        *state.exit_requested = true;
        *state.active_menu = None;
    }
}

/// Render display aspect-ratio selection.
fn render_system_menu(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    ui.horizontal(|ui| {
        ui.strong("Sega Master System");
        ui.colored_label(egui::Color32::LIGHT_GREEN, "ATIVO");
    });
    ui.separator();
    ui.label("Região / padrão de vídeo");
    ui.radio_value(state.region, "usa".to_owned(), "USA / NTSC");
    ui.radio_value(state.region, "brazil".to_owned(), "Brasil / PAL-M");
    ui.radio_value(state.region, "europe".to_owned(), "Europa / PAL");
    ui.radio_value(state.region, "japan".to_owned(), "Japão / NTSC");
    ui.add_enabled(
        false,
        egui::RadioButton::new(false, "Automática (não integrada ao runtime Rust)"),
    );
    ui.small("A região selecionada será aplicada ao carregar a próxima ROM.");
    ui.small(
        "A seleção automática ainda não está integrada nesta interface; não é inferida pelo nome do arquivo.",
    );
    ui.separator();
    ui.label("Outros sistemas");
    for system in [
        "SG-1000",
        "Game Gear",
        "Mega Drive / Genesis",
        "Sega CD",
        "32X",
    ] {
        ui.add_enabled(false, egui::Button::new(system));
    }
    ui.small("Indisponíveis: o runtime ainda não está conectado a esta interface.");
}

fn render_video_menu(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    ui.label("Proporção da imagem");
    ui.radio_value(
        state.aspect_ratio,
        AspectRatioMode::FourThree,
        "Original (4:3)",
    );
    ui.radio_value(state.aspect_ratio, AspectRatioMode::OneToOne, "1:1");
    ui.radio_value(state.aspect_ratio, AspectRatioMode::SixteenNine, "16:9");
}

fn render_controls_menu(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    ui.heading("Dispositivo de entrada");
    render_input_controls(ui, state.devices, state.selected_gamepad, state.input_error);
    if let Some(error) = state.input_mapping_error {
        ui.colored_label(egui::Color32::YELLOW, error);
    }

    ui.separator();
    ui.heading("Teclado");
    egui::Grid::new("sms_keyboard_bindings")
        .num_columns(3)
        .show(ui, |ui| {
            for (action, label) in [
                (KeyboardAction::Up, "Cima"),
                (KeyboardAction::Down, "Baixo"),
                (KeyboardAction::Left, "Esquerda"),
                (KeyboardAction::Right, "Direita"),
                (KeyboardAction::Button1, "Botão 1"),
                (KeyboardAction::Button2, "Botão 2"),
                (KeyboardAction::Nmi, "Solicitar NMI"),
                (KeyboardAction::Start, "Iniciar"),
                (KeyboardAction::Pause, "Pausar"),
                (KeyboardAction::Stop, "Parar"),
                (KeyboardAction::Reset, "Reset"),
                (KeyboardAction::StepFrame, "Avançar quadro"),
                (KeyboardAction::Fullscreen, "Tela cheia"),
            ] {
                ui.label(label);
                ui.monospace(state.keyboard_bindings.key(action).name());
                if *state.pending_key_binding == Some(action) {
                    ui.weak("Pressione uma tecla...");
                } else if ui.small_button("Remapear").clicked() {
                    *state.pending_key_binding = Some(action);
                    *state.pending_gamepad_binding = None;
                }
                ui.end_row();
            }
        });
    if state.pending_key_binding.is_some() && ui.small_button("Cancelar captura de tecla").clicked()
    {
        *state.pending_key_binding = None;
    }

    ui.separator();
    ui.heading("Controle");
    ui.small("Selecione Remapear e pressione o botão físico desejado. Direcionais analógicos e hats continuam ativos.");
    egui::Grid::new("sms_gamepad_bindings")
        .num_columns(3)
        .show(ui, |ui| {
            for (action, label, button) in [
                (GamepadAction::Up, "Cima", state.gamepad_bindings.up),
                (GamepadAction::Down, "Baixo", state.gamepad_bindings.down),
                (GamepadAction::Left, "Esquerda", state.gamepad_bindings.left),
                (
                    GamepadAction::Right,
                    "Direita",
                    state.gamepad_bindings.right,
                ),
                (
                    GamepadAction::Button1,
                    "Botão 1",
                    state.gamepad_bindings.button_1,
                ),
                (
                    GamepadAction::Button2,
                    "Botão 2",
                    state.gamepad_bindings.button_2,
                ),
                (
                    GamepadAction::Pause,
                    "Solicitar NMI",
                    state.gamepad_bindings.pause,
                ),
            ] {
                ui.label(label);
                ui.monospace(format!("Botão {button}"));
                if *state.pending_gamepad_binding == Some(action) {
                    ui.weak("Pressione o botão...");
                } else if ui.small_button("Remapear").clicked() {
                    *state.pending_gamepad_binding = Some(action);
                    *state.pending_key_binding = None;
                }
                ui.end_row();
            }
        });
    if state.pending_gamepad_binding.is_some()
        && ui.small_button("Cancelar captura do controle").clicked()
    {
        *state.pending_gamepad_binding = None;
    }
}

/// Render audio output configuration without consuming permanent viewport space.
fn render_audio_menu(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    let mut audio_enabled = state.audio_state.is_enabled();
    if ui.checkbox(&mut audio_enabled, "Áudio ativo").changed() {
        *state.audio_state = if audio_enabled {
            AudioState::Enabled
        } else {
            AudioState::Disabled
        };
        *state.audio_command = Some(if audio_enabled {
            AudioCommand::Configure
        } else {
            AudioCommand::Disable
        });
    }
    ui.separator();
    ui.label("Saída");
    if ui
        .radio_value(state.audio_backend, AudioBackend::Wasapi, "WASAPI Shared")
        .changed()
        || ui
            .radio_value(state.audio_backend, AudioBackend::XAudio2, "XAudio2")
            .changed()
    {
        *state.audio_state = AudioState::Enabled;
        *state.audio_command = Some(AudioCommand::Configure);
    }
    if ui
        .add(egui::Slider::new(state.audio_volume, 0..=100).text("Volume"))
        .changed()
        && state.audio_state.is_enabled()
    {
        *state.audio_command = Some(AudioCommand::SetVolume(*state.audio_volume));
    }
}

/// Render less frequently used machine and input options.
fn render_options_menu(ui: &mut egui::Ui, state: &mut HeaderState<'_>) {
    ui.strong("FPS no rodapé");
    ui.checkbox(state.fps_overlay_enabled, "Exibir FPS na barra inferior");
    ui.add_enabled_ui(*state.fps_overlay_enabled, |ui| {
        egui::ComboBox::from_id_salt("fps_footer_size")
            .selected_text(state.fps_overlay_size.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(state.fps_overlay_size, FpsOverlaySize::Small, "Pequeno");
                ui.selectable_value(state.fps_overlay_size, FpsOverlaySize::Medium, "Médio");
                ui.selectable_value(*state.fps_overlay_size, FpsOverlaySize::Large, "Grande");
            });
    });
    ui.separator();
    let mut show_status = state.footer_mode.is_visible();
    if ui
        .checkbox(&mut show_status, "Exibir status no rodapé")
        .changed()
    {
        *state.footer_mode = if show_status {
            FooterMode::Visible
        } else {
            FooterMode::Hidden
        };
    }
    let mut fullscreen = state.root_fullscreen;
    if ui.checkbox(&mut fullscreen, "Tela cheia (F11)").changed() {
        *state.fullscreen_requested = Some(fullscreen);
    }
    ui.separator();
}

fn render_machine_controls(
    ui: &mut egui::Ui,
    viewport: &NativeViewportState,
    command: &mut Option<UiCommand>,
) {
    let worker = viewport.worker.as_ref();
    add_machine_button(
        ui,
        "▶",
        "Iniciar",
        worker.is_some_and(|worker| !worker.is_running()),
        UiCommand::Start,
        command,
    );
    add_machine_button(
        ui,
        "Ⅱ",
        "Pausar",
        worker.is_some_and(SmsTemporalWorker::is_running),
        UiCommand::Pause,
        command,
    );
    add_machine_button(ui, "■", "Parar", worker.is_some(), UiCommand::Stop, command);
    add_machine_button(
        ui,
        "↻",
        "Reset",
        worker.is_some(),
        UiCommand::Reset,
        command,
    );
    add_machine_button(
        ui,
        "⏭",
        "Avançar um quadro",
        worker.is_some_and(|worker| !worker.is_running()),
        UiCommand::StepFrame,
        command,
    );
}

fn add_machine_button(
    ui: &mut egui::Ui,
    icon: &str,
    description: &str,
    enabled: bool,
    value: UiCommand,
    command: &mut Option<UiCommand>,
) {
    let button =
        egui::Button::new(egui::RichText::new(icon).size(18.0)).min_size(egui::vec2(38.0, 30.0));
    if ui
        .add_enabled(enabled, button)
        .on_hover_text(description)
        .clicked()
    {
        *command = Some(value);
    }
}

fn map_sdl3_gamepad(state: DeviceState, profile: &InputProfile) -> (ControllerButtons, bool) {
    let mut actions = ActionState::default();
    profile.evaluate(&state, &mut actions);
    let is_active = |index| ActionId::new(index).is_some_and(|action| actions.is_active(action));
    (
        ControllerButtons {
            up: is_active(0),
            down: is_active(1),
            left: is_active(2),
            right: is_active(3),
            button_1: is_active(4),
            button_2: is_active(5),
        },
        is_active(6),
    )
}

fn render_input_controls(
    ui: &mut egui::Ui,
    devices: &[DeviceDescriptor],
    selected: &mut Option<DeviceId>,
    error: Option<&str>,
) {
    ui.horizontal(|ui| {
        ui.label("Controle:");
        let selected_name = devices
            .iter()
            .find(|device| Some(device.id) == *selected)
            .map_or("Nenhum gamepad SDL3", |device| device.name.as_str());
        egui::ComboBox::from_id_salt("sms_gamepad")
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                for device in devices
                    .iter()
                    .filter(|device| device.class == DeviceClass::Gamepad)
                {
                    ui.selectable_value(selected, Some(device.id), &device.name);
                }
            });
        if devices
            .iter()
            .any(|device| device.class == DeviceClass::Gamepad)
        {
            ui.label(format!(
                "{} gamepad(s) detectado(s)",
                devices
                    .iter()
                    .filter(|device| device.class == DeviceClass::Gamepad)
                    .count()
            ));
        } else {
            ui.label("Conecte um gamepad compatível com SDL3");
        }
    });
    if let Some(error) = error {
        ui.colored_label(egui::Color32::YELLOW, format!("Input SDL3: {error}"));
    }
}

fn render_runtime_status(
    ui: &mut egui::Ui,
    viewport: &NativeViewportState,
    diagnostics_enabled: bool,
    diagnostics_command: &mut Option<bool>,
    diagnostics_error: Option<&str>,
    debugger_window_open: &AtomicBool,
) {
    if ui.button("Abrir janela do Debugger").clicked() {
        debugger_window_open.store(true, Ordering::Release);
    }
    let mut enabled = diagnostics_enabled;
    if ui
        .checkbox(&mut enabled, "Ativar DEBUG e coleta de dados")
        .on_hover_text("Desativado por padrão. Habilita logs e métricas diagnósticas opcionais.")
        .changed()
    {
        *diagnostics_command = Some(enabled);
    }
    ui.label(if diagnostics_enabled {
        "DEBUG ATIVO — coleta diagnóstica habilitada"
    } else {
        "DEBUG DESATIVADO — logs e métricas opcionais suspensos"
    });
    if let Some(error) = diagnostics_error {
        ui.colored_label(egui::Color32::RED, error);
    }
    ui.separator();
    ui.label(format!("Estado: {}", viewport.status()));
    if let Some(worker) = viewport.worker.as_ref() {
        if diagnostics_enabled {
            render_runtime_telemetry(ui, worker);
        } else {
            ui.weak("As métricas detalhadas não são coletadas enquanto DEBUG estiver desligado.");
        }
        render_output_errors(ui, worker);
    }
}

fn render_runtime_telemetry(ui: &mut egui::Ui, worker: &SmsTemporalWorker) {
    egui::Grid::new("sms_runtime_telemetry")
        .num_columns(2)
        .show(ui, |ui| {
            render_video_runtime_metrics(ui, worker);
            render_audio_runtime_metrics(ui, worker);
        });
}

fn render_video_runtime_metrics(ui: &mut egui::Ui, worker: &SmsTemporalWorker) {
    ui.label("Frame");
    ui.label(worker.frame_number().to_string());
    ui.end_row();

    ui.label("Instruções Z80");
    ui.label(worker.total_instructions().to_string());
    ui.end_row();

    ui.label("Frames publicados");
    ui.label(worker.video_last_published_sequence().to_string());
    ui.end_row();

    ui.label("Frames substituídos na apresentação");
    ui.label(worker.video_replaced_frames().to_string());
    ui.end_row();
}

fn render_audio_runtime_metrics(ui: &mut egui::Ui, worker: &SmsTemporalWorker) {
    let audio = worker.audio_output_metrics();

    ui.label("Áudio — taxa configurada");
    ui.label(format!("{} Hz", audio.sample_rate_hz));
    ui.end_row();

    ui.label("Áudio — underrun FIFO (eventos / frames)");
    ui.label(format!(
        "{} / {}",
        audio.underrun_events, audio.underrun_frames
    ));
    ui.end_row();

    ui.label("Áudio — starvation endpoint XAudio2");
    ui.label(audio.endpoint_starvation_events.to_string());
    ui.end_row();

    ui.label("Áudio — descartes (overflow / flush)");
    ui.label(format!(
        "{} / {} frames",
        audio.dropped_frames, audio.flushed_frames
    ));
    ui.end_row();

    ui.label("Áudio — perdas em SubmitSourceBuffer");
    ui.label(format!("{} frames", audio.submission_failed_frames));
    ui.end_row();

    ui.label("Áudio — blocos com overflow");
    ui.label(audio.dropped_blocks.to_string());
    ui.end_row();

    ui.label("Áudio — FIFO ocupação / capacidade");
    ui.label(format!(
        "{} / {} frames",
        audio.queue_queued_frames, audio.queue_capacity_frames
    ));
    ui.end_row();

    ui.label("Áudio — high-water / backlog atual");
    ui.label(format!(
        "{} / {} frames",
        audio.queue_high_water_frames, audio.queue_queued_frames
    ));
    ui.end_row();

    ui.label("Áudio — frames aceitos / consumidos");
    ui.label(format!(
        "{} / {}",
        audio.accepted_frames, audio.consumed_frames
    ));
    ui.end_row();

    ui.label("Áudio — backlog FIFO estimado");
    ui.label(audio.queue_duration_micros().map_or_else(
        || "indisponível".to_owned(),
        |micros| format!("{micros} µs"),
    ));
    ui.end_row();
}

fn configure_audio_output(
    worker: &SmsTemporalWorker,
    backend: AudioBackend,
    volume: u8,
) -> Result<(), String> {
    const OUTPUT_SAMPLE_RATE: u32 = 44_100;
    const BUFFER_FRAMES: u32 = 512;
    const QUEUE_BLOCKS: usize = 8;
    worker.configure_audio_output(
        move || -> Result<Box<dyn sega_sms_runtime::AudioSession>, String> {
            match backend {
                AudioBackend::Wasapi => {
                    let sink =
                        NativeWasapiOutput::new(OUTPUT_SAMPLE_RATE, BUFFER_FRAMES, QUEUE_BLOCKS)
                            .map_err(|error| error.to_string())?;
                    let session = sink.into_audio_session(OUTPUT_SAMPLE_RATE, volume)?;
                    Ok(Box::new(session))
                }
                AudioBackend::XAudio2 => {
                    let sink =
                        NativeXAudio2Output::new(OUTPUT_SAMPLE_RATE, BUFFER_FRAMES, QUEUE_BLOCKS)
                            .map_err(|error| error.to_string())?;
                    let session = sink.into_audio_session(OUTPUT_SAMPLE_RATE, volume)?;
                    Ok(Box::new(session))
                }
            }
        },
    )
}

fn render_output_errors(ui: &mut egui::Ui, worker: &SmsTemporalWorker) {
    if let Some(error) = worker.video_output_error() {
        ui.colored_label(egui::Color32::YELLOW, format!("Vídeo: {error}"));
    }
    if let Some(error) = worker.audio_output_error() {
        ui.colored_label(egui::Color32::YELLOW, format!("Áudio: {error}"));
    }
    if let Some(error) = worker.runtime_error() {
        ui.colored_label(egui::Color32::RED, format!("Runtime: {error}"));
    }
}

fn render_video_viewport(
    ui: &mut egui::Ui,
    aspect_ratio: AspectRatioMode,
    show_status: bool,
) -> egui::Rect {
    ui.add_space(1.0);

    // Use all remaining height unless the compact status footer is enabled.
    let available = ui.available_size().max(egui::Vec2::splat(1.0));
    let reserved_footer_height = if show_status { 25.0 } else { 0.0 };
    let viewport_available =
        egui::vec2(available.x, (available.y - reserved_footer_height).max(1.0));
    let viewport_size = fit_viewport_size(viewport_available, aspect_ratio.value());
    let horizontal_padding = ((available.x - viewport_size.x) * 0.5).max(0.0);

    let mut viewport_rect = egui::Rect::NOTHING;
    ui.horizontal(|ui| {
        ui.add_space(horizontal_padding);
        let (rect, _) = ui.allocate_exact_size(viewport_size, egui::Sense::hover());
        viewport_rect = rect;
    });
    ui.painter()
        .rect_filled(viewport_rect, 0.0, egui::Color32::BLACK);
    viewport_rect
}

/// Fit the selected display aspect ratio inside the available viewport without distortion.
fn fit_viewport_size(available: egui::Vec2, aspect_ratio: f32) -> egui::Vec2 {
    let available = available.max(egui::Vec2::splat(1.0));
    let width = available.x.min(available.y * aspect_ratio).max(1.0);
    let height = (width / aspect_ratio).min(available.y).max(1.0);
    egui::vec2(width, height)
}

fn render_footer(
    ui: &mut egui::Ui,
    viewport: &NativeViewportState,
    rom_path: &str,
    fps: Option<f32>,
    fps_font_size: f32,
) {
    ui.horizontal(|ui| {
        ui.small(viewport.status());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(fps) = fps {
                ui.label(
                    egui::RichText::new(format!("{fps:.0} FPS"))
                        .strong()
                        .size(fps_font_size),
                );
                ui.separator();
            }
            let rom_name = Path::new(rom_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Nenhuma ROM aberta");
            ui.small(format!("ROM: {rom_name}"));
        });
    });
}

fn resolve_parent_hwnd(
    frame: &eframe::Frame,
    context: &egui::Context,
    error: &mut Option<String>,
) -> Option<usize> {
    let Some(window) = frame.winit_window() else {
        *error = Some("A janela nativa do eframe ainda não está disponível".to_owned());
        context.request_repaint_after(Duration::from_millis(16));
        return None;
    };

    let window_handle = match window.window_handle() {
        Ok(handle) => handle,
        Err(handle_error) => {
            *error = Some(format!(
                "Não foi possível obter o HWND do eframe: {handle_error}"
            ));
            context.request_repaint_after(Duration::from_millis(16));
            return None;
        }
    };

    if let RawWindowHandle::Win32(handle) = window_handle.as_raw() {
        Some(handle.hwnd.get().cast_unsigned())
    } else {
        *error = Some("O backend nativo atual não forneceu um HWND Win32".to_owned());
        None
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Win32 requires signed integer pixel coordinates; values are rounded and clamped before conversion."
)]
fn to_win32_pixel(value: f32) -> i32 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }

    let rounded = f64::from(value).round();
    if !rounded.is_finite() || rounded >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        rounded as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{FpsCounter, FpsOverlaySize, SegaApp};
    use eframe::egui;
    use sega_input_runtime::{AxisSemantic, DeviceState};
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    #[test]
    fn fullscreen_menu_is_hidden_until_pointer_reaches_top_edge() {
        assert!(!super::should_reveal_fullscreen_menu(
            true,
            false,
            Some(120.0),
            0.0
        ));
        assert!(super::should_reveal_fullscreen_menu(
            true,
            false,
            Some(40.0),
            0.0
        ));
        assert!(super::should_reveal_fullscreen_menu(true, true, None, 0.0));
        assert!(super::should_reveal_fullscreen_menu(
            false, false, None, 0.0
        ));
    }

    #[test]
    fn keyboard_bindings_default_to_documented_shortcuts() {
        let bindings = super::KeyboardBindings::default();
        assert_eq!(bindings.key(super::KeyboardAction::Up), egui::Key::ArrowUp);
        assert_eq!(bindings.key(super::KeyboardAction::Button1), egui::Key::Z);
        assert_eq!(
            bindings.key(super::KeyboardAction::Fullscreen),
            egui::Key::F11
        );
        assert_eq!(super::key_from_name("F9"), Some(egui::Key::F9));
        assert_eq!(super::key_from_name("not-a-key"), None);
        for key in egui::Key::ALL {
            assert_eq!(super::key_from_name(key.name()), Some(*key));
        }
    }

    #[test]
    fn gamepad_bindings_are_used_for_physical_button_mapping() {
        let mut state = DeviceState::default();
        state.buttons.set(4, true).unwrap();
        let mut bindings = super::GamepadBindings::default();
        bindings.set(super::GamepadAction::Button1, 4);
        let profile = bindings.to_profile();
        let (mapped, _) = super::map_sdl3_gamepad(state, &profile);
        assert!(mapped.button_1);
        assert!(!mapped.button_2);
    }

    #[test]
    fn debugger_window_geometry_rejects_invalid_or_unusable_values() {
        assert_eq!(
            crate::debugger_window::parse_saved_position("120.5,240.0"),
            Some(egui::pos2(120.5, 240.0))
        );
        assert!(crate::debugger_window::parse_saved_position("NaN,20").is_none());
        assert!(crate::debugger_window::parse_saved_position("invalid").is_none());
        assert_eq!(
            crate::debugger_window::parse_debug_address("0x00AF"),
            Some(0x00AF)
        );
        assert_eq!(crate::debugger_window::parse_debug_address("invalid"), None);
        assert_eq!(
            crate::debugger_window::parse_saved_size("980,760"),
            Some(egui::vec2(980.0, 760.0))
        );
        assert!(crate::debugger_window::parse_saved_size("400,300").is_none());
        assert!(crate::debugger_window::parse_saved_size("NaN,760").is_none());
        assert_eq!(
            crate::debugger_window::parse_saved_root_size("1440,900"),
            Some(egui::vec2(1440.0, 900.0))
        );
        assert!(crate::debugger_window::parse_saved_root_size("900,600").is_none());
    }

    #[test]
    fn diagnostics_collection_is_opt_in_by_default() {
        let app = SegaApp::default();
        assert!(!app.diagnostics_enabled);
        assert!(app.diagnostics_error.is_none());
        assert_eq!(app.region, "usa");
        assert!(app.fps_overlay_enabled);
        assert_eq!(app.fps_overlay_size, FpsOverlaySize::Medium);
    }

    #[test]
    fn fps_counter_ignores_short_sampling_windows() {
        let now = Instant::now();
        let mut counter = FpsCounter::default();
        counter.reset(10, now);
        assert!(!counter.update(20, now + Duration::from_millis(250)));
        assert_eq!(counter.fps, 0.0);
    }

    #[test]
    fn fps_counter_uses_presented_frame_delta() {
        let now = Instant::now();
        let mut counter = FpsCounter::default();
        counter.reset(100, now);
        assert!(counter.update(160, now + Duration::from_secs(1)));
        assert!((counter.fps - 60.0).abs() < 0.01);
    }

    #[test]
    fn fps_counter_restarts_from_the_presented_sequence_when_overlay_is_reenabled() {
        let hidden_since = Instant::now();
        let mut counter = FpsCounter::default();
        counter.reset(100, hidden_since);

        let reenabled_at = hidden_since + Duration::from_secs(30);
        counter.reset(1_900, reenabled_at);
        assert!(counter.update(1_960, reenabled_at + Duration::from_secs(1)));
        assert!((counter.fps - 60.0).abs() < 0.01);
    }

    #[test]
    fn fps_overlay_sizes_are_monotonic() {
        assert!(FpsOverlaySize::Small.dimensions().x < FpsOverlaySize::Medium.dimensions().x);
        assert!(FpsOverlaySize::Medium.dimensions().x < FpsOverlaySize::Large.dimensions().x);
    }

    #[test]
    fn stale_popup_close_does_not_clear_newly_selected_menu() {
        let mut active_menu = Some(super::TopLevelMenu::Video);

        super::close_menu_if_current(&mut active_menu, super::TopLevelMenu::File, true);

        assert_eq!(active_menu, Some(super::TopLevelMenu::Video));
    }

    #[test]
    fn current_popup_close_clears_active_menu() {
        let mut active_menu = Some(super::TopLevelMenu::Audio);

        super::close_menu_if_current(&mut active_menu, super::TopLevelMenu::Audio, true);

        assert_eq!(active_menu, None);
    }

    #[test]
    fn popup_remains_open_when_no_close_was_requested() {
        let mut active_menu = Some(super::TopLevelMenu::Options);

        super::close_menu_if_current(&mut active_menu, super::TopLevelMenu::Options, false);

        assert_eq!(active_menu, Some(super::TopLevelMenu::Options));
    }

    #[test]
    fn menu_popup_width_stays_bounded_by_preferred_menu_width() {
        let root = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1680.0, 900.0));
        let measured = egui::vec2(1680.0, 245.0);

        let size = super::menu_popup_size(super::TopLevelMenu::File, measured, root);

        assert_eq!(size.x, super::TopLevelMenu::File.minimum_size().x);
        assert!(size.x < root.width());
        assert!(size.y >= super::TopLevelMenu::File.minimum_size().y);
    }

    #[test]
    fn menu_popup_is_clamped_to_root_client_bounds() {
        let root = egui::Rect::from_min_size(egui::pos2(100.0, 60.0), egui::vec2(800.0, 600.0));
        let anchor = egui::pos2(650.0, 540.0);
        let size = egui::vec2(400.0, 300.0);

        let position = super::menu_popup_position(root, anchor, size);

        assert!(position.x >= root.left());
        assert!(position.y >= root.top() + super::MENU_BAR_HEIGHT);
        assert!(position.x + size.x <= root.right());
        assert!(position.y + size.y <= root.bottom());
    }

    #[test]
    fn menu_popup_viewport_is_opaque_undecorated_and_non_resizable() {
        let position = egui::pos2(120.0, 80.0);
        let size = egui::vec2(360.0, 280.0);
        let builder = super::menu_popup_viewport_builder("Arquivo", position, size);

        assert_eq!(builder.position, Some(position));
        assert_eq!(builder.inner_size, Some(size));
        assert_eq!(builder.decorations, Some(false));
        assert_eq!(builder.resizable, Some(false));
        assert_eq!(builder.taskbar, Some(false));
        assert_eq!(builder.transparent, Some(false));
    }

    #[test]
    fn recent_rom_list_moves_duplicate_to_front_and_caps_entries() {
        let mut recent = (0..super::MAX_RECENT_ROMS)
            .map(|index| PathBuf::from(format!("game-{index}.sms")))
            .collect::<Vec<_>>();
        super::update_recent_rom_list(&mut recent, Path::new("game-5.sms"));
        assert_eq!(recent[0], PathBuf::from("game-5.sms"));
        assert_eq!(recent.len(), super::MAX_RECENT_ROMS);

        super::update_recent_rom_list(&mut recent, Path::new("new-game.sms"));
        assert_eq!(recent[0], PathBuf::from("new-game.sms"));
        assert_eq!(recent.len(), super::MAX_RECENT_ROMS);
        assert!(!recent.contains(&PathBuf::from("game-9.sms")));
    }

    #[test]
    fn aspect_ratio_modes_preserve_requested_geometry() {
        assert_eq!(super::AspectRatioMode::OneToOne.value(), 1.0);
        assert!((super::AspectRatioMode::FourThree.value() - 4.0 / 3.0).abs() < f32::EPSILON);
        assert!((super::AspectRatioMode::SixteenNine.value() - 16.0 / 9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn viewport_fit_keeps_square_ratio_inside_wide_window() {
        let available = egui::vec2(1600.0, 800.0);
        let fitted = super::fit_viewport_size(available, 1.0);
        assert_eq!(fitted, egui::vec2(800.0, 800.0));
    }

    #[test]
    fn viewport_fit_keeps_four_three_ratio_inside_wide_window() {
        let available = egui::vec2(1600.0, 800.0);
        let fitted = super::fit_viewport_size(available, 4.0 / 3.0);
        assert!((fitted.x / fitted.y - 4.0 / 3.0).abs() < 0.0001);
        assert!(fitted.x <= available.x && fitted.y <= available.y);
    }

    #[test]
    fn viewport_fit_keeps_sixteen_nine_ratio_inside_wide_window() {
        let available = egui::vec2(1600.0, 800.0);
        let fitted = super::fit_viewport_size(available, 16.0 / 9.0);
        assert!((fitted.x / fitted.y - 16.0 / 9.0).abs() < 0.0001);
        assert!(fitted.x <= available.x && fitted.y <= available.y);
    }

    #[test]
    fn viewport_fit_uses_height_constraint_for_tall_window() {
        let available = egui::vec2(800.0, 1000.0);
        let fitted = super::fit_viewport_size(available, 16.0 / 9.0);
        assert_eq!(fitted, egui::vec2(800.0, 450.0));
    }

    #[test]
    fn sdl3_gamepad_maps_standard_buttons_hat_and_left_stick() {
        let mut state = DeviceState::default();
        state.buttons.set(0, true).unwrap();
        state.buttons.set(1, true).unwrap();
        state.buttons.set(6, true).unwrap();
        state
            .set_axis(0, -20_000, AxisSemantic::LeftStickX)
            .unwrap();
        state.set_hat(0, 0x01).unwrap();

        let profile = super::GamepadBindings::default().to_profile();
        let (buttons, pause) = super::map_sdl3_gamepad(state, &profile);
        assert!(buttons.up);
        assert!(buttons.left);
        assert!(buttons.button_1);
        assert!(buttons.button_2);
        assert!(pause);
    }

    #[test]
    fn sdl3_gamepad_maps_axis_thresholds_without_button_activity() {
        let mut state = DeviceState::default();
        state.set_axis(0, 20_000, AxisSemantic::LeftStickX).unwrap();
        state.set_axis(1, 20_000, AxisSemantic::LeftStickY).unwrap();

        let profile = super::GamepadBindings::default().to_profile();
        let (buttons, pause) = super::map_sdl3_gamepad(state, &profile);
        assert!(buttons.right);
        assert!(buttons.down);
        assert!(!buttons.button_1);
        assert!(!buttons.button_2);
        assert!(!pause);
    }

    #[test]
    fn application_shell_renders_without_native_window_or_hardware() {
        let context = egui::Context::default();
        let mut app = SegaApp::default();

        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let _ = app.render(ui);
        });

        assert_ne!(
            output.shapes.len(),
            0,
            "A interface deve produzir elementos visuais"
        );
        output.drop_without_applying_deltas();
    }

    #[test]
    fn zip_loader_reads_the_only_sms_entry() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("games/test.sms", zip::write::SimpleFileOptions::default())
            .expect("ZIP entry should be created");
        writer
            .write_all(&[0xAA, 0x55, 0x01])
            .expect("ROM bytes should be written");
        let archive = writer.finish().expect("ZIP archive should be finalized");

        let rom = super::read_sms_from_zip(Cursor::new(archive.into_inner()))
            .expect("single SMS entry should load");
        assert_eq!(rom, [0xAA, 0x55, 0x01]);
    }

    #[test]
    fn zip_loader_rejects_archives_with_multiple_sms_entries() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in ["one.sms", "two.SMS"] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .expect("ZIP entry should be created");
            writer
                .write_all(&[0x00])
                .expect("ROM byte should be written");
        }
        let archive = writer.finish().expect("ZIP archive should be finalized");

        let error = super::read_sms_from_zip(Cursor::new(archive.into_inner()))
            .expect_err("ambiguous archives must be rejected");
        assert!(error.contains("mais de um arquivo .sms"));
    }
}
