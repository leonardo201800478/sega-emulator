//! SMS CPU/memory/I/O integration boundary.
//!
//! CPU, cartridge, controller I/O, scanline VDP, and SN76489 PSG are integrated.
//! FM synthesis and cycle-exact scheduling remain separate milestones.

use crate::SmsCoreError;
use crate::bus::CpuBus;
use crate::cartridge::CartridgeImage;
use crate::config::SmsMachineConfig;
use crate::cpu::{CpuError, Z80};
use crate::input::{ControllerButtons, SmsInputState};
use crate::io::{SmsIo, SmsIoError};
use crate::memory::SmsMemory;
use crate::psg::{PsgVariant, Sn76489};
use crate::vdp::{SmsVdp, SmsVdpEye, StereoPresentation};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SmsPsgWriteEvent {
    tstate: u64,
    value: u8,
}

/// SMS-owned implementation of the hardware-neutral Z80 memory/I/O contract.
pub struct SmsMachineBus<'a> {
    memory: &'a mut SmsMemory,
    io: &'a mut SmsIo,
    sega_scope: &'a mut SegaScopeState,
    vdp: &'a mut SmsVdp,
    pending_psg_writes: &'a mut Vec<SmsPsgWriteEvent>,
    tstate: u64,
}

impl<'a> SmsMachineBus<'a> {
    fn new(
        memory: &'a mut SmsMemory,
        io: &'a mut SmsIo,
        sega_scope: &'a mut SegaScopeState,
        vdp: &'a mut SmsVdp,
        pending_psg_writes: &'a mut Vec<SmsPsgWriteEvent>,
        tstate: u64,
    ) -> Self {
        Self {
            memory,
            io,
            sega_scope,
            vdp,
            pending_psg_writes,
            tstate,
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
            self.vdp.set_stereo_eye(match self.sega_scope.lens() {
                SegaScopeLens::Left => SmsVdpEye::Left,
                SegaScopeLens::Right => SmsVdpEye::Right,
            });
        }
    }

    fn read_port(&mut self, port: u16) -> u8 {
        if matches!((port as u8) & 0xC1, 0x80 | 0x81) {
            self.vdp.read_port(port)
        } else {
            match port as u8 {
                0x7E => self.vdp.read_v_counter(),
                0x7F => self.vdp.read_h_counter(),
                _ => self.io.read_port(port),
            }
        }
    }

    fn write_port(&mut self, port: u16, value: u8) {
        if matches!((port as u8) & 0xC1, 0x80 | 0x81) {
            self.vdp.write_port(port, value);
        } else {
            self.io.write_port(port, value);
            if (0x40..=0x7F).contains(&(port as u8)) {
                self.pending_psg_writes.push(SmsPsgWriteEvent {
                    tstate: self.tstate.saturating_add(11),
                    value,
                });
            }
        }
    }
}

