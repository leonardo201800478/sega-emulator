"""Auditoria sequencial de ROMs SMS pelo runtime nativo Rust.

A ferramenta não implementa hardware em Python, não extrai ZIPs ao disco e
nunca inclui bytes de ROM/BIOS no relatório. Os limites de descoberta e leitura
são deliberadamente explícitos para evitar consumo de memória não limitado.
"""

from __future__ import annotations

import hashlib
import json
import time
import zipfile
import zlib
from collections.abc import Callable, Iterator
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from itertools import islice
from pathlib import Path
from threading import Event

SMS_ROM_SUFFIXES = frozenset({".sms", ".rom", ".bin"})
MAX_SMS_ROM_SIZE = 0x100000
MAX_DISCOVERED_FILES = 10_000
MAX_ZIP_MEMBERS = 20_000
MAX_ZIP_ARCHIVE_SIZE = 64 * 1024 * 1024
MAX_DISCOVERED_TARGETS = 20_000
READ_CHUNK_SIZE = 64 * 1024


class SmsRomAuditLimitError(ValueError):
    """Raised when discovery exceeds a configured safety limit."""


@dataclass(frozen=True, slots=True)
class SmsRomAuditTarget:
    """A loose ROM file or a specific member of a ZIP archive."""

    source_path: Path
    display_name: str
    member_name: str | None = None
    member_index: int | None = None
    declared_size: int | None = None
    read_error: str | None = None


@dataclass(frozen=True, slots=True)
class SmsRomAuditResult:
    """Observable result of one isolated machine execution."""

    display_name: str
    source_path: str
    member_name: str | None
    sha256: str | None
    status: str
    frames_requested: int
    frames_advanced: int
    instructions: int
    tstates: int
    pc: int | None
    region: str | None
    bios_enabled_at_start: bool | None
    bios_enabled_at_end: bool | None
    framebuffer_sha256: str | None
    distinct_rgb_colors: int | None
    elapsed_ms: float
    error_type: str | None = None
    details: str | None = None


@dataclass(frozen=True, slots=True)
class SmsRomAuditReport:
    """JSON-serializable telemetry; never contains ROM or BIOS bytes."""

    schema_version: int
    created_at_utc: str
    root_directory: str
    recursive: bool
    frames_per_rom: int
    timeout_seconds: float
    total_discovered: int
    cancelled: bool
    results: tuple[SmsRomAuditResult, ...]

    @property
    def counts(self) -> dict[str, int]:
        """Count every result status without treating inconclusive as failure."""
        counts: dict[str, int] = {}
        for result in self.results:
            counts[result.status] = counts.get(result.status, 0) + 1
        return counts

    def to_json(self) -> str:
        """Serialize report metadata and results as readable UTF-8 JSON."""
        payload = asdict(self)
        payload["counts"] = self.counts
        return json.dumps(payload, indent=2, ensure_ascii=False)


def _bounded_file_list(directory: Path, recursive: bool) -> list[Path]:
    """Enumerate at most MAX_DISCOVERED_FILES + 1 paths before rejecting."""
    iterator: Iterator[Path] = (
        directory.rglob("*") if recursive else directory.iterdir()
    )
    files = list(
        islice((path for path in iterator if path.is_file()), MAX_DISCOVERED_FILES + 1)
    )
    if len(files) > MAX_DISCOVERED_FILES:
        raise SmsRomAuditLimitError(
            f"Varredura excedeu o limite de {MAX_DISCOVERED_FILES} arquivos."
        )
    return sorted(files, key=lambda path: (str(path).casefold(), str(path)))


def _validate_zip_archive_size(path: Path) -> None:
    """Reject oversized ZIP containers before parsing central-directory metadata."""
    size = path.stat().st_size
    if size > MAX_ZIP_ARCHIVE_SIZE:
        raise SmsRomAuditLimitError(
            f"ZIP excede o limite de {MAX_ZIP_ARCHIVE_SIZE} bytes: {path.name}"
        )


