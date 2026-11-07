//! Host-side, content-addressed persistent cartridge RAM storage.
//!
//! The SRM container includes a versioned identity/metadata header and the
//! RAM payload. A complete save is written to a same-directory temporary file,
//! flushed, and atomically renamed into place.

use sega_media::{CartridgeHardwareProfile, CartridgeIdentity, CartridgeRamKind};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SAVE_MAGIC: &[u8; 8] = b"SEGA-SRM";
const SAVE_FORMAT_VERSION: u16 = 1;
const HEADER_SIZE: usize = 116;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveRamError {
    Io(String),
    ProfileNotExecutable,
    ProfileHasNoRam,
    NotPersistent,
    InvalidMagic,
    InvalidHeader,
    UnsupportedVersion(u16),
    InvalidRamKind(u8),
    IdentityMismatch,
    RamKindMismatch,
    RamSizeMismatch { expected: usize, actual: usize },
    PayloadChecksumMismatch,
    Truncated,
}

impl Display for SaveRamError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "save RAM I/O failed: {error}"),
            Self::ProfileNotExecutable => {
                write!(formatter, "cartridge profile is not verified for persistence")
            }
            Self::ProfileHasNoRam => write!(formatter, "cartridge profile has no RAM"),
            Self::NotPersistent => {
                write!(formatter, "volatile cartridge RAM must not be written to an SRM file")
            }
            Self::InvalidMagic => write!(formatter, "save RAM file has an invalid signature"),
            Self::InvalidHeader => write!(formatter, "save RAM file has invalid header flags"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported save RAM format version {version}")
            }
            Self::InvalidRamKind(kind) => write!(formatter, "unknown save RAM kind {kind}"),
            Self::IdentityMismatch => {
                write!(formatter, "save RAM file belongs to a different ROM identity")
            }
            Self::RamKindMismatch => {
                write!(formatter, "save RAM technology differs from the cartridge profile")
            }
            Self::RamSizeMismatch { expected, actual } => {
                write!(formatter, "save RAM size {actual} does not match expected {expected}")
            }
            Self::PayloadChecksumMismatch => {
                write!(formatter, "save RAM payload checksum validation failed")
            }
            Self::Truncated => write!(formatter, "save RAM file is truncated"),
        }
    }
}

impl Error for SaveRamError {}

/// Filesystem-backed save storage. This type belongs to the host layer; it is
/// never called from CPU, mapper, bus, or scheduler execution paths.
#[derive(Clone, Debug)]
pub struct SaveRamStore {
    root: PathBuf,
}

