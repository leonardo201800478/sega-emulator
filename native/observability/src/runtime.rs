use crate::DiagnosticsConsent;
use crate::writer::{FanoutWriter, SizeRotatingWriter, SwitchableMakeWriter};
use serde::Serialize;
use std::any::Any;
use std::collections::VecDeque;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::panic::Location;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock, PoisonError, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing_appender::non_blocking::{ErrorCounter, NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;

static LOGGING: OnceLock<LoggingHandle> = OnceLock::new();
static PANIC_HOOK: Once = Once::new();
static DIAGNOSTICS_ENABLED: AtomicBool = AtomicBool::new(false);

const MAX_CRASH_RECORD_BYTES: u64 = 1_024;
const MAX_CRASH_SESSION_ID_BYTES: usize = 64;
const MAX_CRASH_SOURCE_BASENAME_BYTES: usize = 128;

pub type InitError = Box<dyn Error + Send + Sync>;

type FilterHandle = reload::Handle<EnvFilter, Registry>;
type WriterSlot = Arc<RwLock<Option<NonBlocking>>>;

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
    writer_slot: WriterSlot,
    filter: FilterHandle,
    error_counter: Mutex<ErrorCounter>,
    recent_events: Arc<Mutex<VecDeque<String>>>,
    writer_gate: Arc<RwLock<bool>>,
    config: Mutex<LogConfig>,
    session: SessionMetadata,
}

impl LoggingHandle {
    /// Enable collection after the host receives an explicit user-consent action.
    ///
    /// A consent capability is required for every activation, including
    /// reactivation after revocation. On failure, collection remains disabled.
    pub fn enable(&self, _consent: &DiagnosticsConsent) -> Result<(), String> {
        DIAGNOSTICS_ENABLED.store(false, Ordering::Release);
        set_writer_gate(&self.writer_gate, &self.recent_events, false);
        let config = self
            .config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let filter = EnvFilter::try_new(&config.filter).map_err(|error| error.to_string())?;

        {
            let mut writer_slot = self
                .writer_slot
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            let mut guard_slot = self.guard.lock().unwrap_or_else(PoisonError::into_inner);
            if guard_slot.is_none() {
                let (writer, guard, error_counter) = create_non_blocking_writer(
                    &config,
                    Arc::clone(&self.recent_events),
                    Arc::clone(&self.writer_gate),
                )
                .map_err(|error| error.to_string())?;
                *writer_slot = Some(writer);
                *guard_slot = Some(guard);
                *self
                    .error_counter
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = error_counter;
            }
        }

        self.filter
            .reload(filter)
            .map_err(|error| error.to_string())?;
        set_writer_gate(&self.writer_gate, &self.recent_events, true);
        DIAGNOSTICS_ENABLED.store(true, Ordering::Release);
        Ok(())
    }

    /// Revoke collection, discard queued writes, and clear this session's recent events.
    ///
    /// The writer gate waits for any write already in progress, then rejects
    /// records still queued by the non-blocking appender. The ring buffer is
    /// cleared before this method returns, so a later re-consent starts clean.
    pub fn disable(&self) -> Result<(), String> {
        DIAGNOSTICS_ENABLED.store(false, Ordering::Release);
        set_writer_gate(&self.writer_gate, &self.recent_events, false);
        let filter_result = EnvFilter::try_new("off")
            .map_err(|error| error.to_string())
            .and_then(|filter| {
                self.filter
                    .reload(filter)
                    .map_err(|error| error.to_string())
            });

        // Detach the sender only after in-flight event writers release the slot.
        // Dropping WorkerGuard then drains the old queue while the sink gate is closed.
        {
            let mut writer_slot = self
                .writer_slot
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            drop(writer_slot.take());
        }
        let guard = self
            .guard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(guard);

        filter_result
    }

    /// Return whether diagnostic collection is currently enabled.
    pub fn is_enabled(&self) -> bool {
        DIAGNOSTICS_ENABLED.load(Ordering::Acquire)
    }

