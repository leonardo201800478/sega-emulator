"""No-Intro DAT lookup and Sega ROM/disc-media classification.

The catalog is metadata only: a match never implies that a machine runtime exists.
DAT files are parsed lazily and no ROM/disc images are extracted or modified.
"""

from __future__ import annotations

import hashlib
import re
import zlib
from dataclasses import dataclass
from pathlib import Path
from xml.etree import ElementTree

SYSTEM_BY_DAT_NAME = {
    "Sega - SG-1000 - SC-3000": "sg1000",
    "Sega - Master System - Mark III": "master_system",
    "Sega - Game Gear": "game_gear",
    "Sega - Mega Drive - Genesis": "mega_drive",
    "Sega - Mega CD & Sega CD": "sega_cd",
    "Sega - 32X": "32x",
}

SYSTEM_LABELS = {
    "sg1000": "SG-1000",
    "master_system": "Master System",
    "game_gear": "Game Gear",
    "mega_drive": "Mega Drive",
    "sega_cd": "Sega CD",
    "32x": "32X",
}

DISC_SUFFIXES = frozenset({".iso", ".cue", ".chd"})
ROM_SUFFIXES = frozenset({".sg", ".sms", ".gg", ".md", ".gen", ".32x", ".rom", ".bin"})
CUE_FILE_PATTERN = re.compile(
    r'^\s*FILE\s+(?:"([^"]+)"|([^\s]+))\s+([A-Za-z0-9_]+)',
    re.IGNORECASE | re.MULTILINE,
)


@dataclass(frozen=True, slots=True)
class DatRom:
    system: str
    game_name: str
    description: str
    rom_name: str
    size: int
    crc32: str | None
    md5: str | None
    sha1: str | None
    regions: tuple[str, ...]
    dat_name: str


@dataclass(frozen=True, slots=True)
class CatalogIdentification:
    """Result of exact catalog lookup; unknown metadata remains explicitly unknown."""

    status: str
    system: str | None
    game_name: str | None
    rom_name: str | None
    regions: tuple[str, ...]
    matched_by: str | None
    dat_name: str | None
    candidates: tuple[DatRom, ...] = ()


@dataclass(frozen=True, slots=True)
class DiscMediaDescriptor:
    """Describes disc-file layout without decoding or mounting the image."""

    system: str
    kind: str
    source_path: Path
    data_files: tuple[Path, ...]
    complete: bool
    problems: tuple[str, ...] = ()


def default_dat_directory() -> Path:
    """Return the repository's root-level dats directory."""
    return Path(__file__).resolve().parents[2] / "dats"


def _normalize_hash(value: str | None) -> str | None:
    return value.casefold() if value else None


def _system_for_dat(header_name: str) -> str | None:
    for prefix, system in SYSTEM_BY_DAT_NAME.items():
        if header_name == prefix or header_name.startswith(prefix + " "):
            return system
    return None


def _read_dat(path: Path) -> list[DatRom]:
    try:
        root = ElementTree.parse(path).getroot()
    except (OSError, ElementTree.ParseError) as exc:
        raise ValueError(f"DAT inválido ou ilegível '{path}': {exc}") from exc

    header = root.find("header")
    header_name = header.findtext("name", "") if header is not None else ""
    system = _system_for_dat(header_name.strip())
    if system is None:
        raise ValueError(
            f"Sistema não reconhecido no cabeçalho do DAT: {header_name!r}"
        )

    entries: list[DatRom] = []
    for game in root.findall("game"):
        game_name = game.get("name", "")
        description = game.findtext("description", game_name)
        regions = tuple(
            sorted(
                {
                    region.strip().upper()
                    for release in game.findall("release")
                    if (region := release.get("region", "").strip())
                }
            )
        )
        for rom in game.findall("rom"):
            try:
                size = int(rom.get("size", ""))
            except ValueError:
                continue
            entries.append(
                DatRom(
                    system=system,
                    game_name=game_name,
                    description=description or game_name,
                    rom_name=rom.get("name", ""),
                    size=size,
                    crc32=_normalize_hash(rom.get("crc")),
                    md5=_normalize_hash(rom.get("md5")),
                    sha1=_normalize_hash(rom.get("sha1")),
                    regions=regions,
                    dat_name=path.name,
                )
            )
    return entries


