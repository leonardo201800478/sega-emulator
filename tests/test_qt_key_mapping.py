from __future__ import annotations

import pytest

pytest.importorskip("PySide6.QtCore")
qt_gui = pytest.importorskip("PySide6.QtGui")
from PySide6.QtCore import Qt

QKeySequence = qt_gui.QKeySequence


@pytest.mark.parametrize(
    "key_name",
    [
        "Up",
        "Down",
        "Left",
        "Right",
        "Z",
        "X",
        "P",
        "W",
        "S",
        "A",
        "D",
        "F",
        "G",
        "H",
        "Space",
        "Return",
    ],
)
def test_default_keyboard_key_names_are_parseable_by_qt(key_name: str) -> None:
    sequence = QKeySequence.fromString(
        key_name, QKeySequence.SequenceFormat.PortableText
    )
    assert not sequence.isEmpty()
    assert sequence[0].toCombined() != 0


@pytest.mark.parametrize(
    "key",
    [
        Qt.Key.Key_Up,
        Qt.Key.Key_Down,
        Qt.Key.Key_Left,
        Qt.Key.Key_Right,
        Qt.Key.Key_A,
        Qt.Key.Key_Space,
        Qt.Key.Key_Return,
    ],
)
def test_captured_qt_keys_produce_parseable_portable_names(key: Qt.Key) -> None:
    key_name = QKeySequence(key).toString(QKeySequence.SequenceFormat.PortableText)
    assert key_name
    sequence = QKeySequence.fromString(
        key_name, QKeySequence.SequenceFormat.PortableText
    )
    assert not sequence.isEmpty()
    assert sequence[0].toCombined() == QKeySequence(key)[0].toCombined()
