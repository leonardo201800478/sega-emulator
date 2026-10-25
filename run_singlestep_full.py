"""Executa o corpus SingleStepTests/z80 COMPLETO contra o núcleo Z80 do projeto.

Diferenças em relação a tools/audits/run_singlestep_corpus.py:
  * percorre todos os arquivos do diretório (não só um subconjunto fixo);
  * COMPARA o total de T-states (cpu.step() vs. len(vetor["cycles"])), que o
    adaptador atual registra mas não verifica;
  * agrega divergências por campo, por família de prefixo e por arquivo;
  * usa multiprocessing e escreve relatório JSON + resumo em Markdown.

Uso:
  python run_singlestep_full.py --directory CAMINHO/v1 --report-json rel.json \
      --report-md rel.md [--workers N] [--files "00" "cb 00" ...] [--no-cycles]

Não modifica o projeto; só importa sega_emulator.testing.z80_singlestep.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path
from typing import Any

from sega_emulator.testing.z80_singlestep import run_vector

MAX_SAMPLES_PER_FILE = 3


def family(stem: str) -> str:
    parts = stem.split()
    if not parts:
        return "?"
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
    stem = path.stem
    with path.open(encoding="utf-8") as handle:
        vectors = json.load(handle)
    status = Counter()
    cats: Counter[str] = Counter()
    samples: list[dict[str, Any]] = []
    cycle_expected_hist: Counter[str] = Counter()
    for vector in vectors:
        result = run_vector(vector)
        mism = list(result.mismatches)
        st = result.status
        if check_cycles and st in ("passed", "failed"):
            expected = len(vector.get("cycles", []))
            if expected and result.cycles_returned != expected:
                mism.append(
                    f"tstates: esperado {expected}, obtido {result.cycles_returned}"
                )
                st = "failed"
                cycle_expected_hist[f"{expected}->{result.cycles_returned}"] += 1
        status[st] += 1
        if st != "passed":
            for category_name in {category(m) for m in mism} or {st}:
                cats[category_name] += 1
            if len(samples) < MAX_SAMPLES_PER_FILE:
                samples.append(
                    {
                        "name": result.name,
                        "status": st,
                        "mismatches": mism[:6],
                        "unsupported_fields": result.unsupported_fields,
                    }
                )
    return {
        "file": stem,
        "family": family(stem),
        "total": len(vectors),
        "status": dict(status),
        "categories": dict(cats),
        "tstate_pairs": dict(cycle_expected_hist.most_common(5)),
        "samples": samples,
    }


def file_sort_key(stem: str) -> tuple[int, str]:
    order = {
        "base": 0,
        "cb": 1,
        "ed": 2,
        "dd": 3,
        "fd": 4,
        "ddcb": 5,
        "fdcb": 6,
    }
    return (order.get(family(stem), 9), stem)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--files", nargs="*", default=None, help="stems sem .json")
    parser.add_argument("--workers", type=int, default=os.cpu_count() or 1)
    parser.add_argument("--no-cycles", action="store_true")
    parser.add_argument(
        "--report-json", type=Path, default=Path("singlestep-full.json")
    )
    parser.add_argument("--report-md", type=Path, default=Path("singlestep-full.md"))
    args = parser.parse_args()

    if args.files:
        stems = args.files
    else:
        stems = sorted(
            (p.stem for p in args.directory.glob("*.json")), key=file_sort_key
        )
    paths = [str(args.directory / f"{s}.json") for s in stems]
    missing = [p for p in paths if not Path(p).exists()]
    if missing:
        print(
            f"ERRO: {len(missing)} arquivos ausentes, ex.: {missing[:3]}",
            file=sys.stderr,
        )
        return 2

    started = time.perf_counter()
    results: list[dict[str, Any]] = []
    with ProcessPoolExecutor(max_workers=max(1, args.workers)) as pool:
        futures = {pool.submit(audit_file, p, not args.no_cycles): p for p in paths}
        for index, future in enumerate(as_completed(futures), 1):
            results.append(future.result())
            if index % 50 == 0 or index == len(paths):
                elapsed = time.perf_counter() - started
                print(
                    f"[{index}/{len(paths)}] {elapsed:6.0f}s",
                    file=sys.stderr,
                    flush=True,
                )
    results.sort(key=lambda r: file_sort_key(r["file"]))

    total = Counter()
    by_family: dict[str, Counter[str]] = defaultdict(Counter)
    by_cat: Counter[str] = Counter()
    for r in results:
        for k, v in r["status"].items():
            total[k] += v
            by_family[r["family"]][k] += v
        by_family[r["family"]]["_files"] += 1
        by_family[r["family"]]["_total"] += r["total"]
        if set(r["status"]) != {"passed"}:
            by_family[r["family"]]["_files_with_failures"] += 1
        for k, v in r["categories"].items():
            by_cat[k] += v
    grand = sum(total.values())
    summary = {
        "vectors": grand,
        "files": len(results),
        "status": dict(total),
        "pass_rate_percent": round(total["passed"] / grand * 100, 3) if grand else 0,
        "check_cycles": not args.no_cycles,
        "elapsed_seconds": round(time.perf_counter() - started, 1),
        "by_family": {k: dict(v) for k, v in by_family.items()},
        "by_category": dict(by_cat.most_common()),
    }
    args.report_json.write_text(
        json.dumps(
            {"summary": summary, "files": results}, indent=1, ensure_ascii=False
        ),
        encoding="utf-8",
    )

    lines = ["# Relatório SingleStepTests/z80 — núcleo atual\n"]
    lines.append(
        f"Vetores: **{grand:,}** em **{len(results)}** arquivos. "
        f"Aprovados: **{total['passed']:,}** ({summary['pass_rate_percent']}%). "
        f"T-states comparados: {'sim' if not args.no_cycles else 'não'}. "
        f"Tempo: {summary['elapsed_seconds']} s.\n"
    )
    lines.append(
        "## Por família\n\n| Família | Arquivos | c/ falha | Vetores | Aprovados | % |"
    )
    lines.append("|---|---|---|---|---|---|")
    for fam in ("base", "cb", "ed", "dd", "fd", "ddcb", "fdcb"):
        if fam not in by_family:
            continue
        c = by_family[fam]
        pct = c["passed"] / c["_total"] * 100 if c["_total"] else 0
        lines.append(
            f"| {fam} | {c['_files']} | {c['_files_with_failures']} | {c['_total']:,} | {c['passed']:,} | {pct:.2f} |"
        )
    lines.append("\n## Categorias de divergência (vetores afetados)\n")
    for k, v in by_cat.most_common():
        lines.append(f"- `{k}`: {v:,}")
    bad = [r for r in results if set(r["status"]) != {"passed"}]
    lines.append(f"\n## Arquivos com falha ({len(bad)})\n")
    lines.append("| Arquivo | Aprov./Total | Categorias |")
    lines.append("|---|---|---|")
    for r in bad:
        cats = ", ".join(
            f"{k}:{v}"
            for k, v in sorted(r["categories"].items(), key=lambda kv: -kv[1])[:5]
        )
        lines.append(
            f"| `{r['file']}` | {r['status'].get('passed', 0)}/{r['total']} | {cats} |"
        )
    args.report_md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                k: summary[k]
                for k in ("vectors", "status", "pass_rate_percent", "elapsed_seconds")
            }
        )
    )
    return 0 if not bad else 1


if __name__ == "__main__":
    raise SystemExit(main())
