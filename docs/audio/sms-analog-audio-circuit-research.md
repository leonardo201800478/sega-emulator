# SMS analog audio circuit research

Status: research baseline, not a calibrated electrical reconstruction
Date: 2026-11-12
Scope: Master System PSG output path, with explicit separation between documented facts and unresolved schematic-level details.

## Executive findings

1. Sega's original Master System (Power Base) service procedure identifies audio at IC5 pin 10, specifies a maximum of 300 mV peak-to-peak and a nominal passband of 20 Hz–20 kHz, and directs technicians to check CON1, the PCB path, Q48/C49/R47, R46, IC5 and the RF modulator when picture is present without sound.
2. The Master System II service manual instead identifies IC5 pin 15 as the audio test point and repeats the 300 mVp-p / 20 Hz–20 kHz figures. Its OCR fault list names Q48/Q49/R47/R46, but secondary comparison of the PAL and RGB parts lists warns that some of those generic troubleshooting designators do not appear in the listed SMS II board parts. Treat those designators as revision-dependent or OCR/manual carry-over until each board schematic is inspected.
3. The PAL Model 1 service schematic is identified as IC Board PB PAL VA1, assembly drawing 171-5534. The SMS II manual contains separate PAL and RGB schematics and parts/location drawings. Neither one universal board circuit nor one set of component designators can be assumed for all regions/revisions.
4. The service figures are output acceptance/test specifications, not measured pole frequencies or a complete transfer function. They do not identify the output impedance, load used for the measurement, coupling capacitor values, resistor values, amplifier gain, or the full RF audio route.
5. The optional 20 Hz HPF / 18 kHz LPF preset introduced in the application is therefore a deliberately broad experimental approximation. It is not a circuit-derived SMS1 transfer function and must not be represented as such.

## Signal-path diagram and evidence status

```text
SN76489-family PSG logic inside / associated with the Sega VDP (IC5)
    |  tone 1..3 + noise; programmable attenuation; digital state
    v
VDP PSG output stage / analog output pin
    |  Model 1 service test point: IC5 pin 10
    |  Master System II service test point: IC5 pin 15
    v
Board-level analog components and PCB route
    |  service fault lists mention Q48, C49, R47, R46 (Model 1)
    |  SMS II OCR fault list mentions Q48, Q49, R47, R46
    |  exact topology / values must be matched to board revision
    v
A/V connector CON1 audio (service manual: pins 1 and 2)
    |
    +---- direct wired A/V output / external load (exact load not specified)
    |
    +---- RF modulator audio route (manual lists modulator as a possible
          no-sound fault; exact coupling point and transfer path require
          schematic-level tracing)
```

This is an evidence map, not a claim that every revision has identical wiring. The VDP/PSG output is analog at the service test point; the available service prose does not establish a separate external DAC or a particular active op-amp topology on every console revision.

## Digital sound source

The SMS uses an SN76489-family PSG function associated with the VDP. The digital model has three tone channels and one noise channel, with programmable attenuation. The TI SN76489AN data sheet describes the standalone device's tone/noise generators, attenuation and an internal output buffer/summing amplifier capable of up to 10 mA. That data sheet is useful for the generic SN76489 family architecture, but it is not proof that the Sega VDP ASIC's output pin has the identical internal circuit, output impedance or load capability.

For the emulator, the SN76489 digital event stream remains the hardware-domain source. H2 reconstructs that stream; it does not model the analog output pin or board circuitry.

## Board-level stages and component evidence

| Stage / reference | What is documented | What is not established |
|---|---|---|
| VDP IC5 output | Model 1: pin 10; SMS II: pin 15. Service manuals specify up to 300 mVp-p and 20 Hz–20 kHz. | Output impedance, bias, source resistance, load condition, distortion and exact output topology. |
| Q48 / Q49 | Named in one or more service troubleshooting lists as possible no-sound fault locations. | A universal transistor role or identical presence across board variants. |
| C49 | Named as a possible Model 1 no-sound fault location. | Capacitance, dielectric, coupling role and exact position without the matching schematic/parts list. |
| R47 / R46 | Named as possible no-sound fault locations. | Resistance, whether series/shunt/load/feedback, and board-to-board equivalence. |
| CON1 | Model 1 service procedure checks audio between pins 1 and 2; an earphone or small speaker can be used to hear it. | Specified test load, output impedance and connector-level frequency response. |
| RF modulator | Listed as a possible no-sound fault location. | Exact input coupling, gain, filtering and modulation transfer function from the available text alone. |

