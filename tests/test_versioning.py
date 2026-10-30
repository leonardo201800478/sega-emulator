"""Regression tests for synchronized project version metadata."""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_project_versions_are_synchronized() -> None:
    """Require all Python, Cargo and lockfile versions to match VERSION."""
    result = subprocess.run(
        [sys.executable, str(ROOT / "tools" / "release" / "check_versions.py")],
        cwd=ROOT,
        capture_output=True,
        check=False,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "Version synchronization PASS: 0.2.0-dev.1" in result.stdout
