use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{info, warn, error};
use anyhow::Result;

use crate::config::Config;
use crate::engine::{StreamEngine, StreamInfo};
use crate::engine::sources::SourceId;
use crate::engine::cache::CacheStats;

/// API server for managing camera streams
pub struct ApiServer {
    config: Arc<Config>,
    engine: Arc<StreamEngine>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

/// Request to start a stream
#[derive(Debug, Deserialize)]
struct StartStreamRequest {
    source_id: u32,
}

/// Request to stop a stream
#[derive(Debug, Deserialize)]
struct StopStreamRequest {
    source_id: u32,
}

/// Query parameters for listing streams
#[derive(Debug, Deserialize)]
struct ListStreamsQuery {
    active_only: Option<bool>,
}

/// Stream information for API response
#[derive(Debug, Serialize)]
struct ApiStreamInfo {
    source_id: u32,
    source_name: String,
    resolution: (u32, u32),
    framerate: f64,
    format: String,
    is_active: bool,
    consumer_count: usize,
    rtsp_url: String,
}

/// Source information for API response
#[derive(Debug, Serialize)]
struct ApiSourceInfo {
    id: u32,
    name: String,
    device_path: String,
    source_type: String,
    resolution: (u32, u32),
    framerate: f64,
    format: String,
    capabilities: Vec<String>,
}

/// Server status information
#[derive(Debug, Serialize)]
struct ServerStatus {
    version: String,
    uptime: u64,
    active_streams: usize,
    total_sources: usize,
    cache_stats: CacheStats,
}

impl ApiServer {
    /// Create a new API server
    pub async fn new(config: Arc<Config>, engine: Arc<StreamEngine>) -> Result<Self> {
        info!("Initializing API server");
        
        Ok(Self {
            config,
            engine,
        })
    }
    
    /// Start the API server
    pub async fn run(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.server.host, self.config.server.port);
        info!("Starting API server on {}", addr);
        
        let app = self.create_router().await;
        
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
    
    /// Create the axum router with all endpoints
    async fn create_router(&self) -> Router {
        let state = AppState {
            config: self.config.clone(),
            engine: self.engine.clone(),
            start_time: std::time::Instant::now(),
        };
        
        Router::new()
            // Health check endpoint
            .route("/health", get(health_check))
            
            // Server status endpoint
            .route("/api/v1/status", get(get_server_status))
            
            // Source management endpoints
            .route("/api/v1/sources", get(list_sources))
            .route("/api/v1/sources/:id", get(get_source))
            
            // Stream management endpoints
            .route("/api/v1/streams", get(list_streams))
            .route("/api/v1/streams", post(start_stream))
            .route("/api/v1/streams/:id", get(get_stream))
            .route("/api/v1/streams/:id", delete(stop_stream))
            
            // Cache management endpoints
            .route("/api/v1/cache/stats", get(get_cache_stats))
            .route("/api/v1/cache/clear/:source_id", delete(clear_cache))
            
            // RTSP stream endpoints
            .route("/api/v1/rtsp/:source_id/latest", get(get_latest_frame))
            
            // Apply middleware
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    .layer(tower_http::trace::TraceLayer::new_for_http())
            )
            .with_state(state)
    }
}

/// Application state passed to handlers
#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    engine: Arc<StreamEngine>,
    start_time: std::time::Instant,
}

/// Helper function to create success response
fn success_response<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        success: true,
        data: Some(data),
        error: None,
    })
}

/// Helper function to create error response
fn error_response(error: String) -> Json<ApiResponse<()>> {
    Json(ApiResponse {
        success: false,
        data: None,
        error: Some(error),
    })
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Get server status
async fn get_server_status(State(state): State<AppState>) -> Json<ApiResponse<ServerStatus>> {
    let uptime = state.start_time.elapsed().as_secs();
    
    let sources = match state.engine.list_sources().await {
        Ok(sources) => sources,
        Err(e) => {
            warn!("Failed to list sources: {}", e);
            Vec::new()
        }
    };
    
    // Count active streams
    let mut active_streams = 0;
    for source in &sources {
        if let Ok(Some(_)) = state.engine.get_stream_info(source.id).await {
            active_streams += 1;
        }
    }
    
    // Get cache stats
    let cache_stats = state.engine.stream_cache.get_stats().await;
    
    let status = ServerStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime,
        active_streams,
        total_sources: sources.len(),
        cache_stats,
    };
    
    success_response(status)
}

/// List all available sources
async fn list_sources(State(state): State<AppState>) -> Json<ApiResponse<Vec<ApiSourceInfo>>> {
    match state.engine.list_sources().await {
        Ok(sources) => {
            let api_sources: Vec<ApiSourceInfo> = sources.into_iter().map(|source| {
                ApiSourceInfo {
                    id: source.id.0,
                    name: source.name,
                    device_path: source.device_path,
                    source_type: format!("{:?}", source.source_type),
                    resolution: source.resolution,
                    framerate: source.framerate,
                    format: source.format,
                    capabilities: source.capabilities,
                }
            }).collect();
            success_response(api_sources)
        }
        Err(e) => {
            error!("Failed to list sources: {}", e);
            error_response(format!("Failed to list sources: {}", e))
        }
    }
}

