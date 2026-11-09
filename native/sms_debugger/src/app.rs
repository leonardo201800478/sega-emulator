//! Practical SMS debugger adapter and egui application.
#![forbid(unsafe_code)]

use eframe::egui;
use rfd::FileDialog;
use sega_debugger_core::{
    CpuSnapshot, DebugSession, DebugSnapshot, DebugTarget, FrameBuffer, HardwareValue,
    RegisterValue, RunStopReason, TraceEvent,
};
use sega_sms_core::temporal_machine::{SmsTemporalMachine, SmsTemporalTraceEvent};
use sega_z80_cpu::Z80MicroStep;
use std::{fs, path::PathBuf};

const TRACE_CAPACITY: usize = 2048;
const RUN_BUDGET: usize = 1000;

struct SmsTarget {
    machine: SmsTemporalMachine,
}

impl SmsTarget {
    fn new(rom: Vec<u8>) -> Result<Self, String> {
        let mut machine = SmsTemporalMachine::new(rom, 1).map_err(|e| e.to_string())?;
        machine.set_trace_enabled(true);
        Ok(Self { machine })
    }
}

impl DebugTarget for SmsTarget {
    fn snapshot(&self) -> DebugSnapshot {
        let cpu = self.machine.cpu().export_state();
        let r = &cpu.registers;
        DebugSnapshot {
            console: "Sega Master System".into(),
            cpu: CpuSnapshot {
                architecture: "Zilog Z80".into(),
                pc: Some(u32::from(r.pc)),
                sp: Some(u32::from(r.sp)),
                registers: vec![
                    RegisterValue {
                        name: "AF".into(),
                        value: u32::from(u16::from_be_bytes([r.a, r.f])),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "BC".into(),
                        value: u32::from(r.bc()),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "DE".into(),
                        value: u32::from(r.de()),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "HL".into(),
                        value: u32::from(r.hl()),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "IX".into(),
                        value: u32::from(cpu.ix),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "IY".into(),
                        value: u32::from(cpu.iy),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "I".into(),
                        value: u32::from(cpu.i),
                        width_bits: 8,
                    },
                    RegisterValue {
                        name: "R".into(),
                        value: u32::from(cpu.refresh),
                        width_bits: 8,
                    },
                    RegisterValue {
                        name: "WZ".into(),
                        value: u32::from(cpu.wz),
                        width_bits: 16,
                    },
                    RegisterValue {
                        name: "IFF1".into(),
                        value: u32::from(u8::from(cpu.iff1)),
                        width_bits: 1,
                    },
                    RegisterValue {
                        name: "IFF2".into(),
                        value: u32::from(u8::from(cpu.iff2)),
                        width_bits: 1,
                    },
                ],
                halted: Some(cpu.halted),
                interrupt_mode: Some(cpu.interrupt_mode),
            },
            machine_ticks: self.machine.now().ticks(),
            hardware: vec![
                HardwareValue {
                    component: "Mapper".into(),
                    name: "Sega banks".into(),
                    value: format!(
                        "{:02X}/{:02X}/{:02X}",
                        self.machine.selected_mapper_banks()[0],
                        self.machine.selected_mapper_banks()[1],
                        self.machine.selected_mapper_banks()[2]
                    ),
                },
                HardwareValue {
                    component: "Memory".into(),
                    name: "control $3E".into(),
                    value: format!("{:02X}", self.machine.memory_control()),
                },
                HardwareValue {
                    component: "VDP".into(),
                    name: "frame".into(),
                    value: self.machine.vdp().frame_number().to_string(),
                },
                HardwareValue {
                    component: "CPU".into(),
                    name: "sequencer".into(),
                    value: format!("{:?}", cpu.phase),
                },
                HardwareValue {
                    component: "CPU".into(),
                    name: "instruction T-states".into(),
                    value: cpu.instruction_tstates.to_string(),
                },
            ],
        }
    }

    fn program_counter(&self) -> Option<u32> {
        Some(u32::from(self.machine.cpu().registers.pc))
    }

    fn execute_instruction(&mut self) -> Result<(), String> {
        let step = self
            .machine
            .step_instruction(0)
            .map_err(|e| e.to_string())?;
        if matches!(step, Z80MicroStep::InterruptAcknowledge { .. }) {
            let _ = self
                .machine
                .resume_instruction(0)
                .map_err(|e| e.to_string())?;
        }
        let _ = self.machine.drain_audio_samples(None);
        Ok(())
    }

