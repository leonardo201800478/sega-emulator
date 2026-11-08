//! Bounded single-producer/single-consumer transport for stereo PCM16 frames.
//!
//! The queue transports complete stereo frames. It never overwrites unread data:
//! a full queue rejects the newest frames and reports the exact loss. A consumer
//! always receives the requested number of frames; missing frames are zero-filled
//! and accounted as underrun frames. This module does not own a device clock.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use rtrb::{Consumer, Producer, RingBuffer};

/// One interleaved stereo PCM16 frame: [left, right].
pub type StereoFrame = [i16; 2];

/// Errors returned while constructing or using the PCM frame queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmFrameQueueError {
    /// A bounded queue must have at least one frame of capacity.
    ZeroCapacity,
    /// The consumer endpoint has already been dropped.
    ConsumerDisconnected,
}

impl std::fmt::Display for PcmFrameQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroCapacity => write!(f, "PCM frame queue capacity must be positive"),
            Self::ConsumerDisconnected => write!(f, "PCM frame queue consumer is disconnected"),
        }
    }
}

impl std::error::Error for PcmFrameQueueError {}

#[derive(Debug, Default)]
struct SharedMetrics {
    accepted_frames: AtomicU64,
    consumed_frames: AtomicU64,
    dropped_frames: AtomicU64,
    underrun_frames: AtomicU64,
    underrun_events: AtomicU64,
    high_water_frames: AtomicUsize,
}

/// Immutable snapshot of queue counters and occupancy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PcmFrameQueueMetrics {
    /// Fixed queue capacity in stereo frames.
    pub capacity_frames: usize,
    /// Approximate instantaneous occupancy observed by the queried endpoint.
    pub queued_frames: usize,
    /// Maximum occupancy observed after a successful producer write.
    pub high_water_frames: usize,
    /// Frames successfully published by the producer.
    pub accepted_frames: u64,
    /// Newest frames rejected because the bounded queue was full.
    pub dropped_frames: u64,
    /// Frames consumed from the queue by the output thread.
    pub consumed_frames: u64,
    /// Silence frames inserted because the queue did not have enough data.
    pub underrun_frames: u64,
    /// Consumer read operations that required at least one silence frame.
    pub underrun_events: u64,
}

/// Result of publishing a batch of stereo frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PcmFrameWriteReport {
    /// Frames accepted by the queue.
    pub accepted_frames: usize,
    /// Frames rejected because the queue was full.
    pub dropped_frames: usize,
}

/// Result of filling one consumer buffer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PcmFrameReadReport {
    /// Frames copied from the queue.
    pub queued_frames: usize,
    /// Frames replaced with digital silence.
    pub silence_frames: usize,
    /// True when the producer endpoint has been dropped.
    pub producer_closed: bool,
}

/// Producer endpoint. It must be owned by exactly one producer thread.
pub struct PcmFrameProducer {
    producer: Producer<StereoFrame>,
    metrics: Arc<SharedMetrics>,
    capacity_frames: usize,
}

impl PcmFrameProducer {
    /// Publish as many frames as fit without waiting or overwriting unread data.
    ///
    /// Overflow policy is drop-newest. Every rejected frame is counted
    /// individually. No allocation is performed by this method.
    pub fn push_frames(
        &mut self,
        frames: &[StereoFrame],
    ) -> Result<PcmFrameWriteReport, PcmFrameQueueError> {
        if self.producer.is_abandoned() {
            return Err(PcmFrameQueueError::ConsumerDisconnected);
        }

        let mut report = PcmFrameWriteReport::default();
        for frame in frames {
            match self.producer.push(*frame) {
                Ok(()) => {
                    report.accepted_frames += 1;
                    let occupancy = self.capacity_frames - self.producer.slots();
                    self.metrics
                        .high_water_frames
                        .fetch_max(occupancy, Ordering::Relaxed);
                }
                Err(rtrb::PushError::Full(_)) => {
                    report.dropped_frames += 1;
                }
            }
        }

        self.metrics
            .accepted_frames
            .fetch_add(report.accepted_frames as u64, Ordering::Relaxed);
        self.metrics
            .dropped_frames
            .fetch_add(report.dropped_frames as u64, Ordering::Relaxed);
        Ok(report)
    }