The component references above are intentionally not assigned numerical values or inferred circuit functions where the accessible service text does not establish them.

## Model 1 / Model 2 / regional differences

### Original Master System / Power Base (Model 1)

- The service manual's audio test point is IC5 pin 10.
- Published service test figures: maximum 300 mVp-p and 20 Hz–20 kHz.
- The no-sound troubleshooting list names Q48, C49, R47 and R46.
- The documented PAL schematic is "IC Board PB PAL VA1", drawing 171-5534. The parts documentation lists regional PAL board assemblies including 837-6097, 837-6118 and 837-6135.
- The PAL parts list discussed by the RetroTechCollection guide does not contain Q48, C49 or R46 in the expected way; the nearby transistor is identified there as Q46 (2SC1740). This is a warning not to copy the generic troubleshooting designators into a universal profile.

### Master System II

- The service manual's audio test point is IC5 pin 15, not pin 10.
- The service figure remains up to 300 mVp-p and 20 Hz–20 kHz.
- The OCR troubleshooting list names Q48, Q49, R47 and R46. A secondary comparison notes that Q48/Q49/R46/R47 do not appear in either of the SMS II parts lists, suggesting carried-over generic fault text or mismatched designators. Confirm against the actual PAL/RGB schematic and board revision before assigning roles.
- The SMS II manual includes separate PAL and RGB schematics and parts/location drawings; they must be treated as separate board evidence.

### Regional variants

- The original manual's PAL board schematic and regional parts lists do not cover every NTSC/US or later revision. A PAL VA1 drawing cannot be generalized to an NTSC board without its matching schematic.
- The service documentation distinguishes regional PAL board assemblies. The Model 1 service guide also notes different RGB encoder parts on some NTSC boards; this is primarily video-path evidence and should not be confused with proof of an audio-stage change.
- Japanese Master System models may include YM2413 FM capability; export models are documented as lacking FM sound. That is an additional sound source and not part of the SN76489 H2 path. Do not silently mix YM2413 into the PSG renderer.

## SG-1000 comparison

The emulator's SG-1000 and SMS profiles share the SN76489-family digital core but intentionally preserve distinct LFSR/noise behavior. The available sources reviewed for this research do not establish that the SG-1000 board-level analog output network is identical to any SMS Model 1 or Model 2 network. Consequently:

- Do not apply an SMS analog profile to SG-1000 by default.
- Keep digital PSG profile selection independent from any analog-output profile.
- Require SG-1000 board-specific schematics and component-level evidence before claiming analog equivalence.

## Frequency response, filtering, coupling and load

The 20 Hz–20 kHz figure is a service-manual test range. It is not a specification of a -3 dB corner, passband ripple, attenuation slope or phase response. The 300 mVp-p figure is a maximum service test level, not a calibrated full-scale conversion factor for emulator PCM.

No reliable source reviewed here provides, for every board variant:

- exact series coupling capacitor and resistor values for the audio path;
- complete RC network topology and component tolerances;
- amplifier device, bias, gain and clipping headroom;
- source/output impedance and specified load;
- measured frequency/phase response at both VDP pin and A/V connector;
- crosstalk, noise floor, THD+N or load-dependent response.

Therefore no component-derived cutoff frequency or impedance is asserted in this document.

## Implications for the emulator DSP

1. Keep the PSG renderer and board-level analog output processing as separate layers.
2. H2 is an event-to-PCM reconstruction renderer. It must not be implemented as an EQ/filter over the current PSG PCM output.
3. The optional analog DSP remains post-mixer and host-side. It must not alter SN76489 register semantics, PSG clocks, MachineScheduler time, WAIT states or machine execution.
4. Preserve the digital path as the default and exact bypass when the analog stage is disabled.
5. Do not make the 20 Hz / 18 kHz experimental preset a default or claim it reproduces a physical SMS board.
6. A future hardware profile should be tied to a verified board identifier/revision and source schematic. It should carry only measured/documented parameters and explicit confidence.
7. If electrical captures become available, measure at both IC5 audio output and CON1 under a declared load; record amplitude, frequency sweep, phase, impedance/load and board revision. Use those measurements to fit the DSP, not generic service troubleshooting text.

## Source register

### Primary / near-primary documentation

