//! Temporary integration checks against the maintainer-provided homebrew SMS corpus.
//!
//! These are deterministic smoke/integration checks, not proof of visual
//! equivalence to hardware or cycle-exact timing.

use sega_sms_core::{SmsMachine, SmsMachineConfig};

const SOUND_TEST_ROM: &[u8] =
    include_bytes!("../../../local-assets/sms_test_roms/sms-sound-test.sms");
const VDP_TEST_ROM: &[u8] = include_bytes!("../../../local-assets/sms_test_roms/VDPTEST.sms");

#[test]
fn homebrew_sound_rom_samples_are_deterministic_for_a_bounded_window() {
    let mut first =
        SmsMachine::with_audio(SOUND_TEST_ROM.to_vec(), SmsMachineConfig::default(), 44_100)
            .expect("load SMS sound test ROM");
    let mut second = first.clone();

    for _ in 0..30_000 {
        first.step().expect("execute sound test ROM in SMS core");
        second
            .step()
            .expect("execute deterministic copy of sound test ROM");
    }

    assert_eq!(first.psg(), second.psg());
    let first_samples = first.drain_audio_samples(None);
    let second_samples = second.drain_audio_samples(None);
    assert_eq!(first_samples, second_samples);
    assert!(
        !first_samples.is_empty(),
        "PSG should produce samples after advancing"
    );
}

#[test]
fn homebrew_vdp_rom_advances_raster_and_is_deterministic() {
    let mut first = SmsMachine::new(VDP_TEST_ROM.to_vec()).expect("load VDP test ROM");
    let mut second = first.clone();

    for _ in 0..30_000 {
        first.step().expect("execute VDP test ROM");
        second
            .step()
            .expect("execute deterministic copy of VDP test ROM");
    }

    assert!(first.vdp().frame_number() > 0, "VDP raster should advance");
    assert_eq!(first.vdp().frame_number(), second.vdp().frame_number());
    assert_eq!(first.vdp().registers(), second.vdp().registers());
    assert_eq!(first.vdp().vram(), second.vdp().vram());
    assert_eq!(first.vdp().framebuffer(), second.vdp().framebuffer());
}

#[test]
fn homebrew_cpu_exerciser_starts_and_remains_deterministic_for_a_bounded_window() {
    const ZEXDOC_ROM: &[u8] = include_bytes!("../../../local-assets/sms_test_roms/zexdoc.sms");
    let mut first = SmsMachine::new(ZEXDOC_ROM.to_vec()).expect("load ZEXDOC");
    let mut second = first.clone();

    for _ in 0..5_000 {
        first.step().expect("execute bounded ZEXDOC window");
        second.step().expect("execute deterministic ZEXDOC copy");
    }

    assert_eq!(first.cpu(), second.cpu());
    assert_eq!(first.memory(), second.memory());
}