def discover_sms_rom_targets(
    directory: str | Path, *, recursive: bool = True
) -> tuple[SmsRomAuditTarget, ...]:
    """Discover loose SMS ROMs and supported ZIP members with hard scan caps.

    Corrupt archives and archives without supported members are represented
    by an auditable target. Exceeding a scan cap aborts discovery explicitly.
    """
    root = Path(directory).expanduser().resolve()
    if not root.exists() or not root.is_dir():
        raise ValueError(f"A pasta de ROMs não existe ou não é diretório: {root}")

    targets: list[SmsRomAuditTarget] = []
    for path in _bounded_file_list(root, recursive):
        suffix = path.suffix.casefold()
        if suffix in SMS_ROM_SUFFIXES:
            try:
                size = path.stat().st_size
                targets.append(SmsRomAuditTarget(path, path.name, declared_size=size))
            except OSError as exc:
                targets.append(
                    SmsRomAuditTarget(
                        path, path.name, read_error=f"{type(exc).__name__}: {exc}"
                    )
                )
        elif suffix == ".zip":
            try:
                _validate_zip_archive_size(path)
                with zipfile.ZipFile(path) as archive:
                    infos = archive.infolist()
                    if len(infos) > MAX_ZIP_MEMBERS:
                        raise SmsRomAuditLimitError(
                            f"ZIP excede o limite de {MAX_ZIP_MEMBERS} membros: "
                            f"{path.name}"
                        )
                    candidates = [
                        (index, info)
                        for index, info in enumerate(infos)
                        if not info.is_dir()
                        and Path(info.filename).suffix.casefold() in SMS_ROM_SUFFIXES
                    ]
                    if not candidates:
                        targets.append(
                            SmsRomAuditTarget(
                                path,
                                path.name,
                                read_error="ZIP não contém membros .sms, .rom ou .bin.",
                            )
                        )
                    for index, info in candidates:
                        targets.append(
                            SmsRomAuditTarget(
                                path,
                                Path(info.filename).name,
                                member_name=info.filename,
                                member_index=index,
                                declared_size=info.file_size,
                            )
                        )
            except SmsRomAuditLimitError:
                raise
            except (OSError, zipfile.BadZipFile, RuntimeError) as exc:
                targets.append(
                    SmsRomAuditTarget(
                        path, path.name, read_error=f"{type(exc).__name__}: {exc}"
                    )
                )
        if len(targets) > MAX_DISCOVERED_TARGETS:
            raise SmsRomAuditLimitError(
                f"Varredura excedeu o limite de {MAX_DISCOVERED_TARGETS} ROMs."
            )
    return tuple(targets)


def _read_bounded(stream: object) -> bytes:
    """Read at most MAX_SMS_ROM_SIZE + 1 bytes, independent of declared size."""
    chunks: list[bytes] = []
    total = 0
    while total <= MAX_SMS_ROM_SIZE:
        chunk = stream.read(min(READ_CHUNK_SIZE, MAX_SMS_ROM_SIZE + 1 - total))
        if not chunk:
            break
        chunks.append(chunk)
        total += len(chunk)
    if total > MAX_SMS_ROM_SIZE:
        raise ValueError(f"ROM excede o limite atual de {MAX_SMS_ROM_SIZE} bytes.")
    data = b"".join(chunks)
    if not data:
        raise ValueError("A imagem ROM está vazia.")
    return data


def read_sms_rom_target(target: SmsRomAuditTarget) -> bytes:
    """Read one ROM with a strict streaming bound; never extract ZIP members."""
    if target.read_error:
        raise ValueError(target.read_error)
    if target.declared_size is not None and target.declared_size > MAX_SMS_ROM_SIZE:
        raise ValueError(f"ROM excede o limite atual de {MAX_SMS_ROM_SIZE} bytes.")
    if target.member_index is not None:
        _validate_zip_archive_size(target.source_path)
        with zipfile.ZipFile(target.source_path) as archive:
            infos = archive.infolist()
            if len(infos) > MAX_ZIP_MEMBERS or target.member_index >= len(infos):
                raise ValueError("O ZIP excede os limites ou mudou desde a descoberta.")
            info = infos[target.member_index]
            if info.filename != target.member_name:
                raise ValueError("A estrutura do ZIP mudou desde a descoberta.")
            with archive.open(info, "r") as stream:
                return _read_bounded(stream)
    with target.source_path.open("rb") as stream:
        return _read_bounded(stream)


def _distinct_rgb_colors(framebuffer: bytes) -> int:
    """Count distinct 24-bit RGB colors in a framebuffer."""
    if len(framebuffer) % 3:
        return 0
    return len(
        {
            (framebuffer[offset], framebuffer[offset + 1], framebuffer[offset + 2])
            for offset in range(0, len(framebuffer), 3)
        }
    )