    /// Return a snapshot, including occupancy observed from the producer side.
    pub fn metrics(&self) -> PcmFrameQueueMetrics {
        metrics_snapshot(
            self.capacity_frames,
            self.capacity_frames - self.producer.slots(),
            &self.metrics,
        )
    }
}

/// Consumer endpoint. It must be owned by exactly one consumer thread.
pub struct PcmFrameConsumer {
    consumer: Consumer<StereoFrame>,
    metrics: Arc<SharedMetrics>,
    capacity_frames: usize,
}

impl PcmFrameConsumer {
    /// Fill the complete output slice, inserting silence only for missing frames.
    ///
    /// This method never waits and never allocates. The number of inserted
    /// silence frames and underrun events is recorded exactly.
    pub fn read_frames(&mut self, output: &mut [StereoFrame]) -> PcmFrameReadReport {
        let mut report = PcmFrameReadReport::default();

        while report.queued_frames < output.len() {
            match self.consumer.pop() {
                Ok(frame) => {
                    output[report.queued_frames] = frame;
                    report.queued_frames += 1;
                }
                Err(rtrb::PopError::Empty) => break,
            }
        }

        report.silence_frames = output.len() - report.queued_frames;
        output[report.queued_frames..].fill([0, 0]);

        self.metrics
            .consumed_frames
            .fetch_add(report.queued_frames as u64, Ordering::Relaxed);
        if report.silence_frames > 0 {
            self.metrics
                .underrun_frames
                .fetch_add(report.silence_frames as u64, Ordering::Relaxed);
            self.metrics
                .underrun_events
                .fetch_add(1, Ordering::Relaxed);
            report.producer_closed = self.consumer.is_abandoned();
        }

        report
    }

    /// Return a snapshot, including occupancy observed from the consumer side.
    pub fn metrics(&self) -> PcmFrameQueueMetrics {
        metrics_snapshot(
            self.capacity_frames,
            self.consumer.slots(),
            &self.metrics,
        )
    }
}

fn metrics_snapshot(
    capacity_frames: usize,
    queued_frames: usize,
    metrics: &SharedMetrics,
) -> PcmFrameQueueMetrics {
    PcmFrameQueueMetrics {
        capacity_frames,
        queued_frames,
        high_water_frames: metrics.high_water_frames.load(Ordering::Relaxed),
        accepted_frames: metrics.accepted_frames.load(Ordering::Relaxed),
        dropped_frames: metrics.dropped_frames.load(Ordering::Relaxed),
        consumed_frames: metrics.consumed_frames.load(Ordering::Relaxed),
        underrun_frames: metrics.underrun_frames.load(Ordering::Relaxed),
        underrun_events: metrics.underrun_events.load(Ordering::Relaxed),
    }
}

/// Factory for a fixed-capacity, lock-free SPSC PCM frame queue.
pub struct PcmFrameQueue;

