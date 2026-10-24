"""Modelo determinístico inicial do PSG SN76489 para a família SG-1000.

Este módulo modela escrita nos registradores e geração de amostras mono. Ainda
não integra o relógio da máquina nem pretende reproduzir todas as variantes
analógicas/digitais do SN76489 sem validação diferencial.
"""

from __future__ import annotations

DEFAULT_CLOCK_HZ = 3_579_545
DEFAULT_SAMPLE_RATE = 44_100
TONE_CHANNELS = 3
NOISE_CHANNEL = 3

_ATTENUATION = (
    1.0,
    0.794328,
    0.630957,
    0.501187,
    0.398107,
    0.316228,
    0.251189,
    0.199526,
    0.158489,
    0.125893,
    0.1,
    0.079433,
    0.063096,
    0.050119,
    0.039811,
    0.0,
)


class SN76489:
    """PSG com três canais de tom, ruído, atenuação e saída mono normalizada."""

    def __init__(
        self,
        clock_hz: int = DEFAULT_CLOCK_HZ,
        sample_rate: int = DEFAULT_SAMPLE_RATE,
    ) -> None:
        if type(clock_hz) is not int or clock_hz <= 0:
            raise ValueError("clock_hz deve ser um inteiro positivo.")
        if type(sample_rate) is not int or sample_rate <= 0:
            raise ValueError("sample_rate deve ser um inteiro positivo.")
        self.clock_hz = clock_hz
        self.sample_rate = sample_rate
        self.tone_periods = [0x400, 0x400, 0x400]
        self.attenuations = [15, 15, 15, 15]
        self.noise_control = 0
        self._latched_register = 0
        self._tone_counters = [0, 0, 0]
        self._tone_outputs = [1, 1, 1]
        self._noise_counter = 0
        self._noise_output = 1
        self._lfsr = 0x4000
        self._divider = 0
        self._sample_phase = 0

    def reset(self) -> None:
        """Restaura registradores e osciladores ao estado inicial determinístico."""
        self.tone_periods[:] = [0x400, 0x400, 0x400]
        self.attenuations[:] = [15, 15, 15, 15]
        self.noise_control = 0
        self._latched_register = 0
        self._tone_counters[:] = [0, 0, 0]
        self._tone_outputs[:] = [1, 1, 1]
        self._noise_counter = 0
        self._noise_output = 1
        self._lfsr = 0x4000
        self._divider = 0
        self._sample_phase = 0

    def write(self, value: int) -> None:
        """Escreve um byte no barramento serial do PSG."""
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("value deve ser um byte inteiro.")
        if value & 0x80:
            register = (value >> 4) & 0x07
            self._latched_register = register
            self._write_register(register, value & 0x0F, latched=True)
        else:
            self._write_register(self._latched_register, value & 0x3F, latched=False)

    def _write_register(self, register: int, data: int, *, latched: bool) -> None:
        channel = register >> 1
        is_volume = bool(register & 1)
        if channel < TONE_CHANNELS and not is_volume:
            if latched:
                self.tone_periods[channel] = (self.tone_periods[channel] & 0x3F0) | (
                    data & 0x0F
                )
            else:
                self.tone_periods[channel] = (self.tone_periods[channel] & 0x00F) | (
                    (data & 0x3F) << 4
                )
            self.tone_periods[channel] &= 0x3FF
            return
        if channel < TONE_CHANNELS and is_volume:
            self.attenuations[channel] = data & 0x0F
            return
        if channel == NOISE_CHANNEL and not is_volume:
            self.noise_control = data & 0x07
            self._lfsr = 0x4000
            self._noise_output = 1
            self._noise_counter = 0
            return
        self.attenuations[NOISE_CHANNEL] = data & 0x0F

    def _tick_divider(self) -> None:
        self._divider = (self._divider + 1) & 0x0F
        if self._divider:
            return
        for channel, period in enumerate(self.tone_periods):
            period = period or 0x400
            self._tone_counters[channel] += 1
            if self._tone_counters[channel] >= period:
                self._tone_counters[channel] = 0
                self._tone_outputs[channel] ^= 1
        select = self.noise_control & 0x03
        noise_period = (
            (self.tone_periods[2] or 0x400) if select == 3 else (0x10 << select)
        )
        self._noise_counter += 1
        if self._noise_counter >= noise_period:
            self._noise_counter = 0
            if self.noise_control & 0x04:
                feedback = (self._lfsr ^ (self._lfsr >> 1)) & 1
            else:
                feedback = self._lfsr & 1
            self._lfsr = (self._lfsr >> 1) | (feedback << 14)
            self._noise_output = self._lfsr & 1

    def _mix(self) -> float:
        total = sum(
            _ATTENUATION[self.attenuations[ch]] * (1 if self._tone_outputs[ch] else -1)
            for ch in range(TONE_CHANNELS)
        )
        amplitude = _ATTENUATION[self.attenuations[NOISE_CHANNEL]]
        total += amplitude if self._noise_output else -amplitude
        return total * 0.25

    def render(self, sample_count: int) -> list[float]:
        """Gera amostras mono com acumulação inteira de fase clock/taxa."""
        if type(sample_count) is not int or sample_count < 0:
            raise ValueError("sample_count deve ser um inteiro não negativo.")
        output: list[float] = []
        for _ in range(sample_count):
            self._sample_phase += self.clock_hz
            clocks, self._sample_phase = divmod(self._sample_phase, self.sample_rate)
            for _ in range(clocks):
                self._tick_divider()
            output.append(self._mix())
        return output
