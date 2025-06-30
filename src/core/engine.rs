use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, error, warn, debug};
use anyhow::Result;
use dashmap::DashMap;

use crate::config::Config;

pub mod sources;
pub mod cache;
pub mod pipeline;

use sources::{SourceManager, SourceId, StreamSource};
use cache::StreamCache;
use pipeline::{StreamPipeline, PipelineConfig};

/// Main stream engine that orchestrates all streaming operations
#[derive(Clone)]
pub struct StreamEngine {
    config: Arc<Config>,
    source_manager: Arc<SourceManager>,
    pub stream_cache: Arc<StreamCache>,
    active_pipelines: Arc<DashMap<SourceId, StreamPipeline>>,
    command_sender: mpsc::UnboundedSender<EngineCommand>,
}

/// Commands that can be sent to the stream engine
#[derive(Debug)]
pub enum EngineCommand {
    StartStream { source_id: SourceId, response: oneshot::Sender<Result<()>> },
    StopStream { source_id: SourceId, response: oneshot::Sender<Result<()>> },
    ListSources { response: oneshot::Sender<Vec<StreamSource>> },
    GetStreamInfo { source_id: SourceId, response: oneshot::Sender<Option<StreamInfo>> },
    Shutdown,
}

/// Information about an active stream
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub source_id: SourceId,
    pub source_name: String,
    pub resolution: (u32, u32),
    pub framerate: f64,
    pub format: String,
    pub is_active: bool,
    pub consumer_count: usize,
}

impl StreamEngine {
    /// Create a new stream engine instance
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        info!("Initializing stream engine");
        
        // Initialize source manager
        let source_manager = Arc::new(SourceManager::new(config.clone()).await?);
        
        // Initialize stream cache
        let cache_size = config.parse_cache_size()?;
        let stream_cache = Arc::new(StreamCache::new(cache_size, &config.cache.disk_path).await?);
        
        // Create command channel
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        
        let engine = Self {
            config,
            source_manager,
            stream_cache,
            active_pipelines: Arc::new(DashMap::new()),
            command_sender,
        };
        
        // Start command processing task
        let engine_clone = engine.clone();
        tokio::spawn(async move {
            engine_clone.process_commands(command_receiver).await;
        });
        
