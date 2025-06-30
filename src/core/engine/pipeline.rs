use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{info, warn, error};
use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use chrono::Utc;

use super::sources::StreamSource;
use super::cache::{StreamCache, CachedFrame};

/// Configuration for stream pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub hardware_acceleration: bool,
    pub buffer_size: usize,
}

/// Pipeline commands
#[derive(Debug)]
pub enum PipelineCommand {
    Start,
    Stop,
    GetConsumerCount { response: oneshot::Sender<usize> },
}

/// GStreamer-based stream processing pipeline
pub struct StreamPipeline {
    source: StreamSource,
    config: PipelineConfig,
    cache: Arc<StreamCache>,
    pipeline: Option<gst::Pipeline>,
    command_sender: mpsc::UnboundedSender<PipelineCommand>,
    consumer_count: Arc<RwLock<usize>>,
    sequence_number: Arc<RwLock<u64>>,
}

impl StreamPipeline {
    /// Create a new stream pipeline
    pub async fn new(
        source: StreamSource,
        config: PipelineConfig,
        cache: Arc<StreamCache>,
    ) -> Result<Self> {
        info!("Creating pipeline for source: {}", source.name);
        
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        
        let pipeline = Self {
            source,
            config,
            cache,
            pipeline: None,
            command_sender,
            consumer_count: Arc::new(RwLock::new(0)),
            sequence_number: Arc::new(RwLock::new(0)),
        };
        
        // Start command processing task
        let pipeline_clone = pipeline.clone();
        tokio::spawn(async move {
            pipeline_clone.process_commands(command_receiver).await;
        });
        
        Ok(pipeline)
    }
    
    /// Start the pipeline
    pub async fn start(&self) -> Result<()> {
        let (_response_tx, _response_rx) = oneshot::channel::<Result<(), String>>();
        self.command_sender.send(PipelineCommand::Start)?;
        Ok(())
    }
    
    /// Stop the pipeline
    pub async fn stop(&self) -> Result<()> {
        self.command_sender.send(PipelineCommand::Stop)?;
        Ok(())
    }
    
    /// Get the number of consumers
    pub async fn consumer_count(&self) -> usize {
        let (response_tx, response_rx) = oneshot::channel();
        if self.command_sender.send(PipelineCommand::GetConsumerCount { response: response_tx }).is_ok() {
            response_rx.await.unwrap_or(0)
        } else {
            0
        }
    }
    
    /// Process pipeline commands
    async fn process_commands(&self, mut command_receiver: mpsc::UnboundedReceiver<PipelineCommand>) {
        while let Some(command) = command_receiver.recv().await {
            match command {
                PipelineCommand::Start => {
                    if let Err(e) = self.handle_start().await {
                        error!("Failed to start pipeline: {}", e);
                    }
                }
                PipelineCommand::Stop => {
                    if let Err(e) = self.handle_stop().await {
                        error!("Failed to stop pipeline: {}", e);
                    }
                }
                PipelineCommand::GetConsumerCount { response } => {
                    let count = *self.consumer_count.read().await;
                    let _ = response.send(count);
                }
            }
        }
    }
    
    /// Handle start command
    async fn handle_start(&self) -> Result<()> {
        info!("Starting pipeline for source: {}", self.source.name);
        
        // Create GStreamer pipeline
        let pipeline = self.create_pipeline().await?;
        
        // Set up callbacks for frame processing
        self.setup_frame_callbacks(&pipeline).await?;
        
        // Start the pipeline
        pipeline.set_state(gst::State::Playing)?;
        
        info!("Pipeline started successfully for source: {}", self.source.name);
        Ok(())
    }
    
    /// Handle stop command
    async fn handle_stop(&self) -> Result<()> {
        info!("Stopping pipeline for source: {}", self.source.name);
        
        if let Some(pipeline) = &self.pipeline {
            pipeline.set_state(gst::State::Null)?;
        }
        
        info!("Pipeline stopped for source: {}", self.source.name);
        Ok(())
    }
    
