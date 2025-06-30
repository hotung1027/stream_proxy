#!/bin/bash

# Camera Stream Proxy - Setup Test Script
# This script tests the basic functionality of the camera streaming system

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE="${PROJECT_ROOT}/config/development.yml"
BASE_URL="http://localhost:8080"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
log() {
    echo -e "${GREEN}[TEST]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

# Test functions
test_prerequisites() {
    log "Testing prerequisites..."
    
    # Check if cameras are available
    if ! ls /dev/video* >/dev/null 2>&1; then
        warn "No camera devices found in /dev/video*"
        warn "Please connect a USB camera or create a virtual camera for testing"
        warn "To create a virtual camera:"
        warn "  sudo modprobe v4l2loopback devices=1 video_nr=42 card_label='Virtual Camera'"
        warn "  ffmpeg -re -f lavfi -i testsrc=duration=3600:size=640x480:rate=30 -f v4l2 /dev/video42"
    else
        info "Found camera devices: $(ls /dev/video* | tr '\n' ' ')"
    fi
    
    # Check GStreamer
    if ! command -v gst-launch-1.0 &> /dev/null; then
        error "GStreamer not found. Please install GStreamer:"
        error "  sudo apt-get install gstreamer1.0-tools"
        return 1
    fi
    
    # Test basic GStreamer functionality
    if ! gst-launch-1.0 --version >/dev/null 2>&1; then
        error "GStreamer is not working correctly"
        return 1
    fi
    
    info "GStreamer is available"
    
    # Check if curl is available for API testing
    if ! command -v curl &> /dev/null; then
        error "curl is required for API testing. Please install curl."
        return 1
    fi
    
    log "Prerequisites check passed"
}

test_build() {
    log "Testing build..."
    
    cd "$PROJECT_ROOT"
    
    # Check if binary exists
    if [ ! -f "target/debug/stream-engine" ]; then
        log "Building stream engine..."
        if ! ./scripts/build.sh core; then
            error "Failed to build stream engine"
            return 1
        fi
    fi
    
    log "Build test passed"
}

start_server() {
    log "Starting stream engine..."
    
    cd "$PROJECT_ROOT"
    
    # Start the server in background
    ./target/debug/stream-engine --config "$CONFIG_FILE" &
    SERVER_PID=$!
    
    # Store PID for cleanup
    echo $SERVER_PID > /tmp/camera-stream-test.pid
    
    # Wait for server to start
    for i in {1..30}; do
        if curl -s "$BASE_URL/health" >/dev/null 2>&1; then
            info "Server started successfully (PID: $SERVER_PID)"
            return 0
        fi
        sleep 1
    done
    
    error "Server failed to start within 30 seconds"
    return 1
}

stop_server() {
    if [ -f /tmp/camera-stream-test.pid ]; then
        SERVER_PID=$(cat /tmp/camera-stream-test.pid)
        log "Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID 2>/dev/null || true
        rm -f /tmp/camera-stream-test.pid
    fi
}

test_health_check() {
    log "Testing health check endpoint..."
    
    response=$(curl -s "$BASE_URL/health")
    if [ "$response" = "OK" ]; then
        info "Health check passed"
    else
        error "Health check failed. Expected 'OK', got: $response"
        return 1
    fi
}

test_api_endpoints() {
    log "Testing API endpoints..."
    
    # Test status endpoint
    log "Testing status endpoint..."
    if ! curl -s "$BASE_URL/api/v1/status" | grep -q '"success":true'; then
        error "Status endpoint test failed"
        return 1
    fi
    info "Status endpoint working"
    
    # Test sources endpoint
    log "Testing sources endpoint..."
    sources_response=$(curl -s "$BASE_URL/api/v1/sources")
    if echo "$sources_response" | grep -q '"success":true'; then
        info "Sources endpoint working"
        
        # Check if any sources were detected
        source_count=$(echo "$sources_response" | grep -o '"id":[0-9]*' | wc -l)
        if [ "$source_count" -gt 0 ]; then
            info "Detected $source_count camera source(s)"
            
            # Try to start a stream
            log "Testing stream start..."
            first_source_id=$(echo "$sources_response" | grep -o '"id":[0-9]*' | head -1 | cut -d':' -f2)
            
            start_response=$(curl -s -X POST "$BASE_URL/api/v1/streams" \
                -H "Content-Type: application/json" \
                -d "{\"source_id\": $first_source_id}")
            
            if echo "$start_response" | grep -q '"success":true'; then
                info "Stream started successfully for source $first_source_id"
                
                # Test stream info
                sleep 2
                stream_info=$(curl -s "$BASE_URL/api/v1/streams/$first_source_id")
                if echo "$stream_info" | grep -q '"is_active":true'; then
                    info "Stream is active and working"
                else
                    warn "Stream may not be fully active yet"
                fi
                
                # Test RTSP endpoint (basic check)
                rtsp_port=$((5000 + first_source_id))
                if netstat -tlnp 2>/dev/null | grep -q ":$rtsp_port "; then
                    info "RTSP server is listening on port $rtsp_port"
                    info "Stream should be available at: rtsp://localhost:$rtsp_port/stream/$first_source_id"
                else
                    warn "RTSP server may not be ready on port $rtsp_port"
                fi
                
                # Stop the stream
                log "Testing stream stop..."
                stop_response=$(curl -s -X DELETE "$BASE_URL/api/v1/streams/$first_source_id")
                if echo "$stop_response" | grep -q '"success":true'; then
                    info "Stream stopped successfully"
                else
                    warn "Stream stop may have failed"
                fi
            else
                warn "Failed to start stream: $start_response"
            fi
        else
            warn "No camera sources detected. Check camera connections."
        fi
    else
        error "Sources endpoint test failed: $sources_response"
        return 1
    fi
    
    log "API endpoints test completed"
}

test_cache() {
    log "Testing cache functionality..."
    
    cache_stats=$(curl -s "$BASE_URL/api/v1/cache/stats")
    if echo "$cache_stats" | grep -q '"success":true'; then
        info "Cache stats endpoint working"
        
        total_frames=$(echo "$cache_stats" | grep -o '"total_frames":[0-9]*' | cut -d':' -f2)
        memory_usage=$(echo "$cache_stats" | grep -o '"memory_usage":[0-9]*' | cut -d':' -f2)
        
        info "Cache contains $total_frames frames using $memory_usage bytes"
    else
        warn "Cache stats test failed"
    fi
}

cleanup() {
    log "Cleaning up..."
    stop_server
    
    # Clean up any test files
    rm -f /tmp/camera-stream-test.pid
}

# Trap for cleanup
trap cleanup EXIT

# Main test sequence
main() {
    log "Starting Camera Stream Proxy setup test..."
    
    test_prerequisites || exit 1
    test_build || exit 1
    start_server || exit 1
    
    # Give server time to initialize
    sleep 3
    
    test_health_check || exit 1
    test_api_endpoints || exit 1
    test_cache || exit 1
    
    log "All tests completed successfully!"
    log ""
    log "Your camera streaming system is working correctly."
    log "You can now:"
    log "  1. Start the server: ./target/debug/stream-engine --config config/development.yml"
    log "  2. Check available cameras: curl $BASE_URL/api/v1/sources"
    log "  3. Start streaming: curl -X POST $BASE_URL/api/v1/streams -H 'Content-Type: application/json' -d '{\"source_id\": 1}'"
    log "  4. View RTSP stream: vlc rtsp://localhost:5001/stream/1"
}

# Run main function
main "$@" 