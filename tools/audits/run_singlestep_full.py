"""Auditoria diferencial completa do corpus SingleStepTests/Z80.

O corpus é obtido no workflow a partir de um commit upstream fixado; não
versionamos os JSON externos. Compara estado final, RAM, portas e T-states
agregados (cpu.step() contra o comprimento do vetor de ciclos). Isso não
substitui a comparação de cada evento de barramento/ciclo.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path
from typing import Any

from sega_emulator.testing.z80_singlestep import run_vector

MAX_SAMPLES_PER_FILE = 3


def family(stem: str) -> str:
    parts = stem.lower().split()
    if not parts:
        return "unknown"
    if len(parts) == 1:
        return "base"
    if parts[0] in ("dd", "fd") and len(parts) >= 3 and parts[1] == "cb":
        return f"{parts[0]}cb"
    return parts[0]


def category(message: str) -> str:
    if message.startswith("RAM["):
        return "ram"
    if message.startswith("ports:"):
        return "ports"
    if message.startswith("opcode não suportado"):
        return "unsupported_opcode"
    if message.startswith("erro ao executar"):
        return "exception"
    if message.startswith("tstates:"):
        return "tstates"
    return message.split(":", 1)[0]


def audit_file(path_str: str, check_cycles: bool) -> dict[str, Any]:
    path = Path(path_str)
    with path.open(encoding="utf-8") as handle:
        vectors = json.load(handle)
    if not isinstance(vectors, list) or not all(isinstance(v, dict) for v in vectors):
        raise ValueError(f"{path}: esperado um array JSON de vetores")
    stem = path.stem
    status: Counter[str] = Counter()
    categories: Counter[str] = Counter()
    samples: list[dict[str, Any]] = []
    cycle_pairs: Counter[str] = Counter()

    for vector in vectors:
        result = run_vector(vector)
        mismatches = list(result.mismatches)
        state = result.status
        if check_cycles and state in ("passed", "failed"):
            expected = len(vector.get("cycles", []))
            if expected and result.cycles_returned != expected:
                mismatches.append(
                    f"tstates: esperado {expected}, obtido {result.cycles_returned}"
                )
                state = "failed"
                cycle_pairs[f"{expected}->{result.cycles_returned}"] += 1
        status[state] += 1
        if state != "passed":
            for item in {category(m) for m in mismatches} or {state}:
                categories[item] += 1
            if len(samples) < MAX_SAMPLES_PER_FILE:
                samples.append(
                    {
                        "name": result.name,
                        "status": state,
                        "mismatches": mismatches[:6],
                        "unsupported_fields": result.unsupported_fields,
                    }
                )

    return {
        "file": stem,
        "family": family(stem),
        "total": len(vectors),
        "status": dict(status),
        "categories": dict(categories),
        "tstate_pairs": dict(cycle_pairs.most_common(5)),
        "samples": samples,
    }


def sort_key(stem: str) -> tuple[int, str]:
    order = {"base": 0, "cb": 1, "ed": 2, "dd": 3, "fd": 4, "ddcb": 5, "fdcb": 6}
    return order.get(family(stem), 9), stem.lower()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--files", nargs="*", default=None, help="nomes sem .json")
    parser.add_argument("--workers", type=int, default=min(2, os.cpu_count() or 1))
    parser.add_argument("--no-cycles", action="store_true")
    parser.add_argument("--report-json", type=Path, default=Path("artifacts/singlestep-full.json"))
    parser.add_argument("--report-md", type=Path, default=Path("artifacts/singlestep-full.md"))
    args = parser.parse_args()

    if args.workers < 1:
        parser.error("--workers deve ser >= 1")
    if not args.directory.is_dir():
        parser.error(f"diretório de vetores inexistente: {args.directory}")
    stems = (
        args.files
        if args.files is not None
        else [path.stem for path in args.directory.glob("*.json")]
    )
    stems = sorted(stems, key=sort_key)
    paths = [args.directory / f"{stem}.json" for stem in stems]
    missing = [str(path) for path in paths if not path.is_file()]
    if missing:
        print(f"ERRO: {len(missing)} arquivos ausentes; exemplos: {missing[:3]}", file=sys.stderr)
        return 2
    if not paths:
        print(f"ERRO: nenhum arquivo JSON encontrado em {args.directory}", file=sys.stderr)
        return 2

    started = time.perf_counter()
    results: list[dict[str, Any]] = []
    with ProcessPoolExecutor(max_workers=args.workers) as pool:
        futures = {
            pool.submit(audit_file, str(path), not args.no_cycles): path for path in paths
        }
        for index, future in enumerate(as_completed(futures), 1):
            results.append(future.result())
            if index % 25 == 0 or index == len(paths):
                elapsed = time.perf_counter() - started
                print(f"[{index}/{len(paths)}] {elapsed:.0f}s", file=sys.stderr, flush=True)

    results.sort(key=lambda item: sort_key(item["file"]))
    total: Counter[str] = Counter()
    by_family: dict[str, Counter[str]] = defaultdict(Counter)
    by_category: Counter[str] = Counter()
    for item in results:
        for state, count in item["status"].items():
            total[state] += count
            by_family[item["family"]][state] += count
        by_family[item["family"]]["_files"] += 1
        by_family[item["family"]]["_total"] += item["total"]
        if set(item["status"]) != {"passed"}:
            by_family[item["family"]]["_files_with_failures"] += 1
        by_category.update(item["categories"])

    grand = sum(total.values())
    elapsed = round(time.perf_counter() - started, 1)
    summary = {
        "vectors": grand,
        "files": len(results),
        "status": dict(total),
        "pass_rate_percent": round(total["passed"] / grand * 100, 3) if grand else 0.0,
        "check_cycles": not args.no_cycles,
        "elapsed_seconds": elapsed,
        "by_family": {key: dict(value) for key, value in sorted(by_family.items())},
        "by_category": dict(by_category.most_common()),
    }
    report = {"corpus": "SingleStepTests/Z80", "summary": summary, "files": results}
    args.report_json.parent.mkdir(parents=True, exist_ok=True)
    args.report_md.parent.mkdir(parents=True, exist_ok=True)
    args.report_json.write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    lines = [
        "# Relatório SingleStepTests/Z80 — núcleo atual",
        "",
        f"Vetores: **{grand:,}** em **{len(results)}** arquivos.",
        f"Aprovados: **{total['passed']:,}** ({summary['pass_rate_percent']}%).",
        f"T-states agregados comparados: **{'sim' if not args.no_cycles else 'não'}**.",
        f"Tempo: **{elapsed} s**.",
        "",
        "## Por família",
        "",
        "| Família | Arquivos | Arquivos com falha | Vetores | Aprovados | % |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for name in ("base", "cb", "ed", "dd", "fd", "ddcb", "fdcb"):
        if name not in by_family:
            continue
        stats = by_family[name]
        pct = stats["passed"] / stats["_total"] * 100 if stats["_total"] else 0
        lines.append(
            f"| {name} | {stats['_files']} | {stats['_files_with_failures']} "
            f"| {stats['_total']:,} | {stats['passed']:,} | {pct:.2f} |"
        )
    lines.extend(["", "## Categorias de divergência", ""])
    lines.extend(f"- `{name}`: {count:,}" for name, count in by_category.most_common())
    bad = [item for item in results if set(item["status"]) != {"passed"}]
    lines.extend(["", f"## Arquivos com falha ({len(bad)})", "", "| Arquivo | Aprovados/total | Categorias |", "|---|---:|---|"])
    for item in bad:
        cats = ", ".join(
            f"{name}:{count}"
            for name, count in sorted(item["categories"].items(), key=lambda pair: -pair[1])[:5]
        )
        lines.append(f"| `{item['file']}` | {item['status'].get('passed', 0)}/{item['total']} | {cats} |")
    args.report_md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))
    return 0 if not bad else 1


if __name__ == "__main__":
    raise SystemExit(main())
