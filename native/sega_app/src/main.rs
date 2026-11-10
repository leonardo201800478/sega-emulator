//! Windows entry point for the native egui application shell.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

#[cfg(windows)]
fn main() -> eframe::Result {
    let persistence_path = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .map(|directory| directory.join("Sega Emulator"));
    if let Some(directory) = persistence_path.as_ref() {
        std::fs::create_dir_all(directory).map_err(eframe::Error::AppCreation)?;
    }
    let options = eframe::NativeOptions {
        persistence_path,
        persist_window: true,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 720.0]),
        ..Default::default()
    };

    // Diagnostics are deliberately not initialized here. The user must
    // explicitly enable collection from Options > Diagnostics.
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
