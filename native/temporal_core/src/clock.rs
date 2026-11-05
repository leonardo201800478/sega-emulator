//! Integer machine time and rational clock-domain conversion.

use core::cmp::Ordering;
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

/// Canonical absolute position on the machine's rational timeline.
///
/// An instant consists of an integer profile-defined master tick and a
/// reduced fractional master tick. The fraction uses u128 components so
/// independently generated clock domains can be compared without floating
/// point or overflow-prone cross multiplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineInstant {
    whole_ticks: MachineTime,
    fractional_numerator: u128,
    fractional_denominator: u128,
}

/// Compatibility name for exact clock-edge positions.
pub type ClockEdgeTime = MachineInstant;

impl MachineInstant {
    /// The origin of the machine timeline.
    pub const ZERO: Self = Self {
        whole_ticks: MachineTime::ZERO,
        fractional_numerator: 0,
        fractional_denominator: 1,
    };

    /// Construct an integral instant.
    pub const fn from_ticks(ticks: u128) -> Self {
        Self {
            whole_ticks: MachineTime::from_ticks(ticks),
            fractional_numerator: 0,
            fractional_denominator: 1,
        }
    }

    /// Construct and normalize an exact rational instant.
    ///
    /// The fractional numerator may exceed the denominator; its whole part is
    /// carried into whole_ticks. A zero denominator and whole-tick overflow
    /// are rejected.
    pub fn from_parts(
        whole_ticks: MachineTime,
        fractional_numerator: u128,
        fractional_denominator: u128,
    ) -> Result<Self, ClockError> {
        if fractional_denominator == 0 {
            return Err(ClockError::ZeroInstantDenominator);
        }
        let carry = fractional_numerator / fractional_denominator;
        let remainder = fractional_numerator % fractional_denominator;
        let ticks = whole_ticks
            .checked_add(carry)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let divisor = gcd_u128(remainder, fractional_denominator);
        Ok(Self {
            whole_ticks: ticks,
            fractional_numerator: remainder / divisor,
            fractional_denominator: fractional_denominator / divisor,
        })
    }

    /// Return the integer master-tick component.
    pub const fn whole_ticks(self) -> MachineTime {
        self.whole_ticks
    }

    /// Return the reduced fractional numerator.
    pub const fn fractional_numerator(self) -> u128 {
        self.fractional_numerator
    }

    /// Return the reduced fractional denominator.
    ///
    /// An integral timestamp has a denominator of one and a numerator of zero.
    pub const fn fractional_denominator(self) -> u128 {
        self.fractional_denominator
    }

    /// Return whether this instant lies exactly on an integer master tick.
    pub const fn is_integral(self) -> bool {
        self.fractional_numerator == 0
    }

    /// Return the integer timestamp only when no fractional component is lost.
    pub const fn as_machine_time(self) -> Option<MachineTime> {
        if self.is_integral() {
            Some(self.whole_ticks)
        } else {
            None
        }
    }

