#!/usr/bin/env python3
"""Build and run Patrik Rak's pinned Z80 suite against the SMS CPU core."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
Z80TEST_REPO = "https://github.com/raxoft/z80test.git"
Z80TEST_SHA = "c490c0ca94f1021509f8bd656b413861bd7cfb41"
SJASM_REPO = "https://github.com/samsaga2/sjasm.git"
SJASM_SHA = "d51f5504b24ea9273050ac5fa2896c8970da0b4b"
VARIANTS = ("z80full", "z80flags", "z80doc", "z80docflags", "z80ccf", "z80memptr")


def run(command: list[str], cwd: Path | None = None) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, check=True)


def checkout(repository: str, commit: str, destination: Path) -> None:
    run(["git", "clone", "--quiet", repository, str(destination)])
    run(["git", "checkout", "--quiet", "--detach", commit], cwd=destination)


def patch_print_module(path: Path) -> None:
    # Adapted from upstream print.asm (MIT). Keep print/printhl because upstream
    # CALLs embed NUL-terminated strings in-line; replace display-only routines.
    path.write_text(
        """; Headless CPU test shim based on Patrik Rak's MIT-licensed z80test print.asm.
; Upstream: https://github.com/raxoft/z80test/blob/c490c0ca94f1021509f8bd656b413861bd7cfb41/src/print.asm
; Copyright (c) 2012-2023 Patrik Rak.

printinit:  ret

print:      ex      (sp),hl
            call    printhl
            ex      (sp),hl
            ret

printhl:
.loop       ld      a,(hl)
            inc     hl
            or      a
            ret     z
            call    printchr
            jr      .loop

printdeca:  ret
printcrc:   ret
printhexs:  ret
printhexa:  ret
printchr:   ret
""",
        encoding="utf-8",
    )


def find_assembler(build: Path) -> Path:
    for candidate in (
        build / "sjasm",
        build / "sjasm.exe",
        build / "Release" / "sjasm",
        build / "Release" / "sjasm.exe",
    ):
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(f"sjasm executable not found under {build}")


def run_image(image: Path, max_steps: int) -> None:
    run(
        [
            "cargo",
            "run",
            "--locked",
            "--release",
            "-p",
            "sega-sms-core",
            "--bin",
            "z80test_runner",
            "--",
            str(image.resolve()),
            "--max-steps",
            str(max_steps),
        ],
        cwd=ROOT,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", choices=VARIANTS, default="z80full")
    parser.add_argument("--max-steps", type=int, default=250_000_000)
    parser.add_argument(
        "--work-dir", type=Path, help="preserve fetched sources and build output"
    )
    parser.add_argument(
        "--image", type=Path, help="run an existing raw image assembled at ORG 0x8000"
    )
    args = parser.parse_args()
    if args.max_steps <= 0:
        parser.error("--max-steps must be positive")

    if args.image:
        run_image(args.image, args.max_steps)
        return 0

    if args.work_dir:
        work = args.work_dir.resolve()
        work.mkdir(parents=True, exist_ok=True)
        cleanup = False
    else:
        work = Path(tempfile.mkdtemp(prefix="sega-z80test-"))
        cleanup = True

    try:
        suite = work / "z80test"
        assembler_source = work / "sjasm"
        assembler_build = work / "sjasm-build"
        checkout(Z80TEST_REPO, Z80TEST_SHA, suite)
        checkout(SJASM_REPO, SJASM_SHA, assembler_source)
        run(["cmake", "-S", str(assembler_source), "-B", str(assembler_build)])
        run(
            [
                "cmake",
                "--build",
                str(assembler_build),
                "--config",
                "Release",
                "--parallel",
            ]
        )
        assembler = find_assembler(assembler_build)

        source_dir = suite / "src"
        patch_print_module(source_dir / "print.asm")
        run([str(assembler), f"{args.variant}.asm"], cwd=source_dir)
        image = source_dir / f"{args.variant}.out"
        if not image.is_file():
            raise FileNotFoundError(f"assembler did not produce {image}")
        run_image(image, args.max_steps)
        return 0
    finally:
        if cleanup:
            shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"z80test harness failed: {error}", file=sys.stderr)
        sys.exit(1)
