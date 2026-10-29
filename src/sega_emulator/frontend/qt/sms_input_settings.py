"""Qt settings and visual reference for Sega Master System input devices.

These preferences are persisted by the frontend, but the SMS Qt-to-Rust input
adapter is not integrated yet. The page must not imply that mouse aiming or
Light Phaser hit detection is already active in gameplay.
"""

from __future__ import annotations

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QBrush, QPainter, QPainterPath, QPen
from PySide6.QtWidgets import (
    QCheckBox,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QSpinBox,
    QVBoxLayout,
    QWidget,
)


class _ControllerLayout(QWidget):
    """Paint a schematic Master System controller for the user guide."""

    def __init__(self, player: int, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.player = player
        self.setObjectName(f"smsControllerLayoutPlayer{player}")
        self.setMinimumSize(250, 150)
        self.setAccessibleName(f"Layout do controle Master System, jogador {player}")

    def paintEvent(self, event) -> None:  # noqa: N802
        del event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setPen(QPen(QColor("#52627a"), 2))
        painter.setBrush(QBrush(QColor("#202a3a")))
        painter.drawRoundedRect(QRectF(12, 18, self.width() - 24, 104), 28, 28)

        painter.setPen(Qt.PenStyle.NoPen)
        painter.setBrush(QBrush(QColor("#d5dce7")))
        painter.drawRoundedRect(QRectF(42, 44, 22, 58), 4, 4)
        painter.drawRoundedRect(QRectF(24, 62, 58, 22), 4, 4)
        painter.setBrush(QBrush(QColor("#8e9bb0")))
        painter.drawEllipse(QRectF(44, 64, 18, 18))

        for x, label in ((145, "1"), (193, "2")):
            painter.setBrush(QBrush(QColor("#d95763")))
            painter.setPen(QPen(QColor("#f0a3a8"), 1))
            painter.drawEllipse(QRectF(x, 55, 30, 30))
            painter.setPen(QPen(QColor("#ffffff")))
            painter.drawText(QRectF(x, 55, 30, 30), Qt.AlignmentFlag.AlignCenter, label)

        painter.setPen(QPen(QColor("#d5dce7")))
        painter.drawText(
            QRectF(15, 126, self.width() - 30, 20),
            Qt.AlignmentFlag.AlignCenter,
            f"Jogador {self.player} • Direcional + Botões 1 e 2",
        )
        painter.end()


class _LightPhaserLayout(QWidget):
    """Paint a schematic Light Phaser silhouette for the user guide."""

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("smsLightPhaserLayout")
        self.setMinimumSize(250, 142)
        self.setAccessibleName("Ilustração esquemática da pistola Light Phaser")

    def paintEvent(self, event) -> None:  # noqa: N802
        del event
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setPen(QPen(QColor("#52627a"), 2))
        painter.setBrush(QBrush(QColor("#202a3a")))

        barrel = QPainterPath()
        barrel.moveTo(22, 30)
        barrel.lineTo(178, 30)
        barrel.quadTo(188, 30, 188, 40)
        barrel.lineTo(188, 57)
        barrel.quadTo(188, 65, 178, 65)
        barrel.lineTo(22, 65)
        barrel.closeSubpath()
        painter.drawPath(barrel)

        handle = QPainterPath()
        handle.moveTo(104, 62)
        handle.lineTo(145, 62)
        handle.lineTo(132, 112)
        handle.quadTo(129, 120, 120, 117)
        handle.lineTo(96, 103)
        handle.closeSubpath()
        painter.drawPath(handle)

        painter.setBrush(QBrush(QColor("#d95763")))
        painter.drawEllipse(QRectF(145, 37, 16, 16))
        painter.setPen(QPen(QColor("#8ed5ff"), 2))
        painter.setBrush(Qt.BrushStyle.NoBrush)
        painter.drawEllipse(QRectF(205, 31, 30, 30))
        painter.drawLine(QPointF(220, 22), QPointF(220, 70))
        painter.drawLine(QPointF(196, 46), QPointF(244, 46))
        painter.setPen(QPen(QColor("#d5dce7")))
        painter.drawText(
            QRectF(12, 122, self.width() - 24, 18),
            Qt.AlignmentFlag.AlignCenter,
            "Mira na tela • Gatilho no mouse ou controle compatível",
        )
        painter.end()


class SmsInputSettingsPage(QWidget):
    """SMS controller/Light Phaser settings page embedded in the input dialog."""

    def __init__(self, settings, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("smsInputSettingsPage")
        root = QVBoxLayout(self)
        root.setContentsMargins(12, 12, 12, 12)
        root.setSpacing(12)

        controller_group = QGroupBox("Layout dos controles — Master System")
        controller_layout = QHBoxLayout(controller_group)
        controller_layout.addWidget(_ControllerLayout(1, controller_group), 1)
        controller_layout.addWidget(_ControllerLayout(2, controller_group), 1)
        root.addWidget(controller_group)

        controls_note = QLabel(
            "Cada controle possui direcional e dois botões de ação (1 e 2). "
            "Os mapeamentos de teclado e gamepad são os definidos nas abas "
            "Jogador 1 e Jogador 2; o botão SW/Hold do perfil SG-1000 não é "
            "um terceiro botão do controle SMS."
        )
        controls_note.setWordWrap(True)
        root.addWidget(controls_note)

        phaser_group = QGroupBox("Light Phaser — mira e gatilho")
        phaser_layout = QHBoxLayout(phaser_group)
        phaser_layout.addWidget(_LightPhaserLayout(phaser_group), 1)
        form = QFormLayout()
        self.phaser_enabled = QCheckBox("Habilitar para jogos compatíveis")
        self.phaser_enabled.setObjectName("smsLightPhaserEnabled")
        self.phaser_enabled.setChecked(
            _setting_bool(settings, "sms_light_phaser_enabled", False)
        )
        form.addRow(self.phaser_enabled)

        self.mouse_aim = QCheckBox("Usar o mouse para apontar na área do jogo")
        self.mouse_aim.setObjectName("smsLightPhaserMouseAim")
        self.mouse_aim.setChecked(
            _setting_bool(settings, "sms_light_phaser_mouse_aim", True)
        )
        form.addRow(self.mouse_aim)

        self.mouse_trigger = QCheckBox("Usar botão esquerdo do mouse como gatilho")
        self.mouse_trigger.setObjectName("smsLightPhaserMouseTrigger")
        self.mouse_trigger.setChecked(
            _setting_bool(settings, "sms_light_phaser_mouse_trigger", True)
        )
        form.addRow(self.mouse_trigger)

        self.right_stick_aim = QCheckBox(
            "Permitir mira pelo analógico direito quando disponível"
        )
        self.right_stick_aim.setObjectName("smsLightPhaserRightStickAim")
        self.right_stick_aim.setChecked(
            _setting_bool(settings, "sms_light_phaser_right_stick_aim", False)
        )
        form.addRow(self.right_stick_aim)

        self.invert_y = QCheckBox("Inverter eixo vertical da mira")
        self.invert_y.setObjectName("smsLightPhaserInvertY")
        self.invert_y.setChecked(
            _setting_bool(settings, "sms_light_phaser_invert_y", False)
        )
        form.addRow(self.invert_y)

        self.show_crosshair = QCheckBox("Mostrar mira na tela")
        self.show_crosshair.setObjectName("smsLightPhaserCrosshair")
        self.show_crosshair.setChecked(
            _setting_bool(settings, "sms_light_phaser_crosshair", True)
        )
        form.addRow(self.show_crosshair)

        self.sensitivity = QSpinBox()
        self.sensitivity.setObjectName("smsLightPhaserSensitivity")
        self.sensitivity.setRange(25, 300)
        self.sensitivity.setSingleStep(25)
        self.sensitivity.setSuffix("%")
        self.sensitivity.setValue(
            _setting_int(settings, "sms_light_phaser_sensitivity", 100, 25, 300)
        )
        form.addRow("Sensibilidade da mira:", self.sensitivity)
        phaser_layout.addLayout(form, 2)
        root.addWidget(phaser_group)

        note = QLabel(
            "Estado de integração: estas preferências são salvas, mas a ponte "
            "Qt → núcleo SMS ainda precisa encaminhar eventos de mouse/eixos. "
            "A detecção real do Light Phaser também depende do VDP informar "
            "um pulso de luz válido no raster; a posição do mouse, sozinha, "
            "não confirma um acerto."
        )
        note.setObjectName("smsInputIntegrationNotice")
        note.setWordWrap(True)
        root.addWidget(note)
        root.addStretch(1)

        self.phaser_enabled.toggled.connect(self._update_enabled_state)
        self.mouse_aim.toggled.connect(
            lambda enabled: self.mouse_trigger.setEnabled(
                enabled and self.phaser_enabled.isChecked()
            )
        )
        self._update_enabled_state(self.phaser_enabled.isChecked())

    def _update_enabled_state(self, enabled: bool) -> None:
        for widget in (
            self.mouse_aim,
            self.mouse_trigger,
            self.right_stick_aim,
            self.invert_y,
            self.show_crosshair,
            self.sensitivity,
        ):
            widget.setEnabled(enabled)
        if enabled:
            self.mouse_trigger.setEnabled(self.mouse_aim.isChecked())

    def save_settings(self, settings) -> None:
        """Persist preferences without claiming the SMS runtime consumes them yet."""
        values = {
            "sms_light_phaser_enabled": self.phaser_enabled.isChecked(),
            "sms_light_phaser_mouse_aim": self.mouse_aim.isChecked(),
            "sms_light_phaser_mouse_trigger": self.mouse_trigger.isChecked(),
            "sms_light_phaser_right_stick_aim": self.right_stick_aim.isChecked(),
            "sms_light_phaser_invert_y": self.invert_y.isChecked(),
            "sms_light_phaser_crosshair": self.show_crosshair.isChecked(),
            "sms_light_phaser_sensitivity": self.sensitivity.value(),
        }
        for key, value in values.items():
            settings.setValue(key, value)


def _setting_bool(settings, key: str, default: bool) -> bool:
    value = settings.value(key, default)
    if isinstance(value, bool):
        return value
    if isinstance(value, int) and value in (0, 1):
        return bool(value)
    if isinstance(value, str):
        normalized = value.strip().casefold()
        if normalized in {"true", "1", "yes", "on"}:
            return True
        if normalized in {"false", "0", "no", "off"}:
            return False
    return default


def _setting_int(settings, key: str, default: int, minimum: int, maximum: int) -> int:
    try:
        value = int(settings.value(key, default))
    except TypeError, ValueError, OverflowError:
        return default
    return value if minimum <= value <= maximum else default
