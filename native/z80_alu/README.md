# Shared Z80 ALU reference

This small, hardware-neutral crate is retained as a temporary reference for differential tests while SG-1000 uses its in-crate ALU at `native/sg1000_core/src/alu.rs`.

It provides 8-bit ADD/ADC/SUB/SBC/AND/OR/XOR/CP results and flags plus a stdin/stdout vector runner. It is not a Z80 CPU implementation. Keep it only while the independent oracle is useful; it is not a runtime dependency of `sega-sg1000-core`.
