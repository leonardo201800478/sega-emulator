"""ScreenScraper API client for local ROM metadata and cover caching.

The client is intentionally synchronous and UI-agnostic. Call it from a QRunnable,
never from the Qt event thread. It identifies files by CRC32, MD5, SHA-1 and size;
it does not modify ROM files.
"""

from __future__ import annotations

import hashlib
import json
import urllib.error
import urllib.parse
import urllib.request
import zlib
from dataclasses import dataclass
from pathlib import Path
from xml.etree import ElementTree

API_URL = "https://api.screenscraper.fr/api2/jeuInfos.php"
USER_AGENT = "Sega-Emulator/0.1.0 (free desktop frontend)"
TIMEOUT_SECONDS = 15


class ScreenScraperError(RuntimeError):
    """Raised when ScreenScraper cannot return usable metadata."""


@dataclass(frozen=True, slots=True)
class ScreenScraperCredentials:
    developer_id: str
    developer_password: str
    username: str = ""
    password: str = ""


@dataclass(frozen=True, slots=True)
class ScrapedGame:
    title: str
    game_id: str
    cover_url: str | None
    metadata: dict[str, str]


def _file_hashes(path: Path) -> tuple[str, str, str, int]:
    crc = 0
    md5 = hashlib.md5()
    sha1 = hashlib.sha1()
    size = 0
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                size += len(chunk)
                crc = zlib.crc32(chunk, crc)
                md5.update(chunk)
                sha1.update(chunk)
    except OSError as exc:
        raise ScreenScraperError(f"Não foi possível ler a ROM: {exc}") from exc
    return f"{crc & 0xFFFFFFFF:08X}", md5.hexdigest(), sha1.hexdigest(), size


def lookup_rom(
    path: str | Path,
    credentials: ScreenScraperCredentials,
    *,
    language: str = "pt",
) -> ScrapedGame:
    """Look up one ROM using ScreenScraper's documented jeuInfos API."""
    rom_path = Path(path)
    if not credentials.developer_id.strip() or not credentials.developer_password:
        raise ScreenScraperError(
            "Configure as credenciais de desenvolvedor do ScreenScraper."
        )

    crc, md5, sha1, size = _file_hashes(rom_path)
    params = {
        "devid": credentials.developer_id,
        "devpassword": credentials.developer_password,
        "softname": "Sega Emulator",
        "output": "xml",
        "crc": crc,
        "md5": md5,
        "sha1": sha1,
        "romtype": "rom",
        "romnom": rom_path.name,
        "romtaille": str(size),
    }
    if credentials.username:
        params["ssid"] = credentials.username
    if credentials.password:
        params["sspassword"] = credentials.password
    request = urllib.request.Request(
        f"{API_URL}?{urllib.parse.urlencode(params)}",
        headers={"User-Agent": USER_AGENT},
    )
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
            payload = response.read()
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise ScreenScraperError(f"Falha na consulta ao ScreenScraper: {exc}") from exc

    try:
        root = ElementTree.fromstring(payload)
    except ElementTree.ParseError as exc:
        raise ScreenScraperError("Resposta XML inválida do ScreenScraper.") from exc

    error_text = _first_text(root, {"erreur", "error"})
    if error_text:
        raise ScreenScraperError(f"ScreenScraper: {error_text}")
    game = next((node for node in root.iter() if _local_name(node.tag) == "jeu"), None)
    if game is None or not game.attrib.get("id"):
        raise ScreenScraperError(
            "O ScreenScraper não encontrou correspondência para esta ROM."
        )

    title = _localized_title(game, language) or rom_path.stem
    metadata: dict[str, str] = {"title": title, "game_id": game.attrib["id"]}
    for key, tags in {
        "publisher": {"editeur"},
        "developer": {"developpeur"},
        "players": {"joueurs"},
        "release_date": {"date"},
        "description": {"synopsis"},
    }.items():
        value = _first_text(game, tags, language=language)
        if value:
            metadata[key] = value

    cover_url = None
    for node in game.iter():
        if _local_name(node.tag) != "media":
            continue
        media_type = node.attrib.get("type", "").casefold()
        url = node.attrib.get("url") or (node.text or "").strip()
        if url and any(marker in media_type for marker in ("box-2d", "box2d", "cover")):
            cover_url = url
            break
    return ScrapedGame(title, game.attrib["id"], cover_url, metadata)


def download_cover(url: str, destination: str | Path) -> Path:
    """Download one cover image atomically into the local metadata cache."""
    target = Path(destination)
    target.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
            payload = response.read(12 * 1024 * 1024 + 1)
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise ScreenScraperError(f"Falha ao baixar a capa: {exc}") from exc
    if len(payload) > 12 * 1024 * 1024:
        raise ScreenScraperError("A imagem de capa excede o limite de 12 MiB.")
    if not payload.startswith((b"\\x89PNG\\r\\n\\x1a\\n", b"\\xff\\xd8\\xff", b"RIFF")):
        raise ScreenScraperError(
            "O servidor não retornou uma imagem PNG, JPEG ou WebP válida."
        )
    temporary = target.with_suffix(target.suffix + ".tmp")
    temporary.write_bytes(payload)
    temporary.replace(target)
    return target


def save_metadata(game: ScrapedGame, destination: str | Path) -> Path:
    target = Path(destination)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix(target.suffix + ".tmp")
    temporary.write_text(
        json.dumps(game.metadata, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    temporary.replace(target)
    return target


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1].casefold()


def _first_text(
    root: ElementTree.Element,
    names: set[str],
    *,
    language: str | None = None,
) -> str:
    for node in root.iter():
        if _local_name(node.tag) not in names:
            continue
        if language:
            region = node.attrib.get("langue") or node.attrib.get("region")
            if region and region.casefold() not in {language.casefold(), "wor", "us"}:
                continue
        value = (node.text or "").strip()
        if value:
            return value
    return ""


def _localized_title(root: ElementTree.Element, language: str) -> str:
    for node in root.iter():
        if _local_name(node.tag) != "nom":
            continue
        region = node.attrib.get("region", "").casefold()
        if region == language.casefold():
            value = (node.text or "").strip()
            if value:
                return value
    return _first_text(root, {"nom"})
