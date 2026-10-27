//! Host-independent controller normalization for the SG-1000 core.
//!
//! XInput and SDL adapters translate their host-specific state
//! into this type. The module has no OS, SDL, windowing, polling, or threading
//! dependency, so the emulated machine remains deterministic and testable.
//!
//! Each logical controller uses bits 0..=3 for Up, Down, Left, Right and bits
//! 4..=5 for Button 1 and Button 2, all active-low. This is a logical snapshot,
//! not a literal physical port byte: the SG-1000 wiring splits player 2 across
//! ports 0xDC and 0xDD. Sg1000Io composes those physical reads.

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
    /// Encode pressed buttons as a logical active-low controller byte.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerBackend {
    Sdl,
    XInput,
}

/// Policy used by the host-side controller provider.
///
/// The default is native-first: on Windows, use XInput for devices exposed by
/// XInput, then DirectInput for legacy devices that cannot be read through
/// XInput. SDL remains an explicit fallback/cross-platform backend. This is a
/// selection policy, not a claim that one API is universally lower-latency for
/// every controller; end-to-end latency depends on device, transport, driver,
/// polling cadence, and the host event loop.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ControllerBackendPreference {
    #[default]
    NativeLowLatency,
    Sdl,
}

/// A normalized snapshot for both SG-1000 controllers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ControllerFrame {
    pub player_1: ControllerButtons,
    pub player_2: ControllerButtons,
}

impl ControllerFrame {
    /// Return the logical active-low snapshot for player 1.
    pub const fn port_1(self) -> u8 {
        self.player_1.to_active_low_port()
    }

    /// Return the logical active-low snapshot for player 2.
    pub const fn port_2(self) -> u8 {
        self.player_2.to_active_low_port()
    }

    /// Latch both normalized controller snapshots into the emulated machine.
    ///
    /// This is an explicit snapshot transfer: it performs no host polling and
    /// does not invoke callbacks during CPU execution. Sg1000Io composes the
    /// physical DC/DD port layout when the CPU reads those ports.
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
    use super::{ControllerBackendPreference, ControllerButtons, ControllerFrame};
    use crate::{MemoryConfig, Sg1000Machine};

    #[test]
    fn native_low_latency_is_the_default_backend_policy() {
        assert_eq!(
            ControllerBackendPreference::default(),
            ControllerBackendPreference::NativeLowLatency
        );
    }

    #[test]
    fn no_buttons_pressed_is_all_bits_high() {
        assert_eq!(ControllerButtons::default().to_active_low_port(), 0xFF);
        assert_eq!(ControllerFrame::default().port_1(), 0xFF);
        assert_eq!(ControllerFrame::default().port_2(), 0xFF);
    }

    #[test]
    fn each_control_clears_only_its_active_low_logical_bit() {
        let cases = [
            (
                ControllerButtons {
                    up: true,
                    ..Default::default()
                },
                0xFE,
            ),
            (
                ControllerButtons {
                    down: true,
                    ..Default::default()
                },
                0xFD,
            ),
            (
                ControllerButtons {
                    left: true,
                    ..Default::default()
                },
                0xFB,
            ),
            (
                ControllerButtons {
                    right: true,
                    ..Default::default()
                },
                0xF7,
            ),
            (
                ControllerButtons {
                    button_1: true,
                    ..Default::default()
                },
                0xEF,
            ),
            (
                ControllerButtons {
                    button_2: true,
                    ..Default::default()
                },
                0xDF,
            ),
        ];
        for (buttons, expected) in cases {
            assert_eq!(buttons.to_active_low_port(), expected);
        }
    }

    #[test]
    fn simultaneous_buttons_compose_without_affecting_unused_logical_bits() {
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
    fn two_players_are_encoded_independently_as_logical_snapshots() {
        let frame = ControllerFrame {
            player_1: ControllerButtons {
                up: true,
                ..Default::default()
            },
            player_2: ControllerButtons {
                button_1: true,
                ..Default::default()
            },
        };
        assert_eq!(frame.port_1(), 0xFE);
        assert_eq!(frame.port_2(), 0xEF);
    }

    #[test]
    fn frame_latches_into_logical_and_physical_controller_reads() {
        let mut machine = Sg1000Machine::new(&[0x76], MemoryConfig::default()).unwrap();
        let frame = ControllerFrame {
            player_1: ControllerButtons {
                left: true,
                ..Default::default()
            },
            player_2: ControllerButtons {
                button_2: true,
                ..Default::default()
            },
        };

        frame.apply_to(&mut machine).unwrap();

        assert_eq!(machine.read_port(0x00), 0xFB);
        assert_eq!(machine.read_port(0x01), 0xDF);
        assert_eq!(machine.read_port(0xDC), 0xFB);
        assert_eq!(machine.read_port(0xDD), 0xF7);
    }
}
