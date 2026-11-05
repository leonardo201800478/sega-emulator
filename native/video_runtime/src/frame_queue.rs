//! Lossless bounded queue for completed presentation frames.
//!
//! Unlike FrameMailbox, this queue never replaces a pending frame. Producers
//! block at the submission boundary when capacity is exhausted; the consumer
//! receives frames in sequence order. Callers must submit only completed
//! frames, outside Scheduler advancement. This backpressure may reduce wall
//! clock throughput, but it does not advance, rewind, or skip emulated time.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::{VideoFrame, VideoFrameSink};

/// Errors returned by the bounded lossless frame queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameQueueError {
    /// Queue capacity must be greater than zero.
    ZeroCapacity,
    /// A frame sequence is not newer than the last accepted sequence.
    NonMonotonicSequence { last: u64, incoming: u64 },
    /// The queue is closed and no longer accepts frames.
    Closed,
    /// Queue synchronization was poisoned by a panicking thread.
    SynchronizationPoisoned,
    /// The requested timeout cannot be represented by the platform clock.
    TimeoutOverflow,
}

impl fmt::Display for FrameQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => write!(formatter, "frame queue capacity must be non-zero"),
            Self::NonMonotonicSequence { last, incoming } => write!(
                formatter,
                "frame sequence {incoming} is not newer than last accepted sequence {last}"
            ),
            Self::Closed => write!(formatter, "video frame queue is closed"),
            Self::SynchronizationPoisoned => {
                write!(formatter, "video frame queue synchronization is poisoned")
            }
            Self::TimeoutOverflow => write!(formatter, "frame queue timeout exceeds clock range"),
        }
    }
}

impl std::error::Error for FrameQueueError {}

#[derive(Debug, Default)]
struct QueueState {
    frames: VecDeque<VideoFrame>,
    last_sequence: Option<u64>,
    closed: bool,
}

/// A bounded FIFO queue that preserves every accepted completed frame.
///
/// The producer waits on a full queue instead of replacing or dropping a
/// pending frame. This is intended for a host/presentation boundary after a
/// frame has completed; it must not be called from inside Scheduler event
/// advancement. A slow renderer can throttle wall-clock execution at that
/// boundary, but cannot change logical machine time.
#[derive(Debug, Clone)]
pub struct FrameQueue {
    capacity: usize,
    state: Arc<Mutex<QueueState>>,
    not_empty: Arc<Condvar>,
    not_full: Arc<Condvar>,
}

impl FrameQueue {
    /// Create an empty FIFO with a fixed non-zero capacity.
    pub fn new(capacity: usize) -> Result<Self, FrameQueueError> {
        if capacity == 0 {
            return Err(FrameQueueError::ZeroCapacity);
        }

        Ok(Self {
            capacity,
            state: Arc::new(Mutex::new(QueueState::default())),
            not_empty: Arc::new(Condvar::new()),
            not_full: Arc::new(Condvar::new()),
        })
    }

    /// Submit a completed frame, blocking while the bounded queue is full.
    ///
    /// Accepted frames are never replaced. Sequence numbers must increase
    /// strictly, including after a frame has already been consumed.
    pub fn push(&self, frame: VideoFrame) -> Result<(), FrameQueueError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;

        while state.frames.len() >= self.capacity && !state.closed {
            state = self
                .not_full
                .wait(state)
                .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;
        }

        if state.closed {
            return Err(FrameQueueError::Closed);
        }

        if let Some(last) = state.last_sequence
            && frame.sequence() <= last
        {
            return Err(FrameQueueError::NonMonotonicSequence {
                last,
                incoming: frame.sequence(),
            });
        }

        state.last_sequence = Some(frame.sequence());
        state.frames.push_back(frame);
        self.not_empty.notify_one();
        Ok(())
    }

    /// Remove the oldest frame, waiting until one is available or the queue
    /// has been closed and drained.
    pub fn pop(&self) -> Result<Option<VideoFrame>, FrameQueueError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;

        loop {
            if let Some(frame) = state.frames.pop_front() {
                self.not_full.notify_one();
                return Ok(Some(frame));
            }
            if state.closed {
                return Ok(None);
            }
            state = self
                .not_empty
                .wait(state)
                .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;
        }
    }

    /// Remove the oldest frame, returning None when the timeout expires or
    /// after the closed queue has been drained.
    pub fn pop_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<VideoFrame>, FrameQueueError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(FrameQueueError::TimeoutOverflow)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;

        loop {
            if let Some(frame) = state.frames.pop_front() {
                self.not_full.notify_one();
                return Ok(Some(frame));
            }
            if state.closed {
                return Ok(None);
            }

            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(None);
            };
            if remaining.is_zero() {
                return Ok(None);
            }

            let (next_state, timeout_result) = self
                .not_empty
                .wait_timeout(state, remaining)
                .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;
            state = next_state;
            if timeout_result.timed_out() && state.frames.is_empty() {
                return Ok(None);
            }
        }
    }

    /// Close the queue. Already accepted frames remain available for draining.
    /// Blocked producers and consumers are woken and observe the closed state.
    pub fn close(&self) -> Result<(), FrameQueueError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;
        state.closed = true;
        self.not_empty.notify_all();
        self.not_full.notify_all();
        Ok(())
    }

    /// Return the number of accepted frames still waiting for presentation.
    pub fn len(&self) -> Result<usize, FrameQueueError> {
        let state = self
            .state
            .lock()
            .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;
        Ok(state.frames.len())
    }

    /// Return whether no frame is currently waiting for presentation.
    pub fn is_empty(&self) -> Result<bool, FrameQueueError> {
        Ok(self.len()? == 0)
    }

    /// Return the configured maximum number of queued frames.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return whether the queue has been closed to new frames.
    pub fn is_closed(&self) -> Result<bool, FrameQueueError> {
        let state = self
            .state
            .lock()
            .map_err(|_| FrameQueueError::SynchronizationPoisoned)?;
        Ok(state.closed)
    }
}

