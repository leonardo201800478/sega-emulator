"""Perfis de hardware de cartuchos SG-1000 identificados por hashes conhecidos.

Os CRC-32 vêm do catálogo do Genesis Plus GX. O SHA-256 adicional identifica
a variante de Bomberman Special com logotipo em inglês usada na validação local.
O perfil Type-B exige o adaptador de expansão de RAM de 8 KiB do SG-1000 II.
"""

import hashlib
import zlib

# CRC-32 dos dumps identificados como SYSTEM_SGII_RAM_EXT no Genesis Plus GX.
# Referência: https://github.com/ekeeke/Genesis-Plus-GX/blob/master/core/cart_hw/sms_cart.c
SG1000_II_RAM_EXT_TYPE_B_CRC32 = frozenset(
    {
        0x69FC1494,  # Bomberman Special (Taiwan), CRC também confirmado no MAME
        0xFFC4EE3F,  # Magical Kid Wiz (Taiwan)
        0x2E366CCF,  # The Castle [MSX] (Taiwan)
        0xAAAC12CF,  # Rally-X (Taiwan)
        0xD2EDD329,  # Road Fighter (Taiwan)
    }
)

# SHA-256 da variante "Bomberman Special (Taiwan) (English Logo) (Unl)" usada
# na validação local do projeto; não é necessário incluir a ROM no repositório.
SG1000_II_RAM_EXT_TYPE_B_SHA256 = frozenset(
    {
        "3eff3d6f1f74041f7b732455799d0978ab49724552ff2985f34b76478cd91721",
    }
)

SG1000_II_RAM_EXT_TYPE_B_SIZE = 0x2000


def cartridge_ram_size(cartridge_rom: bytes) -> int:
    """Retorna o tamanho da RAM para perfis conhecidos; padrão é 1 KiB."""
    if not isinstance(cartridge_rom, bytes):
        raise TypeError("cartridge_rom deve ser bytes.")
    crc32 = zlib.crc32(cartridge_rom) & 0xFFFFFFFF
    sha256 = hashlib.sha256(cartridge_rom).hexdigest()
    if (
        crc32 in SG1000_II_RAM_EXT_TYPE_B_CRC32
        or sha256 in SG1000_II_RAM_EXT_TYPE_B_SHA256
    ):
        return SG1000_II_RAM_EXT_TYPE_B_SIZE
    return 0x0400
