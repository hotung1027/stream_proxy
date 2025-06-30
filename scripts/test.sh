#!/bin/bash

# Camera Stream Proxy - Test Script
# Usage: ./scripts/test.sh [component] [options]
# Components: core, api, web, ros, all
# Options: --coverage, --verbose, --watch

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPONENT="${1:-all}"
COVERAGE_FLAG=""
VERBOSE_FLAG=""
WATCH_FLAG=""

# Parse options
for arg in "$@"; do
    case $arg in
        --coverage)
            COVERAGE_FLAG="--coverage"
            shift
            ;;
        --verbose)
            VERBOSE_FLAG="--verbose"
            shift
            ;;
        --watch)
            WATCH_FLAG="--watch"
            shift
            ;;
    esac
done

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

# Check if component tests exist
check_test_exists() {
    local component=$1
    local test_path=""
    
    case $component in
        core)
            test_path="$PROJECT_ROOT/src/core/tests"
            ;;
        api)
            test_path="$PROJECT_ROOT/src/api"
            ;;
        web)
            test_path="$PROJECT_ROOT/src/web"
            ;;
        ros)
            test_path="$PROJECT_ROOT/tests/ros"
            ;;
    esac
    
    if [ ! -d "$test_path" ] && [ ! -f "$test_path" ]; then
        warn "No tests found for component: $component"
        return 1
    fi
    
    return 0
}

# Test Rust core components
test_core() {
    log "Running Rust core tests..."
    
    cd "$PROJECT_ROOT"
    
    local test_cmd="cargo test"
    
    if [ "$COVERAGE_FLAG" = "--coverage" ]; then
        # Install cargo-tarpaulin if not present
        if ! command -v cargo-tarpaulin &> /dev/null; then
            log "Installing cargo-tarpaulin for coverage..."
            cargo install cargo-tarpaulin
        fi
        test_cmd="cargo tarpaulin --out Html --output-dir coverage/rust"
    fi
    
    if [ "$VERBOSE_FLAG" = "--verbose" ]; then
        test_cmd="$test_cmd -- --nocapture"
    fi
    
    if [ "$WATCH_FLAG" = "--watch" ]; then
        # Install cargo-watch if not present
        if ! command -v cargo-watch &> /dev/null; then
            log "Installing cargo-watch for watch mode..."
            cargo install cargo-watch
        fi
        test_cmd="cargo watch -x test"
    fi
    
    log "Executing: $test_cmd"
    eval $test_cmd
    
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        info "✅ Rust core tests passed"
    else
        error "❌ Rust core tests failed"
        return $exit_code
    fi
}

# Test Go API components
test_api() {
    log "Running Go API tests..."
    
    cd "$PROJECT_ROOT"
    
    # Ensure go.mod is up to date
    go mod tidy
    
    local test_cmd="go test ./src/api/... -v"
    
    if [ "$COVERAGE_FLAG" = "--coverage" ]; then
        mkdir -p coverage/go
        test_cmd="go test ./src/api/... -v -coverprofile=coverage/go/coverage.out"
        
        # Generate HTML coverage report
        if [ -f "coverage/go/coverage.out" ]; then
            go tool cover -html=coverage/go/coverage.out -o coverage/go/coverage.html
        fi
    fi
    
    if [ "$WATCH_FLAG" = "--watch" ]; then
        # Install gow if not present
        if ! command -v gow &> /dev/null; then
            log "Installing gow for watch mode..."
            go install github.com/mitranim/gow@latest
        fi
        test_cmd="gow test ./src/api/... -v"
    fi
    
    log "Executing: $test_cmd"
    eval $test_cmd
    
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        info "✅ Go API tests passed"
    else
        error "❌ Go API tests failed"
        return $exit_code
    fi
}

# Test web components
test_web() {
    log "Running web interface tests..."
    
    cd "$PROJECT_ROOT"
    
    # Install dependencies if needed
    if [ ! -d "node_modules" ]; then
        log "Installing Node.js dependencies..."
        npm install
    fi
    
    local test_cmd="npm test"
    
    if [ "$COVERAGE_FLAG" = "--coverage" ]; then
        test_cmd="npm run coverage"
    fi
    
    if [ "$VERBOSE_FLAG" = "--verbose" ]; then
        test_cmd="$test_cmd -- --verbose"
    fi
    
    if [ "$WATCH_FLAG" = "--watch" ]; then
        test_cmd="npm test -- --watch"
    else
        test_cmd="$test_cmd -- --run"
    fi
    
    log "Executing: $test_cmd"
    eval $test_cmd
    
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        info "✅ Web interface tests passed"
    else
        error "❌ Web interface tests failed"
        return $exit_code
    fi
}