impl VideoFrameSink for FrameQueue {
    fn submit(&self, frame: VideoFrame) -> Result<(), crate::FrameMailboxError> {
        self.push(frame).map_err(|error| match error {
            FrameQueueError::NonMonotonicSequence { last, incoming } => {
                crate::FrameMailboxError::NonMonotonicSequence { last, incoming }
            }
            FrameQueueError::Closed => crate::FrameMailboxError::Closed,
            FrameQueueError::SynchronizationPoisoned => {
                crate::FrameMailboxError::SynchronizationPoisoned
            }
            FrameQueueError::ZeroCapacity | FrameQueueError::TimeoutOverflow => {
                crate::FrameMailboxError::SynchronizationPoisoned
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameQueue, FrameQueueError};
    use crate::{PixelFormat, VideoFrame};
    use std::sync::mpsc;
    use std::time::Duration;

    fn frame(sequence: u64) -> VideoFrame {
        VideoFrame::new(
            sequence,
            1,
            1,
            PixelFormat::Rgba8888,
            4,
            vec![sequence as u8, 0, 0, 255],
        )
        .unwrap()
    }

    #[test]
    fn rejects_zero_capacity() {
        assert_eq!(FrameQueue::new(0).unwrap_err(), FrameQueueError::ZeroCapacity);
    }

    #[test]
    fn preserves_fifo_order_without_replacing_frames() {
        let queue = FrameQueue::new(3).unwrap();
        queue.push(frame(1)).unwrap();
        queue.push(frame(2)).unwrap();
        queue.push(frame(3)).unwrap();

        assert_eq!(queue.len().unwrap(), 3);
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 1);
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 2);
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 3);
        assert!(queue.is_empty().unwrap());
    }

    #[test]
    fn producer_waits_for_capacity_and_resumes_after_consumer_pop() {
        let queue = FrameQueue::new(1).unwrap();
        queue.push(frame(1)).unwrap();

        let producer_queue = queue.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (submitted_tx, submitted_rx) = mpsc::channel();
        let producer = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            producer_queue.push(frame(2)).unwrap();
            submitted_tx.send(()).unwrap();
        });

        started_rx.recv().unwrap();
        assert!(submitted_rx.try_recv().is_err());
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 1);
        submitted_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 2);
        producer.join().unwrap();
    }

    #[test]
    fn close_wakes_blocked_producers_and_rejects_new_frames() {
        let queue = FrameQueue::new(1).unwrap();
        queue.push(frame(1)).unwrap();

        let producer_queue = queue.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let producer = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            producer_queue.push(frame(2))
        });

        started_rx.recv().unwrap();
        queue.close().unwrap();
        assert_eq!(producer.join().unwrap(), Err(FrameQueueError::Closed));
        assert_eq!(queue.push(frame(3)), Err(FrameQueueError::Closed));
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 1);
        assert_eq!(queue.pop().unwrap(), None);
    }

    #[test]
    fn rejects_non_monotonic_sequences_after_consumption() {
        let queue = FrameQueue::new(2).unwrap();
        queue.push(frame(8)).unwrap();
        assert_eq!(queue.pop().unwrap().unwrap().sequence(), 8);
        assert_eq!(
            queue.push(frame(8)),
            Err(FrameQueueError::NonMonotonicSequence {
                last: 8,
                incoming: 8,
            })
        );
        assert_eq!(
            queue.push(frame(7)),
            Err(FrameQueueError::NonMonotonicSequence {
                last: 8,
                incoming: 7,
            })
        );
    }

    #[test]
    fn timeout_returns_none_without_consuming_future_frames() {
        let queue = FrameQueue::new(1).unwrap();
        assert_eq!(
            queue.pop_timeout(Duration::from_millis(1)).unwrap(),
            None
        );
        queue.push(frame(12)).unwrap();
        assert_eq!(
            queue.pop_timeout(Duration::from_millis(1))
                .unwrap()
                .unwrap()
                .sequence(),
            12
        );
    }
}
