//! Windows Query Performance Counter access for host-side trace correlation.
//!
//! This clock is intentionally diagnostic-only. It is sampled at low frequency
//! when emitting host telemetry and is never used for pacing or virtual time.

use std::sync::OnceLock;

use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// A cached QPC frequency and a safe accessor for raw QPC timestamps.
#[derive(Debug, Clone, Copy)]
pub struct QpcClock {
    frequency: u64,
}

impl QpcClock {
    /// Create a QPC clock, caching the system frequency for the process lifetime.
    ///
    /// Returns `None` if Windows cannot provide a positive QPC frequency.
    pub fn new() -> Option<Self> {
        static FREQUENCY: OnceLock<Option<u64>> = OnceLock::new();
        let frequency = (*FREQUENCY.get_or_init(|| {
            let mut value = 0_i64;
            // SAFETY: `value` is a valid, writable i64 out-parameter for the API.
            unsafe {
                QueryPerformanceFrequency(&mut value).ok()?;
            }
            u64::try_from(value).ok().filter(|frequency| *frequency > 0)
        }))?;
        Some(Self { frequency })
    }

    /// Read the current raw QPC tick count.
    ///
    /// Returns `None` if the Windows API fails or returns a negative value.
    pub fn now(&self) -> Option<u64> {
        let mut value = 0_i64;
        // SAFETY: `value` is a valid, writable i64 out-parameter for the API.
        unsafe {
            QueryPerformanceCounter(&mut value).ok()?;
        }
        u64::try_from(value).ok()
    }

    /// Return the cached QPC ticks per second.
    pub const fn frequency(&self) -> u64 {
        self.frequency
    }
}

#[cfg(test)]
mod tests {
    use super::QpcClock;

    #[test]
    fn qpc_frequency_is_positive_and_counter_is_monotonic() {
        let clock = QpcClock::new().expect("Windows QPC frequency must be available");
        assert!(clock.frequency() > 0);

        let first = clock.now().expect("first QPC sample must be available");
        let second = clock.now().expect("second QPC sample must be available");
        assert!(second >= first);
    }
}
