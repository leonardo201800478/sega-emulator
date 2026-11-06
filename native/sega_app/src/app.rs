use std::time::Duration;

use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use sega_video_runtime::d3d12::{D3d12ChildWindow, D3d12MailboxSession};
use sega_video_runtime::{PixelFormat, VideoFrame};

/// Native eframe application shell with a D3D12 child-window viewport.
#[derive(Default)]
pub struct SegaApp {
    show_architecture: bool,
    native_viewport: NativeViewportState,
}

/// Owns the UI-thread HWND child and the worker session that borrows it.
///
/// Field ownership is intentionally explicit: shutdown joins the D3D12 worker
/// before the child HWND is destroyed.
#[derive(Default)]
struct NativeViewportState {
    parent_hwnd: Option<usize>,
    session: Option<D3d12MailboxSession>,
    child: Option<D3d12ChildWindow>,
    last_bounds: Option<(i32, i32, i32, i32)>,
    ready: bool,
    diagnostic_frame_submitted: bool,
    error: Option<String>,
}

impl NativeViewportState {
    fn status(&self) -> &str {
        if let Some(error) = self.error.as_deref() {
            error
        } else if self.child.is_none() {
            "Aguardando janela nativa"
        } else if !self.ready {
            "Inicializando D3D12 em worker dedicado"
        } else if self
            .session
            .as_ref()
            .is_some_and(D3d12MailboxSession::is_running)
        {
            "D3D12 ativo — padrão de diagnóstico"
        } else {
            "Worker D3D12 encerrado"
        }
    }

    fn shutdown(&mut self) {
        if let Some(session) = self.session.take() {
            // D3d12MailboxSession joins its worker before returning. The child
            // HWND must remain alive until this operation has completed.
            let _ = session.shutdown();
        }
        self.child = None;
        self.parent_hwnd = None;
        self.last_bounds = None;
        self.ready = false;
        self.diagnostic_frame_submitted = false;
    }
}

impl Drop for NativeViewportState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl SegaApp {
    /// Render the application shell without requiring a native window.
    ///
    /// This entry point remains headless-testable. The eframe callback performs
    /// native HWND/D3D12 work after the layout has produced the viewport rect.
    pub fn render(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let mut viewport_rect = egui::Rect::NOTHING;

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
            ui.heading("Aplicação nativa em construção");
            ui.label(concat!(
                "Esta etapa valida o shell egui/eframe e o host de vídeo D3D12. ",
                "Nenhuma ROM ou máquina é iniciada; o frontend legado permanece intacto."
            ));

            ui.add_space(8.0);
            ui.label(format!("Viewport: {}", self.native_viewport.status()));

            ui.add_space(8.0);
            ui.group(|ui| {
                ui.heading("Estado dos subsistemas");
                egui::Grid::new("sega_application_status")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Interface");
                        ui.label("egui/eframe 0.37.0");
                        ui.end_row();

                        ui.label("Máquina / Scheduler");
                        ui.label("Não iniciado nesta etapa");
                        ui.end_row();

                        ui.label("Vídeo");
                        ui.label("D3D12 nativo em HWND filho");
                        ui.end_row();

                        ui.label("Áudio / Input");
                        ui.label("Não inicializados nesta etapa");
                        ui.end_row();

                        ui.label("Mídia");
                        ui.label("Loader Rust será integrado após o contrato de sessão");
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);
            ui.checkbox(&mut self.show_architecture, "Exibir arquitetura de destino");
            if self.show_architecture {
                ui.group(|ui| {
                    ui.label("sega-app → sega-ui-egui → sega-application");
                    ui.label("                    ↓");
                    ui.label("SMS / SG-1000 runtime Rust + MachineScheduler");
                    ui.label("                    ↓");
                    ui.label("D3D12 host · WASAPI/XAudio2 · input nativo");
                });
            }

            ui.add_space(8.0);
            let width = ui.available_width().max(1.0);
            let height = (width * 0.75).min(ui.available_height().max(1.0));
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
            viewport_rect = rect;

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.separator();
                ui.small("M1b · viewport Win32 filho · renderer D3D12 independente do eframe");
            });
        });

        viewport_rect
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
        self.monitor_native_worker(context);
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
                self.submit_diagnostic_frame();
                context.request_repaint();
            }
            Some(Ok(false)) => {
                // Keep the UI responsive while polling the asynchronous startup.
                context.request_repaint_after(Duration::from_millis(16));
            }
            Some(Err(error)) => {
                self.native_viewport.shutdown();
                self.native_viewport.error = Some(format!("Inicialização D3D12 falhou: {error}"));
                context.request_repaint();
            }
            None => {}
        }
    }

    fn submit_diagnostic_frame(&mut self) {
        if self.native_viewport.diagnostic_frame_submitted {
            return;
        }

        let video_frame = match diagnostic_frame() {
            Ok(frame) => frame,
            Err(error) => {
                self.native_viewport.error = Some(error);
                return;
            }
        };

        let Some(session) = self.native_viewport.session.as_ref() else {
            return;
        };

        match session.submit(video_frame) {
            Ok(()) => self.native_viewport.diagnostic_frame_submitted = true,
            Err(error) => {
                self.native_viewport.error =
                    Some(format!("Falha ao submeter frame diagnóstico: {error}"));
            }
        }
    }

    fn monitor_native_worker(&mut self, context: &egui::Context) {
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
        } else if self.native_viewport.ready {
            context.request_repaint_after(Duration::from_millis(250));
        }
    }
}

impl eframe::App for SegaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let viewport_rect = self.render(ui);
        self.update_native_viewport(frame, ui.ctx(), viewport_rect);
    }
}

fn diagnostic_frame() -> Result<VideoFrame, String> {
    const WIDTH: u32 = 256;
    const HEIGHT: u32 = 192;
    const STRIDE: usize = WIDTH as usize * 4;
    const BARS: [[u8; 4]; 8] = [
        [255, 255, 255, 255],
        [255, 255, 0, 255],
        [0, 255, 255, 255],
        [0, 255, 0, 255],
        [255, 0, 255, 255],
        [255, 0, 0, 255],
        [0, 0, 255, 255],
        [0, 0, 0, 255],
    ];

    let mut pixels = Vec::with_capacity(STRIDE * HEIGHT as usize);
    for _y in 0..HEIGHT {
        for x in 0..WIDTH {
            let bar_index = usize::try_from(x * 8 / WIDTH)
                .map_err(|error| format!("Índice de barra inválido: {error}"))?;
            pixels.extend_from_slice(&BARS[bar_index]);
        }
    }

    VideoFrame::new(1, WIDTH, HEIGHT, PixelFormat::Rgba8888, STRIDE, pixels)
        .map_err(|error| format!("Não foi possível criar o frame diagnóstico: {error}"))
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
    use super::{SegaApp, diagnostic_frame};
    use eframe::egui;

    #[test]
    fn application_shell_renders_without_native_window_or_hardware() {
        let context = egui::Context::default();
        let mut app = SegaApp::default();

        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let _ = app.render(ui);
        });

        assert_ne!(output.shapes, [] as [eframe::epaint::ClippedShape; 0]);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn diagnostic_frame_has_valid_sms_surface_geometry() {
        let frame = diagnostic_frame().expect("diagnostic frame uses fixed valid geometry");
        assert_eq!(frame.width(), 256);
        assert_eq!(frame.height(), 192);
        assert_eq!(frame.stride_bytes(), 256 * 4);
        assert_eq!(frame.sequence(), 1);
    }
}
