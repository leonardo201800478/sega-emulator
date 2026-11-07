use serde::{Deserialize, Serialize};

/// Console family to which a cartridge profile applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartridgeSystem {
    MasterSystem,
}

/// Hardware mapper family. Unknown is intentionally non-executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartridgeMapper {
    SmsSega,
    Unknown,
}

/// Physical RAM technology and persistence semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartridgeRamKind {
    /// SRAM with no battery or other retention source.
    VolatileSram,
    /// SRAM retained by a cartridge battery.
    BatteryBackedSram,
    /// Nonvolatile ferroelectric RAM; it does not require a battery.
    FeRam,
}

/// Address decoder known to expose the cartridge RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartridgeRamMapping {
    /// Sega mapper slot 2 ($8000-$BFFF), with capacity-specific mirroring/banking.
    SegaMapperSlot2,
    /// Address mapping is not sufficiently documented to execute.
    Unknown,
}

/// Evidence state controls whether a profile may configure a live machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileStatus {
    /// Identity and hardware behavior are sufficiently supported for execution.
    Verified,
    /// Some properties are known, but mapper/address behavior is incomplete.
    EvidenceOnly,
    /// A known uncertainty blocks executable mapping.
    Blocked,
}

/// Physical cartridge RAM description. Persistence is independent of mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartridgeRamProfile {
    pub kind: CartridgeRamKind,
    pub capacity_bytes: usize,
    pub mapping: CartridgeRamMapping,
    /// Size of the mapped CPU window, when documented.
    pub mapped_window_bytes: usize,
    /// Physical bank size used by the mapper.
    pub bank_size_bytes: usize,
    /// Whether a smaller physical RAM is mirrored across the mapped window.
    pub mirrored: bool,
}

impl CartridgeRamProfile {
    /// True for nonvolatile RAM that must be retained across emulator sessions.
    pub const fn is_persistent(&self) -> bool {
        matches!(
            self.kind,
            CartridgeRamKind::BatteryBackedSram | CartridgeRamKind::FeRam
        )
    }
}

/// Exact cartridge hardware profile selected by ROM content identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartridgeHardwareProfile {
    pub system: CartridgeSystem,
    pub mapper: CartridgeMapper,
    pub ram: Option<CartridgeRamProfile>,
    pub status: ProfileStatus,
    pub board: Option<String>,
    pub evidence: Vec<String>,
}

impl CartridgeHardwareProfile {
    /// Only verified, fully mapped profiles may configure an executing machine.
    pub fn is_runtime_usable(&self) -> bool {
        self.status == ProfileStatus::Verified
            && self.mapper != CartridgeMapper::Unknown
            && self.ram.as_ref().is_some_and(|ram| {
                ram.mapping != CartridgeRamMapping::Unknown && ram.capacity_bytes > 0
            })
    }

    /// Whether this cartridge has nonvolatile memory requiring persistent storage.
    pub fn requires_persistent_store(&self) -> bool {
        self.ram
            .as_ref()
            .is_some_and(CartridgeRamProfile::is_persistent)
    }

    /// Return the declared RAM capacity, without inferring it from ROM or save size.
    pub fn ram_capacity_bytes(&self) -> Option<usize> {
        self.ram.as_ref().map(|ram| ram.capacity_bytes)
    }
}
