//! Backend-independent immutable video-frame contracts.
//!
//! This crate is intentionally headless and platform-neutral. It contains no
//! emulator timing, Python bindings, window-system handles, or graphics API
//! ownership. Native presentation backends consume completed frames through
//! this contract.

#[cfg(windows)]
pub mod d3d12;

pub mod frame_queue;
pub use frame_queue::{FrameQueue, FrameQueueError};

use core::fmt;
use std::sync::Arc;

/// Pixel encoding stored in a VideoFrame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// 16-bit RGB, little-endian byte order (R5:G6:B5).
    Rgb565Le,
    /// Four bytes per pixel in red, green, blue, alpha order.
    Rgba8888,
}

impl PixelFormat {
    /// Return the number of bytes occupied by one pixel.
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgb565Le => 2,
            Self::Rgba8888 => 4,
        }
    }
}

/// A rectangle in frame-pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoRect {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Rectangle width in pixels.
    pub width: u32,
    /// Rectangle height in pixels.
    pub height: u32,
}

impl VideoRect {
    /// Return true when the rectangle has non-zero dimensions and fits in a frame.
    pub const fn is_within(self, width: u32, height: u32) -> bool {
        self.width > 0
            && self.height > 0
            && self.x <= width
            && self.y <= height
            && self.width <= width - self.x
            && self.height <= height - self.y
    }

    /// Return true when this rectangle fully contains another rectangle.
    pub const fn contains(self, other: Self) -> bool {
        other.width > 0
            && other.height > 0
            && other.x >= self.x
            && other.y >= self.y
            && other.x - self.x <= self.width
            && other.y - self.y <= self.height
            && other.width <= self.width - (other.x - self.x)
            && other.height <= self.height - (other.y - self.y)
    }
}

/// Display aspect ratio, independent of the stored raster dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoAspectRatio {
    /// Aspect-ratio numerator.
    pub numerator: u32,
    /// Aspect-ratio denominator.
    pub denominator: u32,
}

/// Hardware raster timing associated with a completed frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoRasterMetadata {
    /// Total horizontal count positions in one line.
    pub dots_per_line: u32,
    /// Total raster lines in one frame.
    pub lines_per_frame: u32,
    /// Master-clock ticks per dot, numerator.
    pub master_ticks_per_dot_numerator: u32,
    /// Master-clock ticks per dot, denominator.
    pub master_ticks_per_dot_denominator: u32,
}

/// Immutable geometry and timing hints for presenting a completed frame.
///
/// active_rect identifies the console's active image inside the stored frame.
/// visible_rect identifies the video-producing area retained for presentation;
/// blanking/sync intervals need not be stored as pixels. masked_regions can
/// describe hardware-masked areas (for example, the SMS leftmost 8-pixel column)
/// without modifying the raw frame. The frontend may use these hints for a
/// viewport crop, but must never feed that crop back into emulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrameMetadata {
    /// Active image rectangle within the stored frame.
    pub active_rect: VideoRect,
    /// Visible/overscan rectangle within the stored frame.
    pub visible_rect: VideoRect,
    /// Intended display aspect ratio, such as 4:3.
    pub display_aspect_ratio: VideoAspectRatio,
    /// Machine timestamp in the producer's master-clock ticks.
    pub timestamp_ticks: u128,
    /// Frequency of the timestamp timebase; zero means unspecified.
    pub timebase_hz: u64,
    /// Hardware-masked regions suitable for presentation hints.
    pub masked_regions: Vec<VideoRect>,
    /// Optional source raster timing.
    pub raster: Option<VideoRasterMetadata>,
}

impl VideoFrameMetadata {
    /// Create metadata for a frame whose entire image is active and visible.
    pub fn full_frame(width: u32, height: u32) -> Self {
        Self {
            active_rect: VideoRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            visible_rect: VideoRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            display_aspect_ratio: VideoAspectRatio {
                numerator: width,
                denominator: height,
            },
            timestamp_ticks: 0,
            timebase_hz: 0,
            masked_regions: Vec::new(),
            raster: None,
        }
    }

