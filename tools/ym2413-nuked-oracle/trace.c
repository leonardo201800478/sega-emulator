/*
 * YM2413 Nuked-OPLL external oracle trace adapter.
 *
 * This file is project-authored glue. It includes the public opll.h from an
 * independently checked-out Nuked-OPLL tree and links that tree's opll.c;
 * it does not contain or copy Nuked implementation code.
 *
 * Output: one JSON object per OPLL_Clock() invocation, after the call returns.
 */
#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "opll.h"

#ifdef YM2413_TRACE_OPERATOR_BOUNDARY
extern uint16_t opll_trace_pg_out_before_operator;
extern uint16_t opll_trace_operator_phase_index;
extern int16_t opll_trace_operator_output;
extern uint8_t opll_trace_selected_instr;
extern uint8_t opll_trace_selected_op;
extern uint8_t opll_trace_testmode_at_dolfo;
extern uint8_t opll_trace_rm_enable_before_operator;
#endif

#define MAX_EVENTS 65536

static int phase_reset_probe = 0;

#ifdef YM2413_TRACE_PHASE_RESET
void ym2413_trace_phase_reset(
    uint32_t cycle,
    uint8_t rm_enable,
    uint32_t eg_dokon,
    uint8_t testmode,
    uint32_t ismod,
    uint32_t reset_required
) {
    if (phase_reset_probe && (cycle == 13 || cycle == 14)) {
        fprintf(stderr, "YM2413_PHASE_RESET,%u,%u,%" PRIu32 ",%u,%" PRIu32 ",%" PRIu32 "\n",
                (unsigned)cycle, (unsigned)rm_enable, eg_dokon, (unsigned)testmode,
                ismod, reset_required);
    }
}
#endif

typedef struct {
    uint64_t call_index;
    uint32_t port;
    uint8_t value;
} input_event;

static void print_u32_array(const uint32_t *v, size_t n) {
    putchar('[');
    for (size_t i = 0; i < n; ++i) {
        if (i) putchar(',');
        printf("%" PRIu32, v[i]);
    }
    putchar(']');
}

static void print_u16_array(const uint16_t *v, size_t n) {
    putchar('[');
    for (size_t i = 0; i < n; ++i) {
        if (i) putchar(',');
        printf("%u", (unsigned)v[i]);
    }
    putchar(']');
}

static void print_u8_array(const uint8_t *v, size_t n) {
    putchar('[');
    for (size_t i = 0; i < n; ++i) {
        if (i) putchar(',');
        printf("%u", (unsigned)v[i]);
    }
    putchar(']');
}

static void print_i16_array(const int16_t *v, size_t n) {
    putchar('[');
    for (size_t i = 0; i < n; ++i) {
        if (i) putchar(',');
        printf("%d", (int)v[i]);
    }
    putchar(']');
}

static int load_events(const char *path, input_event *events, size_t *count) {
    FILE *f = fopen(path, "r");
    char line[256];
    *count = 0;
    if (!f) {
        fprintf(stderr, "cannot open events file '%s': %s\n", path, strerror(errno));
        return 0;
    }
    while (fgets(line, sizeof line, f)) {
        unsigned long long call_index;
        unsigned port, value;
        char *p = line;
        while (*p == ' ' || *p == '\t') ++p;
        if (*p == '\0' || *p == '\n' || *p == '#') continue;
        if (sscanf(p, "%llu,%u,%u", &call_index, &port, &value) != 3 ||
            port > 1 || value > 255 || *count >= MAX_EVENTS) {
            fprintf(stderr, "invalid event row or event limit exceeded: %s", line);
            fclose(f);
            return 0;
        }
        if (*count && call_index <= events[*count - 1].call_index) {
            fprintf(stderr, "events must have strictly increasing OPLL_Clock call indices\n");
            fclose(f);
            return 0;
        }
        events[*count].call_index = (uint64_t)call_index;
        events[*count].port = port;
        events[*count].value = (uint8_t)value;
        ++*count;
    }
    fclose(f);
    return 1;
}

