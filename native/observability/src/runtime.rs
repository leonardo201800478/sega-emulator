use crate::writer::{FanoutWriter, SizeRotatingWriter};
use serde::Serialize;
use std::backtrace::Backtrace;
use std::collections::VecDeque;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing_appender::non_blocking::{ErrorCounter, NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

static LOGGING: OnceLock<LoggingHandle> = OnceLock::new();
static PANIC_HOOK: Once = Once::new();

pub type InitError = Box<dyn Error + Send + Sync>;

type FilterHandle = reload::Handle<EnvFilter, Registry>;

#[derive(Clone, Debug)]
pub struct LogConfig {
    pub log_dir: PathBuf,
    pub filter: String,
    pub max_file_size_bytes: u64,
    pub retained_files: usize,
    pub queue_capacity: usize,
    pub recent_event_capacity: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        let log_dir = std::env::var_os("SEGA_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_log_directory);
        let filter = std::env::var("SEGA_LOG_FILTER").unwrap_or_else(|_| "info".to_owned());

        Self {
            log_dir,
            filter,
            max_file_size_bytes: 10 * 1024 * 1024,
            retained_files: 5,
            queue_capacity: 16_384,
            recent_event_capacity: 256,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub app_version: String,
    pub git_sha: String,
    pub build_profile: String,
    pub operating_system: String,
    pub architecture: String,
    pub started_at_unix_ms: u128,
}

impl SessionMetadata {
    pub fn current(
        app_version: impl Into<String>,
        git_sha: impl Into<String>,
        build_profile: impl Into<String>,
    ) -> Self {
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let session_id = format!("{started_at_unix_ms:x}-{}", std::process::id());

        Self {
            session_id,
            app_version: app_version.into(),
            git_sha: git_sha.into(),
            build_profile: build_profile.into(),
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            started_at_unix_ms,
        }
    }
}

pub struct LoggingHandle {
    guard: Mutex<Option<WorkerGuard>>,
    filter: FilterHandle,
    error_counter: ErrorCounter,
    recent_events: Arc<Mutex<VecDeque<String>>>,
    config: Mutex<LogConfig>,
    session: SessionMetadata,
}

impl LoggingHandle {
    pub fn set_filter(&self, directives: &str) -> Result<(), String> {
        let filter = EnvFilter::try_new(directives).map_err(|error| error.to_string())?;
        self.filter
            .reload(filter)
            .map_err(|error| error.to_string())?;
        self.config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .filter = directives.to_owned();
        Ok(())
    }

    pub fn filter(&self) -> String {
        self.config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .filter
            .clone()
    }

    pub fn recent_events(&self) -> Vec<String> {
        self.recent_events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    pub fn dropped_lines(&self) -> usize {
        self.error_counter.dropped_lines()
    }

    pub fn session(&self) -> &SessionMetadata {
        &self.session
    }

    pub fn log_directory(&self) -> PathBuf {
        self.config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .log_dir
            .clone()
    }

    /// Write a privacy-conscious session snapshot without ROM contents or absolute paths.
    pub fn write_diagnostics_json(&self) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
        #[derive(Serialize)]
        struct DiagnosticConfig<'a> {
            filter: &'a str,
            max_file_size_bytes: u64,
            retained_files: usize,
            queue_capacity: usize,
            recent_event_capacity: usize,
        }

        #[derive(Serialize)]
        struct Snapshot<'a> {
            session: &'a SessionMetadata,
            logging: DiagnosticConfig<'a>,
            dropped_log_lines: usize,
            recent_events: Vec<String>,
        }

        let config = self
            .config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let recent_events = self.recent_events();
        let snapshot = Snapshot {
            session: &self.session,
            logging: DiagnosticConfig {
                filter: &config.filter,
                max_file_size_bytes: config.max_file_size_bytes,
                retained_files: config.retained_files,
                queue_capacity: config.queue_capacity,
                recent_event_capacity: config.recent_event_capacity,
            },
            dropped_log_lines: self.dropped_lines(),
            recent_events,
        };

        fs::create_dir_all(&config.log_dir)?;
        let destination = config.log_dir.join("sega-emulator-diagnostics.json");
        let temporary = config.log_dir.join("sega-emulator-diagnostics.json.tmp");
        let mut file = File::create(&temporary)?;
        serde_json::to_writer_pretty(&mut file, &snapshot)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        if destination.exists() {
            fs::remove_file(&destination)?;
        }
        fs::rename(&temporary, &destination)?;
        Ok(destination)
    }

    /// Drop the non-blocking worker guard after the final diagnostics snapshot is written.
    pub fn shutdown(&self) {
        let guard = self
            .guard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(guard);
    }
}

pub fn current() -> Option<&'static LoggingHandle> {
    LOGGING.get()
}

