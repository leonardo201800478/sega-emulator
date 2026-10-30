"""Explicit SMS firmware discovery and content-based identification.

The scanner is opt-in: callers decide when to scan a user-selected directory.
Firmware bytes are read only for hashing and are never included in diagnostics.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path

SMS_BIOS_SIZE = 0x2000


class SmsFirmwareRegion(StrEnum):
    """Known regional identity of an SMS BIOS image."""

    JAPAN = "japan"
    EXPORT_US_EU = "export_us_eu"


class SmsFirmwareFileStatus(StrEnum):
    """Validation result for one file found in the selected directory."""

    VALID_JAPAN = "valid_japan"
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


# US and Europe share the same known image; keep one signature with two aliases.
_KNOWN_BIOS: dict[str, SmsFirmwareRegion] = {
    "67846e26764bd862f19179294347f7353a4166b62ac4198a5ec32933b7da486e": (
        SmsFirmwareRegion.JAPAN
    ),
    "477617917a12a30f9f43844909dc2de6e6a617430f5c9a36306c86414a670d50": (
        SmsFirmwareRegion.EXPORT_US_EU
    ),
}


def scan_sms_firmware_directory(
    directory: str | Path | None,
) -> SmsFirmwareScan:
    """Scan a user-selected directory and identify known BIOS files by SHA-256.

    The function performs no network access and never scans implicitly at import
    or machine startup. Every regular file is considered regardless of its name
    or extension. Known BIOS signatures require exactly 8 KiB.
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
    for candidate in candidates:
        try:
            data = candidate.read_bytes()
            size = len(data)
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
        if region is None:
            status = SmsFirmwareFileStatus.NOT_FOUND
        elif region is SmsFirmwareRegion.JAPAN:
            status = SmsFirmwareFileStatus.VALID_JAPAN
        else:
            status = SmsFirmwareFileStatus.VALID_EXPORT_US_EU
        scanned.append(SmsFirmwareFile(candidate, size, digest, region, status))
        if region is not None:
            recognized.setdefault(digest, []).append(len(scanned) - 1)

    for indexes in recognized.values():
        if len(indexes) > 1:
            for index in indexes:
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
