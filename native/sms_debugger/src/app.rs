//! Native egui debugger UI. Hardware execution remains exclusively in SmsTemporalWorker.
#![forbid(unsafe_code)]

use eframe::egui;
use rfd::FileDialog;
use sega_sms_runtime::{SmsDebugSnapshot, SmsTemporalWorker};
use std::{fs, path::PathBuf};

const MEMORY_WINDOW: usize = 64;
const MAX_BREAKPOINTS: usize = 64;

/// Native egui application for the Master System debugger.
///
/// The application owns presentation state and communicates with the emulation
/// runtime only through `SmsTemporalWorker`.
pub struct DebuggerApp {
    worker: Option<SmsTemporalWorker>,
    rom_path: Option<PathBuf>,
    running: bool,
    breakpoint_input: String,
    breakpoints: Vec<u16>,
    memory_address: String,
    memory_bytes: Vec<u8>,
    snapshot: Option<SmsDebugSnapshot>,
    texture: Option<egui::TextureHandle>,
    displayed_frame: u64,
    status: String,
}

impl Default for DebuggerApp {
    fn default() -> Self {
        Self {
            worker: None,
            rom_path: None,
            running: false,
            breakpoint_input: "0088".into(),
            breakpoints: Vec::new(),
            memory_address: "C000".into(),
            memory_bytes: Vec::new(),
            snapshot: None,
            texture: None,
            displayed_frame: u64::MAX,
            status: "Abra uma ROM SMS".into(),
        }
    }
}

