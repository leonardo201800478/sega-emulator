"""Perfis de hardware de cartuchos SG-1000 identificados por hashes conhecidos.

Os CRC-32 dos perfis Type-B vêm do catálogo do Genesis Plus GX. O SHA-256
adicional identifica a variante de Bomberman Special com logotipo em inglês.
The Castle (Japan) usa outro mapa: 8 KiB de RAM de cartucho em 8000h–BFFFh,
enquanto a RAM interna permanece em 1 KiB.
"""

import hashlib
import zlib
from dataclasses import dataclass

# CRC-32 dos dumps identificados como SYSTEM_SGII_RAM_EXT no Genesis Plus GX.
# Referência: https://github.com/ekeeke/Genesis-Plus-GX/blob/master/core/cart_hw/sms_cart.c
SG1000_II_RAM_EXT_TYPE_B_CRC32 = frozenset(
    {
        0x69FC1494,  # Bomberman Special (Taiwan)
        0xFFC4EE3F,  # Magical Kid Wiz (Taiwan)
        0x2E366CCF,  # The Castle [MSX] (Taiwan)
        0xAAAC12CF,  # Rally-X (Taiwan)
        0xD2EDD329,  # Road Fighter (Taiwan)
    }
)

SG1000_II_RAM_EXT_TYPE_B_SHA256 = frozenset(
    {
        "3eff3d6f1f74041f7b732455799d0978ab49724552ff2985f34b76478cd91721",
    }
)

# Conversões MSX1 distribuídas por DahJee/Jumbo. O catálogo MAME declara
# RAM adicional de 9.216 bytes no conjunto: 8 KiB de expansão + 1 KiB interno.
# A janela de expansão mapeada aqui é 2000h–3FFFh; os hashes são do MAME.
SG1000_DAHJEE_TYPE_A_CRC32 = frozenset(
    {
        0x223397A1,  # King's Valley / Jinzita
        0x281D2888,  # Knightmare / Moyu Chuanqi
        0x2E7166D5,  # The Legend of Kage / Yingzi Chuanshuo
        0xDF7CBFA5,  # Pippols / Shenqi Huayuan
        0xE0816BB7,  # Star Soldier / Yuzhou Zhanshi
        0x5CBD1163,  # Tank Battalion / Tanke Dazhan
        0xC550B4F0,  # TwinBee / Shuangxiang Pao
    }
)

# Othello Multivision: slot específico, ROM de 32 KiB e RAM de 2 KiB
# conforme o software list do MAME.
SG1000_OTHELLO_CRC32 = frozenset({0xAF4F14BC, 0x1D1A0CA3})

# The Castle (Japan): RAM de cartucho, mapeada em 8000h–BFFFh.
SG1000_THE_CASTLE_JAPAN_CRC32 = 0x092F29D6
SG1000_II_RAM_EXT_TYPE_B_SIZE = 0x2000
SG1000_INTERNAL_RAM_SIZE = 0x0400


@dataclass(frozen=True)
class CartridgeProfile:
    """Descrição mínima de RAM por cartucho, sem inferência pelo tamanho da ROM."""

    internal_ram_size: int = SG1000_INTERNAL_RAM_SIZE
    cartridge_ram_size: int = 0
    cartridge_ram_start: int = 0
    cartridge_ram_end: int = -1


def cartridge_profile(cartridge_rom: bytes) -> CartridgeProfile:
    """Seleciona o mapa de RAM por hash conhecido; ROMs desconhecidas usam o padrão."""
    if not isinstance(cartridge_rom, bytes):
        raise TypeError("cartridge_rom deve ser bytes.")

    crc32 = zlib.crc32(cartridge_rom) & 0xFFFFFFFF
    sha256 = hashlib.sha256(cartridge_rom).hexdigest()

    if (
        crc32 in SG1000_II_RAM_EXT_TYPE_B_CRC32
        or sha256 in SG1000_II_RAM_EXT_TYPE_B_SHA256
    ):
        return CartridgeProfile(internal_ram_size=SG1000_II_RAM_EXT_TYPE_B_SIZE)

    if crc32 in SG1000_DAHJEE_TYPE_A_CRC32:
        # DahJee/Jumbo 8 KiB RAM adapter maps expanded system RAM at C000h–FFFFh.
        return CartridgeProfile(internal_ram_size=SG1000_II_RAM_EXT_TYPE_B_SIZE)

    if crc32 in SG1000_OTHELLO_CRC32:
        return CartridgeProfile(
            cartridge_ram_size=0x0800,
            cartridge_ram_start=0x8000,
            cartridge_ram_end=0xBFFF,
        )

    if crc32 == SG1000_THE_CASTLE_JAPAN_CRC32:
        return CartridgeProfile(
            cartridge_ram_size=SG1000_II_RAM_EXT_TYPE_B_SIZE,
            cartridge_ram_start=0x8000,
            cartridge_ram_end=0xBFFF,
        )

    return CartridgeProfile()


def cartridge_ram_size(cartridge_rom: bytes) -> int:
    """Retorna o tamanho da RAM interna; mantém a API anterior."""
    return cartridge_profile(cartridge_rom).internal_ram_size