class RomCatalog:
    """Lazy, in-memory index of the DAT files found in a directory."""

    def __init__(self, dat_directory: str | Path | None = None) -> None:
        self.dat_directory = (
            Path(dat_directory) if dat_directory else default_dat_directory()
        )
        self._loaded = False
        self._entries: tuple[DatRom, ...] = ()
        self._by_sha1: dict[tuple[int, str], list[DatRom]] = {}
        self._by_md5: dict[tuple[int, str], list[DatRom]] = {}
        self._by_crc32: dict[tuple[int, str], list[DatRom]] = {}

    @property
    def loaded(self) -> bool:
        return self._loaded

    @property
    def entry_count(self) -> int:
        self._ensure_loaded()
        return len(self._entries)

    def reload(self) -> None:
        """Rebuild the in-memory index from the current DAT files."""
        entries: list[DatRom] = []
        if not self.dat_directory.is_dir():
            self._entries = ()
            self._index_entries()
            self._loaded = True
            return
        for path in sorted(
            self.dat_directory.glob("*.dat"), key=lambda item: item.name.casefold()
        ):
            entries.extend(_read_dat(path))
        self._entries = tuple(entries)
        self._index_entries()
        self._loaded = True

    def _ensure_loaded(self) -> None:
        if not self._loaded:
            self.reload()

    def _index_entries(self) -> None:
        self._by_sha1 = {}
        self._by_md5 = {}
        self._by_crc32 = {}
        for entry in self._entries:
            if entry.sha1:
                self._by_sha1.setdefault((entry.size, entry.sha1), []).append(entry)
            if entry.md5:
                self._by_md5.setdefault((entry.size, entry.md5), []).append(entry)
            if entry.crc32:
                self._by_crc32.setdefault((entry.size, entry.crc32), []).append(entry)

    def identify_bytes(self, data: bytes, *, suffix: str = "") -> CatalogIdentification:
        """Identify a byte image by SHA-1, then MD5, then CRC-32 plus size."""
        self._ensure_loaded()
        size = len(data)
        sha1 = hashlib.sha1(data).hexdigest()
        matches = self._by_sha1.get((size, sha1), [])
        matched_by = "sha1"
        if not matches:
            md5 = hashlib.md5(data).hexdigest()
            matches = self._by_md5.get((size, md5), [])
            matched_by = "md5"
        if not matches:
            crc32 = f"{zlib.crc32(data) & 0xFFFFFFFF:08x}"
            matches = self._by_crc32.get((size, crc32), [])
            matched_by = "crc32+size"
        if not matches:
            return CatalogIdentification(
                status="not_catalogued",
                system=None,
                game_name=None,
                rom_name=None,
                regions=(),
                matched_by=None,
                dat_name=None,
            )

        unique_systems = {entry.system for entry in matches}
        unique_games = {(entry.system, entry.game_name) for entry in matches}
        regions = tuple(
            sorted({region for entry in matches for region in entry.regions})
        )
        first = matches[0]
        return CatalogIdentification(
            status="matched" if len(unique_systems) == 1 else "ambiguous",
            system=first.system if len(unique_systems) == 1 else None,
            game_name=first.game_name if len(unique_games) == 1 else None,
            rom_name=first.rom_name if len(matches) == 1 else None,
            regions=regions,
            matched_by=matched_by,
            dat_name=first.dat_name
            if len({entry.dat_name for entry in matches}) == 1
            else None,
            candidates=tuple(matches),
        )

    def identify_path(self, path: str | Path) -> CatalogIdentification:
        """Hash a single file incrementally; large disc images may take time."""
        source = Path(path)
        digest_sha1 = hashlib.sha1()
        digest_md5 = hashlib.md5()
        digest_crc32 = 0
        size = 0
        try:
            with source.open("rb") as stream:
                while chunk := stream.read(1024 * 1024):
                    size += len(chunk)
                    digest_sha1.update(chunk)
                    digest_md5.update(chunk)
                    digest_crc32 = zlib.crc32(chunk, digest_crc32)
        except OSError as exc:
            raise ValueError(f"Não foi possível ler '{source}': {exc}") from exc

        self._ensure_loaded()
        matches = self._by_sha1.get((size, digest_sha1.hexdigest()), [])
        matched_by = "sha1"
        if not matches:
            matches = self._by_md5.get((size, digest_md5.hexdigest()), [])
            matched_by = "md5"
        if not matches:
            matches = self._by_crc32.get((size, f"{digest_crc32 & 0xFFFFFFFF:08x}"), [])
            matched_by = "crc32+size"
        if not matches:
            return CatalogIdentification(
                status="not_catalogued",
                system=None,
                game_name=None,
                rom_name=None,
                regions=(),
                matched_by=None,
                dat_name=None,
            )
        systems = {entry.system for entry in matches}
        games = {(entry.system, entry.game_name) for entry in matches}
        first = matches[0]
        return CatalogIdentification(
            status="matched" if len(systems) == 1 else "ambiguous",
            system=first.system if len(systems) == 1 else None,
            game_name=first.game_name if len(games) == 1 else None,
            rom_name=first.rom_name if len(matches) == 1 else None,
            regions=tuple(sorted({r for entry in matches for r in entry.regions})),
            matched_by=matched_by,
            dat_name=first.dat_name
            if len({entry.dat_name for entry in matches}) == 1
            else None,
            candidates=tuple(matches),
        )


