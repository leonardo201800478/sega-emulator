"""Verify downloaded SingleStepTests vectors against the committed provenance manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("tools/audits/singlestep-corpus-manifest.json"),
    )
    args = parser.parse_args()

    manifest: dict[str, Any] = json.loads(args.manifest.read_text(encoding="utf-8"))
    upstream = manifest["upstream"]
    print(f"Upstream commit: {upstream['commit']}")
    print(f"License: {upstream['license']} ({upstream['license_url']})")

    failures: list[str] = []
    for expected in manifest["files"]:
        path = args.directory / expected["file"]
        if not path.is_file():
            failures.append(f"{expected['file']}: arquivo ausente")
            continue

        data = path.read_bytes()
        actual_hash = hashlib.sha256(data).hexdigest()
        actual_size = len(data)
        hash_ok = actual_hash == expected["sha256"]
        size_ok = actual_size == expected["size_bytes"]
        print(
            f"{expected['file']}: sha256={'OK' if hash_ok else 'DIVERGENTE'}; "
            f"bytes={'OK' if size_ok else 'DIVERGENTE'}"
        )
        if not hash_ok:
            failures.append(
                f"{expected['file']}: SHA-256 esperado {expected['sha256']}, "
                f"obtido {actual_hash}"
            )
        if not size_ok:
            failures.append(
                f"{expected['file']}: tamanho esperado {expected['size_bytes']}, "
                f"obtido {actual_size}"
            )

    if failures:
        print("Falha na verificação de proveniência:")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print(f"Verificação concluída: {len(manifest['files'])} arquivos íntegros.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
