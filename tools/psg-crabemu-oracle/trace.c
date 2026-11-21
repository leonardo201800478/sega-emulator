/*
 * SMS Plus GX / CrabEmu SN76489 external trace adapter.
 *
 * This is project-authored glue. It includes the public sn76489.h from an
 * independently checked-out, commit-pinned SMS Plus GX source tree and links
 * that tree's sn76489.c. No reference implementation source is copied here.
 *
 * One call to sn76489_execute_samples() is made per PSG internal clock.
 * With sample_rate = input_clock / 16, CrabEmu's clocks_per_sample is 1.
 */
#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "sn76489.h"

#define MAX_EVENTS 65536
#define REFERENCE_COMMIT "3844b46caa926b6494987b97da63092818c4ddef"

/* The upstream translation unit declares this legacy context symbol. */
sn76489_t psg_sn;

typedef struct {
    uint64_t tick;
    uint8_t value;
} input_event;

static void print_u8_array(const uint8_t *values, size_t count) {
    putchar('[');
    for (size_t i = 0; i < count; ++i) {
        if (i) putchar(',');
        printf("%u", (unsigned)values[i]);
    }
    putchar(']');
}

static void print_u16_array(const uint16_t *values, size_t count) {
    putchar('[');
    for (size_t i = 0; i < count; ++i) {
        if (i) putchar(',');
        printf("%u", (unsigned)values[i]);
    }
    putchar(']');
}

static void print_i8_array(const int8_t *values, size_t count) {
    putchar('[');
    for (size_t i = 0; i < count; ++i) {
        if (i) putchar(',');
        printf("%d", (int)values[i]);
    }
    putchar(']');
}

static void print_float_array(const float *values, size_t count) {
    putchar('[');
    for (size_t i = 0; i < count; ++i) {
        if (i) putchar(',');
        printf("%.9g", (double)values[i]);
    }
    putchar(']');
}

static int load_events(const char *path, input_event *events, size_t *count) {
    FILE *file = fopen(path, "r");
    char line[256];
    *count = 0;

    if (!file) {
        fprintf(stderr, "cannot open events file '%s': %s\n", path, strerror(errno));
        return 0;
    }

    while (fgets(line, sizeof line, file)) {
        char *cursor = line;
        unsigned long long tick;
        int parsed_value;

        while (*cursor == ' ' || *cursor == '\t') ++cursor;
        if (*cursor == '\0' || *cursor == '\n' || *cursor == '#') continue;

        if (sscanf(cursor, "%llu,%i", &tick, &parsed_value) != 2 ||
            parsed_value < 0 || parsed_value > 255 || *count >= MAX_EVENTS) {
            fprintf(stderr, "invalid event row or event limit exceeded: %s", line);
            fclose(file);
            return 0;
        }

        if (*count && tick < events[*count - 1].tick) {
            fprintf(stderr, "event ticks must be nondecreasing\n");
            fclose(file);
            return 0;
        }

        events[*count].tick = (uint64_t)tick;
        events[*count].value = (uint8_t)parsed_value;
        ++*count;
    }

    fclose(file);
    return 1;
}

static void print_header(float clock_hz) {
    printf("{\"schema\":\"smsplus-crabemu-psg-trace-v1\",");
    printf("\"source_repo\":\"libretro/smsplus-gx\",");
    printf("\"source_commit\":\"%s\",", REFERENCE_COMMIT);
    printf("\"engine\":\"crabemu_sn76489\",");
    printf("\"license\":\"GPL-2.0\",");
    printf("\"time_unit\":\"psg_internal_clock\",");
    printf("\"sample_point\":\"sample_before_counter_and_lfsr_update\",");
    printf("\"clock_hz\":%.9g,", (double)clock_hz);
    printf("\"sample_rate_hz\":%.9g,", (double)(clock_hz / 16.0f));
    printf("\"noise_bits\":%u,\"noise_tapped\":%u}\n",
           SN76489_NOISE_BITS_SMS, SN76489_NOISE_TAPPED_SMS);
}

