"""Auditoria sequencial de ROMs SMS usando exclusivamente o runtime nativo Rust.

A auditoria não extrai arquivos ZIP para o disco, não altera a biblioteca e não
mantém uma segunda implementação de hardware em Python. Cada item cria uma
instância isolada de SMSMachine e executa frames reais pelo binding PyO3.
"""

from __future__ import annotations

import hashlib
import json
import time
import zipfile
import zlib
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from threading import Event
from typing import Callable

SMS_ROM_SUFFIXES = frozenset({".sms", ".rom", ".bin"})
ZIP_SUFFIX = ".zip"
MAX_SMS_ROM_SIZE = 0x100000


@dataclass(frozen=True, slots=True)
class SmsRomAuditTarget:
    """Um arquivo solto ou um membro específico de um arquivo ZIP."""

    source_path: Path
    display_name: str
    member_name: str | None = None
    member_index: int | None = None
    declared_size: int | None = None
    read_error: str | None = None


@dataclass(frozen=True, slots=True)
class SmsRomAuditResult:
    """Resultado observável de uma única tentativa de inicialização."""

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
    framebuffer_sha256: str | None
    distinct_rgb_colors: int | None
    elapsed_ms: float
    error_type: str | None = None
    details: str | None = None


@dataclass(frozen=True, slots=True)
class SmsRomAuditReport:
    """Relatório serializável, sem bytes de ROM ou BIOS."""

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
        """Conta resultados por estado sem colapsar inconclusivos em falhas."""
        values: dict[str, int] = {}
        for result in self.results:
            values[result.status] = values.get(result.status, 0) + 1
        return values

    def to_json(self) -> str:
        """Serializa o relatório e seus contadores em JSON UTF-8 legível."""
        payload = asdict(self)
        payload["counts"] = self.counts
        return json.dumps(payload, indent=2, ensure_ascii=False)


