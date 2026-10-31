//! Integer machine time and rational clock-domain conversion.

use core::fmt;

/// Absolute tick count in one machine profile's chosen master-time unit.
///
/// The unit is profile-defined; it is not a universal number of hertz. A
/// platform must document how its master unit relates to its hardware clocks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineTime(u128);

impl MachineTime {
    /// Return the zero timestamp.
    pub const ZERO: Self = Self(0);

    /// Construct a timestamp from profile-defined master ticks.
    pub const fn from_ticks(ticks: u128) -> Self {
        Self(ticks)
    }

    /// Return the underlying profile-defined master tick count.
    pub const fn ticks(self) -> u128 {
        self.0
    }

    /// Add a duration without allowing integer wraparound.
    pub const fn checked_add(self, ticks: u128) -> Option<Self> {
        match self.0.checked_add(ticks) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Return elapsed master ticks from an earlier timestamp.
    pub const fn checked_duration_since(self, earlier: Self) -> Option<u128> {
        self.0.checked_sub(earlier.0)
    }
}

/// Rational number of local device clock edges per master-time tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockRatio {
    cycles_numerator: u64,
    ticks_denominator: u64,
}

impl ClockRatio {
    /// Construct a positive rational clock ratio.
    pub const fn new(cycles_numerator: u64, ticks_denominator: u64) -> Result<Self, ClockError> {
        if cycles_numerator == 0 {
            return Err(ClockError::ZeroClockNumerator);
        }
        if ticks_denominator == 0 {
            return Err(ClockError::ZeroClockDenominator);
        }
        Ok(Self { cycles_numerator, ticks_denominator })
    }

    /// Return the numerator of local cycles per master tick.
    pub const fn numerator(self) -> u64 {
        self.cycles_numerator
    }

    /// Return the denominator of local cycles per master tick.
    pub const fn denominator(self) -> u64 {
        self.ticks_denominator
    }
}

/// Errors reported by machine-time and clock-domain operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    /// A clock ratio cannot describe a stopped clock.
    ZeroClockNumerator,
    /// A rational clock ratio cannot have a zero denominator.
    ZeroClockDenominator,
    /// The requested timestamp precedes the domain's current timestamp.
    TimeWentBackwards,
    /// A checked integer operation exceeded the supported range.
    ArithmeticOverflow,
    /// The initial fractional phase must be smaller than the denominator.
    PhaseOutOfRange,
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroClockNumerator => write!(f, "clock numerator must be positive"),
            Self::ZeroClockDenominator => write!(f, "clock denominator must be positive"),
            Self::TimeWentBackwards => write!(f, "clock domain cannot advance backwards"),
            Self::ArithmeticOverflow => write!(f, "clock-domain arithmetic overflow"),
            Self::PhaseOutOfRange => write!(f, "clock phase must be smaller than its denominator"),
        }
    }
}

impl std::error::Error for ClockError {}

/// Snapshot of a clock domain's deterministic conversion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockDomainSnapshot {
    /// Timestamp through which this domain has already been advanced.
    pub time: MachineTime,
    /// Number of local clock edges accumulated since construction/reset.
    pub total_cycles: u128,
    /// Fractional numerator remainder retained for chunk-invariant conversion.
    pub phase: u128,
}

/// Converts machine time into local device clock edges without floating point.
///
/// The domain retains its rational remainder, so advancing in many small
/// chunks produces exactly the same edge count and phase as one large advance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockDomain {
    ratio: ClockRatio,
    time: MachineTime,
    total_cycles: u128,
    phase: u128,
}

impl ClockDomain {
    /// Create a domain at start with zero fractional phase and cycle count.
    pub const fn new(ratio: ClockRatio, start: MachineTime) -> Self {
        Self { ratio, time: start, total_cycles: 0, phase: 0 }
    }

    /// Create a domain with an explicit initial phase.
    pub const fn with_phase(
        ratio: ClockRatio,
        start: MachineTime,
        phase: u128,
    ) -> Result<Self, ClockError> {
        if phase >= ratio.ticks_denominator as u128 {
            return Err(ClockError::PhaseOutOfRange);
        }
        Ok(Self { ratio, time: start, total_cycles: 0, phase })
    }

    /// Return the domain's immutable clock ratio.
    pub const fn ratio(&self) -> ClockRatio {
        self.ratio
    }

    /// Return the timestamp through which this domain has advanced.
    pub const fn current_time(&self) -> MachineTime {
        self.time
    }

