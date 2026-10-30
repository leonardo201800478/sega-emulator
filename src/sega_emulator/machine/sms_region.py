"""SMS region preference, ROM resolution and deterministic frame pacing."""

from __future__ import annotations

from dataclasses import dataclass

from sega_emulator.machine.sms_firmware import sms_bios_region_from_cartridge
from sega_emulator.rom_catalog import RomCatalog

SMS_REGION_ORDER = ("usa", "europe", "japan", "brazil")
SMS_REGION_LABELS = {
    "auto": "Automático (header/DAT; padrão EUA)",
    "usa": "EUA (NTSC)",
    "europe": "Europa (PAL)",
    "japan": "Japão (NTSC)",
    "brazil": "Brasil (PAL-M)",
}

# Nominal host pacing. Brazil PAL-M has the same 262-line/NTSC-like timing;
# this does not model its analog composite colour subcarrier.
_SMS_FRAME_PERIODS = {
    "usa": (228 * 262) / 3_579_545,
    "brazil": (228 * 262) / 3_579_545,
    "europe": (228 * 313) / 3_546_895,
    "japan": (228 * 262) / 3_579_545,
}

_VIDEO_PROFILE_BY_RELEASE_REGION = {
    "JPN": "japan",
    "USA": "usa",
    "EUR": "europe",
    "AUS": "europe",
    "BRA": "brazil",
}
_EXPORT_RELEASE_REGIONS = frozenset({"USA", "EUR", "AUS", "BRA"})
_KOREAN_OR_TAIWAN_REGIONS = frozenset({"KOR", "TAI"})


@dataclass(frozen=True, slots=True)
class SmsRegionResolution:
    """Selected video/territory profile and the evidence used to choose it."""

    region: str
    source: str
    header_family: str | None
    release_regions: tuple[str, ...]
    ambiguous: bool = False
    conflict: bool = False
    catalog_error: bool = False


def normalize_sms_region_preference(value: object) -> str:
    """Return a supported preference, defaulting invalid persisted values to auto."""
    return value if isinstance(value, str) and value in SMS_REGION_LABELS else "auto"


def sms_region_candidates(preference: str) -> tuple[str, ...]:
    """Return one deterministic profile: the manual choice or the USA fallback."""
    normalized = normalize_sms_region_preference(preference)
    return ("usa",) if normalized == "auto" else (normalized,)


def _catalog_video_profile(
    rom: bytes,
    catalog: RomCatalog | None,
) -> tuple[str | None, tuple[str, ...], bool, bool]:
    """Return a uniquely supported profile from exact SMS SHA-1/MD5 DAT matches."""
    if catalog is None:
        return None, (), False, False

    try:
        identification = catalog.identify_bytes(rom, suffix=".sms")
    except ValueError:
        return None, (), False, True

    regions = tuple(sorted(set(identification.regions)))
    if (
        identification.status != "matched"
        or identification.system != "master_system"
        or identification.matched_by not in {"sha1", "md5"}
        or not identification.candidates
    ):
        return None, regions, bool(regions), False

    candidate_profiles: set[str] = set()
    for candidate in identification.candidates:
        if candidate.system != "master_system" or not candidate.regions:
            return None, regions, True, False
        profiles: set[str] = set()
        for release_region in candidate.regions:
            profile = _VIDEO_PROFILE_BY_RELEASE_REGION.get(release_region)
            if profile is None:
                return None, regions, True, False
            profiles.add(profile)
        if len(profiles) != 1:
            return None, regions, True, False
        candidate_profiles.update(profiles)

    if len(candidate_profiles) != 1:
        return None, regions, True, False
    return next(iter(candidate_profiles)), regions, False, False


def resolve_sms_region_profile(
    preference: str,
    rom: bytes,
    catalog: RomCatalog | None = None,
) -> SmsRegionResolution:
    """Resolve the SMS profile without treating an Export header as a territory.

    Manual selection has highest priority. The TMR SEGA header identifies only
    Japan versus Export; an exact DAT match may refine Export to USA, Europe or
    Brazil. Ambiguous multi-territory releases use the documented USA fallback
    and remain marked ambiguous for diagnostics/OSD. Korean and Taiwan releases
    are not collapsed into the Japan profile.
    """
    if not isinstance(rom, bytes):
        raise TypeError("rom deve ser bytes.")

    normalized = normalize_sms_region_preference(preference)
    header_bios_family = sms_bios_region_from_cartridge(rom)
    if header_bios_family == "japan":
        header_family = "japan"
    elif header_bios_family == "usa":
        header_family = "export"
    else:
        header_family = None
    catalog_profile, release_regions, ambiguous, catalog_error = (
        _catalog_video_profile(rom, catalog)
    )

    if normalized != "auto":
        return SmsRegionResolution(
            region=normalized,
            source="manual",
            header_family=header_family,
            release_regions=release_regions,
            ambiguous=ambiguous,
            catalog_error=catalog_error,
        )

    region_set = set(release_regions)
    if header_family == "japan":
        if region_set & _KOREAN_OR_TAIWAN_REGIONS:
            return SmsRegionResolution(
                region="usa",
                source="conflict_fallback",
                header_family=header_family,
                release_regions=release_regions,
                ambiguous=True,
                conflict=True,
                catalog_error=catalog_error,
            )
        conflict = bool(region_set & _EXPORT_RELEASE_REGIONS)
        return SmsRegionResolution(
            region="japan",
            source="header",
            header_family=header_family,
            release_regions=release_regions,
            ambiguous=ambiguous,
            conflict=conflict,
            catalog_error=catalog_error,
        )

    if header_family == "export":
        if catalog_profile in {"usa", "europe", "brazil"}:
            return SmsRegionResolution(
                region=catalog_profile,
                source="dat_refinement",
                header_family=header_family,
                release_regions=release_regions,
                catalog_error=catalog_error,
            )
        if region_set & ({"JPN"} | _KOREAN_OR_TAIWAN_REGIONS):
            return SmsRegionResolution(
                region="usa",
                source="conflict_fallback",
                header_family=header_family,
                release_regions=release_regions,
                ambiguous=True,
                conflict=True,
                catalog_error=catalog_error,
            )
        return SmsRegionResolution(
            region="usa",
            source="ambiguous_fallback" if ambiguous else "header_export_default",
            header_family=header_family,
            release_regions=release_regions,
            ambiguous=ambiguous,
            catalog_error=catalog_error,
        )

    if catalog_profile is not None:
        return SmsRegionResolution(
            region=catalog_profile,
            source="dat",
            header_family=None,
            release_regions=release_regions,
            catalog_error=catalog_error,
        )

    mixed_japan_export = "JPN" in region_set and bool(
        region_set & _EXPORT_RELEASE_REGIONS
    )
    return SmsRegionResolution(
        region="usa",
        source="ambiguous_fallback" if ambiguous else "fallback",
        header_family=None,
        release_regions=release_regions,
        ambiguous=ambiguous,
        conflict=mixed_japan_export,
        catalog_error=catalog_error,
    )


def sms_region_candidates_for_rom(
    preference: str,
    rom: bytes,
    catalog: RomCatalog | None = None,
) -> tuple[str, ...]:
    """Return the single profile selected by manual preference, header and DAT."""
    resolution = resolve_sms_region_profile(preference, rom, catalog)
    return (resolution.region,)


def sms_frame_period_seconds(region: str) -> float:
    """Return nominal host pacing for a concrete SMS profile."""
    try:
        return _SMS_FRAME_PERIODS[region]
    except KeyError as exc:
        raise ValueError("region deve ser usa, brazil, europe ou japan.") from exc
