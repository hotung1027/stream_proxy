use std::sync::Arc;
use clap::Parser;
use tracing::{info, warn, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use anyhow::Result;

mod engine;
mod config;
mod server;
// mod rtsp_server;  // Temporarily disabled for testing

use crate::engine::StreamEngine;
use crate::config::Config;
use crate::server::ApiServer;
// use crate::rtsp_server::{RtspServer, RtspServerConfig};  // Temporarily disabled

/// Camera Stream Proxy - High-performance camera streaming engine
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config/default.yml")]
    config: String,
    
    /// Override log level
    #[arg(short, long)]
    log_level: Option<String>,
    
    /// Disable hardware acceleration
    #[arg(long)]
    no_hardware_accel: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    init_logging(&args)?;
    
    info!("Starting Camera Stream Proxy v{}", env!("CARGO_PKG_VERSION"));
    
    // Load configuration
    let config = Config::load(&args.config).await?;
    info!("Configuration loaded from: {}", args.config);
    
    // Override hardware acceleration if specified
    let mut config = config;
    if args.no_hardware_accel {
        config.streaming.hardware_acceleration = false;
        warn!("Hardware acceleration disabled via command line");
    }
    
    // Initialize GStreamer
    gstreamer::init().map_err(|e| anyhow::anyhow!("Failed to initialize GStreamer: {}", e))?;
    info!("GStreamer initialized successfully");
    
    // Create shared configuration
    let config = Arc::new(config);
    
    // Create stream engine
    let stream_engine = Arc::new(StreamEngine::new(config.clone()).await?);
    
    // Create API server
    let api_server = ApiServer::new(config.clone(), stream_engine.clone()).await?;
    
    // Start services
    let engine_handle = tokio::spawn(async move {
        if let Err(e) = stream_engine.run().await {
            error!("Stream engine error: {}", e);
        }
    });
    
    let server_handle = tokio::spawn(async move {
        if let Err(e) = api_server.run().await {
            error!("API server error: {}", e);
        }
    });
    
    info!("All services started successfully");
    
    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = engine_handle => {
            warn!("Stream engine terminated unexpectedly");
        }
        _ = server_handle => {
            warn!("API server terminated unexpectedly");
        }
    }
    
    info!("Shutting down Camera Stream Proxy");
    Ok(())
}

/// Initialize logging based on configuration and command line arguments
fn init_logging(args: &Args) -> Result<()> {
    let log_level = args.log_level.as_deref().unwrap_or("info");
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("camera_stream_proxy={}", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    Ok(())
} 