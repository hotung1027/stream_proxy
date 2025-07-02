#!/usr/bin/env python3
"""
Test Server for Camera Stream Proxy

This script serves the web test platform and proxies API requests to the camera stream proxy.
It allows you to test the system through a web interface.

Usage:
    python3 test_server.py [--port PORT] [--api-url API_URL]
"""

import argparse
import asyncio
import json
import logging
import os
import sys
from pathlib import Path

import aiohttp
from aiohttp import web
import aiohttp_cors

# Configure logging
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


class TestServer:
    """Web server for testing camera stream proxy"""

    def __init__(self, port=8888, api_url="http://localhost:8080"):
        self.port = port
        self.api_url = api_url
        self.app = web.Application()
        self.setup_routes()
        self.setup_cors()

    def setup_routes(self):
        """Set up web routes"""
        # Serve static files
        self.app.router.add_get("/", self.serve_dashboard)
        self.app.router.add_get("/platform", self.serve_platform)

        # API proxy routes
        self.app.router.add_route("*", "/api/{path:.*}", self.proxy_api)
        self.app.router.add_route("*", "/health", self.proxy_health)

        # WebSocket for real-time updates
        self.app.router.add_get("/ws", self.websocket_handler)

    def setup_cors(self):
        """Set up CORS for cross-origin requests"""
        cors = aiohttp_cors.setup(
            self.app,
            defaults={
                "*": aiohttp_cors.ResourceOptions(
                    allow_credentials=True,
                    expose_headers="*",
                    allow_headers="*",
                    allow_methods="*",
                )
            },
        )

        # Configure CORS on all routes
        for route in list(self.app.router.routes()):
            cors.add(route)

    async def serve_dashboard(self, request):
        """Serve the test dashboard HTML"""
        web_dir = Path(__file__).parent.parent / "web"
        dashboard_path = web_dir / "test-dashboard.html"

        if dashboard_path.exists():
            return web.FileResponse(dashboard_path)
        else:
            return web.Response(text="Dashboard not found", status=404)

    async def serve_platform(self, request):
        """Serve the test platform HTML"""
        web_dir = Path(__file__).parent.parent / "web"
        platform_path = web_dir / "test-platform.html"

        if platform_path.exists():
            return web.FileResponse(platform_path)
        else:
            return web.Response(text="Platform not found", status=404)

    async def proxy_api(self, request):
        """Proxy API requests to the camera stream proxy"""
        path = request.match_info["path"]
        url = f"{self.api_url}/api/{path}"

        logger.info(f"Proxying {request.method} request to: {url}")

        try:
            async with aiohttp.ClientSession() as session:
                # Forward the request
                data = await request.read() if request.body_exists else None
                headers = {
                    k: v
                    for k, v in request.headers.items()
                    if k.lower() not in ["host", "content-length"]
                }

                async with session.request(
                    method=request.method,
                    url=url,
                    data=data,
                    headers=headers,
                    params=request.query,
                ) as response:
                    # Forward the response
                    body = await response.read()
                    return web.Response(
                        body=body, status=response.status, headers=response.headers
                    )
        except Exception as e:
            logger.error(f"Proxy error: {e}")
            return web.json_response({"success": False, "error": str(e)}, status=503)

    async def proxy_health(self, request):
        """Proxy health check requests"""
        url = f"{self.api_url}/health"

        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(url) as response:
                    body = await response.read()
                    return web.Response(
                        body=body, status=response.status, headers=response.headers
                    )
        except Exception as e:
            logger.error(f"Health check error: {e}")
            return web.Response(text="Service Unavailable", status=503)

    async def websocket_handler(self, request):
        """Handle WebSocket connections for real-time updates"""
        ws = web.WebSocketResponse()
        await ws.prepare(request)

        logger.info("WebSocket client connected")

        try:
            # Send initial status
            await ws.send_json(
                {"type": "connected", "message": "Connected to test server"}
            )

            # Keep connection alive and handle messages
            async for msg in ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    data = json.loads(msg.data)

                    # Handle different message types
                    if data.get("type") == "ping":
                        await ws.send_json({"type": "pong"})
                    elif data.get("type") == "subscribe":
                        # Handle subscription to updates
                        logger.info(f"Client subscribed to: {data.get('topic')}")

                elif msg.type == aiohttp.WSMsgType.ERROR:
                    logger.error(f"WebSocket error: {ws.exception()}")

        except Exception as e:
            logger.error(f"WebSocket error: {e}")
        finally:
            logger.info("WebSocket client disconnected")

        return ws

    def run(self):
        """Run the test server"""
        logger.info(f"Starting test server on http://localhost:{self.port}")
        logger.info(f"Proxying API requests to: {self.api_url}")
        logger.info("")
        logger.info("Available endpoints:")
        logger.info(f"  Dashboard: http://localhost:{self.port}/")
        logger.info(f"  Platform:  http://localhost:{self.port}/platform")
        logger.info("")

        web.run_app(self.app, host="0.0.0.0", port=self.port)


