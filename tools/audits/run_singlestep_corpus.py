"""Execute um subconjunto reproduzível do corpus externo SingleStepTests/Z80.

Os arquivos JSON são baixados pelo workflow, não versionados no repositório.
Vetores com estado de prefixo interno ativo (campo "p") são excluídos porque o
núcleo atual não modela esse estado. Traços "cycles" não são comparados: a CPU
retorna apenas T-states agregados. Divergências são reportadas, não ocultadas.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from sega_emulator.testing.z80_singlestep import run_vector

DEFAULT_FILES = ("00", "04", "06", "37", "3f", "76", "c6", "cb 00", "dd 21", "ed 40")


def load_json(path: Path) -> list[dict[str, Any]]:
    with path.open(encoding="utf-8") as source:
        payload = json.load(source)
    if not isinstance(payload, list) or not all(isinstance(item, dict) for item in payload):
        raise ValueError(f"{path}: esperado um array JSON de vetores")
    return payload


def audit_file(path: Path) -> dict[str, Any]:
    vectors = load_json(path)
    counts: Counter[str] = Counter()
    results: list[dict[str, Any]] = []
    for vector in vectors:
        initial = vector.get("initial", {})
        final = vector.get("final", {})
        # "p" representa estado interno de prefixo no corpus. Não é um
        # registrador arquitetural modelado pelo nosso núcleo instruction-level.
        if initial.get("p", 0) or final.get("p", 0):
            counts["skipped_prefix_state"] += 1
            continue
        result = run_vector(vector)
        counts[result.status] += 1
        if result.status != "passed":
            results.append(
                {
                    "name": result.name,
                    "status": result.status,
                    "mismatches": result.mismatches,
                    "unsupported_fields": result.unsupported_fields,
                    "cycles_returned": result.cycles_returned,
                }
            )
    return {
        "file": path.name,
        "source": (
            "https://github.com/SingleStepTests/z80/blob/main/v1/" + path.name
        ),
        "total_vectors": len(vectors),
        "counts": dict(sorted(counts.items())),
        "failures_and_unsupported": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument(
        "--files",
        nargs="+",
        default=list(DEFAULT_FILES),
        help='Nomes de arquivos sem ".json" (prefixos podem conter espaço)',
    )
    parser.add_argument("--report-json", type=Path, required=True)
    args = parser.parse_args()

    reports: list[dict[str, Any]] = []
    for file_stem in args.files:
        normalized = file_stem.lower()
        if (
            not normalized
            or "/" in normalized
            or "\\" in normalized
            or not all(char in "0123456789abcdef " for char in normalized)
        ):
            parser.error(f"nome de arquivo inválido: {file_stem}")
        path = args.directory / f"{normalized}.json"
        if not path.is_file():
            parser.error(f"arquivo de vetor ausente: {path}")
        reports.append(audit_file(path))

    aggregate: Counter[str] = Counter()
    total_vectors = 0
    for report in reports:
        total_vectors += report["total_vectors"]
        aggregate.update(report["counts"])
        print(
            f'{report["file"]}: total={report["total_vectors"]}; '
            + "; ".join(
                f"{key}={value}" for key, value in report["counts"].items()
            )
        )

    report_path = args.report_json
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(
            {
                "corpus": "SingleStepTests/Z80",
                "method": "instruction state + RAM; cycle traces not compared",
                "total_vectors": total_vectors,
                "aggregate": dict(sorted(aggregate.items())),
                "files": reports,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"Relatório completo: {report_path}")
    print("Resumo: " + ", ".join(f"{key}={value}" for key, value in sorted(aggregate.items())))
    # Esta é uma auditoria exploratória: divergências são o resultado a analisar,
    # não motivo para mascarar ou interromper a geração do relatório.
    return 0 if total_vectors else 2


if __name__ == "__main__":
    raise SystemExit(main())
