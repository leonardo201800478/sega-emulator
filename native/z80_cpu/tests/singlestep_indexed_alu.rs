//! Pinned external SingleStepTests coverage for the shared, resumable Z80.
//!
//! Upstream: https://github.com/SingleStepTests/z80
//! Revision: ebe1875d48f374bcfd4b505d8eb8ee751568b5f7 (MIT).
//! The corpus is checked out by the z80-focused CI job. This test intentionally
//! runs only through the explicit --ignored CI invocation so ordinary local
//! cargo test runs do not silently depend on network access.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use sega_z80_cpu::{
    Z80AlternateRegisters, Z80BusOperation, Z80BusRequest, Z80MicroSequencer, Z80MicroStep,
    Z80Registers,
};

const UPSTREAM_REVISION: &str = "ebe1875d48f374bcfd4b505d8eb8ee751568b5f7";
const OPCODES: [u8; 16] = [
    0x84, 0x85, 0x8C, 0x8D, 0x94, 0x95, 0x9C, 0x9D, 0xA4, 0xA5, 0xAC, 0xAD, 0xB4, 0xB5,
    0xBC, 0xBD,
];

const FIXTURES: [(&str, u8, &str); 32] = [
    ("dd", 0x84, "9d9b002ef4b7e05fe290c71bbd87b0a622411040"),
    ("dd", 0x85, "60e745c68c9560799c240a0d8583fdde331f1fb2"),
    ("dd", 0x8C, "b306f9ebfaa88322ac60ff4ec64f9b71c3aa9a55"),
    ("dd", 0x8D, "0b7d4e0a7b599399ca3d5b3aca64d18f1e2ab678"),
    ("dd", 0x94, "777f25a5e20c9f0a5fe532e52982a29a888f6f73"),
    ("dd", 0x95, "83623f50960d9bf1feec5a39552c183da7aba520"),
    ("dd", 0x9C, "02d6df95b142cf4baecca039fb794bc7ff4c9692"),
    ("dd", 0x9D, "058cbb0a541e29557fcdf99c8e67f1aaafa42dc8"),
    ("dd", 0xA4, "bca6b0ade586ae24f86b236ef4674b0dc8e67cbb"),
    ("dd", 0xA5, "0feb3576fb14f6a1bcc2c32cd25c05e92ecb8565"),
    ("dd", 0xAC, "82920d0f208cdffc74bca67a146909590e6052df"),
    ("dd", 0xAD, "1a0cbd1af7e97880a15eafd8bb80376bd9b0818f"),
    ("dd", 0xB4, "970b214b8f514f5841927d741ee3637f154625d0"),
    ("dd", 0xB5, "ad895f9f596014a99e6268c94fa60f90a876c167"),
    ("dd", 0xBC, "252cec202c8391d57758e61f3873ae227245978d"),
    ("dd", 0xBD, "a97a7ad27e6203c0585334b0e900e312862db5c3"),
    ("fd", 0x84, "43a2ecc73f122d5c8b4e4c18e13edcf57968f189"),
    ("fd", 0x85, "0568bcd3cc5d0332130a4a81be1808ede272461f"),
    ("fd", 0x8C, "a163e17a0c4612ae3404f3e9708601850e2fd050"),
    ("fd", 0x8D, "6c0f42dde50ebb899bc4c625f1edc68f1c442cd6"),
    ("fd", 0x94, "b468591bd7c5b7edd2b291cd755065c92a91fdc3"),
    ("fd", 0x95, "e929b84476a724a1215b1d6681e4c7126b141dd8"),
    ("fd", 0x9C, "329b92c30dbb61f5bee586a17db5d7cacbee4357"),
    ("fd", 0x9D, "ee7dad056b01225d856c8e4edf61efe7872d0731"),
    ("fd", 0xA4, "fdce6084a234a7fb745aef43d4da8521bdead5d5"),
    ("fd", 0xA5, "b887386ccd3ba9462f6ba5f751f1a2612085b43d"),
    ("fd", 0xAC, "a4bededc713e86d21a02db4102088a5ae57ebeb3"),
    ("fd", 0xAD, "8c8ebf197bd234404725f456451ac96963e4f918"),
    ("fd", 0xB4, "0573fb238e601e5502f55e7e40a65eba257e4482"),
    ("fd", 0xB5, "fc4a5ed76439393c804298975bed7044aa1a4472"),
    ("fd", 0xBC, "060fd80237ce9b71a2b981973b7ecef38c29a4af"),
    ("fd", 0xBD, "98199f836074844f2d121e3abd1c4add644ccef8"),
];

#[derive(Debug, Deserialize)]
struct CpuState {
    pc: u16,
    sp: u16,
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    i: u8,
    r: u8,
    ei: u8,
    wz: u16,
    ix: u16,
    iy: u16,
    af_: u16,
    bc_: u16,
    de_: u16,
    hl_: u16,
    im: u8,
    p: u8,
    q: u8,
    iff1: u8,
    iff2: u8,
    ram: Vec<(u16, u8)>,
}

