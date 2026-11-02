//! Native WASAPI/XAudio2 adapters for the Rust-owned audio session.

use crate::PcmSink;
use sega_audio_output::{AudioOutputError, NativeWasapiOutput, NativeXAudio2Output};

/// A selected native Windows PCM endpoint. No Python callback is involved.
pub enum NativeAudioOutput {
    Wasapi(NativeWasapiOutput),
    XAudio2(NativeXAudio2Output),
}

impl NativeAudioOutput {
    /// Open the Windows WASAPI shared/event-driven endpoint.
    pub fn open_wasapi(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        balance: i8,
    ) -> Result<Self, AudioOutputError> {
        NativeWasapiOutput::new_with_balance(sample_rate, buffer_size, queue_blocks, balance)
            .map(Self::Wasapi)
    }

    /// Open the Windows XAudio2 endpoint.
    pub fn open_xaudio2(
        sample_rate: u32,
        buffer_size: u32,
        queue_blocks: usize,
        balance: i8,
    ) -> Result<Self, AudioOutputError> {
        NativeXAudio2Output::new_with_balance(sample_rate, buffer_size, queue_blocks, balance)
            .map(Self::XAudio2)
    }
}

impl PcmSink for NativeAudioOutput {
    fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
        match self {
            Self::Wasapi(output) => output.write_pcm(pcm),
            Self::XAudio2(output) => output.write_pcm(pcm),
        }
        .map_err(|error| error.to_string())
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
