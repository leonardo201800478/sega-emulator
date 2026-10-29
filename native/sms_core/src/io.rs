//! Sega Master System controller and I/O port decoding.
//!
//! Controller bits are active-low. The port map follows documented SMS mirrors:
//! $3F on odd $01-$3F, $DC on even $C0-$FE, and $DD on odd $C1-$FF.
//! Unimplemented peripherals deliberately return open-bus $FF.

use crate::config::Territory;
use crate::input::{ControllerButtons, SmsInputState};

/// Errors from validating an SMS controller/peripheral assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsIoError {
    InvalidPlayer(u8),
}

impl std::fmt::Display for SmsIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPlayer(player) => {
                write!(f, "SMS controller player must be 1 or 2, got {player}")
            }
        }
    }
}

impl std::error::Error for SmsIoError {}

/// Deterministic SMS controller input and controller-port control register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsIo {
    territory: Territory,
    input: SmsInputState,
    /// $3F: low nibble directions, high nibble driven levels for TR/TH.
    io_control: u8,
}

impl Default for SmsIo {
    fn default() -> Self {
        Self::new(Territory::Export)
    }
}

impl SmsIo {
    pub fn new(territory: Territory) -> Self {
        Self {
            territory,
            input: SmsInputState::default(),
            // Compatible input-only startup state; passive controllers pull high.
            io_control: 0xFF,
        }
    }

    pub fn input(&self) -> SmsInputState {
        self.input
    }

    pub fn set_input(&mut self, input: SmsInputState) -> Result<(), SmsIoError> {
        if let Some(player) = input.light_phaser_player
            && player != 1
            && player != 2
        {
            return Err(SmsIoError::InvalidPlayer(player));
        }
        self.input = input;
        Ok(())
    }

    pub fn set_controller(
        &mut self,
        player: u8,
        buttons: ControllerButtons,
    ) -> Result<(), SmsIoError> {
        match player {
            1 => self.input.player_1 = buttons,
            2 => self.input.player_2 = buttons,
            other => return Err(SmsIoError::InvalidPlayer(other)),
        }
        Ok(())
    }

    pub fn set_light_phaser_player(&mut self, player: Option<u8>) -> Result<(), SmsIoError> {
        if let Some(player) = player
            && player != 1
            && player != 2
        {
            return Err(SmsIoError::InvalidPlayer(player));
        }
        self.input.light_phaser_player = player;
        Ok(())
    }

    pub fn io_control(&self) -> u8 {
        self.io_control
    }

    /// Read the decoded port using its low eight address bits.
    pub fn read_port(&self, port: u16) -> u8 {
        let port = port as u8;
        if (0xC0..=0xFE).contains(&port) && port & 1 == 0 {
            return self.read_dc();
        }
        if (0xC1..=0xFF).contains(&port) && port & 1 == 1 {
            return self.read_dd();
        }
        // $3F is a write-only control register.
        0xFF
    }

    /// Write the controller control register through any documented mirror.
    pub fn write_port(&mut self, port: u16, value: u8) {
        let port = port as u8;
        if (0x01..=0x3F).contains(&port) && port & 1 == 1 {
            self.io_control = value;
        }
        // VDP, PSG, memory-control, FM and other devices are not connected yet.
    }

    fn read_dc(&self) -> u8 {
        let p1 = self.input.player_1;
        let p2 = self.input.player_2;
        let mut value = 0xFF;

        if p1.up {
            value &= !(1 << 0);
        }
        if p1.down {
            value &= !(1 << 1);
        }
        if p1.left {
            value &= !(1 << 2);
        }
        if p1.right {
            value &= !(1 << 3);
        }
        if p1.button_1
            || (self.input.light_phaser_player == Some(1)
                && self.input.light_phaser.trigger_pressed)
        {
            value &= !(1 << 4);
        }
        if p1.button_2 {
            value &= !(1 << 5);
        }
        if p2.up {
            value &= !(1 << 6);
        }
        if p2.down {
            value &= !(1 << 7);
        }
        value
    }