# Test ROS integration components
test_ros() {
    log "Running ROS integration tests..."
    
    cd "$PROJECT_ROOT"
    
    # Activate Python virtual environment if it exists
    if [ -d ".venv" ]; then
        source .venv/bin/activate
    fi
    
    local test_cmd="python -m pytest tests/ros/ -v"
    
    if [ "$COVERAGE_FLAG" = "--coverage" ]; then
        mkdir -p coverage/python
        test_cmd="python -m pytest tests/ros/ -v --cov=src/ros --cov-report=html:coverage/python --cov-report=term"
    fi
    
    if [ "$WATCH_FLAG" = "--watch" ]; then
        # Install pytest-watch if not present
        pip install pytest-watch
        test_cmd="ptw tests/ros/ -- -v"
    fi
    
    log "Executing: $test_cmd"
    eval $test_cmd
    
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        info "✅ ROS integration tests passed"
    else
        error "❌ ROS integration tests failed"
        return $exit_code
    fi
}

# Test all components
test_all() {
    log "Running all component tests..."
    
    local failed_components=()
    
    # Test each component and track failures
    if check_test_exists "core"; then
        if ! test_core; then
            failed_components+=("core")
        fi
    fi
    
    if check_test_exists "api"; then
        if ! test_api; then
            failed_components+=("api")
        fi
    fi
    
    if check_test_exists "web"; then
        if ! test_web; then
            failed_components+=("web")
        fi
    fi
    
    if check_test_exists "ros"; then
        if ! test_ros; then
            failed_components+=("ros")
        fi
    fi
    
    # Report results
    if [ ${#failed_components[@]} -eq 0 ]; then
        info "🎉 All tests passed successfully!"
        
        if [ "$COVERAGE_FLAG" = "--coverage" ]; then
            log "Coverage reports generated in coverage/ directory"
            log "Open coverage/rust/tarpaulin-report.html for Rust coverage"
            log "Open coverage/go/coverage.html for Go coverage"
            log "Open coverage/python/index.html for Python coverage"
        fi
    else
        error "❌ Tests failed for components: ${failed_components[*]}"
        return 1
    fi
}

# Generate test report
generate_test_report() {
    log "Generating test report..."
    
    local report_file="$PROJECT_ROOT/test-report.md"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    cat > "$report_file" << EOF
# Test Report

**Generated**: $timestamp
**Component**: $COMPONENT
**Options**: Coverage=$COVERAGE_FLAG, Verbose=$VERBOSE_FLAG, Watch=$WATCH_FLAG

## Test Results

EOF
    
    # Add component-specific results based on what was tested
    if [ "$COMPONENT" = "all" ] || [ "$COMPONENT" = "core" ]; then
        echo "### Rust Core Tests" >> "$report_file"
        echo "Status: $(test_core && echo "✅ PASSED" || echo "❌ FAILED")" >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    # Add coverage information if enabled
    if [ "$COVERAGE_FLAG" = "--coverage" ]; then
        echo "## Coverage Reports" >> "$report_file"
        echo "- Rust: coverage/rust/tarpaulin-report.html" >> "$report_file"
        echo "- Go: coverage/go/coverage.html" >> "$report_file"
        echo "- Python: coverage/python/index.html" >> "$report_file"
        echo "- JavaScript: coverage/lcov-report/index.html" >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    info "Test report generated: $report_file"
}

# Main execution
main() {
    cd "$PROJECT_ROOT"
    
    log "Starting test execution..."
    log "Component: $COMPONENT"
    log "Options: Coverage=$COVERAGE_FLAG, Verbose=$VERBOSE_FLAG, Watch=$WATCH_FLAG"
    
    # Create coverage directory if coverage is requested
    if [ "$COVERAGE_FLAG" = "--coverage" ]; then
        mkdir -p coverage/{rust,go,python,javascript}
    fi
    
    case "$COMPONENT" in
        core)
            test_core
            ;;
        api)
            test_api
            ;;
        web)
            test_web
            ;;
        ros)
            test_ros
            ;;
        all)
            test_all
            ;;
        *)
            error "Unknown component: $COMPONENT"
            error "Available components: core, api, web, ros, all"
            exit 1
            ;;
    esac
    
    # Generate report unless in watch mode
    if [ "$WATCH_FLAG" != "--watch" ]; then
        generate_test_report
    fi
    
    log "Test execution completed!"
}

# Run main function
main "$@" 