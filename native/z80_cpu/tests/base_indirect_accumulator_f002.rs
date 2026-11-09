//! E7.7 F-002: LD A,(BC)/(DE) and LD (BC)/(DE),A.
//!
//! The Zilog manual is the oracle for the documented operation, unchanged flags,
//! and 7-T / 2-M-cycle timing. WZ/MEMPTR is deliberately treated as a separate
//! compatibility behavior and checked against MAME, ares, and the independent
//! superzazu implementation rather than attributed to UM0080.

use sega_z80_cpu::bus::CpuBus;
use sega_z80_cpu::{
    Z80, Z80BusOperation, Z80MicroSequencer, Z80MicroSequencerState,
    Z80MicroStep, Z80Registers,
};

const ADDRESS_VECTORS: [u16; 5] = [0x00FE, 0x00FF, 0x1234, 0xFFFE, 0xFFFF];
const INITIAL_WZ: u16 = 0xA55A;

struct FlatBus<'a> {
    memory: &'a mut [u8; 65_536],
}

impl CpuBus for FlatBus<'_> {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Access {
    operation: Z80BusOperation,
    address: u16,
    base_tstates: u8,
}

fn snapshot_round_trip(cpu: Z80MicroSequencer) -> Z80MicroSequencer {
    let encoded = cpu.export_state().encode().expect("snapshot encodes");
    let decoded = Z80MicroSequencerState::decode(&encoded).expect("snapshot decodes");
    Z80MicroSequencer::restore_state(decoded).expect("snapshot restores")
}

fn run_microsequenced(
    mut cpu: Z80MicroSequencer,
    memory: &mut [u8; 65_536],
) -> (Z80MicroSequencer, u32, Vec<Access>) {
    let mut step = cpu.begin_instruction().expect("instruction starts");
    let mut accesses = Vec::with_capacity(2);

    loop {
        match step {
            Z80MicroStep::BusRequest(request) => {
                let wait_tstates = match request.operation {
                    Z80BusOperation::OpcodeFetch => 1,
                    Z80BusOperation::MemoryRead | Z80BusOperation::MemoryWrite { .. } => 2,
                    other => panic!("unexpected F-002 bus operation: {other:?}"),
                };
                cpu.insert_wait_tstates(wait_tstates)
                    .expect("WAIT extends the outstanding request");
                cpu = snapshot_round_trip(cpu);

                accesses.push(Access {
                    operation: request.operation,
                    address: request.address,
                    base_tstates: request.base_tstates,
                });
                let data = match request.operation {
                    Z80BusOperation::OpcodeFetch | Z80BusOperation::MemoryRead => {
                        Some(memory[usize::from(request.address)])
                    }
                    Z80BusOperation::MemoryWrite { value } => {
                        memory[usize::from(request.address)] = value;
                        None
                    }
                    other => panic!("unexpected F-002 bus operation: {other:?}"),
                };
                step = cpu
                    .complete_bus_cycle(request.id, data)
                    .expect("bus response completes the matching request");
                cpu = snapshot_round_trip(cpu);
            }
            Z80MicroStep::InstructionRetired { tstates, .. } => {
                return (cpu, tstates, accesses);
            }
            Z80MicroStep::InternalCycle { .. } => {
                panic!("F-002 must not invent an internal M-cycle");
            }
            other => panic!("unexpected F-002 microstep: {other:?}"),
        }
    }
}

fn initial_registers(opcode: u8, address: u16, value: u8, flags: u8) -> Z80Registers {
    let mut registers = Z80Registers {
        a: if matches!(opcode, 0x02 | 0x12) { value } else { 0xC3 },
        f: flags,
        pc: 0,
        ..Z80Registers::default()
    };
    if matches!(opcode, 0x02 | 0x0A) {
        registers.set_bc(address);
        registers.set_de(0x2468);
    } else {
        registers.set_bc(0x1357);
        registers.set_de(address);
    }
    registers
}

fn expected_wz(opcode: u8, address: u16, value: u8) -> u16 {
    let next = address.wrapping_add(1);
    if matches!(opcode, 0x02 | 0x12) {
        (u16::from(value) << 8) | (next & 0x00FF)
    } else {
        next
    }
}

#[test]
fn bc_de_indirect_accumulator_transfers_match_atomic_and_microsequenced_paths() {
    let mut memory = Box::new([0u8; 65_536]);

    for opcode in [0x02, 0x0A, 0x12, 0x1A] {
        let store = matches!(opcode, 0x02 | 0x12);
        for address in ADDRESS_VECTORS {
            for value in u8::MIN..=u8::MAX {
                memory[0] = opcode;
                memory[usize::from(address)] = if store { 0x96 } else { value };
                let initial_memory = memory[usize::from(address)];
                let flags = value.rotate_left(1) ^ 0xA5;
                let registers = initial_registers(opcode, address, value, flags);

                let mut atomic = Z80::default();
                atomic.registers = registers;
                atomic.wz = INITIAL_WZ;
                let atomic_tstates = atomic
                    .step(&mut FlatBus {
                        memory: &mut memory,
                    })
                    .expect("atomic BASE instruction executes");
                let atomic_memory = memory[usize::from(address)];

                // Restore the pre-instruction byte so the microsequencer sees
                // exactly the same initial memory image as the atomic executor.
                memory[usize::from(address)] = initial_memory;

                let mut micro = Z80MicroSequencer::default();
                micro.registers = registers;
                micro.wz = INITIAL_WZ;
                let (micro, micro_tstates, accesses) = run_microsequenced(micro, &mut memory);

                let expected_operation = if store {
                    Z80BusOperation::MemoryWrite { value }
                } else {
                    Z80BusOperation::MemoryRead
                };
                let expected_accesses = [
                    Access {
                        operation: Z80BusOperation::OpcodeFetch,
                        address: 0,
                        base_tstates: 4,
                    },
                    Access {
                        operation: expected_operation,
                        address,
                        base_tstates: 3,
                    },
                ];
                let expected_accumulator = value;
                let expected_final_memory = if store { value } else { initial_memory };
                let context = format!("opcode={opcode:02X}, address={address:04X}, value={value:02X}");

                assert_eq!(atomic_tstates, 7, "{context}: atomic nominal timing");
                assert_eq!(micro_tstates, 10, "{context}: 7T plus 3 WAIT T-states");
                assert_eq!(accesses.as_slice(), expected_accesses, "{context}: bus order");
                assert_eq!(micro.registers, atomic.registers, "{context}: registers");
                assert_eq!(micro.wz, atomic.wz, "{context}: WZ differential");
                assert_eq!(micro.refresh, atomic.r, "{context}: refresh");
                assert_eq!(atomic.registers.f, flags, "{context}: flags preserved");
                assert_eq!(micro.registers.f, flags, "{context}: micro flags preserved");
                assert_eq!(atomic.registers.a, expected_accumulator, "{context}: A");
                assert_eq!(atomic.wz, expected_wz(opcode, address, value), "{context}: WZ oracle");
                assert_eq!(
                    memory[usize::from(address)],
                    expected_final_memory,
                    "{context}: memory result"
                );
                assert_eq!(atomic_memory, expected_final_memory, "{context}: atomic memory");
                assert_eq!(micro.registers.pc, 1, "{context}: PC");
                assert_eq!(micro.refresh & 0x7F, 1, "{context}: one M1 refresh");
            }
        }
    }
}
