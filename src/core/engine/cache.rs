use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::sync::{RwLock, Mutex};
use tokio::fs;
use tracing::{info, warn, debug};
use anyhow::Result;
use chrono::{DateTime, Utc};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Serialize, Deserialize};

use super::sources::SourceId;

/// A cached frame with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFrame {
    /// The actual frame data
    pub data: Vec<u8>,
    
    /// When this frame was cached
    pub timestamp: DateTime<Utc>,
    
    /// Source ID that produced this frame
    pub source_id: SourceId,
    
    /// Frame sequence number
    pub sequence_number: u64,
    
    /// Original frame format
    pub format: String,
    
    /// Resolution of the frame
    pub resolution: (u32, u32),
    
    /// Whether the data is compressed
    pub compressed: bool,
}

/// Cache key for identifying frames
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub source_id: SourceId,
    pub sequence_number: u64,
}

/// LRU cache node for tracking access order
#[derive(Debug)]
struct CacheNode {
    key: CacheKey,
    frame: CachedFrame,
    access_count: u64,
    last_accessed: DateTime<Utc>,
}

/// Statistics for cache performance monitoring
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub total_frames: usize,
    pub memory_usage: usize,
    pub disk_usage: usize,
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// High-performance stream cache with LRU eviction and disk persistence
pub struct StreamCache {
    /// In-memory cache with LRU ordering
    memory_cache: Arc<RwLock<HashMap<CacheKey, CacheNode>>>,
    
    /// LRU access order tracking
    access_order: Arc<Mutex<Vec<CacheKey>>>,
    
    /// Maximum memory cache size in bytes
    max_memory_size: usize,
    
    /// Current memory usage
    current_memory_usage: Arc<RwLock<usize>>,
    
    /// Disk cache directory
    disk_cache_dir: PathBuf,
    
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

impl StreamCache {
    /// Create a new stream cache
    pub async fn new<P: AsRef<Path>>(max_memory_size: usize, disk_cache_dir: P) -> Result<Self> {
        let disk_cache_dir = disk_cache_dir.as_ref().to_path_buf();
        
        // Create disk cache directory if it doesn't exist
        if !disk_cache_dir.exists() {
            fs::create_dir_all(&disk_cache_dir).await?;
        }
        
        info!("Initializing stream cache with {}MB memory limit", max_memory_size / (1024 * 1024));
        
        let cache = Self {
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            access_order: Arc::new(Mutex::new(Vec::new())),
            max_memory_size,
            current_memory_usage: Arc::new(RwLock::new(0)),
            disk_cache_dir,
            stats: Arc::new(RwLock::new(CacheStats {
                total_frames: 0,
                memory_usage: 0,
                disk_usage: 0,
                hit_rate: 0.0,
                miss_rate: 0.0,
                total_requests: 0,
                cache_hits: 0,
                cache_misses: 0,
            })),
        };
        
        // Load existing cache from disk if present
        cache.load_from_disk().await?;
        
        Ok(cache)
    }
    
    /// Store a frame in the cache
    pub async fn put_frame(&self, frame: CachedFrame) -> Result<()> {
        let key = CacheKey {
            source_id: frame.source_id,
            sequence_number: frame.sequence_number,
        };
        
        debug!("Caching frame: {:?}", key);
        
        // Compress frame data if not already compressed
        let compressed_frame = if !frame.compressed {
            self.compress_frame(frame).await?
        } else {
            frame
        };
        
        let frame_size = compressed_frame.data.len();
        
        // Check if we need to evict frames to make space
        self.ensure_capacity(frame_size).await?;
        
        // Create cache node
        let node = CacheNode {
            key: key.clone(),
            frame: compressed_frame,
            access_count: 1,
            last_accessed: Utc::now(),
        };
        
        // Store in memory cache
        {
            let mut cache = self.memory_cache.write().await;
            cache.insert(key.clone(), node);
        }
        
        // Update access order
        {
            let mut access_order = self.access_order.lock().await;
            access_order.push(key.clone());
        }
        
        // Update memory usage
        {
            let mut usage = self.current_memory_usage.write().await;
            *usage += frame_size;
        }
        
        // Update stats
        self.update_stats(|stats| {
            stats.total_frames += 1;
            stats.memory_usage += frame_size;
        }).await;
        
        // Optionally persist to disk for important frames
        self.persist_frame_if_needed(&key).await?;
        
        Ok(())
    }
    
