"""Gera hashes de framebuffer/áudio e histogramas de instruções para uma ROM."""

from __future__ import annotations

import collections
import hashlib
import json
import pathlib
import sys
import time
import zlib

from sega_emulator.machine.sg1000 import SG1000Machine
from sega_emulator.rom_loader import load_rom

loaded = load_rom(sys.argv[1])  # Extrai o membro do ZIP corretamente.
rom = loaded.data
crc = f"{zlib.crc32(rom):08X}"
print(f"ROM: {loaded.display_name}  {len(rom)} bytes  CRC32={crc}")
output_path = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else "golden.json")

machine = SG1000Machine(rom)
machine.joystick1 = 0xFF
machine.joystick2 = 0xFF
audio_hash = hashlib.sha256()
framebuffer_hashes = {}
sample_count = 0
scripted_input = {
    90: 0xEF,
    96: 0xFF,
    130: 0xDF,
    136: 0xFF,
    170: 0xFB,
    230: 0xFF,
    240: 0xF7,
    290: 0xFF,
}
distinct_framebuffers = set()

for frame in range(1, 301):
    if frame in scripted_input:
        machine.joystick1 = scripted_input[frame]
    machine.run_frame()
    distinct_framebuffers.add(hash(machine.vdp.framebuffer))
    samples = machine.drain_audio_samples()
    sample_count += len(samples)
    audio_hash.update(
        b"".join(
            int(round(sample * 1e6)).to_bytes(4, "little", signed=True)
            for sample in samples
        )
    )
    if frame in (30, 60, 120, 200, 300):
        framebuffer_hashes[frame] = hashlib.sha256(
            machine.vdp.framebuffer
        ).hexdigest()

print(
    "distinct framebuffers over 300 frames (scripted input):",
    len(distinct_framebuffers),
)
print(
    "audio samples/frame:",
    sample_count / 300,
    " sample_rate:",
    machine.psg.sample_rate,
)
video_active = bool(machine.vdp.registers[1] & 0x40)
print("video_ativo:", video_active, " (registrador 1 =", hex(machine.vdp.registers[1]), ")")
if not video_active:
    print(
        "AVISO: vídeo desativado ao fim de 300 quadros - ROM provavelmente não foi "
        "carregada corretamente; hashes NÃO são uma linha de base válida."
    )
print("audio_sha256:", audio_hash.hexdigest())
for frame, digest in framebuffer_hashes.items():
    print(f"fb@{frame}:", digest[:16])

output_path.write_text(
    json.dumps(
        {
            "rom_name": loaded.display_name,
            "rom_crc32": crc,
            "frames": 300,
            "audio_sha256": audio_hash.hexdigest(),
            "audio_samples": sample_count,
            "framebuffer_sha256": framebuffer_hashes,
        },
        indent=1,
    )
    + "\n",
    encoding="utf-8",
)

# Histograma de opcodes com reconhecimento simples de prefixos.
machine = SG1000Machine(rom)
machine.joystick1 = 0xFF
machine.joystick2 = 0xFF
machine.run_frames(60)
read = machine.memory_bus.read
cpu = machine.cpu
counts: collections.Counter[str] = collections.Counter()
tstates: collections.Counter[str] = collections.Counter()
instruction_count = 0
start_frame = machine.vdp.frame_number

while machine.vdp.frame_number < start_frame + 120:
    pc = cpu.pc
    opcode = read(pc)
    if opcode in (0xCB, 0xED):
        key = f"{opcode:02X}{read((pc + 1) & 0xFFFF):02X}"
    elif opcode in (0xDD, 0xFD):
        key = f"{opcode:02X}xx"
    else:
        key = f"{opcode:02X}"
    cycles = machine.step()
    counts[key] += 1
    tstates[key] += cycles
    instruction_count += 1

print(f"\n{instruction_count} instr, distinct opcode groups: {len(counts)}")
cumulative = 0
for key, count in counts.most_common(14):
    cumulative += count
    print(
        f"  {key:5s} {count / instruction_count * 100:5.1f}%  "
        f"cum {cumulative / instruction_count * 100:5.1f}%  "
        f"avgT={tstates[key] / count:.1f}"
    )
prefixed = sum(
    count for key, count in counts.items() if key[:2] in ("CB", "ED", "DD", "FD")
)
print(f"prefixed share: {prefixed / instruction_count * 100:.1f}%")

# Mede o custo de rasterização das scanlines dentro de um avanço normal.
machine = SG1000Machine(rom)
machine.run_frames(60)
vdp = machine.vdp
original_render_scanline = vdp._render_scanline
scanline_elapsed_ns = 0
scanline_count = 0


def timed_render_scanline(*args, **kwargs):
    global scanline_elapsed_ns, scanline_count
    started_ns = time.perf_counter_ns()
    original_render_scanline(*args, **kwargs)
    scanline_elapsed_ns += time.perf_counter_ns() - started_ns
    scanline_count += 1


vdp._render_scanline = timed_render_scanline
started = time.perf_counter()
machine.run_frames(120)
elapsed = time.perf_counter() - started
print(
    f"\nVDP scanline render: {scanline_elapsed_ns / 1e9:.3f}s of {elapsed:.3f}s "
    f"total ({scanline_elapsed_ns / 1e9 / elapsed * 100:.1f}%), "
    f"{scanline_count} lines, {scanline_elapsed_ns / scanline_count / 1000:.1f} us/line"
)
