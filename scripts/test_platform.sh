#!/bin/bash
# Camera Stream Proxy - Test Platform Launcher
#
# This script launches the complete test environment for camera streaming

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
STREAM_ENGINE_PORT=8080
TEST_SERVER_PORT=8888
CONFIG_FILE="config/default.yml"

echo -e "${BLUE}🎥 Camera Stream Proxy - Test Platform Launcher${NC}"
echo "================================================"

# Function to check if a port is in use
check_port() {
    if lsof -Pi :$1 -sTCP:LISTEN -t >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Function to wait for service
wait_for_service() {
    local port=$1
    local service=$2
    local max_attempts=30
    local attempt=0
    
    echo -n "Waiting for $service to start on port $port"
    while ! check_port $port && [ $attempt -lt $max_attempts ]; do
        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done
    
    if [ $attempt -eq $max_attempts ]; then
        echo -e "\n${RED}✗ Failed to start $service${NC}"
        return 1
    else
        echo -e "\n${GREEN}✓ $service is running${NC}"
        return 0
    fi
}

# Check dependencies
echo -e "\n${YELLOW}Checking dependencies...${NC}"

# Check if stream engine is built
if [ ! -f "target/debug/stream-engine" ]; then
    echo -e "${RED}✗ Stream engine not built${NC}"
    echo "  Run: ./scripts/build.sh core"
    exit 1
else
    echo -e "${GREEN}✓ Stream engine found${NC}"
fi

# Check Python dependencies
if ! python3 -c "import aiohttp" 2>/dev/null; then
    echo -e "${RED}✗ Python dependencies not installed${NC}"
    echo "  Run: pip install -r requirements.txt"
    exit 1
else
    echo -e "${GREEN}✓ Python dependencies installed${NC}"
fi

# Check if services are already running
if check_port $STREAM_ENGINE_PORT; then
    echo -e "${YELLOW}⚠ Stream engine already running on port $STREAM_ENGINE_PORT${NC}"
    read -p "Kill existing process? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Stopping existing stream engine..."
        pkill -f "stream-engine" || true
        sleep 2
    fi
fi

if check_port $TEST_SERVER_PORT; then
    echo -e "${YELLOW}⚠ Test server already running on port $TEST_SERVER_PORT${NC}"
    read -p "Kill existing process? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Stopping existing test server..."
        pkill -f "test_server.py" || true
        sleep 2
    fi
fi

# Start services
echo -e "\n${BLUE}Starting services...${NC}"

# Start stream engine
echo -e "\n1. Starting Camera Stream Engine..."
if [ "$PROXY_ENV" = "1" ]; then
    echo "   (Using proxy environment fix)"
    unset ARGV0
fi

# Export environment for stream engine
export RUST_LOG=info
export RUST_BACKTRACE=1

# Start stream engine in background
./target/debug/stream-engine --config "$CONFIG_FILE" > logs/stream-engine.log 2>&1 &
STREAM_ENGINE_PID=$!
echo "   PID: $STREAM_ENGINE_PID"

# Wait for stream engine to start
if ! wait_for_service $STREAM_ENGINE_PORT "Stream Engine"; then
    echo -e "${RED}Failed to start stream engine. Check logs/stream-engine.log${NC}"
    exit 1
fi

# Start test server
echo -e "\n2. Starting Test Server..."
python3 scripts/test_server.py --port $TEST_SERVER_PORT --api-url "http://localhost:$STREAM_ENGINE_PORT" > logs/test-server.log 2>&1 &
TEST_SERVER_PID=$!
echo "   PID: $TEST_SERVER_PID"

# Wait for test server to start
if ! wait_for_service $TEST_SERVER_PORT "Test Server"; then
    echo -e "${RED}Failed to start test server. Check logs/test-server.log${NC}"
    kill $STREAM_ENGINE_PID 2>/dev/null
    exit 1
fi

# Success!
echo -e "\n${GREEN}✨ Test Platform is ready!${NC}"
echo -e "\nAccess the test interfaces at:"
echo -e "  ${BLUE}Dashboard:${NC} http://localhost:$TEST_SERVER_PORT/"
echo -e "  ${BLUE}Platform:${NC}  http://localhost:$TEST_SERVER_PORT/platform"
echo -e "\nAPI endpoints:"
echo -e "  ${BLUE}Health:${NC}    http://localhost:$STREAM_ENGINE_PORT/health"
echo -e "  ${BLUE}Sources:${NC}   http://localhost:$STREAM_ENGINE_PORT/api/v1/sources"
echo -e "\nRTSP streams (once started):"
echo -e "  ${BLUE}Camera 1:${NC}  rtsp://localhost:8554/stream/1"
echo -e "  ${BLUE}Camera 2:${NC}  rtsp://localhost:8554/stream/2"
echo -e "\nLogs:"
echo -e "  ${BLUE}Stream Engine:${NC} tail -f logs/stream-engine.log"
echo -e "  ${BLUE}Test Server:${NC}   tail -f logs/test-server.log"

# Create PID file for easy cleanup
echo "$STREAM_ENGINE_PID" > .stream-engine.pid
echo "$TEST_SERVER_PID" > .test-server.pid

# Set up trap to clean up on exit
cleanup() {
    echo -e "\n${YELLOW}Shutting down services...${NC}"
    
    if [ -f .stream-engine.pid ]; then
        kill $(cat .stream-engine.pid) 2>/dev/null || true
        rm .stream-engine.pid
    fi
    
    if [ -f .test-server.pid ]; then
        kill $(cat .test-server.pid) 2>/dev/null || true
        rm .test-server.pid
    fi
    
    echo -e "${GREEN}✓ Services stopped${NC}"
}

trap cleanup EXIT INT TERM

echo -e "\n${YELLOW}Press Ctrl+C to stop all services${NC}\n"

# Keep script running
wait 