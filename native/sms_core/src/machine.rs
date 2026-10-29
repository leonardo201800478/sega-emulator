//! SMS CPU/memory/I/O integration boundary.
//!
//! This is an instruction-stepping machine shell, not a complete console:
//! VDP, PSG/FM, interrupt generation, and a frame scheduler remain absent.

use crate::SmsCoreError;
use crate::bus::CpuBus;
use crate::cartridge::CartridgeImage;
use crate::config::SmsMachineConfig;
use crate::cpu::{CpuError, Z80};
use crate::input::{ControllerButtons, SmsInputState};
use crate::io::{SmsIo, SmsIoError};
use crate::memory::SmsMemory;

/// Lens selection latched by writes to the SegaScope address range $FFF8-$FFFB.
///
/// It reports the hardware latch only; stereoscopic image generation and
/// frame-synchronized host shutter output require the future VDP/scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SegaScopeLens {
    #[default]
    Right,
    Left,
}

/// Observable state of the SegaScope left/right shutter control latch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SegaScopeState {
    lens: SegaScopeLens,
}

impl SegaScopeState {
    pub fn lens(&self) -> SegaScopeLens {
        self.lens
    }

    fn write_latch(&mut self, value: u8) {
        self.lens = if value & 1 == 0 {
            SegaScopeLens::Right
        } else {
            SegaScopeLens::Left
        };
    }
}

/// SMS-owned implementation of the hardware-neutral Z80 memory/I/O contract.
pub struct SmsMachineBus<'a> {
    memory: &'a mut SmsMemory,
    io: &'a mut SmsIo,
    sega_scope: &'a mut SegaScopeState,
}

impl<'a> SmsMachineBus<'a> {
    pub fn new(
        memory: &'a mut SmsMemory,
        io: &'a mut SmsIo,
        sega_scope: &'a mut SegaScopeState,
    ) -> Self {
        Self {
            memory,
            io,
            sega_scope,
        }
    }
}

impl CpuBus for SmsMachineBus<'_> {
    fn read_memory(&mut self, address: u16) -> u8 {
        self.memory.read(address)
    }

    fn write_memory(&mut self, address: u16, value: u8) {
        self.memory.write(address, value);
        if (0xFFF8..=0xFFFB).contains(&address) {
            self.sega_scope.write_latch(value);
        }
    }

    fn read_port(&mut self, port: u16) -> u8 {
        self.io.read_port(port)
    }

    fn write_port(&mut self, port: u16, value: u8) {
        self.io.write_port(port, value);
    }
}

/// Instruction-stepping SMS core with cartridge mapper, work RAM and controller I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsMachine {
    cpu: Z80,
    memory: SmsMemory,
    io: SmsIo,
    sega_scope: SegaScopeState,
    config: SmsMachineConfig,
}

impl SmsMachine {
    /// Construct a standard-mapper SMS machine using the default export/NTSC profile.
    pub fn new(rom: impl Into<Vec<u8>>) -> Result<Self, SmsCoreError> {
        Self::with_config(rom, SmsMachineConfig::default())
    }

    pub fn with_config(
        rom: impl Into<Vec<u8>>,
        config: SmsMachineConfig,
    ) -> Result<Self, SmsCoreError> {
        let cartridge = CartridgeImage::new(rom)?;
        let memory = SmsMemory::new(cartridge)?;
        let mut cpu = Z80::default();
        cpu.reset();
        Ok(Self {
            cpu,
            memory,
            io: SmsIo::new(config.territory),
            sega_scope: SegaScopeState::default(),
            config,
        })
    }

    /// Execute one Z80 instruction through the SMS-owned memory and I/O bus.
    ///
    /// The returned T-state count is nominal instruction timing, not cycle-exact
    /// machine timing.
    pub fn step(&mut self) -> Result<u32, CpuError> {
        let mut bus = SmsMachineBus::new(&mut self.memory, &mut self.io, &mut self.sega_scope);
        self.cpu.step(&mut bus)
    }

