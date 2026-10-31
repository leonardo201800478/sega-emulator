//! Host-independent input contracts shared by all Sega emulator cores.
//!
//! The shared contracts are host-independent and have no third-party
//! dependencies. Platform adapters are isolated in backend modules; the
//! initial Windows SDL3 adapter dynamically loads SDL3's C ABI. Backends
//! produce timestamped physical-device state; profiles map controls to
//! logical actions; each console maps those actions into its own peripherals.
//!
//! Poll only at an explicit host input boundary. Never poll from CPU
//! instructions, memory accesses, bus callbacks, or pixel/sample paths.
//! Reuse InputBuffers between polls to retain vector capacity.

use std::fmt;

pub mod sdl3_backend;

pub const MAX_BUTTONS: usize = 128;
pub const MAX_AXES: usize = 32;
pub const MAX_HATS: usize = 8;
pub const MAX_ACTIONS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    GameInput,
    Sdl3,
    Gilrs,
    XInput,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceClass {
    Gamepad,
    Joystick,
    Wheel,
    Pedals,
    FlightStick,
    ArcadeStick,
    Keyboard,
    Pointer,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisSemantic {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
    Wheel,
    Accelerator,
    Brake,
    Clutch,
    Rudder,
    Throttle,
    Unknown,
}

/// Session-local identifier. It is not a persistent hardware identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub u64);

/// Opaque cross-backend key derived from a canonical OS device identity.
/// Raw device paths and serial numbers must never be written to logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalDeviceKey([u8; 16]);

impl PhysicalDeviceKey {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapabilities {
    pub button_count: u16,
    pub axis_count: u8,
    pub hat_count: u8,
    pub rumble: bool,
    pub force_feedback: bool,
    pub gyro: bool,
    pub accelerometer: bool,
    pub touchpad: bool,
}

/// Allocated only during enumeration or device changes, never per poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub id: DeviceId,
    pub backend: BackendKind,
    pub class: DeviceClass,
    pub name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub physical_key: Option<PhysicalDeviceKey>,
    pub capabilities: DeviceCapabilities,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ButtonSet {
    words: [u64; 2],
}

impl ButtonSet {
    pub fn set(&mut self, index: usize, pressed: bool) -> Result<(), InputError> {
        if index >= MAX_BUTTONS {
            return Err(InputError::ButtonOutOfRange(index));
        }
        let mask = 1_u64 << (index % 64);
        if pressed {
            self.words[index / 64] |= mask;
        } else {
            self.words[index / 64] &= !mask;
        }
        Ok(())
    }

    pub const fn is_pressed(&self, index: usize) -> bool {
        if index >= MAX_BUTTONS {
            return false;
        }
        self.words[index / 64] & (1_u64 << (index % 64)) != 0
    }

    pub const fn words(&self) -> [u64; 2] {
        self.words
    }
}

/// Raw normalized state. Axes use signed Q15; trigger axes use 0 (released)
/// through 32767 (fully pressed). Raw analog values are never discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceState {
    pub buttons: ButtonSet,
    axes: [i16; MAX_AXES],
    axis_semantics: [AxisSemantic; MAX_AXES],
    axis_count: u8,
    hats: [u8; MAX_HATS],
    hat_count: u8,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            buttons: ButtonSet::default(),
            axes: [0; MAX_AXES],
            axis_semantics: [AxisSemantic::Unknown; MAX_AXES],
            axis_count: 0,
            hats: [0; MAX_HATS],
            hat_count: 0,
        }
    }
}

impl DeviceState {
    pub fn set_axis(
        &mut self,
        index: usize,
        value: i16,
        semantic: AxisSemantic,
    ) -> Result<(), InputError> {
        if index >= MAX_AXES {
            return Err(InputError::AxisOutOfRange(index));
        }
        self.axes[index] = value;
        self.axis_semantics[index] = semantic;
        self.axis_count = self.axis_count.max((index + 1) as u8);
        Ok(())
    }

