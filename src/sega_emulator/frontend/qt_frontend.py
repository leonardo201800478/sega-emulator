"""Interface experimental Qt 6/PySide6 para menus e diagnóstico do SG-1000."""

from __future__ import annotations

import hashlib
import importlib
import json
import logging
import time
from array import array
from pathlib import Path

from sega_emulator.frontend.controller_input import (
    ACTIONS,
    DEFAULT_GAMEPAD_BINDINGS,
    JOYSTICK_ACTIONS,
    SUPPORTED_BUTTONS,
    ControllerInput,
    ControllerInputConfig,
    PlayerInputProfile,
    apply_turbo,
    joystick_value,
)
from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import LoadedRom, list_rom_members, load_rom

logger = logging.getLogger(__name__)

# Clock nominal do Z80 no SG-1000 NTSC: 10,738635 MHz / 3.
Z80_CLOCK_HZ = 3_579_545


def _initialize_pygame_backend() -> tuple[object | None, object | None]:
    """Inicializa entrada SDL independentemente da disponibilidade do áudio."""
    try:
        import pygame as pygame_backend
    except Exception as exc:
        logger.warning("Backend Pygame indisponível: %s", exc)
        return None, None

    audio_channel = None
    try:
        pygame_backend.mixer.pre_init(
            frequency=44_100, size=-16, channels=1, buffer=512
        )
        pygame_backend.mixer.init()
        audio_channel = pygame_backend.mixer.Channel(0)
    except Exception as exc:
        # A falha do mixer não pode desativar joystick/controller do mesmo SDL.
        logger.warning("Saída de áudio indisponível: %s", exc)

    return pygame_backend, audio_channel


