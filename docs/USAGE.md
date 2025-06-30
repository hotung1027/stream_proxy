# Camera Stream Proxy - Usage Guide

This guide demonstrates how to use the camera streaming system to capture USB camera streams and serve them via RTSP, REST API, and caching.

## Quick Start

### 1. Prerequisites

Ensure you have the following installed:
- Rust 1.70+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- GStreamer development libraries (`sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`)
- USB camera connected to your system

### 2. Build the System

```bash
# Build all components
./scripts/build.sh all

# Or build just the core streaming engine
./scripts/build.sh core
```

### 3. Start the Streaming Engine

```bash
# Start with default configuration
./target/debug/stream-engine --config config/default.yml

# Or with custom log level
./target/debug/stream-engine --config config/default.yml --log-level debug
```

### 4. Test the System

```bash
# Check if the server is running
curl http://localhost:8080/health

# List available camera sources
curl http://localhost:8080/api/v1/sources

# Start streaming from the first camera
curl -X POST http://localhost:8080/api/v1/streams \
  -H "Content-Type: application/json" \
  -d '{"source_id": 1}'

# List active streams
curl http://localhost:8080/api/v1/streams?active_only=true
```

## Detailed Usage

### Camera Source Management

#### Auto-Detection
The system automatically detects USB cameras on startup if `sources.usb.auto_detect` is enabled in the configuration.

#### Manual Source Discovery
```bash
# List all detected sources
curl http://localhost:8080/api/v1/sources | jq '.'

# Get specific source information
curl http://localhost:8080/api/v1/sources/1 | jq '.'
```

### Stream Control

#### Starting Streams
```bash
# Start streaming from source ID 1
curl -X POST http://localhost:8080/api/v1/streams \
  -H "Content-Type: application/json" \
  -d '{"source_id": 1}'
```

#### Monitoring Streams
```bash
# Get stream information
curl http://localhost:8080/api/v1/streams/1 | jq '.'

# Get server status with all active streams
curl http://localhost:8080/api/v1/status | jq '.'
```

#### Stopping Streams
```bash
# Stop streaming from source ID 1
curl -X DELETE http://localhost:8080/api/v1/streams/1
```

### RTSP Streaming

Once a stream is started, it's available via RTSP at:
```
rtsp://localhost:5001/stream/1  # For source ID 1
rtsp://localhost:5002/stream/2  # For source ID 2
```

#### Viewing RTSP Streams

Using VLC:
```bash
vlc rtsp://localhost:5001/stream/1
```

Using FFplay:
```bash
ffplay rtsp://localhost:5001/stream/1
```

Using GStreamer:
```bash
gst-launch-1.0 rtspsrc location=rtsp://localhost:5001/stream/1 ! decodebin ! videoconvert ! autovideosink
```

### Cache Management

#### Cache Statistics
```bash
# Get cache performance metrics
curl http://localhost:8080/api/v1/cache/stats | jq '.'
```

#### Cache Clearing
```bash
# Clear cache for specific source
curl -X DELETE http://localhost:8080/api/v1/cache/clear/1

# Clear entire cache (restart required)
sudo systemctl restart camera-stream-proxy
```

#### Latest Frame Access
```bash
# Get the latest cached frame (base64 encoded)
curl http://localhost:8080/api/v1/rtsp/1/latest
```

## Configuration

### Basic Configuration

Edit `config/default.yml`:

```yaml
server:
  host: "0.0.0.0"  # Listen on all interfaces
  port: 8080
  api_version: "v1"

streaming:
  buffer_size: "100MB"
  max_concurrent_streams: 10
  hardware_acceleration: true  # Use GPU encoding if available

sources:
  usb:
    auto_detect: true
    scan_interval: "30s"

cache:
  memory_size: "500MB"
  disk_path: "./cache"
```

### Performance Tuning

#### For High-Performance Systems
```yaml
streaming:
  buffer_size: "1GB"
  max_concurrent_streams: 50
  hardware_acceleration: true

cache:
  memory_size: "2GB"
  disk_path: "/tmp/camera_cache"
```

#### For Resource-Constrained Systems
```yaml
streaming:
  buffer_size: "50MB"
  max_concurrent_streams: 5
  hardware_acceleration: false

cache:
  memory_size: "100MB"
  disk_path: "./cache"
```

## Web Interface Integration

### HTML5 Video Streaming

