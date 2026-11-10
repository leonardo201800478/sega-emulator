//! SMS debugger native-window state and presentation.

use std::time::{Duration, Instant};

use crate::icons::{IconKind, icon_button};
use eframe::egui;
use sega_sms_runtime::{SmsDebugSnapshot, SmsTemporalWorkerDebugHandle};

/// Mutable presentation state for the independent debugger viewport.
pub(crate) struct DebuggerWindowState {
    pub(crate) breakpoint_input: String,
    pub(crate) breakpoints: Vec<u16>,
    pub(crate) breakpoint_rom_path: String,
    pub(crate) memory_address: String,
    pub(crate) memory_bytes: Vec<u8>,
    pub(crate) snapshot: Option<SmsDebugSnapshot>,
    pub(crate) last_refresh: Instant,
    pub(crate) status: String,
    pub(crate) trace_capture_enabled: bool,
    pub(crate) window_position: Option<egui::Pos2>,
    pub(crate) window_size: Option<egui::Vec2>,
    pub(crate) window_initialized: bool,
}

impl Default for DebuggerWindowState {
    fn default() -> Self {
        Self {
            breakpoint_input: "0088".to_owned(),
            breakpoints: Vec::new(),
            breakpoint_rom_path: String::new(),
            memory_address: "C000".to_owned(),
            memory_bytes: Vec::new(),
            snapshot: None,
            last_refresh: Instant::now(),
            status: "Aguardando máquina SMS".to_owned(),
            trace_capture_enabled: false,
            window_position: None,
            window_size: None,
            window_initialized: false,
        }
    }
}

pub(crate) fn parse_debug_address(value: &str) -> Option<u16> {
    u16::from_str_radix(
        value
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X"),
        16,
    )
    .ok()
}

pub(crate) fn parse_saved_position(value: &str) -> Option<egui::Pos2> {
    let (x, y) = value.split_once(',')?;
    let p = egui::pos2(x.parse().ok()?, y.parse().ok()?);
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

pub(crate) fn parse_saved_root_size(value: &str) -> Option<egui::Vec2> {
    let (x, y) = value.split_once(',')?;
    let size = egui::vec2(x.parse().ok()?, y.parse().ok()?);
    (size.x.is_finite() && size.y.is_finite() && size.x >= 1024.0 && size.y >= 720.0)
        .then_some(size)
}

pub(crate) fn parse_saved_size(value: &str) -> Option<egui::Vec2> {
    let (x, y) = value.split_once(',')?;
    let size = egui::vec2(x.parse().ok()?, y.parse().ok()?);
    (size.x.is_finite() && size.y.is_finite() && size.x >= 760.0 && size.y >= 560.0).then_some(size)
}

pub(crate) fn render_debugger_contents(
    ui: &mut egui::Ui,
    worker: Option<&SmsTemporalWorkerDebugHandle>,
    state: &mut DebuggerWindowState,
) {
    if let Some(worker) = worker {
        if state.last_refresh.elapsed() >= Duration::from_millis(100) {
            match worker.debug_snapshot() {
                Ok(snapshot) => {
                    state.snapshot = Some(snapshot);
                    (if worker.is_running() {
                        "Máquina em execução"
                    } else {
                        "Máquina pausada"
                    })
                    .clone_into(&mut state.status);
                }
                Err(error) => {
                    state.status = format!("Falha ao consultar runtime: {error}");
                }
            }
            state.last_refresh = Instant::now();
        }
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }

    ui.horizontal_wrapped(|ui| {
        ui.heading("SMS Debugger");
        ui.separator();
        ui.label(&state.status);
    });

    let Some(worker) = worker else {
        ui.centered_and_justified(|ui| {
            ui.label("Carregue uma ROM do Master System para habilitar o debugger.");
        });
        return;
    };

    render_debugger_toolbar(ui, worker, state);

    if let Some(snapshot) = state.snapshot.as_ref() {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Frame: {}", snapshot.frame_number));
            ui.label(format!("Machine ticks: {}", snapshot.machine_time_ticks));
            ui.label(format!("T-states: {}", snapshot.instruction_tstates));
            ui.label(format!("HALT: {}", snapshot.halted));
        });
        render_debugger_registers(ui, snapshot);
        render_debugger_video_state(ui, snapshot);
    }

    ui.separator();
    ui.columns(2, |columns| {
        render_debugger_breakpoints(&mut columns[0], worker, state);
        render_debugger_memory(&mut columns[1], worker, state);
    });
    render_debugger_trace(ui, state.snapshot.as_ref());
}