def run(
    initial_path: Path | None = None,
    cpu_core: str | None = None,
    vdp_core: str | None = None,
) -> None:
    """Abre a interface Qt e permite selecionar backends experimentais."""
    try:
        from PySide6.QtCore import QEvent, QObject, QSettings, Qt, QTimer
        from PySide6.QtGui import QAction, QActionGroup, QImage, QKeySequence, QPixmap
        from PySide6.QtWidgets import (
            QApplication,
            QCheckBox,
            QComboBox,
            QDialog,
            QDialogButtonBox,
            QDoubleSpinBox,
            QFileDialog,
            QFormLayout,
            QFrame,
            QHBoxLayout,
            QInputDialog,
            QLabel,
            QMainWindow,
            QMenu,
            QMessageBox,
            QPushButton,
            QScrollArea,
            QStatusBar,
            QTabWidget,
            QVBoxLayout,
            QWidget,
        )
    except ImportError as exc:
        raise RuntimeError(
            'Interface Qt não instalada. Execute: python -m pip install -e ".[qt]"'
        ) from exc

    pygame, audio_channel = _initialize_pygame_backend()

    app = QApplication.instance() or QApplication([])
    app.setOrganizationName("SegaEmulator")
    app.setApplicationName("SegaEmulator")
    settings = QSettings()
    selected_cpu_core = cpu_core or settings.value("cpu_core", "python", type=str)
    selected_vdp_core = vdp_core or settings.value("vdp_core", "python", type=str)
    if selected_cpu_core not in ("python", "rust"):
        selected_cpu_core = "python"
    if selected_vdp_core not in ("python", "rust"):
        selected_vdp_core = "python"
    serialized_controller_config = settings.value("controller_config", "", type=str)
    try:
        controller_config = (
            ControllerInputConfig.from_json(serialized_controller_config)
            if serialized_controller_config
            else ControllerInputConfig()
        )
    except ValueError:
        logger.warning("Configuração de controles inválida; usando padrões.")
        controller_config = ControllerInputConfig()
    controller_input = ControllerInput(pygame, controller_config)

    window = QMainWindow()
    window.setWindowTitle("Sega Emulator — SG-1000")
    window.resize(1024, 768)
    window.setMinimumSize(640, 480)
    window.setStyleSheet(
        "QMainWindow { background: #17191f; color: #e8eaf0; }"
        "QMenuBar, QMenu { background: #222630; color: #e8eaf0; }"
        "QMenuBar::item:selected, QMenu::item:selected { background: #39445a; }"
        "QToolBar { background: #222630; border: 0; spacing: 8px; padding: 6px; }"
        "QStatusBar { background: #222630; color: #b9c2d0; }"
        "QLabel { color: #e8eaf0; }"
        "QFrame#screenFrame { background: #090a0d; border: 1px solid #3b4252; "
        "border-radius: 8px; }"
        "QPushButton { padding: 6px 12px; }"
    )
    view = QLabel("Abra uma ROM SG-1000 pelo menu Arquivo")
    view.setAlignment(Qt.AlignmentFlag.AlignCenter)
    view.setMinimumSize(256, 192)
    view.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
    view.setStyleSheet("background: #090a0d; color: #9aa4b2; border: 0;")
    screen_frame = QFrame()
    screen_frame.setObjectName("screenFrame")
    screen_layout = QVBoxLayout(screen_frame)
    screen_layout.setContentsMargins(8, 8, 8, 8)
    screen_layout.addWidget(view)
    central = QWidget()
    layout = QVBoxLayout(central)
    layout.setContentsMargins(16, 16, 16, 16)
    layout.setSpacing(12)
    layout.addWidget(screen_frame)
    window.setCentralWidget(central)
    status = QStatusBar()
    window.setStatusBar(status)

    machine: SG1000Machine | None = None
    loaded: LoadedRom | None = None
    paused = False
    frame_count = 0
    pressed_keys: set[int] = set()
    hold_was_pressed = False
    timer = QTimer(window)
    # Executa tão rápido quanto possível; mede-se a velocidade real.
    timer.setInterval(0)

    def build_key_map() -> dict[int, tuple[int, str]]:
        result: dict[int, tuple[int, str]] = {}
        for player in (1, 2):
            for action, key_name in controller_input.config.players[
                player
            ].keyboard_bindings.items():
                sequence = QKeySequence.fromString(
                    key_name, QKeySequence.SequenceFormat.PortableText
                )
                if not sequence.isEmpty():
                    result[int(sequence[0].toCombined())] = (player, action)
        return result

    key_map = build_key_map()

    def update_joystick() -> None:
        nonlocal hold_was_pressed
        if machine is None:
            hold_was_pressed = False
            return
        actions = {1: set(), 2: set()}
        for key in pressed_keys:
            binding = key_map.get(key)
            if binding is not None:
                player, action = binding
                actions[player].add(action)
        gamepad_actions = controller_input.poll()
        for player in (1, 2):
            actions[player].update(gamepad_actions[player])
            profile = controller_input.config.players[player]
            actions[player] = apply_turbo(
                actions[player], profile.turbo_buttons, frame_count
            )
            machine.set_joystick(
                player, joystick_value(actions[player].intersection(JOYSTICK_ACTIONS))
            )
        hold_pressed = any("hold" in actions[player] for player in (1, 2))
        if hold_pressed and not hold_was_pressed:
            # The console Hold switch is an edge-triggered NMI, not a joystick bit.
            machine.cpu.request_nmi()
        hold_was_pressed = hold_pressed

    class KeyboardInputFilter(QObject):
        """Converte eventos Qt em bits de joystick SG-1000 ativos em nível baixo."""

        def eventFilter(self, watched: QObject, event: QEvent) -> bool:
            if event.type() == QEvent.Type.KeyPress:
                key_event = event
                key = key_event.key()
                if key == Qt.Key.Key_Escape:
                    window.close()
                    return True
                if key in key_map:
                    pressed_keys.add(key)
                    update_joystick()
                    return True
            elif event.type() == QEvent.Type.KeyRelease:
                key = event.key()
                if key in key_map:
                    pressed_keys.discard(key)
                    update_joystick()
                    return True
            return False

    keyboard_filter = KeyboardInputFilter(window)
    window.installEventFilter(keyboard_filter)
    view.installEventFilter(keyboard_filter)

    last_checkpoint_time = time.perf_counter()
    last_checkpoint_tstates = 0
    last_checkpoint_instructions = 0
    last_checkpoint_frame = 0
    phase_totals_ms = {
        "emulation": 0.0,
        "image_copy": 0.0,
        "scale": 0.0,
        "set_pixmap": 0.0,
        "total_frame": 0.0,
    }
    phase_max_ms = dict.fromkeys(phase_totals_ms, 0.0)

    def show_frame() -> None:
        nonlocal frame_count, last_checkpoint_time, last_checkpoint_tstates
        nonlocal last_checkpoint_instructions, last_checkpoint_frame
        if machine is None or paused:
            return
        try:
            frame_started = time.perf_counter()
            phase_started = frame_started
            update_joystick()
            machine.run_frame()
            samples = machine.drain_audio_samples()
            if samples and audio_channel is not None and pygame is not None:
                pcm = array(
                    "h", (max(-32768, min(32767, round(s * 32767))) for s in samples)
                )
                sound = pygame.mixer.Sound(buffer=pcm.tobytes())
                if audio_channel.get_busy():
                    audio_channel.queue(sound)
                else:
                    audio_channel.play(sound)
            emulation_ms = (time.perf_counter() - phase_started) * 1000
            frame_count += 1

            phase_started = time.perf_counter()
            image = QImage(
                machine.vdp.framebuffer,
                256,
                192,
                256 * 3,
                QImage.Format.Format_RGB888,
            ).copy()
            image_copy_ms = (time.perf_counter() - phase_started) * 1000

            phase_started = time.perf_counter()
            pixmap = QPixmap.fromImage(image).scaled(
                view.size(),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.FastTransformation,
            )
            scale_ms = (time.perf_counter() - phase_started) * 1000

            phase_started = time.perf_counter()
            view.setPixmap(pixmap)
            set_pixmap_ms = (time.perf_counter() - phase_started) * 1000
            total_frame_ms = (time.perf_counter() - frame_started) * 1000

            phase_values = {
                "emulation": emulation_ms,
                "image_copy": image_copy_ms,
                "scale": scale_ms,
                "set_pixmap": set_pixmap_ms,
                "total_frame": total_frame_ms,
            }
            for phase, elapsed_ms in phase_values.items():
                phase_totals_ms[phase] += elapsed_ms
                phase_max_ms[phase] = max(phase_max_ms[phase], elapsed_ms)

            now = time.perf_counter()
            elapsed = max(now - last_checkpoint_time, 1e-9)
            tstates_delta = machine.total_tstates - last_checkpoint_tstates
            fps = (frame_count - last_checkpoint_frame) / elapsed
            speed_percent = (tstates_delta / elapsed / Z80_CLOCK_HZ) * 100
            status.showMessage(
                f"{loaded.display_name if loaded else 'Sem ROM'} | "
                f"Quadro {frame_count} | {fps:.1f} FPS | "
                f"Velocidade {speed_percent:.1f}% | "
                f"EMU {emulation_ms:.1f} ms | QImage {image_copy_ms:.2f} ms | "
                f"escala {scale_ms:.2f} ms | UI {set_pixmap_ms:.2f} ms | "
                f"PC={machine.cpu.pc:04X}"
            )

            if frame_count - last_checkpoint_frame >= 60:
                measured_frames = frame_count - last_checkpoint_frame
                phase_averages = {
                    phase: round(total / measured_frames, 3)
                    for phase, total in phase_totals_ms.items()
                }
                phase_maxima = {
                    phase: round(value, 3) for phase, value in phase_max_ms.items()
                }
                logger.info(
                    "Resumo de desempenho",
                    extra={
                        "event": "frame_checkpoint",
                        "frame": frame_count,
                        "pc": machine.cpu.pc,
                        "instructions": machine.total_instructions
                        - last_checkpoint_instructions,
                        "instructions_total": machine.total_instructions,
                        "tstates": machine.total_tstates,
                        "tstates_delta": tstates_delta,
                        "elapsed_ms": round(elapsed * 1000, 3),
                        "fps": round(fps, 3),
                        "speed_percent": round(speed_percent, 3),
                        "phase_avg_ms": phase_averages,
                        "phase_max_ms": phase_maxima,
                        "scanline": machine.vdp.scanline,
                        "status": machine.vdp.status,
                    },
                )
                last_checkpoint_time = now
                last_checkpoint_tstates = machine.total_tstates
                last_checkpoint_instructions = machine.total_instructions
                last_checkpoint_frame = frame_count
                for phase in phase_totals_ms:
                    phase_totals_ms[phase] = 0.0
                    phase_max_ms[phase] = 0.0
        except Exception:
            logger.exception(
                "Falha durante execução de quadro",
                extra={
                    "event": "emulation_exception",
                    "frame": frame_count,
                    "pc": machine.cpu.pc if machine else None,
                },
            )
            timer.stop()
            QMessageBox.critical(
                window,
                "Erro de emulação",
                "A emulação foi interrompida. Consulte o log da sessão.",
            )

    recent_rom_menu: QMenu | None = None

    def refresh_recent_rom_menu() -> None:
        if recent_rom_menu is None:
            return
        recent_rom_menu.clear()
        recent_paths = settings.value("recent_roms", [], type=list)
        valid_paths = [str(path) for path in recent_paths if Path(str(path)).is_file()]
        if not valid_paths:
            action = recent_rom_menu.addAction("(Nenhuma ROM recente)")
            action.setEnabled(False)
        else:
            for recent_path in valid_paths:
                action = recent_rom_menu.addAction(Path(recent_path).name)
                action.setToolTip(recent_path)
                action.triggered.connect(
                    lambda checked=False, selected_path=recent_path: open_rom(
                        selected_path
                    )
                )
            recent_rom_menu.addSeparator()
            clear_action = recent_rom_menu.addAction("Limpar histórico")
            clear_action.triggered.connect(clear_recent_roms)

    def clear_recent_roms() -> None:
        settings.setValue("recent_roms", [])
        refresh_recent_rom_menu()

    def open_rom(path: str | None = None) -> None:
        nonlocal machine, loaded, frame_count, paused
        nonlocal last_checkpoint_time, last_checkpoint_tstates
        nonlocal last_checkpoint_instructions, last_checkpoint_frame
        if not path:
            path, _ = QFileDialog.getOpenFileName(
                window,
                "Abrir ROM SG-1000",
                str(settings.value("last_directory", str(Path.cwd()))),
                "ROMs SG-1000 (*.sg *.zip *.rom *.bin);;Todos os arquivos (*)",
            )
        if not path:
            return
        try:
            try:
                candidate = load_rom(path)
            except ValueError as error:
                if Path(path).suffix.lower() != ".zip" or "várias ROMs" not in str(
                    error
                ):
                    raise
                members = list_rom_members(path)
                selected, accepted = QInputDialog.getItem(
                    window,
                    "Selecionar ROM do ZIP",
                    "O arquivo contém mais de uma ROM. Escolha uma:",
                    members,
                    0,
                    False,
                )
                if not accepted:
                    return
                candidate = load_rom(path, member_name=selected)
            loaded_machine = SG1000Machine(
                candidate.data,
                cpu_core=selected_cpu_core,
                vdp_core=selected_vdp_core,
            )
        except Exception as exc:
            logger.exception(
                "Falha ao carregar ROM",
                extra={"event": "rom_load_failed", "rom_path": path},
            )
            QMessageBox.warning(window, "Não foi possível abrir a ROM", str(exc))
            return

        machine, loaded, frame_count, paused = loaded_machine, candidate, 0, False
        selected_path = str(Path(path).expanduser().resolve())
        settings.setValue("last_directory", str(Path(selected_path).parent))
        settings.setValue("last_rom", selected_path)
        recent_paths = settings.value("recent_roms", [], type=list)
        recent_paths = [
            str(item) for item in recent_paths if str(item) != selected_path
        ]
        recent_paths.insert(0, selected_path)
        settings.setValue("recent_roms", recent_paths[:10])
        refresh_recent_rom_menu()
        pressed_keys.clear()
        update_joystick()
        last_checkpoint_time = time.perf_counter()
        last_checkpoint_tstates = 0
        last_checkpoint_instructions = 0
        last_checkpoint_frame = 0
        for phase in phase_totals_ms:
            phase_totals_ms[phase] = 0.0
            phase_max_ms[phase] = 0.0
        logger.info(
            "ROM carregada",
            extra={
                "event": "rom_loaded",
                "rom_path": str(candidate.source_path),
                "rom_member": candidate.member_name,
                "rom_sha256": candidate.sha256,
            },
        )
        status.showMessage(
            f"ROM: {candidate.display_name} | SHA-256: {candidate.sha256[:16]}…"
        )
        view.setFocus()
        timer.start()

    def pause_resume() -> None:
        nonlocal paused
        paused = not paused
        status.showMessage("Pausado" if paused else "Emulação em execução")
        logger.info(
            "Estado de pausa alterado",
            extra={"event": "pause_changed", "paused": paused},
        )

    def reset_machine() -> None:
        nonlocal machine, frame_count, paused
        nonlocal last_checkpoint_time, last_checkpoint_tstates
        nonlocal last_checkpoint_instructions, last_checkpoint_frame
        if loaded is None:
            return
        machine = SG1000Machine(
            loaded.data,
            cpu_core=selected_cpu_core,
            vdp_core=selected_vdp_core,
        )
        frame_count, paused = 0, False
        pressed_keys.clear()
        update_joystick()
        last_checkpoint_time = time.perf_counter()
        last_checkpoint_tstates = 0
        last_checkpoint_instructions = 0
        last_checkpoint_frame = 0
        for phase in phase_totals_ms:
            phase_totals_ms[phase] = 0.0
            phase_max_ms[phase] = 0.0
        logger.info(
            "Máquina reinicializada",
            extra={"event": "machine_reset", "rom_sha256": loaded.sha256},
        )

    class KeyCaptureButton(QPushButton):
        """Botão que captura uma tecla simples para o mapeamento do teclado."""

        def __init__(self, key_name: str) -> None:
            super().__init__(key_name)
            self.key_name = key_name
            self._capturing = False
            self._previous_key_name = key_name
            self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
            self.setToolTip("Clique e pressione a tecla desejada.")
            self.clicked.connect(self._begin_capture)

        def _begin_capture(self) -> None:
            self._previous_key_name = self.key_name
            self._capturing = True
            self.setText("Pressione uma tecla…")
            self.setFocus()

        def keyPressEvent(self, event: QEvent) -> None:
            if not self._capturing:
                super().keyPressEvent(event)
                return
            key = event.key()
            if key == Qt.Key.Key_Escape:
                self._capturing = False
                self.setText(self._previous_key_name)
                event.accept()
                return
            if key in (
                Qt.Key.Key_Control,
                Qt.Key.Key_Shift,
                Qt.Key.Key_Alt,
                Qt.Key.Key_Meta,
            ):
                self.setText("Pressione uma tecla simples…")
                event.accept()
                return
            key_name = QKeySequence(key).toString(
                QKeySequence.SequenceFormat.PortableText
            )
            if not key_name:
                self.setText("Tecla inválida; tente novamente")
                event.accept()
                return
            self.key_name = key_name
            self._capturing = False
            self.setText(key_name)
            event.accept()

    def configure_controllers() -> None:
        dialog = QDialog(window)
        dialog.setWindowTitle("Configuração de controles")
        dialog.resize(680, 640)
        dialog_layout = QVBoxLayout(dialog)
        tabs = QTabWidget()
        dialog_layout.addWidget(tabs)
        combos: dict[tuple[int, str], QComboBox] = {}
        gamepad_capture_buttons: dict[tuple[int, str], QPushButton] = {}
        key_buttons: dict[tuple[int, str], QPushButton] = {}
        stick_checks: dict[int, QCheckBox] = {}
        turbo_checks: dict[tuple[int, str], QCheckBox] = {}
        capture_target: tuple[int, str] | None = None
        capture_timer = QTimer(dialog)
        capture_timer.setInterval(30)
        action_labels = {
            "up": "Cima",
            "down": "Baixo",
            "left": "Esquerda",
            "right": "Direita",
            "button1": "Botão 1",
            "button2": "Botão 2",
            "hold": "SW / Hold (NMI)",
        }
        button_labels = {
            "a": "A (sul)",
            "b": "B (leste)",
            "x": "X (oeste)",
            "y": "Y (norte)",
            "dpad_up": "D-pad cima",
            "dpad_down": "D-pad baixo",
            "dpad_left": "D-pad esquerda",
            "dpad_right": "D-pad direita",
            "leftshoulder": "Ombro esquerdo",
            "rightshoulder": "Ombro direito",
            "leftstick": "Clique analógico esquerdo",
            "rightstick": "Clique analógico direito",
            "back": "Voltar/Select",
            "guide": "Guia/Home",
            "start": "Start / Hold",
            "misc1": "Botão auxiliar",
            "paddle1": "Paddle 1",
            "paddle2": "Paddle 2",
            "paddle3": "Paddle 3",
            "paddle4": "Paddle 4",
            "touchpad": "Touchpad",
        }

        def poll_gamepad_capture() -> None:
            nonlocal capture_target
            if capture_target is None:
                capture_timer.stop()
                return
            player, action = capture_target
            button_name = controller_input.read_pressed_button(player)
            if button_name is None:
                return
            combo = combos[(player, action)]
            button_index = combo.findData(button_name)
            if button_index < 0:
                display_name = button_labels.get(button_name, button_name)
                combo.addItem(display_name, button_name)
                button_index = combo.findData(button_name)
            combo.setCurrentIndex(button_index)
            gamepad_capture_buttons[(player, action)].setText("Gravar")
            status.showMessage(
                f"Controle do Jogador {player}: "
                f"{action_labels[action]} = "
                f"{button_labels.get(button_name, button_name)}"
            )
            capture_target = None
            capture_timer.stop()

        def begin_gamepad_capture(player: int, action: str) -> None:
            nonlocal capture_target
            capture_target = (player, action)
            gamepad_capture_buttons[(player, action)].setText("Pressione…")
            status.showMessage(
                f"Pressione o botão físico para {action_labels[action]} "
                f"no controle do Jogador {player}."
            )
            capture_timer.start()

        capture_timer.timeout.connect(poll_gamepad_capture)

        for player in (1, 2):
            page = QWidget()
            form = QFormLayout(page)
            profile = controller_input.config.players[player]
            for action in ACTIONS:
                combo = QComboBox()
                for button in SUPPORTED_BUTTONS:
                    combo.addItem(button_labels[button], button)
                selected = profile.gamepad_buttons.get(
                    action, DEFAULT_GAMEPAD_BINDINGS[action]
                )
                if combo.findData(selected) < 0:
                    combo.addItem(selected, selected)
                combo.setCurrentIndex(max(0, combo.findData(selected)))
                combos[(player, action)] = combo
                gamepad_row = QWidget()
                gamepad_row_layout = QHBoxLayout(gamepad_row)
                gamepad_row_layout.setContentsMargins(0, 0, 0, 0)
                gamepad_row_layout.addWidget(combo, 1)
                capture_button = QPushButton("Gravar")
                capture_button.setToolTip(
                    "Clique e pressione o botão físico que deseja atribuir."
                )
                gamepad_capture_buttons[(player, action)] = capture_button
                capture_button.clicked.connect(
                    lambda checked=False, p=player, a=action: begin_gamepad_capture(
                        p, a
                    )
                )
                gamepad_row_layout.addWidget(capture_button)
                form.addRow(f"Controle — {action_labels[action]}", gamepad_row)
                key_button = KeyCaptureButton(profile.keyboard_bindings[action])
                key_buttons[(player, action)] = key_button
                form.addRow(f"Teclado — {action_labels[action]}", key_button)
            turbo1 = QCheckBox("Turbo no Botão 1 (A por padrão)")
            turbo1.setChecked(profile.turbo_buttons.get("button1", False))
            turbo_checks[(player, "button1")] = turbo1
            form.addRow("", turbo1)
            turbo2 = QCheckBox("Turbo no Botão 2 (B por padrão)")
            turbo2.setChecked(profile.turbo_buttons.get("button2", False))
            turbo_checks[(player, "button2")] = turbo2
            form.addRow("", turbo2)
            stick_check = QCheckBox("Usar analógico esquerdo como direcional")
            stick_check.setChecked(profile.use_left_stick)
            stick_checks[player] = stick_check
            form.addRow("", stick_check)
            scroll = QScrollArea()
            scroll.setWidgetResizable(True)
            scroll.setWidget(page)
            tabs.addTab(scroll, f"Jogador {player}")
        deadzone = QDoubleSpinBox()
        deadzone.setRange(0.05, 0.80)
        deadzone.setSingleStep(0.05)
        deadzone.setDecimals(2)
        deadzone.setValue(controller_input.config.deadzone)
        dialog_layout.addWidget(QLabel("Zona morta do analógico"))
        dialog_layout.addWidget(deadzone)
        connected = controller_input.connected_names()
        dialog_layout.addWidget(
            QLabel(
                "Controles detectados: "
                + (", ".join(connected) if connected else "nenhum controle compatível")
            )
        )
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel
        )
        buttons.accepted.connect(dialog.accept)
        buttons.rejected.connect(dialog.reject)
        dialog_layout.addWidget(buttons)
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return
        profiles: dict[int, PlayerInputProfile] = {}
        seen_keyboard_keys: dict[int, tuple[int, str]] = {}
        for player in (1, 2):
            keyboard_bindings = {
                action: key_buttons[(player, action)].key_name for action in ACTIONS
            }
            for action, key_name in keyboard_bindings.items():
                sequence = QKeySequence.fromString(
                    key_name, QKeySequence.SequenceFormat.PortableText
                )
                if sequence.isEmpty():
                    QMessageBox.warning(
                        dialog,
                        "Mapeamento inválido",
                        f"A tecla configurada para {action_labels[action]} é inválida.",
                    )
                    return
                key_code = int(sequence[0].toCombined())
                previous = seen_keyboard_keys.get(key_code)
                if previous is not None:
                    previous_player, previous_action = previous
                    QMessageBox.warning(
                        dialog,
                        "Mapeamento inválido",
                        f"A tecla {key_name} está atribuída a "
                        f"{action_labels[previous_action]} "
                        f"do Jogador {previous_player} "
                        f"e a {action_labels[action]} "
                        f"do Jogador {player}.",
                    )
                    return
                seen_keyboard_keys[key_code] = (player, action)
            profiles[player] = PlayerInputProfile(
                gamepad_buttons={
                    action: str(combos[(player, action)].currentData())
                    for action in ACTIONS
                },
                use_left_stick=stick_checks[player].isChecked(),
                keyboard_bindings=keyboard_bindings,
                turbo_buttons={
                    action: turbo_checks[(player, action)].isChecked()
                    for action in ("button1", "button2")
                },
            )
        controller_input.config = ControllerInputConfig(
            players=profiles, deadzone=deadzone.value()
        )
        key_map.clear()
        key_map.update(build_key_map())
        settings.setValue("controller_config", controller_input.config.to_json())
        update_joystick()
        status.showMessage("Mapeamento de controles salvo.")

    file_menu = window.menuBar().addMenu("&Arquivo")
    open_action = QAction("Abrir ROM…", window)
    open_action.setShortcut(QKeySequence.StandardKey.Open)
    open_action.triggered.connect(lambda: open_rom())
    file_menu.addAction(open_action)
    recent_rom_menu = file_menu.addMenu("ROMs recentes")
    refresh_recent_rom_menu()
    file_menu.addSeparator()
    file_menu.addAction("Sair", window.close)

    emu_menu = window.menuBar().addMenu("&Emulação")
    pause_action = QAction("Pausar/Continuar", window)
    pause_action.setShortcut("F5")
    pause_action.triggered.connect(pause_resume)
    emu_menu.addAction(pause_action)
    reset_action = QAction("Reiniciar máquina", window)
    reset_action.setShortcut("F10")
    reset_action.triggered.connect(reset_machine)
    emu_menu.addAction(reset_action)

    cpu_menu = emu_menu.addMenu("Núcleo da CPU")
    cpu_group = QActionGroup(window)
    cpu_group.setExclusive(True)
    cpu_actions: dict[str, QAction] = {}
    for core, label in (
        ("python", "Python (referência)"),
        ("rust", "Rust (experimental)"),
        ("llvm", "LLVM ORC (protótipo não integrado)"),
    ):
        action = QAction(label, window)
        action.setCheckable(core != "llvm")
        action.setChecked(core == selected_cpu_core)
        if core == "llvm":
            action.setEnabled(False)
            action.setToolTip(
                "O protótipo LLVM ORC ainda não está integrado ao ciclo da máquina."
            )
        else:
            cpu_group.addAction(action)
            action.triggered.connect(
                lambda checked=False, selected=core: select_core("cpu", selected)
            )
        cpu_menu.addAction(action)
        cpu_actions[core] = action

    vdp_menu = emu_menu.addMenu("Núcleo do VDP")
    vdp_group = QActionGroup(window)
    vdp_group.setExclusive(True)
    vdp_actions: dict[str, QAction] = {}
    for core, label in (
        ("python", "Python (referência)"),
        ("rust", "Rust (experimental)"),
    ):
        action = QAction(label, window)
        action.setCheckable(True)
        action.setChecked(core == selected_vdp_core)
        vdp_group.addAction(action)
        action.triggered.connect(
            lambda checked=False, selected=core: select_core("vdp", selected)
        )
        vdp_menu.addAction(action)
        vdp_actions[core] = action

    def select_core(kind: str, core: str) -> None:
        nonlocal selected_cpu_core, selected_vdp_core
        current = selected_cpu_core if kind == "cpu" else selected_vdp_core
        if core == current:
            return

        if core == "rust":
            symbol = "RustZ80Cpu" if kind == "cpu" else "RustTMS9918A"
            try:
                getattr(importlib.import_module("sega_z80_core"), symbol)
            except (ImportError, AttributeError):
                QMessageBox.warning(
                    window,
                    "Backend Rust indisponível",
                    f"O backend {kind.upper()} Rust não está instalado nesta "
                    "versão da extensão. A seleção não foi alterada.",
                )
                actions = cpu_actions if kind == "cpu" else vdp_actions
                for name, action in actions.items():
                    action.setChecked(name == current)
                return

        if kind == "cpu":
            selected_cpu_core = core
        else:
            selected_vdp_core = core

        try:
            if loaded is not None:
                reset_machine()
        except Exception as exc:
            if kind == "cpu":
                selected_cpu_core = current
            else:
                selected_vdp_core = current
            actions = cpu_actions if kind == "cpu" else vdp_actions
            for name, action in actions.items():
                action.setChecked(name == current)
            QMessageBox.warning(window, "Não foi possível trocar o backend", str(exc))
            return

        settings.setValue(f"{kind}_core", core)
        logger.info(
            "Backend de emulação selecionado",
            extra={"event": "emulation_backend_selected", f"{kind}_core": core},
        )
        status.showMessage(
            f"Núcleo da {'CPU' if kind == 'cpu' else 'VDP'}: "
            f"{'Python (referência)' if core == 'python' else 'Rust (experimental)'}"
        )

    toolbar = window.addToolBar("Principal")
    toolbar.setMovable(False)
    toolbar.addAction(open_action)
    toolbar.addAction(pause_action)
    toolbar.addAction(reset_action)

    video_menu = window.menuBar().addMenu("&Vídeo")
    video_menu.addAction("Escala 2×", lambda: window.resize(600, 450))
    video_menu.addAction("Escala 3×", lambda: window.resize(800, 620))
    video_menu.addAction("Escala 4×", lambda: window.resize(1050, 800))

    controls_menu = window.menuBar().addMenu("&Controles")
    controls_menu.addAction("Configurar controles…", configure_controllers)

    def dump_vdp_snapshot() -> None:
        """Salva um snapshot pontual do VDP para investigar divergências gráficas."""
        if machine is None or loaded is None:
            QMessageBox.information(
                window, "Snapshot VDP", "Carregue uma ROM primeiro."
            )
            return

        vdp = machine.vdp
        attr_base = (vdp.registers[5] & 0x7F) << 7
        sprites = []
        for index in range(32):
            offset = (attr_base + index * 4) & 0x3FFF
            raw_y, raw_x, pattern, color = vdp.vram[offset : offset + 4]
            sprites.append(
                {
                    "index": index,
                    "vram_address": offset,
                    "raw_y": raw_y,
                    "raw_x": raw_x,
                    "pattern": pattern,
                    "color_byte": color,
                    "color": color & 0x0F,
                    "early_clock": bool(color & 0x80),
                }
            )
            if raw_y == 0xD0:
                break

        try:
            output_dir = Path("logs") / "vdp_snapshots"
            output_dir.mkdir(parents=True, exist_ok=True)
            stem = f"vdp-{vdp.frame_number:06d}-{machine.cpu.pc:04X}-{time.time_ns()}"
            framebuffer = bytes(vdp.framebuffer)
            snapshot = {
                "schema_version": 1,
                "purpose": "visual-divergence-debugging",
                "rom": {
                    "display_name": loaded.display_name,
                    "sha256": loaded.sha256,
                    "member_name": loaded.member_name,
                },
                "machine": {
                    "pc": machine.cpu.pc,
                    "instructions": machine.total_instructions,
                    "tstates": machine.total_tstates,
                    "frame_count_frontend": frame_count,
                },
                "vdp": {
                    "frame_number": vdp.frame_number,
                    "scanline": vdp.scanline,
                    "cycles_into_line": vdp._cycles_into_line,
                    "status": vdp.status,
                    "registers_hex": bytes(vdp.registers).hex(),
                    "display_mode": vdp._display_mode,
                    "display_enabled": bool(vdp.registers[1] & 0x40),
                    "sprite_attribute_table_base": attr_base,
                    "sprite_pattern_table_base": (vdp.registers[6] & 0x07) << 11,
                    "sprite_entries_raw": sprites,
                    "sprite_line_active_at_current_scanline": (
                        vdp._sprite_line_active(vdp.scanline)[0]
                        if vdp.scanline < 262
                        else []
                    ),
                    "vram_hex": bytes(vdp.vram).hex(),
                    "framebuffer_sha256": hashlib.sha256(framebuffer).hexdigest(),
                    "framebuffer_size": len(framebuffer),
                },
            }
            json_path = output_dir / f"{stem}.json"
            image_path = output_dir / f"{stem}.png"
            json_path.write_text(
                json.dumps(snapshot, indent=2, ensure_ascii=False),
                encoding="utf-8",
            )
            image = QImage(
                framebuffer,
                256,
                192,
                256 * 3,
                QImage.Format.Format_RGB888,
            ).copy()
            if not image.save(str(image_path), "PNG"):
                raise OSError(f"Não foi possível salvar a imagem: {image_path}")
        except Exception as exc:
            logger.exception("Falha ao salvar snapshot VDP")
            QMessageBox.critical(window, "Snapshot VDP", str(exc))
            return

        logger.info(
            "Snapshot VDP salvo",
            extra={
                "event": "vdp_snapshot_saved",
                "json_path": str(json_path),
                "image_path": str(image_path),
                "rom_sha256": loaded.sha256,
                "frame": vdp.frame_number,
                "pc": machine.cpu.pc,
            },
        )
        QMessageBox.information(
            window,
            "Snapshot VDP salvo",
            f"JSON: {json_path.resolve()}\\nPNG: {image_path.resolve()}",
        )

    tools_menu = window.menuBar().addMenu("&Diagnóstico")
    snapshot_action = QAction("Capturar snapshot VDP (JSON + PNG)…", window)
    snapshot_action.setShortcut("F8")
    snapshot_action.triggered.connect(dump_vdp_snapshot)
    tools_menu.addAction(snapshot_action)
    tools_menu.addAction(
        "Abrir pasta de logs",
        lambda: QMessageBox.information(
            window, "Logs", f"Os logs estão na pasta: {Path('logs').resolve()}"
        ),
    )
    tools_menu.addAction(
        "Estado da máquina",
        lambda: QMessageBox.information(
            window,
            "Estado",
            (
                "Nenhuma ROM carregada."
                if machine is None
                else f"ROM: {loaded.display_name}\n"
                f"PC: {machine.cpu.pc:04X}\n"
                f"Instruções: {machine.total_instructions}\n"
                f"T-states: {machine.total_tstates}\n"
                f"Quadro: {machine.vdp.frame_number}\n"
                f"Linha: {machine.vdp.scanline}"
            ),
        ),
    )
    window.menuBar().addMenu("&Ajuda").addAction(
        "Sobre",
        lambda: QMessageBox.about(
            window,
            "Sobre",
            "Sega Emulator SG-1000 — protótipo experimental. "
            "Compatibilidade real e áudio ainda não validados.",
        ),
    )

    timer.timeout.connect(show_frame)
    if initial_path is not None:
        open_rom(str(initial_path))
    window.show()
    app.exec()
    controller_input.close()
    if pygame is not None:
        pygame.mixer.quit()
        pygame.quit()
