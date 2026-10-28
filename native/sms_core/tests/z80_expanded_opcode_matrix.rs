//! Expanded fixed vectors from SingleStepTests/Z80 (MIT), pinned upstream revision.
//! Source: https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1
//! This curated matrix checks architectural state, aggregate T-states, and the
//! final values of RAM locations named by the upstream vector. It does not
//! claim cycle-by-cycle bus timing or complete Z80 conformance.

use sega_sms_core::{CpuBus, Z80};

type StateData = [u16; 25];

struct Vector {
    name: &'static str,
    initial: StateData,
    expected: StateData,
    initial_ram: &'static [(u16, u8)],
    final_ram: &'static [(u16, u8)],
    tstates: usize,
}

struct FlatBus {
    memory: Box<[u8]>,
}
impl Default for FlatBus {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x1_0000].into_boxed_slice(),
        }
    }
}
impl CpuBus for FlatBus {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }
    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }
}

fn apply_state(s: StateData, cpu: &mut Z80) {
    cpu.registers.pc = s[0];
    cpu.registers.sp = s[1];
    cpu.registers.a = s[2] as u8;
    cpu.registers.f = s[3] as u8;
    cpu.registers.b = s[4] as u8;
    cpu.registers.c = s[5] as u8;
    cpu.registers.d = s[6] as u8;
    cpu.registers.e = s[7] as u8;
    cpu.registers.h = s[8] as u8;
    cpu.registers.l = s[9] as u8;
    cpu.i = s[10] as u8;
    cpu.r = s[11] as u8;
    cpu.ix = s[12];
    cpu.iy = s[13];
    cpu.wz = s[14];
    [cpu.alternate.a, cpu.alternate.f] = s[15].to_be_bytes();
    [cpu.alternate.b, cpu.alternate.c] = s[16].to_be_bytes();
    [cpu.alternate.d, cpu.alternate.e] = s[17].to_be_bytes();
    [cpu.alternate.h, cpu.alternate.l] = s[18].to_be_bytes();
    cpu.interrupt_mode = s[19] as u8;
    cpu.p = s[20] != 0;
    cpu.q = s[21] as u8;
    cpu.iff1 = s[22] != 0;
    cpu.iff2 = s[23] != 0;
    cpu.ei_delay = s[24] as u8;
}