    pub fn set_filter(&self, directives: &str) -> Result<(), String> {
        let filter = EnvFilter::try_new(directives).map_err(|error| error.to_string())?;
        if self.is_enabled() {
            self.filter
                .reload(filter)
                .map_err(|error| error.to_string())?;
        }
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
        self.error_counter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .dropped_lines()
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
        ensure_diagnostics_enabled(self.is_enabled())?;
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
            recent_events: Vec<serde_json::Value>,
        }

        let config = self
            .config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let recent_events = self
            .recent_events()
            .into_iter()
            .map(|line| serde_json::from_str(&line).unwrap_or(serde_json::Value::String(line)))
            .collect();
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

    /// Detach and drain the event writer; the in-memory ring remains available for the final snapshot.
    pub fn shutdown(&self) {
        {
            let mut writer_slot = self
                .writer_slot
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            drop(writer_slot.take());
        }
        let guard = self
            .guard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(guard);
    }
}

fn initial_filter_directives() -> &'static str {
    "off"
}

/// Serialize writer activation/revocation with in-flight writes and clear the
/// recent-event ring so records never cross a consent boundary.
fn set_writer_gate(
    writer_gate: &RwLock<bool>,
    recent_events: &Mutex<VecDeque<String>>,
    enabled: bool,
) {
    let mut writer_enabled = writer_gate.write().unwrap_or_else(PoisonError::into_inner);
    recent_events
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    *writer_enabled = enabled;
}

fn create_non_blocking_writer(
    config: &LogConfig,
    recent_events: Arc<Mutex<VecDeque<String>>>,
    writer_gate: Arc<RwLock<bool>>,
) -> Result<(NonBlocking, WorkerGuard, ErrorCounter), InitError> {
    let file_writer = SizeRotatingWriter::new(
        config.log_dir.join("sega-emulator.log"),
        config.max_file_size_bytes,
        config.retained_files,
    )?;
    let writer = FanoutWriter::new(
        file_writer,
        recent_events,
        config.recent_event_capacity,
        writer_gate,
    );
    let (non_blocking, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(config.queue_capacity.max(1))
        .lossy(true)
        .finish(writer);
    let error_counter = non_blocking.error_counter();
    Ok((non_blocking, guard, error_counter))
}

fn ensure_diagnostics_enabled(enabled: bool) -> Result<(), std::io::Error> {
    if enabled {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "diagnostics snapshot requires active user consent",
        ))
    }
}

pub fn current() -> Option<&'static LoggingHandle> {
    LOGGING.get()
}

/// Initialize diagnostics infrastructure only after an explicit user opt-in.
///
/// This performs filesystem, subscriber, writer, and panic-hook setup. Callers
/// must obtain the capability directly from the affirmative UI action; never
/// call this during application startup or preference restoration.
pub fn init_after_explicit_consent(
    _consent: &DiagnosticsConsent,
    config: LogConfig,
    session: SessionMetadata,
) -> Result<&'static LoggingHandle, InitError> {
    if let Some(logging) = LOGGING.get() {
        return Ok(logging);
    }

    // Validate the requested filter before creating directories or files.
    let _validated_filter = EnvFilter::try_new(&config.filter)?;
    fs::create_dir_all(&config.log_dir)?;
    let recent_events = Arc::new(Mutex::new(VecDeque::with_capacity(
        config.recent_event_capacity.max(1),
    )));
    let writer_gate = Arc::new(RwLock::new(false));
    let (non_blocking, guard, error_counter) = create_non_blocking_writer(
        &config,
        Arc::clone(&recent_events),
        Arc::clone(&writer_gate),
    )?;
    let writer_slot = Arc::new(RwLock::new(Some(non_blocking)));

    // Install the subscriber inert; enable() activates it after consent.
    let filter = EnvFilter::try_new(initial_filter_directives())?;
    let (filter_layer, filter_handle) = reload::Layer::new(filter);
    let formatting_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_span_list(true)
        .with_ansi(false)
        .with_writer(SwitchableMakeWriter::new(Arc::clone(&writer_slot)));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(formatting_layer)
        .try_init()?;

    let crash_log = config.log_dir.join("sega-emulator-crash.log");
    // Rotation is performed during consented initialization, never in the panic hook.
    // A failure here is non-fatal: the hook still delegates to Rust's previous hook.
    let _ = prepare_crash_log(
        &crash_log,
        config.max_file_size_bytes,
        config.retained_files,
    );
    install_panic_hook(
        crash_log,
        config.max_file_size_bytes.max(1),
        session.session_id.clone(),
    );

    let logging = LoggingHandle {
        guard: Mutex::new(Some(guard)),
        writer_slot,
        filter: filter_handle,
        error_counter: Mutex::new(error_counter),
        recent_events,
        writer_gate,
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

/// Return a bounded, sanitized source basename without directory components.
fn source_basename(file: &str) -> String {
    let basename = file.rsplit(['/', '\\']).next().unwrap_or(file);
    sanitize_field(basename, MAX_CRASH_SOURCE_BASENAME_BYTES)
}

/// Sanitize a field to printable ASCII and cap its length.
fn sanitize_field(value: &str, max_bytes: usize) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        .take(max_bytes)
        .collect()
}

