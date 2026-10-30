"""SMS region preference and deterministic frame-pacing policy."""

from __future__ import annotations

from sega_emulator.machine.sms_firmware import sms_bios_region_from_cartridge

SMS_REGION_ORDER = ("usa", "europe", "japan")
SMS_REGION_LABELS = {
    "auto": "Automático (EUA → Europa → Japão)",
    "usa": "EUA (NTSC)",
    "europe": "Europa (PAL)",
    "japan": "Japão (NTSC)",
}

# Nominal VDP frame periods from 228 T-states/line and each standard's
# documented line count / system clock. These pace the host; they do not
# replace the emulated VDP's own timing.
_SMS_FRAME_PERIODS = {
    "usa": (228 * 262) / 3_579_545,
    "europe": (228 * 313) / 3_546_895,
    "japan": (228 * 262) / 3_579_545,
}


def normalize_sms_region_preference(value: object) -> str:
    """Return a supported preference, defaulting invalid persisted values to auto."""
    return value if isinstance(value, str) and value in SMS_REGION_LABELS else "auto"


def sms_region_candidates(preference: str) -> tuple[str, ...]:
    """Return region attempts in priority order for automatic or forced selection."""
    normalized = normalize_sms_region_preference(preference)
    return SMS_REGION_ORDER if normalized == "auto" else (normalized,)


def sms_region_candidates_for_rom(
    preference: str,
    rom: bytes,
) -> tuple[str, ...]:
    """Resolve territory attempts using an explicit preference or trusted ROM header.

    Automatic mode uses the SMS region nibble when a valid Sega header exists:
    Japan stays Japan; export tries NTSC then PAL. Unknown headers retain the
    existing deterministic fallback order without selecting a BIOS by guesswork.
    """
    normalized = normalize_sms_region_preference(preference)
    if normalized != "auto":
        return (normalized,)
    cartridge_region = sms_bios_region_from_cartridge(rom)
    if cartridge_region == "japan":
        return ("japan",)
    if cartridge_region == "usa":
        return ("usa", "europe")
    return SMS_REGION_ORDER


def sms_frame_period_seconds(region: str) -> float:
    """Return the nominal host pacing period for a concrete SMS region."""
    try:
        return _SMS_FRAME_PERIODS[region]
    except KeyError as exc:
        raise ValueError("region deve ser usa, europe ou japan.") from exc
