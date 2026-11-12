//! Test-only H2 model. Production PSG synthesis is intentionally untouched.
use super::{PsgProfile, Sn76489Core};
use std::{f64::consts::PI, fs, io::Write, path::Path};

const CLOCK: u64 = 3_579_545;
const RATE: u64 = 48_000;
const CLOCKS: u64 = 1_600_000;
const RADIUS: isize = 32;
const PHASES: usize = 256;

#[derive(Clone, Copy, Default)]
struct Stats { mean: f64, rms: f64, peak: f64, max_delta: f64 }

fn name(profile: PsgProfile) -> &'static str {
    match profile { PsgProfile::Sms => "sms", PsgProfile::Sg1000 => "sg1000" }
}

fn setup_tone(psg: &mut Sn76489Core, period: u16) {
    psg.write(0x80 | (period as u8 & 15));
    psg.write((period >> 4) as u8);
    psg.write(0x90);
    psg.write(0xbf);
    psg.write(0xdf);
    psg.write(0xff);
}

fn stats(samples: &[f64]) -> Stats {
    if samples.is_empty() { return Stats::default(); }
    Stats {
        mean: samples.iter().sum::<f64>() / samples.len() as f64,
        rms: (samples.iter().map(|x| x*x).sum::<f64>() / samples.len() as f64).sqrt(),
        peak: samples.iter().map(|x| x.abs()).fold(0.0, f64::max),
        max_delta: samples.windows(2).map(|w| (w[1]-w[0]).abs()).fold(0.0, f64::max),
    }
}

fn amplitude(samples: &[f64], hz: f64) -> f64 {
    let start = 512.min(samples.len()/4);
    let data = &samples[start..];
    if data.len() < 16 || hz <= 0.0 || hz >= RATE as f64/2.0 { return 0.0; }
    let mut re=0.0; let mut im=0.0; let mut norm=0.0;
    for (i,x) in data.iter().enumerate() {
        let w=0.5-0.5*(std::f64::consts::TAU*i as f64/(data.len()-1) as f64).cos();
        let a=std::f64::consts::TAU*hz*i as f64/RATE as f64;
        re += x*w*a.cos(); im -= x*w*a.sin(); norm += w;
    }
    2.0*re.hypot(im)/norm
}

fn thd(samples: &[f64], hz: f64) -> f64 {
    let fundamental=amplitude(samples,hz);
    if fundamental <= f64::EPSILON { return 0.0; }
    let mut energy=0.0;
    for h in [3.0,5.0,7.0,9.0] {
        if hz*h < RATE as f64/2.0 {
            let a=amplitude(samples,hz*h); energy += a*a;
        }
    }
    energy.sqrt()/fundamental
}

fn linear_tone(profile: PsgProfile, period: u16) -> Vec<f64> {
    let mut p=Sn76489Core::new_with_profile(CLOCK,RATE,profile).unwrap();
    setup_tone(&mut p,period);
    p.advance_clocks(CLOCKS);
    p.drain_samples(None)
}

fn kernel() -> Vec<Vec<f64>> {
    (0..PHASES).map(|phase| {
        let frac=phase as f64/PHASES as f64;
        let mut taps=Vec::with_capacity((2*RADIUS+1) as usize);
        let mut sum=0.0;
        for k in -RADIUS..=RADIUS {
            let x=k as f64-frac;
            let z=x/(RADIUS as f64+1.0);
            let win=0.42+0.5*(PI*z).cos()+0.08*(2.0*PI*z).cos();
            let q=0.94*x;
            let sinc=if q.abs()<1.0e-12 {1.0} else {(PI*q).sin()/(PI*q)};
            let v=0.94*sinc*win; taps.push(v); sum+=v;
        }
        for tap in &mut taps { *tap/=sum; }
        taps
    }).collect()
}