    pub const fn axis(&self, index: usize) -> i16 {
        if index >= self.axis_count as usize {
            0
        } else {
            self.axes[index]
        }
    }

    pub const fn axis_semantic(&self, index: usize) -> Option<AxisSemantic> {
        if index >= self.axis_count as usize {
            None
        } else {
            Some(self.axis_semantics[index])
        }
    }

    pub const fn axis_count(&self) -> u8 {
        self.axis_count
    }

    /// Hat bits: up=1, right=2, down=4, left=8; diagonals combine bits.
    pub fn set_hat(&mut self, index: usize, directions: u8) -> Result<(), InputError> {
        if index >= MAX_HATS {
            return Err(InputError::HatOutOfRange(index));
        }
        if directions & !0x0F != 0 {
            return Err(InputError::InvalidHatMask(directions));
        }
        self.hats[index] = directions;
        self.hat_count = self.hat_count.max((index + 1) as u8);
        Ok(())
    }

    pub const fn hat(&self, index: usize) -> u8 {
        if index >= self.hat_count as usize {
            0
        } else {
            self.hats[index]
        }
    }

    pub const fn hat_count(&self) -> u8 {
        self.hat_count
    }
}

/// Timestamp uses the runtime-wide monotonic nanosecond timebase.
/// Backend-native clocks must be converted at the platform boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceReading {
    pub device_id: DeviceId,
    pub timestamp_ns: u64,
    pub sequence: u64,
    pub state: DeviceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEventKind {
    Connected,
    Disconnected,
    CapabilitiesChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceEvent {
    pub device_id: DeviceId,
    pub timestamp_ns: u64,
    pub kind: DeviceEventKind,
}

/// Reusable output vectors. clear() retains their allocated capacity.
#[derive(Debug, Default)]
pub struct InputBuffers {
    pub readings: Vec<DeviceReading>,
    pub events: Vec<DeviceEvent>,
}

impl InputBuffers {
    pub fn clear(&mut self) {
        self.readings.clear();
        self.events.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    pub backend: BackendKind,
    pub message: String,
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} backend: {}", self.backend, self.message)
    }
}

impl std::error::Error for BackendError {}

/// Backends own OS handles and device lifecycle. This trait is intentionally
/// not Send: a backend may be bound to the Qt/native event-loop thread.
pub trait InputBackend {
    fn kind(&self) -> BackendKind;

    /// Enumerate connected devices. Call at startup and after hotplug events.
    fn enumerate(&mut self, devices: &mut Vec<DeviceDescriptor>) -> Result<(), BackendError>;

    /// Append newest readings and lifecycle events to reusable caller buffers.
    fn poll_into(&mut self, output: &mut InputBuffers) -> Result<(), BackendError>;