    /// Create the GStreamer pipeline
    async fn create_pipeline(&self) -> Result<gst::Pipeline> {
        let pipeline = gst::Pipeline::new();
        pipeline.set_property("name", &format!("pipeline-{}", self.source.id.0));
        
        // Create source element based on source type
        let source_element = match self.source.source_type {
            super::sources::SourceType::UsbCamera => {
                self.create_usb_camera_source().await?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported source type: {:?}", self.source.source_type));
            }
        };
        
        // Create processing elements
        let convert = gst::ElementFactory::make("videoconvert")
            .name("convert")
            .build()?;
            
        let scale = gst::ElementFactory::make("videoscale")
            .name("scale")
            .build()?;
            
        // Create encoder based on configuration
        let encoder = if self.config.hardware_acceleration {
            self.create_hardware_encoder().await.unwrap_or_else(|_| {
                warn!("Hardware encoder not available, falling back to software encoder");
                self.create_software_encoder().unwrap()
            })
        } else {
            self.create_software_encoder()?
        };
        
        // Create tee for multiple outputs
        let tee = gst::ElementFactory::make("tee")
            .name("tee")
            .build()?;
            
        // Create queue for caching branch
        let cache_queue = gst::ElementFactory::make("queue")
            .name("cache_queue")
            .build()?;
            
        // Create appsink for frame caching
        let cache_sink = gst_app::AppSink::builder()
            .name("cache_sink")
            .build();
            
        // Create queue for RTSP branch
        let rtsp_queue = gst::ElementFactory::make("queue")
            .name("rtsp_queue")
            .build()?;
            
        // Create RTSP elements
        let rtp_pay = gst::ElementFactory::make("rtph264pay")
            .name("rtp_pay")
            .build()?;
            
        let rtsp_sink = gst::ElementFactory::make("udpsink")
            .name("rtsp_sink")
            .property("host", "127.0.0.1")
            .property("port", 5000 + self.source.id.0 as i32)
            .build()?;
        
        // Add elements to pipeline
        pipeline.add_many(&[
            &source_element,
            &convert,
            &scale,
            &encoder,
            &tee,
            &cache_queue,
            cache_sink.upcast_ref(),
            &rtsp_queue,
            &rtp_pay,
            &rtsp_sink,
        ])?;
        
        // Link main pipeline
        source_element.link(&convert)?;
        convert.link(&scale)?;
        scale.link(&encoder)?;
        encoder.link(&tee)?;
        
        // Link caching branch
        tee.link(&cache_queue)?;
        cache_queue.link(&cache_sink)?;
        
        // Link RTSP branch
        tee.link(&rtsp_queue)?;
        rtsp_queue.link(&rtp_pay)?;
        rtp_pay.link(&rtsp_sink)?;
        
        Ok(pipeline)
    }
    
    /// Create USB camera source element
    async fn create_usb_camera_source(&self) -> Result<gst::Element> {
        let source = gst::ElementFactory::make("v4l2src")
            .name("usb_source")
            .property("device", &self.source.device_path)
            .build()?;
            
        // Set up caps filter for format and resolution
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", &self.source.format)
            .field("width", self.source.resolution.0 as i32)
            .field("height", self.source.resolution.1 as i32)
            .field("framerate", gst::Fraction::new(self.source.framerate as i32, 1))
            .build();
            
        let caps_filter = gst::ElementFactory::make("capsfilter")
            .name("caps_filter")
            .property("caps", &caps)
            .build()?;
            
        // Create a bin to contain both elements
        let bin = gst::Bin::new();
        bin.set_property("name", "usb_source_bin");
        bin.add_many(&[&source, &caps_filter])?;
        source.link(&caps_filter)?;
        
        // Create ghost pad for the bin
        let src_pad = caps_filter.static_pad("src").unwrap();
        let ghost_pad = gst::GhostPad::with_target(&src_pad)?;
        bin.add_pad(&ghost_pad)?;
        
        Ok(bin.upcast())
    }
    
    /// Create hardware encoder
    async fn create_hardware_encoder(&self) -> Result<gst::Element> {
        // Try different hardware encoders in order of preference
        let encoder_names = vec![
            "nvh264enc",  // NVIDIA
            "vaapih264enc", // Intel/AMD VAAPI
            "qsvh264enc", // Intel Quick Sync
        ];
        
        for encoder_name in encoder_names {
            if let Ok(encoder) = gst::ElementFactory::make(encoder_name)
                .name("hw_encoder")
                .property("bitrate", 2000u32) // 2 Mbps
                .build()
            {
                info!("Using hardware encoder: {}", encoder_name);
                return Ok(encoder);
            }
        }
        
        Err(anyhow::anyhow!("No hardware encoder available"))
    }
    