    /// Retrieve a frame from the cache
    pub async fn get_frame(&self, source_id: SourceId, sequence_number: u64) -> Option<CachedFrame> {
        let key = CacheKey { source_id, sequence_number };
        
        // Update stats
        self.update_stats(|stats| {
            stats.total_requests += 1;
        }).await;
        
        // Try memory cache first
        if let Some(frame) = self.get_from_memory(&key).await {
            self.update_stats(|stats| {
                stats.cache_hits += 1;
                stats.hit_rate = stats.cache_hits as f64 / stats.total_requests as f64;
                stats.miss_rate = 1.0 - stats.hit_rate;
            }).await;
            
            return Some(frame);
        }
        
        // Try disk cache
        if let Some(frame) = self.get_from_disk(&key).await {
            // Promote back to memory cache
            if let Err(e) = self.put_frame(frame.clone()).await {
                warn!("Failed to promote frame from disk to memory: {}", e);
            }
            
            self.update_stats(|stats| {
                stats.cache_hits += 1;
                stats.hit_rate = stats.cache_hits as f64 / stats.total_requests as f64;
                stats.miss_rate = 1.0 - stats.hit_rate;
            }).await;
            
            return Some(frame);
        }
        
        // Cache miss
        self.update_stats(|stats| {
            stats.cache_misses += 1;
            stats.hit_rate = stats.cache_hits as f64 / stats.total_requests as f64;
            stats.miss_rate = 1.0 - stats.hit_rate;
        }).await;
        
        debug!("Cache miss for frame: {:?}", key);
        None
    }
    
    /// Get the latest frame for a source
    pub async fn get_latest_frame(&self, source_id: SourceId) -> Option<CachedFrame> {
        let cache = self.memory_cache.read().await;
        
        let mut latest_frame: Option<&CacheNode> = None;
        let mut latest_sequence = 0;
        
        for node in cache.values() {
            if node.frame.source_id == source_id && node.frame.sequence_number > latest_sequence {
                latest_sequence = node.frame.sequence_number;
                latest_frame = Some(node);
            }
        }
        
        if let Some(node) = latest_frame {
            Some(self.decompress_frame(node.frame.clone()).await.unwrap_or(node.frame.clone()))
        } else {
            None
        }
    }
    
    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }
    
    /// Clear cache for a specific source
    pub async fn clear_source(&self, source_id: SourceId) -> Result<()> {
        info!("Clearing cache for source: {:?}", source_id);
        
        // Remove from memory cache
        let keys_to_remove: Vec<CacheKey> = {
            let cache = self.memory_cache.read().await;
            cache.keys()
                .filter(|key| key.source_id == source_id)
                .cloned()
                .collect()
        };
        
        for key in keys_to_remove {
            self.remove_frame(&key).await?;
        }
        
        // Remove from disk cache
        self.remove_disk_files(source_id).await?;
        
        Ok(())
    }
    
    /// Get frame from memory cache
    async fn get_from_memory(&self, key: &CacheKey) -> Option<CachedFrame> {
        let mut cache = self.memory_cache.write().await;
        
        if let Some(node) = cache.get_mut(key) {
            // Update access information
            node.access_count += 1;
            node.last_accessed = Utc::now();
            
            // Update access order
            self.update_access_order(key).await;
            
            let frame = node.frame.clone();
            return Some(self.decompress_frame(frame).await.unwrap_or(node.frame.clone()));
        }
        
        None
    }
    
