use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};
use anyhow::Result;
use chrono::Utc;

use crate::engine::sources::SourceId;
use crate::engine::cache::{StreamCache, CachedFrame};

// ROS message types (we'll use rosrust or r2r for actual implementation)
#[derive(Debug, Clone)]
pub struct ImageMsg {
    pub header: Header,
    pub height: u32,
    pub width: u32,
    pub encoding: String,
    pub is_bigendian: u8,
    pub step: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CompressedImageMsg {
    pub header: Header,
    pub format: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub seq: u32,
    pub stamp: Time,
    pub frame_id: String,
}

#[derive(Debug, Clone)]
pub struct Time {
    pub sec: u32,
    pub nsec: u32,
}

/// ROS publisher configuration
#[derive(Debug, Clone)]
pub struct RosPublisherConfig {
    pub node_name: String,
    pub namespace: String,
    pub publish_raw: bool,
    pub publish_compressed: bool,
    pub queue_size: usize,
    pub frame_id_prefix: String,
}

impl Default for RosPublisherConfig {
    fn default() -> Self {
        Self {
            node_name: "camera_stream_proxy".to_string(),
            namespace: "/camera".to_string(),
            publish_raw: true,
            publish_compressed: true,
            queue_size: 10,
            frame_id_prefix: "camera".to_string(),
        }
    }
}

/// Stream publisher info
#[derive(Debug, Clone)]
pub struct StreamPublisher {
    source_id: SourceId,
    raw_topic: String,
    compressed_topic: String,
    frame_count: u64,
    last_publish_time: chrono::DateTime<Utc>,
}

/// ROS publisher for camera streams
pub struct RosPublisher {
    config: RosPublisherConfig,
    cache: Arc<StreamCache>,
    publishers: Arc<RwLock<HashMap<SourceId, StreamPublisher>>>,
    command_tx: mpsc::UnboundedSender<PublisherCommand>,
}

/// Publisher commands
#[derive(Debug)]
pub enum PublisherCommand {
    StartPublishing { source_id: SourceId },
    StopPublishing { source_id: SourceId },
    PublishFrame { source_id: SourceId, frame: CachedFrame },
    Shutdown,
}

impl RosPublisher {
    /// Create a new ROS publisher instance
    pub async fn new(config: RosPublisherConfig, cache: Arc<StreamCache>) -> Result<Self> {
        info!("Creating ROS publisher with node name: {}", config.node_name);
        
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        
        let publisher = Self {
            config,
            cache,
            publishers: Arc::new(RwLock::new(HashMap::new())),
            command_tx,
        };
        
        // Start command processing task
        let publisher_clone = publisher.clone();
        tokio::spawn(async move {
            publisher_clone.process_commands(command_rx).await;
        });
        
        // Start frame polling task
        let publisher_clone = publisher.clone();
        tokio::spawn(async move {
            publisher_clone.poll_frames().await;
        });
        
        Ok(publisher)
    }
    
    /// Start publishing for a source
    pub async fn start_publishing(&self, source_id: SourceId) -> Result<()> {
        info!("Starting ROS publishing for source: {:?}", source_id);
        
        let raw_topic = format!("{}/source_{}/image_raw", self.config.namespace, source_id.0);
        let compressed_topic = format!("{}/source_{}/compressed", self.config.namespace, source_id.0);
        
        let publisher = StreamPublisher {
            source_id,
            raw_topic: raw_topic.clone(),
            compressed_topic: compressed_topic.clone(),
            frame_count: 0,
            last_publish_time: Utc::now(),
        };
        
        self.publishers.write().await.insert(source_id, publisher);
        
        info!("ROS topics created: {} and {}", raw_topic, compressed_topic);
        
        Ok(())
    }
    
    /// Stop publishing for a source
    pub async fn stop_publishing(&self, source_id: SourceId) -> Result<()> {
        info!("Stopping ROS publishing for source: {:?}", source_id);
        
        self.publishers.write().await.remove(&source_id);
        
        Ok(())
    }
    
