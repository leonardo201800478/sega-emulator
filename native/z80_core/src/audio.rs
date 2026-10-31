//! Native host-output adapter for the Python-independent Rust audio session.

use sega_audio_output::{
    NativeWasapiOutput as RustWasapiOutput, NativeXAudio2Output as RustXAudio2Output,
};
use sega_audio_runtime::{NativeAudioSession, PcmSink};

pub(crate) enum NativeAudioBackend {
    Wasapi(RustWasapiOutput),
    XAudio2(RustXAudio2Output),
}

pub(crate) type MachineAudioSession = NativeAudioSession<NativeAudioBackend>;

impl NativeAudioBackend {
    fn new(
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        balance: i8,
    ) -> Result<Self, String> {
        match backend {
            "wasapi" => RustWasapiOutput::new_with_balance(
                sample_rate, buffer_size, queue_blocks, balance,
            )
            .map(Self::Wasapi)
            .map_err(|error| error.to_string()),
            "xaudio2" => RustXAudio2Output::new_with_balance(
                sample_rate, buffer_size, queue_blocks, balance,
            )
            .map(Self::XAudio2)
            .map_err(|error| error.to_string()),
            _ => Err(format!("unsupported native audio backend: {backend}")),
        }
    }
}

impl PcmSink for NativeAudioBackend {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Wasapi(output) => output.write_pcm(pcm).map_err(|error| error.to_string()),
            Self::XAudio2(output) => output.write_pcm(pcm).map_err(|error| error.to_string()),
        }
    }

    fn metrics(&self) -> (u64, u64) {
        match self {
            Self::Wasapi(output) => (output.underrun_count(), output.dropped_block_count()),
            Self::XAudio2(output) => (output.underrun_count(), output.dropped_block_count()),
        }
    }

    fn close(&mut self) {
        match self {
            Self::Wasapi(output) => output.close(),
            Self::XAudio2(output) => output.close(),
        }
    }
}

impl MachineAudioSession {
    pub(crate) fn open(
        backend: &str,
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        volume: u8,
        balance: i8,
    ) -> Result<Self, String> {
        let output = NativeAudioBackend::new(
            backend, sample_rate, buffer_size, queue_blocks, balance,
        )?;
        NativeAudioSession::new(sample_rate, volume, output)
    }
}