    fn read_memory(&self, address: u32, length: usize) -> Vec<u8> {
        let start = address as u16;
        (0..length.min(65536))
            .map(|i| self.machine.inspect_memory(start.wrapping_add(i as u16)))
            .collect()
    }

    fn drain_trace(&mut self) -> Vec<TraceEvent> {
        self.machine
            .take_trace()
            .into_iter()
            .map(|event| match event {
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation,
                    address,
                    base_tstates,
                    wait_tstates,
                    data,
                    ..
                } => TraceEvent {
                    timestamp_ticks: timestamp.ticks(),
                    operation: format!("{operation:?}"),
                    address: Some(u32::from(address)),
                    data,
                    wait_tstates: Some(wait_tstates),
                    detail: format!("base={base_tstates}T"),
                },
                SmsTemporalTraceEvent::InternalCycleCompleted {
                    timestamp,
                    continuation_id,
                    tstates,
                } => TraceEvent {
                    timestamp_ticks: timestamp.ticks(),
                    operation: "INTERNAL".into(),
                    address: None,
                    data: None,
                    wait_tstates: None,
                    detail: format!("continuation={continuation_id} duration={tstates}T"),
                },
                SmsTemporalTraceEvent::M1TStateCompleted {
                    timestamp,
                    phase,
                    pins,
                } => TraceEvent {
                    timestamp_ticks: timestamp.ticks(),
                    operation: "M1".into(),
                    address: None,
                    data: None,
                    wait_tstates: None,
                    detail: format!("phase={phase:?} pins={pins:?}"),
                },
            })
            .collect()
    }

    fn framebuffer(&self) -> Option<FrameBuffer> {
        Some(FrameBuffer {
            width: 256,
            height: 192,
            rgb: self.machine.framebuffer().to_vec(),
        })
    }
}

/// Native egui application for interactive SMS ROM diagnostics.
pub struct DebuggerApp {
    session: Option<DebugSession<SmsTarget>>,
    rom_path: Option<PathBuf>,
    running: bool,
    breakpoint: String,
    memory_address: String,
    status: String,
    texture: Option<egui::TextureHandle>,
}