fn render_debugger_toolbar(
    ui: &mut egui::Ui,
    worker: &SmsTemporalWorkerDebugHandle,
    state: &mut DebuggerWindowState,
) {
    ui.horizontal_wrapped(|ui| {
        let running = worker.is_running();
        let run_icon = if running {
            IconKind::Pause
        } else {
            IconKind::Play
        };
        let run_tooltip = if running { "Pausar" } else { "Executar" };
        if icon_button(ui, run_icon, run_tooltip, true).clicked() {
            let result = if running {
                worker.pause()
            } else {
                worker.start()
            };
            if let Err(error) = result {
                state.status = format!("Falha ao alterar execução: {error}");
            }
        }

        if icon_button(ui, IconKind::Step, "Executar uma instrução", !running).clicked() {
            match worker.step_instruction() {
                Ok(()) => {
                    if let Ok(snapshot) = worker.debug_snapshot() {
                        state.snapshot = Some(snapshot);
                    }
                    "Instrução concluída no worker".clone_into(&mut state.status);
                }
                Err(error) => {
                    state.status = format!("Falha no step: {error}");
                }
            }
        }

        let trace_enabled = state.trace_capture_enabled;
        let trace_tooltip = if trace_enabled {
            "Desativar trace"
        } else {
            "Ativar trace"
        };
        if icon_button(ui, IconKind::Trace, trace_tooltip, true).clicked() {
            match worker.set_trace_enabled(!trace_enabled) {
                Ok(()) => {
                    state.trace_capture_enabled = !trace_enabled;
                    if let Ok(snapshot) = worker.debug_snapshot() {
                        state.snapshot = Some(snapshot);
                    }
                }
                Err(error) => {
                    state.status = format!("Falha ao configurar trace: {error}");
                }
            }
        }
    });
}

fn render_debugger_registers(ui: &mut egui::Ui, snapshot: &SmsDebugSnapshot) {
    egui::CollapsingHeader::new("Z80 — registradores")
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("sms_debugger_registers")
                .num_columns(4)
                .show(ui, |ui| {
                    for (name, value) in [
                        ("AF", snapshot.af),
                        ("BC", snapshot.bc),
                        ("DE", snapshot.de),
                        ("HL", snapshot.hl),
                        ("AF'", snapshot.af_alt),
                        ("BC'", snapshot.bc_alt),
                        ("DE'", snapshot.de_alt),
                        ("HL'", snapshot.hl_alt),
                        ("IX", snapshot.ix),
                        ("IY", snapshot.iy),
                        ("SP", snapshot.sp),
                        ("PC", snapshot.pc),
                        ("WZ", snapshot.wz),
                        ("I", u16::from(snapshot.i)),
                        ("R", u16::from(snapshot.r)),
                        ("IM", u16::from(snapshot.interrupt_mode)),
                    ] {
                        ui.strong(name);
                        ui.monospace(format!("{value:04X}"));
                        if matches!(name, "HL" | "HL'" | "PC" | "IM") {
                            ui.end_row();
                        }
                    }
                });
            ui.label(format!("IFF1: {} | IFF2: {}", snapshot.iff1, snapshot.iff2));
        });
}

fn render_debugger_video_state(ui: &mut egui::Ui, snapshot: &SmsDebugSnapshot) {
    egui::CollapsingHeader::new("VDP / mapper")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(format!(
                "VDP status: {:02X} | Scanline: {} | Cycles/line: {}",
                snapshot.vdp_status, snapshot.vdp_scanline, snapshot.vdp_cycles_into_line
            ));
            ui.label(format!(
                "Mapper banks: {:02X} / {:02X} / {:02X} | Control: {:02X} | Memory: {:02X}",
                snapshot.mapper_banks[0],
                snapshot.mapper_banks[1],
                snapshot.mapper_banks[2],
                snapshot.mapper_control,
                snapshot.memory_control
            ));
        });
}