impl SaveRamStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Return the deterministic SRM path derived from the ROM SHA-256.
    pub fn path_for_identity(&self, identity: &CartridgeIdentity) -> PathBuf {
        self.root.join(format!("{}.srm", identity.sha256_hex()))
    }

    /// Load and validate a persistent RAM image. Missing files are not errors.
    pub fn load(
        &self,
        identity: &CartridgeIdentity,
        profile: &CartridgeHardwareProfile,
    ) -> Result<Option<Vec<u8>>, SaveRamError> {
        if !profile.is_runtime_usable() {
            return Err(SaveRamError::ProfileNotExecutable);
        }
        let ram = profile.ram.as_ref().ok_or(SaveRamError::ProfileHasNoRam)?;
        if !ram.is_persistent() {
            return Ok(None);
        }

        let path = self.path_for_identity(identity);
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(SaveRamError::Io(error.to_string())),
        };
        let mut encoded = Vec::new();
        file.read_to_end(&mut encoded)
            .map_err(|error| SaveRamError::Io(error.to_string()))?;

        if encoded.len() < HEADER_SIZE {
            return Err(SaveRamError::Truncated);
        }
        if &encoded[..8] != SAVE_MAGIC {
            return Err(SaveRamError::InvalidMagic);
        }

        let version = read_u16(&encoded[8..10]);
        if version != SAVE_FORMAT_VERSION {
            return Err(SaveRamError::UnsupportedVersion(version));
        }
        if encoded[11] != 0 {
            return Err(SaveRamError::InvalidHeader);
        }

        let stored_kind = decode_ram_kind(encoded[10])?;
        if stored_kind != ram.kind {
            return Err(SaveRamError::RamKindMismatch);
        }

        let stored_size = read_u64(&encoded[68..76]);
        let stored_capacity = read_u32(&encoded[76..80]) as usize;
        let payload = &encoded[HEADER_SIZE..];

        if encoded[12..32] != identity.sha1()[..]
            || encoded[32..64] != identity.sha256()[..]
            || read_u32(&encoded[64..68]) != identity.crc32()
            || stored_size != identity.size_bytes() as u64
        {
            return Err(SaveRamError::IdentityMismatch);
        }

        if stored_capacity != ram.capacity_bytes {
            return Err(SaveRamError::RamSizeMismatch {
                expected: ram.capacity_bytes,
                actual: stored_capacity,
            });
        }
        if payload.len() != ram.capacity_bytes {
            return Err(SaveRamError::RamSizeMismatch {
                expected: ram.capacity_bytes,
                actual: payload.len(),
            });
        }

        let payload_identity = CartridgeIdentity::from_bytes(payload);
        if read_u32(&encoded[80..84]) != payload_identity.crc32()
            || encoded[84..116] != payload_identity.sha256()[..]
        {
            return Err(SaveRamError::PayloadChecksumMismatch);
        }

        Ok(Some(payload.to_vec()))
    }

    /// Atomically replace the save file after validating profile and payload size.
    pub fn save(
        &self,
        identity: &CartridgeIdentity,
        profile: &CartridgeHardwareProfile,
        contents: &[u8],
    ) -> Result<(), SaveRamError> {
        if !profile.is_runtime_usable() {
            return Err(SaveRamError::ProfileNotExecutable);
        }
        let ram = profile.ram.as_ref().ok_or(SaveRamError::ProfileHasNoRam)?;
        if !ram.is_persistent() {
            return Err(SaveRamError::NotPersistent);
        }
        if contents.len() != ram.capacity_bytes {
            return Err(SaveRamError::RamSizeMismatch {
                expected: ram.capacity_bytes,
                actual: contents.len(),
            });
        }

        let ram_capacity = u32::try_from(ram.capacity_bytes).map_err(|_| {
            SaveRamError::RamSizeMismatch {
                expected: u32::MAX as usize,
                actual: ram.capacity_bytes,
            }
        })?;
        let rom_size = u64::try_from(identity.size_bytes())
            .map_err(|error| SaveRamError::Io(error.to_string()))?;
        let payload_identity = CartridgeIdentity::from_bytes(contents);
        let mut encoded = Vec::with_capacity(HEADER_SIZE + contents.len());

        encoded.extend_from_slice(SAVE_MAGIC);
        encoded.extend_from_slice(&SAVE_FORMAT_VERSION.to_le_bytes());
        encoded.push(encode_ram_kind(ram.kind)?);
        encoded.push(0);
        encoded.extend_from_slice(identity.sha1());
        encoded.extend_from_slice(identity.sha256());
        encoded.extend_from_slice(&identity.crc32().to_le_bytes());
        encoded.extend_from_slice(&rom_size.to_le_bytes());
        encoded.extend_from_slice(&ram_capacity.to_le_bytes());
        encoded.extend_from_slice(&payload_identity.crc32().to_le_bytes());
        encoded.extend_from_slice(payload_identity.sha256());
        encoded.extend_from_slice(contents);

        debug_assert_eq!(encoded.len(), HEADER_SIZE + contents.len());
        atomic_replace(&self.path_for_identity(identity), &encoded)
    }
}

fn encode_ram_kind(kind: CartridgeRamKind) -> Result<u8, SaveRamError> {
    match kind {
        CartridgeRamKind::BatteryBackedSram => Ok(1),
        CartridgeRamKind::FeRam => Ok(2),
        CartridgeRamKind::VolatileSram => Err(SaveRamError::NotPersistent),
    }
}

fn decode_ram_kind(value: u8) -> Result<CartridgeRamKind, SaveRamError> {
    match value {
        1 => Ok(CartridgeRamKind::BatteryBackedSram),
        2 => Ok(CartridgeRamKind::FeRam),
        other => Err(SaveRamError::InvalidRamKind(other)),
    }
}

fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn atomic_replace(target: &Path, contents: &[u8]) -> Result<(), SaveRamError> {
    let parent = target
        .parent()
        .ok_or_else(|| SaveRamError::Io("save path has no parent directory".to_owned()))?;
    fs::create_dir_all(parent).map_err(|error| SaveRamError::Io(error.to_string()))?;
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cartridge.srm");

    for _ in 0..64 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(SaveRamError::Io(error.to_string())),
        };

        let write_result = file.write_all(contents).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(SaveRamError::Io(error.to_string()));
        }

        if let Err(error) = fs::rename(&temporary, target) {
            let _ = fs::remove_file(&temporary);
            return Err(SaveRamError::Io(error.to_string()));
        }
        return Ok(());
    }

    Err(SaveRamError::Io(
        "could not allocate a unique temporary save file".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{SaveRamError, SaveRamStore};
    use sega_media::{
        CartridgeHardwareProfile, CartridgeIdentity, CartridgeMapper, CartridgeRamKind,
        CartridgeRamMapping, CartridgeRamProfile, CartridgeSystem, ProfileStatus,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> PathBuf {
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "sega-srm-test-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn battery_profile() -> CartridgeHardwareProfile {
        CartridgeHardwareProfile {
            system: CartridgeSystem::MasterSystem,
            mapper: CartridgeMapper::SmsSega,
            ram: Some(CartridgeRamProfile {
                kind: CartridgeRamKind::BatteryBackedSram,
                capacity_bytes: 0x2000,
                mapping: CartridgeRamMapping::SegaMapperSlot2,
                mapped_window_bytes: 0x4000,
                bank_size_bytes: 0x2000,
                mirrored: true,
            }),
            status: ProfileStatus::Verified,
            board: None,
            evidence: vec!["unit test".to_owned()],
        }
    }

    fn volatile_profile() -> CartridgeHardwareProfile {
        let mut profile = battery_profile();
        profile.ram.as_mut().unwrap().kind = CartridgeRamKind::VolatileSram;
        profile
    }

    #[test]
    fn missing_save_returns_none() {
        let directory = temporary_directory();
        let store = SaveRamStore::new(&directory);
        let identity = CartridgeIdentity::from_bytes(b"save-test-rom");

        assert_eq!(store.load(&identity, &battery_profile()).unwrap(), None);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn save_round_trip_and_atomic_replacement() {
        let directory = temporary_directory();
        let store = SaveRamStore::new(&directory);
        let identity = CartridgeIdentity::from_bytes(b"save-test-rom");
        let profile = battery_profile();
        let first = vec![0x5A; 0x2000];
        let second = vec![0xA5; 0x2000];

        store.save(&identity, &profile, &first).unwrap();
        assert_eq!(store.load(&identity, &profile).unwrap(), Some(first));
        store.save(&identity, &profile, &second).unwrap();
        assert_eq!(store.load(&identity, &profile).unwrap(), Some(second));
        assert!(store.path_for_identity(&identity).is_file());

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn save_file_is_bound_to_the_exact_rom_identity() {
        let directory = temporary_directory();
        let store = SaveRamStore::new(&directory);
        let first_identity = CartridgeIdentity::from_bytes(b"rom-one");
        let second_identity = CartridgeIdentity::from_bytes(b"rom-two");
        let profile = battery_profile();
        store
            .save(&first_identity, &profile, &[0x33; 0x2000])
            .unwrap();

        fs::copy(
            store.path_for_identity(&first_identity),
            store.path_for_identity(&second_identity),
        )
        .unwrap();

        assert_eq!(
            store.load(&second_identity, &profile),
            Err(SaveRamError::IdentityMismatch)
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn truncated_and_corrupted_saves_are_rejected() {
        let directory = temporary_directory();
        let store = SaveRamStore::new(&directory);
        let identity = CartridgeIdentity::from_bytes(b"save-test-rom");
        let profile = battery_profile();
        let path = store.path_for_identity(&identity);

        fs::write(&path, [0_u8; 20]).unwrap();
        assert_eq!(
            store.load(&identity, &profile),
            Err(SaveRamError::Truncated)
        );

        store
            .save(&identity, &profile, &[0x55; 0x2000])
            .unwrap();
        let mut encoded = fs::read(&path).unwrap();
        *encoded.last_mut().unwrap() ^= 0xFF;
        fs::write(&path, encoded).unwrap();
        assert_eq!(
            store.load(&identity, &profile),
            Err(SaveRamError::PayloadChecksumMismatch)
        );

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_wrong_capacity_and_volatile_ram() {
        let directory = temporary_directory();
        let store = SaveRamStore::new(&directory);
        let identity = CartridgeIdentity::from_bytes(b"save-test-rom");
        let persistent = battery_profile();

        assert_eq!(
            store.save(&identity, &persistent, &[0; 16]),
            Err(SaveRamError::RamSizeMismatch {
                expected: 0x2000,
                actual: 16
            })
        );
        assert_eq!(
            store.save(&identity, &volatile_profile(), &[0; 0x2000]),
            Err(SaveRamError::NotPersistent)
        );
        assert_eq!(store.load(&identity, &volatile_profile()).unwrap(), None);

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn blocked_profiles_cannot_load_or_write_saves() {
        let directory = temporary_directory();
        let store = SaveRamStore::new(&directory);
        let identity = CartridgeIdentity::from_bytes(b"save-test-rom");
        let mut blocked = battery_profile();
        blocked.status = ProfileStatus::Blocked;

        assert_eq!(
            store.load(&identity, &blocked),
            Err(SaveRamError::ProfileNotExecutable)
        );
        assert_eq!(
            store.save(&identity, &blocked, &[0; 0x2000]),
            Err(SaveRamError::ProfileNotExecutable)
        );

        let _ = fs::remove_dir_all(directory);
    }
}