    /// Create software encoder
    fn create_software_encoder(&self) -> Result<gst::Element> {
        let encoder = gst::ElementFactory::make("x264enc")
            .name("sw_encoder")
            .property("bitrate", 2000u32) // 2 Mbps
            .property("speed-preset", "ultrafast")
            .property("tune", "zerolatency")
            .build()?;
            
        info!("Using software encoder: x264enc");
        Ok(encoder)
    }
    
    /// Set up frame callbacks for processing
    async fn setup_frame_callbacks(&self, pipeline: &gst::Pipeline) -> Result<()> {
        let appsink = pipeline
            .by_name("appsink")
            .ok_or_else(|| anyhow::anyhow!("Failed to find appsink"))?
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| anyhow::anyhow!("Failed to cast to AppSink"))?;
        
        // Clone necessary data before moving into the callback
        let cache = self.cache.clone();
        let source_id = self.source.id;
        let source_format = self.source.format.clone();
        let source_resolution = self.source.resolution;
        
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    
                    // Get buffer data
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let data = map.as_slice();
                    
                    // Get frame metadata
                    let caps = sample.caps().ok_or(gst::FlowError::NotNegotiated)?;
                    let structure = caps.structure(0).ok_or(gst::FlowError::NotNegotiated)?;
                    
                    let _width = structure.get::<i32>("width").unwrap_or(0) as u32;
                    let _height = structure.get::<i32>("height").unwrap_or(0) as u32;
                    let _format = structure.get::<String>("format").unwrap_or_else(|_| "unknown".to_string());
                    
                    // Get frame number
                    let frame_number = buffer.offset();
                    
                    // Create cached frame
                    let frame = CachedFrame {
                        source_id,
                        sequence_number: frame_number,
                        timestamp: Utc::now(),
                        format: source_format.clone(),
                        resolution: source_resolution,
                        data: data.to_vec(),
                        compressed: false,
                    };
                    
                    // Cache the frame asynchronously
                    let cache_clone = cache.clone();
                    tokio::spawn(async move {
                        if let Err(e) = cache_clone.put_frame(frame).await {
                            warn!("Failed to cache frame: {}", e);
                        }
                    });
                    
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        
        Ok(())
    }
}

// Manual Clone implementation since gst::Pipeline doesn't implement Clone
impl Clone for StreamPipeline {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            config: self.config.clone(),
            cache: self.cache.clone(),
            pipeline: None, // Don't clone the actual pipeline
            command_sender: self.command_sender.clone(),
            consumer_count: self.consumer_count.clone(),
            sequence_number: self.sequence_number.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::sources::{SourceType, StreamSource, SourceId};
    use tempfile::TempDir;
    
    async fn create_test_pipeline() -> StreamPipeline {
        let temp_dir = TempDir::new().unwrap();
        let cache = Arc::new(StreamCache::new(1024 * 1024, temp_dir.path()).await.unwrap());
        
        let source = StreamSource {
            id: SourceId(1),
            name: "Test Camera".to_string(),
            device_path: "/dev/video0".to_string(),
            source_type: SourceType::UsbCamera,
            resolution: (640, 480),
            framerate: 30.0,
            format: "YUY2".to_string(),
            capabilities: vec!["video/x-raw".to_string()],
        };
        
        let config = PipelineConfig {
            hardware_acceleration: false,
            buffer_size: 1024 * 1024,
        };
        
        StreamPipeline::new(source, config, cache).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_pipeline_creation() {
        // Initialize GStreamer for testing
        if gst::init().is_err() {
            // Already initialized
        }
        
        let pipeline = create_test_pipeline().await;
        assert_eq!(pipeline.source.name, "Test Camera");
        assert_eq!(pipeline.consumer_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_pipeline_config() {
        let config = PipelineConfig {
            hardware_acceleration: true,
            buffer_size: 2 * 1024 * 1024,
        };
        
        assert!(config.hardware_acceleration);
        assert_eq!(config.buffer_size, 2 * 1024 * 1024);
    }
} 