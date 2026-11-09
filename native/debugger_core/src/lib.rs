//! Small, console-agnostic debugging primitives shared by emulator runtimes.
//!
//! Hardware-specific snapshots and execution control stay in each console core.
//! This crate only provides address breakpoints and bounded diagnostic history.

use std::collections::VecDeque;

/// Maximum number of address breakpoints accepted by the shared debugger.
pub const MAX_BREAKPOINTS: usize = 64;

/// A validated program-counter breakpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressBreakpoint(u32);

impl AddressBreakpoint {
    /// Create a breakpoint for an address representable by the target CPU.
    #[must_use]
    pub const fn new(address: u32) -> Self {
        Self(address)
    }

    /// Return the address stored by this breakpoint.
    #[must_use]
    pub const fn address(self) -> u32 {
        self.0
    }
}

/// A small set of enabled address breakpoints.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BreakpointSet {
    addresses: Vec<AddressBreakpoint>,
}

impl BreakpointSet {
    /// Insert an address, returning false if it already exists or the set is full.
    pub fn insert(&mut self, address: u32) -> bool {
        let breakpoint = AddressBreakpoint::new(address);
        if self.addresses.contains(&breakpoint) || self.addresses.len() >= MAX_BREAKPOINTS {
            return false;
        }
        self.addresses.push(breakpoint);
        self.addresses.sort_unstable();
        true
    }

    /// Remove an address, returning whether it was present.
    pub fn remove(&mut self, address: u32) -> bool {
        let Some(index) = self
            .addresses
            .iter()
            .position(|item| item.address() == address)
        else {
            return false;
        };
        self.addresses.remove(index);
        true
    }

    /// Check whether an address has an enabled breakpoint.
    #[must_use]
    pub fn contains(&self, address: u32) -> bool {
        self.addresses.iter().any(|item| item.address() == address)
    }

    /// Return the configured addresses in ascending order.
    pub fn addresses(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        self.addresses.iter().map(|item| item.address())
    }

    /// Remove all configured breakpoints.
    pub fn clear(&mut self) {
        self.addresses.clear();
    }

    /// Return the number of configured breakpoints.
    #[must_use]
    pub fn len(&self) -> usize {
        self.addresses.len()
    }

    /// Return whether no breakpoints are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty()
    }
}

/// A fixed-capacity FIFO history that discards the oldest item on overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedHistory<T> {
    capacity: usize,
    entries: VecDeque<T>,
    dropped: u64,
}

impl<T> BoundedHistory<T> {
    /// Create a history with a non-zero maximum number of entries.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::with_capacity(capacity.max(1)),
            dropped: 0,
        }
    }

    /// Append an item, dropping the oldest item when capacity is reached.
    pub fn push(&mut self, item: T) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.entries.push_back(item);
    }

    /// Borrow the retained entries from oldest to newest.
    #[must_use]
    pub fn entries(&self) -> &VecDeque<T> {
        &self.entries
    }

    /// Drain all retained entries from oldest to newest.
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.entries.drain(..)
    }

    /// Return the configured capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return the number of entries discarded due to capacity.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Return the number of retained entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether no entries are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear retained entries and reset the dropped-entry counter.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.dropped = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundedHistory, BreakpointSet, MAX_BREAKPOINTS};

    #[test]
    fn breakpoints_are_unique_sorted_and_removable() {
        let mut points = BreakpointSet::default();
        assert!(points.insert(0x0088));
        assert!(points.insert(0x0000));
        assert!(!points.insert(0x0088));
        assert_eq!(points.addresses().collect::<Vec<_>>(), [0, 0x88]);
        assert!(points.contains(0x88));
        assert!(points.remove(0x88));
        assert!(!points.contains(0x88));
    }

    #[test]
    fn breakpoint_set_has_a_hard_limit() {
        let mut points = BreakpointSet::default();
        for address in 0..MAX_BREAKPOINTS as u32 {
            assert!(points.insert(address));
        }
        assert!(!points.insert(MAX_BREAKPOINTS as u32));
    }

    #[test]
    fn bounded_history_discards_oldest_and_counts_drops() {
        let mut history = BoundedHistory::new(3);
        for value in 0..5 {
            history.push(value);
        }
        assert_eq!(
            history.entries().iter().copied().collect::<Vec<_>>(),
            [2, 3, 4]
        );
        assert_eq!(history.dropped(), 2);
        assert_eq!(history.drain().collect::<Vec<_>>(), [2, 3, 4]);
        assert!(history.is_empty());
    }

    #[test]
    fn zero_capacity_is_normalized_to_one() {
        let mut history = BoundedHistory::new(0);
        history.push(1);
        history.push(2);
        assert_eq!(history.capacity(), 1);
        assert_eq!(history.entries().front(), Some(&2));
        assert_eq!(history.dropped(), 1);
    }
}
