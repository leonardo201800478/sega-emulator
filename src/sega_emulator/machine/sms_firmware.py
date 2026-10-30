"""Explicit SMS firmware discovery and content-based identification.

The scanner is opt-in: callers decide when to scan a user-selected directory.
Firmware bytes are read only for hashing and are never included in diagnostics.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path

from sega_emulator.rom_catalog import DatRom, RomCatalog

SMS_BIOS_SIZE = 0x2000


class SmsFirmwareRegion(StrEnum):
    """Known regional identity of an SMS BIOS image."""

    JAPAN = "japan"
    EUROPE = "europe"
    EXPORT_US_EU = "export_us_eu"


class SmsFirmwareFileStatus(StrEnum):
    """Validation result for one file found in the selected directory."""

    VALID_JAPAN = "valid_japan"
    VALID_EUROPE = "valid_europe"
    VALID_EXPORT_US_EU = "valid_export_us_eu"
    DUPLICATE_HASH = "duplicate_hash"
    NOT_FOUND = "not_found"
    INVALID_FILE = "invalid_file"


class SmsFirmwareDirectoryStatus(StrEnum):
    """State of the user-selected firmware directory."""

    READY = "ready"
    NOT_CONFIGURED = "not_configured"
    DIRECTORY_MISSING = "directory_missing"
    INVALID_DIRECTORY = "invalid_directory"


@dataclass(frozen=True, slots=True)
class SmsFirmwareFile:
    """Metadata for one scanned file; the firmware bytes are not retained."""

    path: Path
    size: int | None
    sha256: str | None
    region: SmsFirmwareRegion | None
    status: SmsFirmwareFileStatus
    diagnostic: str | None = None


@dataclass(frozen=True, slots=True)
class SmsFirmwareScan:
    """Immutable result of an explicit directory scan."""

    directory: Path | None
    status: SmsFirmwareDirectoryStatus
    files: tuple[SmsFirmwareFile, ...]


# Known revisions are identified independently, then mapped to the compatible BIOS family.
_KNOWN_BIOS: dict[str, SmsFirmwareRegion] = {
    "67846e26764bd862f19179294347f7353a4166b62ac4198a5ec32933b7da486e": (
        SmsFirmwareRegion.JAPAN
    ),
    "477617917a12a30f9f43844909dc2de6e6a617430f5c9a36306c86414a670d50": (
        SmsFirmwareRegion.EXPORT_US_EU
    ),
    "1eb16fc06ceaf05651d7cfd69d1713e6ddccc4349a511f6fd17e037d71f9d85b": (
        SmsFirmwareRegion.EUROPE
    ),
}


def _sms_bios_region_from_dat_candidates(
    candidates: tuple[DatRom, ...],
    *,
    require_bios_entry: bool,
) -> SmsFirmwareRegion | None:
    """Resolve only unanimous, known SMS BIOS families from DAT metadata."""
    if not candidates:
        return None

    resolved: set[SmsFirmwareRegion] = set()
    for candidate in candidates:
        if candidate.system != "master_system":
            return None
        if require_bios_entry and (
            not candidate.game_name.casefold().startswith("[bios] sega master system")
            or candidate.status != "verified"
        ):
            return None
        regions = set(candidate.regions)
        if regions and regions <= {"JPN"}:
            resolved.add(SmsFirmwareRegion.JAPAN)
        elif regions == {"EUR"}:
            # A Europe-only DAT identity may select the distinct v2.0 image.
            resolved.add(SmsFirmwareRegion.EUROPE)
        elif regions and regions <= {"USA", "EUR", "BRA", "AUS"}:
            # Multi-territory export releases use the shared v1.3 family.
            # Brazil's PAL-M timing remains independent from BIOS identity.
            resolved.add(SmsFirmwareRegion.EXPORT_US_EU)
        else:
            return None

    return next(iter(resolved)) if len(resolved) == 1 else None


def _read_sms_bios_candidate(path: Path) -> bytes:
    """Read at most one byte beyond the supported BIOS size."""
    with path.open("rb") as stream:
        return stream.read(SMS_BIOS_SIZE + 1)


def scan_sms_firmware_directory(
    directory: str | Path | None,
    *,
    catalog: RomCatalog | None = None,
) -> SmsFirmwareScan:
    """Scan a user-selected directory and identify SMS BIOS images by content.

    The function performs no network access and never scans implicitly at import
    or machine startup. Every regular file is considered regardless of its name
    or extension. Known firmware requires exactly 8 KiB and either a trusted
    SHA-256 signature or an exact SHA-1/MD5 match to a No-Intro SMS BIOS entry.
    """
    if directory is None or not str(directory).strip():
        return SmsFirmwareScan(None, SmsFirmwareDirectoryStatus.NOT_CONFIGURED, ())

    root = Path(directory).expanduser()
    if not root.exists():
        return SmsFirmwareScan(root, SmsFirmwareDirectoryStatus.DIRECTORY_MISSING, ())
    if not root.is_dir():
        return SmsFirmwareScan(root, SmsFirmwareDirectoryStatus.INVALID_DIRECTORY, ())

    try:
        candidates = sorted(
            (item for item in root.iterdir() if item.is_file()),
            key=lambda item: (item.name.casefold(), item.name),
        )
    except OSError:
        return SmsFirmwareScan(root, SmsFirmwareDirectoryStatus.INVALID_DIRECTORY, ())

    scanned: list[SmsFirmwareFile] = []
    recognized: dict[str, list[int]] = {}
    active_catalog = catalog
    for candidate in candidates:
        try:
            size = candidate.stat().st_size
            if size != SMS_BIOS_SIZE:
                scanned.append(
                    SmsFirmwareFile(
                        candidate,
                        size,
                        None,
                        None,
                        SmsFirmwareFileStatus.INVALID_FILE,
                        f"Expected {SMS_BIOS_SIZE} bytes; found {size}.",
                    )
                )
                continue
            data = _read_sms_bios_candidate(candidate)
            if len(data) != SMS_BIOS_SIZE:
                scanned.append(
                    SmsFirmwareFile(
                        candidate,
                        len(data),
                        None,
                        None,
                        SmsFirmwareFileStatus.INVALID_FILE,
                        f"Expected {SMS_BIOS_SIZE} bytes; found {len(data)}.",
                    )
                )
                continue
            digest = hashlib.sha256(data).hexdigest()
        except OSError as exc:
            scanned.append(
                SmsFirmwareFile(
                    candidate,
                    None,
                    None,
                    None,
                    SmsFirmwareFileStatus.INVALID_FILE,
                    f"Unable to read file: {exc.__class__.__name__}.",
                )
            )
            continue

        region = _KNOWN_BIOS.get(digest)
        catalog_diagnostic = None
        if region is None:
            if active_catalog is None:
                active_catalog = RomCatalog()
            try:
                identification = active_catalog.identify_bytes(data, suffix=".sms")
            except ValueError as exc:
                identification = None
                catalog_diagnostic = f"DAT lookup failed: {exc.__class__.__name__}."
            if identification is not None:
                if (
                    identification.status == "matched"
                    and identification.system == "master_system"
                    and identification.matched_by in {"sha1", "md5"}
                ):
                    region = _sms_bios_region_from_dat_candidates(
                        identification.candidates,
                        require_bios_entry=True,
                    )

        if region is None:
            status = SmsFirmwareFileStatus.NOT_FOUND
        elif region is SmsFirmwareRegion.JAPAN:
            status = SmsFirmwareFileStatus.VALID_JAPAN
        elif region is SmsFirmwareRegion.EUROPE:
            status = SmsFirmwareFileStatus.VALID_EUROPE
        else:
            status = SmsFirmwareFileStatus.VALID_EXPORT_US_EU
        scanned.append(
            SmsFirmwareFile(candidate, size, digest, region, status, catalog_diagnostic)
        )
        if region is not None:
            recognized.setdefault(digest, []).append(len(scanned) - 1)

    for indexes in recognized.values():
        # Keep the first deterministic match selectable; only later paths are
        # duplicates of that same validated firmware image.
        for index in indexes[1:]:
            item = scanned[index]
            scanned[index] = SmsFirmwareFile(
                item.path,
                item.size,
                item.sha256,
                item.region,
                SmsFirmwareFileStatus.DUPLICATE_HASH,
                "Another file has the same recognized firmware hash.",
            )

    return SmsFirmwareScan(root, SmsFirmwareDirectoryStatus.READY, tuple(scanned))


def sms_bios_region_from_cartridge(rom: bytes) -> str | None:
    """Infer SMS BIOS territory from a known Sega cartridge header.

    The high nibble at header byte 15 identifies SMS Japan (3) or SMS Export
    (4). Unknown/missing headers deliberately return None so automatic mode
    never selects a potentially incompatible BIOS by guesswork.
    """
    if not isinstance(rom, bytes):
        raise TypeError("rom deve ser bytes.")
    for offset in (0x7FF0, 0x3FF0, 0x1FF0):
        end = offset + 16
        if len(rom) < end or rom[offset : offset + 8] != b"TMR SEGA":
            continue
        region_code = rom[offset + 15] >> 4
        if region_code == 0x3:
            return "japan"
        if region_code == 0x4:
            return "usa"
        return None
    return None


def sms_bios_region_from_catalog(
    rom: bytes,
    catalog: RomCatalog,
) -> str | None:
    """Resolve BIOS territory from an exact SMS DAT match.

    Europe-only identities remain distinct from the shared USA/Europe v1.3
    firmware. Mixed export identities are deliberately conservative.
    """
    if not isinstance(rom, bytes):
        raise TypeError("rom deve ser bytes.")

    identification = catalog.identify_bytes(rom, suffix=".sms")
    if (
        identification.status != "matched"
        or identification.system != "master_system"
        or identification.matched_by not in {"sha1", "md5"}
    ):
        return None

    region = _sms_bios_region_from_dat_candidates(
        identification.candidates,
        require_bios_entry=False,
    )
    if region is SmsFirmwareRegion.JAPAN:
        return "japan"
    if region is SmsFirmwareRegion.EUROPE:
        return "europe"
    if region is SmsFirmwareRegion.EXPORT_US_EU:
        regions = set(identification.regions)
        return "brazil" if regions == {"BRA"} else "usa"
    return None


def sms_bios_region_for_rom(
    rom: bytes,
    catalog: RomCatalog,
) -> str | None:
    """Resolve BIOS territory from header and exact DAT evidence.

    A recognized header distinguishes Japan from Export only. A unanimous,
    exact DAT identity may refine Export to Europe so the Europe v2.0 BIOS can
    be preferred. Ambiguous/mixed identities retain the shared export default.
    """
    if not isinstance(rom, bytes):
        raise TypeError("rom deve ser bytes.")

    header_family = sms_bios_region_from_cartridge(rom)
    try:
        identification = catalog.identify_bytes(rom, suffix=".sms")
    except ValueError:
        return header_family

    if (
        identification.status != "matched"
        or identification.system != "master_system"
        or identification.matched_by not in {"sha1", "md5"}
    ):
        return header_family

    regions = set(identification.regions)
    if regions & {"KOR", "TAI"}:
        return None

    dat_family = _sms_bios_region_from_dat_candidates(
        identification.candidates,
        require_bios_entry=False,
    )
    export_regions = {"USA", "EUR", "BRA", "AUS"}
    if header_family == "japan":
        if regions & export_regions:
            return None
        return "japan"

    if header_family == "usa":
        if "JPN" in regions:
            return None
        if dat_family is SmsFirmwareRegion.EUROPE:
            return "europe"
        if dat_family is SmsFirmwareRegion.EXPORT_US_EU:
            return "brazil" if regions == {"BRA"} else "usa"
        return "usa"

    if dat_family is SmsFirmwareRegion.JAPAN:
        return "japan"
    if dat_family is SmsFirmwareRegion.EUROPE:
        return "europe"
    if dat_family is SmsFirmwareRegion.EXPORT_US_EU:
        return "brazil" if regions == {"BRA"} else "usa"
    return None


def load_sms_bios(
    directory: str | Path | None,
    region: str,
    *,
    catalog: RomCatalog | None = None,
) -> bytes | None:
    """Load a known BIOS compatible with the resolved console territory.

    Europe prefers its distinct v2.0 image and falls back to the shared
    USA/Europe v1.3 image. USA and Brazil never select the Europe-only image.
    The selected file is revalidated immediately before machine creation.
    """
    if region not in {"japan", "usa", "europe", "brazil"}:
        raise ValueError("region deve ser usa, europe, japan ou brazil.")

    if region == "japan":
        preferred_regions = (SmsFirmwareRegion.JAPAN,)
    elif region == "europe":
        preferred_regions = (
            SmsFirmwareRegion.EUROPE,
            SmsFirmwareRegion.EXPORT_US_EU,
        )
    else:
        preferred_regions = (SmsFirmwareRegion.EXPORT_US_EU,)

    scan = scan_sms_firmware_directory(directory, catalog=catalog)
    status_for_region = {
        SmsFirmwareRegion.JAPAN: SmsFirmwareFileStatus.VALID_JAPAN,
        SmsFirmwareRegion.EUROPE: SmsFirmwareFileStatus.VALID_EUROPE,
        SmsFirmwareRegion.EXPORT_US_EU: SmsFirmwareFileStatus.VALID_EXPORT_US_EU,
    }
    selected = None
    for expected_region in preferred_regions:
        selected = next(
            (
                item
                for item in scan.files
                if item.region is expected_region
                and item.status is status_for_region[expected_region]
            ),
            None,
        )
        if selected is not None:
            break
    if selected is None:
        return None

    try:
        data = _read_sms_bios_candidate(selected.path)
    except OSError as exc:
        raise ValueError(
            f"Não foi possível carregar a BIOS SMS '{selected.path.name}': "
            f"{exc.__class__.__name__}."
        ) from exc
    if (
        len(data) != SMS_BIOS_SIZE
        or hashlib.sha256(data).hexdigest() != selected.sha256
    ):
        raise ValueError(
            f"A BIOS SMS '{selected.path.name}' mudou após a verificação; "
            "execute uma nova varredura."
        )
    return data
