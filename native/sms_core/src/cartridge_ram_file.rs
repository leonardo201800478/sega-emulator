//! Persistent host-side storage for Sega Mapper cartridge SRAM.
//!
//! The emulated SRAM remains ordinary Rust memory. This module serializes that
//! memory to a versioned, checksummed .srm file at explicit host lifecycle
//! boundaries; it never performs filesystem I/O from a CPU bus access.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::SmsMemory;

const MAGIC: &[u8; 8] = b"SMSRAM01";
const FORMAT_VERSION: u16 = 1;
const HEADER_SIZE: usize = 18;
const SUPPORTED_SIZES: [usize; 3] = [0x2000, 0x4000, 0x8000];

#[derive(Debug)]
pub enum SmsCartridgeRamFileError {
    Io(io::Error),
    InvalidRamSize { size: usize },
    InvalidFile,
    SizeMismatch { expected: usize, actual: usize },
    ChecksumMismatch,
    NoCartridgeRam,
}

impl std::fmt::Display for SmsCartridgeRamFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cartridge SRAM file I/O failed: {error}"),
            Self::InvalidRamSize { size } => write!(
                f,
                "unsupported cartridge SRAM size {size}; expected 8192, 16384, or 32768 bytes"
            ),
            Self::InvalidFile => f.write_str("cartridge SRAM file header or version is invalid"),
            Self::SizeMismatch { expected, actual } => write!(
                f,
                "cartridge SRAM file has {actual} bytes; configured hardware requires {expected}"
            ),
            Self::ChecksumMismatch => f.write_str("cartridge SRAM file checksum does not match"),
            Self::NoCartridgeRam => f.write_str("the loaded cartridge has no configured SRAM"),
            Self::SizeMismatch { expected, actual } => write!(f, "cartridge SRAM buffer has {actual} bytes; expected {expected}"),
        }
    }
}

impl std::error::Error for SmsCartridgeRamFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SmsCartridgeRamFileError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Explicit .srm persistence handle associated with one cartridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsCartridgeRamFile {
    path: PathBuf,
}

impl SmsCartridgeRamFile {
    /// Use an explicit save-file path. The caller owns ROM-to-save association.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Derive the conventional .srm sibling path from a selected ROM path.
    pub fn for_rom_path(rom_path: impl AsRef<Path>) -> Self {
        Self {
            path: rom_path.as_ref().with_extension("srm"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load an existing image, returning None when no save file exists.
    pub fn load(
        &self,
        expected_size: usize,
    ) -> Result<Option<Vec<u8>>, SmsCartridgeRamFileError> {
        validate_size(expected_size)?;
        self.recover_backup_if_needed()?;

        let mut file = match OpenOptions::new().read(true).open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() < HEADER_SIZE || &bytes[..8] != MAGIC {
            return Err(SmsCartridgeRamFileError::InvalidFile);
        }

        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        let stored_size = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        let stored_checksum = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]);
        if version != FORMAT_VERSION || !SUPPORTED_SIZES.contains(&stored_size) {
            return Err(SmsCartridgeRamFileError::InvalidFile);
        }
        if stored_size != expected_size {
            return Err(SmsCartridgeRamFileError::SizeMismatch {
                expected: expected_size,
                actual: stored_size,
            });
        }
        let payload = &bytes[HEADER_SIZE..];
        if payload.len() != stored_size {
            return Err(SmsCartridgeRamFileError::InvalidFile);
        }
        if crc32(payload) != stored_checksum {
            return Err(SmsCartridgeRamFileError::ChecksumMismatch);
        }
        Ok(Some(payload.to_vec()))
    }

    /// Load a save or return a zero-initialized SRAM image on first launch.
    /// This does not create a file; persistence occurs only when save() is called.
    pub fn load_or_initialize(
        &self,
        expected_size: usize,
    ) -> Result<Vec<u8>, SmsCartridgeRamFileError> {
        validate_size(expected_size)?;
        Ok(self
            .load(expected_size)?
            .unwrap_or_else(|| vec![0; expected_size]))
    }

    /// Persist SRAM at a deliberate checkpoint or clean machine shutdown.
    ///
    /// Writes a complete checksummed image to a temporary sibling, then rotates
    /// the previous file through a backup so an interrupted replacement can be
    /// recovered on the next load. No filesystem access occurs on the emulated
    /// CPU/bus hot path.
    pub fn save(&self, ram: &[u8]) -> Result<(), SmsCartridgeRamFileError> {
        validate_size(ram.len())?;
        if let Some(parent) = self.path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }

        let temporary = sibling_path(&self.path, "tmp");
        let backup = sibling_path(&self.path, "bak");
        let mut image = Vec::with_capacity(HEADER_SIZE + ram.len());
        image.extend_from_slice(MAGIC);
        image.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        image.extend_from_slice(&(ram.len() as u32).to_le_bytes());
        image.extend_from_slice(&crc32(ram).to_le_bytes());
        image.extend_from_slice(ram);

        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&image)?;
            file.sync_all()?;
        }

        let had_previous = self.path.exists();
        if had_previous {
            if backup.exists() {
                fs::remove_file(&backup)?;
            }
            fs::rename(&self.path, &backup)?;
        }

        if let Err(error) = fs::rename(&temporary, &self.path) {
            if had_previous && !self.path.exists() {
                let _ = fs::rename(&backup, &self.path);
            }
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }

        if had_previous {
            fs::remove_file(backup)?;
        }
        Ok(())
    }

    /// Restore a validated save image into the already configured mapper SRAM.
    /// Returns false when no save file exists; never silently replaces bad data.
    pub fn load_into(&self, memory: &mut SmsMemory) -> Result<bool, SmsCartridgeRamFileError> {
        let ram = memory.cartridge_ram_mut().ok_or(SmsCartridgeRamFileError::NoCartridgeRam)?;
        let Some(saved) = self.load(ram.len())? else {
            return Ok(false);
        };
        if saved.len() != ram.len() {
            return Err(SmsCartridgeRamFileError::SizeMismatch {
                expected: ram.len(),
                actual: saved.len(),
            });
        }
        ram.copy_from_slice(&saved);
        Ok(true)
    }

    /// Persist the mapper's current SRAM image, rejecting cartridges without SRAM.
    pub fn save_from_memory(&self, memory: &SmsMemory) -> Result<(), SmsCartridgeRamFileError> {
        let ram = memory.cartridge_ram().ok_or(SmsCartridgeRamFileError::NoCartridgeRam)?;
        self.save(ram)
    }

    fn recover_backup_if_needed(&self) -> Result<(), SmsCartridgeRamFileError> {
        if !self.path.exists() {
            let backup = sibling_path(&self.path, "bak");
            if backup.exists() {
                fs::rename(backup, &self.path)?;
            }
        }
        Ok(())
    }
}