class StreamSimulator:
    """Simulate camera streams for testing without real cameras"""

    def __init__(self, num_cameras=2):
        self.num_cameras = num_cameras
        self.streams = {}

    def generate_test_frame(self, camera_id):
        """Generate a test frame for a camera"""
        import numpy as np
        import cv2
        import base64

        # Create a test image with camera ID
        img = np.zeros((480, 640, 3), dtype=np.uint8)

        # Add some visual elements
        cv2.putText(
            img,
            f"Camera {camera_id}",
            (50, 240),
            cv2.FONT_HERSHEY_SIMPLEX,
            2,
            (0, 255, 0),
            3,
        )

        # Add timestamp
        import datetime

        timestamp = datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3]
        cv2.putText(
            img, timestamp, (50, 400), cv2.FONT_HERSHEY_SIMPLEX, 1, (255, 255, 255), 2
        )

        # Add some random noise for visual effect
        noise = np.random.randint(0, 50, img.shape, dtype=np.uint8)
        img = cv2.add(img, noise)

        # Encode as JPEG
        _, buffer = cv2.imencode(".jpg", img)

        # Convert to base64
        return base64.b64encode(buffer).decode("utf-8")

    def get_test_sources(self):
        """Get simulated camera sources"""
        sources = []
        for i in range(self.num_cameras):
            sources.append(
                {
                    "id": i + 1,
                    "name": f"Test Camera {i + 1}",
                    "device_path": f"/dev/video{i}",
                    "source_type": "usb_camera",
                    "resolution": [640, 480],
                    "framerate": 30.0,
                    "format": "YUY2",
                    "capabilities": ["video/x-raw"],
                }
            )
        return sources


def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description="Test server for Camera Stream Proxy")
    parser.add_argument(
        "--port",
        type=int,
        default=8888,
        help="Port to run the test server on (default: 8888)",
    )
    parser.add_argument(
        "--api-url",
        default="http://localhost:8080",
        help="URL of the camera stream proxy API (default: http://localhost:8080)",
    )
    parser.add_argument(
        "--simulate", action="store_true", help="Simulate camera streams for testing"
    )

    args = parser.parse_args()

    # Check if web files exist
    web_dir = Path(__file__).parent.parent / "web"
    if not web_dir.exists():
        logger.error(f"Web directory not found: {web_dir}")
        sys.exit(1)

    dashboard_path = web_dir / "test-dashboard.html"
    platform_path = web_dir / "test-platform.html"

    if not dashboard_path.exists() and not platform_path.exists():
        logger.error("No test HTML files found in web directory")
        logger.error("Please ensure test-dashboard.html or test-platform.html exist")
        sys.exit(1)

    # Create and run server
    server = TestServer(port=args.port, api_url=args.api_url)

    if args.simulate:
        logger.info("Running in simulation mode - no real cameras needed")
        simulator = StreamSimulator()
        # You could extend this to mock the API responses

    try:
        server.run()
    except KeyboardInterrupt:
        logger.info("Shutting down test server")


if __name__ == "__main__":
    main()
