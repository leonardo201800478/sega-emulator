//! Python-facing facade for the Rust-owned physical input runtime.
//!
//! Python receives semantic actions and device labels only. Device polling,
//! physical mapping, analog deadzones, hotplug and turbo phase stay in Rust.

use std::collections::HashMap;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sega_input_runtime::sdl3_backend::Sdl3Backend;
use sega_input_runtime::{
    ActionBinding, ActionId, ActionState, AxisDirection, BackendError, DeviceDescriptor,
    DeviceEventKind, DeviceId, InputBackend, InputBuffers, InputProfile, PhysicalControl,
};

const ACTIONS: [&str; 7] = ["up", "down", "left", "right", "button1", "button2", "hold"];

fn action_id(name: &str) -> Option<ActionId> {
    ACTIONS
        .iter()
        .position(|candidate| *candidate == name)
        .and_then(|index| ActionId::new(index as u8))
}

fn standard_button(name: &str) -> Option<u8> {
    Some(match name {
        "a" => 0,
        "b" => 1,
        "x" => 2,
        "y" => 3,
        "back" => 4,
        "guide" => 5,
        "start" => 6,
        "leftstick" => 7,
        "rightstick" => 8,
        "leftshoulder" => 9,
        "rightshoulder" => 10,
        "dpad_up" => 11,
        "dpad_down" => 12,
        "dpad_left" => 13,
        "dpad_right" => 14,
        "misc1" => 15,
        "paddle1" => 16,
        "paddle2" => 17,
        "paddle3" => 18,
        "paddle4" => 19,
        "touchpad" => 20,
        _ => return None,
    })
}

fn parse_indexed_binding(value: &str, prefix: &str) -> Option<usize> {
    value.strip_prefix(prefix)?.parse::<usize>().ok()
}

fn parse_binding(value: &str, deadzone: u16) -> PyResult<PhysicalControl> {
    if let Some(index) = standard_button(value) {
        return Ok(PhysicalControl::Button(index));
    }
    if let Some(index) = parse_indexed_binding(value, "raw_button_") {
        let physical = index.checked_add(32).filter(|value| *value <= 127);
        return physical
            .map(|index| PhysicalControl::Button(index as u8))
            .ok_or_else(|| PyValueError::new_err("raw_button index deve estar entre 0 e 95"));
    }
    if let Some(rest) = value.strip_prefix("raw_axis_") {
        let (index, direction) = rest.split_once('_').ok_or_else(|| {
            PyValueError::new_err("binding raw_axis inválido")
        })?;
        let index = index
            .parse::<usize>()
            .map_err(|_| PyValueError::new_err("índice raw_axis inválido"))?;
        let physical = index.checked_add(16).filter(|value| *value < 32);
        let direction = match direction {
            "negative" => AxisDirection::Negative,
            "positive" => AxisDirection::Positive,
            _ => return Err(PyValueError::new_err("direção raw_axis inválida")),
        };
        return physical
            .map(|index| PhysicalControl::Axis {
                index: index as u8,
                direction,
                threshold: deadzone.max(1),
            })
            .ok_or_else(|| PyValueError::new_err("raw_axis index deve estar entre 0 e 15"));
    }
    if let Some(rest) = value.strip_prefix("raw_hat_") {
        let (index, direction) = rest.split_once('_').ok_or_else(|| {
            PyValueError::new_err("binding raw_hat inválido")
        })?;
        let index = index
            .parse::<usize>()
            .map_err(|_| PyValueError::new_err("índice raw_hat inválido"))?;
        if index >= 8 {
            return Err(PyValueError::new_err("raw_hat index deve estar entre 0 e 7"));
        }
        let direction_mask = match direction {
            "up" => 1,
            "right" => 2,
            "down" => 4,
            "left" => 8,
            _ => return Err(PyValueError::new_err("direção raw_hat inválida")),
        };
        return Ok(PhysicalControl::Hat {
            index: index as u8,
            direction_mask,
        });
    }
    Err(PyValueError::new_err(format!(
        "binding físico não suportado: {value}"
    )))
}