```html
<!DOCTYPE html>
<html>
<head>
    <title>Camera Stream Viewer</title>
</head>
<body>
    <video id="camera-stream" controls autoplay muted>
        <source src="http://localhost:8080/api/v1/stream/1.m3u8" type="application/x-mpegURL">
        Your browser does not support the video tag.
    </video>
    
    <script>
        // JavaScript to control streams
        async function startStream(sourceId) {
            const response = await fetch('/api/v1/streams', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ source_id: sourceId })
            });
            return response.json();
        }
        
        async function stopStream(sourceId) {
            const response = await fetch(`/api/v1/streams/${sourceId}`, {
                method: 'DELETE'
            });
            return response.json();
        }
    </script>
</body>
</html>
```

## Recording Streams

### Continuous Recording

```bash
# Record RTSP stream to file
ffmpeg -i rtsp://localhost:5001/stream/1 \
  -c copy \
  -f segment \
  -segment_time 300 \
  -segment_format mp4 \
  recordings/camera1_%03d.mp4
```

### Event-Based Recording

```bash
# Record when motion is detected (requires motion detection)
ffmpeg -i rtsp://localhost:5001/stream/1 \
  -vf "select='gt(scene,0.3)'" \
  -c:v libx264 \
  recordings/motion_$(date +%Y%m%d_%H%M%S).mp4
```

## ROS Integration

### Publishing Camera Topics

```python
#!/usr/bin/env python3
import rospy
import requests
import cv2
import numpy as np
from sensor_msgs.msg import Image, CompressedImage
from cv_bridge import CvBridge

class CameraStreamPublisher:
    def __init__(self):
        rospy.init_node('camera_stream_publisher')
        self.bridge = CvBridge()
        
        # Publishers
        self.image_pub = rospy.Publisher('/camera/image_raw', Image, queue_size=1)
        self.compressed_pub = rospy.Publisher('/camera/image_raw/compressed', CompressedImage, queue_size=1)
        
        # Camera stream URL
        self.stream_url = 'rtsp://localhost:5001/stream/1'
        
    def publish_frames(self):
        cap = cv2.VideoCapture(self.stream_url)
        
        while not rospy.is_shutdown():
            ret, frame = cap.read()
            if ret:
                # Publish uncompressed
                img_msg = self.bridge.cv2_to_imgmsg(frame, "bgr8")
                img_msg.header.stamp = rospy.Time.now()
                self.image_pub.publish(img_msg)
                
                # Publish compressed
                compressed_msg = CompressedImage()
                compressed_msg.header.stamp = rospy.Time.now()
                compressed_msg.format = "jpeg"
                compressed_msg.data = cv2.imencode('.jpg', frame)[1].tobytes()
                self.compressed_pub.publish(compressed_msg)
                
            rospy.sleep(0.033)  # ~30 FPS

if __name__ == '__main__':
    try:
        publisher = CameraStreamPublisher()
        publisher.publish_frames()
    except rospy.ROSInterruptException:
        pass
```

## Troubleshooting

### Common Issues

#### No USB Cameras Detected
```bash
# Check if cameras are visible to the system
ls /dev/video*

# Test camera with GStreamer
gst-launch-1.0 v4l2src device=/dev/video0 ! videoconvert ! autovideosink

# Check permissions
sudo usermod -a -G video $USER
```

#### RTSP Stream Not Accessible
```bash
# Check if port is open
netstat -tulpn | grep :5001

# Test RTSP stream directly
gst-launch-1.0 rtspsrc location=rtsp://localhost:5001/stream/1 ! fakesink
```

#### High CPU Usage
```bash
# Enable hardware acceleration in config
hardware_acceleration: true

# Reduce concurrent streams
max_concurrent_streams: 5

# Lower resolution/framerate in camera settings
```

#### Memory Issues
```bash
# Reduce cache size
cache:
  memory_size: "100MB"

# Clear cache periodically
curl -X DELETE http://localhost:8080/api/v1/cache/clear/1
```

### Performance Monitoring

```bash
# Monitor system resources
htop

# Monitor network usage
iftop

# Check GStreamer pipeline performance
GST_DEBUG=3 ./target/debug/stream-engine --config config/default.yml
```

## API Reference

### Endpoints

- `GET /health` - Health check
- `GET /api/v1/status` - Server status and statistics
- `GET /api/v1/sources` - List all camera sources
- `GET /api/v1/sources/:id` - Get specific source info
- `GET /api/v1/streams` - List all streams
- `POST /api/v1/streams` - Start a new stream
- `GET /api/v1/streams/:id` - Get stream information
- `DELETE /api/v1/streams/:id` - Stop a stream
- `GET /api/v1/cache/stats` - Cache statistics
- `DELETE /api/v1/cache/clear/:source_id` - Clear cache for source

### Response Format

All API responses follow this format:
```json
{
  "success": true,
  "data": { ... },
  "error": null
}
```

For errors:
```json
{
  "success": false,
  "data": null,
  "error": "Error description"
}
``` 