//! Backend-independent immutable video-frame contracts.
//!
//! This crate is intentionally headless and platform-neutral. It contains no
//! emulator timing, Python bindings, window-system handles, or graphics API
//! ownership. Native presentation backends consume completed frames through
//! this contract.

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

        Ok(Self {
            sequence,
            width,
            height,
            pixel_format,
            stride_bytes,
            row_bytes,
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

#[cfg(test)]
mod tests {
    use super::{PixelFormat, VideoFrame, VideoFrameError};

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
        assert_eq!(frame.storage_row(0), Some(&[1, 2, 3, 4, 90, 91, 92, 93][..]));
        assert_eq!(frame.row(1), Some(&[5, 6, 7, 8][..]));
        assert_eq!(frame.row(2), None);
    }

    #[test]
    fn cloned_frames_share_immutable_storage() {
        let frame = VideoFrame::new(
            1,
            1,
            1,
            PixelFormat::Rgba8888,
            4,
            vec![10, 20, 30, 40],
        )
        .unwrap();
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
    fn pixel_format_reports_exact_bytes_per_pixel() {
        assert_eq!(PixelFormat::Rgb565Le.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Rgba8888.bytes_per_pixel(), 4);
    }
}
