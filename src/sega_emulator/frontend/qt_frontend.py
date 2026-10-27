"""Interface experimental Qt 6/PySide6 para menus e diagnóstico do SG-1000."""

from __future__ import annotations

import hashlib
import json
import logging
import time
from pathlib import Path

from sega_emulator.audio.vgm_recorder import VgmRecorder
from sega_emulator.frontend.controller_input import (
    ACTIONS,
    DEFAULT_GAMEPAD_BINDINGS,
    JOYSTICK_ACTIONS,
    SUPPORTED_BUTTONS,
    ControllerInputConfig,
    PlayerInputProfile,
    apply_turbo,
    joystick_value,
)
from sega_emulator.frontend.native_first_controller_input import (
    NativeFirstControllerInput,
)
from sega_emulator.frontend.qt.audio_output import PcmResampler
from sega_emulator.frontend.qt.frame_pacer import FramePacer
from sega_emulator.frontend.qt.preferences import FrontendPreferences
from sega_emulator.frontend.qt.wasapi_audio import WasapiAudioOutput
from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import LoadedRom, list_rom_members, load_rom

logger = logging.getLogger(__name__)

# Clock nominal do Z80 no SG-1000 NTSC: 10,738635 MHz / 3.
Z80_CLOCK_HZ = 3_579_545

# NTSC frame pacing estimate: 228 Z80 T-states per line, 262 lines.
FRAME_PERIOD_SECONDS = (228 * 262) / Z80_CLOCK_HZ


def _runtime_diagnostic(machine: SG1000Machine, rom_sha256: str) -> dict[str, object]:
    """Return verifiable identity of the loaded emulator runtime and native binary."""
    import os
    import sys

    diagnostic: dict[str, object] = {
        "backend_requested": machine.cpu_core,
        "machine_type": type(machine).__module__ + "." + type(machine).__qualname__,
        "cpu_type": type(machine.cpu).__module__ + "." + type(machine.cpu).__qualname__,
        "memory_bus_type": (
            type(machine.memory_bus).__module__
            + "."
            + type(machine.memory_bus).__qualname__
        ),
        "vdp_type": type(machine.vdp).__module__ + "." + type(machine.vdp).__qualname__,
        "psg_type": type(machine.psg).__module__ + "." + type(machine.psg).__qualname__,
        "audio_sync_type": (
            type(machine.audio_sync).__module__
            + "."
            + type(machine.audio_sync).__qualname__
        ),
        "rom_sha256": rom_sha256,
        "python_executable": sys.executable,
        "python_version": sys.version,
        "process_id": os.getpid(),
    }
    if machine.cpu_core != "rust":
        diagnostic["backend_effective"] = "python_reference"
        return diagnostic

    native = getattr(machine, "native_machine", None)
    diagnostic["backend_effective"] = (
        "rust_native_machine"
        if native is not None and type(native).__name__ == "NativeSG1000Machine"
        else "unexpected_backend_state"
    )
    diagnostic["native_machine_type"] = (
        type(native).__module__ + "." + type(native).__qualname__
        if native is not None
        else None
    )
    try:
        import sega_z80_core

        binary_path = Path(sega_z80_core.__file__).resolve()
        diagnostic["native_module"] = sega_z80_core.__name__
        diagnostic["native_binary_path"] = str(binary_path)
        diagnostic["native_binary_size"] = binary_path.stat().st_size
        diagnostic["native_binary_sha256"] = hashlib.sha256(
            binary_path.read_bytes()
        ).hexdigest()
        diagnostic["native_binary_mtime_ns"] = binary_path.stat().st_mtime_ns
    except Exception as exc:
        diagnostic["native_binary_inspection_error"] = (
            type(exc).__name__ + ": " + str(exc)
        )
    if native is not None:
        diagnostic["native_instruction_count"] = native.total_instructions
        diagnostic["native_tstates"] = native.total_tstates
        diagnostic["native_vdp_frame"] = native.vdp.frame_number
    return diagnostic


def _initialize_wasapi_backend(
    preferences: FrontendPreferences,
) -> WasapiAudioOutput | None:
    """Open the Windows WASAPI output without tying audio to the frame loop."""
    if not preferences.audio_enabled:
        return None
    try:
        return WasapiAudioOutput(
            sample_rate=preferences.audio_sample_rate,
            buffer_size=preferences.audio_buffer_size,
            volume=preferences.audio_volume,
        )
    except Exception as exc:
        # Audio initialization failure must not disable video or input.
        logger.warning("Saída WASAPI indisponível: %s", exc)
        return None


