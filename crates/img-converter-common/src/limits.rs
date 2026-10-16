use serde::{Deserialize, Serialize};

/// Resource constraints to protect the system against malicious, corrupted, or bomb files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodeLimits {
    /// Maximum allowed image width in pixels. Default: 16,384.
    pub max_width: u32,
    /// Maximum allowed image height in pixels. Default: 16,384.
    pub max_height: u32,
    /// Maximum total pixel count (width * height). Default: 268,435,456 (16k x 16k).
    pub max_pixels: u64,
    /// Maximum uncompressed raw pixel memory allocation in bytes. Default: 512 MB.
    pub max_memory_bytes: u64,
    /// Maximum input file size on disk in bytes. Default: 100 MB.
    pub max_input_file_size: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_width: 16_384,
            max_height: 16_384,
            max_pixels: 16_384 * 16_384,
            max_memory_bytes: 512 * 1024 * 1024,    // 512 MB
            max_input_file_size: 100 * 1024 * 1024, // 100 MB
        }
    }
}

impl DecodeLimits {
    /// Validates dimensions before allocating memory.
    pub fn check_dimensions(&self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err("Image dimensions cannot be zero".to_string());
        }
        if width > self.max_width {
            return Err(format!(
                "Image width {} exceeds maximum allowed width of {}",
                width, self.max_width
            ));
        }
        if height > self.max_height {
            return Err(format!(
                "Image height {} exceeds maximum allowed height of {}",
                height, self.max_height
            ));
        }
        let total_pixels = (width as u64) * (height as u64);
        if total_pixels > self.max_pixels {
            return Err(format!(
                "Total pixel count {} exceeds maximum allowed pixel count of {}",
                total_pixels, self.max_pixels
            ));
        }
        Ok(())
    }

    /// Validates raw byte buffer allocation size.
    pub fn check_memory_bytes(&self, bytes: u64) -> Result<(), String> {
        if bytes > self.max_memory_bytes {
            return Err(format!(
                "Required memory allocation ({} MB) exceeds limit ({} MB)",
                bytes / (1024 * 1024),
                self.max_memory_bytes / (1024 * 1024)
            ));
        }
        Ok(())
    }

    /// Validates file size before reading into memory.
    pub fn check_file_size(&self, file_size: u64) -> Result<(), String> {
        if file_size > self.max_input_file_size {
            return Err(format!(
                "Input file size ({} MB) exceeds maximum allowed size ({} MB)",
                file_size / (1024 * 1024),
                self.max_input_file_size / (1024 * 1024)
            ));
        }
        Ok(())
    }
}
