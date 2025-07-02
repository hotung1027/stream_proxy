#!/usr/bin/env python3
"""
ROS Bridge for Camera Stream Proxy

This script bridges camera streams from the Camera Stream Proxy to ROS topics.
It demonstrates how to integrate with ROS for robotics applications.
"""

import rospy
import cv2
import numpy as np
import requests
import base64
from sensor_msgs.msg import Image, CompressedImage
from std_msgs.msg import Header
from cv_bridge import CvBridge
import threading
import time


class CameraStreamBridge:
    """Bridge between Camera Stream Proxy and ROS"""

    def __init__(self, api_url="http://localhost:8080", rtsp_base="rtsp://localhost"):
        self.api_url = api_url
        self.rtsp_base = rtsp_base
        self.bridge = CvBridge()
        self.publishers = {}
        self.captures = {}
        self.running = True

        # Initialize ROS node
        rospy.init_node("camera_stream_bridge", anonymous=True)
        rospy.loginfo("Camera Stream Bridge initialized")

    def discover_cameras(self):
        """Discover available camera sources from the API"""
        try:
            response = requests.get(f"{self.api_url}/api/v1/sources")
            if response.status_code == 200:
                data = response.json()
                if data["success"]:
                    return data["data"]
            rospy.logwarn(f"Failed to get sources: {response.status_code}")
            return []
        except Exception as e:
            rospy.logerr(f"Error discovering cameras: {e}")
            return []

    def setup_publishers(self, sources):
        """Set up ROS publishers for each camera source"""
        for source in sources:
            source_id = source["id"]
            source_name = source["name"].replace(" ", "_").lower()

            # Create publishers for raw and compressed images
            raw_topic = f"/camera/{source_name}/image_raw"
            compressed_topic = f"/camera/{source_name}/compressed"

            self.publishers[source_id] = {
                "raw": rospy.Publisher(raw_topic, Image, queue_size=1),
                "compressed": rospy.Publisher(
                    compressed_topic, CompressedImage, queue_size=1
                ),
                "info": source,
            }

            rospy.loginfo(f"Created publishers for {source_name}:")
            rospy.loginfo(f"  Raw: {raw_topic}")
            rospy.loginfo(f"  Compressed: {compressed_topic}")

            # Start streaming thread for this source
            thread = threading.Thread(target=self.stream_camera, args=(source_id,))
            thread.daemon = True
            thread.start()

    def stream_camera(self, source_id):
        """Stream camera data to ROS topics"""
        # Build RTSP URL
        rtsp_url = f"{self.rtsp_base}:8554/stream/{source_id}"
        rospy.loginfo(f"Connecting to RTSP stream: {rtsp_url}")

        # Open video capture
        cap = cv2.VideoCapture(rtsp_url)
        self.captures[source_id] = cap

        if not cap.isOpened():
            rospy.logerr(f"Failed to open RTSP stream for source {source_id}")
            return

        frame_count = 0
        while self.running and not rospy.is_shutdown():
            ret, frame = cap.read()
            if not ret:
                rospy.logwarn(f"Failed to read frame from source {source_id}")
                time.sleep(0.1)
                continue

            # Create header
            header = Header()
            header.stamp = rospy.Time.now()
            header.frame_id = f"camera_{source_id}"
            header.seq = frame_count

            # Publish raw image
            try:
                img_msg = self.bridge.cv2_to_imgmsg(frame, encoding="bgr8")
                img_msg.header = header
                self.publishers[source_id]["raw"].publish(img_msg)
            except Exception as e:
                rospy.logerr(f"Error publishing raw image: {e}")

            # Publish compressed image
            try:
                compressed_msg = CompressedImage()
                compressed_msg.header = header
                compressed_msg.format = "jpeg"
                _, buffer = cv2.imencode(".jpg", frame, [cv2.IMWRITE_JPEG_QUALITY, 90])
                compressed_msg.data = buffer.tobytes()
                self.publishers[source_id]["compressed"].publish(compressed_msg)
            except Exception as e:
                rospy.logerr(f"Error publishing compressed image: {e}")

            frame_count += 1

            # Throttle to approximately 30 FPS
            time.sleep(0.033)

        cap.release()
        rospy.loginfo(f"Stopped streaming source {source_id}")

    def get_cached_frame(self, source_id):
        """Get the latest cached frame from the API"""
        try:
            response = requests.get(f"{self.api_url}/api/v1/rtsp/{source_id}/latest")
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
            rospy.logerr(f"Error getting cached frame: {e}")
        return None

    def run(self):
        """Main run loop"""
        rospy.loginfo("Starting Camera Stream Bridge...")

        # Discover available cameras
        sources = self.discover_cameras()
        if not sources:
            rospy.logwarn("No camera sources found!")
            return

        rospy.loginfo(f"Found {len(sources)} camera sources")

        # Set up publishers and start streaming
        self.setup_publishers(sources)

        # Keep running until shutdown
        rospy.spin()

        # Cleanup
        self.running = False
        for cap in self.captures.values():
            cap.release()

        rospy.loginfo("Camera Stream Bridge stopped")


class MultiCameraSync:
    """Synchronized multi-camera publishing for stereo vision"""

    def __init__(self, camera_pairs, sync_tolerance_ms=10):
        self.camera_pairs = camera_pairs
        self.sync_tolerance_ms = sync_tolerance_ms
        self.bridge = CvBridge()
        self.frame_buffers = {cam_id: [] for pair in camera_pairs for cam_id in pair}

        # Create synchronized publishers
        self.sync_publishers = []
        for i, (left_id, right_id) in enumerate(camera_pairs):
            pub = rospy.Publisher(f"/stereo_{i}/synchronized", Image, queue_size=1)
            self.sync_publishers.append(pub)

    def add_frame(self, camera_id, frame, timestamp):
        """Add a frame to the synchronization buffer"""
        self.frame_buffers[camera_id].append((frame, timestamp))

        # Keep only recent frames
        cutoff_time = timestamp - rospy.Duration(self.sync_tolerance_ms / 1000.0)
        self.frame_buffers[camera_id] = [
            (f, t) for f, t in self.frame_buffers[camera_id] if t > cutoff_time
        ]

    def find_synchronized_frames(self):
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
                    time_diff = abs((left_time - right_time).to_sec() * 1000)
                    if time_diff <= self.sync_tolerance_ms:
                        synchronized.append((i, left_frame, right_frame, left_time))
                        break

        return synchronized

    def publish_synchronized(self):
        """Publish synchronized frame pairs"""
        synced = self.find_synchronized_frames()

        for i, left_frame, right_frame, timestamp in synced:
            # Create combined image (side by side)
            combined = np.hstack((left_frame, right_frame))

            # Convert to ROS message
            img_msg = self.bridge.cv2_to_imgmsg(combined, encoding="bgr8")
            img_msg.header.stamp = timestamp
            img_msg.header.frame_id = f"stereo_{i}"

            self.sync_publishers[i].publish(img_msg)


def main():
    """Main entry point"""
    try:
        # Create and run the bridge
        bridge = CameraStreamBridge()
        bridge.run()
    except rospy.ROSInterruptException:
        rospy.loginfo("ROS interrupt received, shutting down")
    except Exception as e:
        rospy.logerr(f"Unexpected error: {e}")


if __name__ == "__main__":
    main()
