# Camera Stream Proxy - Multi-stage Dockerfile

# Stage 1: Rust Builder
FROM rust:1.75-slim as rust-builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy Rust source
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Build Rust application
RUN cargo build --release

# Stage 2: Go Builder  
FROM golang:1.21-alpine as go-builder

WORKDIR /app

# Copy Go source
COPY go.mod go.sum ./
RUN go mod download

COPY src/api/ ./src/api/

# Build Go application
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o api-service ./src/api/

# Stage 3: Node.js Builder
FROM node:18-alpine as node-builder

WORKDIR /app

# Copy package files
COPY package.json package-lock.json ./
RUN npm ci --only=production

# Copy web source
COPY src/web/ ./src/web/

# Build web application
RUN npm run build

# Stage 4: Python Dependencies
FROM python:3.11-slim as python-deps

WORKDIR /app

# Install Python dependencies
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

# Stage 5: Final Runtime Image
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    gstreamer1.0-tools \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-vaapi \
    libgstreamer1.0-0 \
    libgstreamer-plugins-base1.0-0 \
    python3 \
    python3-pip \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN groupadd -r app && useradd -r -g app app

# Create application directory
WORKDIR /app

# Copy binaries from builders
COPY --from=rust-builder /app/target/release/stream-engine ./bin/
COPY --from=go-builder /app/api-service ./bin/
COPY --from=node-builder /app/dist ./web/
COPY --from=python-deps /usr/local/lib/python3.11/site-packages /usr/local/lib/python3.11/site-packages

# Copy configuration and scripts
COPY config/ ./config/
COPY src/ros/ ./ros/

# Copy startup script
COPY docker/start.sh ./
RUN chmod +x start.sh

# Create necessary directories
RUN mkdir -p cache recordings logs && \
    chown -R app:app /app

# Switch to app user
USER app

# Expose ports
EXPOSE 8080 8081 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Start the application
CMD ["./start.sh"] 