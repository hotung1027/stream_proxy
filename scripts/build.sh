#!/bin/bash

# Camera Stream Proxy - Build Script
# Usage: ./scripts/build.sh [component] [mode]
# Components: core, api, web, ros, all
# Modes: dev, prod

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_MODE="${2:-dev}"
COMPONENT="${1:-all}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
log() {
    echo -e "${GREEN}[BUILD]${NC} $1"
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

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check Rust
    if ! command -v cargo &> /dev/null; then
        error "Rust/Cargo is not installed. Please install Rust from https://rustup.rs/"
        exit 1
    fi
    
    # Check Go
    if ! command -v go &> /dev/null; then
        error "Go is not installed. Please install Go from https://golang.org/"
        exit 1
    fi
    
    # Check Node.js
    if ! command -v node &> /dev/null; then
        error "Node.js is not installed. Please install Node.js from https://nodejs.org/"
        exit 1
    fi
    
    # Check Python
    if ! command -v python3 &> /dev/null; then
        error "Python 3 is not installed. Please install Python 3"
        exit 1
    fi
    
    # Check GStreamer
    if ! pkg-config --exists gstreamer-1.0; then
        warn "GStreamer development libraries not found. Please install gstreamer1.0-dev"
    fi
    
    log "Prerequisites check completed"
}

# Create necessary directories
create_directories() {
    log "Creating necessary directories..."
    
    mkdir -p "$PROJECT_ROOT/build"
    mkdir -p "$PROJECT_ROOT/dist"
    mkdir -p "$PROJECT_ROOT/logs"
    mkdir -p "$PROJECT_ROOT/cache"
    mkdir -p "$PROJECT_ROOT/recordings"
    
    log "Directories created"
}

# Build Rust core
build_core() {
    log "Building Rust core streaming engine..."
    
    cd "$PROJECT_ROOT"
    
    if [ "$BUILD_MODE" = "prod" ]; then
        cargo build --release
        info "Core built in release mode"
    else
        cargo build
        info "Core built in debug mode"
    fi
    
    # Run tests
    log "Running Rust tests..."
    cargo test --lib
    
    log "Core build completed"
}

# Build Go API service
build_api() {
    log "Building Go API service..."
    
    cd "$PROJECT_ROOT"
    
    # Ensure go.mod is properly initialized
    if [ ! -f "go.sum" ]; then
        go mod tidy
    fi
    
    # Build API service
    if [ "$BUILD_MODE" = "prod" ]; then
        CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o build/api-service ./src/api/
        info "API service built for production"
    else
        go build -o build/api-service ./src/api/
        info "API service built for development"
    fi
    
    # Run tests
    log "Running Go tests..."
    go test ./src/api/...
    
    log "API build completed"
}

# Build web interface
build_web() {
    log "Building web interface..."
    
    cd "$PROJECT_ROOT"
    
    # Install dependencies
    if [ ! -d "node_modules" ]; then
        log "Installing Node.js dependencies..."
        npm install
    fi
    
    # Build web interface
    if [ "$BUILD_MODE" = "prod" ]; then
        npm run build
        info "Web interface built for production"
    else
        npm run build -- --mode development
        info "Web interface built for development"
    fi
    
    # Run tests
    log "Running web tests..."
    npm test -- --run
    
    log "Web build completed"
}

# Build ROS integration
build_ros() {
    log "Building ROS integration..."
    
    cd "$PROJECT_ROOT"
    
    # Create Python virtual environment if it doesn't exist
    if [ ! -d ".venv" ]; then
        log "Creating Python virtual environment..."
        python3 -m venv .venv
    fi
    
    # Activate virtual environment
    source .venv/bin/activate
    
    # Install dependencies
    log "Installing Python dependencies..."
    pip install -r requirements.txt
    
    # Run tests
    log "Running Python tests..."
    python -m pytest tests/ -v
    
    log "ROS build completed"
}

# Build all components
build_all() {
    log "Building all components..."
    
    build_core
    build_api
    build_web
    build_ros
    
    log "All components built successfully"
}

# Package for distribution
package_distribution() {
    log "Packaging for distribution..."
    
    cd "$PROJECT_ROOT"
    
    # Create distribution directory
    mkdir -p dist/camera-stream-proxy
    
    # Copy binaries
    if [ "$BUILD_MODE" = "prod" ]; then
        cp target/release/stream-engine dist/camera-stream-proxy/
    else
        cp target/debug/stream-engine dist/camera-stream-proxy/
    fi
    
    cp build/api-service dist/camera-stream-proxy/
    
    # Copy configuration
    cp -r config dist/camera-stream-proxy/
    
    # Copy web assets
    cp -r dist/web dist/camera-stream-proxy/web
    
    # Copy Python scripts
    cp -r src/ros dist/camera-stream-proxy/ros
    
    # Copy documentation
    cp README.md dist/camera-stream-proxy/
    cp docs/* dist/camera-stream-proxy/docs/ 2>/dev/null || true
    
    # Create startup script
    cat > dist/camera-stream-proxy/start.sh << 'EOF'
#!/bin/bash
set -e

# Start the streaming engine
./stream-engine --config config/default.yml &
STREAM_PID=$!

# Start the API service
./api-service &
API_PID=$!

# Cleanup function
cleanup() {
    kill $STREAM_PID $API_PID 2>/dev/null || true
    exit 0
}

trap cleanup SIGINT SIGTERM

# Wait for processes
wait $STREAM_PID $API_PID
EOF
    
    chmod +x dist/camera-stream-proxy/start.sh
    
    log "Distribution package created in dist/camera-stream-proxy"
}

# Main execution
main() {
    cd "$PROJECT_ROOT"
    
    log "Starting build process..."
    log "Component: $COMPONENT"
    log "Mode: $BUILD_MODE"
    
    check_prerequisites
    create_directories
    
    case "$COMPONENT" in
        core)
            build_core
            ;;
        api)
            build_api
            ;;
        web)
            build_web
            ;;
        ros)
            build_ros
            ;;
        all)
            build_all
            package_distribution
            ;;
        *)
            error "Unknown component: $COMPONENT"
            error "Available components: core, api, web, ros, all"
            exit 1
            ;;
    esac
    
    log "Build process completed successfully!"
}

# Run main function
main "$@" 