        info!("Stream engine initialized successfully");
        Ok(engine)
    }
    
    /// Start the stream engine
    pub async fn run(&self) -> Result<()> {
        info!("Starting stream engine");
        
        // Start source manager
        let source_manager = self.source_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = source_manager.run().await {
                error!("Source manager error: {}", e);
            }
        });
        
        // Auto-detect and start USB cameras if enabled
        if self.config.sources.usb.auto_detect {
            self.auto_detect_usb_cameras().await?;
        }
        
        info!("Stream engine started successfully");
        
        // Keep running until shutdown
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
    
    /// Send a command to the engine and wait for response
    pub async fn send_command(&self, command: EngineCommand) -> Result<()> {
        self.command_sender.send(command)
            .map_err(|_| anyhow::anyhow!("Failed to send command to engine"))?;
        Ok(())
    }
    
    /// Start streaming from a specific source
    pub async fn start_stream(&self, source_id: SourceId) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_command(EngineCommand::StartStream { source_id, response: response_tx }).await?;
        response_rx.await.map_err(|_| anyhow::anyhow!("Failed to receive response"))?
    }
    
    /// Stop streaming from a specific source
    pub async fn stop_stream(&self, source_id: SourceId) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_command(EngineCommand::StopStream { source_id, response: response_tx }).await?;
        response_rx.await.map_err(|_| anyhow::anyhow!("Failed to receive response"))?
    }
    
    /// List all available sources
    pub async fn list_sources(&self) -> Result<Vec<StreamSource>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_command(EngineCommand::ListSources { response: response_tx }).await?;
        response_rx.await.map_err(|_| anyhow::anyhow!("Failed to receive response"))
    }
    
    /// Get information about a specific stream
    pub async fn get_stream_info(&self, source_id: SourceId) -> Result<Option<StreamInfo>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_command(EngineCommand::GetStreamInfo { source_id, response: response_tx }).await?;
        response_rx.await.map_err(|_| anyhow::anyhow!("Failed to receive response"))
    }
    
    /// Auto-detect and initialize USB cameras
    async fn auto_detect_usb_cameras(&self) -> Result<()> {
        info!("Auto-detecting USB cameras");
        
        let sources = self.source_manager.detect_usb_cameras().await?;
        
        if sources.is_empty() {
            warn!("No USB cameras detected");
            return Ok(());
        }
        
        info!("Found {} USB camera(s)", sources.len());
        
        // Start streaming from all detected cameras
        for source in sources {
            if let Err(e) = self.start_stream(source.id).await {
                warn!("Failed to start stream for camera {}: {}", source.name, e);
            } else {
                info!("Started stream for camera: {}", source.name);
            }
        }
        
        Ok(())
    }
    
    /// Process incoming commands
    async fn process_commands(&self, mut command_receiver: mpsc::UnboundedReceiver<EngineCommand>) {
        while let Some(command) = command_receiver.recv().await {
            match command {
                EngineCommand::StartStream { source_id, response } => {
                    let result = self.handle_start_stream(source_id).await;
                    let _ = response.send(result);
                }
                EngineCommand::StopStream { source_id, response } => {
                    let result = self.handle_stop_stream(source_id).await;
                    let _ = response.send(result);
                }
                EngineCommand::ListSources { response } => {
                    let sources = self.source_manager.list_sources().await;
                    let _ = response.send(sources);
                }
                EngineCommand::GetStreamInfo { source_id, response } => {
                    let info = self.handle_get_stream_info(source_id).await;
                    let _ = response.send(info);
                }
                EngineCommand::Shutdown => {
                    info!("Received shutdown command");
                    break;
                }
            }
        }
    }
    
    /// Handle start stream command
    async fn handle_start_stream(&self, source_id: SourceId) -> Result<()> {
        debug!("Starting stream for source: {:?}", source_id);
        
        // Check if already active
        if self.active_pipelines.contains_key(&source_id) {
            return Ok(()); // Already running
        }
        
        // Get source information
        let source = self.source_manager.get_source(source_id).await
            .ok_or_else(|| anyhow::anyhow!("Source not found: {:?}", source_id))?;
        
        // Create pipeline configuration
        let pipeline_config = PipelineConfig {
            hardware_acceleration: self.config.streaming.hardware_acceleration,
            buffer_size: self.config.parse_buffer_size()?,
        };
        
        // Create and start pipeline
        let pipeline = StreamPipeline::new(source, pipeline_config, self.stream_cache.clone()).await?;
        
        // Start the pipeline
        pipeline.start().await?;
        
        // Store active pipeline
        self.active_pipelines.insert(source_id, pipeline);
        
        info!("Stream started successfully for source: {:?}", source_id);
        Ok(())
    }
    
    /// Handle stop stream command
    async fn handle_stop_stream(&self, source_id: SourceId) -> Result<()> {
        debug!("Stopping stream for source: {:?}", source_id);
        
        if let Some((_, pipeline)) = self.active_pipelines.remove(&source_id) {
            pipeline.stop().await?;
            info!("Stream stopped for source: {:?}", source_id);
        }
        
        Ok(())
    }
    
    /// Handle get stream info command
    async fn handle_get_stream_info(&self, source_id: SourceId) -> Option<StreamInfo> {
        if let Some(pipeline) = self.active_pipelines.get(&source_id) {
            if let Some(source) = self.source_manager.get_source(source_id).await {
                return Some(StreamInfo {
                    source_id,
                    source_name: source.name,
                    resolution: source.resolution,
                    framerate: source.framerate,
                    format: source.format,
                    is_active: true,
                    consumer_count: pipeline.consumer_count().await,
                });
            }
        }
        None
    }
} 