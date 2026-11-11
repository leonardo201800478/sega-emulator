//! Standalone Windows SMS diagnostic debugger entry point.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

#[cfg(windows)]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Sega Emulator — SMS Debugger",
        options,
        Box::new(|_creation_context| Ok(Box::new(sega_sms_debugger::DebuggerApp::default()))),
    )
}

#[cfg(not(windows))]
fn main() {
    eprintln!("sega-sms-debugger targets Windows x86_64.");
}