fn render_debugger_breakpoints(
    ui: &mut egui::Ui,
    worker: &SmsTemporalWorkerDebugHandle,
    state: &mut DebuggerWindowState,
) {
    ui.heading("Breakpoints");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut state.breakpoint_input).desired_width(72.0));
        if icon_button(ui, IconKind::Add, "Adicionar breakpoint", true).clicked() {
            if let Some(address) = parse_debug_address(&state.breakpoint_input) {
                if !state.breakpoints.contains(&address) && state.breakpoints.len() < 64 {
                    match worker.set_breakpoint(address, true) {
                        Ok(()) => state.breakpoints.push(address),
                        Err(error) => {
                            state.status = format!("Falha no breakpoint: {error}");
                        }
                    }
                }
            } else {
                "Endereço de breakpoint inválido".clone_into(&mut state.status);
            }
        }
    });

    let mut remove = None;
    for address in &state.breakpoints {
        ui.horizontal(|ui| {
            let _ = icon_button(ui, IconKind::Breakpoint, "Breakpoint ativo", false);
            ui.monospace(format!("{address:04X}"));
            if icon_button(ui, IconKind::Remove, "Remover breakpoint", true).clicked() {
                remove = Some(*address);
            }
        });
    }

    if let Some(address) = remove {
        match worker.set_breakpoint(address, false) {
            Ok(()) => state.breakpoints.retain(|item| *item != address),
            Err(error) => {
                state.status = format!("Falha ao remover breakpoint: {error}");
            }
        }
    }

    if ui
        .add_enabled(
            !state.breakpoints.is_empty(),
            egui::Button::new("Limpar todos"),
        )
        .clicked()
    {
        for address in state.breakpoints.clone() {
            if let Err(error) = worker.set_breakpoint(address, false) {
                state.status = format!("Falha ao limpar breakpoints: {error}");
                return;
            }
        }
        state.breakpoints.clear();
    }
}

fn render_debugger_memory(
    ui: &mut egui::Ui,
    worker: &SmsTemporalWorkerDebugHandle,
    state: &mut DebuggerWindowState,
) {
    ui.heading("Inspeção de memória");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut state.memory_address).desired_width(72.0));
        if ui.button("Ler 64 bytes").clicked() {
            if let Some(address) = parse_debug_address(&state.memory_address) {
                match worker.inspect_memory(address, 64) {
                    Ok(bytes) => state.memory_bytes = bytes,
                    Err(error) => {
                        state.status = format!("Falha ao ler memória: {error}");
                    }
                }
            } else {
                "Endereço de memória inválido".clone_into(&mut state.status);
            }
        }
    });

    egui::ScrollArea::vertical()
        .max_height(180.0)
        .show(ui, |ui| {
            for (row, bytes) in state.memory_bytes.chunks(16).enumerate() {
                let address = parse_debug_address(&state.memory_address)
                    .unwrap_or_default()
                    .wrapping_add(u16::try_from(row * 16).unwrap_or(0));
                let hex = bytes
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ascii = bytes
                    .iter()
                    .map(|byte| {
                        if (0x20..=0x7e).contains(byte) {
                            char::from(*byte)
                        } else {
                            '.'
                        }
                    })
                    .collect::<String>();
                ui.monospace(format!("{address:04X}: {hex:<47} {ascii}"));
            }
        });
}

fn render_debugger_trace(ui: &mut egui::Ui, snapshot: Option<&SmsDebugSnapshot>) {
    ui.separator();
    egui::CollapsingHeader::new("Trace do Scheduler / barramento")
        .default_open(false)
        .show(ui, |ui| {
            if let Some(snapshot) = snapshot {
                ui.label(format!(
                    "{} eventos retidos | {} descartados | captura {}",
                    snapshot.trace_events.len(),
                    snapshot.trace_dropped,
                    if snapshot.trace_enabled {
                        "ativa"
                    } else {
                        "inativa"
                    }
                ));
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for event in &snapshot.trace_events {
                            ui.monospace(format!("{event:?}"));
                        }
                    });
            }
        });
}