    /// Get frame from disk cache
    async fn get_from_disk(&self, key: &CacheKey) -> Option<CachedFrame> {
        let file_path = self.get_disk_path(key);
        
        match fs::read(&file_path).await {
            Ok(data) => {
                match bincode::deserialize::<CachedFrame>(&data) {
                    Ok(frame) => Some(frame),
                    Err(e) => {
                        warn!("Failed to deserialize cached frame from {}: {}", file_path.display(), e);
                        None
                    }
                }
            }
            Err(_) => None, // File doesn't exist or can't be read
        }
    }
    
    /// Compress frame data
    async fn compress_frame(&self, mut frame: CachedFrame) -> Result<CachedFrame> {
        let compressed_data = compress_prepend_size(&frame.data);
        frame.data = compressed_data;
        frame.compressed = true;
        Ok(frame)
    }
    
    /// Decompress frame data
    async fn decompress_frame(&self, mut frame: CachedFrame) -> Result<CachedFrame> {
        if frame.compressed {
            let decompressed_data = decompress_size_prepended(&frame.data)
                .map_err(|e| anyhow::anyhow!("Decompression failed: {}", e))?;
            frame.data = decompressed_data;
            frame.compressed = false;
        }
        Ok(frame)
    }
    
    /// Ensure there's enough capacity for a new frame
    async fn ensure_capacity(&self, required_size: usize) -> Result<()> {
        let current_usage = *self.current_memory_usage.read().await;
        
        if current_usage + required_size <= self.max_memory_size {
            return Ok(()); // Enough space available
        }
        
        debug!("Cache capacity exceeded, evicting frames");
        
        // Evict least recently used frames
        let mut evicted_size = 0;
        while current_usage + required_size - evicted_size > self.max_memory_size {
            if let Some(key) = self.get_lru_key().await {
                let frame_size = self.get_frame_size(&key).await;
                self.remove_frame(&key).await?;
                evicted_size += frame_size;
            } else {
                break; // No more frames to evict
            }
        }
        
        Ok(())
    }
    
    /// Get the least recently used cache key
    async fn get_lru_key(&self) -> Option<CacheKey> {
        let access_order = self.access_order.lock().await;
        access_order.first().cloned()
    }
    
    /// Get the size of a cached frame
    async fn get_frame_size(&self, key: &CacheKey) -> usize {
        let cache = self.memory_cache.read().await;
        cache.get(key).map(|node| node.frame.data.len()).unwrap_or(0)
    }
    
    /// Remove a frame from the cache
    async fn remove_frame(&self, key: &CacheKey) -> Result<()> {
        let frame_size = self.get_frame_size(key).await;
        
        // Remove from memory cache
        {
            let mut cache = self.memory_cache.write().await;
            cache.remove(key);
        }
        
        // Remove from access order
        {
            let mut access_order = self.access_order.lock().await;
            access_order.retain(|k| k != key);
        }
        
        // Update memory usage
        {
            let mut usage = self.current_memory_usage.write().await;
            *usage = usage.saturating_sub(frame_size);
        }
        
        // Update stats
        self.update_stats(|stats| {
            stats.total_frames = stats.total_frames.saturating_sub(1);
            stats.memory_usage = stats.memory_usage.saturating_sub(frame_size);
        }).await;
        
        Ok(())
    }
    
    /// Update access order for LRU tracking
    async fn update_access_order(&self, key: &CacheKey) {
        let mut access_order = self.access_order.lock().await;
        
        // Remove from current position
        access_order.retain(|k| k != key);
        
        // Add to end (most recently used)
        access_order.push(key.clone());
    }
    
    /// Get disk file path for a cache key
    fn get_disk_path(&self, key: &CacheKey) -> PathBuf {
        let filename = format!("{}_{}.cache", key.source_id.0, key.sequence_number);
        self.disk_cache_dir.join(filename)
    }
    
    /// Persist frame to disk if needed
    async fn persist_frame_if_needed(&self, key: &CacheKey) -> Result<()> {
        // For now, persist every 10th frame to reduce disk I/O
        if key.sequence_number % 10 == 0 {
            self.persist_frame_to_disk(key).await?;
        }
        Ok(())
    }
    
