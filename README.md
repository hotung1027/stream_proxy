# Camera Stream Proxy

A high-performance, low-latency camera streaming system that aggregates multiple camera sources and distributes streams to various consumers with intelligent caching and format conversion capabilities.

## 🎯 Features

- **Multi-source Support**: USB cameras, RTSP streams, SDK cameras, stereo cameras, media files
- **Real-time Processing**: Sub-100ms latency for local streams
- **Intelligent Caching**: LRU cache with compression and deduplication
- **Format Conversion**: Hardware-accelerated encoding/decoding
- **Web Interface**: Real-time streaming to browsers via WebRTC/WebSocket
- **ROS Integration**: Publish to ROS topics with synchronized multi-camera support
- **Recording**: Continuous and event-based recording capabilities
- **REST API**: Complete API for stream management and configuration
- **Scalable Architecture**: Microservices design with horizontal scaling support

## 🏗️ Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Input Sources  │────│ Core Processing  │────│ Output Consumers│
│                 │    │                  │    │                 │
│ • USB Cameras   │    │ • Stream Ingestion│   │ • Web Interface │
│ • RTSP Streams  │    │ • Buffer Manager  │    │ • Recording     │
│ • SDK Cameras   │    │ • Format Convert  │    │ • ROS Topics    │
│ • Media Files   │    │ • Cache System    │    │ • REST API      │
│ • Stereo Cams   │    │ • Stream Proxy    │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- Linux (Ubuntu 20.04+ recommended)
- GStreamer 1.16+
- Rust 1.70+
- Go 1.21+
- Python 3.8+
- Node.js 18+ (for web interface)
- Docker (optional)

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/camera-stream-proxy.git
cd camera-stream-proxy

# Build the core streaming engine
cargo build --release

# Build configuration services
cd config-service && go build && cd ..

# Install Python dependencies
pip install -r requirements.txt

# Build web interface
cd web && npm install && npm run build && cd ..
```

### Basic Usage

```bash
# Start the core streaming service
./target/release/stream-engine --config config/default.yml

# Start the configuration service
./config-service/config-service --port 8080

# Start the web interface
cd web && npm start
```

Visit `http://localhost:3000` to access the web interface.

## 📋 Project Structure

```
camera-stream/
├── .cursorrules              # Cursor IDE rules
├── README.md                 # This file
├── Cargo.toml               # Rust dependencies
├── go.mod                   # Go dependencies  
├── requirements.txt         # Python dependencies
├── package.json             # Node.js dependencies
│
├── src/                     # Source code
│   ├── core/               # Core streaming engine (Rust)
│   ├── adapters/           # Camera source adapters
│   ├── cache/              # Caching system
│   ├── formats/            # Format conversion
│   ├── api/                # REST API service (Go)
│   ├── web/                # Web interface (React/TS)
│   └── ros/                # ROS integration (Python)
│
├── config/                 # Configuration files
│   ├── default.yml         # Default configuration
│   ├── development.yml     # Development environment
│   └── production.yml      # Production environment
│
├── tests/                  # Test suites
│   ├── unit/              # Unit tests
│   ├── integration/       # Integration tests
│   └── performance/       # Performance tests
│
├── docs/                   # Documentation
│   ├── ARCHITECTURE.md     # System architecture
│   ├── REQUIREMENTS.md     # Project requirements
│   ├── API.md             # API documentation
│   └── DEPLOYMENT.md      # Deployment guide
│
├── scripts/               # Build and deployment scripts
│   ├── build.sh          # Build script
│   ├── deploy.sh         # Deployment script
│   └── test.sh           # Test script
│
└── docker/               # Docker configurations
    ├── Dockerfile        # Main Dockerfile
    ├── docker-compose.yml # Multi-service setup
    └── .dockerignore     # Docker ignore file
```

## 🔧 Configuration

The system uses YAML configuration files with environment variable support:

```yaml
# config/default.yml
server:
  host: "0.0.0.0"
  port: 8080
  
streaming:
  buffer_size: "100MB"
  max_concurrent_streams: 50
  hardware_acceleration: true
  
sources:
  usb:
    auto_detect: true
    scan_interval: "30s"
  rtsp:
    connection_timeout: "10s"
    retry_attempts: 3
```

## 🧪 Testing

```bash
# Unit tests
cargo test                    # Rust tests
go test ./...                # Go tests  
python -m pytest tests/     # Python tests
npm test                     # JavaScript tests

# Integration tests
./scripts/test.sh integration

# Performance tests
./scripts/test.sh performance
```

## 📊 Performance

- **Latency**: < 100ms end-to-end for local streams
- **Throughput**: 50+ concurrent 1080p streams
- **4K Support**: 10+ simultaneous 4K streams
- **Resource Usage**: < 80% CPU under normal load

## 🔒 Security

- JWT-based authentication
- TLS/DTLS encryption for streams
- Role-based access control
- Secure key management

## 🤝 Contributing

Please read our [Contributing Guidelines](CONTRIBUTING.md) before submitting PRs.

### Development Workflow

1. Create feature branch: `git checkout -b feature/new-feature`
2. Make small, incremental commits (< 100 lines each)
3. Add tests for new functionality
4. Update documentation as needed
5. Submit PR with detailed description

### Code Standards

- Maximum 500 lines per file
- Comprehensive type annotations
- Detailed docstrings for all public APIs
- Follow language-specific style guides

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

- **Documentation**: [docs/](docs/)
- **Issues**: [GitHub Issues](https://github.com/your-org/camera-stream-proxy/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-org/camera-stream-proxy/discussions)

## 🗺️ Roadmap

- [x] Core streaming engine
- [x] Basic web interface
- [ ] Hardware acceleration support
- [ ] ROS2 integration
- [ ] Multi-node deployment
- [ ] Advanced analytics
- [ ] Mobile applications

## 📈 Status

![Build Status](https://github.com/your-org/camera-stream-proxy/workflows/CI/badge.svg)
![Coverage](https://codecov.io/gh/your-org/camera-stream-proxy/branch/main/graph/badge.svg)
![License](https://img.shields.io/github/license/your-org/camera-stream-proxy) 