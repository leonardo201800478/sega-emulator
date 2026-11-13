//! Deterministic SMSmemtest input and result driver.
//!
//! The decoder reads the SMS Mode 4 name table through the VDP's existing
//! read-only VRAM accessor. SMSlib's text renderer maps ASCII characters to
//! font tile indices 0..95 (white), 96..191 (green), and 192..287 (red).
//! This avoids framebuffer OCR and does not mutate VDP or Scheduler state.

use sega_sms_core::{ControllerButtons, SmsTemporalMachine};

const SMS_PNT_BASE: usize = 0x3800;
const SMS_PNT_COLUMNS: usize = 32;
const SMS_PNT_ROWS: usize = 28;
const SMS_PNT_ROW_BYTES: usize = SMS_PNT_COLUMNS * 2;
const FONT_TILE_COUNT: u16 = 96;
const GREEN_TILE_OFFSET: u16 = 96;
const RED_TILE_OFFSET: u16 = 192;
const REQUIRED_STABLE_FRAMES: u8 = 2;

pub const RAM_RESULT_CASES: [&str; 7] = [
    "ram_all_zero",
    "ram_all_one",
    "ram_pattern_55",
    "ram_pattern_aa",
    "ram_variable_1",
    "ram_variable_19",
    "ram_variable_71",
];

