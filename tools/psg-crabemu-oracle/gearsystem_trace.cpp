/*
 * Gearsystem SMS APU external state-trace adapter.
 *
 * This is project-authored glue. It compiles against the external Gearsystem
 * Sms_Apu and Blip_Buffer modules at a pinned commit. No Gearsystem source is
 * copied into this repository or linked into the Rust backend.
 */
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include "Sms_Apu.h"
#include "Blip_Buffer.h"

#define REFERENCE_COMMIT "1be9a12b82cae84ddd6f9b7006d546abe2d58bd4"

struct InputEvent {
    uint64_t tick;
    int value;
};

static bool load_events(const char *path, std::vector<InputEvent> &events) {
    std::ifstream input(path);
    std::string line;

    if (!input) {
        std::cerr << "cannot open event file: " << path << "\n";
        return false;
    }

    while (std::getline(input, line)) {
        const char *cursor = line.c_str();
        while (*cursor == ' ' || *cursor == '\t') ++cursor;
        if (*cursor == '\0' || *cursor == '#') continue;

        unsigned long long tick = 0;
        int value = 0;
        if (std::sscanf(cursor, "%llu,%i", &tick, &value) != 2 ||
            value < 0 || value > 255) {
            std::cerr << "invalid event row: " << line << "\n";
            return false;
        }
        if (!events.empty() && tick < events.back().tick) {
            std::cerr << "event ticks must be nondecreasing\n";
            return false;
        }
        events.push_back({static_cast<uint64_t>(tick), value});
    }

    return true;
}

int main(int argc, char **argv) {
    uint64_t calls = 0;
    long clock_hz = 3579545;
    long sample_rate = 48000;
    const char *events_path = nullptr;

    for (int i = 1; i < argc; ++i) {
        if (!std::strcmp(argv[i], "--calls") && i + 1 < argc) {
            calls = std::strtoull(argv[++i], nullptr, 10);
        } else if (!std::strcmp(argv[i], "--clock-hz") && i + 1 < argc) {
            clock_hz = std::strtol(argv[++i], nullptr, 10);
        } else if (!std::strcmp(argv[i], "--sample-rate") && i + 1 < argc) {
            sample_rate = std::strtol(argv[++i], nullptr, 10);
        } else if (!std::strcmp(argv[i], "--events") && i + 1 < argc) {
            events_path = argv[++i];
        } else {
            std::cerr << "usage: " << argv[0]
                      << " --calls N [--events file.csv] [--clock-hz HZ] [--sample-rate HZ]\n";
            return 2;
        }
    }

    if (calls == 0 || clock_hz <= 0 || sample_rate <= 0) {
        std::cerr << "--calls, --clock-hz and --sample-rate must be positive\n";
        return 2;
    }

    std::vector<InputEvent> events;
    if (events_path && !load_events(events_path, events)) return 2;

    Blip_Buffer buffer;
    if (const char *error = buffer.set_sample_rate(sample_rate)) {
        std::cerr << "Blip_Buffer set_sample_rate failed: " << error << "\n";
        return 3;
    }
    buffer.clock_rate(clock_hz);

    Sms_Apu apu;
    apu.reset(false);  // SMS profile, not the TI SN76489 profile.
    apu.output(&buffer);

    std::cout
        << "{\"schema\":\"gearsystem-sms-psg-state-trace-v1\","
        << "\"source_repo\":\"drhelius/Gearsystem\","
        << "\"source_commit\":\"" << REFERENCE_COMMIT << "\","
        << "\"engine\":\"Sms_Apu\","
        << "\"license\":\"GPL-3.0 project; LGPL-2.1 audio modules\","
        << "\"time_unit\":\"z80_t_state\","
        << "\"row_duration_tstates\":16,"
        << "\"clock_hz\":" << clock_hz << ","
        << "\"sample_rate_hz\":" << sample_rate << "}\n";

    size_t event_index = 0;
    for (uint64_t tick = 0; tick < calls; ++tick) {
        uint32_t write_count = 0;
        while (event_index < events.size() && events[event_index].tick == tick) {
            apu.write_data(0, events[event_index].value);
            ++event_index;
            ++write_count;
        }
        if (event_index < events.size() && events[event_index].tick < tick) {
            std::cerr << "event tick was skipped\n";
            return 4;
        }

        const Sms_Apu_State before = apu.GetState();
        apu.end_frame(16);
        buffer.end_frame(16);
        const Sms_Apu_State after = apu.GetState();

        std::cout
            << "{\"tick\":" << tick
            << ",\"write_count\":" << write_count
            << ",\"latch\":" << after.latch
            << ",\"noise_shifter_before\":" << before.noise_shifter
            << ",\"noise_shifter_after\":" << after.noise_shifter
            << ",\"noise_bit_before\":" << (before.noise_shifter & 1)
            << ",\"noise_bit_after\":" << (after.noise_shifter & 1)
            << ",\"noise_feedback\":" << after.noise_feedback
            << ",\"noise_white\":" << (after.noise_white ? "true" : "false")
            << ",\"noise_rate\":" << after.noise_rate
            << ",\"noise_period\":" << after.channels[3].period
            << ",\"tone2_period\":" << after.channels[2].period
            << ",\"tone2_phase\":" << after.channels[2].phase
            << ",\"channel_periods_after\":[" << after.channels[0].period << ","
            << after.channels[1].period << "," << after.channels[2].period << ","
            << after.channels[3].period << "]"
            << ",\"channel_phases_after\":[" << after.channels[0].phase << ","
            << after.channels[1].phase << "," << after.channels[2].phase << ","
            << after.channels[3].phase << "]"
            << ",\"channel_volumes_after\":[" << after.channels[0].volume << ","
            << after.channels[1].volume << "," << after.channels[2].volume << ","
            << after.channels[3].volume << "]"
            << ",\"ti_chip\":" << (after.ti_chip ? "true" : "false")
            << ",\"buffer_samples_available\":" << buffer.samples_avail()
            << "}\n";
    }

    if (event_index != events.size()) {
        std::cerr << "one or more events occur after the requested trace window\n";
        return 5;
    }

    return 0;
}