def run_sms_rom_audit(
    targets: tuple[SmsRomAuditTarget, ...],
    machine_factory: Callable[[bytes], object],
    *,
    root_directory: str | Path,
    recursive: bool = True,
    frames_per_rom: int = 120,
    timeout_seconds: float = 5.0,
    max_steps_per_frame: int = 100_000,
    cancel_event: Event | None = None,
    on_result: Callable[[int, int, SmsRomAuditResult], None] | None = None,
) -> SmsRomAuditReport:
    """Execute each ROM in a fresh native machine and record bounded telemetry.

    The status EXECUTED_VISUAL_ACTIVITY is heuristic evidence only: frame
    advancement and a non-uniform framebuffer do not prove a successful game
    boot. EXECUTED_INCONCLUSIVE is not a failure. Timeout is checked between
    native frames and may overshoot by at most one bounded run_frame call.
    """
    if frames_per_rom < 1:
        raise ValueError("frames_per_rom deve ser positivo.")
    if timeout_seconds <= 0:
        raise ValueError("timeout_seconds deve ser positivo.")
    if max_steps_per_frame < 1:
        raise ValueError("max_steps_per_frame deve ser positivo.")

    results: list[SmsRomAuditResult] = []
    cancelled = False
    total = len(targets)

    for index, target in enumerate(targets, start=1):
        if cancel_event is not None and cancel_event.is_set():
            cancelled = True
            break

        started = time.perf_counter()
        rom_hash = None
        machine = None
        data_loaded = False
        frames_advanced = instructions = tstates = 0
        pc = None
        region = None
        bios_start = bios_end = None
        framebuffer_hash = None
        colors = None
        status = "EXECUTION_FAILED"
        error_type = details = None

        try:
            data = read_sms_rom_target(target)
            data_loaded = True
            rom_hash = hashlib.sha256(data).hexdigest()
            machine = machine_factory(data)
            region_value = getattr(machine, "region", None)
            region = str(region_value) if region_value is not None else None
            bios_value = getattr(machine, "bios_enabled", None)
            bios_start = bool(bios_value) if bios_value is not None else None
            initial_frame = int(machine.vdp.frame_number)
            deadline = started + timeout_seconds

            for _ in range(frames_per_rom):
                if cancel_event is not None and cancel_event.is_set():
                    status = "CANCELLED"
                    cancelled = True
                    break
                if time.perf_counter() >= deadline:
                    status, error_type = "TIMEOUT", "PerRomTimeout"
                    details = f"Limite de {timeout_seconds:.2f}s atingido entre frames."
                    break
                machine.run_frame(max_steps=max_steps_per_frame)
                frames_advanced = int(machine.vdp.frame_number) - initial_frame
                if time.perf_counter() >= deadline:
                    status, error_type = "TIMEOUT", "PerRomTimeout"
                    details = (
                        f"Limite de {timeout_seconds:.2f}s excedido durante um frame."
                    )
                    break
                if frames_advanced < 1:
                    status, error_type = "FRAME_NOT_ADVANCING", "FrameCounterStalled"
                    details = "run_frame terminou sem avançar o contador do VDP."
                    break

            instructions = int(machine.total_instructions)
            tstates = int(machine.total_tstates)
            pc = int(machine.cpu.pc)
            bios_value = getattr(machine, "bios_enabled", None)
            bios_end = bool(bios_value) if bios_value is not None else None

            if status not in {"TIMEOUT", "FRAME_NOT_ADVANCING", "CANCELLED"}:
                framebuffer = bytes(machine.vdp.framebuffer)
                height = int(machine.vdp.visible_height)
                expected_size = 256 * height * 3
                if len(framebuffer) != expected_size:
                    status, error_type = (
                        "INVALID_FRAMEBUFFER",
                        "FramebufferSizeMismatch",
                    )
                    details = (
                        f"Framebuffer tem {len(framebuffer)} bytes; esperado "
                        f"{expected_size} para {height} linhas."
                    )
                else:
                    framebuffer_hash = hashlib.sha256(framebuffer).hexdigest()
                    colors = _distinct_rgb_colors(framebuffer)
                    status = (
                        "EXECUTED_VISUAL_ACTIVITY"
                        if frames_advanced >= frames_per_rom and colors > 1
                        else "EXECUTED_INCONCLUSIVE"
                    )
                    if status == "EXECUTED_INCONCLUSIVE":
                        details = (
                            "Execução observada, mas avanço/atividade visual "
                            "não é evidência suficiente de boot do jogo."
                        )
        except (
            OSError,
            ValueError,
            zipfile.BadZipFile,
            RuntimeError,
            zlib.error,
        ) as exc:
            error_type, details = type(exc).__name__, str(exc)
            status = (
                "READ_OR_EXTRACTION_FAILED"
                if not data_loaded
                else "MACHINE_CREATION_FAILED"
                if machine is None
                else "EXECUTION_FAILED"
            )
        except Exception as exc:
            error_type, details = type(exc).__name__, str(exc)
            status = (
                "READ_OR_EXTRACTION_FAILED"
                if not data_loaded
                else "MACHINE_CREATION_FAILED"
                if machine is None
                else "EXECUTION_FAILED"
            )

        result = SmsRomAuditResult(
            display_name=target.display_name,
            source_path=str(target.source_path),
            member_name=target.member_name,
            sha256=rom_hash,
            status=status,
            frames_requested=frames_per_rom,
            frames_advanced=max(0, frames_advanced),
            instructions=instructions,
            tstates=tstates,
            pc=pc,
            region=region,
            bios_enabled_at_start=bios_start,
            bios_enabled_at_end=bios_end,
            framebuffer_sha256=framebuffer_hash,
            distinct_rgb_colors=colors,
            elapsed_ms=round((time.perf_counter() - started) * 1000, 3),
            error_type=error_type,
            details=details,
        )
        results.append(result)
        if on_result is not None:
            on_result(index, total, result)
        if cancelled:
            break

    return SmsRomAuditReport(
        schema_version=2,
        created_at_utc=datetime.now(UTC).isoformat(),
        root_directory=str(Path(root_directory).expanduser().resolve()),
        recursive=recursive,
        frames_per_rom=frames_per_rom,
        timeout_seconds=timeout_seconds,
        total_discovered=total,
        cancelled=cancelled,
        results=tuple(results),
    )