    /// Process publisher commands
    async fn process_commands(&self, mut command_rx: mpsc::UnboundedReceiver<PublisherCommand>) {
        while let Some(command) = command_rx.recv().await {
            match command {
                PublisherCommand::StartPublishing { source_id } => {
                    if let Err(e) = self.start_publishing(source_id).await {
                        error!("Failed to start publishing: {}", e);
                    }
                }
                PublisherCommand::StopPublishing { source_id } => {
                    if let Err(e) = self.stop_publishing(source_id).await {
                        error!("Failed to stop publishing: {}", e);
                    }
                }
                PublisherCommand::PublishFrame { source_id, frame } => {
                    if let Err(e) = self.publish_frame(source_id, frame).await {
                        warn!("Failed to publish frame: {}", e);
                    }
                }
                PublisherCommand::Shutdown => {
                    info!("Shutting down ROS publisher");
                    break;
                }
            }
        }
    }
    
    /// Poll frames from cache and publish
    async fn poll_frames(&self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(33)); // ~30 FPS
        
        loop {
            interval.tick().await;
            
            let publishers = self.publishers.read().await;
            for (source_id, _) in publishers.iter() {
                // Get latest frame from cache
                if let Ok(Some(frame)) = self.cache.get_latest_frame(*source_id).await {
                    let _ = self.command_tx.send(PublisherCommand::PublishFrame {
                        source_id: *source_id,
                        frame,
                    });
                }
            }
        }
    }
    
    /// Publish a frame to ROS topics
    async fn publish_frame(&self, source_id: SourceId, frame: CachedFrame) -> Result<()> {
        let mut publishers = self.publishers.write().await;
        let publisher = publishers.get_mut(&source_id)
            .ok_or_else(|| anyhow::anyhow!("Publisher not found for source: {:?}", source_id))?;
        
        publisher.frame_count += 1;
        publisher.last_publish_time = Utc::now();
        
        // Create header
        let header = Header {
            seq: publisher.frame_count as u32,
            stamp: Time {
                sec: frame.timestamp.timestamp() as u32,
                nsec: frame.timestamp.timestamp_subsec_nanos(),
            },
            frame_id: format!("{}_{}", self.config.frame_id_prefix, source_id.0),
        };
        
        // Publish raw image if enabled
        if self.config.publish_raw {
            let raw_msg = self.create_raw_image_msg(header.clone(), &frame)?;
            self.publish_raw_image(&publisher.raw_topic, raw_msg).await?;
        }
        
        // Publish compressed image if enabled
        if self.config.publish_compressed {
            let compressed_msg = self.create_compressed_image_msg(header, &frame)?;
            self.publish_compressed_image(&publisher.compressed_topic, compressed_msg).await?;
        }
        
        debug!("Published frame {} for source {:?}", publisher.frame_count, source_id);
        
        Ok(())
    }
    
    /// Create raw image message
    fn create_raw_image_msg(&self, header: Header, frame: &CachedFrame) -> Result<ImageMsg> {
        let encoding = match frame.format.as_str() {
            "YUY2" => "yuv422",
            "RGB" => "rgb8",
            "BGR" => "bgr8",
            "GRAY8" => "mono8",
            _ => "bgr8", // Default
        };
        
        let step = match encoding {
            "yuv422" => frame.resolution.0 * 2,
            "rgb8" | "bgr8" => frame.resolution.0 * 3,
            "mono8" => frame.resolution.0,
            _ => frame.resolution.0 * 3,
        };
        
        Ok(ImageMsg {
            header,
            height: frame.resolution.1,
            width: frame.resolution.0,
            encoding: encoding.to_string(),
            is_bigendian: 0,
            step,
            data: frame.data.clone(),
        })
    }
    
    /// Create compressed image message
    fn create_compressed_image_msg(&self, header: Header, frame: &CachedFrame) -> Result<CompressedImageMsg> {
        // If frame is already compressed, use it directly
        if frame.compressed {
            return Ok(CompressedImageMsg {
                header,
                format: "jpeg".to_string(),
                data: frame.data.clone(),
            });
        }
        
        // Otherwise, compress to JPEG
        // In a real implementation, we'd use image crate or OpenCV for compression
        // For now, we'll just use the raw data
        Ok(CompressedImageMsg {
            header,
            format: "jpeg".to_string(),
            data: frame.data.clone(), // TODO: Implement actual JPEG compression
        })
    }
    
