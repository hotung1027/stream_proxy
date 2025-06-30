use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use serde::{Serialize, Deserialize};

use crate::config::Config;

/// Unique identifier for a stream source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub u32);

/// Represents a camera or stream source
#[derive(Debug, Clone)]
pub struct StreamSource {
    pub id: SourceId,
    pub name: String,
    pub device_path: String,
    pub source_type: SourceType,
    pub resolution: (u32, u32),
    pub framerate: f64,
    pub format: String,
    pub capabilities: Vec<String>,
}

/// Type of stream source
#[derive(Debug, Clone, PartialEq)]
pub enum SourceType {
    UsbCamera,
    RtspStream,
    SdkCamera,
    MediaFile,
}

/// Manages detection and lifecycle of stream sources
pub struct SourceManager {
    config: Arc<Config>,
    sources: Arc<RwLock<HashMap<SourceId, StreamSource>>>,
    next_source_id: Arc<RwLock<u32>>,
}

impl SourceManager {
    /// Create a new source manager
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        info!("Initializing source manager");
        
        let manager = Self {
            config,
            sources: Arc::new(RwLock::new(HashMap::new())),
            next_source_id: Arc::new(RwLock::new(1)),
        };
        
        Ok(manager)
    }
    
    /// Start the source manager (periodic scanning, etc.)
    pub async fn run(&self) -> Result<()> {
        info!("Starting source manager");
        
        // Initial USB camera detection
        if self.config.sources.usb.auto_detect {
            self.scan_usb_cameras().await?;
        }
        
        // Periodic scanning if enabled
        if self.config.sources.usb.auto_detect {
            let scan_interval = parse_duration(&self.config.sources.usb.scan_interval)?;
            let mut interval = tokio::time::interval(scan_interval);
            
            loop {
                interval.tick().await;
                if let Err(e) = self.scan_usb_cameras().await {
                    warn!("Error during USB camera scan: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Detect available USB cameras
    pub async fn detect_usb_cameras(&self) -> Result<Vec<StreamSource>> {
        self.scan_usb_cameras().await?;
        
        let sources = self.sources.read().await;
        Ok(sources.values()
            .filter(|s| s.source_type == SourceType::UsbCamera)
            .cloned()
            .collect())
    }
    
    /// Get a specific source by ID
    pub async fn get_source(&self, source_id: SourceId) -> Option<StreamSource> {
        let sources = self.sources.read().await;
        sources.get(&source_id).cloned()
    }
    
    /// List all available sources
    pub async fn list_sources(&self) -> Vec<StreamSource> {
        let sources = self.sources.read().await;
        sources.values().cloned().collect()
    }
    
    /// Scan for USB cameras using GStreamer device monitor
    async fn scan_usb_cameras(&self) -> Result<()> {
        debug!("Scanning for USB cameras");
        
        // Use GStreamer device monitor to find video sources
        let device_monitor = gst::DeviceMonitor::new();
        
        // Add video source filter
        let caps = gst::Caps::builder("video/x-raw").build();
        device_monitor.add_filter(Some("Video/Source"), Some(&caps));
        
        // Start monitoring
        device_monitor.start()
            .map_err(|e| anyhow::anyhow!("Failed to start device monitor: {}", e))?;
        
        let devices = device_monitor.devices();
        device_monitor.stop();
        
        let mut new_sources = Vec::new();
        
        for device in devices {
            match self.create_source_from_device(&device).await {
                Ok(source) => {
                    new_sources.push(source);
                }
                Err(e) => {
                    warn!("Failed to create source from device: {}", e);
                }
            }
        }
        
        // Update sources map
        let mut sources = self.sources.write().await;
        
        // Remove old USB cameras that are no longer present
        sources.retain(|_, source| {
            if source.source_type == SourceType::UsbCamera {
                new_sources.iter().any(|new_source| new_source.device_path == source.device_path)
            } else {
                true // Keep non-USB sources
            }
        });
        
        // Add new sources
        for new_source in new_sources {
            if !sources.values().any(|existing| existing.device_path == new_source.device_path) {
                info!("Detected new USB camera: {}", new_source.name);
                sources.insert(new_source.id, new_source);
            }
        }
        
        debug!("USB camera scan completed. Found {} total sources", sources.len());
        Ok(())
    }
    
    /// Create a StreamSource from a GStreamer device
    async fn create_source_from_device(&self, device: &gst::Device) -> Result<StreamSource> {
        let display_name = device.display_name();
        let device_class = device.device_class();
        
        // Only process video sources
        if device_class != "Video/Source" {
            return Err(anyhow::anyhow!("Not a video source: {}", device_class));
        }
        
        // Get device path from properties
        let properties = device.properties().ok_or_else(|| {
            anyhow::anyhow!("Device has no properties")
        })?;
        
        let device_path = properties
            .get::<String>("device.path")
            .unwrap_or_else(|_err| {
                // Use a static counter for fallback IDs
                static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);
                let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                format!("/dev/video{}", id)
            });
        
        // Probe device capabilities
        let (resolution, framerate, format, capabilities) = self.probe_device_capabilities(device)?;
        
        Ok(StreamSource {
            id: SourceId(self.generate_source_id().await),
            name: display_name.to_string(),
            device_path,
            source_type: SourceType::UsbCamera,
            resolution,
            framerate,
            format,
            capabilities,
        })
    }
    
    /// Probe device capabilities to get supported formats and resolutions
    fn probe_device_capabilities(&self, device: &gst::Device) -> Result<((u32, u32), f64, String, Vec<String>)> {
        let caps = device.caps().ok_or_else(|| {
            anyhow::anyhow!("Device has no capabilities")
        })?;
        
        // Parse capabilities to extract format information
        let mut max_resolution = (640, 480);
        let mut max_framerate = 30.0;
        let mut preferred_format = "YUY2".to_string();
        let mut capabilities = Vec::new();
        
        for i in 0..caps.size() {
            if let Some(structure) = caps.structure(i) {
                let name = structure.name();
                capabilities.push(name.to_string());
                
                // Extract resolution
                if let (Ok(width), Ok(height)) = (
                    structure.get::<i32>("width"),
                    structure.get::<i32>("height")
                ) {
                    let resolution = (width as u32, height as u32);
                    if resolution.0 * resolution.1 > max_resolution.0 * max_resolution.1 {
                        max_resolution = resolution;
                    }
                }
                
                // Extract framerate
                if let Ok(framerate) = structure.get::<gst::Fraction>("framerate") {
                    let fps = framerate.numer() as f64 / framerate.denom() as f64;
                    if fps > max_framerate {
                        max_framerate = fps;
                    }
                }
                
                // Extract format
                if let Ok(format) = structure.get::<String>("format") {
                    preferred_format = format;
                }
            }
        }
        
        Ok((max_resolution, max_framerate, preferred_format, capabilities))
    }
    
    /// Generate a unique source ID
    async fn generate_source_id(&self) -> u32 {
        let mut next_id = self.next_source_id.write().await;
        let id = *next_id;
        *next_id += 1;
        id
    }
}

/// Parse duration string like "30s", "5m", etc.
fn parse_duration(duration_str: &str) -> Result<tokio::time::Duration> {
    let duration_str = duration_str.trim().to_lowercase();
    
    if let Some(num_str) = duration_str.strip_suffix("s") {
        let seconds: u64 = num_str.parse()?;
        Ok(tokio::time::Duration::from_secs(seconds))
    } else if let Some(num_str) = duration_str.strip_suffix("m") {
        let minutes: u64 = num_str.parse()?;
        Ok(tokio::time::Duration::from_secs(minutes * 60))
    } else if let Some(num_str) = duration_str.strip_suffix("h") {
        let hours: u64 = num_str.parse()?;
        Ok(tokio::time::Duration::from_secs(hours * 3600))
    } else {
        // Assume seconds if no suffix
        let seconds: u64 = duration_str.parse()?;
        Ok(tokio::time::Duration::from_secs(seconds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s").unwrap(), tokio::time::Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), tokio::time::Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), tokio::time::Duration::from_secs(3600));
        assert_eq!(parse_duration("45").unwrap(), tokio::time::Duration::from_secs(45));
    }
    
    #[test]
    fn test_source_id_equality() {
        let id1 = SourceId(1);
        let id2 = SourceId(1);
        let id3 = SourceId(2);
        
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
    
    #[test]
    fn test_source_type_equality() {
        assert_eq!(SourceType::UsbCamera, SourceType::UsbCamera);
        assert_ne!(SourceType::UsbCamera, SourceType::RtspStream);
    }
} 