/// Instruction-stepping SMS core with cartridge mapper, work RAM and controller I/O.
#[derive(Debug, Clone, PartialEq)]
pub struct SmsMachine {
    cpu: Z80,
    memory: SmsMemory,
    io: SmsIo,
    vdp: SmsVdp,
    psg: Sn76489,
    psg_tstate: u64,
    pending_psg_writes: Vec<SmsPsgWriteEvent>,
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
        Self::with_audio(rom, config, 44_100)
    }

    /// Construct an SMS machine with an explicit host sample rate.
    pub fn with_audio(
        rom: impl Into<Vec<u8>>,
        config: SmsMachineConfig,
        sample_rate: u64,
    ) -> Result<Self, SmsCoreError> {
        let cartridge = CartridgeImage::new(rom)?;
        let memory = SmsMemory::new(cartridge)?;
        let mut cpu = Z80::default();
        cpu.reset();
        let vdp = SmsVdp::new(config.video_standard, config.sprite_limit);
        let psg = Sn76489::new_with_variant(vdp.system_clock_hz(), sample_rate, PsgVariant::Sms)?;
        Ok(Self {
            cpu,
            memory,
            io: SmsIo::new(config.territory),
            vdp,
            psg,
            psg_tstate: 0,
            pending_psg_writes: Vec::new(),
            sega_scope: SegaScopeState::default(),
            config,
        })
    }

    /// Advance the PSG in the same regional clock domain as the SMS Z80.
    ///
    /// This uses nominal instruction T-states, not cycle-exact bus timing.
    fn advance_psg_to_tstate(&mut self, target_tstate: u64) {
        if target_tstate <= self.psg_tstate {
            return;
        }
        self.psg.advance_clocks(target_tstate - self.psg_tstate);
        self.psg_tstate = target_tstate;
    }

    /// Execute one Z80 instruction through the SMS-owned memory and I/O bus.
    ///
    /// The returned T-state count is nominal instruction timing, not cycle-exact
    /// machine timing.
    pub fn step(&mut self) -> Result<u32, CpuError> {
        let tstate = self.cpu.total_tstates;
        let cycles = {
            let mut bus = SmsMachineBus::new(
                &mut self.memory,
                &mut self.io,
                &mut self.sega_scope,
                &mut self.vdp,
                &mut self.pending_psg_writes,
                tstate,
            );
            self.cpu.step(&mut bus)?
        };
        let end_tstate = self.cpu.total_tstates;
        self.vdp.advance(u64::from(cycles));
        loop {
            let next = self
                .pending_psg_writes
                .iter()
                .enumerate()
                .filter(|(_, event)| event.tstate <= end_tstate)
                .min_by_key(|(_, event)| event.tstate)
                .map(|(index, _)| index);
            let Some(index) = next else {
                break;
            };
            let event = self.pending_psg_writes.remove(index);
            self.advance_psg_to_tstate(event.tstate);
            self.psg.write(event.value);
        }
        self.advance_psg_to_tstate(end_tstate);
        self.io.set_light_pulse(self.vdp.light_pulse_active());
        if self.vdp.interrupt_pending() {
            self.cpu.request_int(0xFF);
        } else {
            self.cpu.clear_int_request();
        }
        Ok(cycles)
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
        self.io.set_input(input)?;
        self.vdp.set_light_phaser_input(
            input.light_phaser.x,
            input.light_phaser.y,
            input.light_phaser.trigger_pressed && input.light_phaser_player.is_some(),
        );
        Ok(())
    }

    pub fn set_light_phaser_player(&mut self, player: Option<u8>) -> Result<(), SmsIoError> {
        self.io.set_light_phaser_player(player)?;
        let input = self.io.input();
        self.vdp.set_light_phaser_input(
            input.light_phaser.x,
            input.light_phaser.y,
            input.light_phaser.trigger_pressed && player.is_some(),
        );
        Ok(())
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

    pub fn psg(&self) -> &Sn76489 {
        &self.psg
    }

    /// Drain generated mono PSG samples for the host audio backend.
    pub fn drain_audio_samples(&mut self, max_samples: Option<usize>) -> Vec<f64> {
        self.psg.drain_samples(max_samples)
    }

    pub fn vdp(&self) -> &SmsVdp {
        &self.vdp
    }

    pub fn vdp_mut(&mut self) -> &mut SmsVdp {
        &mut self.vdp
    }

    pub fn set_stereo_presentation(&mut self, mode: StereoPresentation) {
        self.vdp.set_stereo_presentation(mode);
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
    use super::{PsgVariant, SegaScopeLens, SmsMachine};
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
    fn cpu_out_routes_sms_vdp_ports_and_advances_raster() {
        // LD A,$40; OUT ($BF),A; LD A,$81; OUT ($BF),A writes R1=$40.
        let rom = [0x3E, 0x40, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF];
        let mut machine = SmsMachine::new(rom.to_vec()).unwrap();
        for _ in 0..4 {
            machine.step().unwrap();
        }
        assert_eq!(machine.vdp().registers()[1], 0x40);
        assert!(machine.vdp().cycles_into_line() > 0);
    }

    #[test]
    fn cpu_out_routes_psg_writes_and_selects_sms_noise_hardware() {
        let rom = [0x3E, 0x85, 0xD3, 0x7F, 0x3E, 0x02, 0xD3, 0x7F];
        let mut machine = SmsMachine::new(rom.to_vec()).unwrap();
        for _ in 0..4 {
            machine.step().unwrap();
        }
        assert_eq!(machine.psg().variant(), PsgVariant::Sms);
        assert_eq!(machine.psg().tone_periods()[0], 0x25);
        assert_eq!(machine.psg().clock_hz(), 3_579_545);
    }

    #[test]
    fn pal_sms_uses_pal_psg_clock_and_audio_samples_are_drainable() {
        use crate::config::{SmsMachineConfig, VideoStandard};
        let config = SmsMachineConfig {
            video_standard: VideoStandard::Pal,
            ..SmsMachineConfig::default()
        };
        let mut machine = SmsMachine::with_audio(vec![0x00], config, 44_100).unwrap();
        assert_eq!(machine.psg().clock_hz(), 3_546_895);
        machine.advance_psg_to_tstate(35_469);
        assert!(!machine.drain_audio_samples(None).is_empty());
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
