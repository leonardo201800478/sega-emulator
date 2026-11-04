#!/usr/bin/env python3
"""Discover and validate every Cargo workspace in the Sega Emulator repository."""
from __future__ import annotations
import argparse, json, os, shutil, subprocess, sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PRUNE_DIRS = {".git", ".venv", "venv", "target", "node_modules", "build", "dist",
              "__pycache__", ".pytest_cache", ".ruff_cache", "legacy"}

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

def run(command: list[str], cwd: Path, timeout: int = 1800) -> Result:
    try:
        proc = subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=timeout, check=False)
        return Result(" ".join(command), "PASS" if proc.returncode == 0 else "FAIL",
                      command, proc.returncode, proc.stdout[-12000:])
    except FileNotFoundError as exc:
        return Result(" ".join(command), "BLOCKED", command, None, str(exc))
    except subprocess.TimeoutExpired as exc:
        output = exc.stdout or ""
        if isinstance(output, bytes):
            output = output.decode(errors="replace")
        return Result(" ".join(command), "FAIL", command, 124,
                      str(output)[-12000:] + "\nTimed out")

def discover_workspaces() -> tuple[list[Workspace], list[str]]:
    cargo = shutil.which("cargo")
    if not cargo:
        return [], ["cargo executable not found on PATH"]
    manifests: list[Path] = []
    for current, dirs, files in os.walk(ROOT):
        dirs[:] = sorted(d for d in dirs if d not in PRUNE_DIRS)
        if "Cargo.toml" in files:
            manifests.append(Path(current) / "Cargo.toml")
    roots: dict[Path, Workspace] = {}
    errors: list[str] = []
    for manifest in sorted(manifests):
        located = run([cargo, "locate-project", "--workspace", "--message-format", "plain",
                       "--manifest-path", str(manifest)], ROOT, 60)
        if located.status != "PASS":
            errors.append(f"{manifest.relative_to(ROOT)}: {located.output.strip()}")
            continue
        workspace_manifest = Path(located.output.strip().splitlines()[-1]).resolve()
        if workspace_manifest in roots:
            continue
        metadata = run([cargo, "metadata", "--no-deps", "--format-version", "1",
                        "--manifest-path", str(workspace_manifest)],
                       workspace_manifest.parent, 120)
        if metadata.status != "PASS":
            errors.append(f"{workspace_manifest}: cargo metadata failed: {metadata.output}")
            continue
        try:
            data: dict[str, Any] = json.loads(metadata.output)
            workspace_root = Path(data["workspace_root"]).resolve()
            lockfile = workspace_root / "Cargo.lock"
            roots[workspace_manifest] = Workspace(
                str(workspace_manifest.relative_to(ROOT)),
                str(workspace_root.relative_to(ROOT)),
                str(lockfile.relative_to(ROOT)),
                sorted(p["name"] for p in data.get("packages", [])),
                workspace_root == ROOT, lockfile.is_file())
        except (KeyError, ValueError, OSError) as exc:
            errors.append(f"{workspace_manifest}: invalid metadata: {exc}")
    return sorted(roots.values(), key=lambda w: w.root), errors

def tool_versions() -> dict[str, str]:
    versions = {}
    for executable, args in (("git", ["--version"]), ("rustc", ["--version"]),
        ("cargo", ["--version"]), ("python", ["--version"]), ("pip", ["--version"]),
        ("ruff", ["--version"]), ("pytest", ["--version"]), ("maturin", ["--version"])):
        path = shutil.which(executable)
        if not path:
            versions[executable] = "NOT FOUND"
            continue
        result = run([path, *args], ROOT, 30)
        versions[executable] = result.output.strip().splitlines()[0] if result.output else result.status
    return versions

