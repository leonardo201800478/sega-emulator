use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

pub(super) struct FanoutWriter {
    file: SizeRotatingWriter,
    recent: Arc<Mutex<VecDeque<String>>>,
    recent_capacity: usize,
    pending: Vec<u8>,
}

impl FanoutWriter {
    pub(super) fn new(
        file: SizeRotatingWriter,
        recent: Arc<Mutex<VecDeque<String>>>,
        recent_capacity: usize,
    ) -> Self {
        Self {
            file,
            recent,
            recent_capacity: recent_capacity.max(1),
            pending: Vec::new(),
        }
    }

    fn publish_complete_lines(&mut self) {
        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=newline).collect();
            self.publish_line(&line);
        }

        if self.pending.len() > 64 * 1024 {
            self.publish_pending();
        }
    }

    fn publish_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let line = std::mem::take(&mut self.pending);
        self.publish_line(&line);
    }

    fn publish_line(&self, bytes: &[u8]) {
        let line = String::from_utf8_lossy(bytes).trim().to_owned();
        if line.is_empty() {
            return;
        }

        let mut recent = self
            .recent
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while recent.len() >= self.recent_capacity {
            recent.pop_front();
        }
        recent.push_back(line);
    }
}

impl Write for FanoutWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.file.write(buffer)?;
        self.pending.extend_from_slice(&buffer[..written]);
        self.publish_complete_lines();
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()?;
        self.publish_pending();
        Ok(())
    }
}

pub(super) struct SizeRotatingWriter {
    path: PathBuf,
    file: Option<File>,
    current_size: u64,
    max_size: u64,
    retained_files: usize,
}

impl SizeRotatingWriter {
    pub(super) fn new(
        path: impl AsRef<Path>,
        max_size: u64,
        retained_files: usize,
    ) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let current_size = file.metadata()?.len();
        let mut writer = Self {
            path,
            file: Some(file),
            current_size,
            max_size: max_size.max(1),
            retained_files,
        };

        if writer.current_size >= writer.max_size {
            writer.rotate()?;
        }

        Ok(writer)
    }

    fn backup_path(&self, index: usize) -> PathBuf {
        let mut name = self.path.as_os_str().to_os_string();
        name.push(format!(".{index}"));
        PathBuf::from(name)
    }

    fn rotate(&mut self) -> io::Result<()> {
        let flush_result = if let Some(mut file) = self.file.take() {
            let result = file.flush();
            drop(file);
            result
        } else {
            Ok(())
        };

        let rotation_result = (|| {
            if self.retained_files == 0 {
                if self.path.exists() {
                    fs::remove_file(&self.path)?;
                }
            } else {
                for index in (1..=self.retained_files).rev() {
                    let source = if index == 1 {
                        self.path.clone()
                    } else {
                        self.backup_path(index - 1)
                    };
                    let destination = self.backup_path(index);

                    if source.exists() {
                        if destination.exists() {
                            fs::remove_file(&destination)?;
                        }
                        fs::rename(source, destination)?;
                    }
                }
            }
            Ok::<(), io::Error>(())
        })();

        let reopened = OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.current_size = reopened.metadata()?.len();
        self.file = Some(reopened);
        flush_result.and(rotation_result)
    }
}

impl Write for SizeRotatingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.current_size > 0
            && self.current_size.saturating_add(buffer.len() as u64) > self.max_size
        {
            self.rotate()?;
        }

        let file = self.file.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "rotating log file is closed")
        })?;
        let written = file.write(buffer)?;
        self.current_size = self.current_size.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FanoutWriter, SizeRotatingWriter};
    use std::collections::VecDeque;
    use std::fs;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory() -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("sega-observability-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn size_rotating_writer_retains_bounded_backups() {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("sega-emulator.log");
        let mut writer = SizeRotatingWriter::new(&path, 8, 2).unwrap();

        writer.write_all(b"12345678").unwrap();
        writer.write_all(b"abcdefgh").unwrap();
        writer.write_all(b"ABCDEFGH").unwrap();
        writer.flush().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"ABCDEFGH");
        assert_eq!(fs::read(path.with_extension("log.1")).unwrap(), b"abcdefgh");
        assert_eq!(fs::read(path.with_extension("log.2")).unwrap(), b"12345678");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fanout_writer_keeps_recent_complete_json_lines() {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).unwrap();
        let file = SizeRotatingWriter::new(directory.join("sega-emulator.log"), 1024, 1).unwrap();
        let recent = Arc::new(Mutex::new(VecDeque::new()));
        let mut writer = FanoutWriter::new(file, recent.clone(), 2);

        writer.write_all(b"{\"level\":\"INFO\"}\n").unwrap();
        writer.write_all(b"{\"level\":\"WARN\"}\n").unwrap();
        writer.write_all(b"{\"level\":\"ERROR\"}\n").unwrap();
        writer.flush().unwrap();

        let records = recent.lock().unwrap();
        assert_eq!(records.len(), 2);
        assert!(records[0].contains("WARN"));
        assert!(records[1].contains("ERROR"));

        drop(records);
        fs::remove_dir_all(directory).unwrap();
    }
}
