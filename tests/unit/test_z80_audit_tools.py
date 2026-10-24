"""Testes unitários do runner da auditoria diferencial Z80."""

import importlib.util
import json
from pathlib import Path

_runner_path = Path(__file__).resolve().parents[2] / "run_singlestep_full.py"
_spec = importlib.util.spec_from_file_location("run_singlestep_full", _runner_path)
assert _spec is not None and _spec.loader is not None
_runner = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_runner)

audit_file = _runner.audit_file
category = _runner.category
family = _runner.family


def test_opcode_family_classification() -> None:
    assert family("00") == "base"
    assert family("cb 40") == "cb"
    assert family("dd cb 00") == "ddcb"
    assert family("fd cb 7f") == "fdcb"


def test_mismatch_category_classification() -> None:
    assert category("RAM[1234h]: esperado 00h") == "ram"
    assert category("ports: esperado [], obtido []") == "ports"
    assert category("tstates: esperado 4, obtido 5") == "tstates"
    assert category("erro ao executar vetor: IndexError") == "exception"


def test_audit_file_compares_aggregated_tstates(tmp_path: Path) -> None:
    vector = {
        "name": "NOP smoke test",
        "initial": {"pc": 0, "ram": [[0, 0]]},
        "final": {"pc": 1},
        "cycles": [
            [0, 0, "m1"],
            [0, 0, "mr"],
            [0, 0, "mr"],
            [0, 0, "mr"],
        ],
    }
    path = tmp_path / "00.json"
    path.write_text(json.dumps([vector]), encoding="utf-8")

    result = audit_file(str(path), check_cycles=True)

    assert result["total"] == 1
    assert result["status"] == {"passed": 1}
    assert result["categories"] == {}
    assert result["tstate_pairs"] == {}
