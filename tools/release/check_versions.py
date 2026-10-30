"""Validate that Python, Rust and lockfile versions match VERSION."""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION_PATTERN = re.compile(
    r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\."
    r"(?P<patch>0|[1-9]\d*)(?:-dev\.(?P<dev>[1-9]\d*))?$"
)


def pep440_version(version: str) -> str:
    """Convert the project's SemVer development form into its PEP 440 form."""
    match = VERSION_PATTERN.fullmatch(version)
    if match is None:
        raise ValueError(f"Unsupported VERSION value: {version!r}")
    base = (
        f"{match.group('major')}.{match.group('minor')}."
        f"{match.group('patch')}"
    )
    dev = match.group("dev")
    return f"{base}.dev{dev}" if dev is not None else base


def read_toml(path: Path) -> dict:
    """Read a TOML file and return its parsed document."""
    with path.open("rb") as stream:
        return tomllib.load(stream)


def validate_versions() -> list[str]:
    """Return every version mismatch found in package manifests and lockfiles."""
    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    pep440 = pep440_version(version)
    errors: list[str] = []

    python_projects = (
        ROOT / "pyproject.toml",
        ROOT / "native" / "z80_core" / "pyproject.toml",
    )
    for path in python_projects:
        actual = read_toml(path).get("project", {}).get("version")
        if actual != pep440:
            errors.append(f"{path.relative_to(ROOT)}: expected {pep440}, got {actual}")

    init_path = ROOT / "src" / "sega_emulator" / "__init__.py"
    init_text = init_path.read_text(encoding="utf-8")
    match = re.search(r'^__version__ = "([^"]+)"$', init_text, re.MULTILINE)
    actual_python_version = match.group(1) if match else None
    if actual_python_version != pep440:
        errors.append(
            f"{init_path.relative_to(ROOT)}: expected {pep440}, "
            f"got {actual_python_version}"
        )

    cargo_versions: dict[str, str] = {}
    for path in sorted((ROOT / "native").rglob("Cargo.toml")):
        package = read_toml(path).get("package", {})
        name = package.get("name")
        actual = package.get("version")
        if not name:
            errors.append(f"{path.relative_to(ROOT)}: missing [package].name")
        elif actual != version:
            errors.append(
                f"{path.relative_to(ROOT)}: expected {version}, got {actual}"
            )
        else:
            cargo_versions[name] = version

    lock_paths = [ROOT / "Cargo.lock", *sorted((ROOT / "native").rglob("Cargo.lock"))]
    for path in lock_paths:
        if not path.exists():
            continue
        packages = read_toml(path).get("package", [])
        for package in packages:
            name = package.get("name")
            if name in cargo_versions and package.get("version") != version:
                errors.append(
                    f"{path.relative_to(ROOT)}: {name} expected {version}, "
                    f"got {package.get('version')}"
                )

    return errors


def main() -> int:
    """Print version consistency and return a process status code."""
    try:
        errors = validate_versions()
    except (OSError, ValueError, tomllib.TOMLDecodeError) as exc:
        print(f"Version validation failed: {exc}", file=sys.stderr)
        return 1
    if errors:
        print("Version validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    print(f"Version synchronization PASS: {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