    /// Add a non-negative duration to this absolute instant.
    ///
    /// Fractional arithmetic is exact and checked. An intermediate or final
    /// value that cannot be represented by the u128 rational fields is
    /// rejected rather than rounded.
    pub fn checked_add(self, duration: MachineDuration) -> Result<Self, ClockError> {
        let common = gcd_u128(self.fractional_denominator, duration.fractional_denominator);
        let left_scale = duration.fractional_denominator / common;
        let right_scale = self.fractional_denominator / common;
        let denominator = self
            .fractional_denominator
            .checked_mul(left_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let left = self
            .fractional_numerator
            .checked_mul(left_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let right = duration
            .fractional_numerator
            .checked_mul(right_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let (carry, numerator) = if left >= denominator - right {
            (1, left - (denominator - right))
        } else {
            (0, left + right)
        };
        let whole = self
            .whole_ticks
            .checked_add(duration.whole_ticks)
            .and_then(|ticks| ticks.checked_add(carry))
            .ok_or(ClockError::ArithmeticOverflow)?;
        Self::from_parts(whole, numerator, denominator)
    }

    /// Return the exact non-negative duration elapsed since an earlier instant.
    pub fn checked_duration_since(self, earlier: Self) -> Result<MachineDuration, ClockError> {
        if self < earlier {
            return Err(ClockError::TimeWentBackwards);
        }
        let common = gcd_u128(self.fractional_denominator, earlier.fractional_denominator);
        let left_scale = earlier.fractional_denominator / common;
        let right_scale = self.fractional_denominator / common;
        let denominator = self
            .fractional_denominator
            .checked_mul(left_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let left = self
            .fractional_numerator
            .checked_mul(left_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let right = earlier
            .fractional_numerator
            .checked_mul(right_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let whole_delta = self
            .whole_ticks
            .checked_duration_since(earlier.whole_ticks)
            .ok_or(ClockError::TimeWentBackwards)?;
        let (whole, numerator) = if left < right {
            let whole = whole_delta
                .checked_sub(1)
                .ok_or(ClockError::TimeWentBackwards)?;
            (whole, denominator - (right - left))
        } else {
            (whole_delta, left - right)
        };
        MachineDuration::from_parts(whole, numerator, denominator)
    }
}

impl From<MachineTime> for MachineInstant {
    fn from(time: MachineTime) -> Self {
        Self::from_ticks(time.ticks())
    }
}

impl Ord for MachineInstant {
    fn cmp(&self, other: &Self) -> Ordering {
        self.whole_ticks.cmp(&other.whole_ticks).then_with(|| {
            cmp_fraction_u128(
                self.fractional_numerator,
                self.fractional_denominator,
                other.fractional_numerator,
                other.fractional_denominator,
            )
        })
    }
}

impl PartialOrd for MachineInstant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A non-negative exact displacement on the machine timeline.
///
/// Unlike MachineInstant, a duration has no absolute origin and can be added
/// to another duration or to an instant. Its fractional component is always
/// reduced; arithmetic never uses floating point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineDuration {
    whole_ticks: u128,
    fractional_numerator: u128,
    fractional_denominator: u128,
}

impl MachineDuration {
    /// The zero duration.
    pub const ZERO: Self = Self {
        whole_ticks: 0,
        fractional_numerator: 0,
        fractional_denominator: 1,
    };

    /// Construct an integral duration.
    pub const fn from_ticks(ticks: u128) -> Self {
        Self {
            whole_ticks: ticks,
            fractional_numerator: 0,
            fractional_denominator: 1,
        }
    }

    /// Construct and normalize a non-negative rational duration.
    pub fn from_parts(
        whole_ticks: u128,
        fractional_numerator: u128,
        fractional_denominator: u128,
    ) -> Result<Self, ClockError> {
        if fractional_denominator == 0 {
            return Err(ClockError::ZeroInstantDenominator);
        }
        let carry = fractional_numerator / fractional_denominator;
        let remainder = fractional_numerator % fractional_denominator;
        let whole_ticks = whole_ticks
            .checked_add(carry)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let divisor = gcd_u128(remainder, fractional_denominator);
        Ok(Self {
            whole_ticks,
            fractional_numerator: remainder / divisor,
            fractional_denominator: fractional_denominator / divisor,
        })
    }

    /// Return the integral component of this duration.
    pub const fn whole_ticks(self) -> u128 {
        self.whole_ticks
    }

    /// Return the reduced fractional numerator.
    pub const fn fractional_numerator(self) -> u128 {
        self.fractional_numerator
    }

    /// Return the reduced fractional denominator.
    pub const fn fractional_denominator(self) -> u128 {
        self.fractional_denominator
    }

    /// Add another exact duration, rejecting unrepresentable arithmetic.
    pub fn checked_add(self, other: Self) -> Result<Self, ClockError> {
        let common = gcd_u128(self.fractional_denominator, other.fractional_denominator);
        let left_scale = other.fractional_denominator / common;
        let right_scale = self.fractional_denominator / common;
        let denominator = self
            .fractional_denominator
            .checked_mul(left_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let left = self
            .fractional_numerator
            .checked_mul(left_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let right = other
            .fractional_numerator
            .checked_mul(right_scale)
            .ok_or(ClockError::ArithmeticOverflow)?;
        let (carry, numerator) = if left >= denominator - right {
            (1, left - (denominator - right))
        } else {
            (0, left + right)
        };
        let whole_ticks = self
            .whole_ticks
            .checked_add(other.whole_ticks)
            .and_then(|ticks| ticks.checked_add(carry))
            .ok_or(ClockError::ArithmeticOverflow)?;
        Self::from_parts(whole_ticks, numerator, denominator)
    }
}

impl Ord for MachineDuration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.whole_ticks.cmp(&other.whole_ticks).then_with(|| {
            cmp_fraction_u128(
                self.fractional_numerator,
                self.fractional_denominator,
                other.fractional_numerator,
                other.fractional_denominator,
            )
        })
    }
}

impl PartialOrd for MachineDuration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compare proper non-negative fractions using Euclid's algorithm.
///
/// Continued-fraction comparison avoids multiplying numerators by the other
/// denominator, so ordering remains exact across the full u128 range.
fn cmp_fraction_u128(
    mut left_numerator: u128,
    mut left_denominator: u128,
    mut right_numerator: u128,
    mut right_denominator: u128,
) -> Ordering {
    let mut reversed = false;
    loop {
        let left_whole = left_numerator / left_denominator;
        let right_whole = right_numerator / right_denominator;
        if left_whole != right_whole {
            let ordering = left_whole.cmp(&right_whole);
            return if reversed {
                ordering.reverse()
            } else {
                ordering
            };
        }

        let left_remainder = left_numerator % left_denominator;
        let right_remainder = right_numerator % right_denominator;
        if left_remainder == 0 || right_remainder == 0 {
            let ordering = match (left_remainder == 0, right_remainder == 0) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (false, false) => unreachable!(),
            };
            return if reversed {
                ordering.reverse()
            } else {
                ordering
            };
        }

        left_numerator = left_denominator;
        left_denominator = left_remainder;
        right_numerator = right_denominator;
        right_denominator = right_remainder;
        reversed = !reversed;
    }
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Convert elapsed machine ticks into local cycles and retained phase.
///
/// Splitting the elapsed value before multiplication keeps the remainder
/// product within u128 while ensuring quotient overflow corresponds to a
/// genuinely unrepresentable cycle count.
fn convert_elapsed(
    elapsed: u128,
    ratio: ClockRatio,
    phase: u128,
) -> Result<(u128, u128), ClockError> {
    let denominator = u128::from(ratio.ticks_denominator);
    if phase >= denominator {
        return Err(ClockError::PhaseOutOfRange);
    }

    let numerator = u128::from(ratio.cycles_numerator);
    let elapsed_quotient = elapsed / denominator;
    let elapsed_remainder = elapsed % denominator;
    let remainder_scaled = elapsed_remainder
        .checked_mul(numerator)
        .and_then(|value| value.checked_add(phase))
        .ok_or(ClockError::ArithmeticOverflow)?;
    let whole_cycles = elapsed_quotient
        .checked_mul(numerator)
        .ok_or(ClockError::ArithmeticOverflow)?;
    let cycles = whole_cycles
        .checked_add(remainder_scaled / denominator)
        .ok_or(ClockError::ArithmeticOverflow)?;
    Ok((cycles, remainder_scaled % denominator))
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
        Ok(Self {
            cycles_numerator,
            ticks_denominator,
        })
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
    /// A local edge index is one-based and cannot be zero.
    ZeroEdgeIndex,
    /// Stored clock history is inconsistent with its exact rational ratio.
    InvalidClockHistory,
    /// An exact machine instant cannot have a zero fractional denominator.
    ZeroInstantDenominator,
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroClockNumerator => write!(f, "clock numerator must be positive"),
            Self::ZeroClockDenominator => write!(f, "clock denominator must be positive"),
            Self::TimeWentBackwards => write!(f, "clock domain cannot advance backwards"),
            Self::ArithmeticOverflow => write!(f, "clock-domain arithmetic overflow"),
            Self::PhaseOutOfRange => write!(f, "clock phase must be smaller than its denominator"),
            Self::ZeroEdgeIndex => write!(f, "local clock edge index must be positive"),
            Self::InvalidClockHistory => write!(f, "clock-domain history is inconsistent"),
            Self::ZeroInstantDenominator => {
                write!(f, "machine instant denominator must be positive")
            }
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
    registration_start: MachineTime,
    initial_phase: u128,
}

impl ClockDomain {
    /// Create a domain at start with zero fractional phase and cycle count.
    pub const fn new(ratio: ClockRatio, start: MachineTime) -> Self {
        Self {
            ratio,
            time: start,
            total_cycles: 0,
            phase: 0,
            registration_start: start,
            initial_phase: 0,
        }
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
        Ok(Self {
            ratio,
            time: start,
            total_cycles: 0,
            phase,
            registration_start: start,
            initial_phase: phase,
        })
    }

    /// Restore a domain from its complete rational-clock history.
    ///
    /// The stored counters are accepted only when they satisfy the exact
    /// conversion equation from registration through `current_time`.
    pub fn restore(
        ratio: ClockRatio,
        current_time: MachineTime,
        total_cycles: u128,
        phase: u128,
        registration_start: MachineTime,
        initial_phase: u128,
    ) -> Result<Self, ClockError> {
        let denominator = u128::from(ratio.denominator());
        if phase >= denominator || initial_phase >= denominator {
            return Err(ClockError::PhaseOutOfRange);
        }
        let elapsed = current_time
            .checked_duration_since(registration_start)
            .ok_or(ClockError::InvalidClockHistory)?;
        let (expected_cycles, expected_phase) = convert_elapsed(elapsed, ratio, initial_phase)?;
        if expected_cycles != total_cycles || expected_phase != phase {
            return Err(ClockError::InvalidClockHistory);
        }
        Ok(Self {
            ratio,
            time: current_time,
            total_cycles,
            phase,
            registration_start,
            initial_phase,
        })
    }

    /// Return the domain's registration timestamp.
    pub const fn registration_start(&self) -> MachineTime {
        self.registration_start
    }

    /// Return the rational phase supplied at registration.
    pub const fn initial_phase(&self) -> u128 {
        self.initial_phase
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

    /// Return the exact timestamp of the edge_index-th future local edge.
    ///
    /// The index is one-based relative to this domain's current integer
    /// timestamp and retained phase. No state is mutated. The returned
    /// timestamp is an exact rational master-time position, not rounded to
    /// MachineTime.
    pub fn edge_time_after(&self, edge_index: u128) -> Result<MachineInstant, ClockError> {
        if edge_index == 0 {
            return Err(ClockError::ZeroEdgeIndex);
        }

        // Divide the edge index first so the intermediate product remains
        // bounded by u64::MAX squared instead of multiplying two u128 values.
        let numerator = u128::from(self.ratio.cycles_numerator);
        let denominator = u128::from(self.ratio.ticks_denominator);
        let edge_quotient = edge_index / numerator;
        let edge_remainder = edge_index % numerator;
        let remainder_ticks = edge_remainder * denominator;

        let (whole_offset, fractional_numerator) = if remainder_ticks >= self.phase {
            let partial = remainder_ticks - self.phase;
            let whole = edge_quotient
                .checked_mul(denominator)
                .and_then(|value| value.checked_add(partial / numerator))
                .ok_or(ClockError::ArithmeticOverflow)?;
            (whole, (partial % numerator) as u64)
        } else {
            // This branch implies edge_quotient > 0: edge_index is positive,
            // and an index below the numerator would contribute at least one
            // complete denominator before the initial phase is subtracted.
            let difference = self.phase - remainder_ticks;
            let borrow = difference.div_ceil(numerator);
            let whole = edge_quotient
                .checked_sub(1)
                .and_then(|value| value.checked_mul(denominator))
                .and_then(|value| value.checked_add(denominator - borrow))
                .ok_or(ClockError::ArithmeticOverflow)?;
            let remainder = difference % numerator;
            let fractional = if remainder == 0 {
                0
            } else {
                (numerator - remainder) as u64
            };
            (whole, fractional)
        };
        let whole_ticks = self
            .time
            .checked_add(whole_offset)
            .ok_or(ClockError::ArithmeticOverflow)?;

        let divisor = gcd_u128(u128::from(fractional_numerator), numerator);
        Ok(MachineInstant {
            whole_ticks,
            fractional_numerator: u128::from(fractional_numerator) / divisor,
            fractional_denominator: numerator / divisor,
        })
    }

    /// Return the exact timestamp of the next local clock edge.
    pub fn next_edge_time(&self) -> Result<MachineInstant, ClockError> {
        self.edge_time_after(1)
    }

    /// Preview an absolute advance without mutating the clock domain.
    ///
    /// This is the scheduler's preflight path: it uses the same exact rational
    /// conversion as advance_to, so validation and commit cannot diverge.
    pub fn preview_advance_to(
        &self,
        target: MachineTime,
    ) -> Result<(u128, ClockDomainSnapshot), ClockError> {
        let elapsed = target
            .checked_duration_since(self.time)
            .ok_or(ClockError::TimeWentBackwards)?;
        let (new_cycles, new_phase) = convert_elapsed(elapsed, self.ratio, self.phase)?;
        let new_total = self
            .total_cycles
            .checked_add(new_cycles)
            .ok_or(ClockError::ArithmeticOverflow)?;

        Ok((
            new_cycles,
            ClockDomainSnapshot {
                time: target,
                total_cycles: new_total,
                phase: new_phase,
            },
        ))
    }

    /// Advance to an absolute machine timestamp and return newly elapsed edges.
    ///
    /// Calls must be monotonic. Arithmetic is checked before state is changed,
    /// so an error leaves the domain unchanged.
    /// Advance to an absolute machine timestamp and return newly elapsed edges.
    ///
    /// Calls must be monotonic. Arithmetic is checked before state is changed,
    /// so an error leaves the domain unchanged.
    pub fn advance_to(&mut self, target: MachineTime) -> Result<u128, ClockError> {
        let (new_cycles, snapshot) = self.preview_advance_to(target)?;
        self.time = snapshot.time;
        self.total_cycles = snapshot.total_cycles;
        self.phase = snapshot.phase;
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
    fn next_edge_time_preserves_fractional_master_tick_exactly() {
        let ratio = ClockRatio::new(7, 3).unwrap();
        let domain = ClockDomain::new(ratio, MachineTime::ZERO);
        let edge = domain.next_edge_time().unwrap();

        assert_eq!(edge.whole_ticks(), MachineTime::ZERO);
        assert_eq!(edge.fractional_numerator(), 3);
        assert_eq!(edge.fractional_denominator(), 7);
    }

    #[test]
    fn consecutive_edges_are_exact_and_monotonically_ordered() {
        let ratio = ClockRatio::new(7, 3).unwrap();
        let domain = ClockDomain::new(ratio, MachineTime::ZERO);
        let edges: Vec<_> = (1..=5)
            .map(|index| domain.edge_time_after(index).unwrap())
            .collect();

        assert_eq!(edges[0].whole_ticks(), MachineTime::ZERO);
        assert_eq!(
            (
                edges[0].fractional_numerator(),
                edges[0].fractional_denominator()
            ),
            (3, 7)
        );
        assert_eq!(
            (
                edges[1].fractional_numerator(),
                edges[1].fractional_denominator()
            ),
            (6, 7)
        );
        assert_eq!(edges[2].whole_ticks(), MachineTime::from_ticks(1));
        assert_eq!(
            (
                edges[2].fractional_numerator(),
                edges[2].fractional_denominator()
            ),
            (2, 7)
        );
        assert!(edges.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn edge_times_support_multiple_edges_inside_one_master_tick() {
        let ratio = ClockRatio::new(3, 2).unwrap();
        let domain = ClockDomain::new(ratio, MachineTime::ZERO);
        let first = domain.edge_time_after(1).unwrap();
        let second = domain.edge_time_after(2).unwrap();
        let third = domain.edge_time_after(3).unwrap();

        assert_eq!(
            (
                first.whole_ticks(),
                first.fractional_numerator(),
                first.fractional_denominator()
            ),
            (MachineTime::ZERO, 2, 3)
        );
        assert_eq!(
            (
                second.whole_ticks(),
                second.fractional_numerator(),
                second.fractional_denominator()
            ),
            (MachineTime::from_ticks(1), 1, 3)
        );
        assert_eq!(
            (
                third.whole_ticks(),
                third.fractional_numerator(),
                third.fractional_denominator()
            ),
            (MachineTime::from_ticks(2), 0, 1)
        );
        assert!(first < second && second < third);
    }

    #[test]
    fn edge_time_uses_retained_fractional_phase() {
        let ratio = ClockRatio::new(1, 3).unwrap();
        let domain = ClockDomain::with_phase(ratio, MachineTime::from_ticks(8), 2).unwrap();
        let edge = domain.next_edge_time().unwrap();

        assert_eq!(edge.whole_ticks(), MachineTime::from_ticks(9));
        assert_eq!(
            (edge.fractional_numerator(), edge.fractional_denominator()),
            (0, 1)
        );
    }

    #[test]
    fn edge_time_comparison_is_exact_for_different_denominators() {
        let ratio_a = ClockRatio::new(3, 2).unwrap();
        let ratio_b = ClockRatio::new(7, 3).unwrap();
        let a = ClockDomain::new(ratio_a, MachineTime::ZERO)
            .next_edge_time()
            .unwrap();
        let b = ClockDomain::new(ratio_b, MachineTime::ZERO)
            .next_edge_time()
            .unwrap();
        assert!(b < a);
    }

    #[test]
    fn zero_edge_index_and_timestamp_overflow_are_rejected() {
        let ratio = ClockRatio::new(1, 1).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::ZERO);
        assert_eq!(domain.edge_time_after(0), Err(ClockError::ZeroEdgeIndex));

        domain.time = MachineTime::from_ticks(u128::MAX);
        assert_eq!(domain.next_edge_time(), Err(ClockError::ArithmeticOverflow));
    }

    #[test]
    fn backwards_advance_is_rejected_without_mutating_state() {
        let ratio = ClockRatio::new(1, 1).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::from_ticks(5));
        let before = domain.clone();

        assert_eq!(
            domain.advance_to(MachineTime::from_ticks(4)),
            Err(ClockError::TimeWentBackwards)
        );
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
    fn restored_clock_domain_preserves_complete_history() {
        let ratio = ClockRatio::new(5, 3).unwrap();
        let mut original = ClockDomain::with_phase(ratio, MachineTime::from_ticks(4), 2).unwrap();
        original.advance_to(MachineTime::from_ticks(11)).unwrap();

        let mut restored = ClockDomain::restore(
            original.ratio(),
            original.current_time(),
            original.total_cycles(),
            original.snapshot().phase,
            original.registration_start(),
            original.initial_phase(),
        )
        .unwrap();
        assert_eq!(restored, original);
        original.advance_to(MachineTime::from_ticks(17)).unwrap();
        restored.advance_to(MachineTime::from_ticks(17)).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn restore_round_trips_common_rational_clock_ratios() {
        for (numerator, denominator, initial_phase) in [(3, 2, 1), (7, 3, 2), (1, 3, 2)] {
            let ratio = ClockRatio::new(numerator, denominator).unwrap();
            let mut original =
                ClockDomain::with_phase(ratio, MachineTime::from_ticks(5), initial_phase).unwrap();
            original.advance_to(MachineTime::from_ticks(29)).unwrap();
            let restored = ClockDomain::restore(
                ratio,
                original.current_time(),
                original.total_cycles(),
                original.snapshot().phase,
                original.registration_start(),
                original.initial_phase(),
            )
            .unwrap();
            assert_eq!(restored, original);
        }
    }

    #[test]
    fn restore_rejects_inconsistent_clock_history() {
        let ratio = ClockRatio::new(5, 3).unwrap();
        assert_eq!(
            ClockDomain::restore(
                ratio,
                MachineTime::from_ticks(10),
                1,
                0,
                MachineTime::ZERO,
                0,
            ),
            Err(ClockError::InvalidClockHistory)
        );
    }

    #[test]
    fn restore_rejects_out_of_range_initial_and_current_phase() {
        let ratio = ClockRatio::new(5, 3).unwrap();
        assert_eq!(
            ClockDomain::restore(ratio, MachineTime::ZERO, 0, 0, MachineTime::ZERO, 3),
            Err(ClockError::PhaseOutOfRange)
        );
        assert_eq!(
            ClockDomain::restore(ratio, MachineTime::ZERO, 0, 3, MachineTime::ZERO, 0),
            Err(ClockError::PhaseOutOfRange)
        );
    }

    #[test]
    fn restore_rejects_registration_after_current_time() {
        let ratio = ClockRatio::new(1, 1).unwrap();
        assert_eq!(
            ClockDomain::restore(
                ratio,
                MachineTime::from_ticks(2),
                0,
                0,
                MachineTime::from_ticks(3),
                0,
            ),
            Err(ClockError::InvalidClockHistory)
        );
    }

    #[test]
    fn rational_advance_does_not_overflow_when_reduced_result_fits() {
        let ratio = ClockRatio::new(u64::MAX, u64::MAX).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::ZERO);
        let elapsed = 1_u128 << 65;

        assert_eq!(
            domain.advance_to(MachineTime::from_ticks(elapsed)),
            Ok(elapsed)
        );
        assert_eq!(domain.total_cycles(), elapsed);
        assert_eq!(domain.snapshot().phase, 0);
    }

    #[test]
    fn edge_time_does_not_overflow_when_reduced_timestamp_fits() {
        let ratio = ClockRatio::new(u64::MAX, u64::MAX).unwrap();
        let domain = ClockDomain::new(ratio, MachineTime::ZERO);
        let edge_index = 1_u128 << 65;
        let edge = domain.edge_time_after(edge_index).unwrap();

        assert_eq!(edge.whole_ticks(), MachineTime::from_ticks(edge_index));
        assert_eq!(edge.fractional_numerator(), 0);
        assert_eq!(edge.fractional_denominator(), 1);
    }

    #[test]
    fn restore_accepts_large_exact_history_without_intermediate_overflow() {
        let ratio = ClockRatio::new(u64::MAX, u64::MAX).unwrap();
        let elapsed = 1_u128 << 65;

        let restored = ClockDomain::restore(
            ratio,
            MachineTime::from_ticks(elapsed),
            elapsed,
            0,
            MachineTime::ZERO,
            0,
        )
        .unwrap();

        assert_eq!(restored.total_cycles(), elapsed);
        assert_eq!(restored.snapshot().phase, 0);
    }

    #[test]
    fn rational_clock_conversion_matches_small_integer_oracles() {
        for numerator in 1_u64..=8 {
            for denominator in 1_u64..=8 {
                let ratio = ClockRatio::new(numerator, denominator).unwrap();
                for initial_phase in 0..denominator {
                    for elapsed in 0_u128..=16 {
                        let mut domain = ClockDomain::with_phase(
                            ratio,
                            MachineTime::ZERO,
                            u128::from(initial_phase),
                        )
                        .unwrap();
                        let before = domain.snapshot();
                        let (cycles, preview) = domain
                            .preview_advance_to(MachineTime::from_ticks(elapsed))
                            .unwrap();
                        let scaled = elapsed * u128::from(numerator) + u128::from(initial_phase);
                        assert_eq!(cycles, scaled / u128::from(denominator));
                        assert_eq!(preview.phase, scaled % u128::from(denominator));
                        assert_eq!(domain.snapshot(), before);
                        assert_eq!(
                            domain.advance_to(MachineTime::from_ticks(elapsed)),
                            Ok(cycles)
                        );
                        assert_eq!(domain.snapshot(), preview);
                    }

                    let domain = ClockDomain::with_phase(
                        ratio,
                        MachineTime::ZERO,
                        u128::from(initial_phase),
                    )
                    .unwrap();
                    for edge_index in 1_u128..=8 {
                        let edge = domain.edge_time_after(edge_index).unwrap();
                        let exact_numerator =
                            edge_index * u128::from(denominator) - u128::from(initial_phase);
                        let expected_whole = exact_numerator / u128::from(numerator);
                        let expected_remainder = exact_numerator % u128::from(numerator);
                        assert_eq!(edge.whole_ticks().ticks(), expected_whole);
                        assert_eq!(
                            edge.fractional_numerator() * u128::from(numerator),
                            expected_remainder * edge.fractional_denominator()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn same_timestamp_is_a_valid_noop() {
        let ratio = ClockRatio::new(7, 3).unwrap();
        let mut domain = ClockDomain::new(ratio, MachineTime::from_ticks(9));
        assert_eq!(domain.advance_to(MachineTime::from_ticks(9)), Ok(0));
        assert_eq!(domain.total_cycles(), 0);
    }

    #[test]
    fn machine_instant_normalizes_equivalent_rational_positions() {
        let a = super::MachineInstant::from_parts(MachineTime::from_ticks(4), 2, 6).unwrap();
        let b = super::MachineInstant::from_parts(MachineTime::from_ticks(4), 1, 3).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.fractional_numerator(), 1);
        assert_eq!(a.fractional_denominator(), 3);
        let carried = super::MachineInstant::from_parts(MachineTime::from_ticks(4), 7, 3).unwrap();
        assert_eq!(carried.whole_ticks(), MachineTime::from_ticks(6));
        assert_eq!(carried.fractional_numerator(), 1);
        assert_eq!(carried.fractional_denominator(), 3);
    }

    #[test]
    fn machine_instant_comparison_does_not_cross_multiply_u128_values() {
        let denominator = u128::MAX;
        let left = super::MachineInstant::from_parts(MachineTime::ZERO, 1, denominator).unwrap();
        let right =
            super::MachineInstant::from_parts(MachineTime::ZERO, 1, denominator - 1).unwrap();
        assert!(left < right);
        assert_eq!(left.cmp(&left), core::cmp::Ordering::Equal);
    }

    #[test]
    fn machine_instant_addition_and_subtraction_are_exact() {
        let a = super::MachineInstant::from_parts(MachineTime::from_ticks(5), 2, 3).unwrap();
        let duration = super::MachineDuration::from_parts(0, 1, 2).unwrap();
        let sum = a.checked_add(duration).unwrap();
        assert_eq!(sum.whole_ticks(), MachineTime::from_ticks(6));
        assert_eq!(sum.fractional_numerator(), 1);
        assert_eq!(sum.fractional_denominator(), 6);

        let difference = sum.checked_duration_since(a).unwrap();
        assert_eq!(difference.whole_ticks(), 0);
        assert_eq!(difference.fractional_numerator(), 1);
        assert_eq!(difference.fractional_denominator(), 2);
        assert_eq!(
            a.checked_duration_since(sum),
            Err(ClockError::TimeWentBackwards)
        );
    }

    #[test]
    fn machine_duration_fraction_order_and_arithmetic_match_small_integer_oracles() {
        for left_denominator in 1_u128..=12 {
            for left_numerator in 0..left_denominator {
                let left = super::MachineDuration::from_parts(0, left_numerator, left_denominator)
                    .unwrap();
                for right_denominator in 1_u128..=12 {
                    for right_numerator in 0..right_denominator {
                        let right = super::MachineDuration::from_parts(
                            0,
                            right_numerator,
                            right_denominator,
                        )
                        .unwrap();
                        let expected = (left_numerator * right_denominator)
                            .cmp(&(right_numerator * left_denominator));
                        assert_eq!(left.cmp(&right), expected);

                        let sum = left.checked_add(right).unwrap();
                        let expected_sum = super::MachineDuration::from_parts(
                            0,
                            left_numerator * right_denominator + right_numerator * left_denominator,
                            left_denominator * right_denominator,
                        )
                        .unwrap();
                        assert_eq!(sum, expected_sum);
                    }
                }
            }
        }
    }

    #[test]
    fn machine_instant_rejects_unrepresentable_denominators_and_tick_overflow() {
        assert_eq!(
            super::MachineInstant::from_parts(MachineTime::ZERO, 0, 0),
            Err(ClockError::ZeroInstantDenominator)
        );
        assert_eq!(
            super::MachineInstant::from_parts(MachineTime::from_ticks(u128::MAX), 1, 1),
            Err(ClockError::ArithmeticOverflow)
        );
    }
}
