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
    expected.push(event(
        0,
        8,
        Z80BusEventKind::MemoryRead,
        Some(hl),
        Some(value),
        3,
    ));
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
    assert_eq!(events, expected_compare_trace(0xA1, 0x2000, 0x5A, false));
}

#[test]
fn cpd_trace_uses_the_decrementing_instruction_encoding() {
    let (cycles, pc, events) = run_trace(0xA9, 0x11, 0x2000, 0, 1, 0x22);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(events, expected_compare_trace(0xA9, 0x2000, 0x22, false));
}

#[test]
fn cpir_trace_adds_repeat_cycle_for_nonmatching_value_and_nonzero_count() {
    let (cycles, pc, events) = run_trace(0xB1, 0x00, 0x2000, 0, 2, 0x11);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(events, expected_compare_trace(0xB1, 0x2000, 0x11, true));
}

#[test]
fn cpdr_trace_adds_repeat_cycle_for_nonmatching_value_and_nonzero_count() {
    let (cycles, pc, events) = run_trace(0xB9, 0x11, 0x2000, 0, 2, 0x22);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(events, expected_compare_trace(0xB9, 0x2000, 0x22, true));
}

#[derive(Clone, Copy)]
struct SingleStepCycle {
    address: Option<u16>,
    data: Option<u8>,
    signals: &'static str,
}

#[test]
fn ldi_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED A0 0000 at commit
    // ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = [
        SingleStepCycle {
            address: Some(0xCE39),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xCE39),
            data: None,
            signals: "r-m-",
        },
        SingleStepCycle {
            address: Some(0xA214),
            data: Some(0xED),
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xA214),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xCE3A),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xCE3A),
            data: None,
            signals: "r-m-",
        },
        SingleStepCycle {
            address: Some(0xA215),
            data: Some(0xA0),
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xA215),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xE765),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xE765),
            data: None,
            signals: "r-m-",
        },
        SingleStepCycle {
            address: Some(0xE765),
            data: Some(0xE8),
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xDA43),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xDA43),
            data: Some(0xE8),
            signals: "-wm-",
        },
        SingleStepCycle {
            address: Some(0xDA43),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xDA43),
            data: None,
            signals: "----",
        },
        SingleStepCycle {
            address: Some(0xDA43),
            data: None,
            signals: "----",
        },
    ];

    assert_eq!(samples.len(), 16);
    assert_eq!(samples[1].signals, "r-m-");
    assert_eq!(samples[5].signals, "r-m-");
    assert_eq!(samples[9].signals, "r-m-");
    assert_eq!(samples[12].signals, "-wm-");

    let mut bus = TraceBus::default();
    bus.memory[0xCE39] = 0xED;
    bus.memory[0xCE3A] = 0xA0;
    bus.memory[0xE765] = 0xE8;
    bus.memory[0xDA43] = 0;

    let mut cpu = Z80::default();
    cpu.registers.pc = 0xCE39;
    cpu.registers.a = 0x4A;
    cpu.registers.f = 0xF3;
    cpu.registers.set_bc(0xEDCF);
    cpu.registers.set_de(0xDA43);
    cpu.registers.set_hl(0xE765);
    cpu.i = 0xA2;
    cpu.r = 20;
    cpu.wz = 0x555C;
    cpu.iff1 = true;
    cpu.iff2 = true;
    cpu.ei_delay = 1;
    cpu.q = 0xF3;

    assert_eq!(cpu.step(&mut bus).expect("pinned LDI executes"), 16);
    assert_eq!(cpu.registers.pc, 0xCE3B);
    assert_eq!(cpu.registers.bc(), 0xEDCE);
    assert_eq!(cpu.registers.de(), 0xDA44);
    assert_eq!(cpu.registers.hl(), 0xE766);
    assert_eq!(cpu.registers.f, 0xE5);
    assert_eq!(bus.memory[0xDA43], 0xE8);

    let instruction_pc = 0xCE39;
    let expected = vec![
        event(
            instruction_pc,
            0,
            Z80BusEventKind::OpcodeFetch,
            samples[0].address,
            samples[2].data,
            4,
        ),
        event(
            instruction_pc,
            4,
            Z80BusEventKind::OpcodeFetch,
            samples[4].address,
            samples[6].data,
            4,
        ),
        event(
            instruction_pc,
            8,
            Z80BusEventKind::MemoryRead,
            samples[8].address,
            samples[10].data,
            3,
        ),
        event(
            instruction_pc,
            11,
            Z80BusEventKind::MemoryWrite,
            samples[11].address,
            samples[12].data,
            5,
        ),
    ];
    assert_eq!(bus.events, expected);
    assert_eq!(
        expected
            .iter()
            .map(|e| u32::from(e.duration_tstates))
            .sum::<u32>(),
        samples.len() as u32
    );
}