    fn validate(&self, width: u32, height: u32) -> Result<(), VideoFrameError> {
        if !self.active_rect.is_within(width, height) {
            return Err(VideoFrameError::InvalidActiveRect);
        }
        if !self.visible_rect.is_within(width, height) {
            return Err(VideoFrameError::InvalidVisibleRect);
        }
        if !self.visible_rect.contains(self.active_rect) {
            return Err(VideoFrameError::ActiveAreaOutsideVisibleArea);
        }
        if self.display_aspect_ratio.numerator == 0 || self.display_aspect_ratio.denominator == 0
        {
            return Err(VideoFrameError::InvalidAspectRatio);
        }
        if self.timestamp_ticks > 0 && self.timebase_hz == 0 {
            return Err(VideoFrameError::InvalidTimestamp);
        }
        if self
            .masked_regions
            .iter()
            .any(|region| !region.is_within(width, height) || !self.visible_rect.contains(*region))
        {
            return Err(VideoFrameError::InvalidMaskedRegion);
        }
        if let Some(raster) = self.raster {
            if raster.dots_per_line == 0
                || raster.lines_per_frame == 0
                || raster.master_ticks_per_dot_numerator == 0
                || raster.master_ticks_per_dot_denominator == 0
            {
                return Err(VideoFrameError::InvalidRasterMetadata);
            }
        }
        Ok(())
    }
}

/// Validation errors for immutable video-frame construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFrameError {
    /// Width or height is zero.
    EmptyDimensions,
    /// Computing the row or complete buffer size overflowed usize.
    DimensionOverflow,
    /// The supplied stride is smaller than the active pixel row.
    StrideTooSmall {
        /// Minimum bytes required for one active pixel row.
        minimum: usize,
        /// Actual bytes between the starts of consecutive rows.
        actual: usize,
    },
    /// The supplied storage length does not match stride multiplied by height.
    BufferLengthMismatch {
        /// Required number of bytes.
        expected: usize,
        /// Supplied number of bytes.
        actual: usize,
    },
    /// Active image rectangle is empty or outside the frame.
    InvalidActiveRect,
    /// Visible/overscan rectangle is empty or outside the frame.
    InvalidVisibleRect,
    /// Active image rectangle is not contained by the visible rectangle.
    ActiveAreaOutsideVisibleArea,
    /// Display aspect ratio contains a zero numerator or denominator.
    InvalidAspectRatio,
    /// A non-zero timestamp has no timebase frequency.
    InvalidTimestamp,
    /// A masked/presentation region is outside the frame.
    InvalidMaskedRegion,
    /// Raster timing contains a zero dimension or clock ratio.
    InvalidRasterMetadata,
}

impl fmt::Display for VideoFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDimensions => write!(formatter, "video frame dimensions must be non-zero"),
            Self::DimensionOverflow => write!(formatter, "video frame dimensions overflow usize"),
            Self::StrideTooSmall { minimum, actual } => write!(
                formatter,
                "video frame stride {actual} is smaller than active row size {minimum}"
            ),
            Self::BufferLengthMismatch { expected, actual } => write!(
                formatter,
                "video frame storage has {actual} bytes; expected exactly {expected}"
            ),
            Self::InvalidActiveRect => write!(formatter, "video frame active rectangle is invalid"),
            Self::InvalidVisibleRect => {
                write!(formatter, "video frame visible rectangle is invalid")
            }
            Self::ActiveAreaOutsideVisibleArea => write!(
                formatter,
                "video frame active area is outside the visible area"
            ),
            Self::InvalidAspectRatio => {
                write!(formatter, "video frame display aspect ratio is invalid")
            }
            Self::InvalidTimestamp => write!(
                formatter,
                "video frame timestamp requires a non-zero timebase"
            ),
            Self::InvalidMaskedRegion => {
                write!(formatter, "video frame masked region is outside the frame")
            }
            Self::InvalidRasterMetadata => {
                write!(formatter, "video frame raster metadata is invalid")
            }
        }
    }
}

impl std::error::Error for VideoFrameError {}

/// A completed frame with immutable, reference-counted pixel storage.
///
/// stride_bytes may exceed the active row size to represent aligned upload
/// rows. Padding bytes are retained in storage but are never exposed by row.
/// The frame owns its storage and can safely be shared between the emulation
/// thread and a presentation thread without allowing either consumer to
/// mutate the pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    sequence: u64,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    stride_bytes: usize,
    row_bytes: usize,
    metadata: VideoFrameMetadata,
    pixels: Arc<[u8]>,
}

