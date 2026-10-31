//! Headless Rust-only integration tests for the SG-1000/SMS PSG audio path.

use std::sync::{Arc, Mutex};

use sega_audio_runtime::{NativeAudioSession, PcmSink};
use sega_sg1000_core::{MemoryConfig, Sg1000Machine};
use sega_sms_core::{SmsMachine, SmsMachineConfig};

#[derive(Clone, Default)]
struct RecordingSink(Arc<Mutex<Vec<Vec<u8>>>>);

impl PcmSink for RecordingSink {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        self.0.lock().expect("recording sink lock").push(pcm);
        Ok(())
    }

    fn metrics(&self) -> (u64, u64) {
        (0, 0)
    }

    fn close(&mut self) {}
}

const PSG_ROM: [u8; 15] = [
    0x3E, 0x90, 0xD3, 0x7F, 0x3E, 0x85, 0xD3, 0x7F, 0x3E, 0x12, 0xD3, 0x7F, 0xC3, 0x00, 0x00,
];

fn render(samples: &[f64]) -> Vec<Vec<u8>> {
    let sink = RecordingSink::default();
    let capture = sink.clone();
    let mut session = NativeAudioSession::new(48_000, 100, sink).expect("audio session");
    session
        .process_samples(samples)
        .expect("process native samples");
    session.close();
    capture.0.lock().expect("recording sink lock").clone()
}

#[test]
fn sg1000_psg_audio_pipeline_is_deterministic_without_python_or_host_audio() {
    let mut left = Sg1000Machine::new(&PSG_ROM, MemoryConfig::default()).unwrap();
    let mut right = Sg1000Machine::new(&PSG_ROM, MemoryConfig::default()).unwrap();
    for _ in 0..10_000 {
        left.step().unwrap();
        right.step().unwrap();
    }

    let left_tstates = left.cpu.total_tstates;
    let right_tstates = right.cpu.total_tstates;
    let left_samples = left.drain_audio_samples(None);
    let right_samples = right.drain_audio_samples(None);
    assert!(!left_samples.is_empty());
    assert!(
        left_samples
            .iter()
            .any(|sample| sample.abs() > f64::EPSILON)
    );
    assert_eq!(left_samples, right_samples);

    let left_pcm = render(&left_samples);
    let right_pcm = render(&right_samples);
    assert!(!left_pcm.is_empty());
    assert!(left_pcm.iter().flatten().any(|byte| *byte != 0));
    assert_eq!(left_pcm, right_pcm);
    assert_eq!(left.cpu.total_tstates, left_tstates);
    assert_eq!(right.cpu.total_tstates, right_tstates);
}

#[test]
fn sms_psg_audio_pipeline_is_deterministic_without_python_or_host_audio() {
    let mut left =
        SmsMachine::with_audio(PSG_ROM.to_vec(), SmsMachineConfig::default(), 44_100).unwrap();
    let mut right =
        SmsMachine::with_audio(PSG_ROM.to_vec(), SmsMachineConfig::default(), 44_100).unwrap();
    for _ in 0..10_000 {
        left.step().unwrap();
        right.step().unwrap();
    }

    let left_tstates = left.cpu().total_tstates;
    let right_tstates = right.cpu().total_tstates;
    let left_samples = left.drain_audio_samples(None);
    let right_samples = right.drain_audio_samples(None);
    assert!(!left_samples.is_empty());
    assert_eq!(left_samples, right_samples);

    let left_pcm = render(&left_samples);
    let right_pcm = render(&right_samples);
    assert!(!left_pcm.is_empty());
    assert_eq!(left_pcm, right_pcm);
    assert_eq!(left.cpu().total_tstates, left_tstates);
    assert_eq!(right.cpu().total_tstates, right_tstates);
}
