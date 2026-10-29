"""SMS region preference and deterministic frame-pacing policy."""

from __future__ import annotations

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


def sms_frame_period_seconds(region: str) -> float:
    """Return the nominal host pacing period for a concrete SMS region."""
    try:
        return _SMS_FRAME_PERIODS[region]
    except KeyError as exc:
        raise ValueError("region deve ser usa, europe ou japan.") from exc
