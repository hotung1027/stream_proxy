//! ROS integration module for camera stream proxy
//!
//! This module provides ROS topic publishing capabilities for camera streams,
//! including both raw and compressed image formats.

pub mod ros_publisher;

pub use ros_publisher::{
    RosPublisher,
    RosPublisherConfig,
    MultiCameraSync,
    StreamPublisher,
}; 