//! Example: Multiple camera streaming with RTSP and ROS integration
//!
//! This example demonstrates:
//! - Auto-detecting multiple USB cameras
//! - Streaming via RTSP (MediaMTX-style)
//! - Publishing to ROS topics
//! - Synchronized multi-camera publishing

use std::sync::Arc;
use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Import from our camera_stream_proxy crate
use camera_stream_proxy::core::{
    engine::{StreamEngine, sources::SourceId},
    config::Config,
    rtsp_server::{RtspServer, RtspServerConfig},
};

use camera_stream_proxy::ros::{
    RosPublisher,
    RosPublisherConfig,
    MultiCameraSync,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("Starting multi-camera ROS streaming example");
    
    // Initialize GStreamer
    gstreamer::init()?;
    
    // Load configuration
    let config = Arc::new(Config::load("config/default.yml").await?);
    
    // Create stream engine
    let stream_engine = Arc::new(StreamEngine::new(config.clone()).await?);
    
    // Start the stream engine
    let engine_clone = stream_engine.clone();
    tokio::spawn(async move {
        if let Err(e) = engine_clone.run().await {
            error!("Stream engine error: {}", e);
        }
    });
    
    // Wait for camera detection
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // List detected cameras
    let sources = stream_engine.list_sources().await?;
    info!("Detected {} cameras", sources.len());
    
    if sources.is_empty() {
        error!("No cameras detected!");
        return Ok(());
    }
    
    // Start streaming from all cameras
    for source in &sources {
        info!("Starting stream for camera: {} ({})", source.name, source.device_path);
        stream_engine.start_stream(source.id).await?;
    }
    
    // Create and start RTSP server (MediaMTX-style)
    let rtsp_config = RtspServerConfig {
        host: "0.0.0.0".to_string(),
        port: 8554,
        auth_enabled: false,
        max_clients: 100,
        protocols: vec!["rtsp".to_string(), "rtsps".to_string()],
    };
    
    let mut rtsp_server = RtspServer::new(rtsp_config, stream_engine.clone()).await?;
    rtsp_server.start().await?;
    
    info!("RTSP server started on port 8554");
    for source in &sources {
        let url = rtsp_server.get_stream_url(source.id);
        info!("Camera {} available at: {}", source.name, url);
    }
    
    // Create ROS publisher
    let ros_config = RosPublisherConfig {
        node_name: "multi_camera_stream".to_string(),
        namespace: "/cameras".to_string(),
        publish_raw: true,
        publish_compressed: true,
        queue_size: 10,
        frame_id_prefix: "camera".to_string(),
    };
    
    let ros_publisher = Arc::new(
        RosPublisher::new(ros_config, stream_engine.stream_cache.clone()).await?
    );
    
    // Start publishing for each camera
    for source in &sources {
        ros_publisher.start_publishing(source.id).await?;
        info!("Started ROS publishing for camera: {}", source.name);
        info!("  Raw topic: /cameras/source_{}/image_raw", source.id.0);
        info!("  Compressed topic: /cameras/source_{}/compressed", source.id.0);
    }
    
    // Create multi-camera synchronizer for stereo/multi-camera setups
    let mut sync = MultiCameraSync::new(10); // 10ms sync tolerance
    sync.add_publisher(ros_publisher.clone());
    
    // If we have multiple cameras, demonstrate synchronized publishing
    if sources.len() >= 2 {
        info!("Starting synchronized publishing for first 2 cameras");
        
        let sync_sources: Vec<SourceId> = sources.iter()
            .take(2)
            .map(|s| s.id)
            .collect();
        
        // Publish synchronized frames in a separate task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(33));
            loop {
                interval.tick().await;
                if let Err(e) = sync.publish_synchronized(sync_sources.clone()).await {
                    error!("Sync publishing error: {}", e);
                }
            }
        });
    }
    
    // Print usage information
    println!("\n=== Camera Streaming Active ===");
    println!("\nRTSP Streams:");
    for source in &sources {
        println!("  vlc rtsp://localhost:8554/stream/{}", source.id.0);
    }
    
    println!("\nROS Topics:");
    for source in &sources {
        println!("  Camera {} ({}):", source.name, source.device_path);
        println!("    Raw: rostopic echo /cameras/source_{}/image_raw", source.id.0);
        println!("    Compressed: rostopic echo /cameras/source_{}/compressed", source.id.0);
    }
    
    println!("\nVisualize in RViz:");
    println!("  rosrun rviz rviz");
    println!("  Add Image displays for each camera topic");
    
    println!("\nPress Ctrl+C to stop...\n");
    
    // Keep running until interrupted
    tokio::signal::ctrl_c().await?;
    
    info!("Shutting down...");
    
    // Stop all streams
    for source in &sources {
        stream_engine.stop_stream(source.id).await?;
        ros_publisher.stop_publishing(source.id).await?;
    }
    
    rtsp_server.stop().await?;
    
    info!("Shutdown complete");
    Ok(())
}

// Helper module to demonstrate MediaMTX-like features
mod mediamtx_features {
    use super::*;
    
    /// Configuration similar to MediaMTX
    pub struct MediaMTXConfig {
        pub protocols: Vec<String>,  // rtsp, rtmp, hls, webrtc
        pub encryption: bool,        // TLS/DTLS support
        pub authentication: bool,    // User auth
        pub api_enabled: bool,       // REST API
        pub metrics_enabled: bool,   // Prometheus metrics
    }
    
    /// Demonstrates MediaMTX-style multi-protocol support
    pub async fn setup_multi_protocol_server(
        stream_engine: Arc<StreamEngine>,
        config: MediaMTXConfig,
    ) -> Result<()> {
        info!("Setting up MediaMTX-style multi-protocol server");
        
        for protocol in &config.protocols {
            match protocol.as_str() {
                "rtsp" => {
                    info!("RTSP protocol enabled on port 8554");
                    // Already implemented above
                }
                "rtmp" => {
                    info!("RTMP protocol would be enabled on port 1935");
                    // TODO: Implement RTMP server
                }
                "hls" => {
                    info!("HLS protocol would be enabled on port 8888");
                    // TODO: Implement HLS server
                }
                "webrtc" => {
                    info!("WebRTC protocol would be enabled on port 8889");
                    // TODO: Implement WebRTC server
                }
                _ => {
                    error!("Unknown protocol: {}", protocol);
                }
            }
        }
        
        if config.authentication {
            info!("Authentication enabled for all protocols");
        }
        
        if config.metrics_enabled {
            info!("Prometheus metrics would be available at :9998/metrics");
        }
        
        Ok(())
    }
} 