pub fn init(
    config: LogConfig,
    session: SessionMetadata,
) -> Result<&'static LoggingHandle, InitError> {
    if let Some(logging) = LOGGING.get() {
        return Ok(logging);
    }

    fs::create_dir_all(&config.log_dir)?;
    let recent_events = Arc::new(Mutex::new(VecDeque::with_capacity(
        config.recent_event_capacity.max(1),
    )));
    let file_writer = SizeRotatingWriter::new(
        config.log_dir.join("sega-emulator.log"),
        config.max_file_size_bytes,
        config.retained_files,
    )?;
    let writer = FanoutWriter::new(
        file_writer,
        recent_events.clone(),
        config.recent_event_capacity,
    );
    let (non_blocking, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(config.queue_capacity.max(1))
        .lossy(true)
        .finish(writer);
    let error_counter = non_blocking.error_counter();

    let filter = EnvFilter::try_new(&config.filter)?;
    let (filter_layer, filter_handle) = reload::Layer::new(filter);
    let formatting_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_span_list(true)
        .with_ansi(false)
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(formatting_layer)
        .try_init()?;

    install_panic_hook(config.log_dir.join("sega-emulator-crash.log"));

    let logging = LoggingHandle {
        guard: Mutex::new(Some(guard)),
        filter: filter_handle,
        error_counter,
        recent_events,
        config: Mutex::new(config),
        session,
    };
    if LOGGING.set(logging).is_err() {
        return Ok(LOGGING
            .get()
            .expect("concurrent logging initialization already installed a handle"));
    }

    Ok(LOGGING.get().expect("logging handle was initialized"))
}

fn default_log_directory() -> PathBuf {
    #[cfg(windows)]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Sega Emulator")
            .join("logs");
    }

    #[cfg(not(windows))]
    if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home).join("Sega Emulator").join("logs");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("Sega Emulator")
            .join("logs");
    }

    std::env::temp_dir().join("Sega Emulator").join("logs")
}

fn install_panic_hook(crash_log: PathBuf) {
    PANIC_HOOK.call_once(move || {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&crash_log) {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let _ = writeln!(file, "timestamp_unix_ms={timestamp}");
                let _ = writeln!(file, "panic={panic_info}");
                let _ = writeln!(file, "backtrace={}", Backtrace::force_capture());
                let _ = file.flush();
            }
            previous(panic_info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::{LogConfig, SessionMetadata};

    #[test]
    fn default_log_configuration_uses_bounded_async_queue_and_rotation() {
        let config = LogConfig::default();
        assert!(config.queue_capacity > 0);
        assert!(config.max_file_size_bytes > 0);
        assert!(config.retained_files > 0);
        assert!(config.recent_event_capacity > 0);
    }

    #[test]
    fn session_metadata_contains_build_and_platform_identity() {
        let session = SessionMetadata::current("0.1.0", "test-sha", "debug");
        assert_eq!(session.app_version, "0.1.0");
        assert_eq!(session.git_sha, "test-sha");
        assert_eq!(session.build_profile, "debug");
        assert!(!session.session_id.is_empty());
        assert!(!session.operating_system.is_empty());
        assert!(!session.architecture.is_empty());
    }
}
