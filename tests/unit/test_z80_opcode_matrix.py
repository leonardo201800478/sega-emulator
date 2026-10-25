from __future__ import annotations

from collections import Counter

from tools.z80_opcode_matrix import (
    base_family,
    build_matrix,
    cb_family,
    ed_family,
    indexed_cb_family,
    indexed_family,
)


def test_matrix_has_all_256_encodings_in_each_space() -> None:
    matrix = build_matrix()
    assert matrix["cell_count"] == 7 * 256
    assert matrix["counts_by_space"] == {
        "base": 256, "CB": 256, "ED": 256, "DD": 256, "FD": 256,
        "DD-CB": 256, "FD-CB": 256,
    }
    counts = Counter((row["space"], row["opcode"]) for row in matrix["rows"])
    assert len(counts) == 7 * 256
    assert all(count == 1 for count in counts.values())


def test_base_prefix_bytes_are_classified_as_prefixes() -> None:
    for opcode, name in (
        (0xCB, "PREFIX CB"),
        (0xDD, "PREFIX DD"),
        (0xED, "PREFIX ED"),
        (0xFD, "PREFIX FD"),
    ):
        assert base_family(opcode)[0] == name
        assert base_family(opcode)[1] == "prefix"


def test_absolute_load_store_families_are_distinguished() -> None:
    assert base_family(0x22)[0] == "LD (nn),HL"
    assert base_family(0x2A)[0] == "LD HL,(nn)"
    assert base_family(0x32)[0] == "LD (nn),A"
    assert base_family(0x3A)[0] == "LD A,(nn)"


def test_cb_sll_is_separately_marked_undocumented() -> None:
    assert cb_family(0x30)[1] == "undocumented"
    assert cb_family(0x36)[0] == "SLL (HL)"


def test_ed_unassigned_bytes_match_current_ed_nop_policy() -> None:
    assert ed_family(0x00) == ("ED-NOP / unassigned byte", "unassigned-classic-z80", "ed-nop-fallback")
    assert ed_family(0xFF)[2] == "ed-nop-fallback"
    assert ed_family(0xA0)[0] == "LDI"
    assert ed_family(0x4B)[0] == "LD rr,(nn)"


def test_indexed_prefix_fallback_and_substitution_are_distinguished() -> None:
    assert indexed_family(0x21)[1] == "index-substitution"
    assert indexed_family(0x24)[1] == "undocumented-index-half"
    assert indexed_family(0x46)[1] == "index-substitution"
    assert indexed_family(0x00)[1] == "ignored-prefix-fallback"
    assert indexed_family(0xCB)[1] == "indexed-cb-prefix"
    assert indexed_family(0xDD)[1] == "repeated-prefix"
    assert indexed_family(0xED)[1] == "ignored-prefix-fallback"


def test_indexed_cb_covers_all_operations() -> None:
    assert indexed_cb_family(0x00)[0] == "RLC (IX/IY+d) -> B"
    assert indexed_cb_family(0x46)[0] == "BIT 0,(IX/IY+d)"
    assert indexed_cb_family(0x86)[0] == "RES 0,(IX/IY+d)"
    assert indexed_cb_family(0xC6)[0] == "SET 0,(IX/IY+d)"
