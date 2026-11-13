# H2 streaming performance

Status: NOT MEASURED — production streaming renderer does not exist yet  
Target: SMS NTSC/PAL, 48 kHz internal PCM  
Reference study: `native/sn76489/src/psg_h2_model.rs`

## 1. Do not interpret offline measurements as real-time results

The existing H2 study allocates full-session event and sample vectors, creates per-render temporary arrays, and performs offline convolution. Its reported historical memory estimate (~6.61 MB per profile) is not a production streaming memory bound. No current CPU-time, p95/p99, worst-block, live memory, event overflow or underrun measurements exist for a production H2 renderer.

The reference kernel's historical group delay is 31.25 output samples (approximately 0.651 ms at 48 kHz). This is not the complete application latency. It does not include safe event lookahead, frame batching, worker scheduling, the CPAL queue or the device buffer.

## 2. Required measurements

Record the exact commit SHA, target triple, Rust toolchain, optimization profile, CPU model, operating system, sample rate, console region, ROM/test trace, block size and run duration for every result.

| Metric | Required method | Acceptance evidence |
|---|---|---|
| Mean processing time | monotonic high-resolution clock around renderer block | mean and sample count |
| p50 / p95 / p99 / max | fixed-capacity rolling histogram or preallocated sample window | percentile values and sample count |
| Real-time factor | render duration divided by audio duration represented by the block | max < 1.0 with margin; target p99 < 0.5 |
| Events per block | count accepted events per frame/block | min/mean/p95/p99/max |
| Samples produced | count PCM samples per block | exact expected rational sample count |
| Memory | fixed storage capacity + allocator instrumentation outside callback | fixed bound; no session-length growth |
| Queue pressure | high-water mark and overflow count | zero overflow |
| Audio underruns | existing CPAL/session metrics | zero attributable to H2 |
| Renderer latency | measured algorithmic delay + worker buffering + sink queue | each component and total reported |
| Fallback | inject invalid timestamp/overflow/init error | PSG restored without CPAL restart |

Do not allocate a vector of per-block timing samples in the render path. Instrumentation must use preallocated storage or aggregated counters.

## 3. Test workload

At minimum:
- silence;
- three simultaneous high-frequency tone channels;
- minimum valid tone periods and period changes;
- maximum period;
- frequent volume writes;
- periodic and white noise for SMS profile;
- sustained mixed-channel playback;
- PAL and NTSC frame durations;
- 44.1 kHz and 48 kHz host devices where supported;
- one-hour soak test after short correctness gates pass.

Benchmark the H2 renderer and existing PSG renderer with identical digital event traces. Keep test-harness overhead outside the timed region. Report debug and release builds separately; only release results determine the real-time budget.

## 4. Real-time budget

For a 48 kHz PCM stream, one 60 Hz NTSC frame represents approximately 800 samples and one 50 Hz PAL frame approximately 960 samples. The exact count must be computed from integer clock/sample phase rather than rounded per frame.

The renderer must finish each block before its deadline. Averages alone are insufficient. Require p99 processing time below half the block's audio duration and maximum below the full block duration across a sustained run, with zero overflow and zero H2-attributable underruns.

## 5. Current result

No production renderer has been benchmarked. No CPU, memory, real-time factor, percentile, overflow or underrun result is claimed. This document is a measurement contract, not a PASS report.

## 6. Result table (populate only from executed benchmarks)

| Commit | Platform / CPU | Profile | Block | Mean | p95 | p99 | Max | RTF p99 | Memory bound | Overflows | Underruns |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Not measured | — | — | — | — | — | — | — | — | — | — | — |


## 7. Implementation footprint (not a benchmark)

The standalone kernel currently retains a 65,537-entry f64 step-response table (approximately 512 KiB) and two fixed 128-entry f64 accumulation rings (2 KiB total), excluding struct metadata and allocator overhead. Kernel construction temporarily allocates the oversampled impulse-response table as well. These are code-derived capacities, not measured process RSS or a complete runtime memory figure. Event queue capacity is selected by the runtime caller and has not yet been integrated or measured.
