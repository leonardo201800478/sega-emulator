use sega_sms_core::{CpuBus, Z80, Z80BusEvent, Z80BusEventKind, Z80TStatePhase, Z80TStateSample};

struct TraceBus {
    memory: Box<[u8]>,
    events: Vec<Z80BusEvent>,
    tstate_samples: Vec<Z80TStateSample>,
}

impl Default for TraceBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
            events: Vec::new(),
            tstate_samples: Vec::new(),
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

    fn observe_tstate_sample(&mut self, sample: Z80TStateSample) {
        self.tstate_samples.push(sample);
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
        expected.push(event(0, 16, Z80BusEventKind::Internal, Some(de), None, 5));
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
    expected.push(event(0, 11, Z80BusEventKind::Internal, Some(hl), None, 5));
    if repeat {
        expected.push(event(0, 16, Z80BusEventKind::Internal, Some(hl), None, 5));
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

fn assert_tstate_samples_match(
    actual: &[Z80TStateSample],
    expected: &[SingleStepCycle],
    is_transfer: bool,
) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual.tstate_offset, index as u32, "offset at T-state {index}");
        assert_eq!(actual.address, expected.address, "address at T-state {index}");
        assert_eq!(actual.data, expected.data, "data at T-state {index}");
        let flags = [
            if actual.rd { 'r' } else { '-' },
            if actual.wr { 'w' } else { '-' },
            if actual.mreq { 'm' } else { '-' },
            if actual.iorq { 'i' } else { '-' },
        ];
        let actual_signals: String = flags.iter().copied().collect();
        assert_eq!(actual_signals.as_str(), expected.signals, "signals at T-state {index}");
        let expected_phase = match index {
            0 | 1 | 4 | 5 => Z80TStatePhase::OpcodeFetch,
            2 | 3 | 6 | 7 => Z80TStatePhase::Refresh,
            8..=10 => Z80TStatePhase::MemoryRead,
            11..=15 if is_transfer => Z80TStatePhase::MemoryWrite,
            _ => Z80TStatePhase::Internal,
        };
        assert_eq!(actual.phase, expected_phase, "phase at T-state {index}");
    }
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
    assert_tstate_samples_match(&bus.tstate_samples, &samples, true);
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
    assert!(
        bus.events
            .windows(2)
            .all(|pair| pair[0].end_tstate() == pair[1].tstate_offset)
    );
}

macro_rules! single_step_cycles {
    ($(($address:expr, $data:expr, $signals:literal)),+ $(,)?) => {
        [
            $(SingleStepCycle {
                address: $address,
                data: $data,
                signals: $signals,
            }),+
        ]
    };
}

fn assert_external_mcycle_trace(
    opcode: u8,
    accumulator: u8,
    bc: u16,
    i: u8,
    r: u8,
    samples: &[SingleStepCycle],
    is_transfer: bool,
) {
    let pc = samples[0]
        .address
        .expect("pinned opcode fetch has an address");
    assert_eq!(samples[2].data, Some(0xED));
    assert_eq!(samples[6].data, Some(opcode));
    let hl = samples[8].address.expect("pinned data read has an address");
    let value = samples[10].data.expect("pinned data read samples a value");
    let de = if is_transfer {
        samples[11]
            .address
            .expect("pinned data write has an address")
    } else {
        0
    };
    let decremented_bc = bc.wrapping_sub(1);
    let repeats = match opcode {
        0xB0 | 0xB8 => decremented_bc != 0,
        0xB1 | 0xB9 => decremented_bc != 0 && accumulator != value,
        _ => false,
    };
    assert_eq!(samples.len(), if repeats { 21 } else { 16 });
    assert_eq!(samples[1].signals, "r-m-");
    assert_eq!(samples[5].signals, "r-m-");
    assert_eq!(samples[9].signals, "r-m-");
    if is_transfer {
        assert_eq!(samples[12].signals, "-wm-");
        assert_eq!(samples[12].data, Some(value));
    }

    let mut bus = TraceBus::default();
    bus.memory[usize::from(pc)] = 0xED;
    bus.memory[usize::from(pc.wrapping_add(1))] = opcode;
    bus.memory[usize::from(hl)] = value;
    let mut cpu = Z80::default();
    cpu.registers.pc = pc;
    cpu.registers.a = accumulator;
    cpu.i = i;
    cpu.r = r;
    cpu.registers.set_bc(bc);
    cpu.registers.set_hl(hl);
    cpu.registers.set_de(de);

    let tstates = cpu
        .step(&mut bus)
        .expect("pinned ED block instruction executes");
    assert_tstate_samples_match(&bus.tstate_samples, samples, is_transfer);
    assert_eq!(tstates, samples.len() as u32);
    assert_eq!(
        cpu.registers.pc,
        if repeats { pc } else { pc.wrapping_add(2) }
    );
    if is_transfer {
        assert_eq!(bus.memory[usize::from(de)], value);
    }

    let mut expected = vec![
        event(
            pc,
            0,
            Z80BusEventKind::OpcodeFetch,
            samples[0].address,
            samples[2].data,
            4,
        ),
        event(
            pc,
            4,
            Z80BusEventKind::OpcodeFetch,
            samples[4].address,
            samples[6].data,
            4,
        ),
        event(
            pc,
            8,
            Z80BusEventKind::MemoryRead,
            samples[8].address,
            samples[10].data,
            3,
        ),
    ];
    if is_transfer {
        expected.push(event(
            pc,
            11,
            Z80BusEventKind::MemoryWrite,
            samples[11].address,
            samples[12].data,
            5,
        ));
    } else {
        expected.push(event(
            pc,
            11,
            Z80BusEventKind::Internal,
            samples[11].address,
            None,
            5,
        ));
    }
    if repeats {
        expected.push(event(
            pc,
            16,
            Z80BusEventKind::Internal,
            samples[16].address,
            None,
            5,
        ));
    }
    assert_eq!(bus.events, expected);
    assert_eq!(
        expected
            .iter()
            .map(|e| u32::from(e.duration_tstates))
            .sum::<u32>(),
        samples.len() as u32
    );
    assert!(
        bus.events
            .windows(2)
            .all(|pair| pair[0].end_tstate() == pair[1].tstate_offset)
    );
}