fn collect_actions(state: &ActionState, turbo: [bool; 2], frame: u64) -> Vec<String> {
    ACTIONS
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let action = ActionId::new(index as u8)?;
            let mut active = state.is_active(action);
            if frame % 4 >= 2 {
                if (*name == "button1" && turbo[0]) || (*name == "button2" && turbo[1]) {
                    active = false;
                }
            }
            active.then(|| (*name).to_owned())
        })
        .collect()
}

#[pyclass(unsendable, module = "sega_z80_core")]
pub struct NativeInputManager {
    backend: Option<Sdl3Backend>,
    backend_error: Option<String>,
    buffers: InputBuffers,
    devices: Vec<DeviceDescriptor>,
    assignments: [Option<DeviceId>; 2],
    profiles: [Option<InputProfile>; 2],
    turbo: [[bool; 2]; 2],
    disconnect_event: bool,
}

impl NativeInputManager {
    fn refresh_devices(&mut self) -> Result<(), BackendError> {
        let Some(backend) = self.backend.as_mut() else {
            return Ok(());
        };
        backend.enumerate(&mut self.devices)
    }

    fn assign_unclaimed_devices(&mut self) {
        for player in 0..2 {
            if self.assignments[player].is_some() {
                continue;
            }
            let candidate = self.devices.iter().find(|device| {
                !self.assignments.iter().flatten().any(|id| *id == device.id)
            });
            if let Some(device) = candidate {
                self.assignments[player] = Some(device.id);
            }
        }
    }

    fn poll_backend(&mut self) -> PyResult<()> {
        let Some(backend) = self.backend.as_mut() else {
            self.buffers.clear();
            return Ok(());
        };
        self.buffers.clear();
        backend
            .poll_into(&mut self.buffers)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

        for event in &self.buffers.events {
            if event.kind == DeviceEventKind::Disconnected {
                self.disconnect_event = true;
                for assignment in &mut self.assignments {
                    if *assignment == Some(event.device_id) {
                        *assignment = None;
                    }
                }
            }
        }
        if !self.buffers.events.is_empty() {
            self.refresh_devices()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            self.assign_unclaimed_devices();
        }
        Ok(())
    }

    fn state_for_player(&self, player: usize) -> Option<&sega_input_runtime::DeviceState> {
        let id = self.assignments[player]?;
        self.buffers
            .readings
            .iter()
            .rev()
            .find(|reading| reading.device_id == id)
            .map(|reading| &reading.state)
    }
}

#[pymethods]
impl NativeInputManager {
    #[new]
    fn new() -> Self {
        let mut manager = Self {
            backend: None,
            backend_error: None,
            buffers: InputBuffers::default(),
            devices: Vec::new(),
            assignments: [None, None],
            profiles: [None, None],
            turbo: [[false; 2]; 2],
            disconnect_event: false,
        };
        match Sdl3Backend::new() {
            Ok(backend) => {
                manager.backend = Some(backend);
                if let Err(error) = manager.refresh_devices() {
                    manager.backend_error = Some(error.to_string());
                    manager.backend = None;
                } else {
                    manager.assign_unclaimed_devices();
                }
            }
            Err(error) => manager.backend_error = Some(error.to_string()),
        }
        manager
    }

    #[getter]
    fn available(&self) -> bool {
        self.backend.is_some()
    }

    #[getter]
    fn backend_error(&self) -> Option<String> {
        self.backend_error.clone()
    }

    fn set_backend_preference(&self, preference: &str) -> PyResult<()> {
        if !matches!(preference, "auto" | "sdl3" | "xinput") {
            return Err(PyValueError::new_err("preferência de backend inválida"));
        }
        Ok(())
    }

