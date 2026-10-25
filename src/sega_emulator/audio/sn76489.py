"""Modelo determinístico do PSG SN76489AN do Sega SG-1000.

O núcleo é acionado pelo clock de entrada e divide-o por 16 antes de atualizar
os geradores. A saída digital mono é uma aproximação da mistura dos quatro
canais; não modela a resposta analógica nem afirma temporização sub-instrução
dos writes de I/O feitos pela máquina.
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
SAMPLE_BUFFER_SECONDS = 1

# Níveis nominais de atenuação em passos de aproximadamente 2 dB.
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
    """SN76489AN para SG-1000, com três tons, ruído e saída mono."""

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
        self._sample_buffer_capacity = sample_rate * SAMPLE_BUFFER_SECONDS
        self._pending_samples: deque[float] = deque(maxlen=self._sample_buffer_capacity)
        self._render_pending: deque[float] = deque()
        self.dropped_samples = 0
        self.reset()

    def reset(self) -> None:
        """Restaura um estado determinístico e limpa os buffers de saída."""
        self.tone_periods = [0, 0, 0]
        self.attenuations = [15, 15, 15, 15]
        self.noise_control = 0
        self._latched_register = 0
        self._tone_counters = [0, 0, 0]
        self._tone_outputs = [1, 1, 1]
        self._noise_counter = 0
        self._noise_counter_phase = 0
        self._lfsr = LFSR_SEED
        self._clock_divider_phase = 0
        self._output_phase = 0
        self._noise_period = 0x10
        self._pending_samples.clear()
        self._render_pending.clear()
        self.dropped_samples = 0
        self._refresh_noise_period()

    def write(self, value: int) -> None:
        """Processa um byte latch/data do protocolo serial do PSG."""
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
            if channel == 2:
                self._refresh_noise_period()
            return
        if channel < TONE_CHANNELS and is_volume:
            self.attenuations[channel] = data & 0x0F
            return
        if channel == NOISE_CHANNEL and not is_volume:
            self.noise_control = data & 0x07
            self._refresh_noise_period()
            self._reset_noise_generator()
            return
        self.attenuations[NOISE_CHANNEL] = data & 0x0F

    def _refresh_noise_period(self) -> None:
        """Atualiza o divisor efetivo sempre que o controle ou tom 3 muda."""
        rate = self.noise_control & 0x03
        self._noise_period = (
            max(1, self.tone_periods[2]) if rate == 3 else (0x10 << rate)
        )

    def _reset_noise_generator(self) -> None:
        self._lfsr = LFSR_SEED
        self._noise_counter = 0
        self._noise_counter_phase = 0

    def _shift_noise_register(self) -> None:
        # Variante de 15 bits adotada para o SN76489AN do SG-1000.
        if self.noise_control & 0x04:
            feedback = (self._lfsr ^ (self._lfsr >> 1)) & 1
        else:
            feedback = self._lfsr & 1
        self._lfsr = ((self._lfsr >> 1) | (feedback << 14)) & LFSR_MASK

    def _tick_internal_clock(self) -> None:
        for channel, period in enumerate(self.tone_periods):
            # Hipótese de variante mantida explicitamente: períodos 0/1 ficam altos.
            if period <= 1:
                self._tone_outputs[channel] = 1
                self._tone_counters[channel] = 0
                continue
            self._tone_counters[channel] += 1
            if self._tone_counters[channel] >= period:
                self._tone_counters[channel] = 0
                self._tone_outputs[channel] ^= 1

        self._noise_counter += 1
        if self._noise_counter >= self._noise_period:
            self._noise_counter = 0
            # O LFSR é acionado a cada segunda borda do divisor de ruído.
            # Isso produz clock / 512, / 1024 e / 2048 nos três modos fixos.
            self._noise_counter_phase ^= 1
            if self._noise_counter_phase:
                self._shift_noise_register()

    def advance_clocks(self, clocks: int) -> None:
        """Avança clocks de entrada e coloca as amostras novas no buffer limitado."""
        if type(clocks) is not int or clocks < 0:
            raise ValueError("clocks deve ser um inteiro não negativo.")
        self._advance_clocks(clocks, self._pending_samples)

    def _advance_clocks(self, clocks: int, sample_sink: deque[float]) -> None:
        internal_total = self._clock_divider_phase + clocks
        internal_clocks, self._clock_divider_phase = divmod(
            internal_total, CLOCK_DIVIDER
        )
        for _ in range(internal_clocks):
            self._tick_internal_clock()
            self._output_phase += self.sample_rate * CLOCK_DIVIDER
            while self._output_phase >= self.clock_hz:
                sample = self._mix()
                if sample_sink is self._pending_samples and len(sample_sink) == (
                    self._sample_buffer_capacity
                ):
                    self.dropped_samples += 1
                sample_sink.append(sample)
                self._output_phase -= self.clock_hz

    def drain_samples(self, max_samples: int | None = None) -> list[float]:
        """Consome amostras geradas por advance_clocks(), em ordem FIFO.

        O buffer guarda até um segundo de áudio. Quando fica cheio, descarta as
        amostras mais antigas e incrementa dropped_samples para tornar a perda
        observável, em vez de silenciosa.
        """
        if max_samples is not None and (
            type(max_samples) is not int or max_samples < 0
        ):
            raise ValueError("max_samples deve ser None ou inteiro não negativo.")
        count = (
            len(self._pending_samples)
            if max_samples is None
            else min(max_samples, len(self._pending_samples))
        )
        return [self._pending_samples.popleft() for _ in range(count)]

    def _mix(self) -> float:
        total = sum(
            _ATTENUATION[self.attenuations[channel]]
            * (1.0 if self._tone_outputs[channel] else -1.0)
            for channel in range(TONE_CHANNELS)
        )
        total += _ATTENUATION[self.attenuations[NOISE_CHANNEL]] * (
            1.0 if self._lfsr & 1 else -1.0
        )
        return total * 0.25

    def render(self, sample_count: int) -> list[float]:
        """Avança o chip e devolve exatamente sample_count amostras mono.

        Esta API usa a mesma fase de clock que advance_clocks(), mas devolve
        diretamente suas amostras, sem também inseri-las no buffer de drain.
        Se uma taxa de amostragem alta produzir várias amostras num mesmo tick,
        as excedentes ficam pendentes para a próxima chamada de render().
        """
        if type(sample_count) is not int or sample_count < 0:
            raise ValueError("sample_count deve ser um inteiro não negativo.")
        output: list[float] = []
        while self._render_pending and len(output) < sample_count:
            output.append(self._render_pending.popleft())
        while len(output) < sample_count:
            clocks_to_next_tick = CLOCK_DIVIDER - self._clock_divider_phase
            self._advance_clocks(clocks_to_next_tick, self._render_pending)
            while self._render_pending and len(output) < sample_count:
                output.append(self._render_pending.popleft())
        return output
