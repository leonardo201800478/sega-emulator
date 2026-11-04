from pathlib import Path

from tools.project_manager import Result, Workspace, cargo_for, print_results


def test_cargo_fmt_manifest_is_before_rustfmt_separator() -> None:
    workspace = Workspace(
        manifest="native/z80_core/Cargo.toml",
        root="native/z80_core",
        lockfile="native/z80_core/Cargo.lock",
    )

    command = cargo_for(workspace, "fmt", "--all", "--", "--check")

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
    workspace = Workspace(
        manifest="native/audio_output/Cargo.toml",
        root="native/audio_output",
        lockfile="native/audio_output/Cargo.lock",
    )

    command = cargo_for(workspace, "check", "--locked")

    assert command[-2:] == [
        "--manifest-path",
        str(Path(__file__).resolve().parents[2] / "native/audio_output/Cargo.toml"),
    ]
    assert "--locked" in command


def test_inventory_can_report_optional_lockfile_as_blocked(capsys) -> None:
    result = Result("optional lockfile", "BLOCKED", output="manual policy review")

    assert print_results([result], strict=False) == 0
    assert "BLOCKED" in capsys.readouterr().out


def test_validation_does_not_treat_blocked_as_pass(capsys) -> None:
    result = Result("required lockfile", "BLOCKED", output="missing")

    assert print_results([result]) == 1
    assert "BLOCKED" in capsys.readouterr().out
