"""Typed, evidence-backed SMS cartridge hardware metadata catalog.

This catalog is intentionally separate from No-Intro ROM identity data. It
contains no inferred defaults and accepts only exact SHA-1/MD5 identities.
"""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass, field
from enum import StrEnum
_SHA1_PATTERN = re.compile(r"^[0-9a-f]{40}$")
_MD5_PATTERN = re.compile(r"^[0-9a-f]{32}$")


class MetadataSource(StrEnum):
    """Origin of an individual hardware metadata field."""

    UNKNOWN = "unknown"
    HEADER = "header"
    NO_INTRO_DAT = "no_intro_dat"
    CURATED_HARDWARE_DB = "curated_hardware_db"
    USER_OVERRIDE = "user_override"
    RUNTIME_OBSERVATION = "runtime_observation"


class MetadataConfidence(StrEnum):
    """Strength and nature of the evidence supporting a field value."""

    CONFIRMED = "confirmed"
    CATALOG_MATCH = "catalog_match"
    HEADER_DERIVED = "header_derived"
    INFERRED = "inferred"
    MANUAL = "manual"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class FieldEvidence[T]:
    """A value together with field-level provenance and confidence.

    Unknown values are represented by FieldEvidence.unknown(). A missing
    value is never silently replaced by a regional or title-based assumption.
    """

    value: T | None
    source: MetadataSource
    confidence: MetadataConfidence
    source_reference: str | None = None
    source_revision: str | None = None
    notes: str | None = None

    def __post_init__(self) -> None:
        if self.value is None:
            if (
                self.source is not MetadataSource.UNKNOWN
                or self.confidence is not MetadataConfidence.UNKNOWN
                or self.source_reference is not None
                or self.source_revision is not None
            ):
                raise ValueError(
                    "Evidence without a value must use UNKNOWN source/confidence "
                    "and cannot carry a source reference or revision."
                )
            return

        if (
            self.source is MetadataSource.UNKNOWN
            or self.confidence is MetadataConfidence.UNKNOWN
        ):
            raise ValueError("Known values require an explicit source and confidence.")
        if not self.source_reference or not self.source_reference.strip():
            raise ValueError("Known values require a non-empty source reference.")
        if self.source_revision is not None and not self.source_revision.strip():
            raise ValueError("source_revision cannot be empty.")

        if (
            self.confidence is MetadataConfidence.HEADER_DERIVED
            and self.source is not MetadataSource.HEADER
        ):
            raise ValueError("HEADER_DERIVED confidence requires HEADER source.")
        if (
            self.confidence is MetadataConfidence.MANUAL
            and self.source is not MetadataSource.USER_OVERRIDE
        ):
            raise ValueError("MANUAL confidence requires USER_OVERRIDE source.")

    @classmethod
    def unknown(cls) -> FieldEvidence[T]:
        """Create an explicit unknown field without fabricating provenance."""
        return cls(
            value=None,
            source=MetadataSource.UNKNOWN,
            confidence=MetadataConfidence.UNKNOWN,
        )


class FmCapability(StrEnum):
    """Whether the selected console hardware exposes an FM sound device."""

    AVAILABLE = "available"
    UNAVAILABLE = "unavailable"


class FmGameUsage(StrEnum):
    """Evidence about whether a specific game uses FM audio."""

    USED = "used"
    NOT_OBSERVED = "not_observed"


class FmRequirement(StrEnum):
    """Known FM requirement declared or demonstrated by a game."""

    REQUIRED = "required"
    OPTIONAL = "optional"
    NONE_KNOWN = "none_known"


@dataclass(frozen=True, slots=True)
class SmsFmMetadata:
    """Keep console FM capability separate from game usage and requirement."""

    console_capability: FieldEvidence[FmCapability] = field(
        default_factory=FieldEvidence.unknown
    )
    game_usage: FieldEvidence[FmGameUsage] = field(
        default_factory=FieldEvidence.unknown
    )
    game_requirement: FieldEvidence[FmRequirement] = field(
        default_factory=FieldEvidence.unknown
    )


class SmsPeripheral(StrEnum):
    """SMS/Mark III peripheral families; these are not cartridge mappers."""

    JOYPAD = "joypad"
    PADDLE = "paddle"
    SPORTS_PAD = "sports_pad"
    LIGHT_PHASER = "light_phaser"
    SEGASCOPE_3D = "segascope_3d"
    RESET = "reset"


class PeripheralState(StrEnum):
    """Per-game peripheral requirement/compatibility state."""

    REQUIRED = "required"
    SUPPORTED = "supported"
    NOT_REQUIRED = "not_required"
    UNKNOWN = "unknown"
    NOT_APPLICABLE = "not_applicable"


