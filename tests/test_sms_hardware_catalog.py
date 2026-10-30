from __future__ import annotations

import hashlib

import pytest

from sega_emulator.machine.sms_hardware_catalog import (
    FieldEvidence,
    FmCapability,
    FmGameUsage,
    FmRequirement,
    MetadataConfidence,
    MetadataSource,
    PeripheralState,
    SmsFmMetadata,
    SmsHardwareCatalog,
    SmsHardwareIdentification,
    SmsHardwareMatchStatus,
    SmsHardwareMetadata,
    SmsHardwareRecord,
    SmsMapperMetadata,
    SmsPeripheral,
    SmsPeripheralMetadata,
    SmsSaveKind,
    SmsSaveMetadata,
)


def _evidence(value, *, reference="sms-power:test", confidence=None, source=None):
    if confidence is None:
        confidence = MetadataConfidence.CONFIRMED
    if source is None:
        source = MetadataSource.CURATED_HARDWARE_DB
    return FieldEvidence(
        value=value,
        source=source,
        confidence=confidence,
        source_reference=reference,
        source_revision="fixture-v1",
    )


def _record(payload: bytes, *, record_id="fixture", sha1=True, md5=True, metadata=None):
    return SmsHardwareRecord(
        record_id=record_id,
        rom_size=len(payload),
        sha1=hashlib.sha1(payload).hexdigest() if sha1 else None,
        md5=hashlib.md5(payload).hexdigest() if md5 else None,
        metadata=metadata or SmsHardwareMetadata(),
    )


def test_empty_catalog_returns_explicit_not_catalogued_result() -> None:
    result = SmsHardwareCatalog().identify_bytes(b"synthetic sms image")

    assert result.status is SmsHardwareMatchStatus.NOT_CATALOGUED
    assert result.metadata is None
    assert result.matched_by is None
    assert result.candidates == ()


def test_unknown_metadata_is_not_filled_with_hardware_defaults() -> None:
    metadata = SmsHardwareMetadata()

    assert metadata.mapper.identifier == FieldEvidence.unknown()
    assert metadata.fm.console_capability == FieldEvidence.unknown()
    assert metadata.fm.game_usage == FieldEvidence.unknown()
    assert metadata.fm.game_requirement == FieldEvidence.unknown()
    assert metadata.peripherals == ()
    assert metadata.save.kind == FieldEvidence.unknown()
    assert metadata.save.capacity_bytes == FieldEvidence.unknown()


def test_exact_sha1_and_md5_match_returns_one_immutable_metadata_record() -> None:
    payload = b"synthetic sms cartridge"
    mapper = SmsMapperMetadata(
        identifier=_evidence("sega_standard"),
        bank_count=_evidence(3),
        bank_size_bytes=_evidence(0x4000),
    )
    fm = SmsFmMetadata(
        console_capability=_evidence(FmCapability.AVAILABLE),
        game_usage=_evidence(FmGameUsage.USED),
        game_requirement=_evidence(FmRequirement.OPTIONAL),
    )
    save = SmsSaveMetadata(
        kind=_evidence(SmsSaveKind.BATTERY_BACKED_SRAM),
        capacity_bytes=_evidence(0x2000),
        bank_count=_evidence(1),
    )
    metadata = SmsHardwareMetadata(mapper=mapper, fm=fm, save=save)
    record = _record(payload, metadata=metadata)

    result = SmsHardwareCatalog((record,)).identify_bytes(payload)

    assert result.status is SmsHardwareMatchStatus.MATCHED
    assert result.matched_by == "sha1+md5"
    assert result.metadata == metadata
    assert result.candidates == (record,)
    assert result.metadata.mapper.identifier.value == "sega_standard"
    assert result.metadata.fm.console_capability.value is FmCapability.AVAILABLE
    assert result.metadata.fm.game_usage.value is FmGameUsage.USED
    assert result.metadata.save.capacity_bytes.value == 0x2000