fn event_tone(profile: PsgProfile, period: u16, chunk: Option<u64>) -> (Vec<f64>,usize) {
    let mut p=Sn76489Core::new_with_profile(CLOCK,RATE,profile).unwrap();
    setup_tone(&mut p,period);
    let initial=p.mix();
    let mut events=Vec::<(u64,f64)>::new();
    let total_ticks=CLOCKS/16;
    let mut tick=0;
    while tick<total_ticks {
        let n=chunk.map(|v|v.max(1).min(total_ticks-tick)).unwrap_or(total_ticks-tick);
        for _ in 0..n {
            let before=p.mix();
            p.tick_internal_clock();
            let delta=p.mix()-before;
            if delta!=0.0 { events.push(((tick+1)*16,delta)); }
            tick+=1;
        }
    }
    let table=kernel();
    let count=(u128::from(CLOCKS)*u128::from(RATE)/u128::from(CLOCK)) as usize;
    let mut impulses=vec![0.0;count];
    for (clock,delta) in &events {
        let t=u128::from(*clock)*u128::from(RATE);
        let base=(t/u128::from(CLOCK)) as isize;
        let rem=t%u128::from(CLOCK);
        let phase=((rem*PHASES as u128)/u128::from(CLOCK)) as usize;
        for k in -RADIUS..=RADIUS {
            let i=base+k;
            if i>=0 && (i as usize)<count {
                impulses[i as usize]+=delta*table[phase][(k+RADIUS) as usize];
            }
        }
    }
    let mut level=initial;
    let output=impulses.into_iter().map(|v| { level+=v; level }).collect();
    (output,events.len())
}

fn write_wav(path: &Path, samples: &[f64]) -> std::io::Result<()> {
    let bytes=u32::try_from(samples.len().checked_mul(4).ok_or_else(||std::io::Error::other("WAV overflow"))?)
        .map_err(|_|std::io::Error::other("WAV too large"))?;
    let riff=36_u32.checked_add(bytes).ok_or_else(||std::io::Error::other("WAV too large"))?;
    let mut w=std::io::BufWriter::new(fs::File::create(path)?);
    w.write_all(b"RIFF")?; w.write_all(&riff.to_le_bytes())?; w.write_all(b"WAVEfmt ")?;
    w.write_all(&16_u32.to_le_bytes())?; w.write_all(&3_u16.to_le_bytes())?; w.write_all(&1_u16.to_le_bytes())?;
    w.write_all(&(RATE as u32).to_le_bytes())?; w.write_all(&((RATE*4) as u32).to_le_bytes())?;
    w.write_all(&4_u16.to_le_bytes())?; w.write_all(&32_u16.to_le_bytes())?;
    w.write_all(b"data")?; w.write_all(&bytes.to_le_bytes())?;
    for x in samples { w.write_all(&(*x as f32).to_le_bytes())?; }
    w.flush()
}

fn export(report: &str, wavs: Vec<(String,Vec<f64>)>) {
    if let Some(dir)=std::env::var_os("PSG_H2_ARTIFACT_DIR") {
        let dir=std::path::PathBuf::from(dir); fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("psg-h2-model-report.csv"),report).unwrap();
        for (file,samples) in wavs { write_wav(&dir.join(file),&samples).unwrap(); }
    }
}

#[test]
fn exhaustive_period_sweep_verifies_divider_and_period_zero_one_semantics() {
    for profile in [PsgProfile::Sms,PsgProfile::Sg1000] {
        for channel in 0..3 {
            for period in 0_u16..=0x03ff {
                let mut p=Sn76489Core::new_with_profile(CLOCK,RATE,profile).unwrap();
                p.tone_periods[channel]=period; p.tone_counters[channel]=period; p.tone_outputs[channel]=1;
                if period<=1 {
                    for _ in 0..64 { p.tick_internal_clock(); assert_eq!(p.tone_outputs[channel],1,"{profile:?} ch={channel} p={period}"); }
                } else {
                    let expected=CLOCK as f64/(32.0*f64::from(period));
                    assert!(expected>0.0 && expected<RATE as f64/2.0 || period<=4);
                    for tick in 1..=period {
                        p.tick_internal_clock();
                        assert_eq!(p.tone_outputs[channel],if tick==period {0}else{1},"{profile:?} ch={channel} p={period} high");
                    }
                    for tick in 1..=period {
                        p.tick_internal_clock();
                        assert_eq!(p.tone_outputs[channel],if tick==period {1}else{0},"{profile:?} ch={channel} p={period} low");
                    }
                }
            }
        }
    }
}