#[derive(Debug, Deserialize)]
struct TestVector {
    name: String,
    initial: CpuState,
    #[serde(rename = "final")]
    final_state: CpuState,
    cycles: Vec<serde_json::Value>,
}

fn command_output(command: &mut Command, description: &str) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{description}: failed to start: {error}"));
    assert!(
        output.status.success(),
        "{description}: command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("{description}: non-UTF-8 output: {error}"))
        .trim()
        .to_owned()
}

fn assert_upstream_checkout(corpus_dir: &Path) -> PathBuf {
    let repository_root = corpus_dir
        .parent()
        .expect("SINGLESTEP_Z80_DIR must point to the upstream v1 directory");
    let revision = command_output(
        Command::new("git")
            .arg("-C")
            .arg(repository_root)
            .args(["rev-parse", "HEAD"]),
        "read pinned SingleStepTests revision",
    );
    assert_eq!(revision, UPSTREAM_REVISION, "unexpected upstream revision");

    let license = fs::read_to_string(repository_root.join("LICENSE"))
        .expect("upstream MIT LICENSE must be checked out");
    assert!(license.contains("MIT License"));
    assert!(license.contains("Copyright (c) 2024 SingleStepTests"));

    repository_root.to_path_buf()
}

fn assert_fixture_blob(repository_root: &Path, fixture_path: &Path, expected_blob_sha1: &str) {
    let actual = command_output(
        Command::new("git")
            .arg("-C")
            .arg(repository_root)
            .arg("hash-object")
            .arg(fixture_path),
        "hash pinned SingleStepTests fixture",
    );
    assert_eq!(
        actual, expected_blob_sha1,
        "fixture content hash mismatch: {}",
        fixture_path.display()
    );
}

fn byte_at(memory: &BTreeMap<u16, u8>, address: u16, name: &str) -> u8 {
    memory
        .get(&address)
        .copied()
        .unwrap_or_else(|| panic!("{name}: missing opcode byte at {address:04X}"))
}

fn opcode_fetch(step: Z80MicroStep, address: u16, name: &str) -> Z80BusRequest {
    let request = match step {
        Z80MicroStep::BusRequest(request) => request,
        other => panic!("{name}: expected opcode fetch request, got {other:?}"),
    };
    assert_eq!(request.operation, Z80BusOperation::OpcodeFetch, "{name}");
    assert_eq!(request.address, address, "{name}: fetch address");
    assert_eq!(request.base_tstates, 4, "{name}: M1 duration");
    request
}

fn seed_microsequencer(initial: &CpuState) -> Z80MicroSequencer {
    let mut cpu = Z80MicroSequencer::default();
    cpu.registers = Z80Registers {
        a: initial.a,
        f: initial.f,
        b: initial.b,
        c: initial.c,
        d: initial.d,
        e: initial.e,
        h: initial.h,
        l: initial.l,
        sp: initial.sp,
        pc: initial.pc,
    };
    cpu.alternate = Z80AlternateRegisters {
        a: (initial.af_ >> 8) as u8,
        f: initial.af_ as u8,
        b: (initial.bc_ >> 8) as u8,
        c: initial.bc_ as u8,
        d: (initial.de_ >> 8) as u8,
        e: initial.de_ as u8,
        h: (initial.hl_ >> 8) as u8,
        l: initial.hl_ as u8,
    };
    cpu.ix = initial.ix;
    cpu.iy = initial.iy;
    cpu.i = initial.i;
    cpu.refresh = initial.r;
    cpu.iff1 = initial.iff1 != 0;
    cpu.iff2 = initial.iff2 != 0;
    cpu.interrupt_mode = initial.im;
    cpu.wz = initial.wz;
    cpu.ei_delay = initial.ei;
    cpu.p = initial.p != 0;
    cpu.q = initial.q;
    cpu
}

