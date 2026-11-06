import sys
from pathlib import Path

from tools import project_manager


def test_cargo_fmt_manifest_is_before_rustfmt_separator() -> None:
    workspace = project_manager.Workspace(
        manifest="native/z80_core/Cargo.toml",
        root="native/z80_core",
        lockfile="native/z80_core/Cargo.lock",
    )

    command = project_manager.cargo_for(workspace, "fmt", "--all", "--", "--check")

    assert command == [
        "cargo",
        "fmt",
        "--all",
        "--manifest-path",
        str(Path(__file__).resolve().parents[2] / "native/z80_core/Cargo.toml"),
        "--",
        "--check",
    ]


def test_cargo_options_keep_manifest_as_cargo_argument() -> None:
    workspace = project_manager.Workspace(
        manifest="native/audio_output/Cargo.toml",
        root="native/audio_output",
        lockfile="native/audio_output/Cargo.lock",
    )

    command = project_manager.cargo_for(workspace, "check", "--locked")

    assert command[-2:] == [
        "--manifest-path",
        str(Path(__file__).resolve().parents[2] / "native/audio_output/Cargo.toml"),
    ]
    assert "--locked" in command


def test_lock_check_validates_metadata_without_compiling(monkeypatch) -> None:
    workspace = project_manager.Workspace(
        manifest="native/audio_output/Cargo.toml",
        root="native/audio_output",
        lockfile="native/audio_output/Cargo.lock",
    )
    commands: list[list[str]] = []

    def fake_run(command, cwd, timeout=1800, output_limit=12000, stream=False):
        commands.append(command)
        return project_manager.Result("metadata", "PASS")

    monkeypatch.setattr(project_manager, "run", fake_run)

    results = project_manager.lock_check([workspace])

    assert [result.status for result in results] == ["PASS"]
    assert len(commands) == 1
    assert commands[0][:5] == [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
    ]
    assert commands[0][-2:] == [
        "--manifest-path",
        str(Path(__file__).resolve().parents[2] / "native/audio_output/Cargo.toml"),
    ]
    assert "check" not in commands[0]


def test_lock_refresh_uses_offline_metadata_without_compiling(monkeypatch) -> None:
    workspace = project_manager.Workspace(
        manifest="native/audio_output/Cargo.toml",
        root="native/audio_output",
        lockfile="native/audio_output/Cargo.lock",
        lockfile_exists=True,
    )
    commands: list[list[str]] = []

    def fake_run(command, cwd, timeout=1800, output_limit=12000, stream=False):
        commands.append(command)
        return project_manager.Result("metadata", "PASS")

    monkeypatch.setattr(project_manager, "run", fake_run)

    results = project_manager.lock_refresh([workspace])

    assert [result.status for result in results] == ["PASS", "PASS"]
    assert len(commands) == 2
    assert commands[0][:5] == [
        "cargo",
        "metadata",
        "--offline",
        "--format-version",
        "1",
    ]
    assert commands[1][:5] == [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
    ]
    assert all("check" not in command for command in commands)


def test_inventory_reports_not_run_without_failing_audit(capsys) -> None:
    result = project_manager.Result(
        "unassigned manifest", "NOT RUN", output="outside active workspace"
    )

    assert project_manager.print_results([result], strict=False) == 0
    assert "NOT RUN" in capsys.readouterr().out


def test_validation_does_not_treat_blocked_as_pass(capsys) -> None:
    result = project_manager.Result("required lockfile", "BLOCKED", output="missing")

    assert project_manager.print_results([result]) == 1
    assert "BLOCKED" in capsys.readouterr().out


def test_discover_workspaces_returns_three_values_without_cargo(monkeypatch) -> None:
    monkeypatch.setattr(project_manager.shutil, "which", lambda _name: None)

    workspaces, errors, unassigned = project_manager.discover_workspaces()

    assert workspaces == []
    assert errors == ["cargo executable not found on PATH"]
    assert unassigned == []


def test_tool_versions_uses_the_current_python_interpreter(monkeypatch) -> None:
    commands: list[list[str]] = []

    def fake_run(command, cwd, timeout=1800, output_limit=12000):
        commands.append(command)
        return project_manager.Result("test", "PASS", output="Python test\n")

    monkeypatch.setattr(project_manager, "run", fake_run)
    monkeypatch.setattr(project_manager.shutil, "which", lambda name: f"/bin/{name}")

    project_manager.tool_versions()

    assert [sys.executable, "--version"] in commands
    assert [sys.executable, "-m", "ruff", "--version"] in commands


def test_streaming_run_forwards_live_output_and_keeps_a_result_tail(capsys) -> None:
    result = project_manager.run(
        [sys.executable, "-c", "print('pytest progress visible')"],
        project_manager.ROOT,
        stream=True,
    )

    assert result.status == "PASS"
    assert "pytest progress visible" in result.output
    assert "pytest progress visible" in capsys.readouterr().out


def test_streaming_run_terminates_a_command_after_its_timeout() -> None:
    result = project_manager.run(
        [sys.executable, "-c", "import time; time.sleep(5)"],
        project_manager.ROOT,
        timeout=0.1,
        stream=True,
    )

    assert result.status == "FAIL"
    assert result.returncode == 124
    assert "Timed out after 0.1 seconds" in result.output
