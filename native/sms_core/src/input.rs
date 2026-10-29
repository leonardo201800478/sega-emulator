//! Host-independent input snapshots for the Sega Master System.
//!
//! Host APIs (XInput/SDL/Qt mouse events) must be normalized outside the CPU
//! execution path. The core stores snapshots only; it never polls the host.

/// One normalized controller snapshot. A true field means physically pressed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ControllerButtons {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub button_1: bool,
    pub button_2: bool,
}

/// Light Phaser state supplied by the host and, later, by the VDP raster model.
///
/// Coordinates are host-normalized logical screen coordinates. This type does
/// not assume a resolution or perform coordinate conversion. A mouse click can
/// drive trigger_pressed; the VDP must determine light_pulse from the rendered
/// pixel/raster position before the CPU sees the active-low TH line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LightPhaserState {
    pub x: u16,
    pub y: u16,
    pub trigger_pressed: bool,
    pub light_pulse: bool,
}

/// Explicit, deterministic snapshot of all host input relevant to SMS I/O.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SmsInputState {
    pub player_1: ControllerButtons,
    pub player_2: ControllerButtons,
    pub reset_pressed: bool,
    pub light_phaser_player: Option<u8>,
    pub light_phaser: LightPhaserState,
}

impl SmsInputState {
    /// Set the mouse/controller aim and trigger without inventing a light pulse.
    pub fn set_light_phaser_aim(&mut self, x: u16, y: u16, trigger_pressed: bool) {
        self.light_phaser.x = x;
        self.light_phaser.y = y;
        self.light_phaser.trigger_pressed = trigger_pressed;
    }

    /// Feed a raster-qualified light pulse from the future VDP/scheduler.
    ///
    /// Until that integration exists, callers should leave this false rather
    /// than treating a mouse pointer alone as proof that the screen is bright.
    pub fn set_light_pulse(&mut self, detected: bool) {
        self.light_phaser.light_pulse = detected;
    }
}

#[cfg(test)]
mod tests {
    use super::{ControllerButtons, LightPhaserState, SmsInputState};

    #[test]
    fn input_defaults_to_released_controls_and_no_light_pulse() {
        let input = SmsInputState::default();
        assert_eq!(input.player_1, ControllerButtons::default());
        assert_eq!(input.light_phaser, LightPhaserState::default());
        assert_eq!(input.light_phaser_player, None);
        assert!(!input.reset_pressed);
    }

    #[test]
    fn mouse_aim_does_not_fabricate_a_light_pulse() {
        let mut input = SmsInputState::default();
        input.set_light_phaser_aim(123, 45, true);
        assert_eq!(input.light_phaser.x, 123);
        assert_eq!(input.light_phaser.y, 45);
        assert!(input.light_phaser.trigger_pressed);
        assert!(!input.light_phaser.light_pulse);
        input.set_light_pulse(true);
        assert!(input.light_phaser.light_pulse);
    }
}
