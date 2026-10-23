"""Seleção determinística de operandos para validar a ALU Z80 de 16 bits."""

MASK16 = 0xFFFF


def select_left_values(count: int, sampling: str) -> tuple[int, ...]:
    """Seleciona HL em ordem determinística, sequencial ou distribuída."""
    if sampling == "sequential":
        return tuple(range(count))

    if count == 1:
        return (0x8000,)
    if count < 64:
        return tuple(round(index * MASK16 / (count - 1)) for index in range(count))

    # Inclui fronteiras importantes para carry, half-carry, sinal e wraparound.
    boundary_values = (
        0x0000,
        0x0001,
        0x000F,
        0x0010,
        0x00FF,
        0x0100,
        0x0FFF,
        0x1000,
        0x7FFF,
        0x8000,
        0x8001,
        0xFFFE,
        0xFFFF,
    )
    selected = set(boundary_values)
    remaining = count - len(selected)

    if remaining:
        denominator = max(1, remaining - 1)
        for index in range(remaining):
            selected.add(round(index * MASK16 / denominator))

    # Quantis intermediários preenchem colisões com as fronteiras sem enviesar
    # a amostra para uma região específica do domínio.
    if len(selected) < count:
        for index in range(remaining * 2):
            candidate = round((index + 0.5) * MASK16 / (remaining * 2))
            selected.add(candidate)
            if len(selected) == count:
                break

    return tuple(sorted(selected))
