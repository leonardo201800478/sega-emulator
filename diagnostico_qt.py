import importlib
import traceback

modulos = [
    "PySide6.QtWidgets",
    "sega_emulator.frontend.qt.debug_logging",
    "sega_emulator.frontend.qt.main_window",
    "sega_emulator.frontend.qt.rom_library_dialog",
    "sega_emulator.frontend.qt.settings_dialog",
]

for nome in modulos:
    print(f"\n--- Testando {nome} ---")
    try:
        importlib.import_module(nome)
        print("PASS")
    except Exception:
        traceback.print_exc()