    /// Publish raw image to ROS topic
    async fn publish_raw_image(&self, topic: &str, msg: ImageMsg) -> Result<()> {
        // In a real implementation, this would use rosrust or r2r to publish
        debug!("Publishing raw image to topic: {}", topic);
        // TODO: Implement actual ROS publishing
        Ok(())
    }
    
    /// Publish compressed image to ROS topic
    async fn publish_compressed_image(&self, topic: &str, msg: CompressedImageMsg) -> Result<()> {
        // In a real implementation, this would use rosrust or r2r to publish
        debug!("Publishing compressed image to topic: {}", topic);
        // TODO: Implement actual ROS publishing
        Ok(())
    }
    
    /// Get publishing statistics
    pub async fn get_stats(&self) -> HashMap<SourceId, StreamPublisher> {
        self.publishers.read().await.clone()
    }
}

// Clone implementation for RosPublisher
impl Clone for RosPublisher {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            cache: self.cache.clone(),
            publishers: self.publishers.clone(),
            command_tx: self.command_tx.clone(),
        }
    }
}

/// Multi-camera synchronization for ROS
pub struct MultiCameraSync {
    publishers: Vec<Arc<RosPublisher>>,
    sync_tolerance_ms: u64,
}

impl MultiCameraSync {
    /// Create a new multi-camera synchronizer
    pub fn new(sync_tolerance_ms: u64) -> Self {
        Self {
            publishers: Vec::new(),
            sync_tolerance_ms,
        }
    }
    
    /// Add a publisher to sync
    pub fn add_publisher(&mut self, publisher: Arc<RosPublisher>) {
        self.publishers.push(publisher);
    }
    
    /// Publish synchronized frames from multiple cameras
    pub async fn publish_synchronized(&self, source_ids: Vec<SourceId>) -> Result<()> {
        // Get frames from all sources
        let mut frames = Vec::new();
        for (i, source_id) in source_ids.iter().enumerate() {
            if let Some(publisher) = self.publishers.get(i) {
                if let Ok(Some(frame)) = publisher.cache.get_latest_frame(*source_id).await {
                    frames.push((*source_id, frame));
                }
            }
        }
        
        // Check if all frames are within sync tolerance
        if frames.len() != source_ids.len() {
            warn!("Not all frames available for synchronization");
            return Ok(());
        }
        
        let base_time = frames[0].1.timestamp;
        for (_, frame) in &frames[1..] {
            let time_diff = (frame.timestamp - base_time).num_milliseconds().abs() as u64;
            if time_diff > self.sync_tolerance_ms {
                warn!("Frame synchronization failed, time difference: {}ms", time_diff);
                return Ok(());
            }
        }
        
        // Publish all synchronized frames
        for (i, (source_id, frame)) in frames.into_iter().enumerate() {
            if let Some(publisher) = self.publishers.get(i) {
                let _ = publisher.command_tx.send(PublisherCommand::PublishFrame {
                    source_id,
                    frame,
                });
            }
        }
        
        debug!("Published synchronized frames for {} cameras", source_ids.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ros_config() {
        let config = RosPublisherConfig::default();
        assert_eq!(config.node_name, "camera_stream_proxy");
        assert_eq!(config.namespace, "/camera");
        assert!(config.publish_raw);
        assert!(config.publish_compressed);
    }
    
    #[test]
    fn test_header_creation() {
        let now = Utc::now();
        let header = Header {
            seq: 1,
            stamp: Time {
                sec: now.timestamp() as u32,
                nsec: now.timestamp_subsec_nanos(),
            },
            frame_id: "camera_0".to_string(),
        };
        
        assert_eq!(header.seq, 1);
        assert_eq!(header.frame_id, "camera_0");
    }
} 