def test_md5_only_record_can_be_identified() -> None:
    payload = b"synthetic md5-only image"
    record = _record(payload, sha1=False, md5=True)

    result = SmsHardwareCatalog((record,)).identify_bytes(payload)

    assert result.status is SmsHardwareMatchStatus.MATCHED
    assert result.matched_by == "md5"


def test_crc32_or_same_size_is_never_used_as_hardware_identity() -> None:
    payload = b"synthetic unknown image"
    record = _record(b"synthetic known image")
    result = SmsHardwareCatalog((record,)).identify_bytes(payload)

    assert result.status is SmsHardwareMatchStatus.NOT_CATALOGUED
    assert result.metadata is None


def test_duplicate_exact_hash_records_are_reported_as_ambiguous() -> None:
    payload = b"synthetic ambiguous image"
    first = _record(payload, record_id="record-a")
    second = _record(payload, record_id="record-b")

    result = SmsHardwareCatalog((second, first)).identify_bytes(payload)

    assert result.status is SmsHardwareMatchStatus.AMBIGUOUS
    assert result.metadata is None
    assert tuple(item.record_id for item in result.candidates) == (
        "record-a",
        "record-b",
    )


def test_sha1_and_md5_pointing_to_different_records_are_a_conflict() -> None:
    payload = b"synthetic conflicting index"
    sha1_record = _record(payload, record_id="sha-record", sha1=True, md5=False)
    md5_record = _record(payload, record_id="md5-record", sha1=False, md5=True)

    result = SmsHardwareCatalog((sha1_record, md5_record)).identify_bytes(payload)

    assert result.status is SmsHardwareMatchStatus.AMBIGUOUS
    assert result.matched_by == "sha1+md5_conflict"
    assert result.metadata is None
    assert {item.record_id for item in result.candidates} == {
        "sha-record",
        "md5-record",
    }


def test_catalog_rejects_duplicate_record_ids() -> None:
    payload = b"duplicate id"
    first = _record(payload, record_id="same")
    second = _record(payload, record_id="same")

    with pytest.raises(ValueError, match="record_id values must be unique"):
        SmsHardwareCatalog((first, second))


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"rom_size": 0}, "rom_size must be positive"),
        ({"sha1": None, "md5": None}, "requires SHA-1 or MD5"),
        ({"sha1": "not-a-hash"}, "lowercase 40-character"),
        ({"md5": "ABCDEF"}, "lowercase 32-character"),
    ],
)
def test_record_rejects_invalid_identity(kwargs, message) -> None:
    payload = b"record validation"
    values = {
        "record_id": "invalid",
        "rom_size": len(payload),
        "sha1": hashlib.sha1(payload).hexdigest(),
        "md5": hashlib.md5(payload).hexdigest(),
        "metadata": SmsHardwareMetadata(),
    }
    values.update(kwargs)

    with pytest.raises(ValueError, match=message):
        SmsHardwareRecord(**values)


def test_known_field_requires_explicit_provenance() -> None:
    with pytest.raises(ValueError, match="non-empty source reference"):
        FieldEvidence(
            value="codemasters",
            source=MetadataSource.CURATED_HARDWARE_DB,
            confidence=MetadataConfidence.CONFIRMED,
        )


def test_unknown_field_cannot_claim_a_known_source() -> None:
    with pytest.raises(ValueError, match="UNKNOWN source/confidence"):
        FieldEvidence(
            value=None,
            source=MetadataSource.NO_INTRO_DAT,
            confidence=MetadataConfidence.UNKNOWN,
        )


def test_header_derived_and_manual_confidence_require_matching_sources() -> None:
    with pytest.raises(ValueError, match="HEADER_DERIVED"):
        _evidence(
            "japan",
            source=MetadataSource.NO_INTRO_DAT,
            confidence=MetadataConfidence.HEADER_DERIVED,
        )

    with pytest.raises(ValueError, match="MANUAL"):
        _evidence(
            "brazil",
            source=MetadataSource.CURATED_HARDWARE_DB,
            confidence=MetadataConfidence.MANUAL,
        )


