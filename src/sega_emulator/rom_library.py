"""Incremental, non-destructive ROM library indexing for the Qt frontend.

Discovery is not ROM validation: this module indexes candidate files only. The
Rust-backed loader remains responsible for validating and loading supported ROMs.
"""

from __future__ import annotations

import hashlib
import json
import os
from collections import defaultdict
from collections.abc import Callable, Iterable
from dataclasses import asdict, dataclass
from pathlib import Path
from threading import Event

SUPPORTED_SYSTEMS = (
    "sg1000",
    "master_system",
    "game_gear",
    "mega_drive",
    "sega_cd",
    "32x",
)

DEFAULT_DIRECTORY_NAMES = {
    "sg1000": "SG-1000",
    "master_system": "Master System",
    "game_gear": "Game Gear",
    "mega_drive": "Mega Drive",
    "sega_cd": "Sega CD",
    "32x": "32X",
}

# These suffixes identify possible library entries, not validated ROM formats.
CANDIDATE_SUFFIXES = frozenset(
    {
        ".sg",
        ".sms",
        ".gg",
        ".md",
        ".gen",
        ".32x",
        ".rom",
        ".bin",
        ".iso",
        ".cue",
        ".chd",
        ".zip",
    }
)

ProgressCallback = Callable[[int, int, str], None]


@dataclass(frozen=True, slots=True)
class RomEntry:
    system: str
    path: str
    name: str
    size: int
    mtime_ns: int
    sha256: str
    suffix: str

    @classmethod
    def from_dict(cls, value: dict[str, object]) -> RomEntry:
        return cls(
            system=str(value["system"]),
            path=str(value["path"]),
            name=str(value["name"]),
            size=int(value["size"]),
            mtime_ns=int(value["mtime_ns"]),
            sha256=str(value["sha256"]),
            suffix=str(value["suffix"]),
        )


@dataclass(frozen=True, slots=True)
class ScanError:
    path: str
    message: str


@dataclass(frozen=True, slots=True)
class ScanResult:
    system: str
    entries: tuple[RomEntry, ...]
    added: int
    updated: int
    removed: int
    unchanged: int
    errors: tuple[ScanError, ...]
    cancelled: bool
    full_scan: bool
    duplicate_groups: tuple[tuple[str, ...], ...]


