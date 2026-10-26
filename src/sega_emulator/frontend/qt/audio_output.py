"""Conversão determinística de amostras do PSG para PCM da saída SDL."""

from __future__ import annotations

import math
from array import array
from collections.abc import Sequence

PSG_SAMPLE_RATE = 44_100
_FILTER_TAPS = 63
_FILTER_HALF = _FILTER_TAPS // 2
_PHASES = 1024


def _pcm16(samples: Sequence[float]) -> bytes:
    return array(
        "h",
        (max(-32768, min(32767, round(sample * 32767))) for sample in samples),
    ).tobytes()


def _sinc(value: float) -> float:
    if abs(value) < 1e-12:
        return 1.0
    return math.sin(math.pi * value) / (math.pi * value)


class PcmResampler:
    """Reamostrador FIR sinc/Blackman com estado preservado entre blocos.

    A saída usa fase fracionária quantizada em 1/1024 de amostra e coeficientes
    normalizados. O filtro simétrico é causalizado com atraso de grupo de 31
    amostras, sem exigir amostras futuras entre blocos; `flush` completa a cauda
    com zeros. O estado do PSG e seus clocks não são alterados.
    """

    def __init__(self, output_rate: int) -> None:
        if type(output_rate) is not int or output_rate <= 0:
            raise ValueError("output_rate deve ser um inteiro positivo")
        self.output_rate = output_rate
        self._step = PSG_SAMPLE_RATE / output_rate
        self._cutoff = min(1.0, output_rate / PSG_SAMPLE_RATE) * 0.94
        self._coefficients = self._build_coefficients()
        self.reset()

    def reset(self) -> None:
        """Descarta o estado de fluxo, por exemplo ao reiniciar a emulação."""
        self._buffer: list[float] = []
        self._buffer_start = 0
        self._available_count = 0
        self._real_input_count = 0
        self._next_position = -float(_FILTER_HALF)
        self._emitted_count = 0

    def _build_coefficients(self) -> tuple[tuple[float, ...], ...]:
        table: list[tuple[float, ...]] = []
        for phase in range(_PHASES):
            fraction = phase / _PHASES
            weights: list[float] = []
            for offset in range(-_FILTER_HALF, _FILTER_HALF + 1):
                distance = offset - fraction
                window = (
                    0.42
                    + 0.5 * math.cos(math.pi * distance / (_FILTER_HALF + 1))
                    + 0.08 * math.cos(
                        2 * math.pi * distance / (_FILTER_HALF + 1)
                    )
                )
                weight = self._cutoff * _sinc(self._cutoff * distance) * window
                weights.append(weight)
            total = sum(weights)
            table.append(tuple(weight / total for weight in weights))
        return tuple(table)

    def process(self, samples: Sequence[float]) -> bytes:
        """Aceita um bloco e devolve PCM pronto; preserva fase e histórico."""
        if not samples:
            return b""
        if self.output_rate == PSG_SAMPLE_RATE:
            return _pcm16(samples)
        self._buffer.extend(float(sample) for sample in samples)
        self._available_count += len(samples)
        self._real_input_count += len(samples)
        pcm = self._emit(flushing=False)
        self._discard_old_samples()
        return pcm

    def flush(self) -> bytes:
        """Emite a cauda pendente uma vez, completando o filtro com silêncio."""
        if self.output_rate == PSG_SAMPLE_RATE or self._real_input_count == 0:
            self.reset()
            return b""
        self._buffer.extend([0.0] * (_FILTER_HALF + 1))
        self._available_count += _FILTER_HALF + 1
        pcm = self._emit(flushing=True)
        self.reset()
        return pcm

    def _emit(self, *, flushing: bool) -> bytes:
        output: list[float] = []
        target_count = round(self._real_input_count * self.output_rate / PSG_SAMPLE_RATE)
        while True:
            if flushing:
                if (
                    self._next_position >= self._real_input_count
                    or self._emitted_count >= target_count
                ):
                    break
            elif (
                self._next_position + _FILTER_HALF >= self._available_count
                or self._emitted_count >= target_count
            ):
                break

            integer_position = math.floor(self._next_position)
            fraction = self._next_position - integer_position
            phase = min(_PHASES - 1, int(fraction * _PHASES))
            coefficients = self._coefficients[phase]
            value = 0.0
            for tap, coefficient in enumerate(coefficients):
                source_index = integer_position + tap - _FILTER_HALF
                if self._buffer_start <= source_index < self._available_count:
                    value += self._buffer[source_index - self._buffer_start] * coefficient
            output.append(value)
            self._emitted_count += 1
            self._next_position += self._step
        return _pcm16(output)

    def _discard_old_samples(self) -> None:
        keep_from = max(0, math.floor(self._next_position) - _FILTER_HALF)
        discard = min(keep_from - self._buffer_start, len(self._buffer))
        if discard > 0:
            del self._buffer[:discard]
            self._buffer_start += discard


def samples_to_pcm(samples: Sequence[float], output_rate: int) -> bytes:
    """Converte um bloco completo para PCM mono signed 16-bit.

    Para taxas diferentes de 44,1 kHz usa FIR sinc com janela Blackman para
    suprimir frequências que sofreriam aliasing na redução de taxa. Esta função
    de conveniência fecha o fluxo ao final; a reprodução contínua deve usar
    `PcmResampler.process` entre quadros e chamar `flush` somente ao encerrar.
    """
    if type(output_rate) is not int or output_rate <= 0:
        raise ValueError("output_rate deve ser um inteiro positivo")
    if not samples:
        return b""
    if output_rate == PSG_SAMPLE_RATE:
        return _pcm16(samples)
    resampler = PcmResampler(output_rate)
    return resampler.process(samples) + resampler.flush()
