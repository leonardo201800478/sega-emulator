//! Windows SDL3 joystick/gamepad backend loaded through its stable C ABI.
//!
//! The frontend never calls SDL or Python controller APIs. This backend is
//! caller-polled on the Qt thread and preserves raw HID-derived axes, buttons,
//! hats, plus SDL's standardized gamepad mapping where available.

use crate::{BackendError, BackendKind, DeviceDescriptor, InputBackend, InputBuffers};

#[cfg(windows)]
use crate::{
    AxisSemantic, DeviceCapabilities, DeviceClass, DeviceEvent, DeviceEventKind, DeviceId,
    DeviceReading, DeviceState, MAX_AXES, MAX_BUTTONS, MAX_HATS,
};

#[cfg(windows)]
mod windows {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::mem::transmute_copy;
    use std::ptr;
    use std::time::{Duration, Instant};

    type SdlBool = bool;
    type SdlJoystickId = u32;

    const SDL_INIT_JOYSTICK: u32 = 0x0000_0200;
    const SDL_INIT_GAMEPAD: u32 = 0x0000_2000;
    const SDL_INIT_EVENTS: u32 = 0x0000_4000;
    const GAMEPAD_BUTTON_COUNT: usize = 21;
    const GAMEPAD_AXIS_COUNT: usize = 6;
    const RAW_BUTTON_OFFSET: usize = 32;
    const RAW_AXIS_OFFSET: usize = 16;

