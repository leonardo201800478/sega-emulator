"""Perfis de hardware de cartuchos SG-1000 conhecidos por CRC-32.

Os identificadores vêm do catálogo de software do Genesis Plus GX. O perfil
Type-B representa os cartuchos que exigem o adaptador de expansão de RAM de
8 KiB do SG-1000 II; não deve ser aplicado a todos os cartuchos não licenciados.
"""

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

SG1000_II_RAM_EXT_TYPE_B_SIZE = 0x2000


def cartridge_ram_size(cartridge_rom: bytes) -> int:
    """Retorna o tamanho da RAM para perfis conhecidos; padrão é 1 KiB."""
    import zlib

    if not isinstance(cartridge_rom, bytes):
        raise TypeError("cartridge_rom deve ser bytes.")
    if zlib.crc32(cartridge_rom) & 0xFFFFFFFF in SG1000_II_RAM_EXT_TYPE_B_CRC32:
        return SG1000_II_RAM_EXT_TYPE_B_SIZE
    return 0x0400
