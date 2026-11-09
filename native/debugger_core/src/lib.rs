//! Small, console-independent debugging primitives shared by Sega targets.
//!
//! Console runtimes implement DebugTarget. The session owns breakpoints and a
//! bounded trace buffer; it never owns or advances hardware clocks itself.

use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterValue {
    pub name: String,
    pub value: u32,
    pub width_bits: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSnapshot {
    pub architecture: String,
    pub pc: Option<u32>,
    pub sp: Option<u32>,
    pub registers: Vec<RegisterValue>,
    pub halted: Option<bool>,
    pub interrupt_mode: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareValue {
    pub component: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSnapshot {
    pub console: String,
    pub cpu: CpuSnapshot,
    pub machine_ticks: u128,
    pub hardware: Vec<HardwareValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub timestamp_ticks: u128,
    pub operation: String,
    pub address: Option<u32>,
    pub data: Option<u8>,
    pub wait_tstates: Option<u32>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    /// Packed RGB888 pixels, row-major.
    pub rgb: Vec<u8>,
}

/// Adapter implemented by each console runtime. Methods are called by the
/// debugger UI thread through coarse-grained operations, never per T-state.
pub trait DebugTarget {
    fn snapshot(&self) -> DebugSnapshot;
    fn execute_instruction(&mut self) -> Result<(), String>;
    fn read_memory(&self, address: u32, length: usize) -> Vec<u8>;
    fn drain_trace(&mut self) -> Vec<TraceEvent>;
    fn framebuffer(&self) -> Option<FrameBuffer>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStopReason {
    InstructionBudget,
    Breakpoint(u32),
}

pub struct DebugSession<T: DebugTarget> {
    target: T,
    breakpoints: BTreeSet<u32>,
    trace: VecDeque<TraceEvent>,
    trace_capacity: usize,
}

impl<T: DebugTarget> DebugSession<T> {
    pub fn new(target: T, trace_capacity: usize) -> Self {
        Self {
            target,
            breakpoints: BTreeSet::new(),
            trace: VecDeque::with_capacity(trace_capacity),
            trace_capacity,
        }
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    pub fn snapshot(&self) -> DebugSnapshot {
        self.target.snapshot()
    }

    pub fn breakpoints(&self) -> &BTreeSet<u32> {
        &self.breakpoints
    }

    pub fn add_breakpoint(&mut self, address: u32) -> bool {
        self.breakpoints.insert(address)
    }

    pub fn remove_breakpoint(&mut self, address: u32) -> bool {
        self.breakpoints.remove(&address)
    }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    pub fn trace(&self) -> &VecDeque<TraceEvent> {
        &self.trace
    }

    pub fn read_memory(&self, address: u32, length: usize) -> Vec<u8> {
        self.target.read_memory(address, length)
    }

    pub fn framebuffer(&self) -> Option<FrameBuffer> {
        self.target.framebuffer()
    }

    /// Execute exactly one instruction and collect any newly emitted bus events.
    pub fn step_instruction(&mut self) -> Result<(), String> {
        self.target.execute_instruction()?;
        self.collect_trace();
        Ok(())
    }

    /// Execute up to the requested instruction budget, stopping before a
    /// breakpoint address is executed. Hardware time advances only in target.
    pub fn run_for(&mut self, instruction_budget: usize) -> Result<RunStopReason, String> {
        for _ in 0..instruction_budget {
            let pc = self.target.snapshot().cpu.pc.unwrap_or_default();
            if self.breakpoints.contains(&pc) {
                self.collect_trace();
                return Ok(RunStopReason::Breakpoint(pc));
            }
            self.target.execute_instruction()?;
            self.collect_trace();
        }
        let pc = self.target.snapshot().cpu.pc.unwrap_or_default();
        if self.breakpoints.contains(&pc) {
            Ok(RunStopReason::Breakpoint(pc))
        } else {
            Ok(RunStopReason::InstructionBudget)
        }
    }

    fn collect_trace(&mut self) {
        for event in self.target.drain_trace() {
            if self.trace_capacity == 0 {
                continue;
            }
            while self.trace.len() >= self.trace_capacity {
                self.trace.pop_front();
            }
            self.trace.push_back(event);
        }
    }

    /// Produce a concise, ROM-content-free diagnostic report suitable for a
    /// local text file. Instruction bytes and cartridge data are never exported.
    pub fn diagnostic_report(&self) -> String {
        let snapshot = self.snapshot();
        let mut output = String::new();
        let _ = writeln!(output, "SEGA DEBUG SESSION");
        let _ = writeln!(output, "Console: {}", snapshot.console);
        let _ = writeln!(output, "CPU: {}", snapshot.cpu.architecture);
        let _ = writeln!(output, "Machine ticks: {}", snapshot.machine_ticks);
        if let Some(pc) = snapshot.cpu.pc {
            let _ = writeln!(output, "PC: {pc:04X}");
        }
        if let Some(sp) = snapshot.cpu.sp {
            let _ = writeln!(output, "SP: {sp:04X}");
        }
        for register in &snapshot.cpu.registers {
            let digits = usize::from(register.width_bits).div_ceil(4);
            let _ = writeln!(
                output,
                "{}: {:0digits$X}",
                register.name,
                register.value,
                digits = digits
            );
        }
        if let Some(halted) = snapshot.cpu.halted {
            let _ = writeln!(output, "HALT: {halted}");
        }
        if let Some(mode) = snapshot.cpu.interrupt_mode {
            let _ = writeln!(output, "Interrupt mode: {mode}");
        }
        for item in &snapshot.hardware {
            let _ = writeln!(output, "{}.{}: {}", item.component, item.name, item.value);
        }
        let _ = writeln!(output, "Breakpoints: {:?}", self.breakpoints);
        let _ = writeln!(output, "TRACE (oldest first, max {} events)", self.trace_capacity);
        for event in &self.trace {
            let address = event.address.map_or_else(|| "----".to_owned(), |v| format!("{v:04X}"));
            let data = event.data.map_or_else(|| "--".to_owned(), |v| format!("{v:02X}"));
            let _ = writeln!(
                output,
                "{} {} {} {} wait={} {}",
                event.timestamp_ticks,
                event.operation,
                address,
                data,
                event.wait_tstates.map_or_else(|| "-".to_owned(), |v| v.to_string()),
                event.detail
            );
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeTarget {
        pc: u32,
        trace: Vec<TraceEvent>,
    }

    impl DebugTarget for FakeTarget {
        fn snapshot(&self) -> DebugSnapshot {
            DebugSnapshot {
                console: "test".to_owned(),
                cpu: CpuSnapshot {
                    architecture: "test cpu".to_owned(),
                    pc: Some(self.pc),
                    sp: None,
                    registers: Vec::new(),
                    halted: None,
                    interrupt_mode: None,
                },
                machine_ticks: u128::from(self.pc),
                hardware: Vec::new(),
            }
        }

        fn execute_instruction(&mut self) -> Result<(), String> {
            self.pc += 1;
            self.trace.push(TraceEvent {
                timestamp_ticks: u128::from(self.pc),
                operation: "READ".to_owned(),
                address: Some(self.pc),
                data: Some(0),
                wait_tstates: Some(0),
                detail: String::new(),
            });
            Ok(())
        }

        fn read_memory(&self, _address: u32, length: usize) -> Vec<u8> {
            vec![0; length]
        }

        fn drain_trace(&mut self) -> Vec<TraceEvent> {
            std::mem::take(&mut self.trace)
        }

        fn framebuffer(&self) -> Option<FrameBuffer> {
            None
        }
    }

    #[test]
    fn run_stops_before_executing_breakpoint_address() {
        let target = FakeTarget { pc: 0, trace: Vec::new() };
        let mut session = DebugSession::new(target, 8);
        session.add_breakpoint(3);
        assert_eq!(session.run_for(10).unwrap(), RunStopReason::Breakpoint(3));
        assert_eq!(session.snapshot().cpu.pc, Some(3));
    }

    #[test]
    fn trace_buffer_discards_oldest_events_at_capacity() {
        let target = FakeTarget { pc: 0, trace: Vec::new() };
        let mut session = DebugSession::new(target, 2);
        session.run_for(4).unwrap();
        assert_eq!(session.trace().len(), 2);
        assert_eq!(session.trace().front().unwrap().timestamp_ticks, 3);
        assert_eq!(session.trace().back().unwrap().timestamp_ticks, 4);
    }

    #[test]
    fn zero_capacity_trace_does_not_retain_events() {
        let target = FakeTarget { pc: 0, trace: Vec::new() };
        let mut session = DebugSession::new(target, 0);
        session.step_instruction().unwrap();
        assert!(session.trace().is_empty());
    }
}