int main(int argc, char **argv) {
    uint64_t calls = 0;
    uint32_t chip_type = opll_type_ym2413;
    const char *events_path = NULL;
    int pg_only = 0;
    int pg_phase_only = 0;
    input_event *events = calloc(MAX_EVENTS, sizeof(*events));
    size_t event_count = 0, event_index = 0;
    opll_t chip;
    if (!events) return 2;

    for (int i = 1; i < argc; ++i) {
        if (!strcmp(argv[i], "--calls") && i + 1 < argc) {
            calls = strtoull(argv[++i], NULL, 10);
        } else if (!strcmp(argv[i], "--events") && i + 1 < argc) {
            events_path = argv[++i];
        } else if (!strcmp(argv[i], "--chip-type") && i + 1 < argc) {
            chip_type = (uint32_t)strtoul(argv[++i], NULL, 0);
        } else if (!strcmp(argv[i], "--pg-only")) {
            pg_only = 1;
        } else if (!strcmp(argv[i], "--pg-phase-only")) {
            pg_phase_only = 1;
        } else if (!strcmp(argv[i], "--phase-reset-probe")) {
            phase_reset_probe = 1;
        } else {
            fprintf(stderr, "usage: %s --calls N [--events file.csv] [--chip-type 0] [--pg-only]\n", argv[0]);
            free(events);
            return 2;
        }
    }
    if (chip_type != opll_type_ym2413) {
        fprintf(stderr, "this adapter supports YM2413 only (chip type 0)\n");
        free(events);
        return 2;
    }
    if (calls == 0) {
        fprintf(stderr, "--calls must be greater than zero\n");
        free(events);
        return 2;
    }
    if (events_path && !load_events(events_path, events, &event_count)) {
        free(events);
        return 2;
    }

    OPLL_Reset(&chip, chip_type);
    if (pg_phase_only) {
        puts("{\"schema\":\"ym2413-nuked-opll-pg-phase-trace-v1\",\"time_unit\":\"opll_clock_call\",\"sample_point\":\"phase_array_after_OPLL_Clock\",\"phase_array_length\":18}");
    } else if (pg_only) {
        puts("{\"schema\":\"ym2413-nuked-opll-pg-trace-v1\",\"time_unit\":\"opll_clock_call\",\"sample_point\":\"pg_increment_after_OPLL_Clock\"}");
    } else {
        puts("{\"schema\":\"ym2413-nuked-opll-trace-v1\",\"time_unit\":\"opll_clock_call\",\"sample_point\":\"after_OPLL_Clock\",\"arrays\":{\"pg_phase\":18,\"eg_state\":18,\"eg_level\":18,\"op_fb1\":9,\"op_fb2\":9,\"op_calc_op_fb1\":9,\"op_calc_op_fb2\":9}}");
    }

    for (uint64_t tick = 0; tick < calls; ++tick) {
        int32_t output[2] = {0, 0};
        uint32_t cycle_before = chip.cycles;
        /* Inputs consumed by OPLL_PhaseCalcIncrement during this call. */
        uint16_t calc_fnum = chip.c_fnum;
        uint16_t calc_block = chip.c_block;
        uint8_t calc_mul = chip.c_multi;
        uint8_t calc_vib = chip.c_vib;
        uint8_t calc_lfo_vib_counter = chip.lfo_vib_counter;
        uint32_t calc_eg_dokon = chip.eg_dokon;
        uint8_t calc_testmode = chip.testmode;
        uint8_t calc_rm_enable = chip.rm_enable;
        /* Inputs consumed by OPLL_Operator during this call. */
        uint16_t op_calc_pg_out = chip.pg_out;
        uint8_t op_calc_eg_out = chip.eg_out;
        uint8_t op_calc_eg_silent = chip.eg_silent;
        uint8_t op_calc_c_fb = chip.c_fb;
        uint8_t op_calc_c_dc = chip.c_dc;
        uint8_t op_calc_c_dm = chip.c_dm;
        int16_t op_calc_op_mod = chip.op_mod;
        int16_t op_calc_op_fbsum = chip.op_fbsum;
        int16_t op_calc_op_fb1[9];
        int16_t op_calc_op_fb2[9];
        memcpy(op_calc_op_fb1, chip.op_fb1, sizeof(op_calc_op_fb1));
        memcpy(op_calc_op_fb2, chip.op_fb2, sizeof(op_calc_op_fb2));
        uint16_t am_calc_lfo_counter = chip.lfo_counter;
        uint8_t am_calc_testmode = chip.testmode;
        uint8_t selected_instr = chip.c_instr;
        uint8_t selected_op = chip.c_op;
        uint32_t phase_ismod =
            ((calc_rm_enable & 0x40) && (cycle_before == 13 || cycle_before == 14))
                ? 0
                : (((cycle_before + 3) / 3) & 1);
        uint32_t phase_next_reset =
            ((calc_testmode & 0x04) != 0)
            || (phase_ismod && (calc_eg_dokon & 0x8000))
            || (!phase_ismod && (calc_eg_dokon & 0x01));
        while (event_index < event_count && events[event_index].call_index == tick) {
            OPLL_Write(&chip, events[event_index].port, events[event_index].value);
            ++event_index;
        }
        if (event_index < event_count && events[event_index].call_index < tick) {
            fprintf(stderr, "event call index was skipped\n");
            free(events);
            return 3;
        }

        if (phase_reset_probe && (chip.cycles == 13 || chip.cycles == 14)) {
            static const uint32_t eg_dokon_probes[4] = {
                0x8000u, 0x0001u, 0x0000u, 0x8001u
            };
            chip.rm_enable = (int8_t)0x80;
            chip.eg_dokon = eg_dokon_probes[(tick / 18u) % 4u];
            chip.testmode = (tick / 72u) % 2u ? 0x04 : 0;
        }

        OPLL_Clock(&chip, output);
#ifdef YM2413_TRACE_OPERATOR_BOUNDARY
        am_calc_testmode = opll_trace_testmode_at_dolfo;
        selected_instr = opll_trace_selected_instr;
        selected_op = opll_trace_selected_op;
#endif
#ifdef YM2413_TRACE_OPERATOR_BOUNDARY
        op_calc_pg_out = opll_trace_pg_out_before_operator;
#endif
        if (pg_phase_only) {
            printf("{\"call_index\":%" PRIu64 ",\"cycle_before\":%" PRIu32
                   ",\"cycle_after\":%" PRIu32 ",\"calc_fnum\":%u,\"calc_block\":%u"
                   ",\"calc_mul\":%u,\"calc_vib\":%u,\"calc_lfo_vib_counter\":%u"
                   ",\"calc_eg_dokon\":%" PRIu32 ",\"calc_testmode\":%u,\"calc_rm_enable\":%u"
                   ",\"phase_next_reset\":%u,\"pg_inc_after\":%" PRIu32
                   ",\"pg_phase_next\":%" PRIu32 ",\"pg_phase\":",
                   tick, cycle_before, chip.cycles, (unsigned)calc_fnum,
                   (unsigned)calc_block, (unsigned)calc_mul, (unsigned)calc_vib,
                   (unsigned)calc_lfo_vib_counter, calc_eg_dokon,
                   (unsigned)calc_testmode, (unsigned)calc_rm_enable,
                   (unsigned)phase_next_reset, chip.pg_inc, chip.pg_phase_next);
            print_u32_array(chip.pg_phase, 18);
            printf(",\"pg_out\":%u}\n", (unsigned)chip.pg_out);
            continue;
        }
        if (pg_only) {
            printf("{\"call_index\":%" PRIu64 ",\"cycle_before\":%" PRIu32 ",\"cycle_after\":%" PRIu32
                   ",\"calc_fnum\":%u,\"calc_block\":%u,\"calc_mul\":%u,\"calc_vib\":%u,\"calc_lfo_vib_counter\":%u,\"pg_inc_after\":%" PRIu32 "}\n",
                   tick, cycle_before, chip.cycles, (unsigned)calc_fnum,
                   (unsigned)calc_block, (unsigned)calc_mul, (unsigned)calc_vib,
                   (unsigned)calc_lfo_vib_counter, chip.pg_inc);
            continue;
        }
        printf("{\"call_index\":%" PRIu64 ",\"cycle_before\":%" PRIu32 ",\"cycle_after\":%" PRIu32,
               tick, cycle_before, chip.cycles);
        printf(",\"calc_fnum\":%u,\"calc_block\":%u,\"calc_mul\":%u,\"calc_vib\":%u,\"calc_lfo_vib_counter\":%u,\"pg_inc_after\":%" PRIu32,
               (unsigned)calc_fnum, (unsigned)calc_block, (unsigned)calc_mul,
               (unsigned)calc_vib, (unsigned)calc_lfo_vib_counter, chip.pg_inc);
        printf(",\"op_calc_pg_out\":%u,\"op_calc_eg_out\":%u,\"op_calc_eg_silent\":%u,\"op_calc_c_fb\":%u,\"op_calc_c_dc\":%u,\"op_calc_c_dm\":%u,\"op_calc_op_mod\":%d,\"op_calc_op_fbsum\":%d",
               (unsigned)op_calc_pg_out, (unsigned)op_calc_eg_out,
               (unsigned)op_calc_eg_silent, (unsigned)op_calc_c_fb,
               (unsigned)op_calc_c_dc, (unsigned)op_calc_c_dm,
               (int)op_calc_op_mod, (int)op_calc_op_fbsum);
        printf(",\"op_calc_op_fb1\":"); print_i16_array(op_calc_op_fb1, 9);
        printf(",\"op_calc_op_fb2\":"); print_i16_array(op_calc_op_fb2, 9);
        printf(",\"output_m\":%" PRId32 ",\"output_r\":%" PRId32, output[0], output[1]);
        printf(",\"pg_inc\":%" PRIu32 ",\"pg_phase_next\":%" PRIu32 ",\"pg_phase\":",
               chip.pg_inc, chip.pg_phase_next);
        print_u32_array(chip.pg_phase, 18);
        printf(",\"pg_out\":%u", (unsigned)chip.pg_out);
        printf(",\"c_instr\":%u,\"c_op\":%u,\"c_tl\":%u,\"c_fb\":%u,\"c_am\":%u,\"c_vib\":%u,\"c_ksr\":%u,\"c_ksl\":%u,\"c_multi\":%u,\"c_fnum\":%u,\"c_block\":%u",
               (unsigned)selected_instr, (unsigned)selected_op, (unsigned)chip.c_tl,
               (unsigned)chip.c_fb, (unsigned)chip.c_am, (unsigned)chip.c_vib,
               (unsigned)chip.c_ksr, (unsigned)chip.c_ksl, (unsigned)chip.c_multi,
               (unsigned)chip.c_fnum, (unsigned)chip.c_block);
        printf(",\"c_adrr\":"); print_u8_array(chip.c_adrr, 3);
        printf(",\"c_dc\":%u,\"c_dm\":%u,\"c_ksl_freq\":%u,\"c_ksl_block\":%u",
               (unsigned)chip.c_dc, (unsigned)chip.c_dm,
               (unsigned)chip.c_ksl_freq, (unsigned)chip.c_ksl_block);
        printf(",\"c_et\":%u,\"c_sl\":%u,\"c_ksr_freq\":%u",
               (unsigned)chip.c_et, (unsigned)chip.c_sl, (unsigned)chip.c_ksr_freq);
        printf(",\"eg_counter_state\":%u,\"eg_counter_state_prev\":%u"
               ",\"eg_timer\":%" PRIu32 ",\"eg_timer_carry\":%u"
               ",\"eg_timer_low_lock\":%u,\"eg_timer_shift\":%u"
               ",\"eg_timer_shift_lock\":%u,\"eg_timer_shift_stop\":%u",
               (unsigned)chip.eg_counter_state, (unsigned)chip.eg_counter_state_prev,
               chip.eg_timer, (unsigned)chip.eg_timer_carry,
               (unsigned)chip.eg_timer_low_lock, (unsigned)chip.eg_timer_shift,
               (unsigned)chip.eg_timer_shift_lock, (unsigned)chip.eg_timer_shift_stop);
        printf(",\"eg_kon\":%u,\"eg_dokon\":%" PRIu32 ",\"eg_off\":%u"
               ",\"eg_rate\":%u,\"eg_rate_hi\":%u,\"eg_maxrate\":%u"
               ",\"eg_zerorate\":%u,\"eg_inc_lo\":%u,\"eg_inc_hi\":%u"
               ",\"eg_sl\":%u,\"eg_silent\":%u",
               (unsigned)chip.eg_kon, chip.eg_dokon, (unsigned)chip.eg_off,
               (unsigned)chip.eg_rate, (unsigned)chip.eg_rate_hi,
               (unsigned)chip.eg_maxrate, (unsigned)chip.eg_zerorate,
               (unsigned)chip.eg_inc_lo, (unsigned)chip.eg_inc_hi,
               (unsigned)chip.eg_sl, (unsigned)chip.eg_silent);
        printf(",\"eg_state\":"); print_u8_array(chip.eg_state, 18);
        printf(",\"eg_level\":"); print_u8_array(chip.eg_level, 18);
        printf(",\"eg_out\":%u,\"eg_ksltl\":%u",
               (unsigned)chip.eg_out, (unsigned)chip.eg_ksltl);
        printf(",\"lfo_counter\":%u,\"lfo_vib_counter\":%u,\"lfo_am_counter\":%u,\"lfo_am_out\":%u",
               (unsigned)chip.lfo_counter, (unsigned)chip.lfo_vib_counter,
               (unsigned)chip.lfo_am_counter, (unsigned)chip.lfo_am_out);
        printf(",\"am_calc_lfo_counter\":%u,\"am_calc_testmode\":%u,\"lfo_am_dir\":%u,\"lfo_am_car\":%u,\"lfo_am_step\":%u",
               (unsigned)am_calc_lfo_counter, (unsigned)am_calc_testmode,
               (unsigned)chip.lfo_am_dir, (unsigned)chip.lfo_am_car,
               (unsigned)chip.lfo_am_step);
        printf(",\"op_fb1\":"); print_i16_array(chip.op_fb1, 9);
        printf(",\"op_fb2\":"); print_i16_array(chip.op_fb2, 9);
        printf(",\"op_fbsum\":%d,\"op_mod\":%d,\"op_neg\":%u,\"op_logsin\":%u,\"op_exp_m\":%u,\"op_exp_s\":%u",
               (int)chip.op_fbsum, (int)chip.op_mod, (unsigned)chip.op_neg,
               (unsigned)chip.op_logsin, (unsigned)chip.op_exp_m, (unsigned)chip.op_exp_s);
#ifdef YM2413_TRACE_OPERATOR_BOUNDARY
        printf(",\"op_phase_index\":%u,\"op_output\":%d",
               (unsigned)opll_trace_operator_phase_index,
               (int)opll_trace_operator_output);
        printf(",\"op_calc_rm_enable\":%u",
               (unsigned)opll_trace_rm_enable_before_operator);
#endif
        printf(",\"ch_out\":%d,\"ch_out_hh\":%d,\"ch_out_tm\":%d,\"ch_out_bd\":%d,\"ch_out_sd\":%d,\"ch_out_tc\":%d",
               (int)chip.ch_out, (int)chip.ch_out_hh, (int)chip.ch_out_tm,
               (int)chip.ch_out_bd, (int)chip.ch_out_sd, (int)chip.ch_out_tc);
        printf(",\"fnum\":"); print_u16_array(chip.fnum, 9);
        printf(",\"block\":"); print_u8_array(chip.block, 9);
        printf(",\"kon\":"); print_u8_array(chip.kon, 9);
        printf(",\"son\":"); print_u8_array(chip.son, 9);
        printf(",\"rhythm\":%u,\"rm_enable\":%u,\"rm_select\":%u,\"rm_noise\":%" PRIu32,
               (unsigned)chip.rhythm, (unsigned)(uint8_t)chip.rm_enable,
               (unsigned)chip.rm_select, chip.rm_noise);
        printf(",\"output_pins_m\":%d,\"output_pins_r\":%d}\n",
               (int)chip.output_m, (int)chip.output_r);
    }

    if (event_index != event_count) {
        fprintf(stderr, "one or more events occur after the requested trace window\n");
        free(events);
        return 4;
    }
    free(events);
    return 0;
}
