use sega_sms_core::{CpuBus, Z80, Z80InstructionTrace};

struct DiagnosticBus {
    memory: Box<[u8]>,
    traces: Vec<Z80InstructionTrace>,
    enabled: bool,
}

impl Default for DiagnosticBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            traces: Vec::new(),
            enabled: true,
        }
    }
}

impl CpuBus for DiagnosticBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    fn instruction_trace_enabled(&self) -> bool {
        self.enabled
    }

    fn observe_instruction_trace(&mut self, trace: Z80InstructionTrace) {
        self.traces.push(trace);
    }
}

#[test]
fn instruction_trace_captures_opcode_register_state_and_aggregate_cycles() {
    let mut bus = DiagnosticBus::default();
    bus.memory[..2].copy_from_slice(&[0x3E, 0x42]);
    let mut cpu = Z80::default();

    assert_eq!(cpu.step(&mut bus), Ok(7));
    assert_eq!(bus.traces.len(), 1);

    let trace = bus.traces[0];
    assert_eq!(trace.instruction_index, 1);
    assert_eq!(trace.opcode_bytes[..usize::from(trace.opcode_len)], [0x3E]);
    assert!(!trace.opcode_bytes_truncated);
    assert_eq!(trace.tstates, Some(7));
    assert_eq!(trace.before.pc, 0);
    assert_eq!(trace.before.total_tstates, 0);
    assert_eq!(trace.after.pc, 2);
    assert_eq!(trace.after.af >> 8, 0x42);
    assert_eq!(trace.after.total_tstates, 7);
}

#[test]
fn instruction_trace_records_prefix_bytes_in_decoder_order() {
    let mut bus = DiagnosticBus::default();
    bus.memory[..2].copy_from_slice(&[0xED, 0xA0]);
    bus.memory[0x100] = 0x5A;

    let mut cpu = Z80::default();
    cpu.registers.set_hl(0x100);
    cpu.registers.set_de(0x200);
    cpu.registers.set_bc(1);

    assert_eq!(cpu.step(&mut bus), Ok(16));
    let trace = bus.traces[0];
    assert_eq!(trace.opcode_bytes[..usize::from(trace.opcode_len)], [0xED, 0xA0]);
    assert_eq!(bus.memory[0x200], 0x5A);
    assert_eq!(trace.after.hl, 0x101);
    assert_eq!(trace.after.de, 0x201);
    assert_eq!(trace.after.bc, 0);
}

#[test]
fn instruction_trace_is_opt_in_and_does_not_change_execution() {
    let mut bus = DiagnosticBus::default();
    bus.enabled = false;
    bus.memory[0] = 0x00;
    let mut cpu = Z80::default();

    assert_eq!(cpu.step(&mut bus), Ok(4));
    assert!(bus.traces.is_empty());
    assert_eq!(cpu.registers.pc, 1);
    assert_eq!(cpu.total_tstates, 4);
}
