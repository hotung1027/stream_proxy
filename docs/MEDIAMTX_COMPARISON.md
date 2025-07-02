# Camera Stream Proxy vs MediaMTX Comparison

## Overview

Both Camera Stream Proxy and MediaMTX are media servers designed to handle real-time streaming, but they have different focuses and capabilities.

## Feature Comparison

| Feature               | MediaMTX    | Camera Stream Proxy | Notes                                       |
| --------------------- | ----------- | ------------------- | ------------------------------------------- |
| **Protocols**         |             |                     |                                             |
| RTSP Server           | ✅ Full      | ✅ Full              | Both support RTSP with authentication       |
| RTMP Server           | ✅           | 🔄 Planned           | MediaMTX has full RTMP support              |
| HLS                   | ✅           | 🔄 Planned           | MediaMTX supports LL-HLS                    |
| WebRTC                | ✅ WHIP/WHEP | 🔄 Planned           | MediaMTX has WebRTC support                 |
| SRT                   | ✅           | ❌                   | MediaMTX supports SRT protocol              |
| **Camera Support**    |             |                     |                                             |
| USB Cameras           | ✅           | ✅ Enhanced          | We have better USB camera auto-detection    |
| IP Cameras            | ✅           | ✅                   | Both support RTSP cameras                   |
| SDK Cameras           | ❌           | ✅                   | We support vendor SDKs (RealSense, Basler)  |
| **Unique Features**   |             |                     |                                             |
| Frame Caching         | ❌           | ✅                   | We have built-in LRU cache with compression |
| ROS Integration       | ❌           | ✅                   | Direct publishing to ROS topics             |
| Multi-camera Sync     | Limited     | ✅                   | Synchronized multi-camera publishing        |
| Hardware Acceleration | ✅           | ✅                   | Both support NVENC, VAAPI, etc.             |

## Architecture Differences

### MediaMTX
- **Language**: Go
- **Architecture**: Monolithic with modular components
- **Focus**: Protocol compatibility and standards compliance
- **Strengths**: 
  - Wide protocol support
  - Production-ready
  - Excellent documentation
  - Active community

### Camera Stream Proxy
- **Language**: Rust (core) + Go/Python (config)
- **Architecture**: Microservices with clear separation
- **Focus**: Low-latency robotics and multi-camera applications
- **Strengths**:
  - Built-in caching for resilience
  - ROS integration for robotics
  - Multi-camera synchronization
  - Modular design for extensibility

## Use Case Comparison

### When to Use MediaMTX
- General-purpose media streaming
- Need for multiple protocols (RTMP, HLS, WebRTC)
- Standard streaming scenarios
- Quick deployment without customization

### When to Use Camera Stream Proxy
- Robotics applications requiring ROS
- Multiple USB cameras with synchronization
- Need for frame caching and buffering
- Custom camera SDK integration
- Low-latency requirements with resilience

## Implementation Example

### MediaMTX Configuration
```yaml
paths:
  cam1:
    source: rtsp://192.168.1.100:554/stream
  cam2:
    source: /dev/video0
```

### Camera Stream Proxy Configuration
```yaml
sources:
  usb:
    auto_detect: true
    scan_interval: "30s"
  
streaming:
  buffer_size: "100MB"
  hardware_acceleration: true
  
cache:
  memory_size: "500MB"
  compression: true
  
ros:
  enabled: true
  namespace: "/cameras"
  publish_raw: true
  publish_compressed: true
```

## Performance Comparison

### Latency
- **MediaMTX**: ~50-100ms typical
- **Camera Stream Proxy**: <50ms with caching

### Throughput
- **MediaMTX**: Excellent, handles 100+ streams
- **Camera Stream Proxy**: 50+ streams with caching overhead

### Resource Usage
- **MediaMTX**: Lower memory usage
- **Camera Stream Proxy**: Higher due to caching, but more resilient

## Integration Examples

### Accessing Streams

#### MediaMTX
```bash
# RTSP
ffplay rtsp://localhost:8554/cam1

# RTMP  
ffplay rtmp://localhost:1935/cam1

# HLS
ffplay http://localhost:8888/cam1/index.m3u8
```

#### Camera Stream Proxy
```bash
# RTSP with caching
ffplay rtsp://localhost:8554/stream/1

# ROS Topics
rostopic echo /cameras/source_1/image_raw
rostopic echo /cameras/source_1/compressed

# REST API for latest frame
curl http://localhost:8080/api/v1/rtsp/1/latest
```

## Feature Roadmap

### What We're Adding from MediaMTX
1. **RTMP Support**: For streaming platforms
2. **HLS Support**: For web browser compatibility  
3. **WebRTC**: For ultra-low latency browser streaming
4. **Metrics**: Prometheus-compatible metrics

### Our Unique Additions
1. **Advanced Caching**: Distributed cache support
2. **ROS2 Support**: Next-gen robotics framework
3. **AI Integration**: On-stream ML processing
4. **Multi-site Sync**: WAN synchronization

## Migration Guide

### From MediaMTX to Camera Stream Proxy

1. **Configuration Migration**:
   - MediaMTX uses single YAML for all paths
   - We use modular configuration with hot-reload

2. **API Differences**:
   - MediaMTX: `GET /v2/paths/list`
   - Ours: `GET /api/v1/sources`

3. **Stream URLs**:
   - MediaMTX: `rtsp://host:8554/pathname`
   - Ours: `rtsp://host:8554/stream/{id}`

## Conclusion

Choose **MediaMTX** if you need:
- Proven production stability
- Multiple streaming protocols
- Simple configuration
- Wide ecosystem support

Choose **Camera Stream Proxy** if you need:
- ROS integration for robotics
- Multiple USB camera management
- Built-in caching and resilience
- Custom camera SDK support
- Low-latency with buffering 