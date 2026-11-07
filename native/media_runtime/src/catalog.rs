use crate::{
    CartridgeHardwareProfile, CartridgeIdentity, CartridgeMapper, CartridgeRamKind,
    CartridgeRamMapping, CartridgeRamProfile, CartridgeSystem, DatGameEntry, DatRomEntry,
    NoIntroDat, ProfileStatus,
};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownCartridgeProfile {
    pub sha1: String,
    pub profile: CartridgeHardwareProfile,
}

impl KnownCartridgeProfile {
    pub fn new(sha1: impl Into<String>, profile: CartridgeHardwareProfile) -> Self {
        Self {
            sha1: sha1.into().to_ascii_lowercase(),
            profile,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CartridgeMatch<'a> {
    pub game: &'a DatGameEntry,
    pub rom: &'a DatRomEntry,
    pub profile: Option<&'a CartridgeHardwareProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogLookupError {
    DatFingerprintMismatch {
        sha1: String,
        dat_size_bytes: usize,
        identity_size_bytes: usize,
        dat_crc32: u32,
        identity_crc32: u32,
    },
}

impl Display for CatalogLookupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DatFingerprintMismatch {
                sha1,
                dat_size_bytes,
                identity_size_bytes,
                dat_crc32,
                identity_crc32,
            } => write!(
                formatter,
                "DAT fingerprint mismatch for SHA-1 {sha1}: size {identity_size_bytes} != {dat_size_bytes}, CRC32 {identity_crc32:08X} != {dat_crc32:08X}"
            ),
        }
    }
}

impl Error for CatalogLookupError {}

/// Content-addressed ROM catalog joined to explicit hardware profiles.
#[derive(Clone, Debug)]
pub struct CartridgeCatalog {
    dat: NoIntroDat,
    profiles: Vec<KnownCartridgeProfile>,
}

impl CartridgeCatalog {
    /// Load the SMS DAT committed in this repository and its explicit hardware profiles.
    pub fn sms_default() -> Result<Self, crate::DatParseError> {
        Ok(Self::from_dat(NoIntroDat::sms_default()?, sms_profiles()))
    }

    pub fn from_dat(dat: NoIntroDat, profiles: Vec<KnownCartridgeProfile>) -> Self {
        Self { dat, profiles }
    }

    pub const fn dat(&self) -> &NoIntroDat {
        &self.dat
    }

    pub fn profiles(&self) -> &[KnownCartridgeProfile] {
        &self.profiles
    }

    /// Match an image by SHA-1 and verify the DAT's size and CRC32.
    ///
    /// The ROM filename is never consulted. A matching SHA-1 with inconsistent
    /// auxiliary metadata is reported as an integrity error, not accepted.
    pub fn lookup(
        &self,
        identity: &CartridgeIdentity,
    ) -> Result<Option<CartridgeMatch<'_>>, CatalogLookupError> {
        let sha1 = identity.sha1_hex();

        for game in &self.dat.entries {
            for rom in &game.roms {
                if !rom.sha1.eq_ignore_ascii_case(&sha1) {
                    continue;
                }
                if rom.size_bytes != identity.size_bytes() || rom.crc32 != identity.crc32() {
                    return Err(CatalogLookupError::DatFingerprintMismatch {
                        sha1,
                        dat_size_bytes: rom.size_bytes,
                        identity_size_bytes: identity.size_bytes(),
                        dat_crc32: rom.crc32,
                        identity_crc32: identity.crc32(),
                    });
                }

                let profile = self
                    .profiles
                    .iter()
                    .find(|known| known.sha1.eq_ignore_ascii_case(&rom.sha1))
                    .map(|known| &known.profile);
                return Ok(Some(CartridgeMatch { game, rom, profile }));
            }
        }

        Ok(None)
    }

    /// Return a profile only when the exact ROM identity has a verified executable mapping.
    pub fn lookup_runtime_profile(
        &self,
        identity: &CartridgeIdentity,
    ) -> Result<Option<&CartridgeHardwareProfile>, CatalogLookupError> {
        Ok(self
            .lookup(identity)?
            .and_then(|matched| matched.profile)
            .filter(|profile| profile.is_runtime_usable()))
    }
}

