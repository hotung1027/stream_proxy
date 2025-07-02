# Camera Stream Proxy - Web Test Platform

This directory contains web-based testing tools for the Camera Stream Proxy system.

## Overview

We provide two comprehensive web interfaces for testing and monitoring the camera streaming system:

1. **Test Dashboard** (`test-dashboard.html`) - Full-featured dashboard with tabs for streams, tests, logs, and configuration
2. **Test Platform** (`test-platform.html`) - Streamlined testing interface with MediaMTX comparison

## Features

### 🎮 System Controls
- **Live Status Monitoring**: Real-time system health, active cameras, cache usage
- **Camera Discovery**: Auto-detect and list all available cameras
- **Stream Control**: Start/stop individual or all streams
- **Cache Management**: Monitor and clear cache

### 📹 Live Stream Viewing
- **Real-time Preview**: See live frames from each camera (via cached frames)
- **Stream Information**: Resolution, framerate, device path
- **RTSP URLs**: Direct RTSP stream URLs for VLC/ffplay
- **Snapshot Capture**: Take snapshots from any camera

### 🧪 Testing Suite
- **Comprehensive Tests**: System health, latency, throughput, multi-camera
- **Performance Metrics**: Measure stream latency and cache performance
- **ROS Integration Tests**: Verify ROS topic publishing
- **MediaMTX Comparison**: Side-by-side feature comparison

### 📊 Monitoring
- **System Logs**: Real-time log viewer with color-coded entries
- **Performance Stats**: CPU usage, memory, frame rates
- **Test Results**: Clear pass/fail indicators for all tests

## Quick Start

### Method 1: Using the Test Server (Recommended)

1. Install Python dependencies:
```bash
pip install -r requirements.txt
```

2. Start the camera stream proxy:
```bash
./target/debug/stream-engine --config config/default.yml
```

3. Start the test server:
```bash
python3 scripts/test_server.py
```

4. Open your browser to:
- Dashboard: http://localhost:8888/
- Platform: http://localhost:8888/platform

### Method 2: Direct File Access

Simply open the HTML files directly in your browser:
```bash
# Linux/Mac
firefox web/test-dashboard.html
# or
google-chrome web/test-platform.html

# Windows
start web/test-dashboard.html
```

Note: When opening files directly, you may need to update the API URL in the JavaScript code from `http://localhost:8080` to your actual API endpoint.

## Usage Guide

### Testing Workflow

1. **Check System Status**
   - Look at the status cards at the top
   - Green "Online" means the system is running

2. **Discover Cameras**
   - Click "🔍 Discover Cameras"
   - Available cameras will appear in the grid

3. **Start Streaming**
   - Click "▶️ Start" on individual cameras
   - Or use "▶️ Start All Streams" for all cameras

4. **Run Tests**
   - Click "🚀 Run All Tests" for comprehensive testing
   - Or run individual tests like latency or cache performance

5. **Monitor Performance**
   - Watch real-time stats in the status bar
   - Check logs for detailed information
   - View test results in the Tests tab

### Viewing Streams

The web interface shows cached frames from the cameras. For actual RTSP streams:

```bash
# View with VLC
vlc rtsp://localhost:8554/stream/1

# View with ffplay
ffplay rtsp://localhost:8554/stream/1

# View with GStreamer
gst-launch-1.0 rtspsrc location=rtsp://localhost:8554/stream/1 ! decodebin ! autovideosink
```

## Test Descriptions

### System Health
Verifies the camera stream proxy is running and responding to requests.

### Stream Latency
Measures time from stream start to first frame availability.

### Cache Performance
Tests cache hit rate and storage efficiency.

### Multi-Camera Support
Verifies simultaneous streaming from multiple cameras.

### ROS Integration
Checks availability of ROS topics for each camera.

## Customization

### Changing API Endpoint

Edit the JavaScript in the HTML files:
```javascript
this.apiUrl = 'http://your-server:8080';  // Change this
```

### Adding Custom Tests

Add new test functions in the JavaScript:
```javascript
async testCustomFeature() {
    this.testResults.set('Custom Test', { status: 'running', message: 'Testing...' });
    // Your test logic here
    this.testResults.set('Custom Test', { status: 'pass', message: 'Success!' });
    this.updateTestResults();
}
```

## Troubleshooting

### "System Offline" Status
- Ensure the camera stream proxy is running
- Check the API URL is correct
- Verify firewall isn't blocking port 8080

### No Cameras Detected
- Check USB cameras are connected
- Verify permissions: `ls -la /dev/video*`
- Run `sudo usermod -a -G video $USER` if needed

### Frames Not Updating
- Ensure streams are started (green indicator)
- Check browser console for errors
- Verify cache is not full

### CORS Errors
Use the test server (`python3 scripts/test_server.py`) which handles CORS properly.

## Browser Compatibility

- ✅ Chrome/Chromium (Recommended)
- ✅ Firefox
- ✅ Safari
- ✅ Edge
- ⚠️ Older browsers may have issues with WebSocket support

## Development

To modify the test platform:

1. Edit the HTML files in this directory
2. No build process required - just refresh browser
3. Use browser DevTools for debugging

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Web Browser   │────▶│   Test Server    │────▶│ Camera Stream   │
│  (Test Platform)│     │  (Python Proxy)  │     │    Proxy API    │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                               │                           │
                               ▼                           ▼
                        ┌──────────────┐           ┌──────────────┐
                        │  WebSocket   │           │   Cameras    │
                        │   Updates    │           │   & Cache    │
                        └──────────────┘           └──────────────┘
```

The test platform communicates with the camera stream proxy API to control streams, monitor status, and retrieve cached frames for display. 