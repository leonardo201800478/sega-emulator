//! Console-neutral primitives for interactive emulator diagnostics.
//!
//! Machine-specific adapters own execution and expose snapshots/events. This
//! module only stores breakpoints and bounded diagnostic history; it never
//! advances emulated time or calls into a CPU.

use std::collections::{BTreeSet, VecDeque};

/// Address breakpoints shared by all console adapters.
///
/// Addresses use a 32-bit value so 8-, 16-, and 24-bit address spaces can use
/// the same type without truncation. Each adapter validates the range it accepts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BreakpointSet {
    addresses: BTreeSet<u32>,
}

impl BreakpointSet {
    /// Insert an address. Returns true when it was not already present.
    pub fn insert(&mut self, address: u32) -> bool { self.addresses.insert(address) }
    /// Remove an address. Returns true when it existed.
    pub fn remove(&mut self, address: u32) -> bool { self.addresses.remove(&address) }
    /// Check whether an address is configured.
    pub fn contains(&self, address: u32) -> bool { self.addresses.contains(&address) }
    /// Remove every configured breakpoint.
    pub fn clear(&mut self) { self.addresses.clear(); }
    /// Iterate over configured addresses in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ { self.addresses.iter().copied() }
    /// Number of configured breakpoints.
    pub fn len(&self) -> usize { self.addresses.len() }
    /// Whether no breakpoint is configured.
    pub fn is_empty(&self) -> bool { self.addresses.is_empty() }
}

/// Fixed-capacity FIFO history for debugger events.
///
/// Oldest entries are discarded when capacity is reached. The number of
/// discarded events is retained so an exported trace cannot imply completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTrace<T> {
    entries: VecDeque<T>,
    capacity: usize,
    dropped: u64,
}

impl<T> BoundedTrace<T> {
    /// Create a history with a non-zero maximum number of entries.
    pub fn new(capacity: usize) -> Self {
        Self { entries: VecDeque::with_capacity(capacity), capacity, dropped: 0 }
    }
    /// Append an event, discarding the oldest event if the buffer is full.
    pub fn push(&mut self, event: T) {
        if self.capacity == 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.entries.push_back(event);
    }
    /// Borrow the retained events in chronological order.
    pub fn as_slices(&self) -> (&[T], &[T]) { self.entries.as_slices() }
    /// Drain all retained events in chronological order.
    pub fn drain(&mut self) -> Vec<T> { self.entries.drain(..).collect() }
    /// Clear retained events and reset the dropped-event counter.
    pub fn clear(&mut self) { self.entries.clear(); self.dropped = 0; }
    /// Current number of retained events.
    pub fn len(&self) -> usize { self.entries.len() }
    /// Whether no events are retained.
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    /// Maximum retained event count.
    pub fn capacity(&self) -> usize { self.capacity }
    /// Number of events discarded because the buffer was full or disabled.
    pub fn dropped(&self) -> u64 { self.dropped }
    /// Iterate over retained events in chronological order.
    pub fn iter(&self) -> impl Iterator<Item = &T> { self.entries.iter() }
}

#[cfg(test)]
mod tests {
    use super::{BoundedTrace, BreakpointSet};

    #[test]
    fn breakpoints_are_unique_ordered_and_removable() {
        let mut points = BreakpointSet::default();
        assert!(points.insert(0x0088));
        assert!(!points.insert(0x0088));
        assert!(points.insert(0x1234));
        assert_eq!(points.iter().collect::<Vec<_>>(), [0x0088, 0x1234]);
        assert!(points.contains(0x0088));
        assert!(points.remove(0x0088));
        assert!(!points.contains(0x0088));
    }

    #[test]
    fn bounded_trace_discards_oldest_and_counts_loss() {
        let mut trace = BoundedTrace::new(3);
        for event in 0..5 { trace.push(event); }
        assert_eq!(trace.iter().copied().collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(trace.dropped(), 2);
        assert_eq!(trace.drain(), [2, 3, 4]);
        assert!(trace.is_empty());
    }

    #[test]
    fn zero_capacity_trace_never_retains_events() {
        let mut trace = BoundedTrace::new(0);
        trace.push(1);
        assert!(trace.is_empty());
        assert_eq!(trace.dropped(), 1);
    }
}
