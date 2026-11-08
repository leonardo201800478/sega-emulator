//! Windows entry point for the native egui application shell.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

#[cfg(windows)]
fn main() -> eframe::Result {
    use eframe::egui;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Sega Emulator",
        options,
        Box::new(|_creation_context| Ok(Box::new(sega_app::SegaApp::default()))),
    )
}

#[cfg(not(windows))]
fn main() {
    eprintln!("sega-app M1 targets Windows x86_64; the non-Windows binary is a build stub.");
}
