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

/// Result of publishing one owned diagnostic event to the collector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishResult {
    /// The collector accepted the event without blocking.
    Accepted,
    /// The bounded queue is full; the event was discarded.
    QueueFull,
    /// The collector has been stopped.
    Inactive,
}

enum CollectorMessage<T> {
    Event(T),
    Barrier(std::sync::mpsc::SyncSender<()>),
    Shutdown,
}

/// Cloneable non-blocking producer for a dedicated diagnostic collector thread.
///
/// Publishing never waits for the collector: a full bounded queue discards the
/// event and increments the producer-side drop counter.
pub struct DiagnosticPublisher<T> {
    sender: std::sync::mpsc::SyncSender<CollectorMessage<T>>,
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl<T> Clone for DiagnosticPublisher<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            active: std::sync::Arc::clone(&self.active),
            dropped: std::sync::Arc::clone(&self.dropped),
        }
    }
}

impl<T> DiagnosticPublisher<T> {
    /// Attempt to publish an event without waiting for queue capacity.
    pub fn try_publish(&self, event: T) -> PublishResult {
        use std::sync::atomic::Ordering;
        if !self.active.load(Ordering::Acquire) {
            return PublishResult::Inactive;
        }
        match self.sender.try_send(CollectorMessage::Event(event)) {
            Ok(()) => PublishResult::Accepted,
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                PublishResult::QueueFull
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => PublishResult::Inactive,
        }
    }

    /// Record events discarded by an upstream bounded producer buffer.
    ///
    /// This keeps machine-side trace overflow in the same diagnostic drop count.
    pub fn record_dropped(&self, count: u64) {
        use std::sync::atomic::Ordering;
        if count > 0 && self.active.load(Ordering::Acquire) {
            self.dropped.fetch_add(count, Ordering::Relaxed);
        }
    }

    /// Return the number of events discarded before reaching retained history.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Return whether this publisher still targets an active collector.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// Owns a dedicated thread that drains diagnostic events into bounded memory.
///
/// Construct this type only after an explicit user request to enable debugger
/// mode. Dropping it stops the collector and clears retained diagnostic data.
pub struct DiagnosticCollector<T: Send + 'static> {
    publisher: DiagnosticPublisher<T>,
    history: std::sync::Arc<std::sync::Mutex<BoundedHistory<T>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl<T: Send + 'static> DiagnosticCollector<T> {
    /// Start a collector with separately bounded ingress and retained history.
    ///
    /// Both capacities are normalized to at least one element.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the operating system cannot create the collector thread.
    pub fn start(queue_capacity: usize, history_capacity: usize) -> std::io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(queue_capacity.max(1));
        let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let history =
            std::sync::Arc::new(std::sync::Mutex::new(BoundedHistory::new(history_capacity)));
        let collector_history = std::sync::Arc::clone(&history);
        let thread = std::thread::Builder::new()
            .name("sega-debugger-collector".to_owned())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        CollectorMessage::Event(event) => {
                            if let Ok(mut retained) = collector_history.lock() {
                                retained.push(event);
                            }
                        }
                        CollectorMessage::Barrier(reply) => {
                            let _ = reply.send(());
                        }
                        CollectorMessage::Shutdown => break,
                    }
                }
            })?;
        Ok(Self {
            publisher: DiagnosticPublisher {
                sender,
                active,
                dropped,
            },
            history,
            thread: Some(thread),
        })
    }

    /// Return a cloneable, non-blocking publisher for the emulation worker.
    #[must_use]
    pub fn publisher(&self) -> DiagnosticPublisher<T> {
        self.publisher.clone()
    }

    /// Return a cloned snapshot of retained events, oldest first.
    #[must_use]
    pub fn snapshot(&self) -> Vec<T>
    where
        T: Clone,
    {
        let (reply, acknowledged) = std::sync::mpsc::sync_channel(1);
        if self
            .publisher
            .sender
            .send(CollectorMessage::Barrier(reply))
            .is_ok()
        {
            let _ = acknowledged.recv();
        }
        self.history
            .lock()
            .map(|history| history.entries().iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Return the number of events discarded by upstream producers or the ingress queue.
    #[must_use]
    pub fn ingress_dropped(&self) -> u64 {
        self.publisher.dropped()
    }

    /// Return the number of events discarded by the bounded retained history.
    #[must_use]
    pub fn history_dropped(&self) -> u64 {
        self.history
            .lock()
            .map(|history| history.dropped())
            .unwrap_or_default()
    }

    /// Stop the collector, join its thread, and clear retained diagnostic data.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        use std::sync::atomic::Ordering;
        self.publisher.active.store(false, Ordering::Release);
        // This may wait for queue capacity, but it runs on the host/control
        // thread, never on the emulation worker.
        let _ = self.publisher.sender.send(CollectorMessage::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if let Ok(mut history) = self.history.lock() {
            history.clear();
        }
    }
}

impl<T: Send + 'static> Drop for DiagnosticCollector<T> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundedHistory, BreakpointSet};

    #[test]
    fn diagnostic_collector_runs_on_a_dedicated_thread_and_retains_events() {
        use super::{DiagnosticCollector, PublishResult};
        let collector = DiagnosticCollector::start(8, 3).unwrap();
        let publisher = collector.publisher();
        assert_eq!(publisher.try_publish(10), PublishResult::Accepted);
        assert_eq!(publisher.try_publish(11), PublishResult::Accepted);
        // A snapshot is an observation of events already drained by the collector.
        assert_eq!(collector.snapshot(), [10, 11]);
        assert_eq!(collector.ingress_dropped(), 0);
    }

    #[test]
    fn stopped_collector_rejects_new_events_and_clears_retained_data() {
        use super::{DiagnosticCollector, PublishResult};
        let collector = DiagnosticCollector::start(4, 4).unwrap();
        let publisher = collector.publisher();
        assert_eq!(publisher.try_publish(7), PublishResult::Accepted);
        collector.stop();
        assert_eq!(publisher.try_publish(8), PublishResult::Inactive);
    }

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
        for address in 0_u32..64 {
            assert!(points.insert(address));
        }
        assert!(!points.insert(64));
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
    #[test]
    fn collector_reports_both_ingress_and_retained_history_drops() {
        use super::{DiagnosticCollector, PublishResult};

        let collector = DiagnosticCollector::start(8, 2).unwrap();
        let publisher = collector.publisher();
        assert_eq!(publisher.try_publish(1), PublishResult::Accepted);
        assert_eq!(publisher.try_publish(2), PublishResult::Accepted);
        assert_eq!(publisher.try_publish(3), PublishResult::Accepted);
        publisher.record_dropped(4);

        assert_eq!(collector.snapshot(), [2, 3]);
        assert_eq!(collector.ingress_dropped(), 4);
        assert_eq!(collector.history_dropped(), 1);
    }
}