def test_mapper_and_save_ranges_are_validated() -> None:
    with pytest.raises(ValueError, match="bank_count must be positive"):
        SmsMapperMetadata(bank_count=_evidence(0))

    with pytest.raises(ValueError, match="window_end cannot precede"):
        SmsSaveMetadata(
            window_start=_evidence(0xC000),
            window_end=_evidence(0xBFFF),
        )

    with pytest.raises(ValueError, match="capacity_bytes cannot be negative"):
        SmsSaveMetadata(capacity_bytes=_evidence(-1))


def test_peripheral_families_are_independent_and_cannot_be_duplicated() -> None:
    joypad = SmsPeripheralMetadata(
        peripheral=SmsPeripheral.JOYPAD,
        state=_evidence(PeripheralState.REQUIRED),
    )
    phaser = SmsPeripheralMetadata(
        peripheral=SmsPeripheral.LIGHT_PHASER,
        state=_evidence(PeripheralState.SUPPORTED),
    )
    metadata = SmsHardwareMetadata(peripherals=(joypad, phaser))

    assert metadata.peripherals == (joypad, phaser)

    with pytest.raises(ValueError, match="only once"):
        SmsHardwareMetadata(peripherals=(joypad, joypad))


def test_identify_bytes_rejects_non_bytes_input() -> None:
    with pytest.raises(TypeError, match="data must be bytes"):
        SmsHardwareCatalog().identify_bytes(bytearray(b"not bytes"))


def test_metadata_snapshot_rejects_mutable_peripheral_collections() -> None:
    joypad = SmsPeripheralMetadata(
        peripheral=SmsPeripheral.JOYPAD,
        state=_evidence(PeripheralState.REQUIRED),
    )

    with pytest.raises(TypeError, match="peripherals must be a tuple"):
        SmsHardwareMetadata(peripherals=[joypad])


def test_lookup_result_rejects_inconsistent_match_contracts() -> None:
    payload = b"result invariant"
    record = _record(payload)
    metadata = record.metadata

    with pytest.raises(ValueError, match="one record, metadata and match type"):
        SmsHardwareIdentification(
            status=SmsHardwareMatchStatus.MATCHED,
            metadata=metadata,
            matched_by=None,
            candidates=(record,),
        )

    with pytest.raises(ValueError, match="multiple candidates and no winner"):
        SmsHardwareIdentification(
            status=SmsHardwareMatchStatus.AMBIGUOUS,
            metadata=metadata,
            matched_by="sha1",
            candidates=(record,),
        )

    with pytest.raises(ValueError, match="cannot contain a match"):
        SmsHardwareIdentification(
            status=SmsHardwareMatchStatus.NOT_CATALOGUED,
            metadata=None,
            matched_by="sha1",
            candidates=(),
        )


def test_exact_hash_with_wrong_rom_size_is_not_a_match() -> None:
    payload = b"size is part of identity"
    record = SmsHardwareRecord(
        record_id="wrong-size",
        rom_size=len(payload) + 1,
        sha1=hashlib.sha1(payload).hexdigest(),
        metadata=SmsHardwareMetadata(),
    )

    result = SmsHardwareCatalog((record,)).identify_bytes(payload)

    assert result.status is SmsHardwareMatchStatus.NOT_CATALOGUED


def test_known_field_rejects_blank_source_revision() -> None:
    with pytest.raises(ValueError, match="source_revision cannot be empty"):
        FieldEvidence(
            value="codemasters",
            source=MetadataSource.CURATED_HARDWARE_DB,
            confidence=MetadataConfidence.CONFIRMED,
            source_reference="sms-power:test",
            source_revision=" ",
        )
