//! Format conversion utilities for camera streams
//! 
//! This crate provides format conversion capabilities including:
//! - Video codec conversion (H.264, H.265, VP8, VP9)
//! - Resolution and framerate conversion
//! - Hardware-accelerated encoding/decoding
//! - Quality adaptation based on network conditions

use thiserror::Error;

/// Errors that can occur during format conversion
#[derive(Error, Debug)]
pub enum FormatError {
    #[error("Unsupported input format: {0}")]
    UnsupportedInputFormat(String),
    
    #[error("Unsupported output format: {0}")]
    UnsupportedOutputFormat(String),
    
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
    
    #[error("Hardware acceleration not available")]
    HardwareAccelerationUnavailable,
    
    #[error("GStreamer error: {0}")]
    GStreamerError(String),
}

/// Result type for format operations
pub type FormatResult<T> = Result<T, FormatError>;

/// Video format specification
#[derive(Debug, Clone, PartialEq)]
pub struct VideoFormat {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Codec name (e.g., "h264", "h265", "vp8")
    pub codec: String,
    /// Bitrate in bits per second
    pub bitrate: Option<u32>,
}

/// Format converter trait
pub trait FormatConverter {
    /// Convert from one format to another
    fn convert(&self, input: &VideoFormat, output: &VideoFormat) -> FormatResult<()>;
    
    /// Check if hardware acceleration is available
    fn has_hardware_acceleration(&self) -> bool;
}

/// Placeholder format converter implementation
pub struct StreamFormatConverter {
    /// Whether hardware acceleration is enabled
    hardware_accel: bool,
}

impl StreamFormatConverter {
    /// Create a new format converter
    pub fn new(hardware_accel: bool) -> Self {
        Self { hardware_accel }
    }
    
    /// Create a new format converter with hardware acceleration detection
    pub fn with_auto_detection() -> Self {
        // TODO: Implement hardware acceleration detection
        Self::new(false)
    }
}

impl FormatConverter for StreamFormatConverter {
    fn convert(&self, input: &VideoFormat, output: &VideoFormat) -> FormatResult<()> {
        // TODO: Implement actual format conversion using GStreamer
        tracing::info!("Converting from {:?} to {:?}", input, output);
        Ok(())
    }
    
    fn has_hardware_acceleration(&self) -> bool {
        self.hardware_accel
    }
}

impl Default for StreamFormatConverter {
    fn default() -> Self {
        Self::with_auto_detection()
    }
} 