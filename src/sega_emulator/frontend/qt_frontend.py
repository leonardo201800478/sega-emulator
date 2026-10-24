"""Interface experimental Qt 6/PySide6 para menus e diagnóstico do SG-1000."""

from __future__ import annotations

import logging
from pathlib import Path

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import LoadedRom, list_rom_members, load_rom

logger = logging.getLogger(__name__)


def run(initial_path: Path | None = None) -> None:
    """Abre janela com menus Arquivo, Emulação, Vídeo e Ajuda.

    A interface é um protótipo de teste; a renderização continua em QImage
    software nesta etapa. O backend D3D11 será adicionado depois de validar
    o caminho de ROM, os eventos da janela e a telemetria.
    """
    try:
        from PySide6.QtCore import Qt, QTimer
        from PySide6.QtGui import QAction, QImage, QKeySequence, QPixmap
        from PySide6.QtWidgets import (
            QApplication, QFileDialog, QLabel, QMainWindow, QMessageBox,
            QStatusBar, QVBoxLayout, QWidget, QInputDialog,
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
    timer = QTimer(window)
    timer.setInterval(16)

    def show_frame() -> None:
        nonlocal frame_count
        if machine is None or paused:
            return
        try:
            machine.run_frame()
            frame_count += 1
            image = QImage(
                machine.vdp.framebuffer, 256, 192, 256 * 3,
                QImage.Format.Format_RGB888,
            ).copy()
            view.setPixmap(QPixmap.fromImage(image).scaled(
                view.size(), Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.FastTransformation,
            ))
            status.showMessage(
                f"{loaded.display_name if loaded else 'Sem ROM'} | "
                f"Quadro {frame_count} | PC={machine.cpu.pc:04X} | "
                f"T-states={machine.cpu.total_cycles}"
            )
            if frame_count % 60 == 0:
                logger.info(
                    "Resumo de execução",
                    extra={"event": "frame_checkpoint", "frame": frame_count,
                           "pc": machine.cpu.pc, "instructions": machine.cpu.total_cycles,
                           "tstates": machine.cpu.total_cycles,
                           "scanline": machine.vdp.scanline, "status": machine.vdp.status},
                )
        except Exception:
            logger.exception(
                "Falha durante execução de quadro",
                extra={"event": "emulation_exception", "frame": frame_count,
                       "pc": machine.cpu.pc if machine else None},
            )
            timer.stop()
            QMessageBox.critical(window, "Erro de emulação",
                                 "A emulação foi interrompida. Consulte o log da sessão.")

    def open_rom(path: str | None = None) -> None:
        nonlocal machine, loaded, frame_count, paused
        if not path:
            path, _ = QFileDialog.getOpenFileName(
                window, "Abrir ROM SG-1000", str(Path.cwd()),
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
                    window, "Selecionar ROM do ZIP",
                    "O arquivo contém mais de uma ROM. Escolha uma:", members, 0, False,
                )
                if not accepted:
                    return
                candidate = load_rom(path, member_name=selected)
            loaded_machine = SG1000Machine(candidate.data)
        except Exception as exc:
            logger.exception("Falha ao carregar ROM", extra={
                "event": "rom_load_failed", "rom_path": path,
            })
            QMessageBox.warning(window, "Não foi possível abrir a ROM", str(exc))
            return
        machine, loaded, frame_count, paused = loaded_machine, candidate, 0, False
        logger.info("ROM carregada", extra={
            "event": "rom_loaded", "rom_path": str(candidate.source_path),
            "rom_member": candidate.member_name, "rom_sha256": candidate.sha256,
        })
        status.showMessage(
            f"ROM: {candidate.display_name} | SHA-256: {candidate.sha256[:16]}…"
        )
        timer.start()

    def choose_zip_member() -> None:
        QMessageBox.information(
            window, "ROMs em ZIP",
            "Se o ZIP tiver mais de uma ROM, selecione a ROM desejada no diálogo "
            "de seleção de conteúdo (o suporte de seleção múltipla será refinado).",
        )

    def pause_resume() -> None:
        nonlocal paused
        paused = not paused
        status.showMessage("Pausado" if paused else "Emulação em execução")
        logger.info("Estado de pausa alterado", extra={
            "event": "pause_changed", "paused": paused,
        })

    def reset_machine() -> None:
        nonlocal machine, frame_count, paused
        if loaded is None:
            return
        machine = SG1000Machine(loaded.data)
        frame_count, paused = 0, False
        logger.info("Máquina reinicializada", extra={
            "event": "machine_reset", "rom_sha256": loaded.sha256,
        })

    file_menu = window.menuBar().addMenu("&Arquivo")
    open_action = QAction("Abrir ROM…", window)
    open_action.setShortcut(QKeySequence.StandardKey.Open)
    open_action.triggered.connect(lambda: open_rom())
    file_menu.addAction(open_action)
    file_menu.addAction("Selecionar ROM dentro de ZIP…", choose_zip_member)
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

    video_menu = window.menuBar().addMenu("&Vídeo")
    video_menu.addAction("Escala 2×", lambda: window.resize(600, 450))
    video_menu.addAction("Escala 3×", lambda: window.resize(800, 620))
    video_menu.addAction("Escala 4×", lambda: window.resize(1050, 800))

    tools_menu = window.menuBar().addMenu("&Diagnóstico")
    tools_menu.addAction("Abrir pasta de logs", lambda: QMessageBox.information(
        window, "Logs", f"Os logs estão na pasta: {Path('logs').resolve()}"
    ))
    tools_menu.addAction("Estado da máquina", lambda: QMessageBox.information(
        window, "Estado", (
            "Nenhuma ROM carregada." if machine is None else
            f"ROM: {loaded.display_name}\nPC: {machine.cpu.pc:04X}\n"
            f"T-states: {machine.cpu.total_cycles}\n"
            f"Quadro: {machine.vdp.frame_number}\nLinha: {machine.vdp.scanline}"
        )
    ))
    window.menuBar().addMenu("&Ajuda").addAction(
        "Sobre", lambda: QMessageBox.about(
            window, "Sobre", "Sega Emulator SG-1000 — protótipo experimental. "
            "Compatibilidade real e áudio ainda não validados."
        )
    )
    timer.timeout.connect(show_frame)
    if initial_path is not None:
        open_rom(str(initial_path))
    window.show()
    app.exec()
