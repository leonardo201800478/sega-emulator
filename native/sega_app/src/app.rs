use eframe::egui;

/// Initial application shell. It deliberately does not create a machine or load media.
#[derive(Default)]
pub struct SegaApp {
    show_architecture: bool,
}

impl SegaApp {
    /// Render the initial application shell without starting emulation.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
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
                ui.label(
                    "Esta etapa valida somente o shell egui/eframe. Nenhuma ROM ou máquina                      é iniciada e o frontend Python legado permanece intacto.",
                );

                ui.add_space(12.0);
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
                            ui.label("D3D12 nativo — integração pendente");
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

                ui.with_layout(
                    egui::Layout::bottom_up(egui::Align::Min),
                    |ui| {
                        ui.separator();
                        ui.small("M1 · shell sem hardware · nenhuma dependência Python no binário");
                    },
                );
            });
        });
    }
}

impl eframe::App for SegaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
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

        let output = context.run(Default::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| app.render(ui));
        });

        assert!(!output.shapes.is_empty());
    }
}
