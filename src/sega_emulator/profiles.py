"""Declarative emulation profiles with capability-gated experimental backends.

The profile presets describe user intent. They do not claim that a native
backend is implemented: resolve_profile() falls back to the reference path
until the corresponding capability is explicitly advertised by the runtime.
"""
from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class CpuBackend(str, Enum):
    INTERPRETER = "interpreter"
    LLVM_JIT = "llvm_jit"


class VdpMode(str, Enum):
    REFERENCE = "reference"
    OPTIMIZED = "optimized"
    FAST = "fast"


class AudioMode(str, Enum):
    PSG_CHIP = "psg_chip"
    BATCHED = "batched"
    HLE = "hle"


class TimingMode(str, Enum):
    STRICT = "strict"
    BALANCED = "balanced"
    RELAXED = "relaxed"


@dataclass(frozen=True, slots=True)
class EmulationProfile:
    """User-selected policy; unsupported features are resolved safely."""

    name: str
    cpu: CpuBackend
    vdp: VdpMode
    audio: AudioMode
    timing: TimingMode


@dataclass(frozen=True, slots=True)
class RuntimeCapabilities:
    """Features actually implemented and validated by the running build."""

    llvm_z80_jit: bool = False
    vdp_fast_path: bool = False
    psg_hle: bool = False
    balanced_timing: bool = False
    relaxed_timing: bool = False


@dataclass(frozen=True, slots=True)
class ResolvedProfile:
    """Effective configuration after gating by runtime capabilities."""

    requested: EmulationProfile
    effective: EmulationProfile
    fallbacks: tuple[str, ...]


ACCURATE_PROFILE = EmulationProfile(
    name="accurate",
    cpu=CpuBackend.INTERPRETER,
    vdp=VdpMode.REFERENCE,
    audio=AudioMode.PSG_CHIP,
    timing=TimingMode.STRICT,
)

PERFORMANCE_PROFILE = EmulationProfile(
    name="performance",
    cpu=CpuBackend.LLVM_JIT,
    vdp=VdpMode.FAST,
    audio=AudioMode.HLE,
    timing=TimingMode.BALANCED,
)


def resolve_profile(
    requested: EmulationProfile,
    capabilities: RuntimeCapabilities = RuntimeCapabilities(),
) -> ResolvedProfile:
    """Resolve a preset without silently enabling unimplemented approximations.

    The safe reference implementation is the default for every capability.
    A feature is selected only when the runtime explicitly advertises it.
    """
    cpu = requested.cpu
    vdp = requested.vdp
    audio = requested.audio
    timing = requested.timing
    fallbacks: list[str] = []

    if cpu is CpuBackend.LLVM_JIT and not capabilities.llvm_z80_jit:
        cpu = CpuBackend.INTERPRETER
        fallbacks.append("LLVM Z80 JIT unavailable; using interpreter")

    if vdp is VdpMode.FAST and not capabilities.vdp_fast_path:
        vdp = VdpMode.REFERENCE
        fallbacks.append("fast VDP path unavailable; using reference VDP")

    if audio is AudioMode.HLE and not capabilities.psg_hle:
        audio = AudioMode.PSG_CHIP
        fallbacks.append("PSG HLE unavailable; using chip model")

    if timing is TimingMode.BALANCED and not capabilities.balanced_timing:
        timing = TimingMode.STRICT
        fallbacks.append("balanced timing unavailable; using strict timing")
    elif timing is TimingMode.RELAXED and not capabilities.relaxed_timing:
        timing = TimingMode.STRICT
        fallbacks.append("relaxed timing unavailable; using strict timing")

    effective = EmulationProfile(
        name=requested.name,
        cpu=cpu,
        vdp=vdp,
        audio=audio,
        timing=timing,
    )
    return ResolvedProfile(
        requested=requested,
        effective=effective,
        fallbacks=tuple(fallbacks),
    )