impl Default for DebuggerApp {
    fn default() -> Self {
        Self {
            session: None,
            rom_path: None,
            running: false,
            breakpoint: "0088".into(),
            memory_address: "C000".into(),
            status: "Abra uma ROM SMS".into(),
            texture: None,
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
        match fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(SmsTarget::new)
        {
            Ok(target) => {
                self.session = Some(DebugSession::new(target, TRACE_CAPACITY));
                self.rom_path = Some(path);
                self.running = false;
                self.texture = None;
                self.status = "ROM carregada; pausada".into();
            }
            Err(error) => self.status = format!("Falha ao carregar ROM: {error}"),
        }
    }

    fn add_breakpoint(&mut self) {
        match u32::from_str_radix(self.breakpoint.trim().trim_start_matches("0x"), 16) {
            Ok(address) if address <= 0xFFFF => {
                if let Some(session) = self.session.as_mut() {
                    session.add_breakpoint(address);
                    self.status = format!("Breakpoint em 0x{address:04X}");
                }
            }
            _ => self.status = "Endereço inválido".into(),
        }
    }

    fn step(&mut self) {
        self.running = false;
        if let Some(session) = self.session.as_mut() {
            self.status = match session.step_instruction() {
                Ok(()) => "Step concluído".into(),
                Err(e) => format!("Erro: {e}"),
            };
        }
    }

    fn export(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let name = self
            .rom_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("sms");
        let Some(path) = FileDialog::new()
            .set_file_name(format!("{name}-diagnostic.txt"))
            .save_file()
        else {
            return;
        };
        self.status = match fs::write(path, session.diagnostic_report()) {
            Ok(()) => "Relatório exportado; bytes da ROM não são incluídos".into(),
            Err(e) => format!("Falha ao exportar: {e}"),
        };
    }

    fn render_video(&mut self, ui: &mut egui::Ui) {
        let Some(frame) = self
            .session
            .as_ref()
            .and_then(|session| session.framebuffer())
        else {
            ui.label("Sem vídeo");
            return;
        };
        let image = egui::ColorImage::from_rgb([frame.width, frame.height], &frame.rgb);
        if let Some(texture) = self.texture.as_mut() {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(ui.ctx().load_texture(
                "sms-debug-frame",
                image,
                egui::TextureOptions::NEAREST,
            ));
        }
        if let Some(texture) = self.texture.as_ref() {
            ui.image((texture.id(), egui::vec2(512.0, 384.0)));
        }
    }
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.running {
            if let Some(session) = self.session.as_mut() {
                match session.run_for(RUN_BUDGET) {
                    Ok(RunStopReason::Breakpoint(pc)) => {
                        self.running = false;
                        self.status = format!("Breakpoint atingido em 0x{pc:04X}");
                    }
                    Ok(RunStopReason::InstructionBudget) => {}
                    Err(e) => {
                        self.running = false;
                        self.status = format!("Erro de execução: {e}");
                    }
                }
            } else {
                self.running = false;
            }
        }

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Abrir ROM").clicked() {
                    self.open_rom();
                }
                if ui
                    .add_enabled(
                        self.session.is_some() && !self.running,
                        egui::Button::new("Run"),
                    )
                    .clicked()
                {
                    self.running = true;
                }
                if ui
                    .add_enabled(self.running, egui::Button::new("Pause"))
                    .clicked()
                {
                    self.running = false;
                }
                if ui
                    .add_enabled(self.session.is_some(), egui::Button::new("Step"))
                    .clicked()
                {
                    self.step();
                }
                if ui
                    .add_enabled(self.session.is_some(), egui::Button::new("Exportar"))
                    .clicked()
                {
                    self.export();
                }
                ui.separator();
                ui.label("Breakpoint");
                ui.text_edit_singleline(&mut self.breakpoint);
                if ui
                    .add_enabled(self.session.is_some(), egui::Button::new("Adicionar"))
                    .clicked()
                {
                    self.add_breakpoint();
                }
            });
            ui.label(&self.status);
        });

        egui::SidePanel::left("cpu")
            .default_width(230.0)
            .show(ctx, |ui| {
                ui.heading("Z80");
                if let Some(session) = self.session.as_ref() {
                    let s = session.snapshot();
                    ui.monospace(format!("PC  {:04X}", s.cpu.pc.unwrap_or_default()));
                    ui.monospace(format!("SP  {:04X}", s.cpu.sp.unwrap_or_default()));
                    for r in &s.cpu.registers {
                        ui.monospace(format!("{:>5}  {:04X}", r.name, r.value));
                    }
                    ui.separator();
                    ui.label(format!("HALT {:?}", s.cpu.halted));
                    ui.label(format!("IM {:?}", s.cpu.interrupt_mode));
                    ui.label(format!("Machine ticks {}", s.machine_ticks));
                    ui.separator();
                    ui.heading("Hardware");
                    for h in &s.hardware {
                        ui.label(format!("{} / {}", h.component, h.name));
                        ui.monospace(&h.value);
                    }
                    ui.separator();
                    ui.heading("Breakpoints");
                    for bp in session.breakpoints() {
                        ui.monospace(format!("{bp:04X}"));
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Vídeo");
            self.render_video(ui);
            ui.separator();
            ui.horizontal(|ui| {
                ui.heading("RAM / memória");
                ui.label("Endereço hex");
                ui.text_edit_singleline(&mut self.memory_address);
            });
            if let Some(session) = self.session.as_ref() {
                let address =
                    u32::from_str_radix(self.memory_address.trim().trim_start_matches("0x"), 16)
                        .unwrap_or(0)
                        .min(0xFFFF);
                let bytes = session.read_memory(address, 64);
                egui::Grid::new("memory")
                    .striped(true)
                    .num_columns(9)
                    .show(ui, |ui| {
                        for (i, b) in bytes.iter().enumerate() {
                            if i % 8 == 0 {
                                if i > 0 {
                                    ui.end_row();
                                }
                                ui.monospace(format!("{:04X}", address + i as u32));
                            }
                            ui.monospace(format!("{b:02X}"));
                        }
                    });
                ui.separator();
                ui.heading("Trace do barramento");
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for e in session.trace().iter().rev().take(24).rev() {
                            let a = e
                                .address
                                .map_or_else(|| "----".into(), |v| format!("{v:04X}"));
                            let d = e.data.map_or_else(|| "--".into(), |v| format!("{v:02X}"));
                            ui.monospace(format!(
                                "{:>12} {:<12} {a} {d} {}",
                                e.timestamp_ticks, e.operation, e.detail
                            ));
                        }
                    });
            }
        });
        if self.running {
            ctx.request_repaint();
        }
    }
}