#[test]
fn cpi_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED A1 0000 at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0xD0B3), None, "----"),
        (Some(0xD0B3), None, "r-m-"),
        (Some(0x7949), Some(0xED), "----"),
        (Some(0x7949), None, "----"),
        (Some(0xD0B4), None, "----"),
        (Some(0xD0B4), None, "r-m-"),
        (Some(0x794A), Some(0xA1), "----"),
        (Some(0x794A), None, "----"),
        (Some(0x723D), None, "----"),
        (Some(0x723D), None, "r-m-"),
        (Some(0x723D), Some(0x6B), "----"),
        (Some(0x723D), None, "----"),
        (Some(0x723D), None, "----"),
        (Some(0x723D), None, "----"),
        (Some(0x723D), None, "----"),
        (Some(0x723D), None, "----"),
    ];
    assert_external_mcycle_trace(0xA1, 0x14, 0x5877, 0x79, 0x49, &samples, false);
}

#[test]
fn ldd_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED A8 0000 at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0x048D), None, "----"),
        (Some(0x048D), None, "r-m-"),
        (Some(0xE36E), Some(0xED), "----"),
        (Some(0xE36E), None, "----"),
        (Some(0x048E), None, "----"),
        (Some(0x048E), None, "r-m-"),
        (Some(0xE36F), Some(0xA8), "----"),
        (Some(0xE36F), None, "----"),
        (Some(0x2A3D), None, "----"),
        (Some(0x2A3D), None, "r-m-"),
        (Some(0x2A3D), Some(0xA7), "----"),
        (Some(0xFF85), None, "----"),
        (Some(0xFF85), Some(0xA7), "-wm-"),
        (Some(0xFF85), None, "----"),
        (Some(0xFF85), None, "----"),
        (Some(0xFF85), None, "----"),
    ];
    assert_external_mcycle_trace(0xA8, 0x87, 0x4D61, 0xE3, 0x6E, &samples, true);
}

#[test]
fn cpd_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED A9 0000 at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0x3717), None, "----"),
        (Some(0x3717), None, "r-m-"),
        (Some(0x6A50), Some(0xED), "----"),
        (Some(0x6A50), None, "----"),
        (Some(0x3718), None, "----"),
        (Some(0x3718), None, "r-m-"),
        (Some(0x6A51), Some(0xA9), "----"),
        (Some(0x6A51), None, "----"),
        (Some(0xBA2A), None, "----"),
        (Some(0xBA2A), None, "r-m-"),
        (Some(0xBA2A), Some(0x25), "----"),
        (Some(0xBA2A), None, "----"),
        (Some(0xBA2A), None, "----"),
        (Some(0xBA2A), None, "----"),
        (Some(0xBA2A), None, "----"),
        (Some(0xBA2A), None, "----"),
    ];
    assert_external_mcycle_trace(0xA9, 0xF5, 0x97AC, 0x6A, 0x50, &samples, false);
}