    /// Persist a frame to disk
    async fn persist_frame_to_disk(&self, key: &CacheKey) -> Result<()> {
        let cache = self.memory_cache.read().await;
        
        if let Some(node) = cache.get(key) {
            let file_path = self.get_disk_path(key);
            let data = bincode::serialize(&node.frame)?;
            fs::write(&file_path, data).await?;
        }
        
        Ok(())
    }
    
    /// Load cache from disk on startup
    async fn load_from_disk(&self) -> Result<()> {
        if !self.disk_cache_dir.exists() {
            return Ok(());
        }
        
        let mut entries = fs::read_dir(&self.disk_cache_dir).await?;
        let mut loaded_count = 0;
        
        while let Some(entry) = entries.next_entry().await? {
            if let Some(extension) = entry.path().extension() {
                if extension == "cache" {
                    if let Err(e) = self.load_frame_from_disk(&entry.path()).await {
                        warn!("Failed to load cached frame from {}: {}", entry.path().display(), e);
                    } else {
                        loaded_count += 1;
                    }
                }
            }
        }
        
        if loaded_count > 0 {
            info!("Loaded {} cached frames from disk", loaded_count);
        }
        
        Ok(())
    }
    
    /// Load a single frame from disk
    async fn load_frame_from_disk(&self, path: &Path) -> Result<()> {
        let data = fs::read(path).await?;
        let frame: CachedFrame = bincode::deserialize(&data)?;
        self.put_frame(frame).await?;
        Ok(())
    }
    
    /// Remove disk files for a source
    async fn remove_disk_files(&self, source_id: SourceId) -> Result<()> {
        let mut entries = fs::read_dir(&self.disk_cache_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            if file_name_str.starts_with(&format!("{}_", source_id.0)) {
                if let Err(e) = fs::remove_file(entry.path()).await {
                    warn!("Failed to remove cached file {}: {}", entry.path().display(), e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Update cache statistics
    async fn update_stats<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut CacheStats),
    {
        let mut stats = self.stats.write().await;
        update_fn(&mut *stats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn create_test_cache() -> (StreamCache, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = StreamCache::new(1024 * 1024, temp_dir.path()).await.unwrap(); // 1MB cache
        (cache, temp_dir)
    }
    
    fn create_test_frame(source_id: u32, sequence: u64) -> CachedFrame {
        CachedFrame {
            source_id: SourceId(source_id),
            sequence_number: sequence,
            timestamp: Utc::now(),
            format: "YUY2".to_string(),
            resolution: (640, 480),
            data: vec![0u8; 1024], // 1KB test data
            compressed: false,
        }
    }
    
    #[tokio::test]
    async fn test_cache_put_get() {
        let (cache, _temp_dir) = create_test_cache().await;
        
        let frame = create_test_frame(1, 1);
        let source_id = frame.source_id;
        let sequence = frame.sequence_number;
        
        // Put frame in cache
        cache.put_frame(frame.clone()).await.unwrap();
        
        // Get frame from cache
        let retrieved = cache.get_frame(source_id, sequence).await;
        assert!(retrieved.is_some());
        
        let retrieved_frame = retrieved.unwrap();
        assert_eq!(retrieved_frame.source_id, source_id);
        assert_eq!(retrieved_frame.sequence_number, sequence);
    }
    
    #[tokio::test]
    async fn test_cache_latest_frame() {
        let (cache, _temp_dir) = create_test_cache().await;
        
        let source_id = SourceId(1);
        
        // Add multiple frames
        for i in 1..=5 {
            let frame = create_test_frame(1, i);
            cache.put_frame(frame).await.unwrap();
        }
        
        // Get latest frame
        let latest = cache.get_latest_frame(source_id).await;
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().sequence_number, 5);
    }
    
    #[tokio::test]
    async fn test_cache_stats() {
        let (cache, _temp_dir) = create_test_cache().await;
        
        let frame = create_test_frame(1, 1);
        cache.put_frame(frame).await.unwrap();
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.total_frames, 1);
        assert!(stats.memory_usage > 0);
    }
} 