    fn read_dd(&self) -> u8 {
        let p2 = self.input.player_2;
        let mut value = 0xFF;

        if p2.left {
            value &= !(1 << 0);
        }
        if p2.right {
            value &= !(1 << 1);
        }
        if p2.button_1
            || (self.input.light_phaser_player == Some(2)
                && self.input.light_phaser.trigger_pressed)
        {
            value &= !(1 << 2);
        }
        if p2.button_2 {
            value &= !(1 << 3);
        }
        if self.input.reset_pressed {
            value &= !(1 << 4);
        }
        // CONT is present on the standard 8-bit console and reads high.
        // SMS II/Japanese variants may differ; no model selection exists yet.
        value |= 1 << 5;

        for (player, direction_bit, level_bit, input_bit) in [(1, 1, 5, 6), (2, 3, 7, 7)] {
            let is_input = self.io_control & (1 << direction_bit) != 0;
            let level = self.io_control & (1 << level_bit) != 0;
            let light_pulse = self.input.light_phaser_player == Some(player)
                && self.input.light_phaser.light_pulse;
            let read_high = if is_input {
                !light_pulse
            } else if self.territory == Territory::Japan {
                // Japanese SMS does not expose driven TH levels in these bits.
                false
            } else {
                level
            };
            if read_high {
                value |= 1 << input_bit;
            } else {
                value &= !(1 << input_bit);
            }
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{SmsIo, SmsIoError};
    use crate::config::Territory;
    use crate::input::{ControllerButtons, SmsInputState};

    #[test]
    fn released_controllers_read_high_and_ports_mirror() {
        let io = SmsIo::default();
        assert_eq!(io.read_port(0xDC), 0xFF);
        assert_eq!(io.read_port(0xC0), 0xFF);
        assert_eq!(io.read_port(0xFE), 0xFF);
        assert_eq!(io.read_port(0xDD), 0xFF);
        assert_eq!(io.read_port(0xC1), 0xFF);
        assert_eq!(io.read_port(0xFF), 0xFF);
    }

    #[test]
    fn controller_bits_are_active_low_and_split_across_sms_ports() {
        let mut io = SmsIo::default();
        io.set_controller(
            1,
            ControllerButtons {
                up: true,
                button_1: true,
                ..Default::default()
            },
        )
        .unwrap();
        io.set_controller(
            2,
            ControllerButtons {
                down: true,
                left: true,
                button_2: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(io.read_port(0xDC), 0x6E);
        assert_eq!(io.read_port(0xDD), 0xF6);
    }

    #[test]
    fn io_control_mirrors_and_driven_th_levels_are_observable_on_export_sms() {
        let mut io = SmsIo::default();
        io.write_port(0x01, 0xDD); // Port A TH is output, driven low.
        assert_eq!(io.io_control(), 0xDD);
        assert_eq!(io.read_port(0xDD) & 0x40, 0);
    }

    #[test]
    fn japanese_sms_does_not_read_back_output_th_levels() {
        let mut io = SmsIo::new(Territory::Japan);
        io.write_port(0x3F, 0xFD);
        assert_eq!(io.read_port(0xDD) & 0x40, 0);
    }

    #[test]
    fn mouse_light_phaser_trigger_maps_to_controller_button_one() {
        let mut io = SmsIo::default();
        let mut input = SmsInputState::default();
        input.set_light_phaser_aim(160, 100, true);
        io.set_input(input).unwrap();
        io.set_light_phaser_player(Some(1)).unwrap();
        assert_eq!(io.read_port(0xDC) & (1 << 4), 0);
        assert_ne!(io.read_port(0xDD) & (1 << 6), 0);
        input.set_light_pulse(true);
        io.set_input(input).unwrap();
        io.set_light_phaser_player(Some(1)).unwrap();
        assert_eq!(io.read_port(0xDD) & (1 << 6), 0);
    }

    #[test]
    fn invalid_player_is_rejected() {
        assert_eq!(
            SmsIo::default().set_light_phaser_player(Some(3)),
            Err(SmsIoError::InvalidPlayer(3))
        );
    }
}
