import statistics
import sys
import time

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import load_rom

pc = time.perf_counter
ns = time.perf_counter_ns

loaded = load_rom(sys.argv[1])  # extrai o membro do ZIP corretamente
rom = loaded.data
print(f"ROM: {loaded.display_name}  {len(rom)} bytes")
FRAMES = 300
WARM = 60


def new():
    m = SG1000Machine(rom)
    m.joystick1 = 0xFF
    m.joystick2 = 0xFF
    return m


# A) run_frame fast path vs step path
def bench_run_frame():
    m = new()
    m.run_frames(WARM)
    t = pc()
    m.run_frames(FRAMES)
    return pc() - t


def bench_step_path():
    m = new()
    m.run_frames(WARM)
    t = pc()
    tgt = m.vdp.frame_number + FRAMES
    while m.vdp.frame_number < tgt:
        m.step()
    return pc() - t


rf = [bench_run_frame() for _ in range(5)]
sp = [bench_step_path() for _ in range(5)]
for name, s in (("run_frame (fast path)", rf), ("machine.step loop", sp)):
    med = statistics.median(s)
    print(
        f"{name:24s} median {med:.3f}s  fps={FRAMES / med:6.1f}  "
        f"speed={FRAMES / med / 59.923 * 100:5.1f}%  best fps={FRAMES / min(s):.1f}"
    )

# B) per-component attribution (timer overhead subtracted)
t0 = ns()
for _ in range(200000):
    ns()
    ns()
ovh = (ns() - t0) / 200000  # cost of a pair of timer calls
m = new()
m.run_frames(WARM)
cpu, vdp, psg = m.cpu, m.vdp, m.psg
acc = {"cpu": 0, "vdp": 0, "psg": 0, "glue": 0}
n = 0
start = m.vdp.frame_number
T = ns()
while vdp.frame_number < start + FRAMES:
    a = ns()
    cpu._q_previous = cpu.q
    cpu.q = 0
    cpu._step_depth = 1
    cyc = cpu._step_impl()
    cpu._step_depth = 0
    b = ns()
    vdp.advance(cyc)
    c = ns()
    psg.advance_clocks(cyc)
    d = ns()
    if vdp.status & 0x80 and vdp.registers[1] & 0x20:
        cpu.request_int()
    else:
        cpu.clear_int_request()
    e = ns()
    acc["cpu"] += b - a - ovh
    acc["vdp"] += c - b - ovh
    acc["psg"] += d - c - ovh
    acc["glue"] += e - d - ovh
    n += 1
    m.total_instructions += 1
    m.total_tstates += cyc
tot = sum(acc.values())
print(f"\ninstr={n} ({n / FRAMES:.0f}/frame)  timer-overhead/pair={ovh:.0f}ns")
for k, v in acc.items():
    print(f"{k:5s} {v / 1e9:6.3f}s  {v / tot * 100:5.1f}%  {v / n:6.0f} ns/instr")
print(
    f"sum(instrumented) {tot / 1e9:.3f}s -> {FRAMES / (tot / 1e9):.1f} fps equivalent; "
    f"avg {tot / n:.0f} ns/instr = {tot / n / (59736 * FRAMES / n):.0f} ns/T-state"
)
