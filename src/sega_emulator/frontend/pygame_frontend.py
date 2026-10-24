"""Frontend opcional Pygame para jogar ROMs SG-1000."""

from __future__ import annotations

from sega_emulator.machine.sg1000 import SG1000Machine


def run(machine: SG1000Machine, scale: int = 3) -> None:
    try:
        import pygame
    except ImportError as exc:
        raise RuntimeError(
            'Frontend gráfico não instalado. Execute: '
            'python -m pip install -e ".[frontend]"'
        ) from exc

    pygame.init()
    screen = pygame.display.set_mode((256 * scale, 192 * scale))
    pygame.display.set_caption("Sega Emulator — SG-1000")
    clock = pygame.time.Clock()
    running = True
    key_map = {
        pygame.K_UP: 0,
        pygame.K_DOWN: 1,
        pygame.K_LEFT: 2,
        pygame.K_RIGHT: 3,
        pygame.K_z: 4,
        pygame.K_x: 5,
        pygame.K_SPACE: 4,
        pygame.K_RETURN: 5,
    }

    try:
        while running:
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    running = False
                elif event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                    running = False

            pressed = pygame.key.get_pressed()
            joystick = 0xFF
            for key, bit in key_map.items():
                if pressed[key]:
                    joystick &= ~(1 << bit)
            machine.set_joystick(1, joystick)

            machine.run_frame()
            frame = pygame.image.frombuffer(
                machine.vdp.framebuffer, (256, 192), "RGB"
            ).copy()
            screen.blit(pygame.transform.scale(frame, screen.get_size()), (0, 0))
            pygame.display.flip()
            clock.tick(60)
    finally:
        pygame.quit()