fn sms_profiles() -> Vec<KnownCartridgeProfile> {
    let mut profiles = Vec::new();

    // Commercial battery-backed 8 KiB SRAM identities explicitly confirmed by
    // the project No-Intro DAT, MAME sms.xml, and the project's SMS mapper audit.
    for sha1 in [
        "fbda0486b393708a89756bb57d116ad6007484e4", // Golden Axe Warrior
        "d0b964dd7cd8ccdd730de4d8e4bb2e87bea7686e", // Golfamania
        "f952406bca4918ee91a89b27e949e224eae96d85", // Miracle Warriors
        "8dd7bb4f666f70f7f57687823d0068c9100af8e5", // Monopoly
        "6bad7176011dd4bbd007498d167399daacde173d", // Monopoly Rev A
        "8762239c339a084dfb8443cc38515301476bde28", // Penguin Land
        "07fcf297be4f4c9d92cd3f119a7ac48467e06838", // Phantasy Star USA/Europe Rev A
        "fd8dad6acb6fa75dc8e9bbaea2a7e9fd486fc2dd", // Phantasy Star Brazil
        "c9a40ddd217c58dddcd6b5c0fe66c3a50d3e68e4", // Phantasy Star Japan
        "257ca76ebcd54c75a414ca7ce968fa59ea42f150", // Phantasy Star USA/Europe
        "a90e21e5961bcf2e10b715a009c04e7c2017a3b1", // Ultima IV
        "614b589080b732e17cc0d253e17216a72a268955", // Ys Japan
        "e73e836c353543e9f48315410b0d72278899ff59", // Ys The Vanished Omens
    ] {
        profiles.push(KnownCartridgeProfile::new(
            sha1,
            battery_sram_8k(ProfileStatus::Verified, false),
        ));
    }

    // Exact prototype, regional, or dump variants whose DAT/MAME RAM metadata
    // is known but whose board/mapping evidence is not sufficient for auto-map.
    for sha1 in [
        "da88dc3e84daa2f8b8d803b00a13b5fb3185d8c5", // Golfamania Beta
        "ebe733ea693fa46edde43260fdb963cab71317e1", // Super Golf Beta
        "46a032004d49fec58099aa6bf0dd796997e95142", // Haja no Fuuin Japan
        "5f1cfc974a8ffc965f18216a89472fd1a980d7cb", // Miracle Warriors Beta
        "68830fb46a010cf2f2d04bdc69e3759f6d42b8de", // Monopoly Beta 1
        "6d100d43cddfb761d3560786c26e345873452a4b", // Monopoly Beta 2
        "c01cf44eee335d509dc20a165add8514e7fbb7c4", // Doki Doki Penguin Land Japan
        "c56417efd78a332a35e4d0e87c7b13c20da85391", // Doki Doki Penguin Land Beta
        "52b2aa52a1c96e15869498a8e42b074705070007", // Phantasy Star Korea unlicensed
        "b3ae447dc739256616b44cbd77cb903c9f19e980", // Phantasy Star Japan Rev 1 (MAME baddump)
        "bb1ae06b62a9f7d3259c51eee4cfded781eb5d30", // Ultima IV Beta
    ] {
        profiles.push(KnownCartridgeProfile::new(
            sha1,
            battery_sram_8k(ProfileStatus::EvidenceOnly, true),
        ));
    }

    // RAM without battery: these profiles are volatile and must never create .srm files.
    for sha1 in [
        "60e2b6ec69d73dd73c1ef846634942c81800655b", // Desert Speedtrap
        "1732b4c13fd00dd5efc5bf1ccb1ab6ed3889c8ba", // Flash, The
    ] {
        profiles.push(KnownCartridgeProfile::new(sha1, volatile_sram_8k()));
    }

    // The Terminator has a 32 KiB volatile RAM region in MAME. Mapping is not
    // confirmed, so these identities remain evidence-only and non-executable.
    for sha1 in [
        "eaf733d385e61526b90c1b194bf605078d43e2d3", // Europe
        "bea0a1d0271faa2273e4cf1e968241d018c81da1", // Brazil
    ] {
        profiles.push(KnownCartridgeProfile::new(
            sha1,
            volatile_sram_32k_evidence(),
        ));
    }

    // Flight of Pigarus is known to use 32 KiB FeRAM, but the mapper/address
    // decode and exact page-selection protocol are still blocked.
    for sha1 in [
        "46941b6ca4c38b07fa55987d3f30c249d5674e2a", // v1.11
        "3bd03c3be402b90e61e6be5b67216c0c6d8056e6", // v1.10
        "e53a16f5bf4a2d804b26c359cf3a727c06379e6d", // v1.00
    ] {
        profiles.push(KnownCartridgeProfile::new(sha1, flight_of_pigarus_blocked()));
    }

    profiles
}

fn battery_sram_8k(status: ProfileStatus, mapping_unknown: bool) -> CartridgeHardwareProfile {
    CartridgeHardwareProfile {
        system: CartridgeSystem::MasterSystem,
        mapper: if mapping_unknown {
            CartridgeMapper::Unknown
        } else {
            CartridgeMapper::SmsSega
        },
        ram: Some(CartridgeRamProfile {
            kind: CartridgeRamKind::BatteryBackedSram,
            capacity_bytes: 0x2000,
            mapping: if mapping_unknown {
                CartridgeRamMapping::Unknown
            } else {
                CartridgeRamMapping::SegaMapperSlot2
            },
            mapped_window_bytes: if mapping_unknown { 0 } else { 0x4000 },
            bank_size_bytes: 0x2000,
            mirrored: !mapping_unknown,
        }),
        status,
        board: None,
        evidence: vec![
            "Project No-Intro SMS DAT 20261013-064924: exact SHA-1/CRC32/size".to_owned(),
            "MAME hash/sms.xml: battery=yes and 8192-byte RAM dataarea".to_owned(),
            "Project docs/hardware/sms-special-mappers-research.md".to_owned(),
            "SMS Power Development/Mappers".to_owned(),
        ],
    }
}

