//! Host-independent controller normalization for the SG-1000 core.
//!
//! SDL, XInput, and DirectInput adapters should translate their host-specific
//! button/axis state into this type. This module deliberately has no OS, SDL,
//! windowing, polling, or threading dependency, so the emulated machine stays
//! deterministic and testable.
//!
//! Port encoding used here: bits 0..=3 are Up, Down, Left, Right; bits 4..=5
//! are Button 1 and Button 2. These six controls are active-low; bits 6..=7
//! remain high. Backend adapters must not apply additional inversion.

/// Host controller button state after backend-specific normalization.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ControllerButtons {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub button_1: bool,
    pub button_2: bool,
}

impl ControllerButtons {
    /// Encode pressed buttons as the SG-1000 active-low controller byte.
    ///
    /// A released control is represented by 1; a pressed control by 0.
    pub const fn to_active_low_port(self) -> u8 {
        let mut value = 0xFF;
        if self.up {
            value &= !(1 << 0);
        }
        if self.down {
            value &= !(1 << 1);
        }
        if self.left {
            value &= !(1 << 2);
        }
        if self.right {
            value &= !(1 << 3);
        }
        if self.button_1 {
            value &= !(1 << 4);
        }
        if self.button_2 {
            value &= !(1 << 5);
        }
        value
    }
}

/// The host API that supplied a normalized controller sample.
///
/// This is diagnostic metadata only; it does not make the core poll a host
/// API. Concrete SDL/XInput/DirectInput adapters belong at the host boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerBackend {
    Sdl,
    XInput,
    DirectInput,
}

/// A normalized snapshot for both SG-1000 controller ports.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ControllerFrame {
    pub player_1: ControllerButtons,
    pub player_2: ControllerButtons,
}

impl ControllerFrame {
    pub const fn port_1(self) -> u8 {
        self.player_1.to_active_low_port()
    }

    pub const fn port_2(self) -> u8 {
        self.player_2.to_active_low_port()
    }

    /// Latch both normalized controller bytes into the emulated machine.
    ///
    /// This is an explicit snapshot transfer: it performs no host polling and
    /// does not invoke callbacks during CPU execution.
    pub fn apply_to(
        self,
        machine: &mut crate::machine::Sg1000Machine,
    ) -> Result<(), crate::io::IoError> {
        machine.set_joystick(1, self.port_1())?;
        machine.set_joystick(2, self.port_2())
    }
}

#[cfg(test)]
mod tests {
    use super::{ControllerButtons, ControllerFrame};
    use crate::{MemoryConfig, Sg1000Machine};

    #[test]
    fn no_buttons_pressed_is_all_bits_high() {
        assert_eq!(ControllerButtons::default().to_active_low_port(), 0xFF);
        assert_eq!(ControllerFrame::default().port_1(), 0xFF);
        assert_eq!(ControllerFrame::default().port_2(), 0xFF);
    }

    #[test]
    fn each_control_clears_only_its_active_low_bit() {
        let cases = [
            (ControllerButtons { up: true, ..Default::default() }, 0xFE),
            (ControllerButtons { down: true, ..Default::default() }, 0xFD),
            (ControllerButtons { left: true, ..Default::default() }, 0xFB),
            (ControllerButtons { right: true, ..Default::default() }, 0xF7),
            (ControllerButtons { button_1: true, ..Default::default() }, 0xEF),
            (ControllerButtons { button_2: true, ..Default::default() }, 0xDF),
        ];
        for (buttons, expected) in cases {
            assert_eq!(buttons.to_active_low_port(), expected);
        }
    }

    #[test]
    fn simultaneous_buttons_compose_without_affecting_unused_bits() {
        let buttons = ControllerButtons {
            up: true,
            right: true,
            button_2: true,
            ..Default::default()
        };
        assert_eq!(buttons.to_active_low_port(), 0xD6);
        assert_eq!(buttons.to_active_low_port() & 0xC0, 0xC0);
    }

    #[test]
    fn two_players_are_encoded_independently() {
        let frame = ControllerFrame {
            player_1: ControllerButtons { up: true, ..Default::default() },
            player_2: ControllerButtons { button_1: true, ..Default::default() },
        };
        assert_eq!(frame.port_1(), 0xFE);
        assert_eq!(frame.port_2(), 0xEF);
    }

    #[test]
    fn frame_latches_into_the_machine_controller_ports() {
        let mut machine = Sg1000Machine::new(&[0x76], MemoryConfig::default()).unwrap();
        let frame = ControllerFrame {
            player_1: ControllerButtons { left: true, ..Default::default() },
            player_2: ControllerButtons { button_2: true, ..Default::default() },
        };

        frame.apply_to(&mut machine).unwrap();

        assert_eq!(machine.read_port(0x00), 0xFB);
        assert_eq!(machine.read_port(0x01), 0xDF);
        assert_eq!(machine.read_port(0xDC), 0xFB);
        assert_eq!(machine.read_port(0xDD), 0xDF);
    }
}
