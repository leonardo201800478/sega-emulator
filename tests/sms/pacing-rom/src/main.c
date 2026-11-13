/*
 * Sega Emulator — SMS frame pacing reference ROM.
 * Original project source; no commercial ROM or third-party game assets.
 *
 * PACING_PROFILE:
 *   0 = CPU-light
 *   1 = fixed CPU workload
 *   2 = VDP/sprite workload
 *   3 = combined CPU + VDP + PSG workload
 */

typedef unsigned char u8;
typedef unsigned int u16;

#ifndef PACING_PROFILE
#define PACING_PROFILE 0
#endif

#if PACING_PROFILE < 0 || PACING_PROFILE > 3
#error "PACING_PROFILE must be 0, 1, 2, or 3"
#endif

__sfr __at (0xBE) VDP_DATA;
__sfr __at (0xBF) VDP_CTRL;
__sfr __at (0x7F) PSG_DATA;

static volatile u8 cpu_sink;
static u16 frame_counter;

static const u8 palette[16] = {
    0x00, 0x03, 0x0C, 0x30,
    0x0F, 0x33, 0x3C, 0x3F,
    0x15, 0x2A, 0x0A, 0x22,
    0x2C, 0x0D, 0x31, 0x1B
};

static void vdp_register(u8 index, u8 value)
{
    VDP_CTRL = value;
    VDP_CTRL = (u8)(0x80u | index);
}

static void vram_address(u16 address)
{
    VDP_CTRL = (u8)(address & 0xFFu);
    VDP_CTRL = (u8)(0x40u | ((address >> 8) & 0x3Fu));
}

static void cram_address(u8 address)
{
    VDP_CTRL = address;
    VDP_CTRL = 0xC0u;
}

static void write_palette(void)
{
    u8 i;
    cram_address(0);
    for (i = 0; i < 16; ++i) {
        VDP_DATA = palette[i];
    }
}

static void write_solid_tiles(void)
{
    u8 tile;
    u8 row;
    u8 plane;

    vram_address(0x0000u);
    for (tile = 0; tile < 16; ++tile) {
        for (row = 0; row < 8; ++row) {
            for (plane = 0; plane < 4; ++plane) {
                VDP_DATA = ((tile & (u8)(1u << plane)) != 0u) ? 0xFFu : 0x00u;
            }
        }
    }
}

static void write_name_table(void)
{
    u8 row;
    u8 column;

    vram_address(0x3800u);
    for (row = 0; row < 24; ++row) {
        for (column = 0; column < 32; ++column) {
            u8 tile = (u8)(1u + ((row + (column >> 3)) % 15u));
            VDP_DATA = tile;
            VDP_DATA = 0x00u;
        }
    }
}

static void initialize_sprite_table(void)
{
    u8 i;

    vram_address(0x3F00u);
    for (i = 0; i < 64; ++i) {
        VDP_DATA = 0xD0u;
    }

    vram_address(0x3F80u);
    for (i = 0; i < 64; ++i) {
        VDP_DATA = 0x00u;
    }
}

static void initialize_video(void)
{
    /* Keep display disabled while programming the mode and VRAM. */
    vdp_register(0, 0x04u); /* Mode 4 */
    vdp_register(1, 0x00u);
    vdp_register(2, 0x0Eu); /* Name table at 0x3800 */
    vdp_register(5, 0x7Eu); /* Sprite attribute table at 0x3F00 */
    vdp_register(6, 0x00u);
    vdp_register(7, 0x01u);
    vdp_register(8, 0x00u);
    vdp_register(9, 0x00u);
    vdp_register(10, 0x00u);

    write_palette();
    write_solid_tiles();
    write_name_table();
    initialize_sprite_table();

    vdp_register(1, 0x40u); /* Display enabled, 16 KiB VRAM */
}

static void wait_next_vblank(void)
{
    /* Drain a current VBlank before waiting for the next rising edge. */
    while ((VDP_CTRL & 0x80u) != 0u) {
    }
    while ((VDP_CTRL & 0x80u) == 0u) {
    }
}

static void run_fixed_cpu_workload(void)
{
    u16 i;
    u8 value = cpu_sink;

    /* Fixed iteration count; volatile sink prevents dead-code elimination. */
    for (i = 0; i < 256u; ++i) {
        value = (u8)((value << 1) ^ (u8)i ^ (u8)(i >> 3));
    }
    cpu_sink = value;
}

static void update_frame_marker(void)
{
    u8 column = (u8)(frame_counter & 31u);
    u8 tile = (u8)(1u + (frame_counter & 15u));
    u16 address = (u16)(0x3820u + ((u16)column << 1));

    vram_address(address);
    VDP_DATA = tile;
    VDP_DATA = 0x00u;
}

static void update_sprites(void)
{
    u8 i;

    vram_address(0x3F00u);
    for (i = 0; i < 64; ++i) {
        if (i < 8u) {
            VDP_DATA = (u8)(79u + ((i & 1u) << 4));
        } else {
            VDP_DATA = 0xD0u;
        }
    }

    vram_address(0x3F80u);
    for (i = 0; i < 8u; ++i) {
        u8 x = (u8)(frame_counter + (u16)i * 28u);
        VDP_DATA = x;
        VDP_DATA = (u8)(1u + i);
    }
}

static void update_psg(void)
{
    u16 period = (u16)(0x0180u + (frame_counter & 0x001Fu));

    /* Deterministic tone 0 update; volume is initialized once. */
    PSG_DATA = (u8)(0x80u | (period & 0x0Fu));
    PSG_DATA = (u8)((period >> 4) & 0x3Fu);
}

static void initialize_psg(void)
{
#if PACING_PROFILE == 3
    PSG_DATA = 0x98u; /* Tone 0 audible only in the combined workload. */
#else
    PSG_DATA = 0x9Fu; /* Keep the PSG muted in isolated CPU/VDP profiles. */
#endif
    PSG_DATA = 0xBFu; /* Tone 1 muted. */
    PSG_DATA = 0xDFu; /* Tone 2 muted. */
    PSG_DATA = 0xFFu; /* Noise muted. */
}

static void execute_profile(void)
{
#if PACING_PROFILE == 1
    run_fixed_cpu_workload();
#elif PACING_PROFILE == 2
    update_sprites();
#elif PACING_PROFILE == 3
    run_fixed_cpu_workload();
    update_sprites();
    update_psg();
#else
    /* CPU-light: one deterministic tile update per VBlank. */
#endif
    update_frame_marker();
}

void main(void)
{
    initialize_video();
    initialize_psg();

    for (;;) {
        wait_next_vblank();
        ++frame_counter;
        execute_profile();
    }
}
