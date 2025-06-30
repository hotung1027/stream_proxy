//! Camera source adapters for various input types
//! 
//! This crate provides adapters for different camera source types including:
//! - USB cameras (UVC compatible)
//! - RTSP streams from IP cameras
//! - SDK-based cameras (Intel RealSense, Basler, etc.)
//! - Stereo camera systems
//! - Media file sources

use thiserror::Error;

/// Errors that can occur in camera adapters
#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Source not found: {0}")]
    SourceNotFound(String),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for adapter operations
pub type AdapterResult<T> = Result<T, AdapterError>;

/// Placeholder module for USB camera adapters
pub mod usb {
    //! USB camera adapter implementation
    
    use super::*;
    
    /// USB camera adapter (placeholder)
    pub struct UsbAdapter;
    
    impl UsbAdapter {
        /// Create a new USB adapter
        pub fn new() -> Self {
            Self
        }
    }
}

/// Placeholder module for RTSP stream adapters
pub mod rtsp {
    //! RTSP stream adapter implementation
    
    use super::*;
    
    /// RTSP stream adapter (placeholder)
    pub struct RtspAdapter;
    
    impl RtspAdapter {
        /// Create a new RTSP adapter
        pub fn new() -> Self {
            Self
        }
    }
}

/// Placeholder module for SDK camera adapters
pub mod sdk {
    //! SDK camera adapter implementation
    
    use super::*;
    
    /// SDK camera adapter (placeholder)
    pub struct SdkAdapter;
    
    impl SdkAdapter {
        /// Create a new SDK adapter
        pub fn new() -> Self {
            Self
        }
    }
} 