"""Carregamento validado de ROMs SG-1000 em arquivos .sg e .zip."""

from __future__ import annotations

import hashlib
import io
import zipfile
from dataclasses import dataclass
from pathlib import Path

MAX_ROM_SIZE = 0xC000
SUPPORTED_SUFFIXES = {".sg", ".rom", ".bin"}


@dataclass(frozen=True, slots=True)
class LoadedRom:
    data: bytes
    display_name: str
    source_path: Path
    member_name: str | None
    sha256: str


def _validate(data: bytes, name: str, source: Path, member: str | None) -> LoadedRom:
    if not data:
        raise ValueError(f"A ROM '{name}' está vazia.")
    if len(data) > MAX_ROM_SIZE:
        raise ValueError(
            f"A ROM '{name}' tem {len(data)} bytes; SG-1000 aceita até "
            f"{MAX_ROM_SIZE} bytes neste estágio."
        )
    return LoadedRom(
        data=data,
        display_name=name,
        source_path=source,
        member_name=member,
        sha256=hashlib.sha256(data).hexdigest(),
    )


def load_rom(path: str | Path, member_name: str | None = None) -> LoadedRom:
    """Carrega .sg/.rom/.bin ou uma ROM suportada dentro de ZIP.

    ZIPs com várias ROMs exigem que o membro seja selecionado explicitamente.
    O arquivo não é extraído para o disco.
    """
    source = Path(path)
    suffix = source.suffix.lower()
    if suffix == ".zip":
        try:
            with zipfile.ZipFile(source) as archive:
                candidates = [
                    item
                    for item in archive.infolist()
                    if not item.is_dir()
                    and Path(item.filename).suffix.lower() in SUPPORTED_SUFFIXES
                ]
                if not candidates:
                    raise ValueError("O ZIP não contém arquivos .sg, .rom ou .bin.")
                if member_name is not None:
                    matches = [
                        item for item in candidates if item.filename == member_name
                    ]
                    if len(matches) != 1:
                        raise ValueError(
                            f"Membro de ROM não encontrado no ZIP: {member_name}"
                        )
                    chosen = matches[0]
                elif len(candidates) == 1:
                    chosen = candidates[0]
                else:
                    names = ", ".join(item.filename for item in candidates[:20])
                    raise ValueError(
                        "O ZIP contém várias ROMs; escolha uma explicitamente: " + names
                    )
                if chosen.file_size > MAX_ROM_SIZE:
                    raise ValueError(
                        f"A ROM no ZIP excede o limite de {MAX_ROM_SIZE} bytes."
                    )
                data = archive.read(chosen)
                return _validate(
                    data, Path(chosen.filename).name, source, chosen.filename
                )
        except zipfile.BadZipFile as exc:
            raise ValueError(
                "O arquivo possui extensão .zip, mas não é um ZIP válido."
            ) from exc

    if suffix not in SUPPORTED_SUFFIXES:
        raise ValueError("Formato não suportado. Use .sg ou .zip (também .rom/.bin).")
    try:
        data = source.read_bytes()
    except OSError as exc:
        raise ValueError(f"Não foi possível ler '{source}': {exc}") from exc
    return _validate(data, source.name, source, None)


def list_rom_members(path: str | Path) -> list[str]:
    """Lista membros de ROM compatíveis de um ZIP sem extrair seu conteúdo."""
    try:
        with zipfile.ZipFile(Path(path)) as archive:
            return [
                item.filename
                for item in archive.infolist()
                if not item.is_dir()
                and Path(item.filename).suffix.lower() in SUPPORTED_SUFFIXES
                and item.file_size <= MAX_ROM_SIZE
            ]
    except (OSError, zipfile.BadZipFile) as exc:
        raise ValueError(f"Não foi possível inspecionar o ZIP: {exc}") from exc
