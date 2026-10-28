# External Z80 conformance harness for the SMS CPU core

This harness runs Patrik Rak's [Zilog Z80 CPU Test Suite](https://github.com/raxoft/z80test)
against the Rust Z80 engine in `native/sms_core`. It does not vendor the suite:
the driver checks out fixed upstream commits, keeps the test vectors and CRCs,
builds the assembler, and assembles the selected variant in a temporary directory.

## Scope and limitations

The upstream suite targets a ZX Spectrum, not an SMS. The driver replaces only
the display module with a headless shim. The inline-string reader is retained;
Spectrum ROM printing and decimal/CRC formatting are omitted. Test instructions
and expected CRC values are not changed. The runner uses flat 64 KiB memory and
returns `0xBF` for ports whose low byte is `0xFE`, satisfying the suite's
Spectrum-specific preflight; other port reads return `0xFF`.

This is a **CPU-core semantic conformance run**, not a complete SMS integration
test. It does not validate mapper, VDP, PSG, device-driven interrupts, or
cycle-by-cycle bus timing. A failure is a discrepancy to triage, not automatic
proof of a core bug until CPU variant assumptions and the harness are checked.

## Requirements

- Python 3.10+
- Git
- CMake and a C++ compiler
- Rust/Cargo

Pinned external dependencies:
- `raxoft/z80test`: `c490c0ca94f1021509f8bd656b413861bd7cfb41` (MIT)
- `samsaga2/sjasm`: `d51f5504b24ea9273050ac5fa2896c8970da0b4b` (Sjasm 0.42c, Zlib license)

## Run

From the repository root:

```bash
python tools/z80test/run_sms_z80test.py
```

The default is `z80full`. Other variants are available:

```bash
python tools/z80test/run_sms_z80test.py --variant z80flags
python tools/z80test/run_sms_z80test.py --variant z80doc
python tools/z80test/run_sms_z80test.py --variant z80docflags
python tools/z80test/run_sms_z80test.py --variant z80ccf
python tools/z80test/run_sms_z80test.py --variant z80memptr
```

The runner reports test/failure counters, executed instructions, nominal
T-states and elapsed time. It exits nonzero on upstream failures, CPU errors,
an invalid zero-test result, or instruction-budget exhaustion.

```bash
python tools/z80test/run_sms_z80test.py --variant z80full --max-steps 500000000
python tools/z80test/run_sms_z80test.py --image path/to/z80full.out
python tools/z80test/run_sms_z80test.py --work-dir path/to/work
```

Use `--work-dir` to preserve fetched sources and generated artifacts for
diagnosis. Do not commit generated binaries or commercial ROMs. Preserve
upstream license and attribution if upstream material is ever incorporated.
