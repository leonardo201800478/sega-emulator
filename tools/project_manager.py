#!/usr/bin/env python3
"""Discover and validate every Cargo workspace in the Sega Emulator repository."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PRUNE_DIRS = {
    ".git",
    ".venv",
    "venv",
    "target",
    "node_modules",
    "build",
    "dist",
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    "legacy",
}


@dataclass
class Workspace:
    manifest: str
    root: str
    lockfile: str
    packages: list[str] = field(default_factory=list)
    is_root: bool = False
    lockfile_exists: bool = False


@dataclass
class Result:
    name: str
    status: str
    command: list[str] = field(default_factory=list)
    returncode: int | None = None
    output: str = ""


def run(
    command: list[str],
    cwd: Path,
    timeout: int = 1800,
    output_limit: int | None = 12000,
) -> Result:
    """Run one command and preserve its output and explicit result classification."""
    try:
        proc = subprocess.run(
            command,
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
            check=False,
        )
        output = proc.stdout
        if output_limit is not None:
            output = output[-output_limit:]
        return Result(
            " ".join(command),
            "PASS" if proc.returncode == 0 else "FAIL",
            command,
            proc.returncode,
            output,
        )
    except FileNotFoundError as exc:
        return Result(" ".join(command), "BLOCKED", command, None, str(exc))
    except subprocess.TimeoutExpired as exc:
        output = exc.stdout or ""
        if isinstance(output, bytes):
            output = output.decode(errors="replace")
        return Result(
            " ".join(command),
            "FAIL",
            command,
            124,
            str(output)[-12000:] + "\nTimed out",
        )


def discover_workspaces() -> tuple[list[Workspace], list[str], list[str]]:
    """Discover unique Cargo workspace roots without guessing directory layout."""
    cargo = shutil.which("cargo")
    if not cargo:
        return [], ["cargo executable not found on PATH"]

    manifests: list[Path] = []
    for current, dirs, files in os.walk(ROOT):
        dirs[:] = sorted(directory for directory in dirs if directory not in PRUNE_DIRS)
        if "Cargo.toml" in files:
            manifests.append(Path(current) / "Cargo.toml")

    roots: dict[Path, Workspace] = {}
    errors: list[str] = []
    unassigned: list[str] = []
    for manifest in sorted(manifests):
        located = run(
            [
                cargo,
                "locate-project",
                "--workspace",
                "--message-format",
                "plain",
                "--manifest-path",
                str(manifest),
            ],
            ROOT,
            60,
        )
        if located.status != "PASS":
            # Excluded standalone packages may not belong to an ancestor workspace.
            standalone = run(
                [
                    cargo,
                    "locate-project",
                    "--message-format",
                    "plain",
                    "--manifest-path",
                    str(manifest),
                ],
                ROOT,
                60,
            )
            if standalone.status != "PASS":
                errors.append(
                    f"{manifest.relative_to(ROOT)}: "
                    f"workspace lookup: {located.output.strip()}; "
                    f"standalone lookup: {standalone.output.strip()}"
                )
                continue
            workspace_manifest = Path(
                standalone.output.strip().splitlines()[-1]
            ).resolve()
        else:
            workspace_manifest = Path(
                located.output.strip().splitlines()[-1]
            ).resolve()
        if workspace_manifest in roots:
            continue

        metadata = run(
            [
                cargo,
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
                str(workspace_manifest),
            ],
            workspace_manifest.parent,
            120,
            output_limit=None,
        )
        if metadata.status != "PASS":
            if "current package believes it's in a workspace when it's not" in metadata.output:
                unassigned.append(str(manifest.relative_to(ROOT)))
            else:
                errors.append(
                    f"{workspace_manifest}: cargo metadata failed: {metadata.output}"
                )
            continue

        try:
            data: dict[str, Any] = json.loads(metadata.output)
            workspace_root = Path(data["workspace_root"]).resolve()
            lockfile = workspace_root / "Cargo.lock"
            roots[workspace_manifest] = Workspace(
                manifest=str(workspace_manifest.relative_to(ROOT)),
                root=str(workspace_root.relative_to(ROOT)),
                lockfile=str(lockfile.relative_to(ROOT)),
                packages=sorted(
                    package["name"] for package in data.get("packages", [])
                ),
                is_root=workspace_root == ROOT,
                lockfile_exists=lockfile.is_file(),
            )
        except (KeyError, ValueError, OSError) as exc:
            errors.append(f"{workspace_manifest}: invalid metadata: {exc}")

    return sorted(roots.values(), key=lambda workspace: workspace.root), errors, unassigned


def tool_versions() -> dict[str, str]:
    """Report installed tool versions; never install or upgrade tools."""
    versions = {}
    tools = (
        ("git", ["--version"]),
        ("rustc", ["--version"]),
        ("cargo", ["--version"]),
        ("python", ["--version"]),
        ("pip", ["--version"]),
        ("ruff", ["--version"]),
        ("pytest", ["--version"]),
        ("maturin", ["--version"]),
    )
    for executable, args in tools:
        path = shutil.which(executable)
        if not path:
            versions[executable] = "NOT FOUND"
            continue
        result = run([path, *args], ROOT, 30)
        versions[executable] = (
            result.output.strip().splitlines()[0] if result.output else result.status
        )
    return versions


def audit() -> tuple[dict[str, Any], list[Result]]:
    """Build a read-only inventory and report missing lockfiles for review."""
    workspaces, errors, unassigned = discover_workspaces()
    report = {
        "repository": str(ROOT),
        "tools": tool_versions(),
        "workspaces": [asdict(workspace) for workspace in workspaces],
        "discovery_errors": errors,
        "unassigned_manifests": unassigned,
        "python_project": (ROOT / "pyproject.toml").is_file(),
        "rust_toolchain_file": next(
            (
                path
                for path in ("rust-toolchain.toml", "rust-toolchain")
                if (ROOT / path).is_file()
            ),
            None,
        ),
    }
    results = [
        Result(
            "workspace discovery",
            "PASS" if workspaces and not errors else "FAIL",
            output=f"{len(workspaces)} workspace(s); {len(errors)} discovery error(s)",
        )
    ]
    for workspace in workspaces:
        results.append(
            Result(
                f"lockfile exists: {workspace.lockfile}",
                "PASS" if workspace.lockfile_exists else "NOT RUN",
                output=(
                    "present"
                    if workspace.lockfile_exists
                    else "not present; cargo --locked determines whether one is required"
                ),
            )
        )
    return report, results


def cargo_for(workspace: Workspace, *args: str) -> list[str]:
    """Build a Cargo command, keeping rustfmt arguments after the separator."""
    manifest = ["--manifest-path", str(ROOT / workspace.manifest)]
    if "--" in args:
        separator = args.index("--")
        return [
            "cargo",
            *args[:separator],
            *manifest,
            "--",
            *args[separator + 1 :],
        ]
    return ["cargo", *args, *manifest]


def lock_check(workspaces: list[Workspace]) -> list[Result]:
    """Verify each existing lockfile without allowing Cargo to modify it."""
    results = []
    for workspace in workspaces:
        results.append(run(cargo_for(workspace, "check", "--locked"), ROOT))
    return results


def lock_refresh(workspaces: list[Workspace]) -> list[Result]:
    """Refresh existing lockfiles from cache, then verify the result with --locked."""
    results = []
    for workspace in workspaces:
        if not workspace.lockfile_exists:
            results.append(
                Result(
                    f"offline lock refresh: {workspace.root}",
                    "BLOCKED",
                    output=f"Missing {workspace.lockfile}; review workspace policy",
                )
            )
            continue

        refresh = run(cargo_for(workspace, "check", "--offline"), ROOT)
        results.append(refresh)
        if refresh.status == "PASS":
            results.append(run(cargo_for(workspace, "check", "--locked"), ROOT))
    return results


def validate(workspaces: list[Workspace], include_python: bool) -> list[Result]:
    """Run the Rust gates once per workspace and Python gates once per repository."""
    results = []
    for workspace in workspaces:
        results.append(
            run(cargo_for(workspace, "fmt", "--all", "--", "--check"), ROOT, 300)
        )
        results.append(
            run(
                cargo_for(
                    workspace,
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ),
                ROOT,
            )
        )
        results.append(
            run(cargo_for(workspace, "test", "--workspace", "--locked"), ROOT)
        )

    if include_python and (ROOT / "pyproject.toml").is_file():
        python = sys.executable
        results.extend(
            [
                run([python, "-m", "ruff", "format", "--check", "."], ROOT),
                run([python, "-m", "ruff", "check", "."], ROOT),
                run([python, "-m", "pytest"], ROOT),
            ]
        )
    return results


def print_results(results: list[Result], strict: bool = True) -> int:
    """Print a status summary; strict mode treats BLOCKED as a failing gate."""
    for result in results:
        print(f"[{result.status:7}] {result.name}")
        if result.status != "PASS" and result.output:
            print(result.output.rstrip())

    counts = {
        status: sum(result.status == status for result in results)
        for status in ("PASS", "FAIL", "BLOCKED", "NOT RUN")
    }
    summary = " | ".join(f"{name}={count}" for name, count in counts.items())
    print(f"\nSummary: {summary}")
    if counts["FAIL"] > 0:
        return 1
    return 1 if strict and counts["BLOCKED"] > 0 else 0


def main() -> int:
    """Parse the CLI and execute the selected read-only or explicit operation."""
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser(
        "audit", help="inventory tools, Cargo workspaces and lockfiles"
    )

    locks = subparsers.add_parser(
        "locks", help="check or explicitly refresh Cargo lockfiles"
    )
    locks.add_argument("--refresh-offline", action="store_true")

    validation = subparsers.add_parser(
        "validate", help="run Rust and Python quality gates"
    )
    validation.add_argument("--rust-only", action="store_true")
    validation.add_argument("--json", dest="json_path", type=Path)
    args = parser.parse_args()

    report, results = audit()
    if args.command == "audit":
        print(json.dumps(report, indent=2, ensure_ascii=False))
        return print_results(results, strict=False)

    workspaces = [Workspace(**workspace) for workspace in report["workspaces"]]
    if args.command == "locks":
        results += (
            lock_refresh(workspaces) if args.refresh_offline else lock_check(workspaces)
        )
    else:
        results += validate(workspaces, not args.rust_only)

    if getattr(args, "json_path", None):
        args.json_path.parent.mkdir(parents=True, exist_ok=True)
        args.json_path.write_text(
            json.dumps(
                {"audit": report, "results": [asdict(result) for result in results]},
                indent=2,
                ensure_ascii=False,
            ),
            encoding="utf-8",
        )
    return print_results(results)


if __name__ == "__main__":
    raise SystemExit(main())