def _audio_output_warning(
    preferences: FrontendPreferences, audio_output: WasapiAudioOutput | None
) -> str | None:
    """Describe an enabled audio backend that failed to initialize."""
    if preferences.audio_enabled and audio_output is None:
        return "Áudio WASAPI indisponível; consulte o log para detalhes."
    return None


def run(initial_path: Path | None = None) -> None:
    """Abre a interface de teste com teclado, telemetria e logs de sessão."""
    try:
        from PySide6.QtCore import QEvent, QObject, QSettings, Qt, QTimer
        from PySide6.QtGui import QImage, QKeySequence
        from PySide6.QtWidgets import (
            QApplication,
            QCheckBox,
            QComboBox,
            QDialog,
            QDialogButtonBox,
            QDoubleSpinBox,
            QFileDialog,
            QFormLayout,
            QHBoxLayout,
            QInputDialog,
            QLabel,
            QMessageBox,
            QPushButton,
            QScrollArea,
            QTabWidget,
            QVBoxLayout,
            QWidget,
        )

        from sega_emulator.frontend.qt.main_window import (
            EmulatorMainWindow,
            persisted_window_dimensions,
        )
        from sega_emulator.frontend.qt.settings_dialog import SettingsDialog
    except ImportError as exc:
        raise RuntimeError(
            'Interface Qt não instalada. Execute: python -m pip install -e ".[qt]"'
        ) from exc

    app = QApplication.instance() or QApplication([])
    app.setOrganizationName("SegaEmulator")
    app.setApplicationName("SegaEmulator")
    settings = QSettings()
    preferences = FrontendPreferences.from_store(settings)
    video_driver = str(settings.value("video_driver", "auto"))
    if video_driver not in {"auto", "vulkan", "d3d11", "d3d12", "raster"}:
        video_driver = "auto"
        settings.setValue("video_driver", video_driver)
    # The production Qt frontend always runs the native Rust machine. Legacy
    # persisted "python" values are migrated unconditionally and are no longer
    # exposed as a selectable runtime.
    selected_backend = "rust"
    settings.setValue("cpu_core", selected_backend)
    settings.setValue("rust_backend_default_migrated", True)
    audio_output = _initialize_wasapi_backend(preferences)
    audio_resampler = PcmResampler(preferences.audio_sample_rate)
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
    input_driver = str(settings.value("input_driver", "auto"))
    if input_driver not in {"auto", "xinput", "sdl3"}:
        input_driver = "auto"
    controller_input = NativeFirstControllerInput(
        controller_config, backend_preference=input_driver
    )
    logger.info(
        "Backend nativo-primeiro de controles inicializado",
        extra={
            "event": "native_first_input_initialized",
            "available": controller_input.available,
            "devices": controller_input.connected_names(),
        },
    )

    window = EmulatorMainWindow(
        on_open_rom=lambda: open_rom(),
        on_pause_resume=lambda: pause_resume(),
        on_reset=lambda: reset_machine(),
        on_configure_controls=lambda: configure_controllers(),
        on_show_controllers=lambda: show_controller_devices(),
        on_vdp_snapshot=lambda: dump_vdp_snapshot(),
        on_open_logs=lambda: show_logs(),
        on_show_machine_state=lambda: show_machine_state(),
        on_video_scale=lambda scale: window.set_video_scale(scale),
        on_configure_drivers=lambda: configure_drivers(),
        on_configure_audio_video=lambda: configure_audio_video(),
        on_start_vgm_recording=lambda: start_vgm_recording(),
        on_stop_vgm_recording=lambda: stop_vgm_recording(),
        video_driver=video_driver,
    )
    window.resize(preferences.window_width, preferences.window_height)
    window._video_scale = preferences.video_scale
    view = window.video_widget
    view.set_filter_mode(preferences.video_filter)
    view.set_integer_scale(preferences.integer_scale)
    status = window.status
    audio_warning = _audio_output_warning(preferences, audio_output)
    if audio_warning is not None:
        status.showMessage(audio_warning)

    machine: SG1000Machine | None = None
    loaded: LoadedRom | None = None
    vgm_recorder = VgmRecorder()
    vgm_output_path: Path | None = None
    paused = False
    frame_count = 0
    pressed_keys: set[int] = set()
    hold_was_pressed = False
    timer = QTimer(window)
    # O loop do emulador é limitado ao período nominal NTSC; o relógio lógico
    # da máquina continua avançando exclusivamente pelos T-states emulados.
    frame_pacer = FramePacer(FRAME_PERIOD_SECONDS)
    timer.setTimerType(Qt.TimerType.PreciseTimer)
    timer.setInterval(1)

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
                    if window.isFullScreen():
                        window.fullscreen_action.setChecked(False)
                    else:
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
        nonlocal paused, frame_count, last_checkpoint_time, last_checkpoint_tstates
        nonlocal last_checkpoint_instructions, last_checkpoint_frame
        if machine is None or paused:
            return
        if not frame_pacer.should_run(time.perf_counter()):
            return
        try:
            frame_started = time.perf_counter()
            phase_started = frame_started
            update_joystick()
            if controller_input.consume_disconnect_event():
                paused = True
                status.showMessage("Emulação pausada: controle desconectado", 5000)
                logger.warning(
                    "Emulação pausada após desconexão de controle",
                    extra={
                        "event": "controller_disconnected_pause",
                        "frame": frame_count,
                    },
                )
                return
            machine.run_frame()
            psg_events = machine.drain_psg_write_events()
            if vgm_recorder.active:
                for event_tstate, event_value in psg_events:
                    vgm_recorder.record_write(event_tstate, event_value)
            samples = machine.drain_audio_samples()
            if samples and audio_output is not None:
                pcm = audio_resampler.process(samples)
                audio_output.write_pcm(pcm)
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
            view.set_framebuffer(image)
            scale_ms = (time.perf_counter() - phase_started) * 1000
            set_pixmap_ms = scale_ms
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

    def refresh_recent_rom_menu() -> None:
        recent_paths = settings.value("recent_roms", [], type=list)
        valid_paths = [str(path) for path in recent_paths if Path(str(path)).is_file()]
        window.set_recent_roms(
            valid_paths,
            on_select=lambda selected_path: open_rom(selected_path),
            on_clear=clear_recent_roms,
        )

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
            loaded_machine = SG1000Machine(candidate.data, cpu_core=selected_backend)
            if not preferences.sprite_limit_enabled:
                loaded_machine.set_sprite_limit_enabled(False)
        except Exception as exc:
            logger.exception(
                "Falha ao carregar ROM",
                extra={"event": "rom_load_failed", "rom_path": path},
            )
            QMessageBox.warning(window, "Não foi possível abrir a ROM", str(exc))
            return

        if vgm_recorder.active and not stop_vgm_recording():
            return
        machine, loaded, frame_count, paused = loaded_machine, candidate, 0, False
        audio_resampler.reset()
        window.set_rom_label(candidate.display_name)
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
        logger.info(
            "Identidade do runtime que executará a ROM",
            extra={
                "event": "emulation_runtime_identity",
                **_runtime_diagnostic(machine, candidate.sha256),
            },
        )
        status.showMessage(
            f"ROM: {candidate.display_name} | SHA-256: {candidate.sha256[:16]}…"
        )
        view.setFocus()
        frame_pacer.reset()
        timer.start()

    def configure_drivers() -> None:
        nonlocal controller_input, audio_output, audio_resampler, preferences

        from sega_emulator.frontend.qt.drivers_dialog import DriversDialog

        dialog = DriversDialog(
            input_driver=str(settings.value("input_driver", "auto")),
            audio_driver=str(settings.value("audio_driver", "wasapi")),
            video_driver=str(settings.value("video_driver", "auto")),
            parent=window,
        )
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return
        input_driver, audio_driver, video_driver = dialog.drivers()

        # Validate forced drivers before applying any of the new preferences.
        if input_driver != "auto" and not (
            controller_input.native.available
            if input_driver == "xinput"
            else bool(controller_input.fallback.available)
        ):
            QMessageBox.warning(
                window,
                "Driver de input indisponível",
                f"A API {input_driver} não está disponível neste sistema. "
                "A configuração anterior foi mantida.",
            )
            return
        current_video_driver = str(settings.value("video_driver", "auto"))
        video_driver_changed = video_driver != current_video_driver
        if (
            not video_driver_changed
            and video_driver in {"auto", "vulkan", "raster"}
            and not view.set_render_backend(video_driver)
        ):
            QMessageBox.warning(
                window,
                "Driver de vídeo indisponível",
                "O backend gráfico selecionado não está disponível nesta sessão. "
                "Escolha Automático ou Qt Raster; as demais configurações "
                "não foram aplicadas.",
            )
            return

        controller_input.set_backend_preference(input_driver)
        settings.setValue("input_driver", input_driver)
        settings.setValue("video_driver", video_driver)

        audio_enabled = audio_driver != "disabled"
        audio_changed = audio_enabled != preferences.audio_enabled
        if audio_changed:
            if audio_output is not None:
                try:
                    audio_output.close()
                except Exception:
                    logger.exception("Falha ao encerrar áudio ao trocar o driver")
            updated_audio = FrontendPreferences(
                window_width=preferences.window_width,
                window_height=preferences.window_height,
                video_scale=preferences.video_scale,
                recent_roms=preferences.recent_roms,
                controller_config=preferences.controller_config,
                audio_enabled=audio_enabled,
                diagnostics_enabled=preferences.diagnostics_enabled,
                cpu_core="rust",
                audio_sample_rate=preferences.audio_sample_rate,
                audio_buffer_size=preferences.audio_buffer_size,
                audio_volume=preferences.audio_volume,
                video_filter=preferences.video_filter,
                integer_scale=preferences.integer_scale,
            )
            audio_output = _initialize_wasapi_backend(updated_audio)
            audio_resampler = PcmResampler(updated_audio.audio_sample_rate)
            preferences = updated_audio
        settings.setValue("audio_driver", audio_driver)
        preferences.save_to_store(settings)

        audio_warning = _audio_output_warning(preferences, audio_output)
        restart_note = (
            " Reinicie o emulador para aplicar a nova API gráfica."
            if video_driver_changed
            else ""
        )
        message = (
            audio_warning
            or f"Drivers aplicados — input: {input_driver}, áudio: {audio_driver}, "
            f"vídeo: {video_driver}."
        ) + restart_note
        status.showMessage(message, 8000)
        logger.info(
            "Drivers de entrada, áudio e vídeo atualizados",
            extra={
                "event": "host_drivers_updated",
                "input_driver": input_driver,
                "audio_driver": audio_driver,
                "video_driver": video_driver,
                "video_driver_restart_required": video_driver_changed,
                "input_available": controller_input.available,
                "audio_enabled": preferences.audio_enabled,
            },
        )

    def configure_audio_video() -> None:
        nonlocal preferences, audio_output, audio_resampler
        dialog = SettingsDialog(preferences, window)
        if dialog.exec() != QDialog.DialogCode.Accepted:
            return
        updated = dialog.preferences()
        if (
            updated.sprite_limit_enabled != preferences.sprite_limit_enabled
            and machine is not None
        ):
            try:
                machine.set_sprite_limit_enabled(updated.sprite_limit_enabled)
            except (AttributeError, RuntimeError, TypeError) as exc:
                QMessageBox.warning(
                    window,
                    "Limite de sprites não aplicado",
                    str(exc),
                )
                return
        audio_device_changed = (
            updated.audio_enabled != preferences.audio_enabled
            or updated.audio_sample_rate != preferences.audio_sample_rate
            or updated.audio_buffer_size != preferences.audio_buffer_size
        )
        if audio_device_changed:
            if audio_output is not None:
                try:
                    audio_output.close()
                except Exception:
                    logger.exception(
                        "Falha ao encerrar WASAPI antes de reconfigurar áudio"
                    )
            audio_output = _initialize_wasapi_backend(updated)
            audio_resampler = PcmResampler(updated.audio_sample_rate)
        elif audio_output is not None:
            audio_output.set_volume(updated.audio_volume)

        preferences = updated
        settings.setValue(
            "audio_driver", "wasapi" if preferences.audio_enabled else "disabled"
        )
        view.set_filter_mode(preferences.video_filter)
        view.set_integer_scale(preferences.integer_scale)
        preferences.save_to_store(settings)
        audio_warning = _audio_output_warning(preferences, audio_output)
        status.showMessage(
            audio_warning or "Configurações de áudio e vídeo aplicadas.",
            8000 if audio_warning is not None else 5000,
        )
        logger.info(
            "Configurações de áudio e vídeo atualizadas",
            extra={
                "event": "audio_video_settings_updated",
                "audio_enabled": preferences.audio_enabled,
                "audio_sample_rate": preferences.audio_sample_rate,
                "audio_buffer_size": preferences.audio_buffer_size,
                "audio_volume": preferences.audio_volume,
                "video_filter": preferences.video_filter,
                "integer_scale": preferences.integer_scale,
                "sprite_limit_enabled": preferences.sprite_limit_enabled,
            },
        )

    def start_vgm_recording() -> None:
        nonlocal vgm_output_path
        if machine is None or loaded is None:
            QMessageBox.information(
                window, "Gravação VGM", "Carregue uma ROM antes de gravar."
            )
            return
        if vgm_recorder.active:
            return
        path, _ = QFileDialog.getSaveFileName(
            window,
            "Salvar gravação PSG VGM",
            (
                str(loaded.source_path.with_suffix(".vgm"))
                if loaded.source_path
                else str(Path.cwd() / "sg1000_capture.vgm")
            ),
            "VGM (*.vgm)",
        )
        if not path:
            return
        if not path.lower().endswith(".vgm"):
            path += ".vgm"
        # Discard queued events from before the recording start.
        machine.drain_psg_write_events()
        vgm_output_path = Path(path)
        vgm_recorder.start(machine.total_tstates)
        window.set_vgm_recording(True)
        status.showMessage("Gravando escritas do PSG em VGM…", 5000)
        logger.info(
            "Gravação VGM iniciada",
            extra={
                "event": "vgm_recording_started",
                "path": str(vgm_output_path),
                "rom_sha256": loaded.sha256,
            },
        )

    def stop_vgm_recording() -> bool:
        nonlocal vgm_output_path
        if not vgm_recorder.active:
            return True
        if machine is None or vgm_output_path is None:
            return False
        for event_tstate, event_value in machine.drain_psg_write_events():
            vgm_recorder.record_write(event_tstate, event_value)
        event_count = vgm_recorder.event_count
        try:
            saved_path = vgm_recorder.save(vgm_output_path, machine.total_tstates)
        except (OSError, ValueError, RuntimeError) as exc:
            QMessageBox.warning(window, "Falha ao salvar VGM", str(exc))
            return False
        window.set_vgm_recording(False)
        status.showMessage(
            f"VGM salvo: {saved_path.name} ({event_count} escritas PSG)", 8000
        )
        logger.info(
            "Gravação VGM salva",
            extra={
                "event": "vgm_recording_saved",
                "path": str(saved_path),
                "psg_writes": event_count,
            },
        )
        vgm_output_path = None
        return True

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
        if vgm_recorder.active and not stop_vgm_recording():
            return
        if loaded is None:
            return
        machine = SG1000Machine(loaded.data, cpu_core=selected_backend)
        audio_resampler.reset()
        frame_count, paused = 0, False
        frame_pacer.reset()
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

    def show_controller_devices() -> None:
        names = controller_input.connected_names()
        devices_text = "\n".join(f"• {name}" for name in names)
        if not devices_text:
            devices_text = "Nenhum controle detectado."
        backend_state = "disponível" if controller_input.available else "indisponível"
        QMessageBox.information(
            window,
            "Dispositivos de controle",
            f"Backend de input: {settings.value('input_driver', 'auto')} "
            f"({backend_state})\n\n{devices_text}",
        )

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

    def show_logs() -> None:
        QMessageBox.information(
            window, "Logs", f"Os logs estão na pasta: {Path('logs').resolve()}"
        )

    def show_machine_state() -> None:
        QMessageBox.information(
            window,
            "Estado",
            (
                "Nenhuma ROM carregada."
                if machine is None
                else f"ROM: {loaded.display_name if loaded else 'desconhecida'}\n"
                f"PC: {machine.cpu.pc:04X}\n"
                f"Instruções: {machine.total_instructions}\n"
                f"T-states: {machine.total_tstates}\n"
                f"Quadro: {machine.vdp.frame_number}\n"
                f"Linha: {machine.vdp.scanline}"
            ),
        )

    timer.timeout.connect(show_frame)
    refresh_recent_rom_menu()
    if initial_path is not None:
        open_rom(str(initial_path))
    window.show()
    app.exec()
    if vgm_recorder.active:
        stop_vgm_recording()
    saved_width, saved_height = persisted_window_dimensions(
        window, preferences.window_width, preferences.window_height
    )
    preferences = FrontendPreferences(
        window_width=max(640, min(7680, saved_width)),
        window_height=max(480, min(4320, saved_height)),
        video_scale=getattr(window, "_video_scale", preferences.video_scale),
        recent_roms=tuple(
            str(path)
            for path in settings.value("recent_roms", [], type=list)
            if isinstance(path, str) and path.strip()
        )[:12],
        controller_config=controller_input.config.to_json(),
        audio_enabled=preferences.audio_enabled,
        diagnostics_enabled=preferences.diagnostics_enabled,
        cpu_core=selected_backend,
        audio_sample_rate=preferences.audio_sample_rate,
        audio_buffer_size=preferences.audio_buffer_size,
        audio_volume=preferences.audio_volume,
        video_filter=preferences.video_filter,
        integer_scale=preferences.integer_scale,
    )
    preferences.save_to_store(settings)
    controller_input.close()
    if audio_output is not None:
        audio_output.close()
