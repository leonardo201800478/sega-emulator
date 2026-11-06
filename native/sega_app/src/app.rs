use std::fs;
use std::time::Duration;

use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use sega_sms_runtime::SmsTemporalWorker;
use sega_video_runtime::d3d12::{D3d12ChildWindow, D3d12MailboxSession};

/// Native eframe application shell connected to the Rust SMS temporal runtime.
#[derive(Default)]
pub struct SegaApp {
    rom_path: String,
    region: String,
    native_viewport: NativeViewportState,
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
            ui.horizontal(|ui| {
                ui.label("ROM (.sms/.bin):");
                ui.text_edit_singleline(&mut self.rom_path);
                load_requested = ui.button("Carregar ROM").clicked();
            });
            ui.horizontal(|ui| {
                ui.label("Região:");
                egui::ComboBox::from_id_salt("sms_region")
                    .selected_text(if self.region.is_empty() { "USA / NTSC" } else { &self.region })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.region, "usa".to_owned(), "USA / NTSC");
                        ui.selectable_value(&mut self.region, "brazil".to_owned(), "Brasil / PAL-M");
                        ui.selectable_value(&mut self.region, "europe".to_owned(), "Europa / PAL");
                        ui.selectable_value(&mut self.region, "japan".to_owned(), "Japão / NTSC");
                    });
            });

            ui.horizontal(|ui| {
                if ui.add_enabled(self.native_viewport.worker.as_ref().is_some_and(|worker| !worker.is_running()), egui::Button::new("Iniciar")).clicked() {
                    command = Some(UiCommand::Start);
                }
                if ui.add_enabled(self.native_viewport.worker.as_ref().is_some_and(SmsTemporalWorker::is_running), egui::Button::new("Pausar")).clicked() {
                    command = Some(UiCommand::Pause);
                }
                if ui.add_enabled(self.native_viewport.worker.is_some(), egui::Button::new("Parar")).clicked() {
                    command = Some(UiCommand::Stop);
                }
                if ui.add_enabled(self.native_viewport.worker.is_some(), egui::Button::new("Reset")).clicked() {
                    command = Some(UiCommand::Reset);
                }
                if ui.add_enabled(self.native_viewport.worker.as_ref().is_some_and(|worker| !worker.is_running()), egui::Button::new("Avançar quadro")).clicked() {
                    command = Some(UiCommand::StepFrame);
                }
                if ui.add_enabled(self.native_viewport.worker.is_some(), egui::Button::new("Fechar ROM")).clicked() {
                    command = Some(UiCommand::CloseMachine);
                }
            });

            ui.separator();
            ui.label(format!("Estado: {}", self.native_viewport.status()));
            if let Some(worker) = self.native_viewport.worker.as_ref() {
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

            ui.add_space(6.0);
            let width = ui.available_width().max(1.0);
            let height = (width * 0.75).min(ui.available_height().max(1.0));
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
            viewport_rect = rect;

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.separator();
                ui.small("SMS temporal runtime · framebuffer RGB888 256×192 · D3D12");
            });
        });

        if load_requested {
            self.load_rom_from_path();
        }
        if let Some(command) = command {
            self.apply_command(command);
        }

        viewport_rect
    }

    fn load_rom_from_path(&mut self) {
        let path = self.rom_path.trim();
        if path.is_empty() {
            self.native_viewport.error = Some("Informe o caminho de uma ROM .sms ou .bin".to_owned());
            return;
        }

        let rom = match fs::read(path) {
            Ok(rom) if !rom.is_empty() => rom,
            Ok(_) => {
                self.native_viewport.error = Some("O arquivo da ROM está vazio".to_owned());
                return;
            }
            Err(error) => {
                self.native_viewport.error = Some(format!("Não foi possível ler a ROM: {error}"));
                return;
            }
        };

        let region = if self.region.is_empty() { "usa" } else { &self.region };
        match SmsTemporalWorker::new(rom, region) {
            Ok(worker) => self.native_viewport.replace_worker(worker),
            Err(error) => self.native_viewport.error = Some(format!("Falha ao criar runtime SMS: {error}")),
        }
    }

    fn apply_command(&mut self, command: UiCommand) {
        if matches!(command, UiCommand::CloseMachine) {
            self.native_viewport.start_requested = false;
            if let Some(worker) = self.native_viewport.worker.take() {
                let _ = worker.stop();
                if self.native_viewport.video_attached {
                    let _ = worker.detach_video_mailbox();
                }
                drop(worker);
            }
            self.native_viewport.video_attached = false;
            self.native_viewport.error = None;
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
        let Some(parent_hwnd) = resolve_parent_hwnd(frame, context, &mut self.native_viewport.error) else {
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
                self.native_viewport.error = Some(format!("Falha ao criar HWND filho D3D12: {error}"));
                return false;
            }
        };

        let session = match D3d12MailboxSession::start_for_existing_window_async(256, 192, child.handle()) {
            Ok(session) => session,
            Err(error) => {
                self.native_viewport.error = Some(format!("Falha ao iniciar worker D3D12: {error}"));
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
                    self.native_viewport.error = Some(format!("Falha ao redimensionar viewport D3D12: {error}"));
                    context.request_repaint();
                }
            }
        }
    }

    fn poll_native_startup(&mut self, context: &egui::Context) {
        let startup = self.native_viewport.session.as_mut().map(D3d12MailboxSession::poll_startup);
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
        let mailbox = self.native_viewport.session.as_ref().map(D3d12MailboxSession::mailbox);
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
                            self.native_viewport.error = Some(format!("Falha ao iniciar SMS: {error}"));
                        }
                    }
                }
                context.request_repaint();
            }
            Err(error) => self.native_viewport.error = Some(format!("Falha ao conectar vídeo SMS: {error}")),
        }
    }

    fn monitor_native_state(&mut self, context: &egui::Context) {
        if self.native_viewport.ready
            && self.native_viewport.session.as_ref().is_some_and(|session| !session.is_running())
        {
            self.native_viewport.shutdown();
            self.native_viewport.error = Some("O worker de apresentação D3D12 foi encerrado".to_owned());
            context.request_repaint();
        } else {
            context.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl eframe::App for SegaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let viewport_rect = self.render(ui);
        self.update_native_viewport(frame, ui.ctx(), viewport_rect);
    }
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
            *error = Some(format!("Não foi possível obter o HWND do eframe: {handle_error}"));
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

    #[test]
    fn application_shell_renders_without_native_window_or_hardware() {
        let context = egui::Context::default();
        let mut app = SegaApp::default();

        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let _ = app.render(ui);
        });

        assert!(!output.shapes.is_empty());
        output.drop_without_applying_deltas();
    }
}
