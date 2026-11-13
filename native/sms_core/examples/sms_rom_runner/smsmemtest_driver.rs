//! Deterministic, frame-boundary input driver for the upstream SMSmemtest flow.
//!
//! The driver detects only the documented prompt rows. It does not infer RAM or
//! VRAM PASS/FAIL from rendered pixels; that requires a separate case oracle.

use sega_sms_core::{ControllerButtons, SmsTemporalMachine};

const FRAME_WIDTH: usize = 256;
const RGB_BYTES_PER_PIXEL: usize = 3;
const TEXT_CELL_HEIGHT: usize = 8;
const INITIAL_PROMPT_TEXT_ROW: usize = 3;
const RAM_COMPLETE_PROMPT_TEXT_ROW: usize = 14;
const VRAM_COMPLETE_PROMPT_TEXT_ROW: usize = 12;
const REQUIRED_STABLE_FRAMES: u8 = 2;
const MIN_VISIBLE_PIXELS: usize = 8;
const CHANNEL_THRESHOLD: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    AwaitInitialPrompt,
    AwaitRamCompletePrompt,
    AwaitVramCompletePrompt,
    Complete,
}

#[derive(Debug, Default)]
pub struct SmsmemtestInputDriver {
    stage: Option<Stage>,
    stable_frames: u8,
    release_button_after_frame: bool,
}

impl SmsmemtestInputDriver {
    pub fn new() -> Self {
        Self {
            stage: Some(Stage::AwaitInitialPrompt),
            stable_frames: 0,
            release_button_after_frame: false,
        }
    }

    /// Observe one completed RGB24 frame and emit deterministic input events.
    ///
    /// Button transitions are applied only after a complete frame. A press is
    /// held through the following frame, then released at its boundary.
    pub fn after_frame(
        &mut self,
        machine: &mut SmsTemporalMachine,
    ) -> Result<Vec<&'static str>, String> {
        let mut events = Vec::new();

        if self.release_button_after_frame {
            machine
                .set_controller(1, ControllerButtons::default())
                .map_err(|error| format!("cannot release SMSmemtest controller: {error}"))?;
            self.release_button_after_frame = false;
            events.push("button_released");
        }

        let Some(stage) = self.stage else {
            return Ok(events);
        };

        let (row, next_stage) = match stage {
            Stage::AwaitInitialPrompt => (
                INITIAL_PROMPT_TEXT_ROW,
                Some(Stage::AwaitRamCompletePrompt),
            ),
            Stage::AwaitRamCompletePrompt => (
                RAM_COMPLETE_PROMPT_TEXT_ROW,
                Some(Stage::AwaitVramCompletePrompt),
            ),
            Stage::AwaitVramCompletePrompt => (VRAM_COMPLETE_PROMPT_TEXT_ROW, None),
            Stage::Complete => return Ok(events),
        };

        if text_row_has_visible_pixels(machine.framebuffer(), row) {
            self.stable_frames = self.stable_frames.saturating_add(1);
        } else {
            self.stable_frames = 0;
        }

        if self.stable_frames < REQUIRED_STABLE_FRAMES {
            return Ok(events);
        }

        self.stable_frames = 0;
        match next_stage {
            Some(next) => {
                machine
                    .set_controller(
                        1,
                        ControllerButtons {
                            button_1: true,
                            ..ControllerButtons::default()
                        },
                    )
                    .map_err(|error| format!("cannot press SMSmemtest controller: {error}"))?;
                self.release_button_after_frame = true;
                self.stage = Some(next);
                events.push("button_pressed");
            }
            None => {
                self.stage = Some(Stage::Complete);
                events.push("vram_completion_row_activity_detected");
            }
        }

        Ok(events)
    }

    pub fn is_complete(&self) -> bool {
        self.stage == Some(Stage::Complete)
    }
}

/// Return true when a text-cell row contains enough non-black RGB24 pixels.
///
/// SMSmemtest uses an 8x8 text grid. This is deliberately a prompt-presence
/// detector, not OCR and not a per-case result classifier.
pub fn text_row_has_visible_pixels(framebuffer: &[u8], text_row: usize) -> bool {
    let stride = FRAME_WIDTH * RGB_BYTES_PER_PIXEL;
    let height = framebuffer.len() / stride;
    let start_y = text_row.saturating_mul(TEXT_CELL_HEIGHT);
    let end_y = start_y.saturating_add(TEXT_CELL_HEIGHT);

    if framebuffer.len() % stride != 0 || end_y > height {
        return false;
    }

    let mut visible_pixels = 0;
    for y in start_y..end_y {
        let row_start = y * stride;
        let row_end = row_start + stride;
        for pixel in framebuffer[row_start..row_end].chunks_exact(RGB_BYTES_PER_PIXEL) {
            if pixel.iter().any(|channel| *channel > CHANNEL_THRESHOLD) {
                visible_pixels += 1;
                if visible_pixels >= MIN_VISIBLE_PIXELS {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{text_row_has_visible_pixels, FRAME_WIDTH};

    #[test]
    fn prompt_row_detector_accepts_visible_rgb24_text() {
        let mut frame = vec![0; FRAME_WIDTH * 192 * 3];
        let row_start = 3 * 8 * FRAME_WIDTH * 3;
        for pixel in 0..12 {
            let offset = row_start + pixel * 3;
            frame[offset..offset + 3].copy_from_slice(&[220, 220, 220]);
        }

        assert!(text_row_has_visible_pixels(&frame, 3));
    }

    #[test]
    fn prompt_row_detector_rejects_blank_and_out_of_range_rows() {
        let frame = vec![0; FRAME_WIDTH * 192 * 3];
        assert!(!text_row_has_visible_pixels(&frame, 3));
        assert!(!text_row_has_visible_pixels(&frame, 24));
    }

    #[test]
    fn prompt_row_detector_rejects_incomplete_rgb24_frames() {
        let frame = vec![255; FRAME_WIDTH * 192 * 3 - 1];
        assert!(!text_row_has_visible_pixels(&frame, 3));
    }
}