/// Format the only fields permitted in a private crash record.
///
/// The payload content and full source path are intentionally not accepted.
fn format_crash_record(
    timestamp_unix_ms: u128,
    session_id: &str,
    location: Option<(&str, u32, u32)>,
    payload_is_text: bool,
) -> String {
    let session_id = sanitize_field(session_id, MAX_CRASH_SESSION_ID_BYTES);
    let (source_file, source_line, source_column) = match location {
        Some((file, line, column)) => (source_basename(file), line.to_string(), column.to_string()),
        None => (
            "unknown".to_owned(),
            "unknown".to_owned(),
            "unknown".to_owned(),
        ),
    };

    format!(
        "timestamp_unix_ms={timestamp_unix_ms}\nsession_id={session_id}\nsource_file={source_file}\nsource_line={source_line}\nsource_column={source_column}\npayload_is_text={payload_is_text}\n\n"
    )
}

/// Rotate an existing crash log before installing the hook.
///
/// This function is called only during consented initialization. The panic hook
/// itself never renames, removes, or rotates files.
fn prepare_crash_log(path: &Path, max_bytes: u64, retained_files: usize) -> std::io::Result<()> {
    prune_crash_backups(path, retained_files)?;
    if retained_files == 0 {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.len().saturating_add(MAX_CRASH_RECORD_BYTES) <= max_bytes.max(1) {
        return Ok(());
    }

    for index in (1..=retained_files).rev() {
        let source = if index == 1 {
            path.to_path_buf()
        } else {
            crash_backup_path(path, index - 1)
        };
        let destination = crash_backup_path(path, index);
        if source.exists() {
            if destination.exists() {
                fs::remove_file(&destination)?;
            }
            fs::rename(source, destination)?;
        }
    }
    Ok(())
}

/// Remove numbered backups outside the configured retention window.
fn prune_crash_backups(path: &Path, retained_files: usize) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let Some(file_name) = path.file_name() else {
        return Ok(());
    };
    let prefix = format!("{}.", file_name.to_string_lossy());

    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(index) = name
            .strip_prefix(&prefix)
            .and_then(|suffix| suffix.parse::<usize>().ok())
            && index > retained_files
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Construct a numbered backup path without changing the original filename.
fn crash_backup_path(path: &Path, index: usize) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{index}"));
    PathBuf::from(name)
}

