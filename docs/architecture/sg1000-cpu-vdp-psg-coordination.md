> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust (`native/sg1000_core`). A aplicação Qt usa uma fachada Python fina para PyO3 e carregadores de ROM/perfil. Implementações de hardware Python, ferramentas e testes que dependiam delas foram arquivados sob `legacy/` e não são executados pelo projeto ativo. Descrições de estados anteriores neste documento são históricas, não instruções atuais.

# SG-1000 collaborative hardware workstreams

This work is coordinated across three independently reviewable workstreams. They share one machine-level contract; no component may invent timing, status semantics, or device clocks independently.

## Workstream A — Z80 CPU

**Ownership:** instruction semantics, register/flag state, prefix decoder, interrupt requests/acceptance, and instruction T-state accounting.

**Contract with VDP/PSG:** every completed instruction returns its documented T-state count; CPU I/O writes retain their event order. The CPU does not call Python or render video/audio from an opcode handler. Instruction-boundary timing is not described as bus-cycle accuracy.

**Required hand-off:** instruction T-states, memory/port events, interrupt line sampling points, and tests for EI delay/HALT/interrupt acceptance.

## Workstream B — VDP TMS9918A

**Ownership:** register and port protocol, VRAM address/read-ahead behavior, raster state, VBlank/status, sprite priority/overflow/collision, and deterministic framebuffer generation.

**Contract with CPU:** raster advances from the same machine T-state timeline; status reads preserve documented side effects; VDP interrupt output is derived from VDP status and register enables, not inferred by the frontend.

**Contract with PSG/scheduler:** no device independently advances the machine clock. The scheduler converts a common elapsed-time interval into device-specific clock edges while preserving fractional remainder.

**Sprite-limit option:** the hardware mode evaluates at most four active sprites per scanline and reports overflow for the fifth sprite. Diagnostic mode renders all active sprites and suppresses only the fifth-sprite overflow indication; collision/priority behavior remains otherwise active. The default is hardware mode.

## Workstream C — PSG SN76489AN

**Ownership:** latch/data protocol, tone/noise counters, LFSR behavior, attenuation, clock-to-sample accounting, and deterministic output buffers.

**Contract with CPU/VDP:** PSG writes carry machine-relative T-state timestamps. The PSG consumes clock edges from the scheduler and never advances CPU or VDP state. Audio filtering is kept separate from the digital chip state.

## Shared integration contract

- Rust remains authoritative for the standalone machine; Python/PySide6 owns the Qt UI, host integration, ROM/profile loading and application logging; the machine facade only forwards to Rust.
- No independent per-device wall-clock timers or threads.
- Preserve remainders when converting machine T-states to PSG clocks; split execution windows must produce the same state and audio samples as a single equivalent window.
- Record documented behavior, measured/reference behavior, inference, and unresolved hypotheses separately.
- Every change to observable behavior adds a regression test before acceptance.
- Independent validators report PASS/FAIL/BLOCKED/NOT RUN against the exact candidate SHA. CI workflows are validators, not AI agents.

## Current coordination state

- CPU work continues on PR #129; this stacked change does not claim all Z80 opcodes or interrupt modes are complete.
- The Rust VDP exposes a configurable per-line sprite limit; the former Python VDP is archived.
- Qt preferences persist the setting and apply it to the active machine. The Qt UI forwards the setting to the Rust binding. The Python hardware backend has been removed from the active package.
- VDP/PSG timing and full CPU/device synchronization remain instruction-boundary based until timestamped bus-cycle evidence proves otherwise.


## Host audio runtime boundary

PSG sample generation and timestamped register writes remain part of the emulated machine timeline. The host audio session is owned by Rust and must not become a timing master for the Z80, VDP or PSG. Qt/Python may configure the session and read diagnostics, but it must not drain, resample, mix or dispatch PCM. WASAPI Shared is the Windows default and XAudio2 is the alternative. See [native multi-system audio runtime](native-audio-runtime.md).