@dataclass(frozen=True, slots=True)
class SmsPeripheralMetadata:
    """Evidence-backed state for one peripheral family."""

    peripheral: SmsPeripheral
    state: FieldEvidence[PeripheralState]


class SmsSaveKind(StrEnum):
    """Cartridge save-memory class, distinct from volatile working RAM."""

    NONE_CONFIRMED = "none_confirmed"
    VOLATILE_RAM = "volatile_ram"
    BATTERY_BACKED_SRAM = "battery_backed_sram"
    NONVOLATILE_MEMORY = "nonvolatile_memory"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class SmsSaveMetadata:
    """Save-memory characteristics; capacities are bytes, not address ranges."""

    kind: FieldEvidence[SmsSaveKind] = field(default_factory=FieldEvidence.unknown)
    capacity_bytes: FieldEvidence[int] = field(default_factory=FieldEvidence.unknown)
    bank_count: FieldEvidence[int] = field(default_factory=FieldEvidence.unknown)
    window_start: FieldEvidence[int] = field(default_factory=FieldEvidence.unknown)
    window_end: FieldEvidence[int] = field(default_factory=FieldEvidence.unknown)

    def __post_init__(self) -> None:
        for name in ("capacity_bytes", "bank_count", "window_start", "window_end"):
            evidence = getattr(self, name)
            if evidence.value is not None and evidence.value < 0:
                raise ValueError(f"{name} cannot be negative.")
        if (
            self.window_start.value is not None
            and self.window_end.value is not None
            and self.window_end.value < self.window_start.value
        ):
            raise ValueError("window_end cannot precede window_start.")


@dataclass(frozen=True, slots=True)
class SmsMapperMetadata:
    """Mapper identity and banking facts, independent of console region."""

    identifier: FieldEvidence[str] = field(default_factory=FieldEvidence.unknown)
    bank_count: FieldEvidence[int] = field(default_factory=FieldEvidence.unknown)
    bank_size_bytes: FieldEvidence[int] = field(default_factory=FieldEvidence.unknown)

    def __post_init__(self) -> None:
        if self.identifier.value is not None and not self.identifier.value.strip():
            raise ValueError("Mapper identifier cannot be empty.")
        for name in ("bank_count", "bank_size_bytes"):
            evidence = getattr(self, name)
            if evidence.value is not None and evidence.value <= 0:
                raise ValueError(f"{name} must be positive.")


@dataclass(frozen=True, slots=True)
class SmsHardwareMetadata:
    """Immutable hardware metadata snapshot with provenance per field."""

    mapper: SmsMapperMetadata = field(default_factory=SmsMapperMetadata)
    fm: SmsFmMetadata = field(default_factory=SmsFmMetadata)
    peripherals: tuple[SmsPeripheralMetadata, ...] = ()
    save: SmsSaveMetadata = field(default_factory=SmsSaveMetadata)

    def __post_init__(self) -> None:
        if not isinstance(self.peripherals, tuple):
            raise TypeError("peripherals must be a tuple.")
        kinds = [item.peripheral for item in self.peripherals]
        if len(kinds) != len(set(kinds)):
            raise ValueError("A peripheral may appear only once per hardware record.")


@dataclass(frozen=True, slots=True)
class SmsHardwareRecord:
    """Curated metadata bound to one exact ROM image identity."""

    record_id: str
    rom_size: int
    metadata: SmsHardwareMetadata
    sha1: str | None = None
    md5: str | None = None

    def __post_init__(self) -> None:
        if not self.record_id.strip():
            raise ValueError("record_id cannot be empty.")
        if self.rom_size <= 0:
            raise ValueError("rom_size must be positive.")
        if self.sha1 is None and self.md5 is None:
            raise ValueError("A hardware record requires SHA-1 or MD5.")
        if self.sha1 is not None and not _SHA1_PATTERN.fullmatch(self.sha1):
            raise ValueError(
                "sha1 must be a lowercase 40-character hexadecimal digest."
            )
        if self.md5 is not None and not _MD5_PATTERN.fullmatch(self.md5):
            raise ValueError("md5 must be a lowercase 32-character hexadecimal digest.")


class SmsHardwareMatchStatus(StrEnum):
    """Outcome of exact lookup in the curated SMS hardware catalog."""

    MATCHED = "matched"
    AMBIGUOUS = "ambiguous"
    NOT_CATALOGUED = "not_catalogued"


