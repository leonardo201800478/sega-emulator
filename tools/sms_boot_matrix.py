"""Run a local, non-persistent SMS game/BIOS boot matrix.

The ROMs and BIOS images are read from user-selected paths and are never copied
to the repository. The report contains hashes and execution metadata only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from sega_emulator.machine.sms import SMSMachine, load_sms_rom
from sega_emulator.machine.sms_firmware import (
    SmsFirmwareFileStatus,
    SmsFirmwareRegion,
    scan_sms_firmware_directory,
    sms_bios_region_for_rom,
)
from sega_emulator.rom_catalog import RomCatalog

SMS_HEADER_OFFSETS = (0x7FF0, 0x3FF0, 0x1FF0)
SMS_SIZE_CODES = {
    0xA: 8 * 1024,
    0xB: 16 * 1024,
    0xC: 32 * 1024,
    0xD: 48 * 1024,
    0xE: 64 * 1024,
    0xF: 128 * 1024,
    0x0: 256 * 1024,
    0x1: 512 * 1024,
    0x2: 1024 * 1024,
}
BIOS_CASES = (
    ("japan_v2_1", SmsFirmwareRegion.JAPAN, "japan"),
    ("export_v1_3", SmsFirmwareRegion.EXPORT_US_EU, "usa"),
    ("europe_v2_0", SmsFirmwareRegion.EUROPE, "europe"),
)


def _is_bcd(value: int) -> bool:
    """Return whether both nibbles encode decimal digits."""
    return value >> 4 <= 9 and value & 0x0F <= 9


def _has_codemasters_header(rom: bytes) -> bool:
    """Recognize the Codemasters $7FE0 header and its build metadata."""
    if len(rom) < 0x8000 or len(rom) % 0x4000:
        return False
    if rom[0x7FF0:0x7FF8] != b"TMR SEGA":
        return False

    header = rom[0x7FE0:0x7FF0]
    if (
        header[0] != len(rom) // 0x4000
        or not all(_is_bcd(value) for value in header[1:6])
        or any(header[10:16])
    ):
        return False

    day = (header[1] >> 4) * 10 + (header[1] & 0x0F)
    month = (header[2] >> 4) * 10 + (header[2] & 0x0F)
    hour = (header[4] >> 4) * 10 + (header[4] & 0x0F)
    minute = (header[5] >> 4) * 10 + (header[5] & 0x0F)
    checksum = int.from_bytes(header[6:8], "little")
    complement = int.from_bytes(header[8:10], "little")
    return (
        1 <= day <= 31
        and 1 <= month <= 12
        and hour <= 23
        and minute <= 59
        and (checksum + complement) & 0xFFFF == 0
    )


def _header_diagnostics(rom: bytes) -> dict[str, Any]:
    """Check the documented export-BIOS header, region, and checksum rules."""
    mapper = "codemasters" if _has_codemasters_header(rom) else "sega"
    for offset in SMS_HEADER_OFFSETS:
        header = rom[offset : offset + 16]
        if len(header) != 16 or header[:8] != b"TMR SEGA":
            continue

        size_code = header[15] & 0x0F
        declared_size = SMS_SIZE_CODES.get(size_code)
        checksum = int.from_bytes(header[10:12], "little")
        calculated = None
        if declared_size is not None and declared_size <= len(rom):
            if declared_size <= 0x8000:
                calculated = sum(rom[: declared_size - 16]) & 0xFFFF
            else:
                calculated = (
                    sum(rom[:0x7FF0]) + sum(rom[0x8000:declared_size])
                ) & 0xFFFF

        return {
            "offset": offset,
            "region_code": header[15] >> 4,
            "size_code": size_code,
            "declared_size": declared_size,
            "actual_size": len(rom),
            "mapper": mapper,
            "checksum": checksum,
            "calculated_checksum": calculated,
            "export_v1_3_header_acceptance": (
                mapper == "sega"
                and header[15] >> 4 == 4
                and calculated is not None
                and calculated == checksum
            ),
            "export_v1_3_mapper_note": (
                "Codemasters mapper: the standard BIOS checksum path is unreliable."
                if mapper == "codemasters"
                else None
            ),
        }

    return {
        "offset": None,
        "export_v1_3_header_acceptance": False,
        "reason": "TMR SEGA header not found",
    }


def _bios_preflight(rom: bytes) -> dict[str, bool]:
    """Evaluate the published cartridge-presence tests used by BIOS v2.x."""
    japanese_values = {value & 0x3C for value in rom[:256]}
    european_values = set(rom[:513])
    return {
        "japan_v2_1_detection_variation": len(japanese_values) > 1,
        "europe_v2_0_detection_variation": len(european_values) > 1,
    }


def _safe_stem(value: str) -> str:
    """Return a filesystem-safe, bounded name for local diagnostic outputs."""
    stem = re.sub(r"[^A-Za-z0-9._-]+", "_", Path(value).stem).strip("._")
    return (stem or "sms_rom")[:64]


def _frame_metrics(framebuffer: bytes) -> dict[str, int | str]:
    """Summarize framebuffer activity without embedding image bytes in the report."""
    colors = {
        framebuffer[index : index + 3] for index in range(0, len(framebuffer) - 2, 3)
    }
    non_black = sum(
        1
        for index in range(0, len(framebuffer) - 2, 3)
        if framebuffer[index] or framebuffer[index + 1] or framebuffer[index + 2]
    )
    return {
        "sha256": hashlib.sha256(framebuffer).hexdigest(),
        "unique_rgb_colors": len(colors),
        "non_black_pixels": non_black,
        "pixel_count": len(framebuffer) // 3,
    }


def _write_ppm(path: Path, framebuffer: bytes, height: int) -> None:
    """Write one local PPM frame using only the Python standard library."""
    expected_size = 256 * height * 3
    if len(framebuffer) != expected_size:
        raise ValueError(
            "Framebuffer size mismatch: "
            f"expected {expected_size}, got {len(framebuffer)}"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as output:
        output.write(f"P6\n256 {height}\n255\n".encode("ascii"))
        output.write(framebuffer)


def _run_case(
    *,
    rom_name: str,
    rom_data: bytes,
    bios_name: str | None,
    bios_data: bytes | None,
    region: str,
    frames: int,
    post_handoff_frames: int,
    capture_dir: Path,
    mapper: str,
) -> dict[str, Any]:
    """Execute a cartridge with or without BIOS and record its boot transition."""
    machine = SMSMachine(rom_data, region=region, bios=bios_data)
    handoff_frame = None
    frames_run = 0
    frames_after_handoff = None
    execution_error = None

    try:
        for frame_index in range(frames):
            machine.run_frame()
            frames_run = frame_index + 1
            if bios_data is not None and handoff_frame is None:
                if not machine.bios_enabled:
                    handoff_frame = frames_run
            if handoff_frame is not None:
                frames_after_handoff = frames_run - handoff_frame
                if frames_after_handoff >= post_handoff_frames:
                    break
    except Exception as exc:  # Keep the matrix running after an individual failure.
        execution_error = f"{type(exc).__name__}: {exc}"

    framebuffer = bytes(machine.vdp.framebuffer)
    visible_height = int(machine.vdp.visible_height)
    frame_metrics = _frame_metrics(framebuffer)
    visual_activity = (
        frame_metrics["non_black_pixels"] > 0 and frame_metrics["unique_rgb_colors"] > 1
    )
    capture_name = f"{_safe_stem(rom_name)}__{bios_name or 'direct'}__{region}.ppm"
    capture_path = capture_dir / capture_name
    _write_ppm(capture_path, framebuffer, visible_height)

    if execution_error is not None:
        outcome = "EXECUTION_ERROR"
    elif bios_data is None:
        outcome = "DIRECT_BOOT_FRAME_ADVANCE" if frames_run == frames else "INCOMPLETE"
        if outcome == "DIRECT_BOOT_FRAME_ADVANCE" and not visual_activity:
            outcome = "DIRECT_BOOT_BLANK_FRAME"
    elif (
        handoff_frame is None and mapper == "codemasters" and bios_name == "export_v1_3"
    ):
        outcome = "BIOS_MAPPER_CHECKSUM_UNRELIABLE"
    elif handoff_frame is None:
        outcome = "NO_BIOS_HANDOFF"
    elif (frames_after_handoff or 0) < post_handoff_frames:
        outcome = "BIOS_HANDOFF_WITHOUT_FULL_POSTRUN"
    elif not visual_activity:
        outcome = "BIOS_HANDOFF_BUT_BLANK_FRAME"
    else:
        outcome = "BIOS_HANDOFF_AND_POSTRUN"

    return {
        "rom": rom_name,
        "rom_sha256": hashlib.sha256(rom_data).hexdigest(),
        "bios": bios_name,
        "mapper": mapper,
        "bios_sha256": (
            hashlib.sha256(bios_data).hexdigest() if bios_data is not None else None
        ),
        "region": region,
        "outcome": outcome,
        "frames_run": frames_run,
        "handoff_frame": handoff_frame,
        "frames_after_handoff": frames_after_handoff,
        "bios_still_enabled": machine.bios_enabled,
        "pc": machine.cpu.pc,
        "instructions": machine.total_instructions,
        "tstates": machine.total_tstates,
        "frame_number": machine.vdp.frame_number,
        "visible_height": visible_height,
        "framebuffer": frame_metrics,
        "visual_activity": visual_activity,
        "capture_path": str(capture_path.resolve()),
        "execution_error": execution_error,
    }


def _run_bios_startup(
    *,
    bios_name: str,
    bios_data: bytes,
    region: str,
    frames: int,
    capture_dir: Path,
) -> dict[str, Any]:
    """Verify that one real BIOS executes and renders with no cartridge present."""
    empty_cartridge = bytes([0xFF]) * 0x8000
    machine = SMSMachine(empty_cartridge, region=region, bios=bios_data)
    frames_run = 0
    execution_error = None
    try:
        for frame_index in range(frames):
            machine.run_frame()
            frames_run = frame_index + 1
    except Exception as exc:
        execution_error = f"{type(exc).__name__}: {exc}"

    framebuffer = bytes(machine.vdp.framebuffer)
    visible_height = int(machine.vdp.visible_height)
    frame_metrics = _frame_metrics(framebuffer)
    visual_activity = (
        frame_metrics["non_black_pixels"] > 0 and frame_metrics["unique_rgb_colors"] > 1
    )
    capture_path = capture_dir / f"bios_only__{bios_name}__{region}.ppm"
    _write_ppm(capture_path, framebuffer, visible_height)
    if execution_error is not None:
        outcome = "BIOS_STARTUP_EXECUTION_ERROR"
    elif frames_run != frames:
        outcome = "BIOS_STARTUP_INCOMPLETE"
    else:
        outcome = "BIOS_STARTUP_VISIBLE" if visual_activity else "BIOS_STARTUP_BLANK"

    return {
        "rom": "NO_CARTRIDGE",
        "rom_sha256": hashlib.sha256(empty_cartridge).hexdigest(),
        "bios": bios_name,
        "bios_sha256": hashlib.sha256(bios_data).hexdigest(),
        "mapper": None,
        "region": region,
        "outcome": outcome,
        "frames_run": frames_run,
        "handoff_frame": None,
        "frames_after_handoff": None,
        "bios_still_enabled": machine.bios_enabled,
        "pc": machine.cpu.pc,
        "instructions": machine.total_instructions,
        "tstates": machine.total_tstates,
        "frame_number": machine.vdp.frame_number,
        "visible_height": visible_height,
        "framebuffer": frame_metrics,
        "visual_activity": visual_activity,
        "capture_path": str(capture_path.resolve()),
        "execution_error": execution_error,
    }


def main() -> int:
    """Parse local inputs and execute BIOS, six-game, and direct-boot cases."""
    parser = argparse.ArgumentParser(
        description=(
            "Run each supplied SMS ROM with the three recognized BIOS revisions "
            "and without BIOS. ROM/BIOS data stays local."
        )
    )
    parser.add_argument(
        "--rom",
        action="append",
        required=True,
        help="Path to one SMS ROM or a ZIP with exactly one ROM; repeat per game.",
    )
    parser.add_argument(
        "--bios-dir", required=True, help="Directory containing the three BIOS files."
    )
    parser.add_argument(
        "--frames", type=int, default=240, help="Maximum frames per combination."
    )
    parser.add_argument(
        "--post-handoff-frames",
        type=int,
        default=60,
        help="Frames to execute after BIOS handoff before capturing the result.",
    )
    parser.add_argument(
        "--output",
        default="logs/sms_boot_matrix.json",
        help="Local JSON report path (hashes and diagnostics only).",
    )
    parser.add_argument(
        "--capture-dir",
        default="logs/sms_boot_matrix_frames",
        help="Local directory for final PPM screenshots.",
    )
    args = parser.parse_args()

    if args.frames < 1 or args.post_handoff_frames < 0:
        parser.error(
            "--frames must be positive and --post-handoff-frames non-negative."
        )

    scan = scan_sms_firmware_directory(args.bios_dir)
    expected_status = {
        SmsFirmwareRegion.JAPAN: SmsFirmwareFileStatus.VALID_JAPAN,
        SmsFirmwareRegion.EXPORT_US_EU: SmsFirmwareFileStatus.VALID_EXPORT_US_EU,
        SmsFirmwareRegion.EUROPE: SmsFirmwareFileStatus.VALID_EUROPE,
    }
    bios_by_region = {
        region: next(
            (
                item
                for item in scan.files
                if item.region is region and item.status is expected_status[region]
            ),
            None,
        )
        for region in expected_status
    }
    missing = [region.value for region, item in bios_by_region.items() if item is None]
    if missing:
        parser.error(
            "Missing recognized BIOS revisions: "
            + ", ".join(missing)
            + f" (scan status: {scan.status.value})"
        )

    bios_payloads = {
        region: item.path.read_bytes()
        for region, item in bios_by_region.items()
        if item is not None
    }
    capture_dir = Path(args.capture_dir)
    results: list[dict[str, Any]] = []
    preflight: list[dict[str, Any]] = []
    catalog = RomCatalog()

    for bios_name, bios_region, console_region in BIOS_CASES:
        results.append(
            _run_bios_startup(
                bios_name=bios_name,
                bios_data=bios_payloads[bios_region],
                region=console_region,
                frames=args.frames,
                capture_dir=capture_dir,
            )
        )

    for rom_path in args.rom:
        loaded = load_sms_rom(rom_path)
        header = _header_diagnostics(loaded.data)
        detection = _bios_preflight(loaded.data)
        direct_region = sms_bios_region_for_rom(loaded.data, catalog) or "usa"
        preflight.append(
            {
                "rom": loaded.display_name,
                "rom_sha256": loaded.sha256,
                "size": len(loaded.data),
                "header": header,
                "bios_detection": detection,
                "direct_boot_region": direct_region,
            }
        )

        for bios_name, bios_region, console_region in BIOS_CASES:
            results.append(
                _run_case(
                    rom_name=loaded.display_name,
                    rom_data=loaded.data,
                    bios_name=bios_name,
                    bios_data=bios_payloads[bios_region],
                    region=console_region,
                    frames=args.frames,
                    post_handoff_frames=args.post_handoff_frames,
                    capture_dir=capture_dir,
                    mapper=header["mapper"],
                )
            )

        results.append(
            _run_case(
                rom_name=loaded.display_name,
                rom_data=loaded.data,
                bios_name=None,
                bios_data=None,
                region=direct_region,
                frames=args.frames,
                post_handoff_frames=0,
                capture_dir=capture_dir,
                mapper=header["mapper"],
            )
        )

    report = {
        "schema_version": 1,
        "method": "native Rust SMS runtime through the installed PyO3 extension",
        "case_count": len(results),
        "rom_and_bios_bytes_included": False,
        "frames_limit": args.frames,
        "post_handoff_frames": args.post_handoff_frames,
        "bios_scan_status": scan.status.value,
        "bios_files": [
            {
                "region": region.value,
                "name": item.path.name,
                "sha256": item.sha256,
                "size": item.size,
            }
            for region, item in bios_by_region.items()
            if item is not None
        ],
        "preflight": preflight,
        "results": results,
    }

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(report, indent=2, ensure_ascii=False),
        encoding="utf-8",
    )

    for result in results:
        print(
            f"{result['outcome']:24} "
            f"{result['rom']} / {result['bios'] or 'sem BIOS'} "
            f"frames={result['frames_run']} "
            f"handoff={result['handoff_frame']} "
            f"PC={result['pc']:04X}"
        )
    print(f"JSON report: {output_path.resolve()}")
    print(f"PPM captures: {capture_dir.resolve()}")
    print("ROM/BIOS bytes were not copied to the report or repository.")

    return (
        0
        if all(
            result["outcome"]
            in {
                "BIOS_HANDOFF_AND_POSTRUN",
                "BIOS_MAPPER_CHECKSUM_UNRELIABLE",
                "BIOS_STARTUP_VISIBLE",
                "DIRECT_BOOT_FRAME_ADVANCE",
            }
            for result in results
        )
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())
