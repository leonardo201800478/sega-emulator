//! Python-independent SG-1000 input and port-I/O boundary.
//!
//! The controller aliases and PSG write range mirror the current Python
//! machine contract. VDP port decoding remains deliberately unimplemented
//! here until the native TMS9918A device is integrated; unknown reads return
//! the existing open-bus placeholder (0xFF), not a hardware-accuracy claim.

use std::fmt;

/// A PSG write captured at the machine's supplied nominal T-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsgWriteEvent {
    pub tstate: u64,
    pub value: u8,
}

/// Errors from validating host input before it enters the emulated machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    InvalidPlayer(u8),
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlayer(player) => {
                write!(f, "SG-1000 controller player must be 1 or 2, got {player}")
            }
        }
    }
}

impl std::error::Error for IoError {}

/// Deterministic controller state and PSG port-write capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sg1000Io {
    joysticks: [u8; 2],
    psg_write_events: Vec<PsgWriteEvent>,
}

impl Default for Sg1000Io {
    fn default() -> Self {
        Self {
            joysticks: [0xFF; 2],
            psg_write_events: Vec::new(),
        }
    }
}

impl Sg1000Io {
    /// Read controller aliases supported by the existing Python machine.
    /// Unimplemented ports currently return the same placeholder value, 0xFF.
    pub fn read_port(&self, address: u16) -> u8 {
        match address as u8 {
            0x00 | 0xDC => self.joysticks[0],
            0x01 | 0xDD => self.joysticks[1],
            _ => 0xFF,
        }
    }

    /// Capture writes to the PSG port range at a supplied nominal T-state.
    /// This records events only; it does not claim cycle-exact bus timing.
    pub fn write_port_at(&mut self, address: u16, value: u8, tstate: u64) {
        if (0x40..=0x7F).contains(&(address as u8)) {
            self.psg_write_events.push(PsgWriteEvent { tstate, value });
        }
    }

    /// Set the active-low byte presented by controller 1 or 2.
    pub fn set_joystick(&mut self, player: u8, value: u8) -> Result<(), IoError> {
        match player {
            1 => self.joysticks[0] = value,
            2 => self.joysticks[1] = value,
            _ => return Err(IoError::InvalidPlayer(player)),
        }
        Ok(())
    }

    /// Drain events in insertion order, preserving equal-T-state ordering.
    pub fn drain_psg_write_events(&mut self) -> Vec<PsgWriteEvent> {
        std::mem::take(&mut self.psg_write_events)
    }
}

#[cfg(test)]
mod tests {
    use super::{IoError, PsgWriteEvent, Sg1000Io};

    #[test]
    fn controllers_default_to_unpressed_and_aliases_match() {
        let io = Sg1000Io::default();
        assert_eq!(io.read_port(0x00), 0xFF);
        assert_eq!(io.read_port(0xDC), 0xFF);
        assert_eq!(io.read_port(0x01), 0xFF);
        assert_eq!(io.read_port(0xDD), 0xFF);
    }

    #[test]
    fn controller_updates_are_visible_through_both_aliases() {
        let mut io = Sg1000Io::default();
        assert_eq!(io.set_joystick(1, 0xFE), Ok(()));
        assert_eq!(io.set_joystick(2, 0xFD), Ok(()));
        assert_eq!(io.read_port(0x00), 0xFE);
        assert_eq!(io.read_port(0xDC), 0xFE);
        assert_eq!(io.read_port(0x01), 0xFD);
        assert_eq!(io.read_port(0xDD), 0xFD);
        assert_eq!(io.set_joystick(0, 0), Err(IoError::InvalidPlayer(0)));
        assert_eq!(io.set_joystick(3, 0), Err(IoError::InvalidPlayer(3)));
    }

    #[test]
    fn psg_writes_capture_timestamp_and_preserve_order() {
        let mut io = Sg1000Io::default();
        io.write_port_at(0x40, 0x90, 12);
        io.write_port_at(0x7F, 0x21, 12);
        io.write_port_at(0x3F, 0xAA, 13);
        io.write_port_at(0x80, 0xBB, 14);
        assert_eq!(
            io.drain_psg_write_events(),
            vec![
                PsgWriteEvent {
                    tstate: 12,
                    value: 0x90
                },
                PsgWriteEvent {
                    tstate: 12,
                    value: 0x21
                },
            ]
        );
        assert!(io.drain_psg_write_events().is_empty());
    }

    #[test]
    fn unknown_ports_use_documented_placeholder() {
        let io = Sg1000Io::default();
        assert_eq!(io.read_port(0x02), 0xFF);
        assert_eq!(io.read_port(0xBE), 0xFF);
    }
}
