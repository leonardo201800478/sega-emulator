//! Immutable cartridge image; bytes are never silently truncated.

use crate::SmsCoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeImage {
    bytes: Vec<u8>,
}

impl CartridgeImage {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Result<Self, SmsCoreError> {
        let bytes = bytes.into();
        if bytes.is_empty() {
            sega_observability::tracing::error!(
                target: "sega::rom",
                format = "raw cartridge image",
                size_bytes = 0,
                "empty cartridge image rejected"
            );
            return Err(SmsCoreError::EmptyCartridge);
        }
        sega_observability::tracing::info!(
            target: "sega::rom",
            format = "raw cartridge image",
            size_bytes = bytes.len(),
            "cartridge image accepted"
        );
        Ok(Self { bytes })
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::CartridgeImage;
    use crate::SmsCoreError;

    #[test]
    fn preserves_every_byte_of_large_images() {
        let mut bytes = vec![0x5A; 8 * 1024 * 1024 + 17];
        bytes[0] = 0x11;
        *bytes.last_mut().unwrap() = 0xE7;
        let image = CartridgeImage::new(bytes.clone()).unwrap();

        assert_eq!(image.len(), 8 * 1024 * 1024 + 17);
        assert_eq!(image.bytes(), bytes.as_slice());
        assert_eq!(image.bytes()[0], 0x11);
        assert_eq!(image.bytes()[image.len() - 1], 0xE7);
    }

    #[test]
    fn rejects_empty_image() {
        assert_eq!(
            CartridgeImage::new(Vec::new()).unwrap_err(),
            SmsCoreError::EmptyCartridge
        );
    }
}
