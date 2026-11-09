# Sega debugger MVP

The debugger is intentionally a small diagnostic utility, not a full development
IDE or a second emulation frontend.

## Architecture

- \`native/debugger_core\` contains console-independent snapshots, register/value
  models, the \`DebugTarget\` adapter contract, breakpoint handling, bounded trace
  retention and local diagnostic report formatting.
- \`native/sms_debugger\` is the first concrete frontend/target adapter. It owns
  an \`SmsTemporalMachine\` and uses only coarse-grained instruction stepping.
- The debugger does not own CPU, VDP, mapper or scheduler logic. All hardware
  state is read from the core; machine time advances only when the target steps.

Future console adapters should implement \`sega_debugger_core::DebugTarget\`.
They can provide a different CPU architecture and hardware summary without
changing the generic breakpoint, trace retention, report or UI model.

## Current SMS functionality

- Load a local SMS ROM.
- Run, pause and execute one complete instruction.
- Stop before a configured 16-bit PC breakpoint.
- Inspect Z80 registers, mapper banks, memory-control latch, VDP frame counter,
  sequencer phase and machine ticks.
- Inspect a 64-byte memory window.
- View the latest 24 bus/CPU trace records; retention is capped at 2,048 events.
- Export a local text report. ROM bytes are never included.

## Build

The app is a separate Windows-targeted Cargo workspace, like the existing native
application:

\`\`\`powershell
cargo run --manifest-path native/sms_debugger/Cargo.toml
\`\`\`

The generic debugger core is part of the main Rust workspace and is covered by
the normal workspace format, Clippy and test gates.

## Explicit limitations

- The first adapter uses the temporal SMS1 profile provided by
  \`SmsTemporalMachine::new\`; region/VDP profile selection is not exposed yet.
- The trace reports actual temporal bus dispatches, but the UI does not claim
  instruction disassembly or reverse execution.
- The memory window is a read-only view through the SMS memory mapper.
- No commercial ROM, BIOS image, ROM bytes or copyrighted instruction dump is
  written to the repository or diagnostic report.
