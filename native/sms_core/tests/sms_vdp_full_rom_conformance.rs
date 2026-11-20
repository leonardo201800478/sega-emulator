//! Full upstream SmsVdpTest ROM execution for SMS1 VDP conformance.
//!
//! The dedicated sms-vdp-full-rom workflow builds the pinned upstream source
//! with WLA-DX and supplies SMSVDPTEST_ROM_PATH. This test is ignored in the
//! ordinary workspace suite so the external assembler is not a normal build
//! dependency.

use std::{collections::VecDeque, env, fs};

use sega_sms_core::{SmsTemporalMachine, SmsTemporalTraceEvent, SmsVdpProfile, Territory};
use sega_z80_cpu::Z80BusOperation;

const PSG_CLOCK_HZ: u64 = 3_579_545;
const SAMPLE_RATE_HZ: u64 = 44_100;
const COMPLETION_MARKER: u16 = 0xC9FF;

type HCounterReadTrace = (u128, u8, u16, u16, bool, Option<u8>, bool, u8, bool, bool);
type IoControlWriteTrace = (
    u128,
    u16,
    u8,
    u16,
    u16,
    bool,
    Option<u8>,
    bool,
    u8,
    bool,
    bool,
);
type CpuMemoryReadTrace = (
    u128,
    u16,
    u16,
    u16,
    u8,
    u32,
    bool,
    Option<u8>,
    bool,
    u8,
    bool,
    bool,
);
type M1FetchTrace = (u128, u16, u16, u16, bool, Option<u8>, bool, u8, bool, bool);
type ThLatchReadPair = (u128, u16, bool, Option<u8>, u128, u8, u16, bool, Option<u8>);
type IrqLineTrace = (u128, Option<u8>, bool, bool);
type InternalCycleTrace = (
    u128,
    u64,
    u32,
    u16,
    u16,
    bool,
    Option<u8>,
    bool,
    u8,
    bool,
    bool,
);
#[derive(Clone, Copy, Debug)]
struct BusTrace {
    timestamp: u128,
    operation: Z80BusOperation,
    address: u16,
    base_tstates: u8,
    wait_tstates: u32,
    data: Option<u8>,
    cpu_pc_before: u16,
    cpu_pc_after: u16,
    cpu_halted: bool,
    cpu_int_line: Option<u8>,
    cpu_iff1: bool,
    cpu_ei_delay: u8,
    cpu_halt_exit_int: bool,
    cpu_halt_exit_nmi: bool,
}

