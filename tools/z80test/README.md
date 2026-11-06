# External Z80 conformance suites

The Python orchestration driver for Patrik Rak's Zilog Z80 CPU Test Suite has been removed as part of the Rust-only migration. A Rust execution binary still exists at `native/sms_core/src/bin/z80test_runner.rs`; it consumes a preassembled test image and remains coupled to compatibility exports in the SMS core. The upstream suite is not vendored.

The existing Rust Z80 unit, semantic, bus and temporal tests remain the active automated regression. The low-level Rust test binary is not yet a complete harness: source acquisition, Sjasm assembly, preflight setup, expected-vector checks and invocation orchestration were in the removed Python driver. The binary should be relocated to the shared Z80 crate and the missing orchestration implemented in Rust. The external z80full and z80ccf variants are NOT RUN until that Rust-native harness is implemented and validated. Their absence must never be reported as PASS, and the project must not advance the formal ZEXDOC/ZEXALL gate on the basis of unit tests alone.

Pinned upstream dependencies for a future Rust harness:

- raxoft/z80test: c490c0ca94f1021509f8bd656b413861bd7cfb41 (MIT)
- samsaga2/sjasm: d51f5504b24ea9273050ac5fa2896c8970da0b4b (Sjasm 0.42c, Zlib license)

The test suite targets a ZX Spectrum, not an SMS. A future harness must preserve the existing documented preflight shim and expected CRC vectors without changing upstream tests. It must use temporary build directories and must not commit generated binaries or commercial ROMs.

Third-party licensing and attribution are recorded in THIRD_PARTY_NOTICES.md.