fn assert_architectural_state(cpu: &Z80MicroSequencer, expected: &CpuState, name: &str) {
    assert_eq!(cpu.registers.pc, expected.pc, "{name}: PC");
    assert_eq!(cpu.registers.sp, expected.sp, "{name}: SP");
    assert_eq!(cpu.registers.a, expected.a, "{name}: A");
    assert_eq!(cpu.registers.f, expected.f, "{name}: F");
    assert_eq!(cpu.registers.b, expected.b, "{name}: B");
    assert_eq!(cpu.registers.c, expected.c, "{name}: C");
    assert_eq!(cpu.registers.d, expected.d, "{name}: D");
    assert_eq!(cpu.registers.e, expected.e, "{name}: E");
    assert_eq!(cpu.registers.h, expected.h, "{name}: H");
    assert_eq!(cpu.registers.l, expected.l, "{name}: L");
    assert_eq!(cpu.i, expected.i, "{name}: I");
    assert_eq!(cpu.refresh, expected.r, "{name}: R");
    assert_eq!(cpu.ix, expected.ix, "{name}: IX");
    assert_eq!(cpu.iy, expected.iy, "{name}: IY");
    assert_eq!(cpu.wz, expected.wz, "{name}: WZ/MEMPTR");
    assert_eq!(cpu.alternate.a, (expected.af_ >> 8) as u8, "{name}: A'");
    assert_eq!(cpu.alternate.f, expected.af_ as u8, "{name}: F'");
    assert_eq!(cpu.alternate.b, (expected.bc_ >> 8) as u8, "{name}: B'");
    assert_eq!(cpu.alternate.c, expected.bc_ as u8, "{name}: C'");
    assert_eq!(cpu.alternate.d, (expected.de_ >> 8) as u8, "{name}: D'");
    assert_eq!(cpu.alternate.e, expected.de_ as u8, "{name}: E'");
    assert_eq!(cpu.alternate.h, (expected.hl_ >> 8) as u8, "{name}: H'");
    assert_eq!(cpu.alternate.l, expected.hl_ as u8, "{name}: L'");
    assert_eq!(cpu.interrupt_mode, expected.im, "{name}: IM");
    assert_eq!(cpu.iff1, expected.iff1 != 0, "{name}: IFF1");
    assert_eq!(cpu.iff2, expected.iff2 != 0, "{name}: IFF2");
    assert_eq!(cpu.ei_delay, expected.ei, "{name}: EI delay");
    assert_eq!(cpu.p, expected.p != 0, "{name}: P latch");
    assert_eq!(cpu.q, expected.q, "{name}: Q latch");
}

fn run_vector(vector: &TestVector) {
    let name = vector.name.as_str();
    let memory: BTreeMap<u16, u8> = vector.initial.ram.iter().copied().collect();
    let mut cpu = seed_microsequencer(&vector.initial);

    let first_fetch = opcode_fetch(
        cpu.begin_instruction().unwrap(),
        vector.initial.pc,
        name,
    );
    let first_byte = byte_at(&memory, first_fetch.address, name);
    let second_step = cpu
        .complete_bus_cycle(first_fetch.id, Some(first_byte))
        .unwrap();

    let second_fetch = opcode_fetch(
        second_step,
        vector.initial.pc.wrapping_add(1),
        name,
    );
    let second_byte = byte_at(&memory, second_fetch.address, name);
    let retired = cpu
        .complete_bus_cycle(second_fetch.id, Some(second_byte))
        .unwrap();

    let (retired_opcode, tstates) = match retired {
        Z80MicroStep::InstructionRetired { opcode, tstates } => (opcode, tstates),
        other => panic!("{name}: expected retirement after two M1 fetches, got {other:?}"),
    };
    assert_eq!(retired_opcode, second_byte, "{name}: terminal opcode");
    let expected_tstates = u32::try_from(vector.cycles.len()).unwrap();
    assert_eq!(expected_tstates, 8, "{name}: upstream nominal duration");
    assert_eq!(tstates, expected_tstates, "{name}: aggregate T-states");

    assert_architectural_state(&cpu, &vector.final_state, name);
    let final_memory: BTreeMap<u16, u8> = vector.final_state.ram.iter().copied().collect();
    assert_eq!(memory, final_memory, "{name}: register-only ALU memory effects");
}

#[test]
#[ignore = "requires the pinned SingleStepTests checkout; run explicitly in z80-focused CI"]
fn all_32000_dd_fd_indexed_half_register_alu_vectors_match_resumable_z80() {
    let corpus_dir = std::env::var_os("SINGLESTEP_Z80_DIR")
        .map(PathBuf::from)
        .expect("SINGLESTEP_Z80_DIR must point to the pinned upstream v1 directory");
    let repository_root = assert_upstream_checkout(&corpus_dir);
    let mut total_vectors = 0usize;

    for (prefix, opcode, expected_blob_sha1) in FIXTURES {
        assert!(OPCODES.contains(&opcode));
        let filename = format!("{prefix} {opcode:02x}.json");
        let fixture_path = corpus_dir.join(&filename);
        assert_fixture_blob(&repository_root, &fixture_path, expected_blob_sha1);

        let json = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", fixture_path.display()));
        let vectors: Vec<TestVector> = serde_json::from_str(&json)
            .unwrap_or_else(|error| panic!("parse {}: {error}", fixture_path.display()));
        assert_eq!(
            vectors.len(),
            1_000,
            "{} must contain exactly 1,000 vectors",
            fixture_path.display()
        );

        for vector in &vectors {
            run_vector(vector);
            total_vectors += 1;
        }
    }

    assert_eq!(total_vectors, 32_000);
}