def audit() -> tuple[dict[str, Any], list[Result]]:
    workspaces, errors = discover_workspaces()
    report = {"repository": str(ROOT), "tools": tool_versions(),
        "workspaces": [asdict(w) for w in workspaces], "discovery_errors": errors,
        "python_project": (ROOT / "pyproject.toml").is_file(),
        "rust_toolchain_file": next((p for p in ("rust-toolchain.toml", "rust-toolchain")
                                     if (ROOT / p).is_file()), None)}
    results = [Result("workspace discovery",
        "PASS" if workspaces and not errors else "FAIL",
        output=f"{len(workspaces)} workspace(s); {len(errors)} discovery error(s)")]
    for ws in workspaces:
        results.append(Result(f"lockfile exists: {ws.lockfile}",
            "PASS" if ws.lockfile_exists else "FAIL",
            output="present" if ws.lockfile_exists else "missing"))
    return report, results

def cargo_for(ws: Workspace, *args: str) -> list[str]:
    return ["cargo", *args, "--manifest-path", str(ROOT / ws.manifest)]

def lock_check(workspaces: list[Workspace]) -> list[Result]:
    results = []
    for ws in workspaces:
        if not ws.lockfile_exists:
            results.append(Result(f"locked check: {ws.root}", "FAIL",
                                  output=f"Missing {ws.lockfile}"))
        else:
            results.append(run(cargo_for(ws, "check", "--locked"), ROOT))
    return results

def lock_refresh(workspaces: list[Workspace]) -> list[Result]:
    results = []
    for ws in workspaces:
        if not ws.lockfile_exists:
            results.append(Result(f"offline lock refresh: {ws.root}", "BLOCKED",
                                  output=f"Missing {ws.lockfile}; review workspace policy"))
            continue
        refresh = run(cargo_for(ws, "check", "--offline"), ROOT)
        results.append(refresh)
        if refresh.status == "PASS":
            results.append(run(cargo_for(ws, "check", "--locked"), ROOT))
    return results

def validate(workspaces: list[Workspace], include_python: bool) -> list[Result]:
    results = []
    for ws in workspaces:
        results.append(run(cargo_for(ws, "fmt", "--all", "--", "--check"), ROOT, 300))
        results.append(run(cargo_for(ws, "clippy", "--workspace", "--all-targets",
                                     "--locked", "--", "-D", "warnings"), ROOT))
        results.append(run(cargo_for(ws, "test", "--workspace", "--locked"), ROOT))
    if include_python and (ROOT / "pyproject.toml").is_file():
        py = sys.executable
        results.extend([run([py, "-m", "ruff", "format", "--check", "."], ROOT),
                        run([py, "-m", "ruff", "check", "."], ROOT),
                        run([py, "-m", "pytest"], ROOT)])
    return results

def print_results(results: list[Result]) -> int:
    for item in results:
        print(f"[{item.status:7}] {item.name}")
        if item.status != "PASS" and item.output:
            print(item.output.rstrip())
    counts = {s: sum(r.status == s for r in results)
              for s in ("PASS", "FAIL", "BLOCKED", "NOT RUN")}
    print("\nSummary: " + " | ".join(f"{k}={v}" for k, v in counts.items()))
    return 0 if counts["FAIL"] == 0 and counts["BLOCKED"] == 0 else 1

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("audit", help="inventory tools, Cargo workspaces and lockfiles")
    locks = sub.add_parser("locks", help="check or explicitly refresh Cargo lockfiles")
    locks.add_argument("--refresh-offline", action="store_true")
    val = sub.add_parser("validate", help="run Rust and Python quality gates")
    val.add_argument("--rust-only", action="store_true")
    val.add_argument("--json", dest="json_path", type=Path)
    args = parser.parse_args()
    report, results = audit()
    if args.command == "audit":
        print(json.dumps(report, indent=2, ensure_ascii=False))
        return print_results(results)
    workspaces = [Workspace(**w) for w in report["workspaces"]]
    if args.command == "locks":
        results += lock_refresh(workspaces) if args.refresh_offline else lock_check(workspaces)
    else:
        results += validate(workspaces, not args.rust_only)
    if getattr(args, "json_path", None):
        args.json_path.parent.mkdir(parents=True, exist_ok=True)
        args.json_path.write_text(json.dumps({"audit": report,
            "results": [asdict(r) for r in results]}, indent=2, ensure_ascii=False),
            encoding="utf-8")
    return print_results(results)

if __name__ == "__main__":
    raise SystemExit(main())