    #[pyo3(signature = (player, bindings, use_left_stick, deadzone, turbo_button1=false, turbo_button2=false))]
    fn set_profile(
        &mut self,
        player: u8,
        bindings: HashMap<String, String>,
        use_left_stick: bool,
        deadzone: f64,
        turbo_button1: bool,
        turbo_button2: bool,
    ) -> PyResult<()> {
        if !matches!(player, 1 | 2) {
            return Err(PyValueError::new_err("player deve ser 1 ou 2"));
        }
        if !deadzone.is_finite() || !(0.0..1.0).contains(&deadzone) {
            return Err(PyValueError::new_err("deadzone deve estar no intervalo [0, 1)"));
        }
        let threshold = (deadzone * i16::MAX as f64).round() as u16;
        let mut result = Vec::with_capacity(9);
        for (name, binding) in bindings {
            let action = action_id(&name)
                .ok_or_else(|| PyValueError::new_err(format!("ação desconhecida: {name}")))?;
            result.push(ActionBinding {
                action,
                control: parse_binding(&binding, threshold)?,
            });
        }
        if use_left_stick {
            for (action_name, axis, direction) in [
                ("left", 0, AxisDirection::Negative),
                ("right", 0, AxisDirection::Positive),
                ("up", 1, AxisDirection::Negative),
                ("down", 1, AxisDirection::Positive),
            ] {
                result.push(ActionBinding {
                    action: action_id(action_name).expect("known action"),
                    control: PhysicalControl::Axis {
                        index: axis,
                        direction,
                        threshold: threshold.max(1),
                    },
                });
            }
        }
        let profile = InputProfile::new(result, threshold)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let index = (player - 1) as usize;
        self.profiles[index] = Some(profile);
        self.turbo[index] = [turbo_button1, turbo_button2];
        Ok(())
    }

    fn poll_actions(&mut self, frame_number: u64) -> PyResult<(Vec<String>, Vec<String>)> {
        self.poll_backend()?;
        let mut output: [Vec<String>; 2] = [Vec::new(), Vec::new()];
        for player in 0..2 {
            let (Some(profile), Some(state)) =
                (self.profiles[player].as_ref(), self.state_for_player(player))
            else {
                continue;
            };
            let mut actions = ActionState::default();
            profile.evaluate(state, &mut actions);
            output[player] = collect_actions(&actions, self.turbo[player], frame_number);
        }
        Ok((output[0].clone(), output[1].clone()))
    }

    fn read_pressed_button(&mut self, player: u8) -> PyResult<Option<String>> {
        if !matches!(player, 1 | 2) {
            return Err(PyValueError::new_err("player deve ser 1 ou 2"));
        }
        self.poll_backend()?;
        let Some(state) = self.state_for_player((player - 1) as usize) else {
            return Ok(None);
        };
        const BUTTON_NAMES: [&str; 21] = [
            "a", "b", "x", "y", "back", "guide", "start", "leftstick", "rightstick",
            "leftshoulder", "rightshoulder", "dpad_up", "dpad_down", "dpad_left",
            "dpad_right", "misc1", "paddle1", "paddle2", "paddle3", "paddle4", "touchpad",
        ];
        for (index, name) in BUTTON_NAMES.iter().enumerate() {
            if state.buttons.is_pressed(index) {
                return Ok(Some((*name).to_owned()));
            }
        }
        for index in 0..96 {
            if state.buttons.is_pressed(32 + index) {
                return Ok(Some(format!("raw_button_{index}")));
            }
        }
        for index in 0..8 {
            let hat = state.hat(index);
            for (mask, direction) in [(1, "up"), (2, "right"), (4, "down"), (8, "left")] {
                if hat & mask != 0 {
                    return Ok(Some(format!("raw_hat_{index}_{direction}")));
                }
            }
        }
        for index in 0..16 {
            let axis = state.axis(16 + index);
            if axis <= -18_000 {
                return Ok(Some(format!("raw_axis_{index}_negative")));
            }
            if axis >= 18_000 {
                return Ok(Some(format!("raw_axis_{index}_positive")));
            }
        }
        Ok(None)
    }

    fn connected_names(&self) -> Vec<String> {
        self.devices.iter().map(|device| device.name.clone()).collect()
    }

    fn consume_disconnect_event(&mut self) -> bool {
        std::mem::take(&mut self.disconnect_event)
    }

    fn close(&mut self) {
        if let Some(mut backend) = self.backend.take() {
            backend.shutdown();
        }
        self.devices.clear();
        self.assignments = [None, None];
    }
}