    /// Return the accumulated local clock edges.
    pub const fn total_cycles(&self) -> u128 {
        self.total_cycles
    }

    /// Advance to an absolute machine timestamp and return newly elapsed edges.
    ///
    /// Calls must be monotonic. Arithmetic is checked before state is changed,
    /// so an error leaves the domain unchanged.
    pub fn advance_to(&mut self, target: MachineTime) -> Result<u128, ClockError> {
        let elapsed = target.checked_duration_since(self.time)
            .ok_or(ClockError::TimeWentBackwards)?;
        let scaled = elapsed
            .checked_mul(u128::from(self.ratio.cycles_numerator))
            .and_then(|value| value.checked_add(self.phase))
            .ok_or(ClockError::ArithmeticOverflow)?;
        let denominator = u128::from(self.ratio.ticks_denominator);
        let new_cycles = scaled / denominator;
        let new_phase = scaled % denominator;
        let new_total = self.total_cycles.checked_add(new_cycles)
            .ok_or(ClockError::ArithmeticOverflow)?;

        self.time = target;
        self.total_cycles = new_total;
        self.phase = new_phase;
        Ok(new_cycles)
    }

    /// Capture all state required to reproduce future clock conversion.
    pub const fn snapshot(&self) -> ClockDomainSnapshot {
        ClockDomainSnapshot {
            time: self.time,
            total_cycles: self.total_cycles,
            phase: self.phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClockDomain, ClockError, ClockRatio, MachineTime};

    #[test]
    fn rational_clock_conversion_is_chunk_invariant() {
        let ratio = ClockRatio::new(3, 2).unwrap();
        let mut whole = ClockDomain::new(ratio, MachineTime::ZERO);
        let mut chunked = whole.clone();

        assert_eq!(whole.advance_to(MachineTime::from_ticks(10)), Ok(15));
        assert_eq!(chunked.advance_to(MachineTime::from_ticks(3)), Ok(4));
        assert_eq!(chunked.advance_to(MachineTime::from_ticks(7)), Ok(6));
        assert_eq!(chunked.advance_to(MachineTime::from_ticks(10)), Ok(5));
        assert_eq!(chunked, whole);
    }

    #[test]
    fn subcycle_phase_is_preserved_until_an_edge_occurs() {
        let ratio = ClockRatio::new(1, 3).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::ZERO);

        assert_eq!(domain.advance_to(MachineTime::from_ticks(1)), Ok(0));
        assert_eq!(domain.snapshot().phase, 1);
        assert_eq!(domain.advance_to(MachineTime::from_ticks(2)), Ok(0));
        assert_eq!(domain.advance_to(MachineTime::from_ticks(3)), Ok(1));
        assert_eq!(domain.total_cycles(), 1);
        assert_eq!(domain.snapshot().phase, 0);
    }

    #[test]
    fn backwards_advance_is_rejected_without_mutating_state() {
        let ratio = ClockRatio::new(1, 1).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::from_ticks(5));
        let before = domain.clone();

        assert_eq!(domain.advance_to(MachineTime::from_ticks(4)), Err(ClockError::TimeWentBackwards));
        assert_eq!(domain, before);
    }

    #[test]
    fn invalid_ratios_and_phases_are_rejected() {
        assert_eq!(ClockRatio::new(0, 1), Err(ClockError::ZeroClockNumerator));
        assert_eq!(ClockRatio::new(1, 0), Err(ClockError::ZeroClockDenominator));
        let ratio = ClockRatio::new(1, 2).unwrap();
        assert_eq!(
            ClockDomain::with_phase(ratio, MachineTime::ZERO, 2),
            Err(ClockError::PhaseOutOfRange)
        );
    }

    #[test]
    fn overflow_does_not_partially_advance_the_domain() {
        let ratio = ClockRatio::new(u64::MAX, 1).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::ZERO);
        let before = domain.clone();

        assert_eq!(
            domain.advance_to(MachineTime::from_ticks(u128::MAX)),
            Err(ClockError::ArithmeticOverflow)
        );
        assert_eq!(domain, before);
    }

    #[test]
    fn same_timestamp_is_a_valid_noop() {
        let ratio = ClockRatio::new(7, 3).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::from_ticks(9));
        assert_eq!(domain.advance_to(MachineTime::from_ticks(9)), Ok(0));
        assert_eq!(domain.total_cycles(), 0);
    }
}