def discover_sms_rom_targets(
    directory: str | Path, *, recursive: bool = True
) -> tuple[SmsRomAuditTarget, ...]:
    """Descobre ROMs SMS soltas e todos os membros SMS suportados em ZIPs.

    O ZIP é apenas inspecionado; nenhum membro é extraído para o sistema de
    arquivos. Arquivos corrompidos e ZIPs sem ROM também viram resultados
    auditáveis, em vez de desaparecerem silenciosamente da contagem.
    """
    root = Path(directory).expanduser().resolve()
    if not root.exists() or not root.is_dir():
        raise ValueError(f"A pasta de ROMs não existe ou não é diretório: {root}")

    iterator = root.rglob("*") if recursive else root.iterdir()
    files = sorted(
        (path for path in iterator if path.is_file()),
        key=lambda path: (str(path).casefold(), str(path)),
    )
    targets: list[SmsRomAuditTarget] = []
    for path in files:
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
        elif suffix == ZIP_SUFFIX:
            try:
                with zipfile.ZipFile(path) as archive:
                    candidates = [
                        (index, info)
                        for index, info in enumerate(archive.infolist())
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
            except (OSError, zipfile.BadZipFile, RuntimeError) as exc:
                targets.append(
                    SmsRomAuditTarget(
                        path,
                        path.name,
                        read_error=f"{type(exc).__name__}: {exc}",
                    )
                )
    return tuple(targets)


def read_sms_rom_target(target: SmsRomAuditTarget) -> bytes:
    """Lê um item sem extração persistente e aplica o limite do loader SMS."""
    if target.read_error:
        raise ValueError(target.read_error)
    if target.declared_size is not None and target.declared_size > MAX_SMS_ROM_SIZE:
        raise ValueError(f"ROM excede o limite atual de {MAX_SMS_ROM_SIZE} bytes.")
    if target.member_index is not None:
        with zipfile.ZipFile(target.source_path) as archive:
            infos = archive.infolist()
            if target.member_index >= len(infos):
                raise ValueError("O membro ZIP mudou desde a descoberta.")
            info = infos[target.member_index]
            if info.filename != target.member_name:
                raise ValueError("A estrutura do ZIP mudou desde a descoberta.")
            data = archive.read(info)
    else:
        data = target.source_path.read_bytes()
    if not data:
        raise ValueError("A imagem ROM está vazia.")
    if len(data) > MAX_SMS_ROM_SIZE:
        raise ValueError(f"ROM excede o limite atual de {MAX_SMS_ROM_SIZE} bytes.")
    return data


def _distinct_rgb_colors(framebuffer: bytes) -> int:
    """Conta cores RGB distintas em um framebuffer de 24 bits."""
    if len(framebuffer) % 3:
        return 0
    return len(
        {
            (framebuffer[index], framebuffer[index + 1], framebuffer[index + 2])
            for index in range(0, len(framebuffer), 3)
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
    """Executa cada ROM sequencialmente em uma máquina Rust recém-criada.

    BOOT_CONFIRMED exige avanço do contador de frames e framebuffer RGB com
    mais de uma cor. EXECUTED_NO_VISUAL_VARIATION é inconclusivo, não falha:
    uma tela legítima pode ser uniforme. Timeout é verificado entre frames;
    run_frame permanece limitado por max_steps_per_frame.
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
        rom_hash: str | None = None
        machine = None
        rom_data_loaded = False
        frames_advanced = 0
        instructions = 0
        tstates = 0
        pc: int | None = None
        framebuffer_hash: str | None = None
        colors: int | None = None
        status = "BOOT_FAILED"
        error_type: str | None = None
        details: str | None = None

        try:
            data = read_sms_rom_target(target)
            rom_data_loaded = True
            rom_hash = hashlib.sha256(data).hexdigest()
            machine = machine_factory(data)
            initial_frame = int(machine.vdp.frame_number)
            deadline = started + timeout_seconds
            for _ in range(frames_per_rom):
                if cancel_event is not None and cancel_event.is_set():
                    status = "CANCELLED"
                    cancelled = True
                    break
                if time.perf_counter() >= deadline:
                    status = "TIMEOUT"
                    error_type = "PerRomTimeout"
                    details = f"Limite de {timeout_seconds:.2f}s atingido entre frames."
                    break
                machine.run_frame(max_steps=max_steps_per_frame)
                current_frame = int(machine.vdp.frame_number)
                frames_advanced = current_frame - initial_frame
                if time.perf_counter() >= deadline:
                    status = "TIMEOUT"
                    error_type = "PerRomTimeout"
                    details = (
                        f"Limite de {timeout_seconds:.2f}s excedido durante um frame."
                    )
                    break
                if frames_advanced < 1:
                    status = "FRAME_NOT_ADVANCING"
                    error_type = "FrameCounterStalled"
                    details = "run_frame terminou sem avançar o contador do VDP."
                    break
            instructions = int(machine.total_instructions)
            tstates = int(machine.total_tstates)
            pc = int(machine.cpu.pc)
            if status not in {"TIMEOUT", "FRAME_NOT_ADVANCING", "CANCELLED"}:
                framebuffer = bytes(machine.vdp.framebuffer)
                height = int(machine.vdp.visible_height)
                expected_size = 256 * height * 3
                if len(framebuffer) != expected_size:
                    status = "INVALID_FRAMEBUFFER"
                    error_type = "FramebufferSizeMismatch"
                    details = (
                        f"Framebuffer tem {len(framebuffer)} bytes; "
                        f"esperado {expected_size} para {height} linhas."
                    )
                else:
                    framebuffer_hash = hashlib.sha256(framebuffer).hexdigest()
                    colors = _distinct_rgb_colors(framebuffer)
                    status = (
                        "BOOT_CONFIRMED"
                        if frames_advanced >= frames_per_rom and colors > 1
                        else "EXECUTED_NO_VISUAL_VARIATION"
                    )
                    if status != "BOOT_CONFIRMED":
                        details = (
                            "A CPU/VDP executou, mas não houve evidência visual "
                            "suficiente para confirmar o boot."
                        )
        except (
            OSError,
            ValueError,
            zipfile.BadZipFile,
            RuntimeError,
            zlib.error,
        ) as exc:
            error_type = type(exc).__name__
            details = str(exc)
            status = (
                "READ_OR_EXTRACTION_FAILED"
                if not rom_data_loaded
                else "MACHINE_CREATION_FAILED"
                if machine is None
                else "BOOT_FAILED"
            )
        except Exception as exc:
            error_type = type(exc).__name__
            details = str(exc)
            status = (
                "READ_OR_EXTRACTION_FAILED"
                if not rom_data_loaded
                else "MACHINE_CREATION_FAILED"
                if machine is None
                else "BOOT_FAILED"
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
        schema_version=1,
        created_at_utc=datetime.now(timezone.utc).isoformat(),
        root_directory=str(Path(root_directory).expanduser().resolve()),
        recursive=recursive,
        frames_per_rom=frames_per_rom,
        timeout_seconds=timeout_seconds,
        total_discovered=total,
        cancelled=cancelled,
        results=tuple(results),
    )
