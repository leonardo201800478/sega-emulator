"""Execute um subconjunto reproduzível do corpus externo SingleStepTests/Z80.

Os arquivos JSON são baixados pelo workflow, não versionados no repositório.
O latch interno "p" (LD A,I/LD A,R e o erratum de INT) é modelado e comparado.
Traços "cycles" não são comparados: a CPU retorna apenas T-states agregados.
Divergências e estados ainda não suportados bloqueiam o workflow.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any

from sega_emulator.testing.z80_singlestep import run_vector

UPSTREAM_COMMIT = "ebe1875d48f374bcfd4b505d8eb8ee751568b5f7"

DEFAULT_FILES = (
    "00",
    "04",
    "06",
    "37",
    "3f",
    "76",
    "c6",
    "d3",
    "cb 00",
    "dd 21",
    "ed 40",
    "18",
    "20",
    "21",
    "22",
    "24",
    "26",
    "2a",
    "32",
    "cb 06",
    "cb 40",
    "ed 42",
    "ed 44",
    "ed 45",
    "dd 46",
)


def load_json(path: Path) -> list[dict[str, Any]]:
    with path.open(encoding="utf-8") as source:
        payload = json.load(source)
    if not isinstance(payload, list) or not all(
        isinstance(item, dict) for item in payload
    ):
        raise ValueError(f"{path}: esperado um array JSON de vetores")
    return payload


def audit_file(path: Path, manifest_entry: dict[str, Any]) -> dict[str, Any]:
    vectors = load_json(path)
    counts: Counter[str] = Counter()
    results: list[dict[str, Any]] = []
    for vector in vectors:
        # "p" é o latch de LD A,I/R do Z80 (não um estado de prefixo).
        # O adaptador agora carrega e compara esse estado interno.
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
            "https://github.com/SingleStepTests/z80/blob/"
            + UPSTREAM_COMMIT
            + "/v1/"
            + path.name.replace(" ", "%20")
        ),
        "sha256": manifest_entry["sha256"],
        "size_bytes": manifest_entry["size_bytes"],
        "upstream_commit": UPSTREAM_COMMIT,
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
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("tools/audits/singlestep-corpus-manifest.json"),
    )
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    if manifest["upstream"]["commit"] != UPSTREAM_COMMIT:
        parser.error("commit do manifesto diverge do commit fixado no executor")
    manifest_by_file = {entry["file"]: entry for entry in manifest["files"]}

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
        if path.name not in manifest_by_file:
            parser.error(f"arquivo sem entrada no manifesto SHA-256: {path.name}")
        reports.append(audit_file(path, manifest_by_file[path.name]))

    aggregate: Counter[str] = Counter()
    total_vectors = 0
    for report in reports:
        total_vectors += report["total_vectors"]
        aggregate.update(report["counts"])
        print(
            f"{report['file']}: total={report['total_vectors']}; "
            + "; ".join(f"{key}={value}" for key, value in report["counts"].items())
        )

    report_path = args.report_json
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(
            {
                "corpus": "SingleStepTests/Z80",
                "upstream_commit": UPSTREAM_COMMIT,
                "manifest": str(args.manifest),
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
    print(
        "Resumo: "
        + ", ".join(f"{key}={value}" for key, value in sorted(aggregate.items()))
    )
    # Sempre preservar o relatório (o workflow publica artefatos com if: always),
    # mas bloquear o gate quando houver divergências ou estados não suportados.
    # Vetores com estado interno de prefixo ativo são contabilizados separadamente;
    # não são tratados como aprovados nem bloqueiam este subconjunto delimitado.
    if aggregate["failed"] or aggregate["unsupported"]:
        return 1
    return 0 if aggregate["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