impl VideoFrame {
    /// Construct a frame, validating dimensions, stride, and exact storage size.
    ///
    /// The input vector is consumed. The resulting pixel storage is immutable
    /// and can be cheaply cloned through its Arc reference count.
    pub fn new(
        sequence: u64,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        stride_bytes: usize,
        pixels: Vec<u8>,
    ) -> Result<Self, VideoFrameError> {
        let metadata = VideoFrameMetadata::full_frame(width, height);
        Self::new_with_metadata(
            sequence,
            width,
            height,
            pixel_format,
            stride_bytes,
            pixels,
            metadata,
        )
    }

    /// Construct a frame with explicit active/visible geometry and machine timing.
    pub fn new_with_metadata(
        sequence: u64,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        stride_bytes: usize,
        pixels: Vec<u8>,
        metadata: VideoFrameMetadata,
    ) -> Result<Self, VideoFrameError> {
        if width == 0 || height == 0 {
            return Err(VideoFrameError::EmptyDimensions);
        }

        let row_bytes = (width as usize)
            .checked_mul(pixel_format.bytes_per_pixel())
            .ok_or(VideoFrameError::DimensionOverflow)?;

        if stride_bytes < row_bytes {
            return Err(VideoFrameError::StrideTooSmall {
                minimum: row_bytes,
                actual: stride_bytes,
            });
        }

        let expected_len = stride_bytes
            .checked_mul(height as usize)
            .ok_or(VideoFrameError::DimensionOverflow)?;

        if pixels.len() != expected_len {
            return Err(VideoFrameError::BufferLengthMismatch {
                expected: expected_len,
                actual: pixels.len(),
            });
        }

        metadata.validate(width, height)?;

        Ok(Self {
            sequence,
            width,
            height,
            pixel_format,
            stride_bytes,
            row_bytes,
            metadata,
            pixels: Arc::from(pixels.into_boxed_slice()),
        })
    }

    /// Return the frame sequence supplied by the producer.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Return the active image width in pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Return the active image height in pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Return the stored pixel encoding.
    pub const fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    /// Return the byte distance between the starts of adjacent rows.
    pub const fn stride_bytes(&self) -> usize {
        self.stride_bytes
    }

    /// Return the number of active pixel bytes in one row, excluding padding.
    pub const fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// Borrow immutable frame geometry and timing metadata.
    pub const fn metadata(&self) -> &VideoFrameMetadata {
        &self.metadata
    }

    /// Borrow all immutable pixel storage, including any row padding.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Borrow one active pixel row, excluding alignment padding.
    pub fn row(&self, y: u32) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }

        let start = (y as usize) * self.stride_bytes;
        Some(&self.pixels[start..start + self.row_bytes])
    }

    /// Borrow one complete storage row, including alignment padding.
    pub fn storage_row(&self, y: u32) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }

        let start = (y as usize) * self.stride_bytes;
        Some(&self.pixels[start..start + self.stride_bytes])
    }

    /// Return the strong-reference count for diagnostics and tests.
    ///
    /// This value must not be used for synchronization.
    pub fn storage_reference_count(&self) -> usize {
        Arc::strong_count(&self.pixels)
    }
}

/// Errors returned by the bounded presentation mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameMailboxError {
    /// A frame sequence is not newer than the last accepted sequence.
    NonMonotonicSequence {
        /// Last sequence accepted by the mailbox.
        last: u64,
        /// Sequence supplied by the producer.
        incoming: u64,
    },
    /// The mailbox has been closed and no longer accepts frames.
    Closed,
    /// The internal mutex was poisoned by a panicking thread.
    SynchronizationPoisoned,
}

impl fmt::Display for FrameMailboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonMonotonicSequence { last, incoming } => write!(
                formatter,
                "frame sequence {incoming} is not newer than last accepted sequence {last}"
            ),
            Self::Closed => write!(formatter, "video frame mailbox is closed"),
            Self::SynchronizationPoisoned => {
                write!(formatter, "video frame mailbox synchronization is poisoned")
            }
        }
    }
}

impl std::error::Error for FrameMailboxError {}

/// Backend-independent sink for completed immutable frames.
pub trait VideoFrameSink: Send + Sync {
    /// Submit one completed frame to the presentation side.
    fn submit(&self, frame: VideoFrame) -> Result<(), FrameMailboxError>;
}

