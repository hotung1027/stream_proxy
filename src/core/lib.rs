//! Core streaming engine for camera stream proxy
//! 
//! This crate provides the core functionality for the camera streaming system including:
//! - Stream engine orchestration
//! - Configuration management
//! - HTTP/REST API server
//! - GStreamer pipeline management

pub mod config;
pub mod engine;
pub mod server;

// Re-export commonly used types
pub use config::{Config, StreamingConfig, SourcesConfig};
pub use engine::{StreamEngine, EngineError};
pub use server::{create_app, ApiError};

use thiserror::Error;

/// Core errors that can occur in the streaming system
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),
    
    #[error("Engine error: {0}")]
    EngineError(#[from] engine::EngineError),
    
    #[error("Server error: {0}")]
    ServerError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for core operations
pub type CoreResult<T> = Result<T, CoreError>; 