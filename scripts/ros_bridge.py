#!/usr/bin/env python3
"""
ROS Bridge for Camera Stream Proxy

This script bridges camera streams from the Camera Stream Proxy to ROS topics.
It demonstrates how to integrate with ROS for robotics applications.
"""

import rclpy
from rclpy.node import Node
from rclpy.executors import MultiThreadedExecutor
import cv2
import numpy as np
import requests
import base64
from sensor_msgs.msg import Image, CompressedImage
from std_msgs.msg import Header
from cv_bridge import CvBridge
import threading
import time
from typing import Dict, List, Tuple, Optional


class CameraStreamBridge(Node):
    """Bridge between Camera Stream Proxy and ROS2"""

    def __init__(self, api_url="http://localhost:8080", rtsp_base="rtsp://localhost"):
        super().__init__("camera_stream_bridge")

        self.api_url = api_url
        self.rtsp_base = rtsp_base
        self.bridge = CvBridge()
        self.publishers: Dict[str, dict] = {}
        self.captures: Dict[str, cv2.VideoCapture] = {}
        self.running = True

        # Initialize node
        self.get_logger().info("Camera Stream Bridge initialized")

    def discover_cameras(self) -> List[dict]:
        """Discover available camera sources from the API"""
        try:
            response = requests.get(f"{self.api_url}/api/v1/sources", timeout=5)
            if response.status_code == 200:
                data = response.json()
                if data["success"]:
                    return data["data"]
            self.get_logger().warn(f"Failed to get sources: {response.status_code}")
            return []
        except requests.exceptions.RequestException as e:
            self.get_logger().error(f"Error discovering cameras: {e}")
            return []
        except Exception as e:
            self.get_logger().error(f"Unexpected error discovering cameras: {e}")
            return []

    def setup_publishers(self, sources: List[dict]) -> None:
        """Set up ROS publishers for each camera source"""
        for source in sources:
            try:
                source_id = source["id"]
                source_name = source["name"].replace(" ", "_").lower()

                # Create publishers for raw and compressed images
                raw_topic = f"/camera/{source_name}/image_raw"
                compressed_topic = f"/camera/{source_name}/compressed"

                self.publishers[source_id] = {
                    "raw": self.create_publisher(Image, raw_topic, 1),
                    "compressed": self.create_publisher(
                        CompressedImage, compressed_topic, 1
                    ),
                    "info": source,
                }

                self.get_logger().info(f"Created publishers for {source_name}:")
                self.get_logger().info(f"  Raw: {raw_topic}")
                self.get_logger().info(f"  Compressed: {compressed_topic}")

                # Start streaming thread for this source
                thread = threading.Thread(target=self.stream_camera, args=(source_id,))
                thread.daemon = True
                thread.start()
            except Exception as e:
                self.get_logger().error(
                    f"Error setting up publisher for source {source.get('id', 'unknown')}: {e}"
                )

    def stream_camera(self, source_id: str) -> None:
        """Stream camera data to ROS topics"""
        # Build RTSP URL
        rtsp_url = f"{self.rtsp_base}:8554/stream/{source_id}"
        self.get_logger().info(f"Connecting to RTSP stream: {rtsp_url}")

        # Open video capture
        try:
            cap = cv2.VideoCapture(rtsp_url)
            cap.set(cv2.CAP_PROP_BUFFERSIZE, 1)  # Reduce buffer to minimize latency
            self.captures[source_id] = cap

            if not cap.isOpened():
                self.get_logger().error(
                    f"Failed to open RTSP stream for source {source_id}"
                )
                return
        except Exception as e:
            self.get_logger().error(
                f"Error opening video capture for source {source_id}: {e}"
            )
            return

        frame_count = 0
        while self.running and rclpy.ok():
            try:
                ret, frame = cap.read()
                if not ret:
                    self.get_logger().warn(
                        f"Failed to read frame from source {source_id}"
                    )
                    time.sleep(0.1)
                    continue

                # Create header
                header = Header()
                header.stamp = self.get_clock().now().to_msg()
                header.frame_id = f"camera_{source_id}"

                # Publish raw image
                try:
                    img_msg = self.bridge.cv2_to_imgmsg(frame, encoding="bgr8")
                    img_msg.header = header
                    self.publishers[source_id]["raw"].publish(img_msg)
                except Exception as e:
                    self.get_logger().error(f"Error publishing raw image: {e}")

                # Publish compressed image
                try:
                    compressed_msg = CompressedImage()
                    compressed_msg.header = header
                    compressed_msg.format = "jpeg"
                    _, buffer = cv2.imencode(
                        ".jpg", frame, [cv2.IMWRITE_JPEG_QUALITY, 90]
                    )
                    compressed_msg.data = buffer.tobytes()
                    self.publishers[source_id]["compressed"].publish(compressed_msg)
                except Exception as e:
                    self.get_logger().error(f"Error publishing compressed image: {e}")

                frame_count += 1

                # Throttle to approximately 30 FPS
                time.sleep(0.033)

            except Exception as e:
                self.get_logger().error(
                    f"Error in stream loop for source {source_id}: {e}"
                )
                break

        cap.release()
        self.get_logger().info(f"Stopped streaming source {source_id}")

    def get_cached_frame(self, source_id: str) -> Optional[np.ndarray]:
        """Get the latest cached frame from the API"""
        try:
            response = requests.get(
                f"{self.api_url}/api/v1/rtsp/{source_id}/latest", timeout=5
            )
            if response.status_code == 200:
                data = response.json()
                if data["success"] and data["data"]:
                    # Decode base64 frame
                    frame_data = base64.b64decode(data["data"]["data"])
                    # Convert to numpy array
                    nparr = np.frombuffer(frame_data, np.uint8)
                    # Decode image
                    frame = cv2.imdecode(nparr, cv2.IMREAD_COLOR)
                    return frame
        except Exception as e:
            self.get_logger().error(f"Error getting cached frame: {e}")
        return None

    def run(self) -> None:
        """Main run loop"""
        self.get_logger().info("Starting Camera Stream Bridge...")

        # Discover available cameras
        sources = self.discover_cameras()
        if not sources:
            self.get_logger().warn("No camera sources found!")
            return

        self.get_logger().info(f"Found {len(sources)} camera sources")

        # Set up publishers and start streaming
        self.setup_publishers(sources)

    def cleanup(self) -> None:
        """Cleanup resources"""
        self.running = False
        for cap in self.captures.values():
            try:
                cap.release()
            except Exception as e:
                self.get_logger().error(f"Error releasing capture: {e}")

        self.get_logger().info("Camera Stream Bridge stopped")