/// Append a bounded record only when it fits; never rotate in the panic path.
fn append_crash_record(path: &Path, max_bytes: u64, record: &str) -> std::io::Result<()> {
    if record.len() as u64 > MAX_CRASH_RECORD_BYTES {
        return Ok(());
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let current_size = file.metadata()?.len();
    if current_size.saturating_add(record.len() as u64) > max_bytes.max(1) {
        return Ok(());
    }
    file.write_all(record.as_bytes())?;
    file.flush()
}

/// Install the private crash hook after consented diagnostics initialization.
fn install_panic_hook(crash_log: PathBuf, max_bytes: u64, session_id: String) {
    PANIC_HOOK.call_once(move || {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            if DIAGNOSTICS_ENABLED.load(Ordering::Acquire) {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let location = panic_info.location().map(|location: &Location<'_>| {
                    (location.file(), location.line(), location.column())
                });
                let payload_is_text = panic_payload_is_text(panic_info.payload());
                let record = format_crash_record(timestamp, &session_id, location, payload_is_text);
                // I/O errors are intentionally ignored so the previous hook is always called.
                let _ = append_crash_record(&crash_log, max_bytes, &record);
            }
            previous(panic_info);
        }));
    });
}

/// Detect only the payload type; never format or persist its contents.
fn panic_payload_is_text(payload: &(dyn Any + Send)) -> bool {
    payload.is::<String>() || payload.is::<&'static str>()
}

