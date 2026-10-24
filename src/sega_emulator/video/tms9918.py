"""Modelo funcional inicial do VDP compatível com TMS9918A do SG-1000.

O relógio lógico é avançado em T-states nominais do Z80. Este modelo ainda não
simula acessos VRAM por pixel, contenção ou temporização elétrica do barramento.
"""

from __future__ import annotations

SCREEN_WIDTH = 256
SCREEN_HEIGHT = 192
TSTATES_PER_LINE = 228
LINES_PER_FRAME = 262
VBLANK_LINE = 192
VRAM_SIZE = 0x4000
VRAM_MASK = VRAM_SIZE - 1

PALETTE: tuple[tuple[int, int, int], ...] = (
    (0, 0, 0),
    (0, 0, 0),
    (33, 200, 66),
    (94, 220, 120),
    (84, 85, 237),
    (125, 118, 252),
    (212, 82, 77),
    (66, 235, 245),
    (252, 85, 84),
    (255, 121, 120),
    (212, 193, 84),
    (230, 206, 128),
    (33, 176, 59),
    (201, 91, 186),
    (204, 204, 204),
    (255, 255, 255),
)


class TMS9918A:
    """VDP com VRAM, registradores, portas, status, sprites e renderização RGB."""

    def __init__(self) -> None:
        self.vram = bytearray(VRAM_SIZE)
        self.registers = bytearray(8)
        self.status = 0x1F
        self.address = 0
        self.read_buffer = 0
        self._control_first: int | None = None
        self._cycles_into_line = 0
        self.scanline = 0
        self.frame_number = 0
        self.framebuffer = bytes(SCREEN_WIDTH * SCREEN_HEIGHT * 3)
        self._frame_ready = False

    def reset(self, *, clear_vram: bool = False) -> None:
        """Reinicia registradores e temporização sem apagar VRAM por padrão.

        O conteúdo da VRAM após reset físico depende do hardware e da memória
        usada; preservar os bytes é uma convenção explícita do modelo.
        """
        if type(clear_vram) is not bool:
            raise TypeError("clear_vram deve ser bool.")
        self.registers[:] = bytes(len(self.registers))
        self.status = 0x1F
        self.address = 0
        self.read_buffer = 0
        self._control_first = None
        self._cycles_into_line = 0
        self.scanline = 0
        self.frame_number = 0
        self.framebuffer = bytes(SCREEN_WIDTH * SCREEN_HEIGHT * 3)
        self._frame_ready = False
        if clear_vram:
            self.vram[:] = bytes(VRAM_SIZE)

    @property
    def interrupt_pending(self) -> bool:
        """A saída INT fica ativa durante VBlank se R1 habilitar interrupções."""
        return bool((self.status & 0x80) and (self.registers[1] & 0x20))

    def read_port(self, port: int) -> int:
        port = (port & 0xFF) & 0xC1
        if port == 0x80:
            value = self.read_buffer
            self.read_buffer = self.vram[self.address & VRAM_MASK]
            self.address = (self.address + 1) & VRAM_MASK
            self._control_first = None
            return value
        if port == 0x81:
            value = self.status
            self.status = 0x1F
            self._control_first = None
            return value
        return 0xFF

    def write_port(self, port: int, value: int) -> None:
        port = (port & 0xFF) & 0xC1
        value &= 0xFF
        if port == 0x80:
            address = self.address & VRAM_MASK
            self.vram[address] = value
            self.read_buffer = value
            self.address = (address + 1) & VRAM_MASK
            self._control_first = None
            return
        if port != 0x81:
            return
        if self._control_first is None:
            self._control_first = value
            return

        first = self._control_first
        self._control_first = None
        if value & 0x80:
            self.registers[value & 0x07] = first
            return

        self.address = (((value & 0x3F) << 8) | first) & VRAM_MASK
        if value & 0x40:
            return
        self.read_buffer = self.vram[self.address & VRAM_MASK]
        self.address = (self.address + 1) & VRAM_MASK

    def advance(self, tstates: int) -> None:
        """Avança linhas e quadros; um quadro NTSC usa 228 T-states por linha."""
        if type(tstates) is not int or tstates < 0:
            raise ValueError("tstates deve ser um inteiro não negativo.")
        remaining = tstates
        while remaining:
            step = min(remaining, TSTATES_PER_LINE - self._cycles_into_line)
            self._cycles_into_line += step
            remaining -= step
            if self._cycles_into_line == TSTATES_PER_LINE:
                self._cycles_into_line = 0
                self.scanline += 1
                if self.scanline == VBLANK_LINE:
                    self.status |= 0x80
                    self._frame_ready = True
                if self.scanline >= LINES_PER_FRAME:
                    self.framebuffer = self.render_frame()
                    self.scanline = 0
                    self.frame_number += 1

    def consume_frame_ready(self) -> bool:
        ready = self._frame_ready
        self._frame_ready = False
        return ready

    def render_frame(self) -> bytes:
        """Renderiza 256x192 RGB24 em Graphics I/II, texto ou multicolor."""
        backdrop = self.registers[7] & 0x0F
        pixels = bytearray(PALETTE[backdrop] * (SCREEN_WIDTH * SCREEN_HEIGHT))
        if not (self.registers[1] & 0x40):
            return bytes(pixels)

        r0, r1 = self.registers[0], self.registers[1]
        mode = ((r1 >> 4) & 1) | ((r1 >> 2) & 2) | (((r0 >> 1) & 1) << 2)
        if mode == 1:
            self._render_text(pixels, backdrop)
        elif mode == 4:
            self._render_graphics_ii(pixels, backdrop)
        elif mode == 2:
            self._render_multicolor(pixels, backdrop)
        else:
            self._render_graphics_i(pixels, backdrop)
        if mode != 1:
            self._render_sprites(pixels)
        return bytes(pixels)

    def _put_pixel(self, pixels: bytearray, x: int, y: int, color: int) -> None:
        if 0 <= x < SCREEN_WIDTH and 0 <= y < SCREEN_HEIGHT:
            offset = (y * SCREEN_WIDTH + x) * 3
            pixels[offset : offset + 3] = bytes(PALETTE[color & 0x0F])

    def _render_graphics_i(self, pixels: bytearray, backdrop: int) -> None:
        name_base = (self.registers[2] & 0x0F) << 10
        pattern_base = (self.registers[4] & 0x07) << 11
        color_base = (self.registers[3] << 6) & 0x3FC0
        for y in range(SCREEN_HEIGHT):
            tile_y, row = divmod(y, 8)
            for tile_x in range(32):
                name = self.vram[(name_base + tile_y * 32 + tile_x) & 0x3FFF]
                pattern = self.vram[(pattern_base + name * 8 + row) & 0x3FFF]
                color = self.vram[(color_base + name // 8) & 0x3FFF]
                foreground = (color >> 4) or backdrop
                background = (color & 0x0F) or backdrop
                for bit in range(8):
                    self._put_pixel(
                        pixels,
                        tile_x * 8 + bit,
                        y,
                        foreground if pattern & (0x80 >> bit) else background,
                    )

    def _render_graphics_ii(self, pixels: bytearray, backdrop: int) -> None:
        name_base = (self.registers[2] & 0x0F) << 10
        pattern_base = (self.registers[4] & 0x04) << 11
        color_base = (self.registers[3] & 0x80) << 6
        for y in range(SCREEN_HEIGHT):
            row = y & 7
            section = (y & 0xC0) << 5
            for tile_x in range(32):
                name = self.vram[(name_base + (y // 8) * 32 + tile_x) & 0x3FFF]
                offset = section + (name << 3) + row
                pattern = self.vram[(pattern_base + offset) & 0x3FFF]
                color = self.vram[(color_base + offset) & 0x3FFF]
                foreground = (color >> 4) or backdrop
                background = (color & 0x0F) or backdrop
                for bit in range(8):
                    self._put_pixel(
                        pixels,
                        tile_x * 8 + bit,
                        y,
                        foreground if pattern & (0x80 >> bit) else background,
                    )

    def _render_text(self, pixels: bytearray, backdrop: int) -> None:
        name_base = (self.registers[2] & 0x0F) << 10
        pattern_base = (self.registers[4] & 0x07) << 11
        foreground = (self.registers[7] >> 4) or backdrop
        for row in range(24):
            for column in range(40):
                character = self.vram[(name_base + row * 40 + column) & 0x3FFF]
                for line in range(8):
                    pattern = self.vram[(pattern_base + character * 8 + line) & 0x3FFF]
                    for bit in range(6):
                        self._put_pixel(
                            pixels,
                            8 + column * 6 + bit,
                            row * 8 + line,
                            foreground if pattern & (0x80 >> bit) else backdrop,
                        )

    def _render_multicolor(self, pixels: bytearray, backdrop: int) -> None:
        name_base = (self.registers[2] & 0x0F) << 10
        pattern_base = (self.registers[4] & 0x07) << 11
        for tile_y in range(24):
            for tile_x in range(32):
                name = self.vram[(name_base + tile_y * 32 + tile_x) & 0x3FFF]
                for local_y in range(8):
                    pattern_row = local_y >> 2
                    pattern = self.vram[
                        (pattern_base + name * 8 + pattern_row) & 0x3FFF
                    ]
                    left = (pattern >> 4) or backdrop
                    right = (pattern & 0x0F) or backdrop
                    for local_x in range(8):
                        self._put_pixel(
                            pixels,
                            tile_x * 8 + local_x,
                            tile_y * 8 + local_y,
                            left if local_x < 4 else right,
                        )

    def _render_sprites(self, pixels: bytearray) -> None:
        """Compõe sprites respeitando ordem, transparência, colisão e overflow.

        O primeiro sprite da tabela tem prioridade visual. Pixels com padrão
        ativo contam para colisão mesmo quando a cor do sprite é zero
        (transparente). Só os quatro primeiros sprites ativos em cada linha
        são desenhados; o quinto é registrado no status quando F=0.
        """
        attr_base = (self.registers[5] & 0x7F) << 7
        pattern_base = (self.registers[6] & 0x07) << 11
        size = 16 if self.registers[1] & 0x02 else 8
        scale = 2 if self.registers[1] & 0x01 else 1

        # Processar por linha reflete o limite físico de quatro sprites por
        # scanline e impede que um sprite posterior cubra um anterior.
        for line in range(SCREEN_HEIGHT):
            active: list[tuple[int, int, int, int]] = []
            fifth_index: int | None = None

            for index in range(32):
                offset = (attr_base + index * 4) & VRAM_MASK
                raw_y, x, pattern_number, color_byte = self.vram[offset : offset + 4]
                if raw_y == 0xD0:
                    break

                y = (raw_y + 1) & 0xFF
                if y >= 0xE0:
                    y -= 0x100
                height = size * scale
                if not y <= line < y + height:
                    continue

                if len(active) == 4:
                    fifth_index = index
                    break
                color = color_byte & 0x0F
                if color_byte & 0x80:
                    x -= 32
                active.append((index, x, pattern_number, color))

            # O overflow é um latch independente da colisão. O manual indica
            # que o fifth-sprite flag não é gerado enquanto F (VBlank) está 1.
            if (
                fifth_index is not None
                and not (self.status & 0x80)
                and not (self.status & 0x40)
            ):
                self.status = (self.status & 0xA0) | 0x40 | fifth_index

            occupied: set[int] = set()
            visible: set[int] = set()
            for _, x, pattern_number, color in active:
                source_y = (line - ((self.vram[(attr_base + _ * 4) & VRAM_MASK] + 1) & 0xFF))
                # Recover the signed Y coordinate used by the VDP's Y+1 rule.
                raw_y = self.vram[(attr_base + _ * 4) & VRAM_MASK]
                sprite_y = (raw_y + 1) & 0xFF
                if sprite_y >= 0xE0:
                    sprite_y -= 0x100
                source_y = (line - sprite_y) // scale

                for source_x in range(size):
                    if size == 16:
                        tile = (
                            (pattern_number & 0xFC)
                            + (source_y // 8) * 2
                            + source_x // 8
                        )
                    else:
                        tile = pattern_number
                    address = pattern_base + tile * 8 + (source_y & 7)
                    pattern = self.vram[address & VRAM_MASK]
                    if not (pattern & (0x80 >> (source_x & 7))):
                        continue

                    for dx in range(scale):
                        px = x + source_x * scale + dx
                        if not 0 <= px < SCREEN_WIDTH:
                            continue
                        key = line * SCREEN_WIDTH + px
                        if key in occupied:
                            self.status |= 0x20
                        else:
                            occupied.add(key)

                        # Transparent color still collides, but does not hide
                        # sprites behind it. The first nontransparent sprite
                        # owns the visible pixel.
                        if color and key not in visible:
                            self._put_pixel(pixels, px, line, color)
                            visible.add(key)
