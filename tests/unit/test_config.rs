use camera_stream_proxy::core::config::Config;
use tempfile::NamedTempFile;
use std::io::Write;

#[tokio::test]
async fn test_config_loading() {
    let config_content = r#"
server:
  host: "127.0.0.1"
  port: 8080
  api_version: "v1"

streaming:
  buffer_size: "100MB"
  max_concurrent_streams: 50
  hardware_acceleration: true

sources:
  usb:
    auto_detect: true
    scan_interval: "30s"

cache:
  memory_size: "500MB"
  disk_path: "./cache"

outputs:
  web:
    enabled: true
    port: 8081

logging:
  level: "info"
  format: "json"
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    
    let config = Config::load(temp_file.path()).await.unwrap();
    
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.streaming.max_concurrent_streams, 50);
    assert!(config.streaming.hardware_acceleration);
    assert!(config.sources.usb.auto_detect);
    assert_eq!(config.cache.disk_path, "./cache");
}

#[tokio::test]
async fn test_config_buffer_size_parsing() {
    let config_content = r#"
server:
  host: "127.0.0.1"
  port: 8080
  api_version: "v1"

streaming:
  buffer_size: "256MB"
  max_concurrent_streams: 50
  hardware_acceleration: true

sources:
  usb:
    auto_detect: true
    scan_interval: "30s"

cache:
  memory_size: "1GB"
  disk_path: "./cache"

outputs:
  web:
    enabled: true
    port: 8081

logging:
  level: "info"
  format: "json"
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    
    let config = Config::load(temp_file.path()).await.unwrap();
    
    assert_eq!(config.parse_buffer_size().unwrap(), 256 * 1024 * 1024);
    assert_eq!(config.parse_cache_size().unwrap(), 1024 * 1024 * 1024);
}

#[tokio::test]
async fn test_config_validation() {
    let invalid_config_content = r#"
server:
  host: "127.0.0.1"
  port: 0
  api_version: "v1"

streaming:
  buffer_size: "100MB"
  max_concurrent_streams: 0
  hardware_acceleration: true

sources:
  usb:
    auto_detect: true
    scan_interval: "30s"

cache:
  memory_size: "500MB"
  disk_path: "./cache"

outputs:
  web:
    enabled: true
    port: 8081

logging:
  level: "info"
  format: "json"
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(invalid_config_content.as_bytes()).unwrap();
    
    let result = Config::load(temp_file.path()).await;
    assert!(result.is_err());
} 