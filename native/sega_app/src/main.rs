//! Windows entry point for the native egui application shell.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

#[cfg(windows)]
fn main() -> eframe::Result {
    use eframe::egui;
    use sega_observability::{init, LogConfig, SessionMetadata};

    let session = SessionMetadata::current(
        env!("CARGO_PKG_VERSION"),
        option_env!("SEGA_GIT_SHA").unwrap_or("unknown"),
        option_env!("SEGA_BUILD_PROFILE").unwrap_or("unknown"),
    );
    let logging = match init(LogConfig::default(), session) {
        Ok(logging) => {
            let session = logging.session();
            sega_observability::tracing::info!(
                target: "sega::app",
                event = "application_start",
                session_id = %session.session_id,
                version = %session.app_version,
                git_sha = %session.git_sha,
                build_profile = %session.build_profile,
                operating_system = %session.operating_system,
                architecture = %session.architecture,
            );
            Some(logging)
        }
        Err(error) => {
            eprintln!("Sega Emulator logging initialization failed: {error}");
            None
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 720.0])
            .with_maximized(true),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Sega Emulator",
        options,
        Box::new(|_creation_context| Ok(Box::new(sega_app::SegaApp::default()))),
    );

    if let Some(logging) = logging {
        sega_observability::tracing::info!(
            target: "sega::app",
            event = "application_shutdown",
            session_id = %logging.session().session_id,
        );
        logging.shutdown();
        if let Err(error) = logging.write_diagnostics_json() {
            eprintln!("Unable to write diagnostics snapshot: {error}");
        }
    }

    result
}

#[cfg(not(windows))]
fn main() {
    eprintln!("sega-app M1 targets Windows x86_64; the non-Windows binary is a build stub.");
}