const RAM_RESULT_LABELS: [&str; 7] = [
    "-all bits to zero...",
    "-all bits to one ...",
    "-all bytes to $55...",
    "-all bytes to $AA...",
    "-different values...",
    "-different values...",
    "-different values...",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsmemtestCaseResult {
    Pass,
    Fail,
    PassAllReportedByRom,
    NotObserved,
    Ambiguous,
}

impl SmsmemtestCaseResult {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::PassAllReportedByRom => "PASS_ALL_REPORTED_BY_ROM",
            Self::NotObserved => "NOT_OBSERVED",
            Self::Ambiguous => "AMBIGUOUS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmsmemtestTextColor {
    White,
    Green,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedTextRow {
    text: String,
    colors: Vec<SmsmemtestTextColor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsmemtestDecodedCase {
    pub case_id: &'static str,
    pub result: SmsmemtestCaseResult,
    pub observed_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    AwaitInitialPrompt,
    AwaitRamCompletePrompt,
    AwaitVramResult,
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

    /// Observe the SMSlib name table after a completed frame and emit input
    /// transitions only at frame boundaries.
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

        let vram = machine.vdp().vram();
        let marker_observed = match stage {
            Stage::AwaitInitialPrompt => decode_smsmemtest_text_row(vram, 3)
                .is_some_and(|row| row_is_white(&row, "Press any button to start")),
            Stage::AwaitRamCompletePrompt => {
                let first = decode_smsmemtest_text_row(vram, 14);
                let second = decode_smsmemtest_text_row(vram, 15);
                first.is_some_and(|row| row_is_white(&row, "Press any controller button"))
                    && second.is_some_and(|row| row_is_white(&row, "to start VIDEO RAM tests"))
            }
            Stage::AwaitVramResult => classify_vram_result_row(vram).is_some(),
            Stage::Complete => return Ok(events),
        };

        if marker_observed {
            self.stable_frames = self.stable_frames.saturating_add(1);
        } else {
            self.stable_frames = 0;
        }

        if self.stable_frames < REQUIRED_STABLE_FRAMES {
            return Ok(events);
        }

        self.stable_frames = 0;
        match stage {
            Stage::AwaitInitialPrompt => {
                self.press_button(machine)?;
                self.stage = Some(Stage::AwaitRamCompletePrompt);
                events.push("button_pressed");
            }
            Stage::AwaitRamCompletePrompt => {
                events.push("ram_results_ready");
                self.press_button(machine)?;
                self.stage = Some(Stage::AwaitVramResult);
                events.push("button_pressed");
            }
            Stage::AwaitVramResult => {
                self.stage = Some(Stage::Complete);
                events.push("vram_result_text_observed");
            }
            Stage::Complete => {}
        }

        Ok(events)
    }

    fn press_button(&mut self, machine: &mut SmsTemporalMachine) -> Result<(), String> {
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
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.stage == Some(Stage::Complete)
    }
}

/// Decode one 32-column SMSlib text row directly from the Mode 4 name table.
///
/// SMSlib defines PNT write address 0x7800; after removing the VDP write flag,
/// the backing VRAM offset is 0x3800. Each entry is a little-endian 16-bit
/// tile/attribute word. The low nine bits identify the font tile.
fn decode_smsmemtest_text_row(vram: &[u8], row: usize) -> Option<DecodedTextRow> {
    if row >= SMS_PNT_ROWS {
        return None;
    }

    let row_start = SMS_PNT_BASE.checked_add(row.checked_mul(SMS_PNT_ROW_BYTES)?)?;
    let row_end = row_start.checked_add(SMS_PNT_ROW_BYTES)?;
    if row_end > vram.len() {
        return None;
    }

    let mut text = String::with_capacity(SMS_PNT_COLUMNS);
    let mut colors = Vec::with_capacity(SMS_PNT_COLUMNS);
    for column in 0..SMS_PNT_COLUMNS {
        let offset = row_start + column * 2;
        let entry = u16::from_le_bytes([vram[offset], vram[offset + 1]]);
        let tile = entry & 0x01FF;
        let (character, color) = decode_font_tile(tile)?;
        text.push(character);
        colors.push(color);
    }

    let leading_spaces = text.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed_end = text.trim_end_matches(' ').len();
    let text = text[leading_spaces..trimmed_end].to_owned();
    let colors = colors[leading_spaces..trimmed_end].to_vec();

    Some(DecodedTextRow { text, colors })
}

fn decode_font_tile(tile: u16) -> Option<(char, SmsmemtestTextColor)> {
    let (offset, color) = match tile {
        0..FONT_TILE_COUNT => (0, SmsmemtestTextColor::White),
        value if (GREEN_TILE_OFFSET..GREEN_TILE_OFFSET + FONT_TILE_COUNT).contains(&value) => {
            (GREEN_TILE_OFFSET, SmsmemtestTextColor::Green)
        }
        value if (RED_TILE_OFFSET..RED_TILE_OFFSET + FONT_TILE_COUNT).contains(&value) => {
            (RED_TILE_OFFSET, SmsmemtestTextColor::Red)
        }
        _ => return None,
    };

    let ascii = tile.checked_sub(offset)?.checked_add(32)?;
    char::from_u32(u32::from(ascii)).map(|character| (character, color))
}

fn row_is_white(row: &DecodedTextRow, expected_text: &str) -> bool {
    row.text == expected_text
        && row
            .colors
            .iter()
            .all(|color| *color == SmsmemtestTextColor::White)
}

fn classify_result_text(
    row: &DecodedTextRow,
    expected_label: &str,
) -> SmsmemtestCaseResult {
    let pass_text = format!("{expected_label}OK!");
    if row.text == pass_text {
        let label_is_white = row.colors[..expected_label.len()]
            .iter()
            .all(|color| *color == SmsmemtestTextColor::White);
        let result_is_green = row.colors[expected_label.len()..]
            .iter()
            .all(|color| *color == SmsmemtestTextColor::Green);
        return if label_is_white && result_is_green {
            SmsmemtestCaseResult::Pass
        } else {
            SmsmemtestCaseResult::Ambiguous
        };
    }

    let fail_text = format!("{expected_label}FAILED!");
    if row.text == fail_text {
        let label_is_white = row.colors[..expected_label.len()]
            .iter()
            .all(|color| *color == SmsmemtestTextColor::White);
        let result_is_red = row.colors[expected_label.len()..]
            .iter()
            .all(|color| *color == SmsmemtestTextColor::Red);
        return if label_is_white && result_is_red {
            SmsmemtestCaseResult::Fail
        } else {
            SmsmemtestCaseResult::Ambiguous
        };
    }

    if row.text.is_empty() {
        SmsmemtestCaseResult::NotObserved
    } else {
        SmsmemtestCaseResult::Ambiguous
    }
}

/// Decode the seven independent RAM result rows (text rows 4 through 10).
pub fn classify_ram_result_rows(vram: &[u8]) -> [SmsmemtestDecodedCase; 7] {
    std::array::from_fn(|index| {
        let decoded = decode_smsmemtest_text_row(vram, index + 4);
        let (result, observed_text) = match decoded {
            Some(row) => (
                classify_result_text(&row, RAM_RESULT_LABELS[index]),
                row.text,
            ),
            None => (SmsmemtestCaseResult::NotObserved, String::new()),
        };

        SmsmemtestDecodedCase {
            case_id: RAM_RESULT_CASES[index],
            result,
            observed_text,
        }
    })
}

/// Decode the VRAM result row, including the exact failed pattern when present.
pub fn classify_vram_result_row(vram: &[u8]) -> Option<SmsmemtestDecodedCase> {
    let heading = decode_smsmemtest_text_row(vram, 3)?;
    if !row_is_white(&heading, "Testing system VIDEO RAM:") {
        return None;
    }

    let row = decode_smsmemtest_text_row(vram, 4)?;
    let all_tests_label = "-all tests       ...";
    let all_tests_result = classify_result_text(&row, all_tests_label);
    if all_tests_result == SmsmemtestCaseResult::Pass {
        return Some(SmsmemtestDecodedCase {
            case_id: "vram_all_tests",
            result: SmsmemtestCaseResult::PassAllReportedByRom,
            observed_text: row.text,
        });
    }

    const VRAM_FAILURE_CASES: [(&str, &str); 5] = [
        ("vram_all_zero", "-all bits to zero..."),
        ("vram_all_one", "-all bits to one ..."),
        ("vram_pattern_55", "-all bytes to $55..."),
        ("vram_pattern_aa", "-all bytes to $AA..."),
        ("vram_variable", "-different values..."),
    ];

    for (case_id, label) in VRAM_FAILURE_CASES {
        if classify_result_text(&row, label) == SmsmemtestCaseResult::Fail {
            return Some(SmsmemtestDecodedCase {
                case_id,
                result: SmsmemtestCaseResult::Fail,
                observed_text: row.text,
            });
        }
    }

    if row.text.is_empty() {
        None
    } else {
        Some(SmsmemtestDecodedCase {
            case_id: "vram_unknown_result",
            result: SmsmemtestCaseResult::Ambiguous,
            observed_text: row.text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GREEN_TILE_OFFSET, RED_TILE_OFFSET, RAM_RESULT_CASES, SMS_PNT_BASE, SMS_PNT_COLUMNS,
        SMS_PNT_ROW_BYTES, SmsmemtestCaseResult, classify_ram_result_rows,
        classify_vram_result_row, decode_smsmemtest_text_row,
    };

    #[derive(Clone, Copy)]
    enum TestColor {
        White,
        Green,
        Red,
    }

    fn write_text_row(vram: &mut [u8], row: usize, segments: &[(&str, TestColor)]) {
        let mut column = 1;
        for (text, color) in segments {
            let tile_offset = match color {
                TestColor::White => 0,
                TestColor::Green => GREEN_TILE_OFFSET,
                TestColor::Red => RED_TILE_OFFSET,
            };
            for character in text.chars() {
                let tile = (character as u16 - 32) + tile_offset;
                let offset = SMS_PNT_BASE + row * SMS_PNT_ROW_BYTES + column * 2;
                vram[offset..offset + 2].copy_from_slice(&tile.to_le_bytes());
                column += 1;
            }
        }

        while column < SMS_PNT_COLUMNS {
            let offset = SMS_PNT_BASE + row * SMS_PNT_ROW_BYTES + column * 2;
            vram[offset..offset + 2].copy_from_slice(&0_u16.to_le_bytes());
            column += 1;
        }
    }

    #[test]
    fn name_table_decoder_reads_white_prompt_text() {
        let mut vram = vec![0; 0x4000];
        write_text_row(
            &mut vram,
            3,
            &[("Press any button to start", TestColor::White)],
        );

        let row = decode_smsmemtest_text_row(&vram, 3).expect("valid SMS name table row");
        assert_eq!(row.text, "Press any button to start");
    }

    #[test]
    fn name_table_decoder_rejects_out_of_range_rows_and_tiles() {
        let mut vram = vec![0; 0x4000];
        assert!(decode_smsmemtest_text_row(&vram, 28).is_none());

        let offset = SMS_PNT_BASE + 3 * SMS_PNT_ROW_BYTES;
        vram[offset..offset + 2].copy_from_slice(&400_u16.to_le_bytes());
        assert!(decode_smsmemtest_text_row(&vram, 3).is_none());
    }

    #[test]
    fn ram_result_rows_decode_individual_pass_and_fail_text() {
        let mut vram = vec![0; 0x4000];
        write_text_row(
            &mut vram,
            4,
            &[
                ("-all bits to zero...", TestColor::White),
                ("OK!", TestColor::Green),
            ],
        );
        write_text_row(
            &mut vram,
            5,
            &[
                ("-all bits to one ...", TestColor::White),
                ("FAILED!", TestColor::Red),
            ],
        );

        let results = classify_ram_result_rows(&vram);
        assert_eq!(results[0].case_id, RAM_RESULT_CASES[0]);
        assert_eq!(results[0].result, SmsmemtestCaseResult::Pass);
        assert_eq!(results[1].case_id, RAM_RESULT_CASES[1]);
        assert_eq!(results[1].result, SmsmemtestCaseResult::Fail);
        assert_eq!(results[2].result, SmsmemtestCaseResult::NotObserved);
    }

    #[test]
    fn vram_failure_decoder_identifies_the_exact_failed_pattern() {
        let mut vram = vec![0; 0x4000];
        write_text_row(
            &mut vram,
            3,
            &[("Testing system VIDEO RAM:", TestColor::White)],
        );
        write_text_row(
            &mut vram,
            4,
            &[
                ("-all bytes to $55...", TestColor::White),
                ("FAILED!", TestColor::Red),
            ],
        );

        let result = classify_vram_result_row(&vram).expect("VRAM failure result");
        assert_eq!(result.case_id, "vram_pattern_55");
        assert_eq!(result.result, SmsmemtestCaseResult::Fail);
    }

    #[test]
    fn vram_success_decoder_requires_rom_all_tests_label() {
        let mut vram = vec![0; 0x4000];
        write_text_row(
            &mut vram,
            3,
            &[("Testing system VIDEO RAM:", TestColor::White)],
        );
        write_text_row(
            &mut vram,
            4,
            &[
                ("-all tests       ...", TestColor::White),
                ("OK!", TestColor::Green),
            ],
        );

        let result = classify_vram_result_row(&vram).expect("VRAM aggregate result");
        assert_eq!(result.case_id, "vram_all_tests");
        assert_eq!(
            result.result,
            SmsmemtestCaseResult::PassAllReportedByRom
        );
    }
}
