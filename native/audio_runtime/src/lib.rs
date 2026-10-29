//! Native PCM conversion shared by host audio backends.
use std::f64::consts::PI;

const INPUT_RATE: u32 = 44_100;
const TAPS: usize = 63;
const HALF: isize = 31;
const PHASES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError { InvalidSampleRate, InvalidVolume }

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSampleRate => write!(f, "output sample rate must be positive"),
            Self::InvalidVolume => write!(f, "volume must be in 0..=100"),
        }
    }
}
impl std::error::Error for AudioError {}

#[derive(Debug, Clone)]
pub struct PcmResampler {
    rate: u32, step: f64, volume: u8, coefficients: Vec<[f64; TAPS]>,
    buffer: Vec<f64>, start: usize, available: usize, real: usize,
    next: f64, emitted: usize,
}

impl PcmResampler {
    pub fn new(rate: u32, volume: u8) -> Result<Self, AudioError> {
        if rate == 0 { return Err(AudioError::InvalidSampleRate); }
        if volume > 100 { return Err(AudioError::InvalidVolume); }
        let cutoff = (rate as f64 / INPUT_RATE as f64).min(1.0) * 0.94;
        let mut coefficients = Vec::with_capacity(PHASES);
        for phase in 0..PHASES {
            let frac = phase as f64 / PHASES as f64;
            let mut weights = [0.0; TAPS];
            let mut sum = 0.0;
            for (i, weight) in weights.iter_mut().enumerate() {
                let x = i as f64 - HALF as f64 - frac;
                let window = 0.42 + 0.5 * (PI*x/(HALF as f64+1.0)).cos()
                    + 0.08 * (2.0*PI*x/(HALF as f64+1.0)).cos();
                let z = cutoff*x;
                let sinc = if z.abs() < 1e-12 { 1.0 } else { (PI*z).sin()/(PI*z) };
                *weight = cutoff*sinc*window;
                sum += *weight;
            }
            for weight in &mut weights { *weight /= sum; }
            coefficients.push(weights);
        }
        Ok(Self { rate, step: INPUT_RATE as f64 / rate as f64, volume, coefficients,
            buffer: Vec::new(), start: 0, available: 0, real: 0, next: -(HALF as f64), emitted: 0 })
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<(), AudioError> {
        if volume > 100 { return Err(AudioError::InvalidVolume); }
        self.volume = volume; Ok(())
    }
    pub fn output_rate(&self) -> u32 { self.rate }
    pub fn volume(&self) -> u8 { self.volume }
    pub fn reset(&mut self) {
        self.buffer.clear(); self.start=0; self.available=0; self.real=0;
        self.next=-(HALF as f64); self.emitted=0;
    }
    pub fn process(&mut self, input: &[f64]) -> Vec<u8> {
        if input.is_empty() { return Vec::new(); }
        if self.rate == INPUT_RATE { return self.encode(input.iter().copied()); }
        self.buffer.extend_from_slice(input); self.available+=input.len(); self.real+=input.len();
        let out=self.emit(false); self.discard_old(); out
    }
    pub fn flush(&mut self) -> Vec<u8> {
        if self.rate == INPUT_RATE || self.real == 0 { self.reset(); return Vec::new(); }
        self.buffer.extend(std::iter::repeat_n(0.0, TAPS/2+1)); self.available+=TAPS/2+1;
        let out=self.emit(true); self.reset(); out
    }
    fn emit(&mut self, flushing: bool) -> Vec<u8> {
        let target=(self.real as f64*self.rate as f64/INPUT_RATE as f64).round() as usize;
        let mut out=Vec::new();
        loop {
            if flushing {
                if self.next>=self.real as f64 || self.emitted>=target { break; }
            } else if self.next+HALF as f64>=self.available as f64 || self.emitted>=target { break; }
            let base=self.next.floor() as isize;
            let phase=(((self.next-base as f64)*PHASES as f64) as usize).min(PHASES-1);
            let mut value=0.0;
            for (i,c) in self.coefficients[phase].iter().enumerate() {
                let idx=base+i as isize-HALF;
                if idx>=self.start as isize && idx<self.available as isize {
                    value+=self.buffer[idx as usize-self.start]*c;
                }
            }
            out.extend_from_slice(&self.pcm16(value).to_le_bytes());
            self.emitted+=1; self.next+=self.step;
        }
        out
    }
    fn encode(&self, values: impl Iterator<Item=f64>) -> Vec<u8> {
        let mut out=Vec::new();
        for value in values { out.extend_from_slice(&self.pcm16(value).to_le_bytes()); }
        out
    }
    fn pcm16(&self, value: f64) -> i16 {
        (value*self.volume as f64/100.0*32767.0).round().clamp(-32768.0,32767.0) as i16
    }
    fn discard_old(&mut self) {
        let keep=(self.next.floor() as isize-HALF).max(0) as usize;
        let count=keep.saturating_sub(self.start).min(self.buffer.len());
        if count>0 { self.buffer.drain(..count); self.start+=count; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm16_clips_and_scales() {
        let mut r=PcmResampler::new(INPUT_RATE,100).unwrap();
        let bytes=r.process(&[-2.0,-1.0,0.0,1.0,2.0]);
        let v:Vec<i16>=bytes.chunks_exact(2).map(|b|i16::from_le_bytes([b[0],b[1]])).collect();
        assert_eq!(v,vec![-32768,-32767,0,32767,32767]);
        r.set_volume(50).unwrap();
        let b=r.process(&[0.5]);
        assert_eq!(i16::from_le_bytes([b[0],b[1]]),16384);
    }
    #[test]
    fn resampling_is_repeatable() {
        let input:Vec<f64>=(0..2048).map(|n|(n as f64*0.03).sin()).collect();
        let mut a=PcmResampler::new(48000,100).unwrap();
        let mut b=PcmResampler::new(48000,100).unwrap();
        let expected=[a.process(&input),a.flush()].concat();
        let mut actual=Vec::new();
        for block in input.chunks(113) { actual.extend(b.process(block)); }
        actual.extend(b.flush());
        assert_eq!(actual,expected);
    }
    #[test]
    fn rejects_bad_configuration() {
        assert!(PcmResampler::new(0,100).is_err());
        assert!(PcmResampler::new(44100,101).is_err());
    }
}