#[test]
fn event_reconstruction_is_chunk_invariant_and_compared_with_linear_output() {
    let mut report=String::from("profile,period,frequency_hz,event_count,linear_fundamental,event_fundamental,linear_thd,event_thd,linear_peak,event_peak,event_mean,event_max_delta\n");
    let mut wavs=Vec::new();
    for profile in [PsgProfile::Sms,PsgProfile::Sg1000] {
        for period in [5_u16,6,7,9,14,28,56,112] {
            let (event,events)=event_tone(profile,period,None);
            let (segmented,seg_events)=event_tone(profile,period,Some(997));
            assert_eq!(events,seg_events); assert_eq!(event,segmented);
            let linear=linear_tone(profile,period);
            assert_eq!(event.len(),linear.len());
            assert!(event.iter().all(|x|x.is_finite()));
            let es=stats(&event); let ls=stats(&linear);
            assert!(es.peak<1.0);
            let hz=CLOCK as f64/(32.0*f64::from(period));
            report.push_str(&format!("{},{},{:.9},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12}\n",
                name(profile),period,hz,events,amplitude(&linear,hz),amplitude(&event,hz),thd(&linear,hz),thd(&event,hz),ls.peak,es.peak,es.mean,es.max_delta));
            if [5,14,56].contains(&period) {
                wavs.push((format!("event-{}-p{}.wav",name(profile),period),event.clone()));
                wavs.push((format!("linear-{}-p{}.wav",name(profile),period),linear));
            }
        }
    }
    export(&report,wavs);
}

fn transient_stats(profile: PsgProfile) -> Vec<(String,Stats,Stats)> {
    let mut p=Sn76489Core::new_with_profile(CLOCK,RATE,profile).unwrap();
    setup_tone(&mut p,112);
    let mut out=Vec::new();
    p.advance_clocks(400_000); let a=p.drain_samples(None);
    p.write(0x98); p.advance_clocks(400_000); let b=p.drain_samples(None);
    out.push(("volume-step".into(),stats(&a),stats(&b)));
    p.write(0x8c); p.write(0x11); p.advance_clocks(400_000); let c=p.drain_samples(None);
    out.push(("period-step".into(),stats(&b),stats(&c)));
    p.write(0x9f); p.write(0xe4); p.write(0xf0); p.advance_clocks(400_000); let d=p.drain_samples(None);
    out.push(("tone-to-white-noise".into(),stats(&c),stats(&d)));
    p.write(0xe0); p.advance_clocks(400_000); let e=p.drain_samples(None);
    out.push(("white-to-periodic-noise".into(),stats(&d),stats(&e)));
    out
}

fn noise_stats(profile: PsgProfile, control: u8) -> (Stats,f64,f64) {
    let mut p=Sn76489Core::new_with_profile(CLOCK,RATE,profile).unwrap();
    p.write(0x9f); p.write(0xbf); p.write(0xdf); p.write(0xc5); p.write(0x11);
    p.write(0xe0|control); p.write(0xf0); p.advance_clocks(CLOCKS);
    let s=p.drain_samples(None); let m=stats(&s); let mut cross=0.0; let mut a=0.0; let mut b=0.0;
    for pair in s.windows(2) { cross+=(pair[0]-m.mean)*(pair[1]-m.mean); a+=(pair[0]-m.mean).powi(2); b+=(pair[1]-m.mean).powi(2); }
    let corr=if a>0.0&&b>0.0 {cross/(a*b).sqrt()} else {0.0};
    let zc=s.windows(2).filter(|w|(w[0]<0.0)!=(w[1]<0.0)).count() as f64/s.len().max(1) as f64;
    (m,corr,zc)
}

#[test]
fn transient_and_noise_metrics_cover_both_hardware_profiles() {
    let mut report=String::from("kind,profile,scenario,mean,rms,peak,max_delta,lag1_correlation,zero_crossing_ratio\n");
    for profile in [PsgProfile::Sms,PsgProfile::Sg1000] {
        for (scenario,before,after) in transient_stats(profile) {
            report.push_str(&format!("transient-before,{},{},{:.12},{:.12},{:.12},{:.12},,\n",name(profile),scenario,before.mean,before.rms,before.peak,before.max_delta));
            report.push_str(&format!("transient-after,{},{},{:.12},{:.12},{:.12},{:.12},,\n",name(profile),scenario,after.mean,after.rms,after.peak,after.max_delta));
        }
        for control in 0_u8..8 {
            let (m,corr,zc)=noise_stats(profile,control);
            report.push_str(&format!("noise,{},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12}\n",
                name(profile),if control&4==0 {"periodic"}else{"white"},m.mean,m.rms,m.peak,m.max_delta,corr,zc));
        }
    }
    if let Some(dir)=std::env::var_os("PSG_H2_ARTIFACT_DIR") {
        let dir=std::path::PathBuf::from(dir); fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("psg-h2-transient-noise-report.csv"),report).unwrap();
    }
}
