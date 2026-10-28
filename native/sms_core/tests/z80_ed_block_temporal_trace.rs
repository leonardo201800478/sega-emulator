use sega_sms_core::{CpuBus, Z80, Z80BusEvent, Z80BusEventKind};

struct TraceBus {
    memory: Box<[u8]>,
    events: Vec<Z80BusEvent>,
}

impl Default for TraceBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            events: Vec::new(),
        }
    }
}

impl CpuBus for TraceBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    fn observe_timing_event(&mut self, event: Z80BusEvent) {
        self.events.push(event);
    }
}

fn event(
    instruction_pc: u16,
    tstate_offset: u32,
    kind: Z80BusEventKind,
    address: Option<u16>,
    data: Option<u8>,
    duration_tstates: u8,
) -> Z80BusEvent {
    Z80BusEvent {
        instruction_pc,
        tstate_offset,
        kind,
        address,
        data,
        duration_tstates,
    }
}

fn run_trace(
    opcode: u8,
    a: u8,
    hl: u16,
    de: u16,
    bc: u16,
    value: u8,
) -> (u32, u16, Vec<Z80BusEvent>) {
    let mut bus = TraceBus::default();
    bus.memory[0] = 0xED;
    bus.memory[1] = opcode;
    bus.memory[usize::from(hl)] = value;

    let mut cpu = Z80::default();
    cpu.registers.a = a;
    cpu.registers.pc = 0;
    cpu.registers.set_hl(hl);
    cpu.registers.set_de(de);
    cpu.registers.set_bc(bc);
    let cycles = cpu.step(&mut bus).expect("ED block instruction executes");
    (cycles, cpu.registers.pc, bus.events)
}

fn fetch_events(opcode: u8) -> [Z80BusEvent; 2] {
    [
        event(0, 0, Z80BusEventKind::OpcodeFetch, Some(0), Some(0xED), 4),
        event(0, 4, Z80BusEventKind::OpcodeFetch, Some(1), Some(opcode), 4),
    ]
}

fn expected_transfer_trace(
    opcode: u8,
    hl: u16,
    de: u16,
    value: u8,
    repeat: bool,
) -> Vec<Z80BusEvent> {
    let mut expected = fetch_events(opcode).to_vec();
    expected.push(event(
        0,
        8,
        Z80BusEventKind::MemoryRead,
        Some(hl),
        Some(value),
        3,
    ));
    expected.push(event(
        0,
        11,
        Z80BusEventKind::MemoryWrite,
        Some(de),
        Some(value),
        5,
    ));
    if repeat {
        expected.push(event(0, 16, Z80BusEventKind::Internal, None, None, 5));
    }
    expected
}

fn expected_compare_trace(opcode: u8, hl: u16, value: u8, repeat: bool) -> Vec<Z80BusEvent> {
    let mut expected = fetch_events(opcode).to_vec();
    expected.push(event(0, 8, Z80BusEventKind::MemoryRead, Some(hl), Some(value), 3));
    expected.push(event(0, 11, Z80BusEventKind::Internal, None, None, 5));
    if repeat {
        expected.push(event(0, 16, Z80BusEventKind::Internal, None, None, 5));
    }
    expected
}

#[test]
fn ldi_trace_orders_fetch_read_and_write_at_documented_tstate_offsets() {
    let (cycles, pc, events) = run_trace(0xA0, 0x10, 0x2000, 0x3000, 1, 0x5A);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(
        events,
        expected_transfer_trace(0xA0, 0x2000, 0x3000, 0x5A, false)
    );
}

#[test]
fn ldd_trace_uses_the_decrementing_instruction_encoding() {
    let (cycles, pc, events) = run_trace(0xA8, 0x10, 0x2000, 0x3000, 1, 0xA5);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(
        events,
        expected_transfer_trace(0xA8, 0x2000, 0x3000, 0xA5, false)
    );
}

#[test]
fn ldir_trace_adds_the_fifth_mcycle_when_count_remains_nonzero() {
    let (cycles, pc, events) = run_trace(0xB0, 0x10, 0x2000, 0x3000, 2, 0x5A);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(
        events,
        expected_transfer_trace(0xB0, 0x2000, 0x3000, 0x5A, true)
    );
}

#[test]
fn lddr_trace_covers_zero_count_wrap_to_ffff() {
    let (cycles, pc, events) = run_trace(0xB8, 0x10, 0x2000, 0x3000, 0, 0x5A);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(
        events,
        expected_transfer_trace(0xB8, 0x2000, 0x3000, 0x5A, true)
    );
}

#[test]
fn cpi_trace_contains_the_read_and_five_tstate_internal_cycle() {
    let (cycles, pc, events) = run_trace(0xA1, 0x5A, 0x2000, 0, 1, 0x5A);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(
        events,
        expected_compare_trace(0xA1, 0x2000, 0x5A, false)
    );
}

#[test]
fn cpd_trace_uses_the_decrementing_instruction_encoding() {
    let (cycles, pc, events) = run_trace(0xA9, 0x11, 0x2000, 0, 1, 0x22);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(
        events,
        expected_compare_trace(0xA9, 0x2000, 0x22, false)
    );
}

#[test]
fn cpir_trace_adds_repeat_cycle_for_nonmatching_value_and_nonzero_count() {
    let (cycles, pc, events) = run_trace(0xB1, 0x00, 0x2000, 0, 2, 0x11);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(
        events,
        expected_compare_trace(0xB1, 0x2000, 0x11, true)
    );
}

#[test]
fn cpdr_trace_adds_repeat_cycle_for_nonmatching_value_and_nonzero_count() {
    let (cycles, pc, events) = run_trace(0xB9, 0x11, 0x2000, 0, 2, 0x22);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(
        events,
        expected_compare_trace(0xB9, 0x2000, 0x22, true)
    );
}