#[test]
#[ignore = "run by the dedicated SMS VDP full-ROM conformance workflow"]
fn smsvdptest_v131_full_rom_passes_vcounter_and_irq_hcount_tests() {
    let rom_path = env::var_os("SMSVDPTEST_ROM_PATH")
        .expect("SMSVDPTEST_ROM_PATH must point to the patched upstream SmsVdpTest ROM");
    let rom = fs::read(&rom_path).expect("the assembled SmsVdpTest ROM must be readable");

    let mut machine = SmsTemporalMachine::new_with_hardware_profile(
        rom,
        1,
        PSG_CLOCK_HZ,
        SAMPLE_RATE_HZ,
        SmsVdpProfile::sms1_t0_ntsc(),
        Territory::Export,
    )
    .expect("the full SmsVdpTest ROM should initialize on the SMS1 temporal machine");

    machine.set_trace_enabled(true);
    let mut completed_at_frame = None;
    let mut hcounter_reference_reads: Vec<HCounterReadTrace> = Vec::new();
    let mut vcounter_reference_reads = Vec::new();
    let mut diagnostic_memory_writes = Vec::new();
    let mut run_test32_m1_fetches = Vec::new();
    let mut run_test34_m1_fetches = Vec::new();
    let mut io_control_writes: Vec<IoControlWriteTrace> = Vec::new();
    let mut cpu_memory_read_trace: VecDeque<CpuMemoryReadTrace> = VecDeque::with_capacity(65_536);
    let mut cpu_m1_fetch_trace: VecDeque<M1FetchTrace> = VecDeque::with_capacity(65_536);
    let mut irq_line_changes: Vec<IrqLineTrace> = Vec::new();
    let mut cpu_internal_cycle_trace: VecDeque<InternalCycleTrace> =
        VecDeque::with_capacity(65_536);
    let mut bus_dispatch_trace: VecDeque<BusTrace> = VecDeque::with_capacity(65_536);

    for frame in 0..600 {
        if let Err(error) = machine.step_frame(100_000) {
            panic!("SmsVdpTest failed while advancing frame {frame}: {error}");
        }
        for event in machine.take_trace() {
            if let SmsTemporalTraceEvent::BusCompleted {
                timestamp,
                operation,
                address,
                base_tstates,
                wait_tstates,
                data,
                cpu_pc_before,
                cpu_pc_after,
                cpu_halted,
                cpu_int_line,
                cpu_iff1,
                cpu_ei_delay,
                cpu_halt_exit_int,
                cpu_halt_exit_nmi,
                ..
            } = event
            {
                if bus_dispatch_trace.len() == 65_536 {
                    bus_dispatch_trace.pop_front();
                }
                bus_dispatch_trace.push_back(BusTrace {
                    timestamp: timestamp.ticks(),
                    operation,
                    address,
                    base_tstates,
                    wait_tstates,
                    data,
                    cpu_pc_before,
                    cpu_pc_after,
                    cpu_halted,
                    cpu_int_line,
                    cpu_iff1,
                    cpu_ei_delay,
                    cpu_halt_exit_int,
                    cpu_halt_exit_nmi,
                });
            }
            match event {
                SmsTemporalTraceEvent::M1FetchCompleted {
                    timestamp,
                    address,
                    refresh_address,
                    cpu_pc,
                    cpu_halted,
                    cpu_int_line,
                    cpu_iff1,
                    cpu_ei_delay,
                    cpu_halt_exit_int,
                    cpu_halt_exit_nmi,
                } => {
                    if cpu_m1_fetch_trace.len() == 65_536 {
                        cpu_m1_fetch_trace.pop_front();
                    }
                    cpu_m1_fetch_trace.push_back((
                        timestamp.ticks(),
                        address,
                        refresh_address,
                        cpu_pc,
                        cpu_halted,
                        cpu_int_line,
                        cpu_iff1,
                        cpu_ei_delay,
                        cpu_halt_exit_int,
                        cpu_halt_exit_nmi,
                    ));
                }
                SmsTemporalTraceEvent::InternalCycleCompleted {
                    timestamp,
                    continuation_id,
                    tstates,
                    cpu_pc_before,
                    cpu_pc_after,
                    cpu_halted,
                    cpu_int_line,
                    cpu_iff1,
                    cpu_ei_delay,
                    cpu_halt_exit_int,
                    cpu_halt_exit_nmi,
                } => {
                    if cpu_internal_cycle_trace.len() == 65_536 {
                        cpu_internal_cycle_trace.pop_front();
                    }
                    cpu_internal_cycle_trace.push_back((
                        timestamp.ticks(),
                        continuation_id,
                        tstates,
                        cpu_pc_before,
                        cpu_pc_after,
                        cpu_halted,
                        cpu_int_line,
                        cpu_iff1,
                        cpu_ei_delay,
                        cpu_halt_exit_int,
                        cpu_halt_exit_nmi,
                    ));
                }
                SmsTemporalTraceEvent::InterruptLineChanged {
                    timestamp,
                    line,
                    vdp_irq,
                    external_irq,
                } => {
                    if irq_line_changes.len() == 4096 {
                        irq_line_changes.remove(0);
                    }
                    irq_line_changes.push((timestamp.ticks(), line, vdp_irq, external_irq));
                }
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation: Z80BusOperation::MemoryRead,
                    address,
                    base_tstates,
                    wait_tstates,
                    cpu_pc_before,
                    cpu_pc_after,
                    cpu_halted,
                    cpu_int_line,
                    cpu_iff1,
                    cpu_ei_delay,
                    cpu_halt_exit_int,
                    cpu_halt_exit_nmi,
                    ..
                } => {
                    if cpu_memory_read_trace.len() == 65_536 {
                        cpu_memory_read_trace.pop_front();
                    }
                    cpu_memory_read_trace.push_back((
                        timestamp.ticks(),
                        address,
                        cpu_pc_before,
                        cpu_pc_after,
                        base_tstates,
                        wait_tstates,
                        cpu_halted,
                        cpu_int_line,
                        cpu_iff1,
                        cpu_ei_delay,
                        cpu_halt_exit_int,
                        cpu_halt_exit_nmi,
                    ));
                }
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation: Z80BusOperation::PortRead,
                    address,
                    data,
                    ..
                } if address & 0x00FF == 0x7E => {
                    vcounter_reference_reads.push((timestamp.ticks(), data.unwrap_or(0)));
                }
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation: Z80BusOperation::MemoryWrite { value },
                    address,
                    ..
                } if (0xC9F0..=0xC9F2).contains(&address)
                    || address == 0xC91F
                    || address == 0xC921 =>
                {
                    diagnostic_memory_writes.push((timestamp.ticks(), address, value));
                    if address == 0xC91F {
                        run_test32_m1_fetches =
                            cpu_m1_fetch_trace.iter().rev().take(64).copied().collect();
                        run_test32_m1_fetches.reverse();
                    } else if address == 0xC921 {
                        run_test34_m1_fetches =
                            cpu_m1_fetch_trace.iter().rev().take(64).copied().collect();
                        run_test34_m1_fetches.reverse();
                    }
                }
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation: Z80BusOperation::PortRead,
                    address,
                    data,
                    cpu_pc_before,
                    cpu_pc_after,
                    cpu_halted,
                    cpu_int_line,
                    cpu_iff1,
                    cpu_ei_delay,
                    cpu_halt_exit_int,
                    cpu_halt_exit_nmi,
                    ..
                } if address & 0x00FF == 0x7F => {
                    hcounter_reference_reads.push((
                        timestamp.ticks(),
                        data.unwrap_or(0),
                        cpu_pc_before,
                        cpu_pc_after,
                        cpu_halted,
                        cpu_int_line,
                        cpu_iff1,
                        cpu_ei_delay,
                        cpu_halt_exit_int,
                        cpu_halt_exit_nmi,
                    ));
                }
                SmsTemporalTraceEvent::BusCompleted {
                    timestamp,
                    operation: Z80BusOperation::PortWrite { value },
                    address,
                    cpu_pc_before,
                    cpu_pc_after,
                    cpu_halted,
                    cpu_int_line,
                    cpu_iff1,
                    cpu_ei_delay,
                    cpu_halt_exit_int,
                    cpu_halt_exit_nmi,
                    ..
                } if address & 0x00FF == 0x3F => {
                    io_control_writes.push((
                        timestamp.ticks(),
                        address,
                        value,
                        cpu_pc_before,
                        cpu_pc_after,
                        cpu_halted,
                        cpu_int_line,
                        cpu_iff1,
                        cpu_ei_delay,
                        cpu_halt_exit_int,
                        cpu_halt_exit_nmi,
                    ));
                }
                _ => {}
            }
        }
        if machine.inspect_memory(COMPLETION_MARKER) == 0xAA {
            completed_at_frame = Some(frame + 1);
            break;
        }
    }

    let results = [
        (25, 0xC918),
        (31, 0xC91E),
        (32, 0xC91F),
        (33, 0xC920),
        (34, 0xC921),
    ]
    .map(|(test, address)| (test, machine.inspect_memory(address)));
    let all_results = (21u16..=34)
        .map(|test| (test, machine.inspect_memory(0xC900 + test - 1)))
        .collect::<Vec<_>>();
    // prepare_sms_vdp_test_rom.py relocates RunTest25's TestScratch to $C9F0
    // without adding cycles, keeping its three VCounter samples isolated.
    let run_test25_samples = [
        machine.inspect_memory(0xC9F0),
        machine.inspect_memory(0xC9F1),
        machine.inspect_memory(0xC9F2),
    ];
    let run_test25_write_times = diagnostic_memory_writes
        .iter()
        .filter(|(_, address, _)| (0xC9F0..=0xC9F2).contains(address))
        .map(|(timestamp, address, value)| (*timestamp, *address, *value))
        .collect::<Vec<_>>();
    let run_test25_vcounter_reads = match (
        run_test25_write_times.first(),
        run_test25_write_times.last(),
    ) {
        (Some((first, _, _)), Some((last, _, _))) => vcounter_reference_reads
            .iter()
            .filter(|(timestamp, _)| {
                *timestamp >= (*first).saturating_sub(40)
                    && *timestamp <= (*last).saturating_add(40)
            })
            .copied()
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let irq_hcounter_reads = |result_address: u16| {
        diagnostic_memory_writes
            .iter()
            .find(|(_, address, _)| *address == result_address)
            .map(|(result_time, _, _)| {
                let mut reads = hcounter_reference_reads
                    .iter()
                    .filter(|(timestamp, _, _, _, _, _, _, _, _, _)| {
                        *timestamp >= (*result_time).saturating_sub(150_000)
                            && *timestamp <= *result_time
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if reads.len() > 32 {
                    reads.drain(..reads.len() - 32);
                }
                reads
            })
            .unwrap_or_default()
    };
    let run_test32_hcounter_reads = irq_hcounter_reads(0xC91F);
    let run_test34_hcounter_reads = irq_hcounter_reads(0xC921);
    let irq_io_control_writes = |result_address: u16| {
        diagnostic_memory_writes
            .iter()
            .find(|(_, address, _)| *address == result_address)
            .map(|(result_time, _, _)| {
                let mut writes = io_control_writes
                    .iter()
                    .filter(|(timestamp, _, _, _, _, _, _, _, _, _, _)| {
                        *timestamp >= (*result_time).saturating_sub(150_000)
                            && *timestamp <= *result_time
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if writes.len() > 24 {
                    writes.drain(..writes.len() - 24);
                }
                writes
            })
            .unwrap_or_default()
    };
    let run_test32_io_control_writes = irq_io_control_writes(0xC91F);
    let run_test34_io_control_writes = irq_io_control_writes(0xC921);
    let hcounter_reads_after_th_rise = |writes: &[IoControlWriteTrace]| -> Vec<ThLatchReadPair> {
        let mut pairs = writes
            .iter()
            .filter(|(_, _, value, _, _, _, _, _, _, _, _)| *value == 0xFF)
            .filter_map(
                |(write_time, _, _, write_pc, _, write_halted, write_int, _, _, _, _)| {
                    hcounter_reference_reads
                        .iter()
                        .find(|(timestamp, _, _, _, _, _, _, _, _, _)| {
                            *timestamp >= *write_time && *timestamp <= write_time.saturating_add(32)
                        })
                        .map(
                            |(timestamp, value, read_pc, _, read_halted, read_int, _, _, _, _)| {
                                (
                                    *write_time,
                                    *write_pc,
                                    *write_halted,
                                    *write_int,
                                    *timestamp,
                                    *value,
                                    *read_pc,
                                    *read_halted,
                                    *read_int,
                                )
                            },
                        )
                },
            )
            .collect::<Vec<_>>();
        if pairs.len() > 12 {
            pairs.drain(..pairs.len() - 12);
        }
        pairs
    };
    let run_test32_th_latches = hcounter_reads_after_th_rise(&run_test32_io_control_writes);
    let run_test34_th_latches = hcounter_reads_after_th_rise(&run_test34_io_control_writes);
    let cpu_fetches_before_result = |result_address: u16| {
        diagnostic_memory_writes
            .iter()
            .find(|(_, address, _)| *address == result_address)
            .map(|(result_time, _, _)| {
                let mut fetches = cpu_memory_read_trace
                    .iter()
                    .filter(|(timestamp, _, _, _, _, _, _, _, _, _, _, _)| {
                        *timestamp >= (*result_time).saturating_sub(150_000)
                            && *timestamp <= *result_time
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if fetches.len() > 64 {
                    fetches.drain(..fetches.len() - 64);
                }
                fetches
            })
            .unwrap_or_default()
    };
    let run_test32_pc_trace = cpu_fetches_before_result(0xC91F);
    let run_test34_pc_trace = cpu_fetches_before_result(0xC921);

    let cpu_internal_cycles_before_result = |result_address: u16| {
        diagnostic_memory_writes
            .iter()
            .find(|(_, address, _)| *address == result_address)
            .map(|(result_time, _, _)| {
                let mut cycles = cpu_internal_cycle_trace
                    .iter()
                    .filter(|(timestamp, _, _, _, _, _, _, _, _, _, _)| {
                        *timestamp >= (*result_time).saturating_sub(150_000)
                            && *timestamp <= *result_time
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if cycles.len() > 64 {
                    cycles.drain(..cycles.len() - 64);
                }
                cycles
            })
            .unwrap_or_default()
    };
    let run_test32_internal_cycles = cpu_internal_cycles_before_result(0xC91F);
    let run_test34_internal_cycles = cpu_internal_cycles_before_result(0xC921);
    let irq_events_before_result = |result_address: u16| {
        diagnostic_memory_writes
            .iter()
            .find(|(_, address, _)| *address == result_address)
            .map(|(result_time, _, _)| {
                let start = (*result_time).saturating_sub(150_000);
                let mut lines = irq_line_changes
                    .iter()
                    .filter(|(timestamp, _, _, _)| {
                        *timestamp >= start && *timestamp <= *result_time
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if lines.len() > 32 {
                    lines.drain(..lines.len() - 32);
                }
                lines
            })
            .unwrap_or_default()
    };
    let run_test32_irq_lines = irq_events_before_result(0xC91F);
    let run_test34_irq_lines = irq_events_before_result(0xC921);
    let irq_execution_windows = |result_address: u16| {
        diagnostic_memory_writes
            .iter()
            .find(|(_, address, _)| *address == result_address)
            .map(|(result_time, _, _)| {
                let start = (*result_time).saturating_sub(150_000);
                irq_line_changes
                    .iter()
                    .filter(|(timestamp, line, _, _)| {
                        *timestamp >= start && *timestamp <= *result_time && line.is_some()
                    })
                    .map(|(irq_time, _, _, _)| {
                        let window_start = irq_time.saturating_sub(8);
                        let window_end = irq_time.saturating_add(96);
                        let bus = bus_dispatch_trace
                            .iter()
                            .filter(|event| {
                                event.timestamp >= window_start && event.timestamp <= window_end
                            })
                            .map(|event| {
                                format!(
                                    "t={} {:?} addr={:04X} base={} wait={} data={:?} pc={:04X}->{:04X} halted={} int={:?} iff1={} ei_delay={} halt_exit_int={} halt_exit_nmi={}",
                                    event.timestamp,
                                    event.operation,
                                    event.address,
                                    event.base_tstates,
                                    event.wait_tstates,
                                    event.data,
                                    event.cpu_pc_before,
                                    event.cpu_pc_after,
                                    event.cpu_halted,
                                    event.cpu_int_line,
                                    event.cpu_iff1,
                                    event.cpu_ei_delay,
                                    event.cpu_halt_exit_int,
                                    event.cpu_halt_exit_nmi
                                )
                            })
                            .collect::<Vec<_>>();
                        let m1 = cpu_m1_fetch_trace
                            .iter()
                            .filter(|event| event.0 >= window_start && event.0 <= window_end)
                            .copied()
                            .collect::<Vec<_>>();
                        let internal = cpu_internal_cycle_trace
                            .iter()
                            .filter(|event| event.0 >= window_start && event.0 <= window_end)
                            .copied()
                            .collect::<Vec<_>>();
                        (*irq_time, bus, m1, internal)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let run_test32_irq_windows = irq_execution_windows(0xC91F);
    let run_test34_irq_windows = irq_execution_windows(0xC921);

    assert!(
        completed_at_frame.is_some(),
        "SmsVdpTest did not reach the post-RunTest34 marker within 600 frames; results={results:?}; all={all_results:?}"
    );

    assert_eq!(
        results,
        [(25, 0), (31, 0), (32, 0), (33, 0), (34, 0)],
        "upstream SmsVdpTest v1.31 failures; all test results={all_results:?}; RunTest25 VCounter samples at $C9F0-$C9F2={run_test25_samples:?}; sample writes={run_test25_write_times:?}; VCounter reads in sample window={run_test25_vcounter_reads:?}; RunTest32 HCounter reads before result={run_test32_hcounter_reads:?}; RunTest34 HCounter reads before result={run_test34_hcounter_reads:?}; RunTest32 IO control writes={run_test32_io_control_writes:?}; RunTest32 TH-rise/read pairs={run_test32_th_latches:?}; RunTest34 IO control writes={run_test34_io_control_writes:?}; RunTest34 TH-rise/read pairs={run_test34_th_latches:?}; RunTest32 last CPU memory reads={run_test32_pc_trace:?}; RunTest34 last CPU memory reads={run_test34_pc_trace:?}; RunTest32 HALT/M1 T4 fetches={run_test32_m1_fetches:?}; RunTest34 HALT/M1 T4 fetches={run_test34_m1_fetches:?}; RunTest32 internal-cycle trace={run_test32_internal_cycles:?}; RunTest34 internal-cycle trace={run_test34_internal_cycles:?}; RunTest32 IRQ line changes={run_test32_irq_lines:?}; RunTest32 IRQ execution windows={run_test32_irq_windows:?}; RunTest34 IRQ line changes={run_test34_irq_lines:?}; RunTest34 IRQ execution windows={run_test34_irq_windows:?}"
    );
}
