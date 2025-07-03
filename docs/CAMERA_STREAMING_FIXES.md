# Camera Streaming and Connection Fixes

This document summarizes the fixes applied to resolve camera streaming issues reported by the user.

## Issues Reported

1. **Camera stream not showing on webpage** - The test platform was unable to display camera streams
2. **Camera connection changing** - Detection/connection only activated when test button was clicked, not persistent

## Root Causes

### 1. API Response Format Mismatch
- **Problem**: The server was returning `{success: true, data: "base64string", error: null}`
- **Expected**: The client JavaScript expected `{success: true, data: {data: "base64string", ...}, error: null}`
- **Location**: `src/core/server.rs` - `get_latest_frame` endpoint

### 2. Non-Persistent Connections
- **Problem**: Streams were only started manually via button clicks
- **Expected**: Streams should auto-start when cameras are discovered
- **Location**: `web/test-platform.html` - `discoverCameras` function

### 3. Pipeline Management Issues
- **Problem**: Created GStreamer pipeline wasn't stored in the StreamPipeline struct
- **Impact**: Stop command couldn't properly stop the pipeline
- **Location**: `src/core/engine/pipeline.rs` - `handle_start` method

### 4. Frame Format Issues
- **Problem**: Pipeline was caching H.264 encoded frames instead of displayable JPEG
- **Impact**: Web browser couldn't display the H.264 frames
- **Location**: `src/core/engine/pipeline.rs` - pipeline structure

## Fixes Applied

### 1. Fixed API Response Format
```rust
// Before
Ok(success_response(encoded))

// After
let frame_data = serde_json::json!({
    "data": encoded,
    "timestamp": frame.timestamp.to_rfc3339(),
    "sequence_number": frame.sequence_number,
    "format": frame.format,
    "resolution": {
        "width": frame.resolution.0,
        "height": frame.resolution.1
    }
});
Ok(success_response(frame_data))
```

### 2. Added Auto-Start for Discovered Cameras
```javascript
// In discoverCameras() function
if (sources.length > 0) {
    this.log('info', 'Auto-starting camera streams...');
    for (const source of sources) {
        await this.startStream(source.id);
        await new Promise(resolve => setTimeout(resolve, 200));
    }
}
```

### 3. Fixed Pipeline Storage
```rust
// Changed from
pipeline: Option<gst::Pipeline>,

// To
pipeline: Arc<Mutex<Option<gst::Pipeline>>>,

// And properly store the pipeline
{
    let mut pipeline_guard = self.pipeline.lock().await;
    *pipeline_guard = Some(pipeline.clone());
}
```

### 4. Modified Pipeline for JPEG Caching
- Moved the tee element before encoding
- Created separate branches:
  - RTSP branch: video → H.264 encoder → RTP payload → UDP sink
  - Cache branch: video → JPEG encoder → AppSink
- Updated cached frame format to "JPEG" with compressed=true

### 5. Added Reconnection Logic
```javascript
async restartStream(sourceId) {
    await this.stopStream(sourceId);
    await new Promise(resolve => setTimeout(resolve, 1000));
    await this.startStream(sourceId);
}
```

## Testing Instructions

1. **Start the Camera Stream Proxy server**:
   ```bash
   cargo run --release
   ```

2. **Start the test server**:
   ```bash
   python3 scripts/test_server.py
   ```

3. **Open the test platform**:
   - Navigate to http://localhost:8888/platform
   - Click "Discover Cameras"
   - Cameras should auto-start and display streams

4. **Verify fixes**:
   - ✓ Camera streams should display immediately after discovery
   - ✓ Streams should persist without manual intervention
   - ✓ Frame updates should occur every 100ms
   - ✓ Lost connections should auto-reconnect

## Future Improvements

1. **Add WebRTC support** for lower latency streaming
2. **Implement adaptive bitrate** based on network conditions
3. **Add stream health monitoring** with automatic recovery
4. **Support multiple video formats** (H.265, VP9, etc.)
5. **Add authentication** for stream access control 