- Sega Master System Service Manual (Power Base), reproduced/transcribed by SMS Power!: https://www.smspower.org/Development/SegaMasterSystemServiceManual
  - Service audio check: IC5 pin 10, up to 300 mVp-p, 20 Hz–20 kHz; CON1 pins 1–2; no-sound fault list.
  - Schematics heading identifies "IC BD PB PAL VA1 171-5534" and PC MAIN M4.
- Sega Master System II Service Manual, reproduced/transcribed by SMS Power!: https://www.smspower.org/Development/SegaMasterSystemIIServiceManual
  - Service audio check: IC5 pin 15, up to 300 mVp-p, 20 Hz–20 kHz; SMS II PAL/RGB schematics and troubleshooting.
- Texas Instruments, SN76489AN data sheet: https://map.grauw.nl/resources/sound/texas_instruments_sn76489an.pdf
  - Standalone chip architecture: three tone generators, noise generator, attenuation and internal output buffer/summing amplifier. This is not a substitute for Sega VDP ASIC characterization.

### Secondary cross-checks

- RetroTechCollection, Sega Master System Troubleshooting Guide: https://wiki.retrotechcollection.com/Sega_Master_System_Troubleshooting_Guide
  - Summarizes the PAL 171-5534 schematic/parts list, audio service checks, Model 1 component-reference discrepancy and regional caveats.
- RetroTechWiki, Sega Master System II Troubleshooting Guide: https://retrotechwiki.com/Sega_Master_System_II_Troubleshooting_Guide
  - Cross-checks the SMS II pin 15 test point and discrepancy between generic troubleshooting text and parts lists.
- RetroTechWiki, Sega Master System II Service Manual (1991): https://retrotechwiki.com/Sega_Master_System_II_Service_Manual_%281991%29
  - Notes multiple regional boards and the limits of carried-over fault-list component designators.

### Project sources

- `docs/audio/slint-audio-controls.md`: documents the existing optional post-mixer DSP and explicitly labels 20 Hz / 18 kHz as experimental values rather than component-derived cutoffs.
- `docs/audio/modern-hifi-h2-continuity-checkpoint.md`: records the H2 offline study's kernel parameters, latency and limitations.
- `native/sn76489/src/psg_h2_model.rs`: original test/study implementation; not a production streaming renderer.

## Open research items

- Inspect original PDF schematic pages at high resolution and trace the audio net from IC5 pin 10 / pin 15 through every board component to CON1 and RF modulator for each supported board.
- Transcribe exact values and references from Model 1 PAL VA1, Model 1 NTSC and SMS II PAL/RGB parts lists.
- Resolve OCR/designator conflicts by checking schematic symbols and PCB silkscreen, not by assuming the troubleshooting table is authoritative for every revision.
- Obtain physical captures from at least one identified Model 1 board and one SMS II board under a specified load.
- Obtain SG-1000 board-specific audio schematic/measurement before proposing any analog profile.

Until those tasks are complete, the correct software model is a neutral digital PSG path plus optional explicitly experimental output DSP, with no asserted board-calibrated analog transfer function.


## Reconciliation with the supplied project audio specification

The supplied file `Especificação Técnica de Áudio do Master System (PSG e DSP em Rust).md` is a useful engineering proposal for the software pipeline, but several numerical statements in it are not corroborated by the service-manual evidence reviewed for this document. In particular, its diagram/code proposes approximately 35 Hz high-pass, 12 kHz low-pass, 1–10 µF coupling capacitance, 1–10 kΩ mixing resistors and a transistor-like `tanh` soft clipper. The same proposal also describes a unipolar summing node and a 111.86 kHz carrier for period 0/1.

Those values and circuit descriptions must remain **design hypotheses**, not board facts, until the corresponding board schematic and physical measurements support them. The service-manual check figures of 20 Hz–20 kHz and 300 mVp-p do not prove a 35 Hz or 12 kHz -3 dB corner, nor do they establish the cited capacitor/resistor ranges, clipping transfer curve or output impedance. The VDP ASIC output must also not be assumed identical to the standalone TI SN76489AN analog output buffer.

For this mission, the codebase's existing 20 Hz HPF / 18 kHz LPF preset remains an optional experimental DSP profile only. Do not replace it with the proposal's 35 Hz / 12 kHz values, add a `tanh` stage, or claim analog accuracy based solely on the supplied proposal. The H2 reconstruction renderer remains logically upstream and independent of any of these output-stage hypotheses.