/// Get a specific source by ID
async fn get_source(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<ApiSourceInfo>>, StatusCode> {
    let source_id = SourceId(id);
    
    match state.engine.list_sources().await {
        Ok(sources) => {
            if let Some(source) = sources.into_iter().find(|s| s.id == source_id) {
                let api_source = ApiSourceInfo {
                    id: source.id.0,
                    name: source.name,
                    device_path: source.device_path,
                    source_type: format!("{:?}", source.source_type),
                    resolution: source.resolution,
                    framerate: source.framerate,
                    format: source.format,
                    capabilities: source.capabilities,
                };
                Ok(success_response(api_source))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(e) => {
            error!("Failed to get source: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List all streams
async fn list_streams(
    Query(query): Query<ListStreamsQuery>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ApiStreamInfo>>> {
    match state.engine.list_sources().await {
        Ok(sources) => {
            let mut streams = Vec::new();
            
            for source in sources {
                let stream_info = state.engine.get_stream_info(source.id).await;
                
                match stream_info {
                    Ok(Some(info)) => {
                        if query.active_only.unwrap_or(false) && !info.is_active {
                            continue;
                        }
                        
                        let api_stream = ApiStreamInfo {
                            source_id: info.source_id.0,
                            source_name: info.source_name,
                            resolution: info.resolution,
                            framerate: info.framerate,
                            format: info.format,
                            is_active: info.is_active,
                            consumer_count: info.consumer_count,
                            rtsp_url: format!("rtsp://{}:{}/stream/{}", 
                                state.config.server.host, 
                                5000 + info.source_id.0, 
                                info.source_id.0),
                        };
                        streams.push(api_stream);
                    }
                    Ok(None) => {
                        // Source exists but no active stream
                        if !query.active_only.unwrap_or(false) {
                            let api_stream = ApiStreamInfo {
                                source_id: source.id.0,
                                source_name: source.name,
                                resolution: source.resolution,
                                framerate: source.framerate,
                                format: source.format,
                                is_active: false,
                                consumer_count: 0,
                                rtsp_url: format!("rtsp://{}:{}/stream/{}", 
                                    state.config.server.host, 
                                    5000 + source.id.0, 
                                    source.id.0),
                            };
                            streams.push(api_stream);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get stream info for source {}: {}", source.id.0, e);
                    }
                }
            }
            
            success_response(streams)
        }
        Err(e) => {
            error!("Failed to list streams: {}", e);
            error_response(format!("Failed to list streams: {}", e))
        }
    }
}

/// Start a stream
async fn start_stream(
    State(state): State<AppState>,
    Json(request): Json<StartStreamRequest>,
) -> Json<ApiResponse<String>> {
    let source_id = SourceId(request.source_id);
    
    match state.engine.start_stream(source_id).await {
        Ok(()) => {
            info!("Started stream for source: {}", request.source_id);
            success_response("Stream started successfully".to_string())
        }
        Err(e) => {
            error!("Failed to start stream: {}", e);
            error_response(format!("Failed to start stream: {}", e))
        }
    }
}

/// Get stream information
async fn get_stream(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<ApiStreamInfo>>, StatusCode> {
    let source_id = SourceId(id);
    
    match state.engine.get_stream_info(source_id).await {
        Ok(Some(info)) => {
            let api_stream = ApiStreamInfo {
                source_id: info.source_id.0,
                source_name: info.source_name,
                resolution: info.resolution,
                framerate: info.framerate,
                format: info.format,
                is_active: info.is_active,
                consumer_count: info.consumer_count,
                rtsp_url: format!("rtsp://{}:{}/stream/{}", 
                    state.config.server.host, 
                    5000 + info.source_id.0, 
                    info.source_id.0),
            };
            Ok(success_response(api_stream))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get stream info: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Stop a stream
async fn stop_stream(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> Json<ApiResponse<String>> {
    let source_id = SourceId(id);
    
    match state.engine.stop_stream(source_id).await {
        Ok(()) => {
            info!("Stopped stream for source: {}", id);
            success_response("Stream stopped successfully".to_string())
        }
        Err(e) => {
            error!("Failed to stop stream: {}", e);
            error_response(format!("Failed to stop stream: {}", e))
        }
    }
}

/// Get cache statistics
async fn get_cache_stats(State(state): State<AppState>) -> Json<ApiResponse<CacheStats>> {
    let stats = state.engine.stream_cache.get_stats().await;
    success_response(stats)
}

/// Clear cache for a specific source
async fn clear_cache(
    Path(source_id): Path<u32>,
    State(state): State<AppState>,
) -> Json<ApiResponse<String>> {
    let source_id = SourceId(source_id);
    
    match state.engine.stream_cache.clear_source(source_id).await {
        Ok(()) => {
            info!("Cleared cache for source: {}", source_id.0);
            success_response("Cache cleared successfully".to_string())
        }
        Err(e) => {
            error!("Failed to clear cache: {}", e);
            error_response(format!("Failed to clear cache: {}", e))
        }
    }
}

/// Get the latest frame for a source
async fn get_latest_frame(
    Path(source_id): Path<u32>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let source_id = SourceId(source_id);
    
    match state.engine.stream_cache.get_latest_frame(source_id).await {
        Some(frame) => {
            // Convert frame to base64 for JSON response
            let encoded = base64::encode(&frame.data);
            Ok(success_response(encoded))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use serde_json::Value;
    
    #[tokio::test]
    async fn test_health_check() {
        let app = Router::new().route("/health", get(health_check));
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/health").await;
        assert_eq!(response.status_code(), 200);
        assert_eq!(response.text(), "OK");
    }
    
    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse {
            success: true,
            data: Some("test data".to_string()),
            error: None,
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\":\"test data\""));
    }
    
    #[test]
    fn test_error_response() {
        let response = error_response("test error".to_string());
        assert!(!response.0.success);
        assert!(response.0.error.is_some());
        assert_eq!(response.0.error.unwrap(), "test error");
    }
} 