int main(int argc, char **argv) {
    uint64_t calls = 0;
    float clock_hz = 3579545.0f;
    const char *events_path = NULL;
    input_event *events = calloc(MAX_EVENTS, sizeof(*events));
    size_t event_count = 0;
    size_t event_index = 0;
    sn76489_t psg;

    if (!events) {
        fprintf(stderr, "cannot allocate event table\n");
        return 2;
    }

    for (int i = 1; i < argc; ++i) {
        if (!strcmp(argv[i], "--calls") && i + 1 < argc) {
            calls = strtoull(argv[++i], NULL, 10);
        } else if (!strcmp(argv[i], "--events") && i + 1 < argc) {
            events_path = argv[++i];
        } else if (!strcmp(argv[i], "--clock-hz") && i + 1 < argc) {
            clock_hz = strtof(argv[++i], NULL);
        } else {
            fprintf(stderr, "usage: %s --calls N [--events file.csv] [--clock-hz HZ]\n", argv[0]);
            free(events);
            return 2;
        }
    }

    if (calls == 0 || clock_hz <= 0.0f) {
        fprintf(stderr, "--calls and --clock-hz must be positive\n");
        free(events);
        return 2;
    }

    if (events_path && !load_events(events_path, events, &event_count)) {
        free(events);
        return 2;
    }

    sn76489_init(&psg, clock_hz, clock_hz / 16.0f,
                 SN76489_NOISE_BITS_SMS, SN76489_NOISE_TAPPED_SMS);
    print_header(clock_hz);

    for (uint64_t tick = 0; tick < calls; ++tick) {
        uint32_t write_count = 0;
        int16_t sample_left = 0;
        int16_t sample_right = 0;

        while (event_index < event_count && events[event_index].tick == tick) {
            sn76489_write(&psg, events[event_index].value);
            ++event_index;
            ++write_count;
        }

        if (event_index < event_count && events[event_index].tick < tick) {
            fprintf(stderr, "event tick was skipped\n");
            free(events);
            return 3;
        }

        uint16_t tone_before[3];
        uint8_t volume_before[4];
        int8_t tone_state_before[4];
        float counter_before[4];
        memcpy(tone_before, psg.tone, sizeof tone_before);
        memcpy(volume_before, psg.volume, sizeof volume_before);
        memcpy(tone_state_before, psg.tone_state, sizeof tone_state_before);
        memcpy(counter_before, psg.counter, sizeof counter_before);

        const uint16_t noise_shift_before = psg.noise_shift;
        const uint8_t noise_bit_before = (uint8_t)(psg.noise_shift & 1);
        const uint8_t noise_reg = psg.noise;
        const uint8_t latch = psg.latched_reg;

        sn76489_execute_samples(&psg, &sample_left, &sample_right, 1);

        printf("{\"tick\":%" PRIu64 ",\"write_count\":%u,", tick, write_count);
        printf("\"latched_reg\":%u,\"noise_reg\":%u,", (unsigned)latch, (unsigned)noise_reg);
        printf("\"tone\":"); print_u16_array(tone_before, 3);
        printf(",\"volume\":"); print_u8_array(volume_before, 4);
        printf(",\"tone_state_before\":"); print_i8_array(tone_state_before, 4);
        printf(",\"tone_state_after\":"); print_i8_array(psg.tone_state, 4);
        printf(",\"counter_before\":"); print_float_array(counter_before, 4);
        printf(",\"counter_after\":"); print_float_array(psg.counter, 4);
        printf(",\"noise_shift_before\":%u,\"noise_bit_before\":%u,",
               (unsigned)noise_shift_before, (unsigned)noise_bit_before);
        printf("\"noise_shift_after\":%u,\"noise_bit_after\":%u,",
               (unsigned)psg.noise_shift, (unsigned)(psg.noise_shift & 1));
        printf("\"sample_l\":%d,\"sample_r\":%d}\n",
               (int)sample_left, (int)sample_right);
    }

    if (event_index != event_count) {
        fprintf(stderr, "one or more events occur after the requested trace window\n");
        free(events);
        return 4;
    }

    free(events);
    return 0;
}
