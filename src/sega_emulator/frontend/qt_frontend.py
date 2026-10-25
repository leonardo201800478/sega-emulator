"""Interface experimental Qt 6/PySide6 para menus e diagnóstico do SG-1000."""

from __future__ import annotations

import hashlib
import json
import logging
import time
from array import array
from pathlib import Path

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import LoadedRom, list_rom_members, load_rom

logger = logging.getLogger(__name__)

# Clock nominal do Z80 no SG-1000 NTSC: 10,738635 MHz / 3.
Z80_CLOCK_HZ = 3_579_545


def run(initial_path: Path | None = None, cpu_core: str | None = None) -> None:
    """Abre a interface de teste com teclado, telemetria e logs de sessão."""
    try:
        from PySide6.QtCore import QEvent, QObject, QSettings, Qt, QTimer
        from PySide6.QtGui import QAction, QActionGroup, QImage, QKeySequence, QPixmap
        from PySide6.QtWidgets import (
            QApplication,
            QFileDialog,
            QInputDialog,
            QLabel,
            QMainWindow,
            QMenu,
            QMessageBox,
            QStatusBar,
            QVBoxLayout,
            QWidget,
        )
    except ImportError as exc:
        raise RuntimeError(
            'Interface Qt não instalada. Execute: python -m pip install -e ".[qt]"'
        ) from exc

    pygame = None
    audio_channel = None
    try:
        import pygame as pygame_backend

        pygame_backend.mixer.pre_init(
            frequency=44_100, size=-16, channels=1, buffer=512
        )
        pygame_backend.mixer.init()
        audio_channel = pygame_backend.mixer.Channel(0)
        pygame = pygame_backend
    except Exception as exc:
        logger.warning("Saída de áudio indisponível: %s", exc)

    app = QApplication.instance() or QApplication([])
    app.setOrganizationName("SegaEmulator")
    app.setApplicationName("SegaEmulator")
    settings = QSettings()
    selected_cpu_core = cpu_core or settings.value("cpu_core", "python", type=str)
    if selected_cpu_core not in ("python", "rust"):
        selected_cpu_core = "python"
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
    # Executa tão rápido quanto possível; mede-se a velocidade real.
    timer.setInterval(0)

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
            loaded_machine = SG1000Machine(candidate.data, cpu_core=selected_cpu_core)
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
        machine = SG1000Machine(loaded.data, cpu_core=selected_cpu_core)
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

    core_menu = emu_menu.addMenu("Núcleo da CPU")
    core_group = QActionGroup(window)
    core_group.setExclusive(True)
    python_core_action = QAction("Python (referência)", window)
    python_core_action.setCheckable(True)
    python_core_action.setChecked(selected_cpu_core == "python")
    rust_core_action = QAction("Rust (experimental)", window)
    rust_core_action.setCheckable(True)
    rust_core_action.setChecked(selected_cpu_core == "rust")
    core_group.addAction(python_core_action)
    core_group.addAction(rust_core_action)
    core_menu.addAction(python_core_action)
    core_menu.addAction(rust_core_action)

    def select_cpu_core(core: str) -> None:
        nonlocal selected_cpu_core
        if core == selected_cpu_core:
            return
        if core == "rust":
            try:
                from sega_z80_core import RustZ80Cpu as _RustZ80Cpu  # noqa: F401
            except ImportError:
                QMessageBox.warning(
                    window,
                    "Núcleo Rust indisponível",
                    "O núcleo Rust experimental não está instalado. Execute "
                    "'maturin develop --features python-extension' em native/z80_core.",
                )
                python_core_action.setChecked(selected_cpu_core == "python")
                rust_core_action.setChecked(selected_cpu_core == "rust")
                return
        previous_core = selected_cpu_core
        selected_cpu_core = core
        if loaded is not None:
            try:
                reset_machine()
            except Exception as exc:
                selected_cpu_core = previous_core
                python_core_action.setChecked(selected_cpu_core == "python")
                rust_core_action.setChecked(selected_cpu_core == "rust")
                QMessageBox.warning(
                    window, "Não foi possível trocar o núcleo", str(exc)
                )
                return
        settings.setValue("cpu_core", selected_cpu_core)
        core_label = (
            "Python (referência)" if core == "python" else "Rust (experimental)"
        )
        status.showMessage(f"Núcleo da CPU: {core_label}")
        logger.info(
            "Núcleo da CPU selecionado",
            extra={"event": "cpu_core_selected", "cpu_core": selected_cpu_core},
        )

    python_core_action.triggered.connect(
        lambda checked=False: select_cpu_core("python")
    )
    rust_core_action.triggered.connect(lambda checked=False: select_cpu_core("rust"))

    video_menu = window.menuBar().addMenu("&Vídeo")
    video_menu.addAction("Escala 2×", lambda: window.resize(600, 450))
    video_menu.addAction("Escala 3×", lambda: window.resize(800, 620))
    video_menu.addAction("Escala 4×", lambda: window.resize(1050, 800))

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
    if pygame is not None:
        pygame.mixer.quit()
