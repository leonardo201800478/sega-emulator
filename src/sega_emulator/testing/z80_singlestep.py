"""Adaptador de vetores SingleStepTests para o núcleo Z80 do projeto.

Executa um vetor de uma instrução com RAM plana de 64 KiB. Não compara os
traços de barramento: Z80Cpu ainda expõe apenas T-states agregados.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict, deque
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from sega_emulator.cpu.z80 import UnsupportedOpcodeError, Z80Cpu

ADDRESS_SPACE_SIZE = 0x10000
REGISTER_FIELDS = {
    "a": "a",
    "f": "f",
    "b": "b",
    "c": "c",
    "d": "d",
    "e": "e",
    "h": "h",
    "l": "l",
    "i": "i",
    "r": "r",
    "pc": "pc",
    "sp": "sp",
    "ix": "ix",
    "iy": "iy",
    "wz": "wz",
    "q": "q",
    "p": "p",
    "af_": "af_alt",
    "bc_": "bc_alt",
    "de_": "de_alt",
    "hl_": "hl_alt",
    "im": "interrupt_mode",
    "iff1": "iff1",
    "iff2": "iff2",
    "ei": "_ei_delay",
}
BOOLEAN_FIELDS = {"iff1", "iff2"}


class FlatMemoryBus:
    """Memória plana de teste, independente do mapa físico do SG-1000."""

    def __init__(self) -> None:
        self.memory = bytearray(ADDRESS_SPACE_SIZE)
        self.written_addresses: set[int] = set()

    def read(self, address: int) -> int:
        if type(address) is not int or not 0 <= address < ADDRESS_SPACE_SIZE:
            raise ValueError(f"Endereço de memória inválido: {address!r}")
        return self.memory[address]

    def write(self, address: int, value: int) -> None:
        if type(address) is not int or not 0 <= address < ADDRESS_SPACE_SIZE:
            raise ValueError(f"Endereço de memória inválido: {address!r}")
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError(f"Byte inválido para escrita: {value!r}")
        self.memory[address] = value
        self.written_addresses.add(address)


def _ports_from_cycles(cycles: Any) -> list[list[Any]]:
    """Extrai valores/transações de I/O do traço SingleStepTests.

    Em leituras, o valor costuma aparecer no registro seguinte do mesmo
    endereço; em escritas, o valor está no próprio registro de controle.
    Isso alimenta o barramento de I/O sem alegar comparação ciclo a ciclo.
    """
    if not isinstance(cycles, list):
        return []
    ports: list[list[Any]] = []
    for index, entry in enumerate(cycles):
        if not isinstance(entry, list) or len(entry) != 3:
            continue
        address, value, control = entry
        if not isinstance(control, str) or "i" not in control:
            continue
        if "r" in control:
            if value is None:
                value = next(
                    (
                        later[1]
                        for later in cycles[index + 1 :]
                        if isinstance(later, list)
                        and len(later) == 3
                        and later[0] == address
                        and later[1] is not None
                    ),
                    0,
                )
            ports.append([address, value, "r"])
        elif "w" in control:
            if value is None:
                value = next(
                    (
                        later[1]
                        for later in cycles[index + 1 :]
                        if isinstance(later, list)
                        and len(later) == 3
                        and later[0] == address
                        and later[1] is not None
                    ),
                    0,
                )
            ports.append([address, value, "w"])
    return ports


class RecordingIOBus:
    """Fornece bytes de entrada declarados no vetor e registra transações de I/O."""

    def __init__(self, expected_ports: list[list[Any]] | None = None) -> None:
        self._reads: dict[int, deque[int]] = defaultdict(deque)
        self.transactions: list[list[Any]] = []
        for entry in expected_ports or []:
            if len(entry) != 3:
                raise ValueError(f"Transação de porta inválida: {entry!r}")
            address, value, direction = entry
            if direction == "r":
                self._reads[address].append(value)

    def read_port(self, address: int) -> int:
        value = self._reads[address].popleft() if self._reads[address] else 0
        self.transactions.append([address, value, "r"])
        return value

    def write_port(self, address: int, value: int) -> None:
        self.transactions.append([address, value, "w"])


@dataclass
class VectorResult:
    """Resultado de um vetor individual, serializável em JSON."""

    name: str
    status: str
    mismatches: list[str] = field(default_factory=list)
    unsupported_fields: list[str] = field(default_factory=list)
    cycles_returned: int | None = None


def _state_value(cpu: Z80Cpu, field_name: str) -> int | bool:
    attribute = REGISTER_FIELDS[field_name]
    value = getattr(cpu, attribute)
    if field_name in BOOLEAN_FIELDS:
        return bool(value)
    return int(value)


def _load_state(cpu: Z80Cpu, state: dict[str, Any]) -> list[str]:
    """Aplica campos representáveis; retorna campos não representáveis ativos."""
    unsupported: list[str] = []
    for field_name, value in state.items():
        if field_name == "ram":
            continue
        if field_name not in REGISTER_FIELDS:
            if value not in (0, False, None):
                unsupported.append(field_name)
            continue
        if field_name in BOOLEAN_FIELDS:
            setattr(cpu, REGISTER_FIELDS[field_name], bool(value))
        else:
            setattr(cpu, REGISTER_FIELDS[field_name], value)
    return unsupported


def _load_ram(memory: FlatMemoryBus, state: dict[str, Any]) -> dict[int, int]:
    contents: dict[int, int] = {}
    for entry in state.get("ram", []):
        if not isinstance(entry, list) or len(entry) != 2:
            raise ValueError(f"Entrada RAM inválida: {entry!r}")
        address, value = entry
        memory.write(address, value)
        contents[address] = value
    memory.written_addresses.clear()
    return contents


def run_vector(vector: dict[str, Any]) -> VectorResult:
    """Executa e compara um vetor SingleStepTests sem exigir traço ciclo a ciclo."""
    name = str(vector.get("name", "<vetor sem nome>"))
    result = VectorResult(name=name, status="failed")
    initial = vector.get("initial")
    final = vector.get("final")
    if not isinstance(initial, dict) or not isinstance(final, dict):
        raise ValueError(f"{name}: vetor precisa conter objetos initial e final")

    memory = FlatMemoryBus()
    initial_ram = _load_ram(memory, initial)
    expected_ports = vector.get("ports")
    if expected_ports is None:
        expected_ports = _ports_from_cycles(vector.get("cycles", []))
    io_bus = RecordingIOBus(expected_ports)
    cpu = Z80Cpu(memory, io_bus)
    unsupported = _load_state(cpu, initial)
    unsupported.extend(
        field_name
        for field_name, value in final.items()
        if field_name not in REGISTER_FIELDS
        and field_name != "ram"
        and value not in (0, False, None)
        and field_name not in unsupported
    )
    result.unsupported_fields = sorted(set(unsupported))
    if result.unsupported_fields:
        result.status = "unsupported"
        return result

    try:
        result.cycles_returned = cpu.step()
    except UnsupportedOpcodeError as exc:
        result.status = "unsupported"
        result.mismatches.append(f"opcode não suportado: {exc}")
        return result
    except (ValueError, TypeError, IndexError, KeyError) as exc:
        result.status = "error"
        result.mismatches.append(f"erro ao executar vetor: {type(exc).__name__}: {exc}")
        return result

    # SingleStepTests records one entry per T-state. Compare the aggregate
    # count returned by step(); this does not validate bus-cycle ordering.
    cycle_trace = vector.get("cycles")
    if (
        isinstance(cycle_trace, list)
        and cycle_trace
        and result.cycles_returned != len(cycle_trace)
    ):
        result.mismatches.append(
            f"tstates: esperado {len(cycle_trace)}, obtido {result.cycles_returned}"
        )

    for field_name, expected in final.items():
        if field_name in ("ram",):
            continue
        if field_name not in REGISTER_FIELDS:
            continue
        actual = _state_value(cpu, field_name)
        normalized_expected = (
            bool(expected) if field_name in BOOLEAN_FIELDS else expected
        )
        if actual != normalized_expected:
            result.mismatches.append(
                f"{field_name}: esperado {normalized_expected!r}, obtido {actual!r}"
            )

    expected_ram = {address: value for address, value in final.get("ram", [])}
    addresses_to_compare = (
        set(initial_ram) | set(expected_ram) | memory.written_addresses
    )
    for address in sorted(addresses_to_compare):
        expected = expected_ram.get(address, initial_ram.get(address, 0))
        actual = memory.read(address)
        if actual != expected:
            result.mismatches.append(
                f"RAM[{address:04X}h]: esperado {expected:02X}h, obtido {actual:02X}h"
            )

    if io_bus.transactions != expected_ports:
        result.mismatches.append(
            f"ports: esperado {expected_ports!r}, obtido {io_bus.transactions!r}"
        )

    result.status = "passed" if not result.mismatches else "failed"
    return result


def load_vectors(path: Path) -> list[dict[str, Any]]:
    """Carrega um arquivo JSON SingleStepTests (lista de vetores)."""
    with path.open(encoding="utf-8") as source:
        payload = json.load(source)
    if not isinstance(payload, list):
        raise ValueError("O arquivo SingleStepTests deve conter uma lista JSON.")
    if not all(isinstance(vector, dict) for vector in payload):
        raise ValueError("Cada elemento do arquivo deve ser um objeto JSON.")
    return payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Executa vetores JSON SingleStepTests contra a CPU Z80."
    )
    parser.add_argument(
        "json_file", type=Path, help="Caminho local para o arquivo de vetores"
    )
    parser.add_argument(
        "--start-index", type=int, default=0, help="Índice inicial (base zero)"
    )
    parser.add_argument(
        "--limit", type=int, default=25, help="Máximo de vetores; 0 significa todos"
    )
    parser.add_argument(
        "--allow-unsupported",
        action="store_true",
        help="Não retorna erro de processo quando houver vetores não suportados",
    )
    parser.add_argument(
        "--report-json", type=Path, help="Grava relatório JSON neste caminho"
    )
    args = parser.parse_args(argv)

    if args.start_index < 0 or args.limit < 0:
        parser.error("--start-index e --limit devem ser >= 0")

    try:
        vectors = load_vectors(args.json_file)
        selected = vectors[args.start_index :]
        if args.limit:
            selected = selected[: args.limit]
        results = [run_vector(vector) for vector in selected]
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Erro: {exc}", file=sys.stderr)
        return 2

    counts = {
        status: sum(result.status == status for result in results)
        for status in ("passed", "failed", "unsupported", "error")
    }
    print(
        "SingleStepTests: "
        f"{len(results)} vetores; aprovados={counts['passed']}; "
        f"divergentes={counts['failed']}; não suportados={counts['unsupported']}; "
        f"erros={counts['error']}"
    )
    for result in results:
        if result.status != "passed":
            print(f"[{result.status.upper()}] {result.name}")
            for mismatch in result.mismatches:
                print(f"  - {mismatch}")
            if result.unsupported_fields:
                print(
                    "  - campos não suportados: " + ", ".join(result.unsupported_fields)
                )

    if args.report_json:
        args.report_json.parent.mkdir(parents=True, exist_ok=True)
        args.report_json.write_text(
            json.dumps(
                {
                    "source_file": str(args.json_file),
                    "start_index": args.start_index,
                    "limit": args.limit,
                    "counts": counts,
                    "results": [asdict(result) for result in results],
                },
                indent=2,
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )

    if counts["failed"] or counts["error"]:
        return 1
    if counts["unsupported"] and not args.allow_unsupported:
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
