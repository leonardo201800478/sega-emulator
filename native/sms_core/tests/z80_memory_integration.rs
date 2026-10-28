use sega_sms_core::{CartridgeImage, SmsMemory, Z80};

#[test]
fn native_z80_executes_cartridge_code_and_accesses_sms_work_ram() {
    let mut rom = vec![0x00; 0x4000];
    rom[..9].copy_from_slice(&[
        0x3E, 0x42,       // LD A,0x42
        0x32, 0x00, 0xC0, // LD (0xC000),A
        0x3A, 0x00, 0xC0, // LD A,(0xC000)
        0x76,             // HALT
    ]);
    let cartridge = CartridgeImage::new(rom).expect("synthetic cartridge should be valid");
    let mut memory = SmsMemory::new(cartridge).expect("standard mapper should accept ROM");
    let mut cpu = Z80::default();

    assert_eq!(cpu.step(&mut memory), Ok(7));
    assert_eq!(cpu.step(&mut memory), Ok(13));
    assert_eq!(cpu.step(&mut memory), Ok(13));
    assert_eq!(cpu.registers.a, 0x42);
    assert_eq!(memory.read_memory(0xC000), 0x42);
    assert_eq!(cpu.step(&mut memory), Ok(4));
    assert!(cpu.halted);
}
