//! SMS CPU/memory/I/O integration boundary.
//!
//! Instruction-stepping SMS machine with cartridge, VDP, and native PSG.
//! FM synthesis and a cycle-exact machine scheduler remain separate milestones.

use crate::SmsCoreError;
use crate::bus::CpuBus;
use crate::cartridge::CartridgeImage;
use crate::config::SmsMachineConfig;
use crate::cpu::{CpuError, Z80};
use crate::input::{ControllerButtons, SmsInputState};
use crate::io::{SmsIo, SmsIoError};
use crate::memory::SmsMemory;
use crate::psg::Sn76489;
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
    pending_psg_writes: Option<&'a mut Vec<SmsPsgWriteEvent>>,
    tstate: u64,
}

impl<'a> SmsMachineBus<'a> {
    pub fn new(
        memory: &'a mut SmsMemory,
        io: &'a mut SmsIo,
        sega_scope: &'a mut SegaScopeState,
        vdp: &'a mut SmsVdp,
    ) -> Self {
        Self {
            memory,
            io,
            sega_scope,
            vdp,
            pending_psg_writes: None,
            tstate: 0,
        }
    }

    fn with_psg_queue(
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
            pending_psg_writes: Some(pending_psg_writes),
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
        match (port as u8) & 0xC1 {
            0x80 | 0x81 => self.vdp.read_port(port),
            0x40 => self.vdp.read_v_counter(),
            0x41 => self.vdp.read_h_counter(),
            _ => self.io.read_port(port),
        }
    }

