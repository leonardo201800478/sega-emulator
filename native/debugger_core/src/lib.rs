//! Small, console-independent debugging primitives shared by Sega targets.
//!
//! Console runtimes implement DebugTarget. The session owns breakpoints and a
//! bounded trace buffer; it never owns or advances hardware clocks itself.

use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;

/// A named CPU register and its current unsigned value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterValue {
    /// Register display name, such as AF, PC or IX.
    pub name: String,
    /// Register value, stored in the low bits.
    pub value: u32,
    /// Architectural register width in bits.
    pub width_bits: u8,
}

/// Architecture-neutral view of the processor state at a debugger stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSnapshot {
    /// Human-readable CPU architecture name.
    pub architecture: String,
    /// Current program counter, when the target exposes one.
    pub pc: Option<u32>,
    /// Current stack pointer, when the target exposes one.
    pub sp: Option<u32>,
    /// Registers selected by the target for its diagnostic view.
    pub registers: Vec<RegisterValue>,
    /// HALT state, when meaningful for the selected CPU.
    pub halted: Option<bool>,
    /// Interrupt mode, when meaningful for the selected CPU.
    pub interrupt_mode: Option<u8>,
}

/// One human-readable hardware register or status value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareValue {
    /// Hardware component name, for example VDP or cartridge mapper.
    pub component: String,
    /// Register or status field name.
    pub name: String,
    /// Formatted value suitable for display and text reports.
    pub value: String,
}

/// Cross-console snapshot used by debugger views and diagnostic reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSnapshot {
    /// Console system name.
    pub console: String,
    /// Processor state.
    pub cpu: CpuSnapshot,
    /// Absolute machine time in the target's master-clock ticks.
    pub machine_ticks: u128,
    /// Hardware-specific state exposed by the target adapter.
    pub hardware: Vec<HardwareValue>,
}

/// Normalized bus or internal-cycle event retained by the debugger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// Absolute timestamp in machine master-clock ticks.
    pub timestamp_ticks: u128,
    /// Operation label, such as OpcodeFetch, MemoryWrite or INTERNAL.
    pub operation: String,
    /// Bus address, when the event has one.
    pub address: Option<u32>,
    /// Data transferred, when the event has one.
    pub data: Option<u8>,
    /// Wait-state count, when applicable.
    pub wait_tstates: Option<u32>,
    /// Additional target-specific details.
    pub detail: String,
}

/// Optional rendered output from a target, in packed RGB888 format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBuffer {
    /// Pixel width.
    pub width: usize,
    /// Pixel height.
    pub height: usize,
    /// Packed RGB888 pixels, row-major.
    pub rgb: Vec<u8>,
}

/// Adapter implemented by each console runtime. Methods are called by the
/// debugger UI thread through coarse-grained operations, never per T-state.
pub trait DebugTarget {
    /// Return the current CPU and hardware snapshot.
    fn snapshot(&self) -> DebugSnapshot;

    /// Return the current PC without constructing a full snapshot when possible.
    fn program_counter(&self) -> Option<u32> {
        self.snapshot().cpu.pc
    }

    /// Execute exactly one architectural instruction, including its temporal bus work.
    ///
    /// # Errors
    /// Returns an error when the target cannot complete the instruction.
    fn execute_instruction(&mut self) -> Result<(), String>;

    /// Read a range through the target's active memory map.
    fn read_memory(&self, address: u32, length: usize) -> Vec<u8>;

    /// Drain trace events accumulated since the previous call.
    fn drain_trace(&mut self) -> Vec<TraceEvent>;

    /// Return the latest rendered frame when the console has video output.
    fn framebuffer(&self) -> Option<FrameBuffer>;
}

/// Reason why a bounded run operation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStopReason {
    /// The requested instruction budget was exhausted.
    InstructionBudget,
    /// Execution stopped before the instruction at this PC.
    Breakpoint(u32),
}

/// Stateful debugger session shared by console-specific frontends.
///
/// The session owns only breakpoints and a bounded diagnostic trace. Hardware
/// state and timing remain exclusively owned by the target implementation.
pub struct DebugSession<T: DebugTarget> {
    target: T,
    breakpoints: BTreeSet<u32>,
    trace: VecDeque<TraceEvent>,
    trace_capacity: usize,
}

impl<T: DebugTarget> DebugSession<T> {
    /// Create a session with a maximum retained trace-event count.
    pub fn new(target: T, trace_capacity: usize) -> Self {
        Self {
            target,
            breakpoints: BTreeSet::new(),
            trace: VecDeque::with_capacity(trace_capacity),
            trace_capacity,
        }
    }

    /// Borrow the console-specific target.
    pub fn target(&self) -> &T {
        &self.target
    }

    /// Return the current cross-console snapshot.
    pub fn snapshot(&self) -> DebugSnapshot {
        self.target.snapshot()
    }