    fn shutdown(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(u8);

impl ActionId {
    pub const fn new(value: u8) -> Option<Self> {
        if value as usize >= MAX_ACTIONS {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActionState {
    words: [u64; 2],
}

impl ActionState {
    pub fn set(&mut self, action: ActionId, active: bool) {
        let index = action.0 as usize;
        let mask = 1_u64 << (index % 64);
        if active {
            self.words[index / 64] |= mask;
        } else {
            self.words[index / 64] &= !mask;
        }
    }

    pub const fn is_active(&self, action: ActionId) -> bool {
        let index = action.0 as usize;
        self.words[index / 64] & (1_u64 << (index % 64)) != 0
    }

    pub const fn words(&self) -> [u64; 2] {
        self.words
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisDirection {
    Negative,
    Positive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalControl {
    Button(u8),
    Axis {
        index: u8,
        direction: AxisDirection,
        threshold: u16,
    },
    Hat {
        index: u8,
        direction_mask: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionBinding {
    pub action: ActionId,
    pub control: PhysicalControl,
}

/// Generic physical-to-logical profile. It does not encode any Sega port layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputProfile {
    bindings: Vec<ActionBinding>,
    deadzone: u16,
}

impl InputProfile {
    pub fn new(bindings: Vec<ActionBinding>, deadzone: u16) -> Result<Self, InputError> {
        if deadzone > i16::MAX as u16 {
            return Err(InputError::InvalidDeadzone(deadzone));
        }
        for binding in &bindings {
            match binding.control {
                PhysicalControl::Button(index) if index as usize >= MAX_BUTTONS => {
                    return Err(InputError::ButtonOutOfRange(index as usize));
                }
                PhysicalControl::Axis {
                    index, threshold, ..
                } => {
                    if index as usize >= MAX_AXES {
                        return Err(InputError::AxisOutOfRange(index as usize));
                    }
                    if threshold > i16::MAX as u16 {
                        return Err(InputError::InvalidThreshold(threshold));
                    }
                }
                PhysicalControl::Hat {
                    index,
                    direction_mask,
                } => {
                    if index as usize >= MAX_HATS {
                        return Err(InputError::HatOutOfRange(index as usize));
                    }
                    if direction_mask == 0 || direction_mask & !0x0F != 0 {
                        return Err(InputError::InvalidHatMask(direction_mask));
                    }
                }
                _ => {}
            }
        }
        Ok(Self { bindings, deadzone })
    }

    /// Evaluate into a caller-owned fixed-size action bitset; no allocation occurs.
    pub fn evaluate(&self, state: &DeviceState, output: &mut ActionState) {
        *output = ActionState::default();
        for binding in &self.bindings {
            let active = match binding.control {
                PhysicalControl::Button(index) => state.buttons.is_pressed(index as usize),
                PhysicalControl::Axis {
                    index,
                    direction,
                    threshold,
                } => {
                    let threshold = self.deadzone.max(threshold).max(1) as i32;
                    let value = state.axis(index as usize) as i32;
                    match direction {
                        AxisDirection::Negative => value <= -threshold,
                        AxisDirection::Positive => value >= threshold,
                    }
                }
                PhysicalControl::Hat {
                    index,
                    direction_mask,
                } => state.hat(index as usize) & direction_mask == direction_mask,
            };
            if active {
                output.set(binding.action, true);
            }
        }
    }

    pub fn bindings(&self) -> &[ActionBinding] {
        &self.bindings
    }

    pub const fn deadzone(&self) -> u16 {
        self.deadzone
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    ButtonOutOfRange(usize),
    AxisOutOfRange(usize),
    HatOutOfRange(usize),
    InvalidHatMask(u8),
    InvalidDeadzone(u16),
    InvalidThreshold(u16),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ButtonOutOfRange(index) => write!(f, "button index {index} is out of range"),
            Self::AxisOutOfRange(index) => write!(f, "axis index {index} is out of range"),
            Self::HatOutOfRange(index) => write!(f, "hat index {index} is out of range"),
            Self::InvalidHatMask(mask) => write!(f, "invalid hat direction mask {mask:#04x}"),
            Self::InvalidDeadzone(value) => write!(f, "invalid deadzone {value}"),
            Self::InvalidThreshold(value) => write!(f, "invalid axis threshold {value}"),
        }
    }
}

impl std::error::Error for InputError {}

#[cfg(test)]
mod tests {
    use super::{
        ActionBinding, ActionId, ActionState, AxisDirection, AxisSemantic, ButtonSet, DeviceState,
        InputBuffers, InputError, InputProfile, MAX_AXES, MAX_BUTTONS, PhysicalControl,
    };

    fn action(value: u8) -> ActionId {
        ActionId::new(value).expect("test action is within range")
    }

    #[test]
    fn button_set_covers_128_buttons_without_allocation() {
        let mut buttons = ButtonSet::default();
        for index in [0, 63, 64, 127] {
            buttons.set(index, true).unwrap();
            assert!(buttons.is_pressed(index));
        }
        assert!(!buttons.is_pressed(MAX_BUTTONS));
        assert_eq!(buttons.words(), [1 | (1_u64 << 63), 1 | (1_u64 << 63)]);
    }

    #[test]
    fn device_state_preserves_analog_axes_and_hat_diagonals() {
        let mut state = DeviceState::default();
        state.set_axis(0, -16_384, AxisSemantic::Wheel).unwrap();
        state
            .set_axis(1, 32_767, AxisSemantic::Accelerator)
            .unwrap();
        state.set_hat(0, 0b0011).unwrap();
        assert_eq!(state.axis(0), -16_384);
        assert_eq!(state.axis(1), 32_767);
        assert_eq!(state.axis_semantic(0), Some(AxisSemantic::Wheel));
        assert_eq!(state.axis_count(), 2);
        assert_eq!(state.hat(0), 0b0011);
    }

    #[test]
    fn profile_maps_buttons_axes_and_hats_to_generic_actions() {
        let profile = InputProfile::new(
            vec![
                ActionBinding {
                    action: action(0),
                    control: PhysicalControl::Button(0),
                },
                ActionBinding {
                    action: action(1),
                    control: PhysicalControl::Axis {
                        index: 0,
                        direction: AxisDirection::Negative,
                        threshold: 8_000,
                    },
                },
                ActionBinding {
                    action: action(2),
                    control: PhysicalControl::Hat {
                        index: 0,
                        direction_mask: 0b0011,
                    },
                },
            ],
            4_000,
        )
        .unwrap();
        let mut state = DeviceState::default();
        state.buttons.set(0, true).unwrap();
        state.set_axis(0, -12_000, AxisSemantic::Unknown).unwrap();
        state.set_hat(0, 0b0011).unwrap();
        let mut actions = ActionState::default();
        profile.evaluate(&state, &mut actions);
        assert!(actions.is_active(action(0)));
        assert!(actions.is_active(action(1)));
        assert!(actions.is_active(action(2)));
        assert!(!actions.is_active(action(3)));
    }

    #[test]
    fn deadzone_suppresses_axis_noise_and_neutral_is_not_pressed() {
        let profile = InputProfile::new(
            vec![ActionBinding {
                action: action(0),
                control: PhysicalControl::Axis {
                    index: 0,
                    direction: AxisDirection::Positive,
                    threshold: 0,
                },
            }],
            5_000,
        )
        .unwrap();
        let mut state = DeviceState::default();
        let mut actions = ActionState::default();
        state.set_axis(0, 0, AxisSemantic::Unknown).unwrap();
        profile.evaluate(&state, &mut actions);
        assert!(!actions.is_active(action(0)));
        state.set_axis(0, 4_999, AxisSemantic::Unknown).unwrap();
        profile.evaluate(&state, &mut actions);
        assert!(!actions.is_active(action(0)));
        state.set_axis(0, 5_000, AxisSemantic::Unknown).unwrap();
        profile.evaluate(&state, &mut actions);
        assert!(actions.is_active(action(0)));
    }

    #[test]
    fn invalid_profile_indices_and_ranges_are_rejected() {
        assert_eq!(
            InputProfile::new(
                vec![ActionBinding {
                    action: action(0),
                    control: PhysicalControl::Button(MAX_BUTTONS as u8),
                }],
                0
            )
            .unwrap_err(),
            InputError::ButtonOutOfRange(MAX_BUTTONS)
        );
        assert_eq!(
            InputProfile::new(
                vec![ActionBinding {
                    action: action(0),
                    control: PhysicalControl::Axis {
                        index: MAX_AXES as u8,
                        direction: AxisDirection::Positive,
                        threshold: 1,
                    },
                }],
                0
            )
            .unwrap_err(),
            InputError::AxisOutOfRange(MAX_AXES)
        );
        assert_eq!(
            InputProfile::new(Vec::new(), 32_768).unwrap_err(),
            InputError::InvalidDeadzone(32_768)
        );
    }

    #[test]
    fn clearing_poll_buffers_retains_allocated_capacity() {
        let mut buffers = InputBuffers::default();
        buffers.readings.reserve(16);
        buffers.events.reserve(8);
        let reading_capacity = buffers.readings.capacity();
        let event_capacity = buffers.events.capacity();
        buffers.clear();
        assert_eq!(buffers.readings.capacity(), reading_capacity);
        assert_eq!(buffers.events.capacity(), event_capacity);
    }
}