fn validate_size(size: usize) -> Result<(), SmsCartridgeRamFileError> {
    if SUPPORTED_SIZES.contains(&size) {
        Ok(())
    } else {
        Err(SmsCartridgeRamFileError::InvalidRamSize { size })
    }
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::{SmsCartridgeRamFile, SmsCartridgeRamFileError};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_file() -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "sega-sms-sram-{}-{id}.srm",
            std::process::id()
        ))
    }

    fn cleanup(path: &std::path::Path) {
        for suffix in ["", ".tmp", ".bak"] {
            let candidate = if suffix.is_empty() {
                path.to_path_buf()
            } else {
                PathBuf::from(format!("{}{suffix}", path.display()))
            };
            let _ = fs::remove_file(candidate);
        }
    }

    #[test]
    fn missing_file_initializes_zeroed_sram_without_creating_a_file() {
        let path = temporary_file();
        cleanup(&path);
        let store = SmsCartridgeRamFile::new(&path);

        let ram = store.load_or_initialize(0x2000).unwrap();

        assert_eq!(ram, vec![0; 0x2000]);
        assert!(!path.exists());
        cleanup(&path);
    }

    #[test]
    fn save_and_load_round_trip_all_supported_capacities() {
        for size in [0x2000, 0x4000, 0x8000] {
            let path = temporary_file();
            cleanup(&path);
            let store = SmsCartridgeRamFile::new(&path);
            let ram: Vec<u8> = (0..size).map(|index| (index as u8).wrapping_mul(17)).collect();

            store.save(&ram).unwrap();

            assert_eq!(store.load(size).unwrap(), Some(ram));
            cleanup(&path);
        }
    }

    #[test]
    fn rejects_a_save_created_for_a_different_sram_capacity() {
        let path = temporary_file();
        cleanup(&path);
        let store = SmsCartridgeRamFile::new(&path);
        store.save(&vec![0xA5; 0x2000]).unwrap();

        assert!(matches!(
            store.load(0x4000),
            Err(SmsCartridgeRamFileError::SizeMismatch {
                expected: 0x4000,
                actual: 0x2000
            })
        ));
        cleanup(&path);
    }

    #[test]
    fn detects_corrupted_payload_instead_of_silently_resetting_progress() {
        let path = temporary_file();
        cleanup(&path);
        let store = SmsCartridgeRamFile::new(&path);
        store.save(&vec![0x5A; 0x2000]).unwrap();
        let mut image = fs::read(&path).unwrap();
        *image.last_mut().unwrap() ^= 0x01;
        fs::write(&path, image).unwrap();

        assert!(matches!(
            store.load(0x2000),
            Err(SmsCartridgeRamFileError::ChecksumMismatch)
        ));
        cleanup(&path);
    }

    #[test]
    fn replaces_an_existing_save_without_losing_the_new_payload() {
        let path = temporary_file();
        cleanup(&path);
        let store = SmsCartridgeRamFile::new(&path);
        store.save(&vec![0x11; 0x2000]).unwrap();
        store.save(&vec![0x22; 0x2000]).unwrap();

        assert_eq!(store.load(0x2000).unwrap(), Some(vec![0x22; 0x2000]));
        assert!(!PathBuf::from(format!("{}.bak", path.display())).exists());
        cleanup(&path);
    }

    #[test]
    fn derives_srm_as_a_sibling_of_the_rom() {
        let store = SmsCartridgeRamFile::for_rom_path("roms/sonic.sms");
        assert_eq!(store.path(), std::path::Path::new("roms/sonic.srm"));
    }

    #[test]
    fn rejects_unsupported_ram_capacity_before_touching_disk() {
        let path = temporary_file();
        cleanup(&path);
        let store = SmsCartridgeRamFile::new(&path);

        assert!(matches!(
            store.save(&[0; 0x1000]),
            Err(SmsCartridgeRamFileError::InvalidRamSize { size: 0x1000 })
        ));
        assert!(!path.exists());
    }
}
