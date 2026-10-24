"""Tests for opt-in execution profiles and capability-gated fallbacks."""

from sega_emulator.profiles import (
    ACCURATE_PROFILE,
    PERFORMANCE_PROFILE,
    AudioMode,
    CpuBackend,
    RuntimeCapabilities,
    TimingMode,
    VdpMode,
    resolve_profile,
)


def test_accurate_profile_is_reference_only() -> None:
    resolved = resolve_profile(ACCURATE_PROFILE)

    assert resolved.effective == ACCURATE_PROFILE
    assert resolved.fallbacks == ()
    assert resolved.effective.cpu is CpuBackend.INTERPRETER
    assert resolved.effective.vdp is VdpMode.REFERENCE
    assert resolved.effective.audio is AudioMode.PSG_CHIP
    assert resolved.effective.timing is TimingMode.STRICT


def test_performance_preset_does_not_enable_unavailable_features() -> None:
    resolved = resolve_profile(PERFORMANCE_PROFILE)

    assert resolved.requested == PERFORMANCE_PROFILE
    assert resolved.effective.cpu is CpuBackend.INTERPRETER
    assert resolved.effective.vdp is VdpMode.REFERENCE
    assert resolved.effective.audio is AudioMode.PSG_CHIP
    assert resolved.effective.timing is TimingMode.STRICT
    assert len(resolved.fallbacks) == 4


def test_performance_profile_uses_only_advertised_capabilities() -> None:
    capabilities = RuntimeCapabilities(
        llvm_z80_jit=True,
        vdp_fast_path=False,
        psg_hle=True,
        balanced_timing=True,
    )

    resolved = resolve_profile(PERFORMANCE_PROFILE, capabilities)

    assert resolved.effective.cpu is CpuBackend.LLVM_JIT
    assert resolved.effective.vdp is VdpMode.REFERENCE
    assert resolved.effective.audio is AudioMode.HLE
    assert resolved.effective.timing is TimingMode.BALANCED
    assert resolved.fallbacks == (
        "fast VDP path unavailable; using reference VDP",
    )


def test_relaxed_timing_falls_back_to_strict_without_capability() -> None:
    profile = PERFORMANCE_PROFILE.__class__(
        name="relaxed-test",
        cpu=CpuBackend.INTERPRETER,
        vdp=VdpMode.REFERENCE,
        audio=AudioMode.PSG_CHIP,
        timing=TimingMode.RELAXED,
    )

    resolved = resolve_profile(profile)

    assert resolved.effective.timing is TimingMode.STRICT
    assert resolved.fallbacks == (
        "relaxed timing unavailable; using strict timing",
    )
