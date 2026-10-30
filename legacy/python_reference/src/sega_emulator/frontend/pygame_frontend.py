"""Frontend opcional Pygame para jogar ROMs SG-1000."""

from __future__ import annotations

from array import array

from sega_emulator.machine.sg1000 import SG1000Machine


def run(machine: SG1000Machine, scale: int = 3) -> None:
    try:
        import pygame
    except ImportError as exc:
        raise RuntimeError(
            "Frontend gráfico não instalado. Execute: "
            'python -m pip install -e ".[frontend]"'
        ) from exc

    pygame.mixer.pre_init(frequency=44_100, size=-16, channels=1, buffer=512)
    pygame.init()
    try:
        audio_channel = pygame.mixer.Channel(0)
    except pygame.error as exc:
        audio_channel = None
        print(f"Saída de áudio indisponível: {exc}")
    screen = pygame.display.set_mode((256 * scale, 192 * scale))
    pygame.display.set_caption("Sega Emulator — SG-1000")
    clock = pygame.time.Clock()
    running = True
    player_one_keys = {
        pygame.K_UP: 0,
        pygame.K_DOWN: 1,
        pygame.K_LEFT: 2,
        pygame.K_RIGHT: 3,
        pygame.K_z: 4,
        pygame.K_x: 5,
        pygame.K_SPACE: 4,
        pygame.K_RETURN: 5,
    }
    player_two_keys = {
        pygame.K_w: 0,
        pygame.K_s: 1,
        pygame.K_a: 2,
        pygame.K_d: 3,
        pygame.K_f: 4,
        pygame.K_g: 5,
    }

    try:
        while running:
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    running = False
                elif event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
                    running = False

            pressed = pygame.key.get_pressed()
            for player, key_map in ((1, player_one_keys), (2, player_two_keys)):
                joystick = 0xFF
                for key, bit in key_map.items():
                    if pressed[key]:
                        joystick &= ~(1 << bit)
                machine.set_joystick(player, joystick)

            machine.run_frame()
            samples = machine.drain_audio_samples()
            if samples and audio_channel is not None:
                pcm = array(
                    "h", (max(-32768, min(32767, round(s * 32767))) for s in samples)
                )
                sound = pygame.mixer.Sound(buffer=pcm.tobytes())
                if audio_channel.get_busy():
                    audio_channel.queue(sound)
                else:
                    audio_channel.play(sound)
            frame = pygame.image.frombuffer(
                machine.vdp.framebuffer, (256, 192), "RGB"
            ).copy()
            screen.blit(pygame.transform.scale(frame, screen.get_size()), (0, 0))
            pygame.display.flip()
            clock.tick(60)
    finally:
        pygame.quit()
