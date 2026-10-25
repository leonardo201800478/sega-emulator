"""Build an explicit 1,792-cell Z80 opcode classification matrix.

This is a classification inventory, not a semantic conformance oracle.
Run: python tools/z80_opcode_matrix.py --json
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from typing import Any

REGS = ("B", "C", "D", "E", "H", "L", "(HL)", "A")
RP = ("BC", "DE", "HL", "SP")
RP2 = ("BC", "DE", "HL", "AF")
CONDS = ("NZ", "Z", "NC", "C", "PO", "PE", "P", "M")
ALU = ("ADD A", "ADC A", "SUB", "SBC A", "AND", "XOR", "OR", "CP")
ROT = ("RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL")


def alu_mnemonic(operation: int, operand: str) -> str:
    name = ALU[operation]
    return f"{name},{operand}" if operation in (0, 1) else f"{name} {operand}"


def base_family(op: int) -> tuple[str, str]:
    """Return (family, architectural classification) for a base opcode byte."""
    x, y, z = op >> 6, (op >> 3) & 7, op & 7
    p, q = y >> 1, y & 1
    if x == 0:
        if z == 0:
            if y == 0:
                return "NOP", "documented"
            if y == 1:
                return "EX AF,AF'", "documented"
            if y == 2:
                return "DJNZ e", "documented"
            if y == 3:
                return "JR e", "documented"
            return f"JR {CONDS[y - 4]},e", "documented"
        if z == 1:
            return (f"LD {RP[p]},nn" if not q else f"ADD HL,{RP[p]}", "documented")
        if z == 2:
            if p in (0, 1):
                pair = "BC" if p == 0 else "DE"
                name = f"LD ({pair}),A" if not q else f"LD A,({pair})"
            elif p == 2:
                name = "LD (nn),HL" if not q else "LD HL,(nn)"
            else:
                name = "LD (nn),A" if not q else "LD A,(nn)"
            return name, "documented"
        if z == 3:
            return (f"{'INC' if not q else 'DEC'} {RP[p]}", "documented")
        if z in (4, 5):
            return (f"{'INC' if z == 4 else 'DEC'} {REGS[y]}", "documented")
        if z == 6:
            return f"LD {REGS[y]},n", "documented"
        return (
            ("RLCA", "documented")
            if y == 0
            else ("RRCA", "documented")
            if y == 1
            else ("RLA", "documented")
            if y == 2
            else ("RRA", "documented")
            if y == 3
            else ("DAA", "documented")
            if y == 4
            else ("CPL", "documented")
            if y == 5
            else ("SCF", "documented")
            if y == 6
            else ("CCF", "documented")
        )
    if x == 1:
        return (
            ("HALT", "documented")
            if op == 0x76
            else (f"LD {REGS[y]},{REGS[z]}", "documented")
        )
    if x == 2:
        return alu_mnemonic(y, REGS[z]), "documented"
    if z == 0:
        return f"RET {CONDS[y]}", "documented"
    if z == 1:
        if not q:
            return f"POP {RP2[p]}", "documented"
        return (
            ("RET", "documented")
            if y == 1
            else ("EXX", "documented")
            if y == 3
            else ("JP (HL)", "documented")
            if y == 5
            else ("LD SP,HL", "documented")
        )
    if z == 2:
        return f"JP {CONDS[y]},nn", "documented"
    if z == 3:
        names = {
            0: ("JP nn", "documented"),
            1: ("PREFIX CB", "prefix"),
            2: ("OUT (n),A", "documented"),
            3: ("IN A,(n)", "documented"),
            4: ("EX (SP),HL", "documented"),
            5: ("EX DE,HL", "documented"),
            6: ("DI", "documented"),
            7: ("EI", "documented"),
        }
        return names[y]
    if z == 4:
        return f"CALL {CONDS[y]},nn", "documented"
    if z == 5:
        if not q:
            return f"PUSH {RP2[p]}", "documented"
        return (
            ("CALL nn", "documented")
            if y == 1
            else ("PREFIX DD", "prefix")
            if y == 3
            else ("PREFIX ED", "prefix")
            if y == 5
            else ("PREFIX FD", "prefix")
        )
    if z == 6:
        return alu_mnemonic(y, "n"), "documented"
    return f"RST {y * 8:02X}h", "documented"


def cb_family(op: int) -> tuple[str, str]:
    group, bit, reg = op >> 6, (op >> 3) & 7, op & 7
    if group == 0:
        return f"{ROT[bit]} {REGS[reg]}", "undocumented" if bit == 6 else "documented"
    if group == 1:
        return f"BIT {bit},{REGS[reg]}", "documented"
    if group == 2:
        return f"RES {bit},{REGS[reg]}", "documented"
    return f"SET {bit},{REGS[reg]}", "documented"


ED_ASSIGNED_EXACT = {
    0x47: "LD I,A",
    0x4F: "LD R,A",
    0x57: "LD A,I",
    0x5F: "LD A,R",
    0x67: "RRD",
    0x6F: "RLD",
    0xA0: "LDI",
    0xA8: "LDD",
    0xB0: "LDIR",
    0xB8: "LDDR",
    0xA1: "CPI",
    0xA9: "CPD",
    0xB1: "CPIR",
    0xB9: "CPDR",
    0xA2: "INI",
    0xAA: "IND",
    0xB2: "INIR",
    0xBA: "INDR",
    0xA3: "OUTI",
    0xAB: "OUTD",
    0xB3: "OTIR",
    0xBB: "OTDR",
}
ED_NEG = {0x44, 0x4C, 0x54, 0x5C, 0x64, 0x6C, 0x74, 0x7C}
ED_RETURN = {0x45, 0x4D, 0x55, 0x5D, 0x65, 0x6D, 0x75, 0x7D}
ED_IM = {
    0x46: 0,
    0x4E: 0,
    0x66: 0,
    0x6E: 0,
    0x56: 1,
    0x76: 1,
    0x5E: 2,
    0x7E: 2,
}


def ed_family(op: int) -> tuple[str, str, str]:
    """Return family, architecture class and implementation path."""
    if op in ED_ASSIGNED_EXACT:
        return ED_ASSIGNED_EXACT[op], "documented", "ed-implemented"
    if op in ED_NEG:
        if op == 0x44:
            return "NEG", "documented", "ed-implemented"
        return "NEG (alias encoding)", "alias", "ed-implemented"
    if op in ED_RETURN:
        if op == 0x4D:
            return "RETI", "documented", "ed-implemented"
        return "RETN (alias encoding)", "alias", "ed-implemented"
    if op in ED_IM:
        canonical = {0x46, 0x56, 0x5E}
        if op in canonical:
            return f"IM {ED_IM[op]}", "documented", "ed-implemented"
        return f"IM {ED_IM[op]} (alias encoding)", "alias", "ed-implemented"
    if op == 0x70:
        return "IN (C)", "undocumented", "ed-implemented"
    if op & 0xC7 == 0x40:
        return "IN r,(C)", "documented", "ed-implemented"
    if op == 0x71:
        return "OUT (C),0", "undocumented", "ed-implemented"
    if op & 0xC7 == 0x41:
        return "OUT (C),r", "documented", "ed-implemented"
    if op & 0xCF == 0x42:
        return "SBC HL,rr", "documented", "ed-implemented"
    if op & 0xCF == 0x4A:
        return "ADC HL,rr", "documented", "ed-implemented"
    if op & 0xCF == 0x43:
        return "LD (nn),rr", "documented", "ed-implemented"
    if op & 0xCF == 0x4B:
        return "LD rr,(nn)", "documented", "ed-implemented"
    return "ED-NOP / unassigned byte", "unassigned-classic-z80", "ed-nop-fallback"


INDEXED_FAMILIES = {
    0x21: "LD IX/IY,nn",
    0x22: "LD (nn),IX/IY",
    0x2A: "LD IX/IY,(nn)",
    0x23: "INC IX/IY",
    0x2B: "DEC IX/IY",
    0x09: "ADD IX/IY,BC",
    0x19: "ADD IX/IY,DE",
    0x29: "ADD IX/IY,IX/IY",
    0x39: "ADD IX/IY,SP",
    0xE1: "POP IX/IY",
    0xE5: "PUSH IX/IY",
    0xE3: "EX (SP),IX/IY",
    0xE9: "JP (IX/IY)",
    0xF9: "LD SP,IX/IY",
    0x36: "LD (IX/IY+d),n",
    0x34: "INC (IX/IY+d)",
    0x35: "DEC (IX/IY+d)",
}


def indexed_family(op: int) -> tuple[str, str]:
    if op == 0xCB:
        return "PREFIX (IX/IY+d)-CB", "indexed-cb-prefix"
    if op in (0xDD, 0xFD):
        return "repeated DD/FD prefix; last prefix selects index", "repeated-prefix"
    if op == 0xED:
        return "ignored DD/FD before ED instruction", "ignored-prefix-fallback"
    if op in (0x24, 0x25, 0x26, 0x2C, 0x2D, 0x2E):
        return "INC/DEC/LD IXH/IXL or IYH/IYL", "undocumented-index-half"
    if op in INDEXED_FAMILIES:
        return INDEXED_FAMILIES[op], "index-substitution"
    if 0x40 <= op <= 0x7F and op != 0x76:
        destination, source = (op >> 3) & 7, op & 7
        if destination == 6 or source == 6:
            return "LD r,(IX/IY+d) / LD (IX/IY+d),r", "index-substitution"
        if destination in (4, 5) or source in (4, 5):
            return "LD with IXH/IXL or IYH/IYL", "index-substitution"
        return "LD r,r' with ignored DD/FD prefix", "ignored-prefix-equivalent"
    if 0x80 <= op <= 0xBF:
        operation, source = (op >> 3) & 7, op & 7
        if source == 6:
            return alu_mnemonic(operation, "(IX/IY+d)"), "index-substitution"
        if source in (4, 5):
            operand = "IXH/IYH" if source == 4 else "IXL/IYL"
            return alu_mnemonic(operation, operand), "index-substitution"
        return (
            alu_mnemonic(operation, REGS[source]),
            "ignored-prefix-equivalent",
        )
    return "base instruction with ignored DD/FD prefix", "ignored-prefix-fallback"


def indexed_cb_family(op: int) -> tuple[str, str]:
    group, bit, reg = op >> 6, (op >> 3) & 7, op & 7
    operand = "(IX/IY+d)"
    if group == 0:
        return f"{ROT[bit]} {operand}" + (
            f" -> {REGS[reg]}" if reg != 6 else ""
        ), "undocumented" if bit == 6 else "documented"
    if group == 1:
        return f"BIT {bit},{operand}", "documented"
    name = "RES" if group == 2 else "SET"
    return f"{name} {bit},{operand}" + (
        f" -> {REGS[reg]}" if reg != 6 else ""
    ), "documented"


def validation_scope(space: str, path: str, arch: str) -> tuple[str, str]:
    """Describe evidence level and next gap without overstating test coverage."""
    if arch == "undocumented":
        return (
            "variant-specific behavior; separate policy required",
            "Do not infer compatibility from dispatch alone.",
        )
    if path == "ed-nop-fallback":
        return (
            "fallback-policy tests exist",
            "ED-NOP is not semantic implementation of an assigned instruction.",
        )
    if path in (
        "ignored-prefix-fallback",
        "ignored-prefix-equivalent",
        "repeated-prefix",
    ):
        return (
            "targeted prefix tests exist",
            "No exhaustive oracle for every prefix/opcode/state combination.",
        )
    if space == "base" and path != "base-decoder":
        return (
            "prefix dispatch; see the prefixed opcode space",
            "Validate the delegated instruction in its own space.",
        )
    if space == "base":
        return (
            "family-level tests exist; per-opcode proof incomplete",
            "Expand differential cases across operands and state boundaries.",
        )
    if space == "CB":
        return (
            "all-byte dispatch test plus selected semantic/flag tests",
            "Compare results and flags for all operation/register combinations.",
        )
    if space == "ED":
        return (
            "targeted family tests; differential coverage incomplete",
            "Validate flags, aliases, WZ, I/O side effects and nominal timing.",
        )
    if space in ("DD", "FD"):
        return (
            "selected IX/IY tests; differential coverage incomplete",
            "Verify H/L substitution exceptions, displacement, flags and timing.",
        )
    return (
        "all-byte dispatch test plus selected indexed-bit tests",
        "Validate copy-to-register variants, flags and timing across all operations.",
    )


def build_matrix() -> dict[str, Any]:
    rows: list[dict[str, str]] = []
    for op in range(256):
        family, arch = base_family(op)
        path = "base-decoder"
        if op in (0xCB, 0xDD, 0xED, 0xFD):
            path = {
                0xCB: "cb-prefix",
                0xDD: "dd-prefix",
                0xED: "ed-prefix",
                0xFD: "fd-prefix",
            }[op]
        validation, gap = validation_scope("base", path, arch)
        rows.append(
            {
                "space": "base",
                "opcode": f"{op:02X}",
                "family": family,
                "architecture_class": arch,
                "implementation_path": path,
                "validation_scope": validation,
                "known_gap": gap,
            }
        )
    for space in ("CB", "ED", "DD", "FD", "DD-CB", "FD-CB"):
        for op in range(256):
            if space == "CB":
                family, arch = cb_family(op)
                path = "cb-decoder"
            elif space == "ED":
                family, arch, path = ed_family(op)
            elif space in ("DD", "FD"):
                family, path = indexed_family(op)
                arch = (
                    "undocumented"
                    if path == "undocumented-index-half"
                    else "prefix-behavior"
                )
            else:
                family, arch = indexed_cb_family(op)
                path = "indexed-cb-decoder"
            validation, gap = validation_scope(space, path, arch)
            rows.append(
                {
                    "space": space,
                    "opcode": f"{op:02X}",
                    "family": family,
                    "architecture_class": arch,
                    "implementation_path": path,
                    "validation_scope": validation,
                    "known_gap": gap,
                }
            )
    counts = Counter(row["space"] for row in rows)
    return {
        "schema_version": 1,
        "scope": "classic Zilog Z80 opcode classification; semantic correctness is not implied",
        "source_code": "src/sega_emulator/cpu/z80.py",
        "classification_rule": "One row per second-byte encoding per opcode space; DD/FD rows describe the second byte following that prefix.",
        "counts_by_space": {
            space: counts[space]
            for space in ("base", "CB", "ED", "DD", "FD", "DD-CB", "FD-CB")
        },
        "cell_count": len(rows),
        "rows": rows,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    matrix = build_matrix()
    if args.json:
        print(json.dumps(matrix, indent=2, ensure_ascii=True))
    else:
        print(f"Z80 opcode matrix: {matrix['cell_count']} cells")
        for space, count in matrix["counts_by_space"].items():
            print(f"{space:7} {count}")
        print("Classification is not semantic conformance.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