    /// Return all configured program-counter breakpoints.
    pub fn breakpoints(&self) -> &BTreeSet<u32> {
        &self.breakpoints
    }

    /// Add a breakpoint; return false when it was already present.
    pub fn add_breakpoint(&mut self, address: u32) -> bool {
        self.breakpoints.insert(address)
    }

    /// Remove a breakpoint; return false when it was not present.
    pub fn remove_breakpoint(&mut self, address: u32) -> bool {
        self.breakpoints.remove(&address)
    }

    /// Remove every configured breakpoint.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Borrow the retained trace in chronological order.
    pub fn trace(&self) -> &VecDeque<TraceEvent> {
        &self.trace
    }

    /// Read a range from the target's active memory map.
    pub fn read_memory(&self, address: u32, length: usize) -> Vec<u8> {
        self.target.read_memory(address, length)
    }

    /// Return the target's latest rendered frame, if available.
    pub fn framebuffer(&self) -> Option<FrameBuffer> {
        self.target.framebuffer()
    }

    /// Execute exactly one instruction and collect newly emitted bus events.
    ///
    /// # Errors
    /// Returns an error propagated by the console-specific target.
    pub fn step_instruction(&mut self) -> Result<(), String> {
        self.target.execute_instruction()?;
        self.collect_trace();
        Ok(())
    }

    /// Execute up to the requested instruction budget, stopping before a
    /// breakpoint address is executed. Hardware time advances only in target.
    ///
    /// # Errors
    /// Returns an error propagated by the console-specific target.
    pub fn run_for(&mut self, instruction_budget: usize) -> Result<RunStopReason, String> {
        for _ in 0..instruction_budget {
            let pc = self.target.program_counter().unwrap_or_default();
            if self.breakpoints.contains(&pc) {
                self.collect_trace();
                return Ok(RunStopReason::Breakpoint(pc));
            }
            self.target.execute_instruction()?;
            self.collect_trace();
        }
        let pc = self.target.program_counter().unwrap_or_default();
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

    /// Produce a concise, ROM-content-free report suitable for a local text file.
    ///
    /// Instruction bytes and cartridge data are never exported.
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
        let _ = writeln!(
            output,
            "TRACE (oldest first, max {} events)",
            self.trace_capacity
        );
        for event in &self.trace {
            let address = event
                .address
                .map_or_else(|| "----".to_owned(), |v| format!("{v:04X}"));
            let cartridge_access = event.operation == "OpcodeFetch"
                || event.address.is_some_and(|address| address < 0xC000);
            let data = if cartridge_access {
                "--".to_owned()
            } else {
                event
                    .data
                    .map_or_else(|| "--".to_owned(), |v| format!("{v:02X}"))
            };
            let _ = writeln!(
                output,
                "{} {} {} {} wait={} {}",
                event.timestamp_ticks,
                event.operation,
                address,
                data,
                event
                    .wait_tstates
                    .map_or_else(|| "-".to_owned(), |v| v.to_string()),
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
                data: Some(0xA5),
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
        let target = FakeTarget {
            pc: 0,
            trace: Vec::new(),
        };
        let mut session = DebugSession::new(target, 8);
        session.add_breakpoint(3);
        assert_eq!(session.run_for(10).unwrap(), RunStopReason::Breakpoint(3));
        assert_eq!(session.snapshot().cpu.pc, Some(3));
    }

    #[test]
    fn trace_buffer_discards_oldest_events_at_capacity() {
        let target = FakeTarget {
            pc: 0,
            trace: Vec::new(),
        };
        let mut session = DebugSession::new(target, 2);
        session.run_for(4).unwrap();
        assert_eq!(session.trace().len(), 2);
        assert_eq!(session.trace().front().unwrap().timestamp_ticks, 3);
        assert_eq!(session.trace().back().unwrap().timestamp_ticks, 4);
    }

    #[test]
    fn diagnostic_report_redacts_cartridge_bytes_and_preserves_ram_diagnostics() {
        let target = FakeTarget {
            pc: 0x1233,
            trace: Vec::new(),
        };
        let mut session = DebugSession::new(target, 8);
        session.step_instruction().unwrap();
        let report = session.diagnostic_report();
        assert!(report.contains("READ 1234 --"));
        assert!(!report.contains("1234 A5"));

        let target = FakeTarget {
            pc: 0xC000,
            trace: Vec::new(),
        };
        let mut session = DebugSession::new(target, 8);
        session.step_instruction().unwrap();
        let report = session.diagnostic_report();
        assert!(report.contains("READ C001 A5"));
    }

    #[test]
    fn zero_capacity_trace_does_not_retain_events() {
        let target = FakeTarget {
            pc: 0,
            trace: Vec::new(),
        };
        let mut session = DebugSession::new(target, 0);
        session.step_instruction().unwrap();
        assert!(session.trace().is_empty());
    }
}