#[cfg(test)]
mod tests {
    use super::{
        DIAGNOSTICS_ENABLED, LogConfig, Ordering, SessionMetadata, append_crash_record,
        ensure_diagnostics_enabled, format_crash_record, initial_filter_directives,
        prepare_crash_log, set_writer_gate,
    };
    use std::collections::VecDeque;
    use std::fs;
    use std::io::Write;
    use std::process::Command;
    use std::sync::{Mutex, RwLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sega-observability-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn diagnostics_collection_starts_disabled() {
        assert!(!DIAGNOSTICS_ENABLED.load(Ordering::Acquire));
    }

    #[test]
    fn subscriber_initial_filter_is_off() {
        assert_eq!(initial_filter_directives(), "off");
    }

    #[test]
    fn revocation_and_reconsent_clear_recent_events() {
        let writer_gate = RwLock::new(true);
        let recent_events = Mutex::new(VecDeque::from(["stale-event".to_owned()]));

        set_writer_gate(&writer_gate, &recent_events, false);
        assert!(!*writer_gate.read().unwrap());
        assert!(recent_events.lock().unwrap().is_empty());

        recent_events
            .lock()
            .unwrap()
            .push_back("another-stale-event".to_owned());
        set_writer_gate(&writer_gate, &recent_events, true);
        assert!(*writer_gate.read().unwrap());
        assert!(recent_events.lock().unwrap().is_empty());
    }

    #[test]
    fn diagnostics_snapshot_requires_active_consent() {
        assert_eq!(
            ensure_diagnostics_enabled(false).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(ensure_diagnostics_enabled(true).is_ok());
    }

    #[test]
    fn crash_formatter_excludes_payload_and_absolute_source_paths() {
        let record = format_crash_record(
            1_800_000_000_000,
            "session/ID",
            Some((r"C:\Users\private\ROM_SENTINEL\source\cpu.rs", 42, 7)),
            true,
        );

        assert!(record.contains("session_id=sessionID"));
        assert!(record.contains("source_file=cpu.rs"));
        assert!(record.contains("source_line=42"));
        assert!(record.contains("source_column=7"));
        assert!(record.contains("payload_is_text=true"));
        assert!(!record.contains("Users"));
        assert!(!record.contains("private"));
        assert!(!record.contains("ROM_SENTINEL"));
        assert!(!record.contains("SECRET"));
        assert!(record.len() <= 1_024);
    }

    #[test]
    fn crash_record_append_obeys_file_size_limit() {
        let directory = temporary_directory("crash-size");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("crash.log");
        fs::write(&path, b"123456").unwrap();

        append_crash_record(&path, 8, "123\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"123456");

        append_crash_record(&path, 16, "123\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"123456123\n");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn crash_log_rotation_is_bounded_and_runs_before_hook() {
        let directory = temporary_directory("crash-retention");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("crash.log");
        fs::write(&path, b"active-log-near-limit").unwrap();
        fs::write(directory.join("crash.log.1"), b"older-1").unwrap();
        fs::write(directory.join("crash.log.2"), b"older-2").unwrap();
        fs::write(directory.join("crash.log.3"), b"older-3").unwrap();

        prepare_crash_log(&path, 8, 2).unwrap();

        assert!(!path.exists());
        assert_eq!(
            fs::read(directory.join("crash.log.1")).unwrap(),
            b"active-log-near-limit"
        );
        assert_eq!(fs::read(directory.join("crash.log.2")).unwrap(), b"older-1");
        assert!(!directory.join("crash.log.3").exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn crash_log_rotation_can_disable_backups() {
        let directory = temporary_directory("crash-no-retention");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("crash.log");
        fs::write(&path, b"old").unwrap();
        fs::write(directory.join("crash.log.1"), b"older").unwrap();

        prepare_crash_log(&path, 1, 0).unwrap();

        assert!(!path.exists());
        assert!(!directory.join("crash.log.1").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn crash_log_prunes_stale_backups_even_without_an_active_file() {
        let directory = temporary_directory("crash-prune");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("crash.log");
        fs::write(directory.join("crash.log.1"), b"newest").unwrap();
        fs::write(directory.join("crash.log.2"), b"older").unwrap();
        fs::write(directory.join("crash.log.3"), b"oldest").unwrap();

        prepare_crash_log(&path, 1_024, 1).unwrap();

        assert_eq!(fs::read(directory.join("crash.log.1")).unwrap(), b"newest");
        assert!(!directory.join("crash.log.2").exists());
        assert!(!directory.join("crash.log.3").exists());
        fs::remove_dir_all(directory).unwrap();
    }

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

    /// Child entry point used to validate hook delegation in an isolated process.
    #[test]
    fn panic_hook_subprocess_child() {
        let Ok(mode) = std::env::var("SEGA_OBSERVABILITY_PANIC_CHILD") else {
            return;
        };

        let directory =
            std::path::PathBuf::from(std::env::var_os("SEGA_OBSERVABILITY_PANIC_DIR").unwrap());
        let marker = directory.join("previous-hook-called");
        let crash_log = if mode == "io-failure" {
            directory.join("not-a-directory").join("crash.log")
        } else {
            directory.join("crash.log")
        };

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&marker)
                .unwrap();
            writeln!(file, "previous").unwrap();
            previous(panic_info);
        }));

        if mode != "no-opt-in" {
            super::install_panic_hook(crash_log, 4_096, "child-session".to_owned());
        }
        DIAGNOSTICS_ENABLED.store(mode == "opt-in" || mode == "io-failure", Ordering::Release);

        panic!("PAYLOAD_SENTINEL /private/ROM_SENTINEL");
    }

    #[test]
    fn panic_hook_delegates_once_and_respects_consent_matrix_in_subprocesses() {
        for mode in ["no-opt-in", "opt-in", "revoked", "io-failure"] {
            let directory = temporary_directory(mode);
            fs::create_dir_all(&directory).unwrap();
            if mode == "io-failure" {
                fs::write(directory.join("not-a-directory"), b"block").unwrap();
            }

            let output = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("runtime::tests::panic_hook_subprocess_child")
                .arg("--nocapture")
                .env("SEGA_OBSERVABILITY_PANIC_CHILD", mode)
                .env("SEGA_OBSERVABILITY_PANIC_DIR", &directory)
                .output()
                .unwrap();

            assert!(
                !output.status.success(),
                "panic child unexpectedly succeeded for mode {mode}"
            );
            assert_eq!(
                fs::read_to_string(directory.join("previous-hook-called")).unwrap(),
                "previous\n",
                "previous hook was not called exactly once for mode {mode}"
            );

            let crash_log = directory.join("crash.log");
            if mode == "opt-in" {
                let contents = fs::read_to_string(&crash_log).unwrap();
                assert!(contents.contains("session_id=child-session"));
                assert!(contents.contains("payload_is_text=true"));
                assert!(!contents.contains("PAYLOAD_SENTINEL"));
                assert!(!contents.contains("ROM_SENTINEL"));
            } else {
                assert!(
                    !crash_log.exists(),
                    "private crash log unexpectedly exists for mode {mode}"
                );
            }

            fs::remove_dir_all(directory).unwrap();
        }
    }
}