impl PcmFrameQueue {
    /// Allocate the queue and return its unique producer and consumer endpoints.
    pub fn new(
        capacity_frames: usize,
    ) -> Result<(PcmFrameProducer, PcmFrameConsumer), PcmFrameQueueError> {
        if capacity_frames == 0 {
            return Err(PcmFrameQueueError::ZeroCapacity);
        }

        let (producer, consumer) = RingBuffer::new(capacity_frames);
        let metrics = Arc::new(SharedMetrics::default());
        Ok((
            PcmFrameProducer {
                producer,
                metrics: Arc::clone(&metrics),
                capacity_frames,
            },
            PcmFrameConsumer {
                consumer,
                metrics,
                capacity_frames,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PcmFrameQueue, PcmFrameQueueError, PcmFrameReadReport, PcmFrameWriteReport, StereoFrame,
    };

    #[test]
    fn rejects_zero_capacity() {
        assert_eq!(
            PcmFrameQueue::new(0).err(),
            Some(PcmFrameQueueError::ZeroCapacity)
        );
    }

    #[test]
    fn preserves_frame_order_across_different_write_and_read_sizes() {
        let (mut producer, mut consumer) = PcmFrameQueue::new(8).unwrap();
        let input: [StereoFrame; 5] = [[1, -1], [2, -2], [3, -3], [4, -4], [5, -5]];

        assert_eq!(
            producer.push_frames(&input[..2]).unwrap(),
            PcmFrameWriteReport {
                accepted_frames: 2,
                dropped_frames: 0,
            }
        );
        assert_eq!(
            producer.push_frames(&input[2..]).unwrap(),
            PcmFrameWriteReport {
                accepted_frames: 3,
                dropped_frames: 0,
            }
        );

        let mut first = [[0, 0]; 3];
        assert_eq!(
            consumer.read_frames(&mut first),
            PcmFrameReadReport {
                queued_frames: 3,
                silence_frames: 0,
                producer_closed: false,
            }
        );
        assert_eq!(first, input[..3]);

        let mut second = [[0, 0]; 2];
        assert_eq!(
            consumer.read_frames(&mut second),
            PcmFrameReadReport {
                queued_frames: 2,
                silence_frames: 0,
                producer_closed: false,
            }
        );
        assert_eq!(second, input[3..]);
    }

    #[test]
    fn overflow_drops_newest_frames_and_counts_exact_loss() {
        let (mut producer, mut consumer) = PcmFrameQueue::new(3).unwrap();
        let input: [StereoFrame; 5] = [[1, 1], [2, 2], [3, 3], [4, 4], [5, 5]];

        assert_eq!(
            producer.push_frames(&input).unwrap(),
            PcmFrameWriteReport {
                accepted_frames: 3,
                dropped_frames: 2,
            }
        );

        let mut output = [[0, 0]; 3];
        assert_eq!(consumer.read_frames(&mut output).queued_frames, 3);
        assert_eq!(output, input[..3]);

        let metrics = consumer.metrics();
        assert_eq!(metrics.capacity_frames, 3);
        assert_eq!(metrics.queued_frames, 0);
        assert_eq!(metrics.high_water_frames, 3);
        assert_eq!(metrics.accepted_frames, 3);
        assert_eq!(metrics.dropped_frames, 2);
        assert_eq!(metrics.consumed_frames, 3);
    }

    #[test]
    fn underflow_silences_only_missing_frames_and_counts_exact_loss() {
        let (mut producer, mut consumer) = PcmFrameQueue::new(4).unwrap();
        let input: [StereoFrame; 2] = [[100, -100], [200, -200]];
        producer.push_frames(&input).unwrap();

        let mut output = [[9, 9]; 4];
        assert_eq!(
            consumer.read_frames(&mut output),
            PcmFrameReadReport {
                queued_frames: 2,
                silence_frames: 2,
                producer_closed: false,
            }
        );
        assert_eq!(output, [[100, -100], [200, -200], [0, 0], [0, 0]]);

        let metrics = consumer.metrics();
        assert_eq!(metrics.underrun_frames, 2);
        assert_eq!(metrics.underrun_events, 1);
    }

    #[test]
    fn empty_reads_count_silence_but_zero_length_reads_do_not() {
        let (_producer, mut consumer) = PcmFrameQueue::new(2).unwrap();
        let mut empty: [StereoFrame; 0] = [];
        assert_eq!(consumer.read_frames(&mut empty).silence_frames, 0);
        assert_eq!(consumer.metrics().underrun_events, 0);

        let mut output = [[1, 1]; 3];
        assert_eq!(consumer.read_frames(&mut output).silence_frames, 3);
        assert_eq!(output, [[0, 0]; 3]);
        assert_eq!(consumer.metrics().underrun_frames, 3);
        assert_eq!(consumer.metrics().underrun_events, 1);
    }

    #[test]
    fn reports_disconnected_producer_after_draining_queued_frames() {
        let (mut producer, mut consumer) = PcmFrameQueue::new(2).unwrap();
        producer.push_frames(&[[7, -7]]).unwrap();
        drop(producer);

        let mut output = [[0, 0]; 2];
        assert_eq!(
            consumer.read_frames(&mut output),
            PcmFrameReadReport {
                queued_frames: 1,
                silence_frames: 1,
                producer_closed: true,
            }
        );
        assert_eq!(output, [[7, -7], [0, 0]]);
    }

    #[test]
    fn reports_consumer_disconnect_to_producer() {
        let (mut producer, consumer) = PcmFrameQueue::new(2).unwrap();
        drop(consumer);
        assert_eq!(
            producer.push_frames(&[[1, 1]]),
            Err(PcmFrameQueueError::ConsumerDisconnected)
        );
    }
}
