"""Interface experimental Qt 6/PySide6 para menus e diagnóstico do SG-1000."""

from __future__ import annotations

import logging
import time
from pathlib import Path

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import LoadedRom, list_rom_members, load_rom

logger = logging.getLogger(__name__)

# Clock nominal do Z80 no SG-1000 NTSC: 10,738635 MHz / 3.
Z80_CLOCK_HZ = 3_579_545


def run(initial_path: Path | None = None) -> None:
    """Abre a interface de teste com teclado, telemetria e logs de sessão."""
    try:
        from PySide6.QtCore import QEvent, QObject, Qt, QTimer
        from PySide6.QtGui import QAction, QImage, QKeySequence, QPixmap
        from PySide6.QtWidgets import (
            QApplication,
            QFileDialog,
            QInputDialog,
            QLabel,
            QMainWindow,
            QMessageBox,
            QStatusBar,
            QVBoxLayout,
            QWidget,
        )
    except ImportError as exc:
        raise RuntimeError(
            'Interface Qt não instalada. Execute: python -m pip install -e ".[qt]"'
        ) from exc

    app = QApplication.instance() or QApplication([])
    window = QMainWindow()
    window.setWindowTitle("Sega Emulator — SG-1000 (experimental)")
    window.resize(800, 620)
    view = QLabel("Abra uma ROM SG-1000 pelo menu Arquivo")
    view.setAlignment(Qt.AlignmentFlag.AlignCenter)
    view.setMinimumSize(256, 192)
    view.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
    view.setStyleSheet("background: #202020; color: #dddddd;")
    central = QWidget()
    layout = QVBoxLayout(central)
    layout.addWidget(view)
    window.setCentralWidget(central)
    status = QStatusBar()
    window.setStatusBar(status)

    machine: SG1000Machine | None = None
    loaded: LoadedRom | None = None
    paused = False
    frame_count = 0
    pressed_keys: set[int] = set()
    timer = QTimer(window)
    timer.setInterval(0)  # Executa tão rápido quanto possível; mede-se a velocidade real.

    key_map = {
        Qt.Key.Key_Up: 0,
        Qt.Key.Key_Down: 1,
        Qt.Key.Key_Left: 2,
        Qt.Key.Key_Right: 3,
        Qt.Key.Key_Z: 4,
        Qt.Key.Key_Space: 4,
        Qt.Key.Key_X: 5,
        Qt.Key.Key_Return: 5,
        Qt.Key.Key_Enter: 5,
    }

    def update_joystick() -> None:
        if machine is None:
            return
        value = 0xFF
        for key in pressed_keys:
            bit = key_map.get(key)
            if bit is not None:
                value &= ~(1 << bit)
        machine.set_joystick(1, value)

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

    def show_frame() -> None:
        nonlocal frame_count
        if machine is None or paused:
            return
        try:
            machine.run_frame()
            frame_count += 1
            image = QImage(
                machine.vdp.framebuffer,
                256,
                192,
                256 * 3,
                QImage.Format.Format_RGB888,
            ).copy()
            view.setPixmap(
                QPixmap.fromImage(image).scaled(
                    view.size(),
                    Qt.AspectRatioMode.KeepAspectRatio,
                    Qt.TransformationMode.FastTransformation,
                )
            )

            now = time.perf_counter()
            elapsed = max(now - last_checkpoint_time, 1e-9)
            tstates_delta = machine.total_tstates - last_checkpoint_tstates
            fps = (frame_count - last_checkpoint_frame) / elapsed
            speed_percent = (tstates_delta / elapsed / Z80_CLOCK_HZ) * 100
            status.showMessage(
                f"{loaded.display_name if loaded else 'Sem ROM'} | "
                f"Quadro {frame_count} | {fps:.1f} FPS | "
                f"Velocidade {speed_percent:.1f}% | PC={machine.cpu.pc:04X}"
            )

            if frame_count - last_checkpoint_frame >= 60:
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
                        "scanline": machine.vdp.scanline,
                        "status": machine.vdp.status,
                    },
                )
                last_checkpoint_time = now
                last_checkpoint_tstates = machine.total_tstates
                last_checkpoint_instructions = machine.total_instructions
                last_checkpoint_frame = frame_count
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

    def open_rom(path: str | None = None) -> None:
        nonlocal machine, loaded, frame_count, paused
        nonlocal last_checkpoint_time, last_checkpoint_tstates
        nonlocal last_checkpoint_instructions, last_checkpoint_frame
        if not path:
            path, _ = QFileDialog.getOpenFileName(
                window,
                "Abrir ROM SG-1000",
                str(Path.cwd()),
                "ROMs SG-1000 (*.sg *.zip *.rom *.bin);;Todos os arquivos (*)",
            )
        if not path:
            return
        try:
            try:
                candidate = load_rom(path)
            except ValueError as error:
                if Path(path).suffix.lower() != ".zip" or "várias ROMs" not in str(error):
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
            loaded_machine = SG1000Machine(candidate.data)
        except Exception as exc:
            logger.exception(
                "Falha ao carregar ROM",
                extra={"event": "rom_load_failed", "rom_path": path},
            )
            QMessageBox.warning(window, "Não foi possível abrir a ROM", str(exc))
            return

        machine, loaded, frame_count, paused = loaded_machine, candidate, 0, False
        pressed_keys.clear()
        update_joystick()
        last_checkpoint_time = time.perf_counter()
        last_checkpoint_tstates = 0
        last_checkpoint_instructions = 0
        last_checkpoint_frame = 0
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
        machine = SG1000Machine(loaded.data)
        frame_count, paused = 0, False
        pressed_keys.clear()
        update_joystick()
        last_checkpoint_time = time.perf_counter()
        last_checkpoint_tstates = 0
        last_checkpoint_instructions = 0
        last_checkpoint_frame = 0
        logger.info(
            "Máquina reinicializada",
            extra={"event": "machine_reset", "rom_sha256": loaded.sha256},
        )

    file_menu = window.menuBar().addMenu("&Arquivo")
    open_action = QAction("Abrir ROM…", window)
    open_action.setShortcut(QKeySequence.StandardKey.Open)
    open_action.triggered.connect(lambda: open_rom())
    file_menu.addAction(open_action)
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

    video_menu = window.menuBar().addMenu("&Vídeo")
    video_menu.addAction("Escala 2×", lambda: window.resize(600, 450))
    video_menu.addAction("Escala 3×", lambda: window.resize(800, 620))
    video_menu.addAction("Escala 4×", lambda: window.resize(1050, 800))

    tools_menu = window.menuBar().addMenu("&Diagnóstico")
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
