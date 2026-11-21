/*
 * Ares SN76489 external state trace adapter.
 *
 * This is project-authored glue around the exact, externally checked-out Ares
 * component. The compatibility header supplies only the small Ares type
 * vocabulary needed to compile that component; it does not contain chip logic.
 * No Ares source is copied into this repository or linked into the Rust core.
 */
#include <cstdint>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include <ares/ares.hpp>
#include "sn76489.hpp"

#define REFERENCE_COMMIT "8bc30c8c3d3c8081d2a66587add602df771441dd"

struct InputEvent {
  std::uint64_t tick;
  std::uint8_t value;
};

struct TraceAres : ares::SN76489 {
  auto state() const -> void {
    std::cout
      << "\"tone_pitch\":[" << static_cast<std::uint32_t>(tone0.pitch) << ","
      << static_cast<std::uint32_t>(tone1.pitch) << ","
      << static_cast<std::uint32_t>(tone2.pitch) << "],"
      << "\"tone_counter\":[" << static_cast<std::uint32_t>(tone0.counter) << ","
      << static_cast<std::uint32_t>(tone1.counter) << ","
      << static_cast<std::uint32_t>(tone2.counter) << "],"
      << "\"tone_output\":[" << static_cast<std::uint32_t>(tone0.output) << ","
      << static_cast<std::uint32_t>(tone1.output) << ","
      << static_cast<std::uint32_t>(tone2.output) << "],"
      << "\"tone_volume\":[" << static_cast<std::uint32_t>(tone0.volume) << ","
      << static_cast<std::uint32_t>(tone1.volume) << ","
      << static_cast<std::uint32_t>(tone2.volume) << "],"
      << "\"noise_counter\":" << static_cast<std::uint32_t>(noise.counter) << ","
      << "\"noise_pitch\":" << static_cast<std::uint32_t>(noise.pitch) << ","
      << "\"noise_volume\":" << static_cast<std::uint32_t>(noise.volume) << ","
      << "\"noise_enable\":" << static_cast<std::uint32_t>(noise.enable) << ","
      << "\"noise_rate\":" << static_cast<std::uint32_t>(noise.rate) << ","
      << "\"lfsr\":" << static_cast<std::uint32_t>(noise.lfsr) << ","
      << "\"noise_flip\":" << static_cast<std::uint32_t>(noise.flip) << ","
      << "\"noise_output\":" << static_cast<std::uint32_t>(noise.output) << ","
      << "\"latch_type\":" << static_cast<std::uint32_t>(latch.type) << ","
      << "\"latch_channel\":" << static_cast<std::uint32_t>(latch.channel);
  }
};

static bool load_events(const char* path, std::vector<InputEvent>& events) {
  std::ifstream input(path);
  std::string line;
  if (!input) {
    std::cerr << "cannot open event file: " << path << "\n";
    return false;
  }
  while (std::getline(input, line)) {
    const char* cursor = line.c_str();
    while (*cursor != '\0' && std::isspace(static_cast<unsigned char>(*cursor))) ++cursor;
    if (*cursor == '\0' || *cursor == '#') continue;
    unsigned long long tick = 0;
    int parsed_value = 0;
    if (std::sscanf(cursor, "%llu,%i", &tick, &parsed_value) != 2 ||
        parsed_value < 0 || parsed_value > 255) {
      std::cerr << "invalid event row: " << line << "\n";
      return false;
    }
    if (!events.empty() && tick < events.back().tick) {
      std::cerr << "event ticks must be nondecreasing\n";
      return false;
    }
    events.push_back({static_cast<std::uint64_t>(tick), static_cast<std::uint8_t>(parsed_value)});
  }
  return true;
}

int main(int argc, char** argv) {
  std::uint64_t calls = 0;
  const char* events_path = nullptr;
  for (int i = 1; i < argc; ++i) {
    if (!std::strcmp(argv[i], "--calls") && i + 1 < argc) {
      calls = std::strtoull(argv[++i], nullptr, 10);
    } else if (!std::strcmp(argv[i], "--events") && i + 1 < argc) {
      events_path = argv[++i];
    } else {
      std::cerr << "usage: " << argv[0] << " --calls N [--events file.csv]\n";
      return 2;
    }
  }
  if (calls == 0) {
    std::cerr << "--calls must be positive\n";
    return 2;
  }

  std::vector<InputEvent> events;
  if (events_path && !load_events(events_path, events)) return 2;

  TraceAres psg;
  psg.power();

  std::cout
    << "{\"schema\":\"ares-sn76489-state-trace-v1\","
    << "\"source_repo\":\"ares-emulator/ares\","
    << "\"source_commit\":\"" << REFERENCE_COMMIT << "\","
    << "\"engine\":\"ares::SN76489\","
    << "\"license\":\"Ares project license (ISC-style)\","
    << "\"time_unit\":\"sn76489_component_clock\","
    << "\"clock_divider_context\":\"one call per Ares component clock\"}\n";

  std::size_t event_index = 0;
  for (std::uint64_t tick = 0; tick < calls; ++tick) {
    std::uint32_t write_count = 0;
    while (event_index < events.size() && events[event_index].tick == tick) {
      psg.write(events[event_index].value);
      ++event_index;
      ++write_count;
    }
    if (event_index < events.size() && events[event_index].tick < tick) {
      std::cerr << "event tick was skipped\n";
      return 3;
    }

    std::cout << "{\"tick\":" << tick << ",\"write_count\":" << write_count << ",";
    psg.state();
    const auto output = psg.clock();
    std::cout << ",\"output\":[" << static_cast<std::uint32_t>(output[0]) << ","
              << static_cast<std::uint32_t>(output[1]) << ","
              << static_cast<std::uint32_t>(output[2]) << ","
              << static_cast<std::uint32_t>(output[3]) << "],";
    std::cout << "\"post_clock\":{";
    psg.state();
    std::cout << "}}\n";
  }
  if (event_index != events.size()) {
    std::cerr << "one or more events occur after the requested trace window\n";
    return 4;
  }
  return 0;
}
