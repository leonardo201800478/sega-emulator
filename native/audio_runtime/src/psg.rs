//! Scheduler-facing SN76489 device adapter.
//!
//! The machine scheduler owns emulated time. This adapter only consumes clock
//! advances produced by that scheduler and applies PSG writes at dispatched
//! event boundaries; it never owns or advances a private machine clock.

use sega_machine_scheduler::{
    ClockAdvance, ClockDomainId, ClockRatio, MachineScheduler, SchedulerError,
};
use sega_sn76489::{PsgError, PsgProfile, Sn76489Core};

use crate::{NativeAudioSession, PcmSink};

/// Error returned when a scheduler advance belongs to another device domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgClockError {
    WrongClockDomain {
        expected: ClockDomainId,
        actual: ClockDomainId,
    },
}

impl std::fmt::Display for PsgClockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongClockDomain { expected, actual } => write!(
                f,
                "PSG expected clock domain {}, received {}",
                expected.0, actual.0
            ),
        }
    }
}

impl std::error::Error for PsgClockError {}

/// A PSG bound to one scheduler clock domain.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledPsg {
    domain: ClockDomainId,
    core: Sn76489Core,
}

impl ScheduledPsg {
    /// Create a PSG device. The domain must be unique within its machine scheduler.
    pub fn new(
        domain: ClockDomainId,
        profile: PsgProfile,
        clock_hz: u64,
        sample_rate: u64,
    ) -> Result<Self, PsgError> {
        Ok(Self {
            domain,
            core: Sn76489Core::new_with_profile(clock_hz, sample_rate, profile)?,
        })
    }

    /// Register local PSG input-clock edges per machine master tick.
    ///
    /// SMS NTSC with a 10.738635 MHz master and 3.579545 MHz PSG clock uses 1/3.
    pub fn register_clock_domain<T>(
        &self,
        scheduler: &mut MachineScheduler<T>,
        ratio: ClockRatio,
    ) -> Result<(), SchedulerError> {
        scheduler.register_clock_domain(self.domain, ratio)
    }

    /// Consume the PSG clock-edge count produced by one scheduler dispatch.
    pub fn apply_clock_advance(&mut self, advance: &ClockAdvance) -> Result<(), PsgClockError> {
        if advance.domain != self.domain {
            return Err(PsgClockError::WrongClockDomain {
                expected: self.domain,
                actual: advance.domain,
            });
        }
        let mut remaining = advance.elapsed_cycles;
        while remaining > u128::from(u64::MAX) {
            self.core.advance_clocks(u64::MAX);
            remaining -= u128::from(u64::MAX);
        }
        self.core.advance_clocks(remaining as u64);
        Ok(())
    }

    /// Apply a PSG bus write at the timestamp of the currently dispatched event.
    pub fn write_at_dispatch(&mut self, value: u8) {
        self.core.write(value);
    }

    /// Transfer generated PSG samples into the native DSP/mixer/output session.
    pub fn submit_pending_audio<S: PcmSink>(
        &mut self,
        session: &mut NativeAudioSession<S>,
    ) -> Result<usize, String> {
        let samples = self.core.drain_samples(None);
        let count = samples.len();
        session.process_samples(&samples)?;
        Ok(count)
    }

    /// Access the underlying chip state for diagnostics and save states.
    pub const fn core(&self) -> &Sn76489Core {
        &self.core
    }

    /// Mutably access the underlying chip state for diagnostics and tests.
    pub fn core_mut(&mut self) -> &mut Sn76489Core {
        &mut self.core
    }

    /// Return the scheduler domain assigned to this PSG.
    pub const fn clock_domain(&self) -> ClockDomainId {
        self.domain
    }
}

#[cfg(test)]
mod tests {
    use super::{PsgClockError, ScheduledPsg};
    use crate::{NativeAudioSession, PcmSink};
    use sega_machine_scheduler::{
        ClockDomainId, ClockRatio, EventPriority, MachineScheduler, MachineTime,
    };
    use sega_sn76489::PsgProfile;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingSink {
        blocks: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl PcmSink for RecordingSink {
        fn write_pcm(&mut self, pcm: Vec<u8>) -> Result<(), String> {
            self.blocks.lock().unwrap().push(pcm);
            Ok(())
        }
        fn suspend_and_flush(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn metrics(&self) -> (u64, u64) {
            (0, 0)
        }
        fn close(&mut self) {}
    }

    #[test]
    fn scheduler_is_the_only_source_of_psg_clock_advances() {
        let domain = ClockDomainId(7);
        let mut scheduler = MachineScheduler::<u8>::new();
        let mut psg = ScheduledPsg::new(domain, PsgProfile::Sms, 3_579_545, 44_100).unwrap();
        psg.register_clock_domain(&mut scheduler, ClockRatio::new(1, 3).unwrap())
            .unwrap();
        scheduler
            .schedule_at(MachineTime::from_ticks(30), EventPriority::new(0), 0x90)
            .unwrap();

        let dispatch = scheduler.dispatch_next().unwrap().unwrap();
        let advance = dispatch
            .clock_advances
            .iter()
            .find(|advance| advance.domain == domain)
            .unwrap();
        assert_eq!(advance.elapsed_cycles, 10);
        psg.apply_clock_advance(advance).unwrap();
        psg.write_at_dispatch(dispatch.event.payload);
        assert_eq!(psg.core().clock_divider_phase(), 10);
        assert_eq!(scheduler.now(), MachineTime::from_ticks(30));
    }

    #[test]
    fn psg_rejects_clock_advances_from_another_device() {
        let mut psg =
            ScheduledPsg::new(ClockDomainId(7), PsgProfile::Sms, 3_579_545, 44_100).unwrap();
        let mut scheduler = MachineScheduler::<()>::new();
        scheduler
            .register_clock_domain(ClockDomainId(8), ClockRatio::new(1, 3).unwrap())
            .unwrap();
        scheduler
            .schedule_at(MachineTime::from_ticks(3), EventPriority::new(0), ())
            .unwrap();
        let dispatch = scheduler.dispatch_next().unwrap().unwrap();
        assert_eq!(
            psg.apply_clock_advance(&dispatch.clock_advances[0]),
            Err(PsgClockError::WrongClockDomain {
                expected: ClockDomainId(7),
                actual: ClockDomainId(8),
            })
        );
    }

    #[test]
    fn scheduled_psg_samples_reach_the_rust_audio_session() {
        let domain = ClockDomainId(7);
        let mut scheduler = MachineScheduler::<u8>::new();
        let mut psg = ScheduledPsg::new(domain, PsgProfile::Sms, 3_579_545, 44_100).unwrap();
        psg.register_clock_domain(&mut scheduler, ClockRatio::new(1, 1).unwrap())
            .unwrap();
        scheduler
            .schedule_at(MachineTime::from_ticks(48_000), EventPriority::new(0), 0x90)
            .unwrap();
        let dispatch = scheduler.dispatch_next().unwrap().unwrap();
        psg.apply_clock_advance(&dispatch.clock_advances[0])
            .unwrap();

        let sink = RecordingSink::default();
        let blocks = Arc::clone(&sink.blocks);
        let mut session = NativeAudioSession::new(44_100, 100, sink).unwrap();
        let submitted = psg.submit_pending_audio(&mut session).unwrap();
        assert!(submitted > 0);
        session.close();
        assert!(!blocks.lock().unwrap().is_empty());
    }
}