@dataclass(frozen=True, slots=True)
class SmsHardwareIdentification:
    """Exact hardware-catalog lookup result; ambiguous matches expose no winner."""

    status: SmsHardwareMatchStatus
    metadata: SmsHardwareMetadata | None
    matched_by: str | None
    candidates: tuple[SmsHardwareRecord, ...] = ()

    def __post_init__(self) -> None:
        if not isinstance(self.candidates, tuple):
            raise TypeError("candidates must be a tuple.")
        if self.status is SmsHardwareMatchStatus.MATCHED:
            if (
                self.metadata is None
                or len(self.candidates) != 1
                or not self.matched_by
            ):
                raise ValueError(
                    "A matched result requires one record, metadata and match type."
                )
            if self.candidates[0].metadata != self.metadata:
                raise ValueError("Matched metadata must come from its candidate.")
        elif self.status is SmsHardwareMatchStatus.AMBIGUOUS:
            if len(self.candidates) < 2 or self.metadata is not None:
                raise ValueError(
                    "An ambiguous result requires multiple candidates and no winner."
                )
            if not self.matched_by:
                raise ValueError("An ambiguous result requires a match type.")
        elif self.status is SmsHardwareMatchStatus.NOT_CATALOGUED:
            if (
                self.candidates
                or self.metadata is not None
                or self.matched_by is not None
            ):
                raise ValueError("An unknown result cannot contain a match.")


class SmsHardwareCatalog:
    """Immutable in-memory index of curated SMS hardware records.

    Unlike RomCatalog, this catalog does not identify game titles or release
    regions. It only returns hardware facts tied to exact SHA-1/MD5 identities.
    CRC-32 and filename matching are intentionally unsupported.
    """

    def __init__(self, records: tuple[SmsHardwareRecord, ...] = ()) -> None:
        if not isinstance(records, tuple):
            records = tuple(records)
        record_ids = [record.record_id for record in records]
        if len(record_ids) != len(set(record_ids)):
            raise ValueError("record_id values must be unique.")

        self._records = records
        self._by_sha1: dict[tuple[int, str], list[SmsHardwareRecord]] = {}
        self._by_md5: dict[tuple[int, str], list[SmsHardwareRecord]] = {}
        for record in records:
            if record.sha1 is not None:
                self._by_sha1.setdefault((record.rom_size, record.sha1), []).append(
                    record
                )
            if record.md5 is not None:
                self._by_md5.setdefault((record.rom_size, record.md5), []).append(
                    record
                )

    @property
    def records(self) -> tuple[SmsHardwareRecord, ...]:
        """Return the immutable curated record collection."""
        return self._records

    def identify_bytes(self, data: bytes) -> SmsHardwareIdentification:
        """Resolve exact SHA-1/MD5 identity; never fall back to CRC or filename."""
        if not isinstance(data, bytes):
            raise TypeError("data must be bytes.")

        size = len(data)
        sha1 = hashlib.sha1(data).hexdigest()
        md5 = hashlib.md5(data).hexdigest()
        sha1_matches = self._by_sha1.get((size, sha1), [])
        md5_matches = self._by_md5.get((size, md5), [])

        if sha1_matches and md5_matches:
            sha1_ids = {item.record_id for item in sha1_matches}
            md5_ids = {item.record_id for item in md5_matches}
            if sha1_ids != md5_ids:
                candidates = tuple(
                    sorted(
                        {
                            item.record_id: item
                            for item in (*sha1_matches, *md5_matches)
                        }.values(),
                        key=lambda item: item.record_id,
                    )
                )
                return SmsHardwareIdentification(
                    status=SmsHardwareMatchStatus.AMBIGUOUS,
                    metadata=None,
                    matched_by="sha1+md5_conflict",
                    candidates=candidates,
                )
            matches = sha1_matches
            matched_by = "sha1+md5"
        elif sha1_matches:
            matches = sha1_matches
            matched_by = "sha1"
        elif md5_matches:
            matches = md5_matches
            matched_by = "md5"
        else:
            return SmsHardwareIdentification(
                status=SmsHardwareMatchStatus.NOT_CATALOGUED,
                metadata=None,
                matched_by=None,
            )

        if len(matches) != 1:
            return SmsHardwareIdentification(
                status=SmsHardwareMatchStatus.AMBIGUOUS,
                metadata=None,
                matched_by=matched_by,
                candidates=tuple(sorted(matches, key=lambda item: item.record_id)),
            )

        record = matches[0]
        return SmsHardwareIdentification(
            status=SmsHardwareMatchStatus.MATCHED,
            metadata=record.metadata,
            matched_by=matched_by,
            candidates=(record,),
        )