    pub fn reset_cpu(&mut self) {
        self.cpu.reset();
    }

    pub fn set_controller(
        &mut self,
        player: u8,
        buttons: ControllerButtons,
    ) -> Result<(), SmsIoError> {
        self.io.set_controller(player, buttons)
    }

    pub fn set_input(&mut self, input: SmsInputState) -> Result<(), SmsIoError> {
        self.io.set_input(input)
    }

    pub fn set_light_phaser_player(&mut self, player: Option<u8>) -> Result<(), SmsIoError> {
        self.io.set_light_phaser_player(player)
    }

    pub fn cpu(&self) -> &Z80 {
        &self.cpu
    }

    pub fn memory(&self) -> &SmsMemory {
        &self.memory
    }

    pub fn io(&self) -> &SmsIo {
        &self.io
    }

    pub fn sega_scope(&self) -> SegaScopeState {
        self.sega_scope
    }

    pub fn config(&self) -> &SmsMachineConfig {
        &self.config
    }

    /// Debug/test access routed through the same memory map as the CPU.
    pub fn read_memory(&self, address: u16) -> u8 {
        self.memory.read(address)
    }

    /// Debug/test access routed through the same I/O decoder as the CPU.
    pub fn read_port(&self, port: u16) -> u8 {
        self.io.read_port(port)
    }
}

#[cfg(test)]
mod tests {
    use super::{SegaScopeLens, SmsMachine};
    use crate::input::{ControllerButtons, SmsInputState};

    #[test]
    fn cpu_fetches_cartridge_rom_and_uses_the_same_mapper_memory_for_ram() {
        // LD A,$42; LD ($C000),A; LD A,($C000); LD ($C001),A
        let rom = [
            0x3E, 0x42, 0x32, 0x00, 0xC0, 0x3A, 0x00, 0xC0, 0x32, 0x01, 0xC0,
        ];
        let mut machine = SmsMachine::new(rom.to_vec()).unwrap();
        for _ in 0..4 {
            machine.step().unwrap();
        }
        assert_eq!(machine.cpu().registers.a, 0x42);
        assert_eq!(machine.read_memory(0xC000), 0x42);
        assert_eq!(machine.read_memory(0xC001), 0x42);
    }

    #[test]
    fn cpu_in_instruction_reads_sms_controller_port() {
        // IN A,($DC) reads (A << 8) | $DC; decoder uses the low byte.
        let rom = [0xDB, 0xDC];
        let mut machine = SmsMachine::new(rom.to_vec()).unwrap();
        machine
            .set_controller(
                1,
                ControllerButtons {
                    up: true,
                    ..Default::default()
                },
            )
            .unwrap();
        machine.step().unwrap();
        assert_eq!(machine.cpu().registers.a & 1, 0);
    }

    #[test]
    fn mouse_trigger_reaches_the_same_active_low_sms_button_bit() {
        let mut machine = SmsMachine::new(vec![0x00]).unwrap();
        let mut input = SmsInputState::default();
        input.set_light_phaser_aim(100, 70, true);
        machine.set_input(input).unwrap();
        machine.set_light_phaser_player(Some(1)).unwrap();
        assert_eq!(machine.read_port(0xDC) & 0x10, 0);
    }

    #[test]
    fn sega_scope_latch_observes_cpu_memory_writes_without_corrupting_ram_mirror() {
        // LD A,$01; LD ($FFF8),A
        let mut machine = SmsMachine::new(vec![0x3E, 0x01, 0x32, 0xF8, 0xFF]).unwrap();
        machine.step().unwrap();
        machine.step().unwrap();
        assert_eq!(machine.sega_scope().lens(), SegaScopeLens::Left);
        assert_eq!(machine.read_memory(0xFFF8), 0x01);
        assert_eq!(machine.read_memory(0xDFF8), 0x01);
    }
}