fn volatile_sram_8k() -> CartridgeHardwareProfile {
    CartridgeHardwareProfile {
        system: CartridgeSystem::MasterSystem,
        mapper: CartridgeMapper::SmsSega,
        ram: Some(CartridgeRamProfile {
            kind: CartridgeRamKind::VolatileSram,
            capacity_bytes: 0x2000,
            mapping: CartridgeRamMapping::SegaMapperSlot2,
            mapped_window_bytes: 0x4000,
            bank_size_bytes: 0x2000,
            mirrored: true,
        }),
        status: ProfileStatus::Verified,
        board: None,
        evidence: vec![
            "Project No-Intro SMS DAT 20261013-064924: exact SHA-1/CRC32/size".to_owned(),
            "MAME hash/sms.xml: 8192-byte RAM dataarea, no battery feature".to_owned(),
            "Project docs/hardware/sms-special-mappers-research.md: RAM without battery".to_owned(),
        ],
    }
}

fn volatile_sram_32k_evidence() -> CartridgeHardwareProfile {
    CartridgeHardwareProfile {
        system: CartridgeSystem::MasterSystem,
        mapper: CartridgeMapper::Unknown,
        ram: Some(CartridgeRamProfile {
            kind: CartridgeRamKind::VolatileSram,
            capacity_bytes: 0x8000,
            mapping: CartridgeRamMapping::Unknown,
            mapped_window_bytes: 0,
            bank_size_bytes: 0x4000,
            mirrored: false,
        }),
        status: ProfileStatus::EvidenceOnly,
        board: Some("Europe PCB 171-6339A; IC2 KM62256BLP-10L".to_owned()),
        evidence: vec![
            "Project No-Intro SMS DAT 20261013-064924: exact regional SHA-1/CRC32/size".to_owned(),
            "MAME hash/sms.xml: 32768-byte RAM dataarea; no battery feature".to_owned(),
            "Europe board/IC identification is not inherited by the Brazil revision".to_owned(),
        ],
    }
}

fn flight_of_pigarus_blocked() -> CartridgeHardwareProfile {
    CartridgeHardwareProfile {
        system: CartridgeSystem::MasterSystem,
        mapper: CartridgeMapper::Unknown,
        ram: Some(CartridgeRamProfile {
            kind: CartridgeRamKind::FeRam,
            capacity_bytes: 0x8000,
            mapping: CartridgeRamMapping::Unknown,
            mapped_window_bytes: 0,
            bank_size_bytes: 0x4000,
            mirrored: false,
        }),
        status: ProfileStatus::Blocked,
        board: Some("Lapis MR48V256 FeRAM (256 Kibit)".to_owned()),
        evidence: vec![
            "Project No-Intro SMS DAT 20261013-064924: three exact ROM fingerprints".to_owned(),
            "MAME hash/sms.xml: 32768-byte RAM dataarea".to_owned(),
            "SMS Power developer Q&A: two logical 16 KiB pages".to_owned(),
            "SMS Power PCB designer discussion: mapper and SRAM changed for two pages".to_owned(),
            "Exact CPU address decode and page-select protocol remain unconfirmed".to_owned(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{CartridgeCatalog, flight_of_pigarus_blocked, sms_profiles};
    use crate::{
        CartridgeIdentity, CartridgeRamKind, NoIntroDat, ProfileStatus,
    };

    #[test]
    fn every_known_sms_profile_has_an_exact_project_dat_entry() {
        let dat = NoIntroDat::sms_default().unwrap();
        for profile in sms_profiles() {
            assert!(
                dat.find_sha1(&profile.sha1).is_some(),
                "profile SHA-1 {} is absent from the project DAT",
                profile.sha1
            );
        }
    }

    #[test]
    fn default_catalog_keeps_flight_of_pigarus_blocked() {
        let profile = flight_of_pigarus_blocked();
        assert_eq!(profile.status, ProfileStatus::Blocked);
        assert!(!profile.is_runtime_usable());
        assert_eq!(
            profile.ram.as_ref().map(|ram| ram.kind),
            Some(CartridgeRamKind::FeRam)
        );
    }

    #[test]
    fn catalog_does_not_assign_profiles_to_unknown_roms() {
        let catalog = CartridgeCatalog::sms_default().unwrap();
        let identity = CartridgeIdentity::from_bytes(b"unknown cartridge image");
        assert!(catalog.lookup(&identity).unwrap().is_none());
    }

    #[test]
    fn volatile_ram_profiles_do_not_request_persistent_storage() {
        let catalog = CartridgeCatalog::sms_default().unwrap();
        let profile = catalog
            .profiles()
            .iter()
            .find(|entry| entry.sha1 == "60e2b6ec69d73dd73c1ef846634942c81800655b")
            .unwrap();
        assert_eq!(
            profile.profile.ram.as_ref().map(|ram| ram.kind),
            Some(CartridgeRamKind::VolatileSram)
        );
        assert!(!profile.profile.requires_persistent_store());
    }
}