#[derive(Debug, Default)]
struct MailboxState {
    latest: Option<VideoFrame>,
    last_sequence: Option<u64>,
    replaced_frames: u64,
    closed: bool,
}

/// A bounded, single-slot mailbox for presentation.
///
/// The emulation producer never waits for the GPU. If the presentation host
/// has not consumed the previous frame, a newer completed frame replaces it.
/// This coalesces presentation work only: it does not skip machine execution,
/// alter Scheduler time, or discard emulation events. Sequence numbers remain
/// strictly monotonic across consumed and replaced frames.
#[derive(Debug, Clone, Default)]
pub struct FrameMailbox {
    state: Arc<std::sync::Mutex<MailboxState>>,
}

impl FrameMailbox {
    /// Create an empty mailbox accepting frames.
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish a completed frame, replacing an older pending presentation frame.
    ///
    /// Frames must have strictly increasing sequence numbers. A frame already
    /// consumed by the host still participates in this monotonicity check.
    pub fn publish(&self, frame: VideoFrame) -> Result<(), FrameMailboxError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameMailboxError::SynchronizationPoisoned)?;

        if state.closed {
            return Err(FrameMailboxError::Closed);
        }

        if let Some(last) = state.last_sequence
            && frame.sequence() <= last
        {
            return Err(FrameMailboxError::NonMonotonicSequence {
                last,
                incoming: frame.sequence(),
            });
        }

        if state.latest.replace(frame).is_some() {
            state.replaced_frames = state.replaced_frames.saturating_add(1);
        }

        state.last_sequence = state.latest.as_ref().map(VideoFrame::sequence);
        Ok(())
    }

    /// Take the newest pending frame, leaving the mailbox empty.
    pub fn take_latest(&self) -> Result<Option<VideoFrame>, FrameMailboxError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameMailboxError::SynchronizationPoisoned)?;
        Ok(state.latest.take())
    }

    /// Return how many pending frames were replaced before presentation.
    pub fn replaced_frames(&self) -> Result<u64, FrameMailboxError> {
        let state = self
            .state
            .lock()
            .map_err(|_| FrameMailboxError::SynchronizationPoisoned)?;
        Ok(state.replaced_frames)
    }

    /// Close the mailbox and discard any frame still pending presentation.
    ///
    /// Closing is intended for host shutdown. It does not change the last
    /// accepted sequence number or any emulated machine state.
    pub fn close(&self) -> Result<(), FrameMailboxError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FrameMailboxError::SynchronizationPoisoned)?;
        state.closed = true;
        state.latest = None;
        Ok(())
    }

    /// Return whether the mailbox has been closed.
    pub fn is_closed(&self) -> Result<bool, FrameMailboxError> {
        let state = self
            .state
            .lock()
            .map_err(|_| FrameMailboxError::SynchronizationPoisoned)?;
        Ok(state.closed)
    }
}

