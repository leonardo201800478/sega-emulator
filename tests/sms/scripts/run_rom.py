#!/usr/bin/env python3
"""Hash a diagnostic ROM and invoke the Rust full-machine SMS runner."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    """Parse the ROM, provenance, hardware profile, and execution limits."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", required=True, type=Path, help="diagnostic .sms ROM image")
    parser.add_argument("--suite-id", required=True, help="suite identifier from manifest.toml")
    parser.add_argument("--suite-revision", default="unspecified", help="pinned upstream commit/tag")
    parser.add_argument("--region", choices=("usa", "brazil", "europe", "japan"), default="usa")
    parser.add_argument("--frames", type=int, default=120)
    parser.add_argument("--max-instructions-per-frame", type=int, default=100_000)
    parser.add_argument("--debug", action="store_true", help="build/run without Cargo --release")
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    """Return the SHA-256 digest of a ROM using bounded-memory streaming reads."""
    digest = hashlib.sha256()
    with path.open("rb") as rom_file:
        for chunk in iter(lambda: rom_file.read(1024 * 1024), b""):
            digest.update(chunk)
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
    """Validate inputs, compute provenance, and launch the Rust machine runner."""
    args = parse_args()
    rom_path = args.rom.resolve()
    if not rom_path.is_file():
        print(f"ROM file does not exist: {rom_path}", file=sys.stderr)
        return 64
    if args.frames <= 0 or args.max_instructions_per_frame <= 0:
        print("frame and instruction limits must be greater than zero", file=sys.stderr)
        return 64

    repository_root = Path(__file__).resolve().parents[3]
    command = ["cargo", "run"]
    if not args.debug:
        command.append("--release")
    command.extend(
        [
            "-p",
            "sega-sms-core",
            "--example",
            "sms_rom_runner",
            "--",
            "--rom",
            str(rom_path),
            "--rom-sha256",
            sha256_file(rom_path),
            "--emulator-sha",
            git_revision(repository_root),
            "--suite-id",
            args.suite_id,
            "--suite-revision",
            args.suite_revision,
            "--region",
            args.region,
            "--frames",
            str(args.frames),
            "--max-instructions-per-frame",
            str(args.max_instructions_per_frame),
        ]
    )
    return subprocess.run(command, cwd=repository_root, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
