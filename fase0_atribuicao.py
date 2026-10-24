"""Mede desempenho e atribui custo aproximado por componente na SG-1000."""

from __future__ import annotations

import statistics
import sys
import time

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import load_rom

pc = time.perf_counter
ns = time.perf_counter_ns
loaded = load_rom(sys.argv[1])  # Extrai o membro do ZIP corretamente.
rom = loaded.data
print(f"ROM: {loaded.display_name}  {len(rom)} bytes")

FRAMES = 300
WARM = 60


def new_machine() -> SG1000Machine:
    machine = SG1000Machine(rom)
    machine.joystick1 = 0xFF
    machine.joystick2 = 0xFF
    return machine


def bench_run_frame() -> float:
    machine = new_machine()
    machine.run_frames(WARM)
    started = pc()
    machine.run_frames(FRAMES)
    return pc() - started


def bench_step_path() -> float:
    machine = new_machine()
    machine.run_frames(WARM)
    started = pc()
    target = machine.vdp.frame_number + FRAMES
    while machine.vdp.frame_number < target:
        machine.step()
    return pc() - started


run_frame_times = [bench_run_frame() for _ in range(5)]
step_path_times = [bench_step_path() for _ in range(5)]
for name, samples in (
    ("run_frame (fast path)", run_frame_times),
    ("machine.step loop", step_path_times),
):
    median = statistics.median(samples)
    fps = FRAMES / median
    print(
        f"{name:24s} median {median:.3f}s  fps={fps:6.1f}  "
        f"speed={fps / 59.923 * 100:5.1f}%  best fps={FRAMES / min(samples):.1f}"
    )

# Atribuição por componente, subtraindo o custo estimado de dois timers.
started_ns = ns()
for _ in range(200_000):
    ns()
    ns()
timer_overhead = (ns() - started_ns) / 200_000

machine = new_machine()
machine.run_frames(WARM)
machine.total_instructions = 0
machine.total_tstates = 0
cpu = machine.cpu
vdp = machine.vdp
psg = machine.psg
accumulated = {"cpu": 0, "vdp": 0, "psg": 0, "glue": 0}
instruction_count = 0
start_frame = vdp.frame_number

while vdp.frame_number < start_frame + FRAMES:
    before = ns()
    cpu._q_previous = cpu.q
    cpu.q = 0
    cpu._step_depth = 1
    cycles = cpu._step_impl()
    cpu._step_depth = 0
    after_cpu = ns()

    vdp.advance(cycles)
    after_vdp = ns()
    psg.advance_clocks(cycles)
    after_psg = ns()

    if vdp.status & 0x80 and vdp.registers[1] & 0x20:
        cpu.request_int()
    else:
        cpu.clear_int_request()
    after_glue = ns()

    accumulated["cpu"] += after_cpu - before - timer_overhead
    accumulated["vdp"] += after_vdp - after_cpu - timer_overhead
    accumulated["psg"] += after_psg - after_vdp - timer_overhead
    accumulated["glue"] += after_glue - after_psg - timer_overhead
    instruction_count += 1
    machine.total_instructions += 1
    machine.total_tstates += cycles

total = sum(accumulated.values())
print(
    f"\ninstr={instruction_count} ({instruction_count / FRAMES:.0f}/frame)  "
    f"timer-overhead/pair={timer_overhead:.0f}ns"
)
for name, elapsed_ns in accumulated.items():
    print(
        f"{name:5s} {elapsed_ns / 1e9:6.3f}s  "
        f"{elapsed_ns / total * 100:5.1f}%  "
        f"{elapsed_ns / instruction_count:6.0f} ns/instr"
    )
tstates = machine.total_tstates
print(
    f"sum(instrumented) {total / 1e9:.3f}s -> {FRAMES / (total / 1e9):.1f} "
    f"fps equivalent; avg {total / instruction_count:.0f} ns/instr = "
    f"{total / tstates:.0f} ns/T-state"
)
