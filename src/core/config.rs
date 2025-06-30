use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;
use tokio::fs;

/// Main configuration structure for the camera stream proxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub streaming: StreamingConfig,
    pub sources: SourcesConfig,
    pub cache: CacheConfig,
    pub outputs: OutputsConfig,
    pub logging: LoggingConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub api_version: String,
}

/// Streaming engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub buffer_size: String,
    pub max_concurrent_streams: u32,
    pub hardware_acceleration: bool,
}

/// Input sources configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesConfig {
    pub usb: UsbConfig,
}

/// USB camera configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbConfig {
    pub auto_detect: bool,
    pub scan_interval: String,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub memory_size: String,
    pub disk_path: String,
}

/// Output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputsConfig {
    pub web: WebOutputConfig,
}

/// Web output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebOutputConfig {
    pub enabled: bool,
    pub port: u16,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Config {
    /// Load configuration from a YAML file
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).await
            .map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", path.display(), e))?;
        
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file {}: {}", path.display(), e))?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// Parse buffer size string to bytes
    pub fn parse_buffer_size(&self) -> Result<usize> {
        parse_size_string(&self.streaming.buffer_size)
    }
    
    /// Parse cache memory size string to bytes
    pub fn parse_cache_size(&self) -> Result<usize> {
        parse_size_string(&self.cache.memory_size)
    }
    
    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        // Validate port numbers
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("Server port cannot be 0"));
        }
        
        if self.outputs.web.enabled && self.outputs.web.port == 0 {
            return Err(anyhow::anyhow!("Web output port cannot be 0"));
        }
        
        // Validate buffer size
        self.parse_buffer_size()?;
        
        // Validate cache size
        self.parse_cache_size()?;
        
        // Validate concurrent streams limit
        if self.streaming.max_concurrent_streams == 0 {
            return Err(anyhow::anyhow!("Max concurrent streams must be greater than 0"));
        }
        
        Ok(())
    }
}

/// Parse size strings like "100MB", "1GB", etc. to bytes
fn parse_size_string(size_str: &str) -> Result<usize> {
    let size_str = size_str.trim().to_uppercase();
    
    if let Some(num_str) = size_str.strip_suffix("KB") {
        let num: f64 = num_str.parse()?;
        Ok((num * 1024.0) as usize)
    } else if let Some(num_str) = size_str.strip_suffix("MB") {
        let num: f64 = num_str.parse()?;
        Ok((num * 1024.0 * 1024.0) as usize)
    } else if let Some(num_str) = size_str.strip_suffix("GB") {
        let num: f64 = num_str.parse()?;
        Ok((num * 1024.0 * 1024.0 * 1024.0) as usize)
    } else if let Some(num_str) = size_str.strip_suffix("B") {
        Ok(num_str.parse()?)
    } else {
        // Assume bytes if no suffix
        Ok(size_str.parse()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_size_string() {
        assert_eq!(parse_size_string("1024").unwrap(), 1024);
        assert_eq!(parse_size_string("1KB").unwrap(), 1024);
        assert_eq!(parse_size_string("1MB").unwrap(), 1024 * 1024);
        assert_eq!(parse_size_string("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size_string("100B").unwrap(), 100);
    }
    
    #[test]
    fn test_parse_size_string_case_insensitive() {
        assert_eq!(parse_size_string("1mb").unwrap(), 1024 * 1024);
        assert_eq!(parse_size_string("1Mb").unwrap(), 1024 * 1024);
        assert_eq!(parse_size_string("1MB").unwrap(), 1024 * 1024);
    }
} 