    fn write_port(&mut self, port: u16, value: u8) {
        let port_byte = port as u8;
        if port_byte <= 0x3F && port_byte & 1 == 0 {
            // A0 selects the memory-control latch throughout $00-$3F;
            // the upper address bits are not decoded by the SMS.
            self.memory.write_control_port(value);
        } else if matches!(port_byte & 0xC1, 0x80 | 0x81) {
            self.vdp.write_port(port, value);
        } else {
            self.io.write_port(port, value);
            if (0x40..=0x7F).contains(&port_byte)
                && let Some(pending) = self.pending_psg_writes.as_mut()
            {
                pending.push(SmsPsgWriteEvent {
                    // Nominal edge for immediate OUT (n),A; full bus-phase
                    // timing is a separate temporal-accuracy milestone.
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

    /// Construct an SMS machine with a native PSG sample rate.
    pub fn with_audio(
        rom: impl Into<Vec<u8>>,
        config: SmsMachineConfig,
        sample_rate: u64,
    ) -> Result<Self, SmsCoreError> {
        Self::with_bios_audio(rom, None, config, sample_rate)
    }

    /// Construct an SMS machine with an optional user-supplied 8 KiB BIOS.
    pub fn with_bios(
        rom: impl Into<Vec<u8>>,
        bios: Option<Vec<u8>>,
        config: SmsMachineConfig,
    ) -> Result<Self, SmsCoreError> {
        Self::with_bios_audio(rom, bios, config, 44_100)
    }

    pub fn with_bios_audio(
        rom: impl Into<Vec<u8>>,
        bios: Option<Vec<u8>>,
        config: SmsMachineConfig,
        sample_rate: u64,
    ) -> Result<Self, SmsCoreError> {
        let cartridge = CartridgeImage::new(rom)?;
        let memory = SmsMemory::with_bios(cartridge, bios)?;
        let mut cpu = Z80::default();
        cpu.reset();
        let vdp = SmsVdp::new(config.video_standard, config.sprite_limit);
        let psg = Sn76489::new(vdp.system_clock_hz(), sample_rate)?;
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
        let start_tstate = self.cpu.total_tstates;
        let cycles = {
            let mut bus = SmsMachineBus::with_psg_queue(
                &mut self.memory,
                &mut self.io,
                &mut self.sega_scope,
                &mut self.vdp,
                &mut self.pending_psg_writes,
                start_tstate,
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
        self.memory.reset_control_port();
    }

    /// Latch the Master System console Pause switch as a Z80 non-maskable interrupt.
    pub fn request_nmi(&mut self) {
        self.cpu.request_nmi();
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

    /// Drain generated mono samples for a host audio backend or diagnostics.
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
    use super::{SegaScopeLens, SmsMachine, SmsMachineBus};
    use crate::bus::CpuBus;
    use crate::config::{SmsMachineConfig, VideoStandard};
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
    fn console_pause_switch_enters_the_z80_nmi_vector() {
        // Initialize a safe stack before asserting the physical Pause/NMI input.
        let mut machine = SmsMachine::new(vec![0x31, 0xF0, 0xDF, 0x00]).unwrap();
        machine.step().unwrap();
        assert_eq!(machine.cpu().registers.sp, 0xDFF0);

        machine.request_nmi();

        assert_eq!(machine.step().unwrap(), 11);
        assert_eq!(machine.cpu().registers.pc, 0x0066);
        assert_eq!(machine.cpu().registers.sp, 0xDFEE);
    }

    #[test]
    fn bios_can_use_stack_and_work_ram_immediately_after_power_on() {
        // LD SP,$CFF0; LD BC,$1234; PUSH BC; POP DE;
        // LD A,$5A; LD ($C000),A; LD A,($C000); HALT.
        let mut bios = vec![0x00; 0x2000];
        bios[..16].copy_from_slice(&[
            0x31, 0xF0, 0xCF, 0x01, 0x34, 0x12, 0xC5, 0xD1, 0x3E, 0x5A, 0x32, 0x00, 0xC0, 0x3A,
            0x00, 0xC0,
        ]);
        bios[16] = 0x76;
        let mut machine =
            SmsMachine::with_bios(vec![0xFF; 0x4000], Some(bios), SmsMachineConfig::default())
                .unwrap();

        assert_eq!(machine.memory().control_port(), 0xE0);
        assert!(machine.memory().bios_enabled());
        assert_eq!(machine.read_memory(0xC000), 0x00);
        for _ in 0..8 {
            machine.step().unwrap();
        }

        assert_eq!(machine.cpu().registers.sp, 0xCFF0);
        assert_eq!(machine.cpu().registers.d, 0x12);
        assert_eq!(machine.cpu().registers.e, 0x34);
        assert_eq!(machine.cpu().registers.a, 0x5A);
        assert_eq!(machine.read_memory(0xC000), 0x5A);
        assert!(machine.memory().bios_enabled());
        assert_eq!(machine.memory().control_port(), 0xE0);
    }

    #[test]
    fn bios_copies_bootstrap_to_ram_then_hands_off_to_cartridge() {
        // BIOS enables RAM while keeping its own ROM mapped, copies a small
        // hand-off routine to RAM, then disables itself and enables cartridge.
        let mut bios = vec![0x00; 0x2000];
        let startup = [
            0x3E, 0xE3, // LD A,$E3: BIOS + work RAM enabled, cartridge disabled.
            0xD3, 0x3E, // OUT ($3E),A
            0x21, 0x20, 0x00, // LD HL,$0020
            0x11, 0x00, 0xC0, // LD DE,$C000
            0x01, 0x07, 0x00, // LD BC,$0007
            0xED, 0xB0, // LDIR
            0xC3, 0x00, 0xC0, // JP $C000
        ];
        bios[..startup.len()].copy_from_slice(&startup);
        bios[0x20..0x27].copy_from_slice(&[
            0x3E, 0xAB, // LD A,$AB: cartridge + work RAM enabled, BIOS disabled.
            0xD3, 0x3E, // OUT ($3E),A
            0xC3, 0x00, 0x00, // JP $0000 (now the cartridge).
        ]);

        let cartridge = vec![0xC3; 0x4000];
        let mut machine =
            SmsMachine::with_bios(cartridge, Some(bios), SmsMachineConfig::default()).unwrap();
        assert_eq!(machine.memory().control_port(), 0xE0);
        assert!(machine.memory().bios_enabled());
        assert_eq!(machine.read_memory(0x0000), 0x3E);
        assert_eq!(machine.read_memory(0x0400), 0x00);
        assert_eq!(machine.read_memory(0xC000), 0x00);

        // LDIR is modeled as one repeated transfer per CPU step.
        for _ in 0..16 {
            machine.step().unwrap();
        }

        assert!(!machine.memory().bios_enabled());
        assert_eq!(machine.memory().control_port(), 0xAB);
        assert_eq!(machine.cpu().registers.pc, 0x0000);
        assert_eq!(machine.read_memory(0x0000), 0xC3);
        assert_eq!(machine.read_memory(0xC000), 0x3E);
    }

    #[test]
    fn mirrored_even_ports_select_the_sms_memory_control_latch() {
        let bios = vec![0xA5; 0x2000];
        let mut machine =
            SmsMachine::with_bios(vec![0xC3; 0x4000], Some(bios), SmsMachineConfig::default())
                .unwrap();
        assert!(machine.memory().bios_enabled());

        {
            let mut bus = SmsMachineBus::new(
                &mut machine.memory,
                &mut machine.io,
                &mut machine.sega_scope,
                &mut machine.vdp,
            );
            // SMS decodes A0 in the $00-$3F range; $1E mirrors $3E.
            bus.write_port(0x001E, 0x08);
        }

        assert!(!machine.memory().bios_enabled());
        assert_eq!(machine.read_memory(0x0000), 0xC3);
    }

    #[test]
    fn cpu_reset_restores_bios_overlay_after_firmware_disabled_it() {
        let mut bios = vec![0x3E, 0x08, 0xD3, 0x3E];
        bios.resize(0x2000, 0x00);
        let mut machine =
            SmsMachine::with_bios(vec![0xC3; 0x4000], Some(bios), SmsMachineConfig::default())
                .unwrap();
        machine.step().unwrap();
        machine.step().unwrap();
        assert!(!machine.memory().bios_enabled());
        machine.reset_cpu();
        assert!(machine.memory().bios_enabled());
        assert_eq!(machine.read_memory(0), 0x3E);
    }

    #[test]
    fn machine_without_bios_preserves_direct_cartridge_boot() {
        let machine = SmsMachine::new(vec![0xA7; 0x4000]).unwrap();
        assert!(!machine.memory().bios_enabled());
        assert_eq!(machine.read_memory(0x0000), 0xA7);
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
    fn vdp_vertical_and_horizontal_counter_ports_are_mirrored() {
        let mut machine = SmsMachine::new(vec![0x00]).unwrap();
        machine.vdp.advance(228 * 219 + 11);
        let mut bus = SmsMachineBus::new(
            &mut machine.memory,
            &mut machine.io,
            &mut machine.sega_scope,
            &mut machine.vdp,
        );
        assert_eq!(bus.read_port(0x40), 0xD5);
        assert_eq!(bus.read_port(0x42), 0xD5);
        assert_eq!(bus.read_port(0x7E), 0xD5);
        let h_counter = bus.read_port(0x7F);
        assert_eq!(bus.read_port(0x41), h_counter);
        assert_eq!(bus.read_port(0x43), h_counter);
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
    fn cpu_out_routes_psg_register_writes_to_independent_sms_core() {
        // LD A,$90; OUT ($7F),A; LD A,$85; OUT ($7F),A;
        // LD A,$12; OUT ($7F),A
        let rom = [
            0x3E, 0x90, 0xD3, 0x7F, 0x3E, 0x85, 0xD3, 0x7F, 0x3E, 0x12, 0xD3, 0x7F,
        ];
        let mut machine = SmsMachine::new(rom.to_vec()).unwrap();
        for _ in 0..6 {
            machine.step().unwrap();
        }
        assert_eq!(machine.psg().tone_periods()[0], 0x125);
        assert_eq!(machine.psg().attenuations()[0], 0);
        assert_eq!(machine.psg().clock_hz(), 3_579_545);
    }

    #[test]
    fn pal_clock_and_generated_audio_samples_are_native_and_deterministic() {
        let config = SmsMachineConfig {
            video_standard: VideoStandard::Pal,
            ..SmsMachineConfig::default()
        };
        let rom = [
            0x3E, 0x90, 0xD3, 0x7F, 0x3E, 0x85, 0xD3, 0x7F, 0x3E, 0x12, 0xD3, 0x7F, 0xC3, 0x00,
            0x00,
        ];
        let mut first = SmsMachine::with_audio(rom.to_vec(), config, 44_100).unwrap();
        let mut second = first.clone();
        assert_eq!(first.psg().clock_hz(), 3_546_895);
        for _ in 0..10_000 {
            first.step().unwrap();
            second.step().unwrap();
        }
        let first_samples = first.drain_audio_samples(None);
        let second_samples = second.drain_audio_samples(None);
        assert_eq!(first_samples, second_samples);
        assert!(!first_samples.is_empty());
        assert!(first_samples.iter().any(|sample| sample.abs() > 0.001));
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
