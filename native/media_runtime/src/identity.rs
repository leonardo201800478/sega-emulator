use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;

/// Content-derived identity for one exact ROM image.
///
/// SHA-1 is retained for No-Intro DAT compatibility. SHA-256 is the storage
/// identity and integrity key. CRC32 and size are auxiliary consistency checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartridgeIdentity {
    sha1: [u8; 20],
    sha256: [u8; 32],
    crc32: u32,
    size_bytes: usize,
}

impl CartridgeIdentity {
    /// Calculate all identity fields from the exact bytes that will be loaded.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            sha1: Sha1::digest(bytes).into(),
            sha256: Sha256::digest(bytes).into(),
            crc32: crc32fast::hash(bytes),
            size_bytes: bytes.len(),
        }
    }

    /// Return the No-Intro-compatible SHA-1 digest as lowercase hexadecimal.
    pub fn sha1_hex(&self) -> String {
        hex::encode(self.sha1)
    }

    /// Return the persistent storage identity as lowercase SHA-256 hexadecimal.
    pub fn sha256_hex(&self) -> String {
        hex::encode(self.sha256)
    }

    /// Return the auxiliary CRC32 fingerprint.
    pub const fn crc32(&self) -> u32 {
        self.crc32
    }

    /// Return the exact ROM byte length.
    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    /// Return the raw SHA-1 digest.
    pub const fn sha1(&self) -> &[u8; 20] {
        &self.sha1
    }

    /// Return the raw SHA-256 digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

#[cfg(test)]
mod tests {
    use super::CartridgeIdentity;

    #[test]
    fn computes_standard_hash_vectors_for_abc() {
        let identity = CartridgeIdentity::from_bytes(b"abc");

        assert_eq!(
            identity.sha1_hex(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            identity.sha256_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(identity.crc32(), 0x3524_41C2);
        assert_eq!(identity.size_bytes(), 3);
    }

    #[test]
    fn identity_changes_when_rom_bytes_change() {
        let first = CartridgeIdentity::from_bytes(b"rom-a");
        let second = CartridgeIdentity::from_bytes(b"rom-b");

        assert_ne!(first, second);
        assert_ne!(first.sha256(), second.sha256());
    }
}