    #[link(name = "kernel32")]
    #[allow(non_snake_case)]
    unsafe extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }

    type InitSubSystem = unsafe extern "C" fn(u32) -> SdlBool;
    type QuitSubSystem = unsafe extern "C" fn(u32);
    type GetJoysticks = unsafe extern "C" fn(*mut c_int) -> *mut SdlJoystickId;
    type IsGamepad = unsafe extern "C" fn(SdlJoystickId) -> SdlBool;
    type OpenJoystick = unsafe extern "C" fn(SdlJoystickId) -> *mut c_void;
    type OpenGamepad = unsafe extern "C" fn(SdlJoystickId) -> *mut c_void;
    type GetGamepadJoystick = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
    type CloseJoystick = unsafe extern "C" fn(*mut c_void);
    type CloseGamepad = unsafe extern "C" fn(*mut c_void);
    type GetJoystickName = unsafe extern "C" fn(*mut c_void) -> *const c_char;
    type GetJoystickVendor = unsafe extern "C" fn(*mut c_void) -> u16;
    type GetJoystickProduct = unsafe extern "C" fn(*mut c_void) -> u16;
    type GetJoystickType = unsafe extern "C" fn(*mut c_void) -> c_int;
    type GetNumJoystickAxes = unsafe extern "C" fn(*mut c_void) -> c_int;
    type GetNumJoystickButtons = unsafe extern "C" fn(*mut c_void) -> c_int;
    type GetNumJoystickHats = unsafe extern "C" fn(*mut c_void) -> c_int;
    type GetJoystickAxis = unsafe extern "C" fn(*mut c_void, c_int) -> i16;
    type GetJoystickButton = unsafe extern "C" fn(*mut c_void, c_int) -> SdlBool;
    type GetJoystickHat = unsafe extern "C" fn(*mut c_void, c_int) -> u8;
    type JoystickConnected = unsafe extern "C" fn(*mut c_void) -> SdlBool;
    type GamepadConnected = unsafe extern "C" fn(*mut c_void) -> SdlBool;
    type GetGamepadButton = unsafe extern "C" fn(*mut c_void, c_int) -> SdlBool;
    type GetGamepadAxis = unsafe extern "C" fn(*mut c_void, c_int) -> i16;
    type PumpEvents = unsafe extern "C" fn();
    type UpdateJoysticks = unsafe extern "C" fn();
    type UpdateGamepads = unsafe extern "C" fn();
    type Free = unsafe extern "C" fn(*mut c_void);
    type GetError = unsafe extern "C" fn() -> *const c_char;

    struct SdlApi {
        handle: *mut c_void,
        init_subsystem: InitSubSystem,
        quit_subsystem: QuitSubSystem,
        get_joysticks: GetJoysticks,
        is_gamepad: IsGamepad,
        open_joystick: OpenJoystick,
        open_gamepad: OpenGamepad,
        get_gamepad_joystick: GetGamepadJoystick,
        close_joystick: CloseJoystick,
        close_gamepad: CloseGamepad,
        get_joystick_name: GetJoystickName,
        get_joystick_vendor: GetJoystickVendor,
        get_joystick_product: GetJoystickProduct,
        get_joystick_type: GetJoystickType,
        get_num_axes: GetNumJoystickAxes,
        get_num_buttons: GetNumJoystickButtons,
        get_num_hats: GetNumJoystickHats,
        get_axis: GetJoystickAxis,
        get_button: GetJoystickButton,
        get_hat: GetJoystickHat,
        joystick_connected: JoystickConnected,
        gamepad_connected: GamepadConnected,
        get_gamepad_button: GetGamepadButton,
        get_gamepad_axis: GetGamepadAxis,
        pump_events: PumpEvents,
        update_joysticks: UpdateJoysticks,
        update_gamepads: UpdateGamepads,
        free: Free,
        get_error: GetError,
    }

    impl SdlApi {
        unsafe fn load_symbol<T: Copy>(
            handle: *mut c_void,
            symbol: &'static str,
        ) -> Result<T, String> {
            let name = CString::new(symbol).expect("static SDL symbol has no NUL");
            let address = unsafe { GetProcAddress(handle, name.as_ptr()) };
            if address.is_null() {
                return Err(format!("SDL3.dll não exporta {symbol}"));
            }
            Ok(unsafe { transmute_copy(&address) })
        }

        fn load() -> Result<Self, String> {
            let configured = std::env::var_os("SEGA_EMULATOR_SDL3_DLL");
            let path = configured.unwrap_or_else(|| "SDL3.dll".into());
            let wide: Vec<u16> = path
                .to_string_lossy()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return Err(format!(
                    "SDL3.dll não foi encontrado (SEGA_EMULATOR_SDL3_DLL={})",
                    path.to_string_lossy()
                ));
            }

            macro_rules! load {
                ($name:literal, $ty:ty) => {
                    match unsafe { Self::load_symbol::<$ty>(handle, $name) } {
                        Ok(value) => value,
                        Err(error) => {
                            unsafe { FreeLibrary(handle) };
                            return Err(error);
                        }
                    }
                };
            }

            Ok(Self {
                handle,
                init_subsystem: load!("SDL_InitSubSystem", InitSubSystem),
                quit_subsystem: load!("SDL_QuitSubSystem", QuitSubSystem),
                get_joysticks: load!("SDL_GetJoysticks", GetJoysticks),
                is_gamepad: load!("SDL_IsGamepad", IsGamepad),
                open_joystick: load!("SDL_OpenJoystick", OpenJoystick),
                open_gamepad: load!("SDL_OpenGamepad", OpenGamepad),
                get_gamepad_joystick: load!("SDL_GetGamepadJoystick", GetGamepadJoystick),
                close_joystick: load!("SDL_CloseJoystick", CloseJoystick),
                close_gamepad: load!("SDL_CloseGamepad", CloseGamepad),
                get_joystick_name: load!("SDL_GetJoystickName", GetJoystickName),
                get_joystick_vendor: load!("SDL_GetJoystickVendor", GetJoystickVendor),
                get_joystick_product: load!("SDL_GetJoystickProduct", GetJoystickProduct),
                get_joystick_type: load!("SDL_GetJoystickType", GetJoystickType),
                get_num_axes: load!("SDL_GetNumJoystickAxes", GetNumJoystickAxes),
                get_num_buttons: load!("SDL_GetNumJoystickButtons", GetNumJoystickButtons),
                get_num_hats: load!("SDL_GetNumJoystickHats", GetNumJoystickHats),
                get_axis: load!("SDL_GetJoystickAxis", GetJoystickAxis),
                get_button: load!("SDL_GetJoystickButton", GetJoystickButton),
                get_hat: load!("SDL_GetJoystickHat", GetJoystickHat),
                joystick_connected: load!("SDL_JoystickConnected", JoystickConnected),
                gamepad_connected: load!("SDL_GamepadConnected", GamepadConnected),
                get_gamepad_button: load!("SDL_GetGamepadButton", GetGamepadButton),
                get_gamepad_axis: load!("SDL_GetGamepadAxis", GetGamepadAxis),
                pump_events: load!("SDL_PumpEvents", PumpEvents),
                update_joysticks: load!("SDL_UpdateJoysticks", UpdateJoysticks),
                update_gamepads: load!("SDL_UpdateGamepads", UpdateGamepads),
                free: load!("SDL_free", Free),
                get_error: load!("SDL_GetError", GetError),
            })
        }

        fn error(&self, operation: &str) -> String {
            let message = unsafe { (self.get_error)() };
            let message = if message.is_null() {
                "erro SDL3 sem descrição".to_owned()
            } else {
                unsafe { CStr::from_ptr(message) }
                    .to_string_lossy()
                    .into_owned()
            };
            format!("{operation}: {message}")
        }
    }

    impl Drop for SdlApi {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { FreeLibrary(self.handle) };
                self.handle = ptr::null_mut();
            }
        }
    }

    struct OpenDevice {
        id: DeviceId,
        name: String,
        vendor_id: u16,
        product_id: u16,
        device_class: DeviceClass,
        joystick: *mut c_void,
        gamepad: *mut c_void,
        raw_axes: usize,
        raw_buttons: usize,
        raw_hats: usize,
    }

    impl OpenDevice {
        fn descriptor(&self) -> DeviceDescriptor {
            DeviceDescriptor {
                id: self.id,
                backend: BackendKind::Sdl3,
                class: self.device_class,
                name: self.name.clone(),
                vendor_id: (self.vendor_id != 0).then_some(self.vendor_id),
                product_id: (self.product_id != 0).then_some(self.product_id),
                physical_key: None,
                capabilities: DeviceCapabilities {
                    button_count: self
                        .raw_buttons
                        .max(if self.gamepad.is_null() {
                            0
                        } else {
                            GAMEPAD_BUTTON_COUNT
                        })
                        .min(MAX_BUTTONS) as u16,
                    axis_count: self
                        .raw_axes
                        .max(if self.gamepad.is_null() {
                            0
                        } else {
                            GAMEPAD_AXIS_COUNT
                        })
                        .min(MAX_AXES) as u8,
                    hat_count: self.raw_hats.min(MAX_HATS) as u8,
                    rumble: false,
                    force_feedback: false,
                    gyro: false,
                    accelerometer: false,
                    touchpad: false,
                },
            }
        }
    }

    /// SDL3 backend. All SDL handles are owned and polled on the creating thread.
    pub struct Sdl3Backend {
        api: SdlApi,
        devices: HashMap<u32, OpenDevice>,
        next_device_id: u64,
        sequence: u64,
        started: Instant,
        last_scan: Instant,
        initialized: bool,
    }

    impl Sdl3Backend {
        pub fn new() -> Result<Self, BackendError> {
            let api = SdlApi::load().map_err(|message| BackendError {
                backend: BackendKind::Sdl3,
                message,
            })?;
            let flags = SDL_INIT_JOYSTICK | SDL_INIT_GAMEPAD | SDL_INIT_EVENTS;
            if !unsafe { (api.init_subsystem)(flags) } {
                return Err(BackendError {
                    backend: BackendKind::Sdl3,
                    message: api.error("SDL_InitSubSystem"),
                });
            }
            let now = Instant::now();
            Ok(Self {
                api,
                devices: HashMap::new(),
                next_device_id: 1,
                sequence: 0,
                started: now,
                last_scan: now,
                initialized: true,
            })
        }

        fn timestamp_ns(&self) -> u64 {
            self.started.elapsed().as_nanos().min(u64::MAX as u128) as u64
        }

        fn refresh(&mut self, output: &mut InputBuffers) -> Result<(), BackendError> {
            let mut count = 0;
            let ids = unsafe { (self.api.get_joysticks)(&mut count) };
            if ids.is_null() && count > 0 {
                return Err(BackendError {
                    backend: BackendKind::Sdl3,
                    message: self.api.error("SDL_GetJoysticks"),
                });
            }
            let connected: HashSet<u32> = if ids.is_null() || count <= 0 {
                HashSet::new()
            } else {
                let values = unsafe { std::slice::from_raw_parts(ids, count as usize) };
                values.iter().copied().collect()
            };
            if !ids.is_null() {
                unsafe { (self.api.free)(ids.cast()) };
            }

            let mut removed: Vec<u32> = self
                .devices
                .keys()
                .filter(|id| !connected.contains(id))
                .copied()
                .collect();
            removed.sort_unstable();
            for raw_id in removed {
                if let Some(device) = self.devices.remove(&raw_id) {
                    let device_id = device.id;
                    self.close_device(device);
                    output.events.push(DeviceEvent {
                        device_id,
                        timestamp_ns: self.timestamp_ns(),
                        kind: DeviceEventKind::Disconnected,
                    });
                }
            }

            let mut connected_ids: Vec<u32> = connected.into_iter().collect();
            connected_ids.sort_unstable();
            for raw_id in connected_ids {
                if self.devices.contains_key(&raw_id) {
                    continue;
                }
                let is_gamepad = unsafe { (self.api.is_gamepad)(raw_id) };
                let gamepad = if is_gamepad {
                    unsafe { (self.api.open_gamepad)(raw_id) }
                } else {
                    ptr::null_mut()
                };
                let joystick = if !gamepad.is_null() {
                    unsafe { (self.api.get_gamepad_joystick)(gamepad) }
                } else {
                    unsafe { (self.api.open_joystick)(raw_id) }
                };
                if joystick.is_null() {
                    if !gamepad.is_null() {
                        unsafe { (self.api.close_gamepad)(gamepad) };
                    }
                    continue;
                }
                let name_ptr = unsafe { (self.api.get_joystick_name)(joystick) };
                let name = if name_ptr.is_null() {
                    format!("SDL device {raw_id}")
                } else {
                    unsafe { CStr::from_ptr(name_ptr) }
                        .to_string_lossy()
                        .into_owned()
                };
                let joystick_type = unsafe { (self.api.get_joystick_type)(joystick) };
                let device_class = if !gamepad.is_null() {
                    DeviceClass::Gamepad
                } else {
                    match joystick_type {
                        2 => DeviceClass::Wheel,
                        3 | 8 => DeviceClass::ArcadeStick,
                        4 => DeviceClass::FlightStick,
                        _ => DeviceClass::Joystick,
                    }
                };
                let raw_axes = unsafe { (self.api.get_num_axes)(joystick) }.max(0) as usize;
                let raw_buttons = unsafe { (self.api.get_num_buttons)(joystick) }.max(0) as usize;
                let raw_hats = unsafe { (self.api.get_num_hats)(joystick) }.max(0) as usize;
                let device = OpenDevice {
                    id: DeviceId(self.next_device_id),
                    name,
                    vendor_id: unsafe { (self.api.get_joystick_vendor)(joystick) },
                    product_id: unsafe { (self.api.get_joystick_product)(joystick) },
                    device_class,
                    joystick,
                    gamepad,
                    raw_axes,
                    raw_buttons,
                    raw_hats,
                };
                self.next_device_id = self.next_device_id.saturating_add(1);
                output.events.push(DeviceEvent {
                    device_id: device.id,
                    timestamp_ns: self.timestamp_ns(),
                    kind: DeviceEventKind::Connected,
                });
                self.devices.insert(raw_id, device);
            }
            self.last_scan = Instant::now();
            Ok(())
        }

        fn close_device(&self, device: OpenDevice) {
            unsafe {
                if !device.gamepad.is_null() {
                    // The joystick handle is owned by the gamepad.
                    (self.api.close_gamepad)(device.gamepad);
                } else if !device.joystick.is_null() {
                    (self.api.close_joystick)(device.joystick);
                }
            }
        }

        fn read_device(&self, device: &OpenDevice) -> DeviceState {
            let mut state = DeviceState::default();
            if !device.gamepad.is_null() {
                for button in 0..GAMEPAD_BUTTON_COUNT {
                    let pressed =
                        unsafe { (self.api.get_gamepad_button)(device.gamepad, button as c_int) };
                    let _ = state.buttons.set(button, pressed);
                }
                let semantics = [
                    AxisSemantic::LeftStickX,
                    AxisSemantic::LeftStickY,
                    AxisSemantic::RightStickX,
                    AxisSemantic::RightStickY,
                    AxisSemantic::LeftTrigger,
                    AxisSemantic::RightTrigger,
                ];
                for (axis, semantic) in semantics.into_iter().enumerate() {
                    let value =
                        unsafe { (self.api.get_gamepad_axis)(device.gamepad, axis as c_int) };
                    let _ = state.set_axis(axis, value, semantic);
                }
            }

            for button in 0..device.raw_buttons.min(MAX_BUTTONS - RAW_BUTTON_OFFSET) {
                let pressed = unsafe { (self.api.get_button)(device.joystick, button as c_int) };
                let _ = state.buttons.set(RAW_BUTTON_OFFSET + button, pressed);
            }
            for axis in 0..device.raw_axes.min(MAX_AXES - RAW_AXIS_OFFSET) {
                let value = unsafe { (self.api.get_axis)(device.joystick, axis as c_int) };
                let _ = state.set_axis(RAW_AXIS_OFFSET + axis, value, AxisSemantic::Unknown);
                if device.gamepad.is_null() && axis < 2 {
                    let semantic = if axis == 0 {
                        AxisSemantic::LeftStickX
                    } else {
                        AxisSemantic::LeftStickY
                    };
                    let _ = state.set_axis(axis, value, semantic);
                }
            }
            for hat in 0..device.raw_hats.min(MAX_HATS) {
                let value = unsafe { (self.api.get_hat)(device.joystick, hat as c_int) };
                let directions = value & 0x0F;
                let _ = state.set_hat(hat, directions);
            }
            state
        }
    }

    impl InputBackend for Sdl3Backend {
        fn kind(&self) -> BackendKind {
            BackendKind::Sdl3
        }

        fn enumerate(&mut self, devices: &mut Vec<DeviceDescriptor>) -> Result<(), BackendError> {
            let mut events = InputBuffers::default();
            self.refresh(&mut events)?;
            devices.clear();
            devices.extend(self.devices.values().map(OpenDevice::descriptor));
            devices.sort_by_key(|device| device.id);
            Ok(())
        }

        fn poll_into(&mut self, output: &mut InputBuffers) -> Result<(), BackendError> {
            unsafe {
                (self.api.pump_events)();
                (self.api.update_joysticks)();
                (self.api.update_gamepads)();
            }
            if self.last_scan.elapsed() >= Duration::from_millis(250) {
                self.refresh(output)?;
            }
            let timestamp_ns = self.timestamp_ns();
            for device in self.devices.values() {
                let connected = unsafe {
                    if device.gamepad.is_null() {
                        (self.api.joystick_connected)(device.joystick)
                    } else {
                        (self.api.gamepad_connected)(device.gamepad)
                    }
                };
                if !connected {
                    continue;
                }
                self.sequence = self.sequence.saturating_add(1);
                output.readings.push(DeviceReading {
                    device_id: device.id,
                    timestamp_ns,
                    sequence: self.sequence,
                    state: self.read_device(device),
                });
            }
            Ok(())
        }

        fn shutdown(&mut self) {
            let devices = std::mem::take(&mut self.devices);
            for (_, device) in devices {
                self.close_device(device);
            }
            if self.initialized {
                unsafe {
                    (self.api.quit_subsystem)(
                        SDL_INIT_JOYSTICK | SDL_INIT_GAMEPAD | SDL_INIT_EVENTS,
                    )
                };
                self.initialized = false;
            }
        }
    }

    impl Drop for Sdl3Backend {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
}

#[cfg(windows)]
pub use windows::Sdl3Backend;

#[cfg(not(windows))]
pub struct Sdl3Backend;

#[cfg(not(windows))]
impl Sdl3Backend {
    pub fn new() -> Result<Self, BackendError> {
        Err(BackendError {
            backend: BackendKind::Sdl3,
            message: "backend SDL3 dinâmico está disponível inicialmente apenas no Windows".into(),
        })
    }
}

#[cfg(not(windows))]
impl InputBackend for Sdl3Backend {
    fn kind(&self) -> BackendKind {
        BackendKind::Sdl3
    }

    fn enumerate(&mut self, _devices: &mut Vec<DeviceDescriptor>) -> Result<(), BackendError> {
        Err(BackendError {
            backend: BackendKind::Sdl3,
            message: "plataforma não suportada".into(),
        })
    }

    fn poll_into(&mut self, _output: &mut InputBuffers) -> Result<(), BackendError> {
        Err(BackendError {
            backend: BackendKind::Sdl3,
            message: "plataforma não suportada".into(),
        })
    }

    fn shutdown(&mut self) {}
}
