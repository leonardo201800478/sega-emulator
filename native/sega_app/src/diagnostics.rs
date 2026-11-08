//! Opt-in, bounded diagnostics capture for the native Windows UI.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

const QUEUE_CAPACITY: usize = 2_048;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const BACKUP_COUNT: usize = 3;

struct DiagnosticEvent {
    sequence: u64,
    timestamp_ms: u128,
    event: String,
    details: String,
    dropped_before: u64,
}

/// Owns a bounded, non-blocking producer and a dedicated JSONL writer thread.
#[derive(Default)]
pub(super) struct DiagnosticsCapture {
    sender: Option<SyncSender<DiagnosticEvent>>,
    writer: Option<JoinHandle<()>>,
    sequence: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    file_path: Option<PathBuf>,
    writer_error: Arc<Mutex<Option<String>>>,
}

impl DiagnosticsCapture {
    /// Start capture explicitly. Disk creation and writer startup happen only after opt-in.
    pub(super) fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        if enabled {
            if self.sender.is_some() {
                return Ok(());
            }
            if self.writer.as_ref().is_some_and(|writer| !writer.is_finished()) {
                return Err("A gravação anterior ainda está finalizando".to_owned());
            }
            if let Some(writer) = self.writer.take() {
                let _ = writer.join();
            }

            let directory = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .ok_or_else(|| "APPDATA não está definido".to_owned())?
                .join("Sega Emulator")
                .join("logs");
            fs::create_dir_all(&directory)
                .map_err(|error| format!("Não foi possível criar a pasta de logs: {error}"))?;
            let timestamp = now_millis();
            let path = directory.join(format!("debug-session-{timestamp}.jsonl"));
            let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
            let writer_path = path.clone();
            let writer_error = Arc::clone(&self.writer_error);
            if let Ok(mut error) = self.writer_error.lock() { *error = None; }
            self.writer = Some(thread::Builder::new()
                .name("sega-ui-diagnostics".to_owned())
                .spawn(move || {
                    write_events(&receiver, &writer_path, writer_error.as_ref())
                })
                .map_err(|error| format!("Não foi possível iniciar a thread de logs: {error}"))?);
            self.sender = Some(sender);
            self.file_path = Some(path);
            self.sequence.store(0, Ordering::Relaxed);
            self.dropped.store(0, Ordering::Relaxed);
            return Ok(());
        }

        // Dropping the sender closes the channel; the writer drains queued events and exits.
        self.sender = None;
        Ok(())
    }

    /// Queue one structured event without waiting for disk I/O or queue capacity.
    pub(super) fn record(&self, event: &str, details: impl Into<String>) {
        let Some(sender) = self.sender.as_ref() else {
            return;
        };
        let record = DiagnosticEvent {
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed) + 1,
            timestamp_ms: now_millis(),
            event: event.to_owned(),
            details: details.into(),
            dropped_before: self.dropped.load(Ordering::Relaxed),
        };
        if let Err(TrySendError::Full(_)) = sender.try_send(record) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.sender.is_some()
    }

    pub(super) fn file_path(&self) -> Option<&std::path::Path> {
        self.file_path.as_deref()
    }

    pub(super) fn dropped_events(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub(super) fn writer_error(&self) -> Option<String> {
        self.writer_error.lock().ok().and_then(|error| error.clone())
    }
}

impl Drop for DiagnosticsCapture {
    fn drop(&mut self) {
        self.sender = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn write_events(
    receiver: &mpsc::Receiver<DiagnosticEvent>,
    path: &std::path::Path,
    writer_error: &Mutex<Option<String>>,
) {
    let mut writer = match open_writer(&path) {
        Ok(writer) => writer,
        Err(error) => { set_writer_error(&writer_error, format!("Falha ao abrir arquivo de log: {error}")); return; }
    };
    let mut bytes_written = writer.get_ref().metadata().map_or(0, |metadata| metadata.len());

    while let Ok(event) = receiver.recv() {
        let line = format!(
            "{{\"sequence\":{},\"timestamp_unix_ms\":{},\"event\":\"{}\",\"details\":\"{}\",\"dropped_before\":{}}}\n",
            event.sequence,
            event.timestamp_ms,
            json_escape(&event.event),
            json_escape(&event.details),
            event.dropped_before
        );
        let line_bytes = u64::try_from(line.len()).unwrap_or(u64::MAX);
        if bytes_written.saturating_add(line_bytes) > MAX_FILE_BYTES {
            if let Err(error) = writer.flush().and_then(|()| rotate_files(&path)) {
                set_writer_error(&writer_error, format!("Falha ao girar arquivo de log: {error}"));
                return;
            }
            writer = match open_writer(&path) {
                Ok(writer) => writer,
                Err(error) => { set_writer_error(&writer_error, format!("Falha ao reabrir arquivo de log: {error}")); return; }
            };
            bytes_written = 0;
        }
        if let Err(error) = writer.write_all(line.as_bytes()) {
            set_writer_error(&writer_error, format!("Falha ao gravar evento de log: {error}"));
            return;
        }
        bytes_written = bytes_written.saturating_add(line_bytes);
    }
    if let Err(error) = writer.flush() {
        set_writer_error(&writer_error, format!("Falha ao finalizar arquivo de log: {error}"));
    }
}

fn set_writer_error(target: &Mutex<Option<String>>, message: String) {
    if let Ok(mut error) = target.lock() { *error = Some(message); }
}

fn open_writer(path: &std::path::Path) -> std::io::Result<BufWriter<File>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(BufWriter::new)
}

fn rotate_files(path: &std::path::Path) -> std::io::Result<()> {
    for index in (1..=BACKUP_COUNT).rev() {
        let source = if index == 1 {
            path.to_path_buf()
        } else {
            suffixed_path(path, index - 1)
        };
        let destination = suffixed_path(path, index);
        if source.exists() {
            if destination.exists() {
                fs::remove_file(&destination)?;
            }
            fs::rename(source, destination)?;
        }
    }
    Ok(())
}

fn suffixed_path(path: &std::path::Path, index: usize) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".{index}"));
    PathBuf::from(value)
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", u32::from(character));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::json_escape;

    #[test]
    fn json_escape_protects_quotes_backslashes_and_control_characters() {
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }
}