def inspect_disc_media(path: str | Path) -> DiscMediaDescriptor:
    """Classify Sega CD ISO/CUE/CHD media and validate referenced CUE files exist.

    A BIN is treated as a disc track only when referenced by a sibling CUE file.
    This describes media; it does not claim the Sega CD runtime can boot it.
    """
    source = Path(path).expanduser().resolve()
    suffix = source.suffix.casefold()
    if suffix not in DISC_SUFFIXES and suffix != ".bin":
        raise ValueError("Mídia de disco não suportada; use .iso, .cue, .bin ou .chd.")
    if suffix == ".bin":
        cue_candidates = sorted(
            source.parent.glob("*.cue"), key=lambda item: item.name.casefold()
        )
        referenced = False
        for cue in cue_candidates:
            try:
                text = cue.read_text(encoding="utf-8-sig", errors="replace")
            except OSError:
                continue
            if any(
                Path(quoted or unquoted).name.casefold() == source.name.casefold()
                for quoted, unquoted, _kind in CUE_FILE_PATTERN.findall(text)
            ):
                referenced = True
                break
        if not referenced:
            raise ValueError(
                "Arquivo .bin isolado: não há CUE que o identifique "
                "como faixa de disco."
            )
        return DiscMediaDescriptor("sega_cd", "bin_track", source, (source,), True)

    if suffix in {".iso", ".chd"}:
        exists = source.is_file()
        return DiscMediaDescriptor(
            "sega_cd",
            "iso" if suffix == ".iso" else "chd",
            source,
            (source,) if exists else (),
            exists,
            () if exists else ("Arquivo de imagem não encontrado.",),
        )

    try:
        cue_text = source.read_text(encoding="utf-8-sig", errors="replace")
    except OSError as exc:
        raise ValueError(f"Não foi possível ler CUE '{source}': {exc}") from exc
    references = [
        quoted or unquoted
        for quoted, unquoted, _kind in CUE_FILE_PATTERN.findall(cue_text)
    ]
    if not references:
        return DiscMediaDescriptor(
            "sega_cd",
            "cue_bin",
            source,
            (),
            False,
            ("CUE não contém diretivas FILE reconhecíveis.",),
        )
    data_files = tuple((source.parent / name).resolve() for name in references)
    missing = tuple(str(item) for item in data_files if not item.is_file())
    return DiscMediaDescriptor(
        "sega_cd",
        "cue_bin",
        source,
        data_files,
        not missing,
        tuple(f"Faixa referenciada ausente: {item}" for item in missing),
    )
