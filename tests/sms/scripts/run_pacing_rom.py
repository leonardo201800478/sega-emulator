#!/usr/bin/env python3
"""Snapshot and run an SMS ROM through the production worker pacing loop."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
import tempfile
from pathlib import Path


def parse_args() -> argparse.Namespace:
    """Parse ROM provenance, region, capture duration, and optional controller pulses."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", required=True, type=Path, help="diagnostic .sms ROM image")
    parser.add_argument("--suite-id", required=True, help="suite identifier from tests/sms/manifest.toml")
    parser.add_argument("--suite-revision", default="unspecified", help="pinned upstream commit/tag")
    parser.add_argument("--region", choices=("usa", "brazil", "europe", "japan"), default="usa")
    parser.add_argument("--duration-seconds", type=int, default=300, help="measured capture duration")
    parser.add_argument("--warmup-seconds", type=int, default=10, help="discarded warm-up before reset")
    parser.add_argument("--sample-interval-ms", type=int, default=1000)
    parser.add_argument(
        "--button1-pulse-frames",
        default="",
        help="comma-separated completed-frame numbers at which to pulse controller 1 button 1",
    )
    parser.add_argument("--debug", action="store_true", help="build/run without Cargo --release")
    return parser.parse_args()


def snapshot_and_hash(source: Path, destination: Path) -> str:
    """Copy one ROM snapshot while hashing exactly the bytes written to it."""
    digest = hashlib.sha256()
    with source.open("rb") as source_file, destination.open("xb") as snapshot_file:
        for chunk in iter(lambda: source_file.read(1024 * 1024), b""):
            snapshot_file.write(chunk)
            digest.update(chunk)
    if destination.stat().st_size == 0:
        raise ValueError("ROM image is empty")
    return digest.hexdigest()


def git_revision(repository_root: Path) -> str:
    """Return the checked-out Git commit, or 'unknown' outside a Git worktree."""
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository_root,
        check=False,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() if result.returncode == 0 else "unknown"


def main() -> int:
    """Snapshot the ROM and invoke the Rust production-worker pacing harness."""
    args = parse_args()
    rom_path = args.rom.resolve()
    if not rom_path.is_file():
        print(f"ROM file does not exist: {rom_path}", file=sys.stderr)
        return 64
    if args.duration_seconds <= 0 or args.duration_seconds > 3600:
        print("--duration-seconds must be between 1 and 3600", file=sys.stderr)
        return 64
    if args.warmup_seconds < 0 or args.warmup_seconds > 300:
        print("--warmup-seconds must be between 0 and 300", file=sys.stderr)
        return 64
    if args.sample_interval_ms < 250 or args.sample_interval_ms > 10_000:
        print("--sample-interval-ms must be between 250 and 10000", file=sys.stderr)
        return 64

    repository_root = Path(__file__).resolve().parents[3]
    with tempfile.TemporaryDirectory(prefix="sega-sms-pacing-") as temporary_directory:
        snapshot_path = Path(temporary_directory) / f"input{rom_path.suffix or '.sms'}"
        try:
            rom_sha256 = snapshot_and_hash(rom_path, snapshot_path)
        except (OSError, ValueError) as error:
            print(f"cannot snapshot ROM: {error}", file=sys.stderr)
            return 64

        command = ["cargo", "run"]
        if not args.debug:
            command.append("--release")
        command.extend(
            [
                "-p",
                "sega-sms-runtime",
                "--example",
                "sms_pacing_rom_runner",
                "--",
                "--rom",
                str(snapshot_path),
                "--rom-sha256",
                rom_sha256,
                "--emulator-sha",
                git_revision(repository_root),
                "--suite-id",
                args.suite_id,
                "--suite-revision",
                args.suite_revision,
                "--region",
                args.region,
                "--duration-seconds",
                str(args.duration_seconds),
                "--warmup-seconds",
                str(args.warmup_seconds),
                "--sample-interval-ms",
                str(args.sample_interval_ms),
                "--button1-pulse-frames",
                args.button1_pulse_frames,
            ]
        )
        return subprocess.run(command, cwd=repository_root, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
