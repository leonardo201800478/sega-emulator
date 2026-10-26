"""Conversão entre amostras digitais do PSG e o formato PCM da saída SDL."""

from __future__ import annotations

from array import array
from collections.abc import Sequence

PSG_SAMPLE_RATE = 44_100


def samples_to_pcm(samples: Sequence[float], output_rate: int) -> bytes:
    """Reamostra mono por interpolação linear para a taxa do mixer.

    O modelo SN76489 continua gerando no seu domínio de 44,1 kHz. Esta função
    adapta apenas o buffer de saída e não altera o clock ou o estado do hardware.
    """
    if type(output_rate) is not int or output_rate <= 0:
        raise ValueError("output_rate deve ser um inteiro positivo")
    if not samples:
        return b""
    if output_rate == PSG_SAMPLE_RATE:
        return array(
            "h",
            (max(-32768, min(32767, round(sample * 32767))) for sample in samples),
        ).tobytes()

    output_count = max(1, round(len(samples) * output_rate / PSG_SAMPLE_RATE))
    last_index = len(samples) - 1
    pcm = array("h")
    for index in range(output_count):
        position = index * PSG_SAMPLE_RATE / output_rate
        left = min(int(position), last_index)
        right = min(left + 1, last_index)
        fraction = position - left
        value = samples[left] + (samples[right] - samples[left]) * fraction
        pcm.append(max(-32768, min(32767, round(value * 32767))))
    return pcm.tobytes()
