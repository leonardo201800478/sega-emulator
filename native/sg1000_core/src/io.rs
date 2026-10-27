//! Python-independent SG-1000 input and port-I/O boundary.
//!
//! Logical controller snapshots are kept separately from the physical port
//! encoding. SG-1000 port 0xDC carries player 1's six controls and player 2's
//! Up/Down; port 0xDD carries player 2's Left/Right/Button 1/Button 2.
//! Ports 0x00 and 0x01 remain logical aliases for compatibility with the
//! existing machine contract. VDP port decoding remains deliberately
//! unimplemented here; unrelated reads return the 0xFF placeholder.

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

/// Deterministic logical controller state and PSG port-write capture.
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
    /// Read logical compatibility aliases or the modeled physical SG-1000
    /// controller ports. Controller inputs are active-low.
    pub fn read_port(&self, address: u16) -> u8 {
        let player_1 = self.joysticks[0];
        let player_2 = self.joysticks[1];
        match address as u8 {
            // Logical aliases retained for the existing host/API contract.
            0x00 => player_1,
            0x01 => player_2,
            // Physical ports: DC has P1 bits 0..5 and P2 Up/Down at bits 6..7.
            0xDC => (player_1 & 0x3F) | ((player_2 & 0x03) << 6),
            // DD has P2 Left/Right/Button 1/Button 2 at bits 0..3.
            // Remaining bits are held high in this model.
            0xDD => 0xF0 | ((player_2 >> 2) & 0x0F),
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

    /// Set the active-low logical button byte for controller 1 or 2.
    /// Physical port reads compose the controller bits according to the SG-1000
    /// wiring; this method does not treat the logical byte as a physical port.
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
    fn controllers_default_to_unpressed_and_ports_are_released() {
        let io = Sg1000Io::default();
        assert_eq!(io.read_port(0x00), 0xFF);
        assert_eq!(io.read_port(0x01), 0xFF);
        assert_eq!(io.read_port(0xDC), 0xFF);
        assert_eq!(io.read_port(0xDD), 0xFF);
    }

    #[test]
    fn logical_aliases_and_physical_ports_encode_controller_state() {
        let mut io = Sg1000Io::default();
        assert_eq!(io.set_joystick(1, 0xFE), Ok(()));
        assert_eq!(io.set_joystick(2, 0xFD), Ok(()));
        assert_eq!(io.read_port(0x00), 0xFE);
        assert_eq!(io.read_port(0x01), 0xFD);
        // P1 Up is bit 0; P2 Down is bit 7 on physical port DC.
        assert_eq!(io.read_port(0xDC), 0x7E);
        // P2 Down is not represented in DD's low nibble.
        assert_eq!(io.read_port(0xDD), 0xFF);
        assert_eq!(io.set_joystick(0, 0), Err(IoError::InvalidPlayer(0)));
        assert_eq!(io.set_joystick(3, 0), Err(IoError::InvalidPlayer(3)));
    }

    #[test]
    fn player_two_directions_and_buttons_are_split_across_physical_ports() {
        let mut io = Sg1000Io::default();
        io.set_joystick(2, !(1 << 0)).unwrap(); // Up
        assert_eq!(io.read_port(0xDC), 0xBF);
        assert_eq!(io.read_port(0xDD), 0xFF);

        io.set_joystick(2, !(1 << 2)).unwrap(); // Left
        assert_eq!(io.read_port(0xDC), 0xFF);
        assert_eq!(io.read_port(0xDD), 0xFE);

        io.set_joystick(2, !(1 << 5)).unwrap(); // Button 2
        assert_eq!(io.read_port(0xDC), 0xFF);
        assert_eq!(io.read_port(0xDD), 0xF7);
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