class RomLibrary:
    """Persistent index with independent libraries for each emulated system."""

    CACHE_VERSION = 1

    def __init__(self, index_path: str | Path) -> None:
        self.index_path = Path(index_path)
        self._entries: dict[str, dict[str, RomEntry]] = {
            system: {} for system in SUPPORTED_SYSTEMS
        }
        self._load_index()

    @staticmethod
    def default_directories(root: str | Path) -> dict[str, tuple[Path, ...]]:
        base = Path(root).expanduser()
        return {
            system: (base / directory_name,)
            for system, directory_name in DEFAULT_DIRECTORY_NAMES.items()
        }

    def entries(self, system: str) -> tuple[RomEntry, ...]:
        self._validate_system(system)
        return tuple(
            sorted(
                self._entries[system].values(),
                key=lambda entry: (entry.name.casefold(), entry.path.casefold()),
            )
        )

    def scan(
        self,
        system: str,
        directories: Iterable[str | Path],
        *,
        full: bool = False,
        recursive: bool = True,
        progress: ProgressCallback | None = None,
        cancel: Event | None = None,
    ) -> ScanResult:
        """Reconcile one system's configured roots without touching ROM files.

        Incremental scans reuse a hash only when file size and nanosecond mtime
        match the cached record. Full scans re-hash every discovered candidate.
        """
        self._validate_system(system)
        roots: list[Path] = []
        root_errors: list[ScanError] = []
        for directory in directories:
            try:
                root = Path(directory).expanduser().resolve()
            except (OSError, RuntimeError) as exc:
                root_errors.append(ScanError(str(directory), str(exc)))
                continue
            if root not in roots:
                roots.append(root)

        candidates: dict[str, Path] = {}
        errors = list(root_errors)
        completed_roots: list[Path] = []
        for root in roots:
            if not root.exists():
                errors.append(ScanError(str(root), "Diretório não encontrado."))
                continue
            if not root.is_dir():
                errors.append(ScanError(str(root), "O caminho não é um diretório."))
                continue
            try:
                iterator = root.rglob("*") if recursive else root.glob("*")
                for path in iterator:
                    if path.is_file() and path.suffix.casefold() in CANDIDATE_SUFFIXES:
                        try:
                            resolved = path.resolve()
                            candidates[str(resolved)] = resolved
                        except (OSError, RuntimeError) as exc:
                            errors.append(ScanError(str(path), str(exc)))
                            raise OSError(f"Não foi possível resolver {path}") from exc
                completed_roots.append(root)
            except OSError as exc:
                errors.append(ScanError(str(root), str(exc)))

        previous = self._entries[system]
        before = dict(previous)
        current: dict[str, RomEntry] = {}
        added = updated = unchanged = 0
        ordered_paths = sorted(candidates.values(), key=lambda path: str(path).casefold())
        total = len(ordered_paths)

        for index, path in enumerate(ordered_paths, start=1):
            if cancel is not None and cancel.is_set():
                break
            path_text = str(path)
            try:
                stat = path.stat()
                cached = before.get(path_text)
                if (
                    not full
                    and cached is not None
                    and cached.size == stat.st_size
                    and cached.mtime_ns == stat.st_mtime_ns
                ):
                    entry = cached
                    unchanged += 1
                else:
                    digest = _sha256_file(path)
                    entry = RomEntry(
                        system=system,
                        path=path_text,
                        name=path.name,
                        size=stat.st_size,
                        mtime_ns=stat.st_mtime_ns,
                        sha256=digest,
                        suffix=path.suffix.casefold(),
                    )
                    if cached is None:
                        added += 1
                    else:
                        updated += 1
                current[path_text] = entry
            except OSError as exc:
                errors.append(ScanError(path_text, str(exc)))
                if cached := before.get(path_text):
                    current[path_text] = cached
            if progress is not None:
                progress(index, total, path_text)

        cancelled = cancel is not None and cancel.is_set()
        if cancelled:
            # Cancellation must not drop entries which have not been visited yet.
            current.update({key: value for key, value in before.items() if key not in current})
            removed = 0
        else:
            scoped_before = {
                key: entry
                for key, entry in before.items()
                if _is_under_any_root(Path(key), completed_roots)
            }
            removed = len(set(scoped_before) - set(current))
            # Keep cache entries under missing/inaccessible roots; transient disk or
            # permission failures must not erase the user's library index.
            protected_roots = [root for root in roots if root not in completed_roots]
            protected_before = {
                key: value
                for key, value in before.items()
                if _is_under_any_root(Path(key), protected_roots)
            }
            current.update(protected_before)
            current.update(
                {
                    key: value
                    for key, value in before.items()
                    if key not in scoped_before and key not in protected_before
                }
            )

        self._entries[system] = current
        self._save_index()
        duplicate_groups = _find_duplicate_groups(current.values())
        return ScanResult(
            system=system,
            entries=tuple(sorted(current.values(), key=lambda entry: entry.name.casefold())),
            added=added,
            updated=updated,
            removed=removed,
            unchanged=unchanged,
            errors=tuple(errors),
            cancelled=cancelled,
            full_scan=full,
            duplicate_groups=duplicate_groups,
        )

    def _load_index(self) -> None:
        try:
            payload = json.loads(self.index_path.read_text(encoding="utf-8"))
            if payload.get("version") != self.CACHE_VERSION:
                return
            for system in SUPPORTED_SYSTEMS:
                records = payload.get("systems", {}).get(system, [])
                self._entries[system] = {
                    entry.path: entry
                    for entry in (RomEntry.from_dict(record) for record in records)
                }
        except (OSError, ValueError, TypeError, KeyError, AttributeError):
            # A missing/corrupt index is recoverable: the next scan rebuilds it.
            self._entries = {system: {} for system in SUPPORTED_SYSTEMS}

    def _save_index(self) -> None:
        payload = {
            "version": self.CACHE_VERSION,
            "systems": {
                system: [asdict(entry) for entry in self.entries(system)]
                for system in SUPPORTED_SYSTEMS
            },
        }
        self.index_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.index_path.with_suffix(self.index_path.suffix + ".tmp")
        temporary.write_text(
            json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        os.replace(temporary, self.index_path)

    @staticmethod
    def _validate_system(system: str) -> None:
        if system not in SUPPORTED_SYSTEMS:
            raise ValueError(f"Sistema desconhecido: {system}")


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _is_under_any_root(path: Path, roots: list[Path]) -> bool:
    for root in roots:
        try:
            path.relative_to(root)
            return True
        except ValueError:
            continue
    return False


def _find_duplicate_groups(
    entries: Iterable[RomEntry],
) -> tuple[tuple[str, ...], ...]:
    by_hash: dict[str, list[str]] = defaultdict(list)
    for entry in entries:
        by_hash[entry.sha256].append(entry.path)
    return tuple(
        tuple(sorted(paths, key=str.casefold))
        for paths in by_hash.values()
        if len(paths) > 1
    )
