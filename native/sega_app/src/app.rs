use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rfd::FileDialog;
use sega_sms_runtime::SmsTemporalWorker;
use sega_video_runtime::d3d12::{D3d12ChildWindow, D3d12MailboxSession};
use zip::ZipArchive;

const MAX_ROM_SIZE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ROM_SIZE_BYTES_U64: u64 = 16 * 1024 * 1024;
const APP_DATA_DIRECTORY: &str = "Sega Emulator";
const LAST_ROM_DIRECTORY_FILE: &str = "last_rom_directory.txt";

/// Native eframe application shell connected to the Rust SMS temporal runtime.
pub struct SegaApp {
    rom_path: String,
    region: String,
    last_rom_directory: Option<PathBuf>,
    rom_dialog_requested: bool,
    aspect_ratio: AspectRatioMode,
    native_viewport: NativeViewportState,
}

impl Default for SegaApp {
    fn default() -> Self {
        Self {
            rom_path: String::new(),
            region: String::new(),
            last_rom_directory: load_last_rom_directory(),
            rom_dialog_requested: false,
            aspect_ratio: AspectRatioMode::FourThree,
            native_viewport: NativeViewportState::default(),
        }
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

    const fn label(self) -> &'static str {
        match self {
            Self::OneToOne => "1:1",
            Self::FourThree => "4:3",
            Self::SixteenNine => "16:9",
        }
    }
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

impl SegaApp {
    /// Render the shell and collect coarse-grained user commands.
    ///
    /// This method remains headless-testable; native HWND/D3D12 work is handled
    /// by the eframe callback after layout has produced the viewport rectangle.
    pub fn render(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let mut viewport_rect = egui::Rect::NOTHING;
        let mut command = None;
        let mut load_requested = false;

        ui.vertical(|ui| {
            render_header(ui);
            render_rom_controls(
                ui,
                &mut self.rom_path,
                &mut self.region,
                &mut self.rom_dialog_requested,
                &mut self.aspect_ratio,
                &mut load_requested,
            );
            render_machine_controls(ui, &self.native_viewport, &mut command);
            render_runtime_status(ui, &self.native_viewport);
            viewport_rect = render_video_viewport(ui, self.aspect_ratio);
            render_footer(ui);
        });

        if load_requested {
            self.load_rom_from_path();
        }
        if let Some(command) = command {
            self.apply_command(command);
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
            .add_filter("ROM do Master System", &["sms", "zip"]);
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
        match SmsTemporalWorker::new(rom, region) {
            Ok(worker) => self.native_viewport.replace_worker(worker),
            Err(error) => {
                self.native_viewport.error = Some(format!("Falha ao criar runtime SMS: {error}"));
            }
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
            Ok(child) => child,
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
                Ok(()) => self.native_viewport.last_bounds = Some(bounds),
                Err(error) => {
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
            context.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl eframe::App for SegaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let viewport_rect = self.render(ui);
        if self.rom_dialog_requested {
            self.rom_dialog_requested = false;
            self.open_rom_dialog(frame);
        }
        self.update_native_viewport(frame, ui.ctx(), viewport_rect);
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

fn render_header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Sega Emulator");
        ui.separator();
        ui.label("Rust · egui/eframe");
        if ui.button("Sair").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    });
    ui.separator();
    ui.heading("Sega Master System");
}

fn render_rom_controls(
    ui: &mut egui::Ui,
    rom_path: &mut String,
    region: &mut String,
    browse_requested: &mut bool,
    aspect_ratio: &mut AspectRatioMode,
    load_requested: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("ROM:");
        ui.add(
            egui::TextEdit::singleline(rom_path)
                .desired_width(420.0)
                .hint_text("Cole o caminho ou procure um arquivo .sms/.zip"),
        );
        *browse_requested = ui.button("Procurar...").clicked();
        *load_requested = ui.button("Carregar ROM").clicked();
    });
    ui.horizontal(|ui| {
        ui.label("Região:");
        egui::ComboBox::from_id_salt("sms_region")
            .selected_text(if region.is_empty() {
                "USA / NTSC".to_owned()
            } else {
                region.clone()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(region, "usa".to_owned(), "USA / NTSC");
                ui.selectable_value(region, "brazil".to_owned(), "Brasil / PAL-M");
                ui.selectable_value(region, "europe".to_owned(), "Europa / PAL");
                ui.selectable_value(region, "japan".to_owned(), "Japão / NTSC");
            });
        ui.label("Proporção:");
        egui::ComboBox::from_id_salt("sms_aspect_ratio")
            .selected_text(aspect_ratio.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(aspect_ratio, AspectRatioMode::OneToOne, "1:1");
                ui.selectable_value(aspect_ratio, AspectRatioMode::FourThree, "4:3");
                ui.selectable_value(aspect_ratio, AspectRatioMode::SixteenNine, "16:9");
            });
    });
}

fn render_machine_controls(
    ui: &mut egui::Ui,
    viewport: &NativeViewportState,
    command: &mut Option<UiCommand>,
) {
    ui.horizontal(|ui| {
        let worker = viewport.worker.as_ref();
        add_machine_button(
            ui,
            "Iniciar",
            worker.is_some_and(|worker| !worker.is_running()),
            UiCommand::Start,
            command,
        );
        add_machine_button(
            ui,
            "Pausar",
            worker.is_some_and(SmsTemporalWorker::is_running),
            UiCommand::Pause,
            command,
        );
        add_machine_button(ui, "Parar", worker.is_some(), UiCommand::Stop, command);
        add_machine_button(ui, "Reset", worker.is_some(), UiCommand::Reset, command);
        add_machine_button(
            ui,
            "Avançar quadro",
            worker.is_some_and(|worker| !worker.is_running()),
            UiCommand::StepFrame,
            command,
        );
        add_machine_button(
            ui,
            "Fechar ROM",
            worker.is_some(),
            UiCommand::CloseMachine,
            command,
        );
    });
}

fn add_machine_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    value: UiCommand,
    command: &mut Option<UiCommand>,
) {
    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
        *command = Some(value);
    }
}

fn render_runtime_status(ui: &mut egui::Ui, viewport: &NativeViewportState) {
    ui.separator();
    ui.label(format!("Estado: {}", viewport.status()));
    if let Some(worker) = viewport.worker.as_ref() {
        egui::Grid::new("sms_runtime_telemetry")
            .num_columns(2)
            .show(ui, |ui| {
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
            });
        render_output_errors(ui, worker);
    }
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

fn render_video_viewport(ui: &mut egui::Ui, aspect_ratio: AspectRatioMode) -> egui::Rect {
    ui.add_space(6.0);

    let available = ui.available_size().max(egui::Vec2::splat(1.0));
    let viewport_size = fit_viewport_size(available, aspect_ratio.value());
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

fn render_footer(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.separator();
        ui.small("SMS temporal runtime · framebuffer RGB888 256×192 · proporção configurável");
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
    use super::SegaApp;
    use eframe::egui;
    use std::io::{Cursor, Write};

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