class MultiCameraSync(Node):
    """Synchronized multi-camera publishing for stereo vision"""

    def __init__(self, camera_pairs: List[Tuple[str, str]], sync_tolerance_ms: int = 10):
        super().__init__('multi_camera_sync')
        
        self.camera_pairs = camera_pairs
        self.sync_tolerance_ms = sync_tolerance_ms
        self.bridge = CvBridge()
        self.frame_buffers: Dict[str, List[Tuple[np.ndarray, rclpy.time.Time]]] = {
            cam_id: [] for pair in camera_pairs for cam_id in pair
        }

        # Create synchronized publishers
        self.sync_publishers = []
        for i, (left_id, right_id) in enumerate(camera_pairs):
            pub = self.create_publisher(Image, f"/stereo_{i}/synchronized", 1)
            self.sync_publishers.append(pub)

    def add_frame(self, camera_id: str, frame: np.ndarray, timestamp: rclpy.time.Time) -> None:
        """Add a frame to the synchronization buffer"""
        self.frame_buffers[camera_id].append((frame, timestamp))

        # Keep only recent frames
        cutoff_time = timestamp - rclpy.duration.Duration(seconds=self.sync_tolerance_ms / 1000.0)
        self.frame_buffers[camera_id] = [
            (f, t) for f, t in self.frame_buffers[camera_id] if t > cutoff_time
        ]

    def find_synchronized_frames(self) -> List[Tuple[int, np.ndarray, np.ndarray, rclpy.time.Time]]:
        """Find synchronized frames across camera pairs"""
        synchronized = []

        for i, (left_id, right_id) in enumerate(self.camera_pairs):
            left_frames = self.frame_buffers[left_id]
            right_frames = self.frame_buffers[right_id]

            if not left_frames or not right_frames:
                continue

            # Find closest timestamp match
            for left_frame, left_time in left_frames:
                for right_frame, right_time in right_frames:
                    time_diff = abs((left_time - right_time).nanoseconds / 1e6)  # Convert to milliseconds
                    if time_diff <= self.sync_tolerance_ms:
                        synchronized.append((i, left_frame, right_frame, left_time))
                        break

        return synchronized

    def publish_synchronized(self) -> None:
        """Publish synchronized frame pairs"""
        synced = self.find_synchronized_frames()

        for i, left_frame, right_frame, timestamp in synced:
            try:
                # Create combined image (side by side)
                combined = np.hstack((left_frame, right_frame))

                # Convert to ROS message
                img_msg = self.bridge.cv2_to_imgmsg(combined, encoding="bgr8")
                img_msg.header.stamp = timestamp.to_msg()
                img_msg.header.frame_id = f"stereo_{i}"

                self.sync_publishers[i].publish(img_msg)
            except Exception as e:
                self.get_logger().error(f"Error publishing synchronized frames: {e}")


def main(args=None):
    """Main entry point"""
    rclpy.init(args=args)
    
    try:
        # Create and run the bridge
        bridge = CameraStreamBridge()
        
        # Set up executor for multi-threaded execution
        executor = MultiThreadedExecutor()
        executor.add_node(bridge)
        
        # Run the bridge
        bridge.run()
        
        try:
            executor.spin()
        finally:
            bridge.cleanup()
            bridge.destroy_node()
            
    except KeyboardInterrupt:
        pass
    except Exception as e:
        print(f"Unexpected error: {e}")
    finally:
        rclpy.shutdown()


if __name__ == "__main__":
    main()