impl VideoFrameSink for FrameMailbox {
    fn submit(&self, frame: VideoFrame) -> Result<(), FrameMailboxError> {
        self.publish(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::{PixelFormat, VideoFrame, VideoFrameError};

    #[test]
    fn explicit_metadata_preserves_active_visible_and_timing_geometry() {
        let metadata = super::VideoFrameMetadata {
            active_rect: super::VideoRect {
                x: 13,
                y: 24,
                width: 256,
                height: 192,
            },
            visible_rect: super::VideoRect {
                x: 0,
                y: 0,
                width: 284,
                height: 240,
            },
            display_aspect_ratio: super::VideoAspectRatio {
                numerator: 4,
                denominator: 3,
            },
            timestamp_ticks: 179_208,
            timebase_hz: 10_738_635,
            masked_regions: vec![super::VideoRect {
                x: 13,
                y: 24,
                width: 8,
                height: 192,
            }],
            raster: Some(super::VideoRasterMetadata {
                dots_per_line: 342,
                lines_per_frame: 262,
                master_ticks_per_dot_numerator: 2,
                master_ticks_per_dot_denominator: 1,
            }),
        };
        let frame = VideoFrame::new_with_metadata(
            9,
            284,
            240,
            PixelFormat::Rgba8888,
            284 * 4,
            vec![0; 284 * 240 * 4],
            metadata.clone(),
        )
        .unwrap();

        assert_eq!(frame.metadata(), &metadata);
        assert_eq!(frame.metadata().active_rect.width, 256);
        assert_eq!(frame.metadata().masked_regions[0].width, 8);
        assert_eq!(frame.metadata().display_aspect_ratio.numerator, 4);
    }

    #[test]
    fn default_metadata_marks_the_entire_frame_active_and_visible() {
        let frame = VideoFrame::new(
            1,
            256,
            192,
            PixelFormat::Rgba8888,
            1024,
            vec![0; 256 * 192 * 4],
        )
        .unwrap();

        assert_eq!(
            frame.metadata().active_rect,
            super::VideoRect {
                x: 0,
                y: 0,
                width: 256,
                height: 192,
            }
        );
        assert_eq!(frame.metadata().active_rect, frame.metadata().visible_rect);
        assert_eq!(frame.metadata().display_aspect_ratio.numerator, 256);
        assert_eq!(frame.metadata().display_aspect_ratio.denominator, 192);
    }

    #[test]
    fn rejects_active_area_outside_visible_rectangle() {
        let mut metadata = super::VideoFrameMetadata::full_frame(16, 16);
        metadata.visible_rect = super::VideoRect {
            x: 0,
            y: 0,
            width: 8,
            height: 16,
        };
        assert_eq!(
            VideoFrame::new_with_metadata(
                1,
                16,
                16,
                PixelFormat::Rgba8888,
                64,
                vec![0; 16 * 16 * 4],
                metadata,
            ),
            Err(VideoFrameError::ActiveAreaOutsideVisibleArea)
        );
    }

    #[test]
    fn rejects_masked_region_outside_visible_area() {
        let mut metadata = super::VideoFrameMetadata::full_frame(16, 16);
        metadata.visible_rect = super::VideoRect {
            x: 4,
            y: 0,
            width: 8,
            height: 16,
        };
        metadata.active_rect = metadata.visible_rect;
        metadata.masked_regions.push(super::VideoRect {
            x: 0,
            y: 0,
            width: 2,
            height: 16,
        });
        assert_eq!(
            VideoFrame::new_with_metadata(
                1,
                16,
                16,
                PixelFormat::Rgba8888,
                64,
                vec![0; 16 * 16 * 4],
                metadata,
            ),
            Err(VideoFrameError::InvalidMaskedRegion)
        );
    }

    #[test]
    fn rejects_invalid_raster_clock_ratio() {
        let mut metadata = super::VideoFrameMetadata::full_frame(8, 8);
        metadata.raster = Some(super::VideoRasterMetadata {
            dots_per_line: 342,
            lines_per_frame: 262,
            master_ticks_per_dot_numerator: 2,
            master_ticks_per_dot_denominator: 0,
        });
        assert_eq!(
            VideoFrame::new_with_metadata(
                1,
                8,
                8,
                PixelFormat::Rgba8888,
                32,
                vec![0; 8 * 8 * 4],
                metadata,
            ),
            Err(VideoFrameError::InvalidRasterMetadata)
        );
    }

    #[test]
    fn accepts_tightly_packed_rgb565_frame() {
        let frame = VideoFrame::new(
            7,
            2,
            2,
            PixelFormat::Rgb565Le,
            4,
            vec![1, 2, 3, 4, 5, 6, 7, 8],
        )
        .unwrap();

        assert_eq!(frame.sequence(), 7);
        assert_eq!(frame.width(), 2);
        assert_eq!(frame.height(), 2);
        assert_eq!(frame.row_bytes(), 4);
        assert_eq!(frame.stride_bytes(), 4);
        assert_eq!(frame.row(0), Some(&[1, 2, 3, 4][..]));
        assert_eq!(frame.row(1), Some(&[5, 6, 7, 8][..]));
    }

    #[test]
    fn row_access_excludes_alignment_padding() {
        let frame = VideoFrame::new(
            3,
            1,
            2,
            PixelFormat::Rgba8888,
            8,
            vec![1, 2, 3, 4, 90, 91, 92, 93, 5, 6, 7, 8, 80, 81, 82, 83],
        )
        .unwrap();

        assert_eq!(frame.row(0), Some(&[1, 2, 3, 4][..]));
        assert_eq!(
            frame.storage_row(0),
            Some(&[1, 2, 3, 4, 90, 91, 92, 93][..])
        );
        assert_eq!(frame.row(1), Some(&[5, 6, 7, 8][..]));
        assert_eq!(frame.row(2), None);
    }

    #[test]
    fn cloned_frames_share_immutable_storage() {
        let frame =
            VideoFrame::new(1, 1, 1, PixelFormat::Rgba8888, 4, vec![10, 20, 30, 40]).unwrap();
        let clone = frame.clone();

        assert_eq!(frame.storage_reference_count(), 2);
        assert_eq!(clone.pixels(), frame.pixels());
    }

    #[test]
    fn rejects_empty_dimensions() {
        assert_eq!(
            VideoFrame::new(0, 0, 1, PixelFormat::Rgb565Le, 0, Vec::new()),
            Err(VideoFrameError::EmptyDimensions)
        );
    }

    #[test]
    fn rejects_stride_smaller_than_active_row() {
        assert_eq!(
            VideoFrame::new(0, 2, 1, PixelFormat::Rgb565Le, 3, vec![0; 3]),
            Err(VideoFrameError::StrideTooSmall {
                minimum: 4,
                actual: 3,
            })
        );
    }

    #[test]
    fn rejects_storage_length_mismatch() {
        assert_eq!(
            VideoFrame::new(0, 1, 2, PixelFormat::Rgba8888, 4, vec![0; 7]),
            Err(VideoFrameError::BufferLengthMismatch {
                expected: 8,
                actual: 7,
            })
        );
    }

    #[test]
    fn rejects_storage_size_overflow() {
        assert_eq!(
            VideoFrame::new(0, 2, 2, PixelFormat::Rgb565Le, usize::MAX, Vec::new()),
            Err(VideoFrameError::DimensionOverflow)
        );
    }

    fn test_frame(sequence: u64) -> VideoFrame {
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
    fn mailbox_returns_the_newest_pending_frame() {
        let mailbox = super::FrameMailbox::new();

        mailbox.publish(test_frame(1)).unwrap();
        mailbox.publish(test_frame(2)).unwrap();

        let frame = mailbox.take_latest().unwrap().unwrap();
        assert_eq!(frame.sequence(), 2);
        assert_eq!(mailbox.replaced_frames().unwrap(), 1);
        assert_eq!(mailbox.take_latest().unwrap(), None);
    }

    #[test]
    fn mailbox_rejects_non_monotonic_sequences_even_after_consumption() {
        let mailbox = super::FrameMailbox::new();
        mailbox.publish(test_frame(10)).unwrap();
        mailbox.take_latest().unwrap();

        assert_eq!(
            mailbox.publish(test_frame(10)),
            Err(super::FrameMailboxError::NonMonotonicSequence {
                last: 10,
                incoming: 10,
            })
        );
        assert_eq!(
            mailbox.publish(test_frame(9)),
            Err(super::FrameMailboxError::NonMonotonicSequence {
                last: 10,
                incoming: 9,
            })
        );
    }

    #[test]
    fn mailbox_closes_and_rejects_new_frames() {
        let mailbox = super::FrameMailbox::new();
        mailbox.publish(test_frame(1)).unwrap();
        mailbox.close().unwrap();

        assert!(mailbox.is_closed().unwrap());
        assert_eq!(mailbox.take_latest().unwrap(), None);
        assert_eq!(
            mailbox.publish(test_frame(2)),
            Err(super::FrameMailboxError::Closed)
        );
    }

    #[test]
    fn cloned_mailboxes_share_the_same_pending_frame() {
        let producer = super::FrameMailbox::new();
        let consumer = producer.clone();

        producer.publish(test_frame(44)).unwrap();

        assert_eq!(consumer.take_latest().unwrap().unwrap().sequence(), 44);
    }

    #[test]
    fn video_frame_sink_trait_publishes_to_mailbox() {
        let mailbox = super::FrameMailbox::new();
        let sink: &dyn super::VideoFrameSink = &mailbox;

        sink.submit(test_frame(5)).unwrap();

        assert_eq!(mailbox.take_latest().unwrap().unwrap().sequence(), 5);
    }

    #[test]
    fn pixel_format_reports_exact_bytes_per_pixel() {
        assert_eq!(PixelFormat::Rgb565Le.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Rgba8888.bytes_per_pixel(), 4);
    }
}
