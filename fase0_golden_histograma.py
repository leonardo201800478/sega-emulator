import collections
import hashlib
import json
import pathlib
import sys
import time
import zlib

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import load_rom

loaded = load_rom(sys.argv[1])  # extrai o membro do ZIP corretamente
rom = loaded.data
CRC = f"{zlib.crc32(rom):08X}"
print(f"ROM: {loaded.display_name}  {len(rom)} bytes  CRC32={CRC}")
OUT = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else "golden.json")
m = SG1000Machine(rom)
m.joystick1 = 0xFF
m.joystick2 = 0xFF
# golden hashes: framebuffer per checkpoint + audio stream hash
audio = hashlib.sha256()
fb = {}
nsamp = 0
SCRIPT = {
    90: 0xEF,
    96: 0xFF,
    130: 0xDF,
    136: 0xFF,
    170: 0xFB,
    230: 0xFF,
    240: 0xF7,
    290: 0xFF,
}
distinct = set()
for f in range(1, 301):
    if f in SCRIPT:
        m.joystick1 = SCRIPT[f]
    m.run_frame()
    distinct.add(hash(m.vdp.framebuffer))
    s = m.drain_audio_samples()
    nsamp += len(s)
    audio.update(
        b"".join(int(round(x * 1e6)).to_bytes(4, "little", signed=True) for x in s)
    )
    if f in (30, 60, 120, 200, 300):
        fb[f] = hashlib.sha256(m.vdp.framebuffer).hexdigest()
print("distinct framebuffers over 300 frames (scripted input):", len(distinct))
print("audio samples/frame:", nsamp / 300, " sample_rate:", m.psg.sample_rate)
print(
    "video_ativo:",
    bool(m.vdp.registers[1] & 0x40),
    " (registrador 1 =",
    hex(m.vdp.registers[1]),
    ")",
)
if not (m.vdp.registers[1] & 0x40):
    print(
        "AVISO: vídeo desativado ao fim de 300 quadros - ROM provavelmente não foi "
        "carregada corretamente; hashes NÃO são uma linha de base válida."
    )
print("audio_sha256:", audio.hexdigest())
for k, v in fb.items():
    print(f"fb@{k}:", v[:16])
json.dump(
    {
        "rom_name": loaded.display_name,
        "rom_crc32": CRC,
        "frames": 300,
        "audio_sha256": audio.hexdigest(),
        "audio_samples": nsamp,
        "framebuffer_sha256": fb,
    },
    open(OUT, "w"),
    indent=1,
)

# opcode histogram (first byte + prefix-aware) over 120 frames
m = SG1000Machine(rom)
m.joystick1 = 0xFF
m.joystick2 = 0xFF
m.run_frames(60)
rd = m.memory_bus.read
cpu = m.cpu
cnt = collections.Counter()
T = collections.Counter()
n = 0
start = m.vdp.frame_number
while m.vdp.frame_number < start + 120:
    pc = cpu.pc
    op = rd(pc)
    key = (
        f"{op:02X}"
        if op not in (0xCB, 0xED, 0xDD, 0xFD)
        else f"{op:02X}{rd((pc + 1) & 0xFFFF):02X}"
        if op in (0xCB, 0xED)
        else f"{op:02X}xx"
    )
    c = m.step()
    cnt[key] += 1
    T[key] += c
    n += 1
print(f"\n{n} instr, distinct opcode groups: {len(cnt)}")
cum = 0
for k, v in cnt.most_common(14):
    cum += v
    print(
        f"  {k:5s} {v / n * 100:5.1f}%  cum {cum / n * 100:5.1f}%  avgT={T[k] / v:.1f}"
    )
pref = sum(v for k, v in cnt.items() if k[:2] in ("CB", "ED", "DD", "FD"))
print(f"prefixed share: {pref / n * 100:.1f}%")

# VDP: advance vs scanline render, and PSG sample volume
m = SG1000Machine(rom)
m.run_frames(60)
v = m.vdp
orig = v._render_scanline
acc = [0, 0]


def w(*a, **k):
    t = time.perf_counter_ns()
    orig(*a, **k)
    acc[0] += time.perf_counter_ns() - t
    acc[1] += 1


v._render_scanline = w
t = time.perf_counter()
m.run_frames(120)
el = time.perf_counter() - t
print(
    f"\nVDP scanline render: {acc[0] / 1e9:.3f}s of {el:.3f}s total "
    f"({acc[0] / 1e9 / el * 100:.1f}%), {acc[1]} lines, "
    f"{acc[0] / acc[1] / 1000:.1f} us/line"
)