impl DebuggerApp {
    fn open_rom(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("ROM SMS", &["sms", "bin", "rom"])
            .pick_file()
        else {
            return;
        };

        let result = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|rom| SmsTemporalWorker::new(rom, "usa"));
        match result {
            Ok(worker) => {
                worker.set_diagnostics_enabled(true);
                self.worker = Some(worker);
                self.rom_path = Some(path);
                self.running = false;
                self.breakpoints.clear();
                self.snapshot = None;
                self.memory_bytes.clear();
                self.texture = None;
                self.displayed_frame = u64::MAX;
                self.status = "ROM carregada; máquina pausada".into();
                self.refresh_snapshot();
                self.refresh_memory();
            }
            Err(error) => self.status = format!("Falha ao carregar ROM: {error}"),
        }
    }

    fn refresh_snapshot(&mut self) {
        let Some(worker) = self.worker.as_ref() else {
            self.snapshot = None;
            return;
        };
        let was_running = self.running;
        self.running = worker.is_running();
        match worker.debug_snapshot() {
            Ok(snapshot) => {
                self.snapshot = Some(snapshot);
                if let Some(error) = worker.runtime_error() {
                    self.status = format!("Erro no runtime: {error}");
                } else if was_running && !self.running {
                    self.status = "Execução interrompida pelo runtime".into();
                }
            }
            Err(error) => self.status = format!("Falha ao obter snapshot: {error}"),
        }
    }

    fn refresh_memory(&mut self) {
        let Some(worker) = self.worker.as_ref() else {
            self.memory_bytes.clear();
            return;
        };
        let address = match parse_address(&self.memory_address) {
            Some(address) => address,
            None => {
                self.status = "Endereço de memória inválido".into();
                return;
            }
        };
        match worker.inspect_memory(address, MEMORY_WINDOW) {
            Ok(bytes) => self.memory_bytes = bytes,
            Err(error) => self.status = format!("Falha ao ler memória: {error}"),
        }
    }

    fn add_breakpoint(&mut self) {
        let Some(address) = parse_address(&self.breakpoint_input) else {
            self.status = "Endereço de breakpoint inválido".into();
            return;
        };
        if self.breakpoints.contains(&address) {
            self.status = format!("Breakpoint 0x{address:04X} já existe");
            return;
        }
        if self.breakpoints.len() >= MAX_BREAKPOINTS {
            self.status = format!("Limite de {MAX_BREAKPOINTS} breakpoints atingido");
            return;
        }
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        match worker.set_breakpoint(address, true) {
            Ok(()) => {
                self.breakpoints.push(address);
                self.status = format!("Breakpoint configurado em 0x{address:04X}");
            }
            Err(error) => self.status = format!("Falha ao configurar breakpoint: {error}"),
        }
    }

    fn remove_breakpoint(&mut self, address: u16) {
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        match worker.set_breakpoint(address, false) {
            Ok(()) => {
                self.breakpoints.retain(|item| *item != address);
                self.status = format!("Breakpoint removido de 0x{address:04X}");
            }
            Err(error) => self.status = format!("Falha ao remover breakpoint: {error}"),
        }
    }

    fn step(&mut self) {
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        match worker.step_instruction() {
            Ok(()) => {
                self.running = false;
                self.refresh_snapshot();
                self.refresh_memory();
                self.status = "Uma instrução concluída pelo runtime".into();
            }
            Err(error) => self.status = format!("Falha no step: {error}"),
        }
    }

    fn toggle_run(&mut self) {
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        let result = if self.running {
            worker.pause()
        } else {
            worker.start()
        };
        match result {
            Ok(()) => {
                self.running = !self.running;
                self.status = if self.running {
                    "Execução iniciada no worker".into()
                } else {
                    "Execução pausada no worker".into()
                };
            }
            Err(error) => self.status = format!("Falha ao alterar execução: {error}"),
        }
    }

    fn export_diagnostic(&mut self) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.status = "Nenhum snapshot disponível para exportação".into();
            return;
        };
        let Some(path) = FileDialog::new()
            .set_file_name("sms-debug-report.txt")
            .add_filter("Relatório de diagnóstico", &["txt"])
            .save_file()
        else {
            return;
        };
        let report = format!(
            "Sega Master System — Debug Report\nROM: {}\nPC: {:04X}\nSP: {:04X}\nAF: {:04X}\nBC: {:04X}\nDE: {:04X}\nHL: {:04X}\nAF': {:04X}\nBC': {:04X}\nDE': {:04X}\nHL': {:04X}\nIX: {:04X}\nIY: {:04X}\nWZ: {:04X}\nI: {:02X}\nR: {:02X}\nIFF1: {}\nIFF2: {}\nIM: {}\nHALT: {}\nInstruction T-states: {}\nMachine ticks: {}\nFrame: {}\nVDP status: {:02X}\nVDP scanline: {}\nVDP cycles/line: {}\nMapper banks: {:02X}/{:02X}/{:02X}\nMapper control: {:02X}\nMemory control: {:02X}\n",
            self.rom_path
                .as_ref()
                .map_or_else(|| "(unknown)".into(), |path| path.display().to_string()),
            snapshot.pc,
            snapshot.sp,
            snapshot.af,
            snapshot.bc,
            snapshot.de,
            snapshot.hl,
            snapshot.af_alt,
            snapshot.bc_alt,
            snapshot.de_alt,
            snapshot.hl_alt,
            snapshot.ix,
            snapshot.iy,
            snapshot.wz,
            snapshot.i,
            snapshot.r,
            snapshot.iff1,
            snapshot.iff2,
            snapshot.interrupt_mode,
            snapshot.halted,
            snapshot.instruction_tstates,
            snapshot.machine_time_ticks,
            snapshot.frame_number,
            snapshot.vdp_status,
            snapshot.vdp_scanline,
            snapshot.vdp_cycles_into_line,
            snapshot.mapper_banks[0],
            snapshot.mapper_banks[1],
            snapshot.mapper_banks[2],
            snapshot.mapper_control,
            snapshot.memory_control,
        );
        match fs::write(path, report) {
            Ok(()) => self.status = "Relatório exportado sem dados da ROM".into(),
            Err(error) => self.status = format!("Falha ao exportar relatório: {error}"),
        }
    }

    fn update_video(&mut self, context: &egui::Context) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if self.displayed_frame == snapshot.frame_number
            || snapshot.framebuffer_rgb.len() != 256 * 192 * 3
        {
            return;
        }
        let image = egui::ColorImage::from_rgb([256, 192], &snapshot.framebuffer_rgb);
        if let Some(texture) = self.texture.as_mut() {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(context.load_texture("sms-framebuffer", image, egui::TextureOptions::NEAREST));
        }
        self.displayed_frame = snapshot.frame_number;
    }
}

fn parse_address(value: &str) -> Option<u16> {
    u16::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok()
}

