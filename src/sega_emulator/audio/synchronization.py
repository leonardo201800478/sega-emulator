"""Sincronização determinística de eventos de escrita no PSG por T-state."""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass
from typing import Protocol


class _ClockedAudioDevice(Protocol):
    def advance_clocks(self, clocks: int) -> None: ...

    def write(self, value: int) -> None: ...


@dataclass(frozen=True, slots=True)
class AudioWriteEvent:
    """Escrita serial do PSG associada a um T-state absoluto da máquina."""

    tstate: int
    value: int


class AudioSynchronizer:
    """Avança o PSG em segmentos separados por writes temporizados.

    Os eventos são fornecidos em ordem cronológica pelo executor de CPU.
    Eventos com o mesmo timestamp preservam a ordem de inserção. A classe não
    executa CPU nem inventa ciclos: usa os timestamps nominais que recebe.
    """

    def __init__(self, device: _ClockedAudioDevice) -> None:
        self._device = device
        self.current_tstate = 0
        self._events: deque[AudioWriteEvent] = deque()

    @property
    def pending_events(self) -> tuple[AudioWriteEvent, ...]:
        """Snapshot imutável dos eventos ainda não aplicados."""
        return tuple(self._events)

    def schedule_write(self, tstate: int, value: int) -> None:
        """Agenda um byte do PSG sem alterar antecipadamente seus registradores."""
        if type(tstate) is not int or tstate < 0:
            raise ValueError("tstate deve ser um inteiro não negativo.")
        if type(value) is not int or not 0 <= value <= 0xFF:
            raise ValueError("value deve ser um byte inteiro.")
        if tstate < self.current_tstate:
            raise ValueError("Não é possível agendar uma escrita no passado.")
        if self._events and tstate < self._events[-1].tstate:
            raise ValueError("Os eventos devem ser agendados em ordem cronológica.")
        self._events.append(AudioWriteEvent(tstate, value))

    def advance_to(self, target_tstate: int) -> None:
        """Avança até o T-state absoluto, aplicando cada write em sua posição."""
        if type(target_tstate) is not int or target_tstate < 0:
            raise ValueError("target_tstate deve ser um inteiro não negativo.")
        if target_tstate < self.current_tstate:
            raise ValueError("A linha temporal de áudio não pode retroceder.")

        while self._events and self._events[0].tstate <= target_tstate:
            event = self._events.popleft()
            delta = event.tstate - self.current_tstate
            if delta:
                self._device.advance_clocks(delta)
            self.current_tstate = event.tstate
            self._device.write(event.value)

        delta = target_tstate - self.current_tstate
        if delta:
            self._device.advance_clocks(delta)
        self.current_tstate = target_tstate