#[test]
fn ldir_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED B0 0000 at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0x2D25), None, "----"),
        (Some(0x2D25), None, "r-m-"),
        (Some(0x7473), Some(0xED), "----"),
        (Some(0x7473), None, "----"),
        (Some(0x2D26), None, "----"),
        (Some(0x2D26), None, "r-m-"),
        (Some(0x7474), Some(0xB0), "----"),
        (Some(0x7474), None, "----"),
        (Some(0xD07B), None, "----"),
        (Some(0xD07B), None, "r-m-"),
        (Some(0xD07B), Some(0x89), "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), Some(0x89), "-wm-"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
        (Some(0x5B54), None, "----"),
    ];
    assert_external_mcycle_trace(0xB0, 0xB2, 0x98EA, 0x74, 0x73, &samples, true);
}

#[test]
fn cpir_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED B1 0000 at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0xE842), None, "----"),
        (Some(0xE842), None, "r-m-"),
        (Some(0xFC12), Some(0xED), "----"),
        (Some(0xFC12), None, "----"),
        (Some(0xE843), None, "----"),
        (Some(0xE843), None, "r-m-"),
        (Some(0xFC13), Some(0xB1), "----"),
        (Some(0xFC13), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "r-m-"),
        (Some(0xBFF6), Some(0x91), "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
        (Some(0xBFF6), None, "----"),
    ];
    assert_external_mcycle_trace(0xB1, 0xA3, 0x1495, 0xFC, 0x12, &samples, false);
}

#[test]
fn lddr_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED B8 035E at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0x5912), None, "----"),
        (Some(0x5912), None, "r-m-"),
        (Some(0x8062), Some(0xED), "----"),
        (Some(0x8062), None, "----"),
        (Some(0x5913), None, "----"),
        (Some(0x5913), None, "r-m-"),
        (Some(0x8063), Some(0xB8), "----"),
        (Some(0x8063), None, "----"),
        (Some(0xB675), None, "----"),
        (Some(0xB675), None, "r-m-"),
        (Some(0xB675), Some(0x92), "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), Some(0x92), "-wm-"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
        (Some(0xAC08), None, "----"),
    ];
    assert_external_mcycle_trace(0xB8, 0xCA, 0x0000, 0x80, 0x62, &samples, true);
}

#[test]
fn cpdr_mcycle_trace_matches_pinned_singlestep_bus_samples() {
    // SingleStepTests/Z80 vector ED B9 0000 at commit ebe1875d48f374bcfd4b505d8eb8ee751568b5f7.
    let samples = single_step_cycles![
        (Some(0xB4F3), None, "----"),
        (Some(0xB4F3), None, "r-m-"),
        (Some(0x123C), Some(0xED), "----"),
        (Some(0x123C), None, "----"),
        (Some(0xB4F4), None, "----"),
        (Some(0xB4F4), None, "r-m-"),
        (Some(0x123D), Some(0xB9), "----"),
        (Some(0x123D), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "r-m-"),
        (Some(0x20AB), Some(0x4A), "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
        (Some(0x20AB), None, "----"),
    ];
    assert_external_mcycle_trace(0xB9, 0x2F, 0x2BD9, 0x12, 0x3C, &samples, false);
}

#[test]
fn ldir_trace_omits_repeat_cycle_when_bc_decrements_to_zero() {
    let (cycles, pc, events) = run_trace(0xB0, 0x10, 0x2000, 0x3000, 1, 0x5A);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(
        events,
        expected_transfer_trace(0xB0, 0x2000, 0x3000, 0x5A, false)
    );
}

#[test]
fn cpir_trace_stops_when_accumulator_matches_memory() {
    let (cycles, pc, events) = run_trace(0xB1, 0x5A, 0x2000, 0, 2, 0x5A);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(events, expected_compare_trace(0xB1, 0x2000, 0x5A, false));
}

#[test]
fn cpdr_trace_stops_when_bc_decrements_to_zero_even_without_match() {
    let (cycles, pc, events) = run_trace(0xB9, 0x10, 0x2000, 0, 1, 0x5A);
    assert_eq!(cycles, 16);
    assert_eq!(pc, 2);
    assert_eq!(events, expected_compare_trace(0xB9, 0x2000, 0x5A, false));
}

#[test]
fn cpir_trace_repeats_after_zero_bc_wraps_to_ffff() {
    let (cycles, pc, events) = run_trace(0xB1, 0x10, 0x2000, 0, 0, 0x5A);
    assert_eq!(cycles, 21);
    assert_eq!(pc, 0);
    assert_eq!(events, expected_compare_trace(0xB1, 0x2000, 0x5A, true));
}