impl eframe::App for DebuggerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        if self.worker.is_some() {
            self.refresh_snapshot();
            if !self.running {
                self.refresh_memory_if_idle();
            }
            self.update_video(&context);
        }

        ui.horizontal_wrapped(|ui| {
            if ui.button("Abrir ROM").clicked() {
                self.open_rom();
            }
            if ui
                .add_enabled(self.worker.is_some(), egui::Button::new("Run / Pause"))
                .clicked()
            {
                self.toggle_run();
            }
            if ui
                .add_enabled(
                    self.worker.is_some() && !self.running,
                    egui::Button::new("Step"),
                )
                .clicked()
            {
                self.step();
            }
            if ui
                .add_enabled(
                    self.snapshot.is_some(),
                    egui::Button::new("Exportar diagnóstico"),
                )
                .clicked()
            {
                self.export_diagnostic();
            }
        });
        ui.label(&self.status);
        ui.separator();

        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ui.heading("CPU / Z80");
                if let Some(snapshot) = self.snapshot.as_ref() {
                    for (name, value) in [
                        ("PC", u32::from(snapshot.pc)),
                        ("SP", u32::from(snapshot.sp)),
                        ("AF", u32::from(snapshot.af)),
                        ("BC", u32::from(snapshot.bc)),
                        ("DE", u32::from(snapshot.de)),
                        ("HL", u32::from(snapshot.hl)),
                        ("AF'", u32::from(snapshot.af_alt)),
                        ("BC'", u32::from(snapshot.bc_alt)),
                        ("DE'", u32::from(snapshot.de_alt)),
                        ("HL'", u32::from(snapshot.hl_alt)),
                        ("IX", u32::from(snapshot.ix)),
                        ("IY", u32::from(snapshot.iy)),
                        ("WZ", u32::from(snapshot.wz)),
                    ] {
                        ui.monospace(format!("{name:>4}  {value:04X}"));
                    }
                    ui.label(format!("I: {:02X}  R: {:02X}", snapshot.i, snapshot.r));
                    ui.label(format!(
                        "IFF1: {}  IFF2: {}  IM: {}  HALT: {}",
                        snapshot.iff1, snapshot.iff2, snapshot.interrupt_mode, snapshot.halted
                    ));
                    ui.label(format!(
                        "Instruction T-states: {}",
                        snapshot.instruction_tstates
                    ));
                    ui.label(format!("Machine ticks: {}", snapshot.machine_time_ticks));
                    ui.separator();
                    ui.heading("Hardware");
                    ui.label(format!("Frame: {}", snapshot.frame_number));
                    ui.label(format!(
                        "VDP status: {:02X}  scanline: {}  cycles: {}",
                        snapshot.vdp_status, snapshot.vdp_scanline, snapshot.vdp_cycles_into_line
                    ));
                    ui.label(format!(
                        "Mapper banks: {:02X}/{:02X}/{:02X}",
                        snapshot.mapper_banks[0],
                        snapshot.mapper_banks[1],
                        snapshot.mapper_banks[2]
                    ));
                    ui.label(format!(
                        "Mapper control: {:02X}  Memory control: {:02X}",
                        snapshot.mapper_control, snapshot.memory_control
                    ));
                } else {
                    ui.label("Aguardando ROM");
                }
                ui.separator();
                ui.heading("Breakpoints");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.breakpoint_input);
                    if ui
                        .add_enabled(self.worker.is_some(), egui::Button::new("Adicionar"))
                        .clicked()
                    {
                        self.add_breakpoint();
                    }
                });
                let mut remove = None;
                for address in &self.breakpoints {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("0x{address:04X}"));
                        if ui.small_button("Remover").clicked() {
                            remove = Some(*address);
                        }
                    });
                }
                if let Some(address) = remove {
                    self.remove_breakpoint(address);
                }
            });

            columns[1].vertical(|ui| {
                ui.heading("Vídeo");
                if let Some(texture) = self.texture.as_ref() {
                    ui.image((texture.id(), texture.size_vec2()));
                } else {
                    ui.label("Aguardando primeiro frame");
                }
                ui.separator();
                ui.heading("Memória");
                ui.horizontal(|ui| {
                    ui.label("Endereço hexadecimal");
                    ui.text_edit_singleline(&mut self.memory_address);
                    if ui.button("Ler").clicked() {
                        self.refresh_memory();
                    }
                });
                egui::Grid::new("sms-memory")
                    .striped(true)
                    .num_columns(9)
                    .show(ui, |ui| {
                        for (index, byte) in self.memory_bytes.iter().enumerate() {
                            if index % 8 == 0 {
                                if index > 0 {
                                    ui.end_row();
                                }
                                let start = parse_address(&self.memory_address).unwrap_or(0);
                                ui.monospace(format!("{:04X}", start.wrapping_add(index as u16)));
                            }
                            ui.monospace(format!("{byte:02X}"));
                        }
                    });
            });
        });

        if self.running {
            context.request_repaint();
        }
    }
}

impl DebuggerApp {
    fn refresh_memory_if_idle(&mut self) {
        if self.memory_bytes.is_empty() {
            self.refresh_memory();
        }
    }
}