fn assert_state(s: StateData, cpu: &Z80, name: &str) {
    let actual: StateData = [
        cpu.registers.pc, cpu.registers.sp,
        u16::from(cpu.registers.a), u16::from(cpu.registers.f),
        u16::from(cpu.registers.b), u16::from(cpu.registers.c),
        u16::from(cpu.registers.d), u16::from(cpu.registers.e),
        u16::from(cpu.registers.h), u16::from(cpu.registers.l),
        u16::from(cpu.i), u16::from(cpu.r), cpu.ix, cpu.iy, cpu.wz,
        u16::from_be_bytes([cpu.alternate.a, cpu.alternate.f]),
        u16::from_be_bytes([cpu.alternate.b, cpu.alternate.c]),
        u16::from_be_bytes([cpu.alternate.d, cpu.alternate.e]),
        u16::from_be_bytes([cpu.alternate.h, cpu.alternate.l]),
        u16::from(cpu.interrupt_mode), u16::from(cpu.p), u16::from(cpu.q),
        u16::from(cpu.iff1), u16::from(cpu.iff2), u16::from(cpu.ei_delay),
    ];
    let labels = [
        "PC", "SP", "A", "F", "B", "C", "D", "E", "H", "L", "I", "R", "IX", "IY", "WZ", "AF'",
        "BC'", "DE'", "HL'", "IM", "P", "Q", "IFF1", "IFF2", "EI delay",
    ];
    for i in 0..s.len() {
        assert_eq!(actual[i], s[i], "{name}: {}", labels[i]);
    }
}
#[test]
fn singlestep_expanded_opcode_matrix_and_memory_effects() {
    let vectors: &[Vector] = &[
        Vector {
            name: "01 0000",
            initial: [
                58878, 12870, 81, 243, 100, 162, 165, 189, 149, 112, 9, 86, 25547, 30450, 11589,
                48331, 14039, 64413, 36944, 2, 0, 0, 0, 1, 1
            ],
            expected: [
                58881, 12870, 81, 243, 16, 222, 165, 189, 149, 112, 9, 87, 25547, 30450, 11589,
                48331, 14039, 64413, 36944, 2, 0, 0, 0, 1, 0
            ],
            initial_ram: &[(58878, 1), (58879, 222), (58880, 16)],
            final_ram: &[(58878, 1), (58879, 222), (58880, 16)],
            tstates: 10,
        },
        Vector {
            name: "02 0000",
            initial: [
                17818, 16123, 162, 158, 138, 30, 250, 254, 4, 181, 223, 15, 52317, 54730, 41029,
                799, 25204, 54176, 40653, 0, 0, 0, 1, 1, 0
            ],
            expected: [
                17819, 16123, 162, 158, 138, 30, 250, 254, 4, 181, 223, 16, 52317, 54730, 41503,
                799, 25204, 54176, 40653, 0, 0, 0, 1, 1, 0
            ],
            initial_ram: &[(17818, 2), (35358, 0)],
            final_ram: &[(17818, 2), (35358, 162)],
            tstates: 7,
        },
        Vector {
            name: "03 0000",
            initial: [
                49030, 44097, 151, 13, 165, 100, 162, 33, 66, 2, 27, 20, 43006, 9306, 35883,
                19229, 42601, 10181, 34259, 1, 0, 0, 1, 0, 1
            ],
            expected: [
                49031, 44097, 151, 13, 165, 101, 162, 33, 66, 2, 27, 21, 43006, 9306, 35883,
                19229, 42601, 10181, 34259, 1, 0, 0, 1, 0, 0
            ],
            initial_ram: &[(49030, 3)],
            final_ram: &[(49030, 3)],
            tstates: 6,
        },
        Vector {
            name: "04 0000",
            initial: [
                39227, 41151, 125, 11, 86, 182, 23, 236, 167, 96, 215, 25, 65480, 59040, 31428,
                17994, 5391, 45317, 1504, 2, 1, 11, 0, 0, 1
            ],
            expected: [
                39228, 41151, 125, 1, 87, 182, 23, 236, 167, 96, 215, 26, 65480, 59040, 31428,
                17994, 5391, 45317, 1504, 2, 0, 1, 0, 0, 0
            ],
            initial_ram: &[(39227, 4)],
            final_ram: &[(39227, 4)],
            tstates: 4,
        },
        Vector {
            name: "07 0000",
            initial: [
                9459, 54537, 165, 8, 20, 74, 6, 179, 238, 93, 70, 103, 35691, 54114, 57683,
                61754, 44302, 36521, 57927, 2, 0, 8, 1, 0, 1
            ],
            expected: [
                9460, 54537, 75, 9, 20, 74, 6, 179, 238, 93, 70, 104, 35691, 54114, 57683,
                61754, 44302, 36521, 57927, 2, 0, 9, 1, 0, 0
            ],
            initial_ram: &[(9459, 7)],
            final_ram: &[(9459, 7)],
            tstates: 4,
        },
        Vector {
            name: "09 0000",
            initial: [
                42699, 11409, 29, 75, 97, 243, 114, 248, 176, 21, 91, 126, 60906, 11221, 40062,
                12737, 34283, 31584, 20838, 1, 1, 0, 1, 1, 0
            ],
            expected: [
                42700, 11409, 29, 65, 97, 243, 114, 248, 18, 8, 91, 127, 60906, 11221, 45078,
                12737, 34283, 31584, 20838, 1, 0, 65, 1, 1, 0
            ],
            initial_ram: &[(42699, 9)],
            final_ram: &[(42699, 9)],
            tstates: 11,
        },
        Vector {
            name: "0A 0000",
            initial: [
                16826, 9383, 64, 79, 95, 205, 147, 168, 98, 251, 60, 66, 14528, 18788, 56058,
                48718, 45204, 42805, 59464, 0, 1, 0, 1, 1, 0
            ],
            expected: [
                16827, 9383, 174, 79, 95, 205, 147, 168, 98, 251, 60, 67, 14528, 18788, 24526,
                48718, 45204, 42805, 59464, 0, 0, 0, 1, 1, 0
            ],
            initial_ram: &[(16826, 10), (24525, 174)],
            final_ram: &[(16826, 10), (24525, 174)],
            tstates: 7,
        },
        Vector {
            name: "18 0000",
            initial: [
                24335, 62307, 55, 105, 96, 251, 252, 147, 160, 108, 236, 21, 51617, 49375, 47658,
                48678, 18572, 24070, 39501, 2, 0, 0, 1, 0, 0
            ],
            expected: [
                24390, 62307, 55, 105, 96, 251, 252, 147, 160, 108, 236, 22, 51617, 49375, 24390,
                48678, 18572, 24070, 39501, 2, 0, 0, 1, 0, 0
            ],
            initial_ram: &[(24335, 24), (24336, 53)],
            final_ram: &[(24335, 24), (24336, 53)],
            tstates: 12,
        },
        Vector {
            name: "21 0000",
            initial: [
                8031, 27621, 207, 10, 77, 52, 86, 32, 217, 37, 207, 116, 2297, 18591, 3144,
                26417, 8397, 22039, 57800, 2, 0, 0, 1, 1, 1
            ],
            expected: [
                8034, 27621, 207, 10, 77, 52, 86, 32, 143, 62, 207, 117, 2297, 18591, 3144,
                26417, 8397, 22039, 57800, 2, 0, 0, 1, 1, 0
            ],
            initial_ram: &[(8031, 33), (8032, 62), (8033, 143)],
            final_ram: &[(8031, 33), (8032, 62), (8033, 143)],
            tstates: 10,
        },
        Vector {
            name: "22 0000",
            initial: [
                59422, 22503, 178, 59, 1, 132, 215, 50, 235, 11, 88, 93, 50140, 50309, 14133,
                57183, 14150, 7311, 59355, 0, 1, 59, 1, 1, 1
            ],
            expected: [
                59425, 22503, 178, 59, 1, 132, 215, 50, 235, 11, 88, 94, 50140, 50309, 51554,
                57183, 14150, 7311, 59355, 0, 0, 0, 1, 1, 0
            ],
            initial_ram: &[(51553, 0), (51554, 0), (59422, 34), (59423, 97), (59424, 201)],
            final_ram: &[(51553, 11), (51554, 235), (59422, 34), (59423, 97), (59424, 201)],
            tstates: 16,
        },
        Vector {
            name: "34 0000",
            initial: [
                19604, 57090, 242, 171, 191, 212, 100, 178, 141, 16, 148, 94, 30199, 49621, 42631,
                23134, 64595, 56341, 44798, 1, 1, 171, 0, 0, 1
            ],
            expected: [
                19605, 57090, 242, 9, 191, 212, 100, 178, 141, 16, 148, 95, 30199, 49621, 42631,
                23134, 64595, 56341, 44798, 1, 0, 9, 0, 0, 0
            ],
            initial_ram: &[(19604, 52), (36112, 94)],
            final_ram: &[(19604, 52), (36112, 95)],
            tstates: 11,
        },
        Vector {
            name: "36 0000",
            initial: [
                13988, 12173, 210, 160, 74, 36, 48, 105, 10, 26, 163, 2, 31842, 47452, 32050,
                3224, 7645, 14489, 10624, 2, 0, 0, 1, 0, 0
            ],
            expected: [
                13990, 12173, 210, 160, 74, 36, 48, 105, 10, 26, 163, 3, 31842, 47452, 32050,
                3224, 7645, 14489, 10624, 2, 0, 0, 1, 0, 0
            ],
            initial_ram: &[(2586, 0), (13988, 54), (13989, 254)],
            final_ram: &[(2586, 254), (13988, 54), (13989, 254)],
            tstates: 10,
        },
        Vector {
            name: "3A 0000",
            initial: [
                51452, 16857, 65, 174, 227, 62, 128, 31, 86, 26, 110, 12, 64314, 4930, 41726,
                13830, 17137, 32751, 50593, 0, 1, 174, 1, 1, 0
            ],
            expected: [
                51455, 16857, 10, 174, 227, 62, 128, 31, 86, 26, 110, 13, 64314, 4930, 4101,
                13830, 17137, 32751, 50593, 0, 0, 0, 1, 1, 0
            ],
            initial_ram: &[(4100, 10), (51452, 58), (51453, 4), (51454, 16)],
            final_ram: &[(4100, 10), (51452, 58), (51453, 4), (51454, 16)],
            tstates: 13,
        },
        Vector {
            name: "ED A0 0000",
            initial: [
                52793, 2396, 74, 243, 237, 207, 218, 67, 231, 101, 162, 20, 30062, 9194, 21852,
                40850, 36044, 46518, 61744, 0, 0, 243, 1, 1, 1
            ],
            expected: [
                52795, 2396, 74, 229, 237, 206, 218, 68, 231, 102, 162, 22, 30062, 9194, 21852,
                40850, 36044, 46518, 61744, 0, 0, 229, 1, 1, 0
            ],
            initial_ram: &[(52793, 237), (52794, 160), (55875, 0), (59237, 232)],
            final_ram: &[(52793, 237), (52794, 160), (55875, 232), (59237, 232)],
            tstates: 16,
        },
        Vector {
            name: "ED B0 0000",
            initial: [
                11557, 4041, 178, 34, 152, 234, 91, 84, 208, 123, 116, 115, 3003, 63854, 45431,
                22811, 42083, 24379, 63981, 2, 1, 34, 1, 0, 0
            ],
            expected: [
                11557, 4041, 178, 44, 152, 233, 91, 85, 208, 124, 116, 117, 3003, 63854, 11558,
                22811, 42083, 24379, 63981, 2, 0, 44, 1, 0, 0
            ],
            initial_ram: &[(11557, 237), (11558, 176), (23380, 0), (53371, 137)],
            final_ram: &[(11557, 237), (11558, 176), (23380, 137), (53371, 137)],
            tstates: 21,
        },
        Vector {
            name: "DD 21 0000",
            initial: [
                10017, 36761, 250, 127, 150, 193, 17, 185, 114, 72, 1, 31, 21557, 55124, 61761,
                50243, 5300, 55419, 59805, 1, 0, 0, 1, 1, 1
            ],
            expected: [
                10021, 36761, 250, 127, 150, 193, 17, 185, 114, 72, 1, 33, 49027, 55124, 61761,
                50243, 5300, 55419, 59805, 1, 0, 0, 1, 1, 0
            ],
            initial_ram: &[(10017, 221), (10018, 33), (10019, 131), (10020, 191)],
            final_ram: &[(10017, 221), (10018, 33), (10019, 131), (10020, 191)],
            tstates: 14,
        },
        Vector {
            name: "FD 21 0000",
            initial: [
                34183, 302, 161, 112, 222, 19, 88, 209, 133, 176, 141, 52, 34704, 22859, 61920,
                60349, 3274, 54911, 56768, 2, 1, 112, 0, 0, 0
            ],
            expected: [
                34187, 302, 161, 112, 222, 19, 88, 209, 133, 176, 141, 54, 34704, 8504, 61920,
                60349, 3274, 54911, 56768, 2, 0, 0, 0, 0, 0
            ],
            initial_ram: &[(34183, 253), (34184, 33), (34185, 56), (34186, 33)],
            final_ram: &[(34183, 253), (34184, 33), (34185, 56), (34186, 33)],
            tstates: 14,
        },
        Vector {
            name: "CB 06 0000",
            initial: [
                55186, 8975, 32, 108, 197, 214, 98, 106, 140, 155, 46, 16, 55033, 50548, 46301,
                64022, 22006, 59909, 13657, 2, 1, 0, 1, 1, 0
            ],
            expected: [
                55188, 8975, 32, 40, 197, 214, 98, 106, 140, 155, 46, 18, 55033, 50548, 46301,
                64022, 22006, 59909, 13657, 2, 0, 40, 1, 1, 0
            ],
            initial_ram: &[(35995, 21), (55186, 203), (55187, 6)],
            final_ram: &[(35995, 42), (55186, 203), (55187, 6)],
            tstates: 15,
        },
        Vector {
            name: "CB 7E 0000",
            initial: [
                22767, 6835, 156, 230, 50, 38, 159, 141, 87, 177, 38, 126, 61281, 19445, 52271,
                48461, 40642, 15498, 24222, 0, 1, 0, 0, 1, 1
            ],
            expected: [
                22769, 6835, 156, 92, 50, 38, 159, 141, 87, 177, 38, 0, 61281, 19445, 52271,
                48461, 40642, 15498, 24222, 0, 0, 92, 0, 1, 0
            ],
            initial_ram: &[(22449, 97), (22767, 203), (22768, 126)],
            final_ram: &[(22449, 97), (22767, 203), (22768, 126)],
            tstates: 12,
        },
    ];
    for vector in vectors {
        let mut bus = FlatBus::default();
        for &(address, value) in vector.initial_ram {
            bus.memory[usize::from(address)] = value;
        }
        let mut cpu = Z80::default();
        apply_state(vector.initial, &mut cpu);
        assert_eq!(
            cpu.step(&mut bus),
            Ok(vector.tstates as u32),
            "{}: T-states",
            vector.name
        );
        assert_eq!(
            cpu.total_tstates,
            vector.tstates as u64,
            "{}: total T-states",
            vector.name
        );
        assert_state(vector.expected, &cpu, vector.name);
        for &(address, expected) in vector.final_ram {
            assert_eq!(
                bus.memory[usize::from(address)],
                expected,
                "{}: RAM[{address:#06x}]",
                vector.name
            );
        }
    }
}
