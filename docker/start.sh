#!/bin/bash

# Camera Stream Proxy - Docker Startup Script

set -e

# Configuration
STREAM_ENGINE="./bin/stream-engine"
API_SERVICE="./bin/api-service"
CONFIG_FILE="./config/default.yml"

# Colors for logging
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
log() {
    echo -e "${GREEN}[STARTUP]${NC} $1"
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

# Cleanup function
cleanup() {
    log "Shutting down services..."
    
    # Kill child processes
    jobs -p | xargs -r kill 2>/dev/null || true
    
    # Wait for processes to terminate
    wait
    
    log "Shutdown complete"
    exit 0
}

# Set up signal handlers
trap cleanup SIGINT SIGTERM

# Pre-flight checks
preflight_checks() {
    log "Running pre-flight checks..."
    
    # Check if binaries exist
    if [ ! -f "$STREAM_ENGINE" ]; then
        error "Stream engine binary not found: $STREAM_ENGINE"
        exit 1
    fi
    
    if [ ! -f "$API_SERVICE" ]; then
        error "API service binary not found: $API_SERVICE"
        exit 1
    fi
    
    # Check if config exists
    if [ ! -f "$CONFIG_FILE" ]; then
        error "Configuration file not found: $CONFIG_FILE"
        exit 1
    fi
    
    # Check if required directories exist
    for dir in cache recordings logs; do
        if [ ! -d "$dir" ]; then
            mkdir -p "$dir"
            log "Created directory: $dir"
        fi
    done
    
    log "Pre-flight checks completed"
}

# Start services
start_services() {
    log "Starting services..."
    
    # Start stream engine
    log "Starting stream engine..."
    $STREAM_ENGINE --config "$CONFIG_FILE" &
    STREAM_PID=$!
    info "Stream engine started with PID: $STREAM_PID"
    
    # Wait a moment for stream engine to initialize
    sleep 2
    
    # Start API service
    log "Starting API service..."
    $API_SERVICE &
    API_PID=$!
    info "API service started with PID: $API_PID"
    
    # Wait a moment for API service to initialize
    sleep 2
    
    log "All services started successfully"
}

# Health check
health_check() {
    log "Performing health check..."
    
    # Simple health check - verify processes are running
    if ! kill -0 $STREAM_PID 2>/dev/null; then
        error "Stream engine process is not running"
        return 1
    fi
    
    if ! kill -0 $API_PID 2>/dev/null; then
        error "API service process is not running"
        return 1
    fi
    
    info "Health check passed"
    return 0
}

# Main execution
main() {
    log "Camera Stream Proxy starting up..."
    
    # Run pre-flight checks
    preflight_checks
    
    # Start all services
    start_services
    
    # Initial health check
    if ! health_check; then
        error "Initial health check failed"
        cleanup
        exit 1
    fi
    
    log "Startup complete. Services are running."
    log "Stream Engine PID: $STREAM_PID"
    log "API Service PID: $API_PID"
    
    # Periodic health checks
    while true; do
        sleep 30
        
        if ! health_check; then
            error "Health check failed. Initiating shutdown..."
            cleanup
            exit 1
        fi
    done
}

# Execute main function
main "$@" 