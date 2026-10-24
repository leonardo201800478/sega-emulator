"""Modelo determinístico do PSG SN76489AN usado no Sega SG-1000.

O núcleo opera com o clock de entrada do chip e um divisor interno por 16.
O LFSR segue a variante de 15 bits documentada para o SG-1000 (realimentação
branca pelos bits 0 e 1). A saída é uma mistura mono normalizada; não inclui
a resposta analógica do circuito nem integração com a máquina/frontend.
"""

from __future__ import annotations

from collections import deque

DEFAULT_CLOCK_HZ = 3_579_545
DEFAULT_SAMPLE_RATE = 44_100
CLOCK_DIVIDER = 16
TONE_CHANNELS = 3
NOISE_CHANNEL = 3
LFSR_MASK = 0x7FFF
LFSR_SEED = 0x4000

# Um passo por nível: atenuação aproximada de 2 dB, valor 15 = silêncio.
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
    """PSG com três canais de onda quadrada, ruído e saída mono determinística."""

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
        self.tone_periods = [0, 0, 0]
        self.attenuations = [15, 15, 15, 15]
        self.noise_control = 0
        self._latched_register = 0
        self._tone_counters = [0, 0, 0]
        self._tone_outputs = [1, 1, 1]
        self._noise_counter = 0
        self._noise_clock = 0
        self._lfsr = LFSR_SEED
        self._clock_divider_phase = 0
        self._sample_phase = 0
        self._output_phase = 0
        self._pending_samples.clear()
        self._output_phase = 0
        self._pending_samples: deque[float] = deque(maxlen=self.sample_rate)

    def reset(self) -> None:
        """Restaura um estado inicial determinístico e silencioso."""
        self.tone_periods[:] = [0, 0, 0]
        self.attenuations[:] = [15, 15, 15, 15]
        self.noise_control = 0
        self._latched_register = 0
        self._tone_counters[:] = [0, 0, 0]
        self._tone_outputs[:] = [1, 1, 1]
        self._noise_counter = 0
        self._noise_clock = 0
        self._lfsr = LFSR_SEED
        self._clock_divider_phase = 0
        self._sample_phase = 0

    def write(self, value: int) -> None:
        """Processa um byte latch/data no barramento serial do PSG."""
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
            self._reset_noise_generator()
            return
        self.attenuations[NOISE_CHANNEL] = data & 0x0F

    def _reset_noise_generator(self) -> None:
        self._lfsr = LFSR_SEED
        self._noise_clock = 0
        self._noise_counter = 0

    def _shift_noise_register(self) -> None:
        # SN76489AN/SG-1000: 15-bit LFSR; white-noise taps are bits 0 and 1.
        if self.noise_control & 0x04:
            feedback = (self._lfsr ^ (self._lfsr >> 1)) & 1
        else:
            feedback = self._lfsr & 1
        self._lfsr = ((self._lfsr >> 1) | (feedback << 14)) & LFSR_MASK

    def _tick_internal_clock(self) -> None:
        for channel, period in enumerate(self.tone_periods):
            # No SN76489AN discreto, períodos 0 e 1 mantêm a saída alta.
            if period <= 1:
                self._tone_outputs[channel] = 1
                self._tone_counters[channel] = 0
                continue
            self._tone_counters[channel] += 1
            if self._tone_counters[channel] >= period:
                self._tone_counters[channel] = 0
                self._tone_outputs[channel] ^= 1

        rate = self.noise_control & 0x03
        noise_period = max(1, self.tone_periods[2]) if rate == 3 else (0x10 << rate)
        self._noise_counter += 1
        if self._noise_counter >= noise_period:
            self._noise_counter = 0
            self._noise_clock ^= 1
            if self._noise_clock:
                self._shift_noise_register()

    def advance_clocks(self, clocks: int) -> None:
        """Avança clocks de entrada do chip sem gerar amostras de áudio."""
        if type(clocks) is not int or clocks < 0:
            raise ValueError("clocks deve ser um inteiro não negativo.")
        internal_total = self._clock_divider_phase + clocks
        internal_clocks, self._clock_divider_phase = divmod(
            internal_total, CLOCK_DIVIDER
        )
        for _ in range(internal_clocks):
            self._tick_internal_clock()
            # Mantém a taxa de amostragem ligada ao clock real do chip.
            self._output_phase += self.sample_rate * CLOCK_DIVIDER
            while self._output_phase >= self.clock_hz:
                self._pending_samples.append(self._mix())
                self._output_phase -= self.clock_hz

    def drain_samples(self, max_samples: int | None = None) -> list[float]:
        """Retira amostras produzidas pelo avanço do clock da máquina.

        O buffer mantém no máximo um segundo de áudio para que execuções
        headless longas não consumam memória sem limite.
        """
        if max_samples is not None and (type(max_samples) is not int or max_samples < 0):
            raise ValueError("max_samples deve ser None ou inteiro não negativo.")
        count = len(self._pending_samples) if max_samples is None else min(
            max_samples, len(self._pending_samples)
        )
        return [self._pending_samples.popleft() for _ in range(count)]

    def _mix(self) -> float:
        total = sum(
            _ATTENUATION[self.attenuations[channel]]
            * (1.0 if self._tone_outputs[channel] else -1.0)
            for channel in range(TONE_CHANNELS)
        )
        noise = _ATTENUATION[self.attenuations[NOISE_CHANNEL]]
        total += noise * (1.0 if self._lfsr & 1 else -1.0)
        return total * 0.25

    def render(self, sample_count: int) -> list[float]:
        """Gera amostras mono; a fase fracionária preserva a taxa entre chamadas."""
        if type(sample_count) is not int or sample_count < 0:
            raise ValueError("sample_count deve ser um inteiro não negativo.")
        output: list[float] = []
        for _ in range(sample_count):
            self._sample_phase += self.clock_hz
            clocks, self._sample_phase = divmod(self._sample_phase, self.sample_rate)
            self.advance_clocks(clocks)
            output.append(self._mix())
        return output
