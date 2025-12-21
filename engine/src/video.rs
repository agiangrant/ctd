//! Video playback and input support
//!
//! This module provides video decoding, playback, and input functionality using
//! platform-native APIs for optimal performance:
//! - macOS/iOS: AVFoundation (hardware-accelerated H.264/HEVC/VP9, camera capture)
//! - Linux: GStreamer (planned)
//! - Windows: Media Foundation (planned)
//!
//! Supports:
//! - Video playback from files and URLs
//! - Camera capture with device enumeration
//! - Multiple simultaneous video inputs

pub mod decoder;
pub mod player;
pub mod input;

// macOS and iOS share AVFoundation for video
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod macos;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod macos_input;

// Android uses JNI to access Camera2 API and MediaCodec
#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub mod android_input;

// Linux uses GStreamer for video decoding and V4L2 for camera capture
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod linux_input;

use std::error::Error;
use std::fmt;

/// Video frame ready for GPU upload
#[derive(Clone)]
pub struct VideoFrame {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// RGBA pixel data (width * height * 4 bytes)
    pub data: Vec<u8>,
    /// Presentation timestamp in milliseconds
    pub timestamp_ms: u64,
}

impl VideoFrame {
    /// Create a new video frame
    pub fn new(width: u32, height: u32, data: Vec<u8>, timestamp_ms: u64) -> Self {
        Self {
            width,
            height,
            data,
            timestamp_ms,
        }
    }

    /// Create a black frame (for placeholders)
    pub fn black(width: u32, height: u32) -> Self {
        let data = vec![0u8; (width * height * 4) as usize];
        Self {
            width,
            height,
            data,
            timestamp_ms: 0,
        }
    }
}

/// Video metadata
#[derive(Clone, Debug)]
pub struct VideoInfo {
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Duration in milliseconds (0 for live streams)
    pub duration_ms: u64,
    /// Frame rate (frames per second)
    pub frame_rate: f32,
    /// Whether this is a live stream
    pub is_live: bool,
}

impl Default for VideoInfo {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            duration_ms: 0,
            frame_rate: 30.0,
            is_live: false,
        }
    }
}

/// Video playback state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PlaybackState {
    /// No video loaded
    Idle = 0,
    /// Video is loading/buffering
    Loading = 1,
    /// Video is playing
    Playing = 2,
    /// Video is paused
    Paused = 3,
    /// Video has ended
    Ended = 4,
    /// Error occurred
    Error = 5,
}

/// Video error types
#[derive(Debug)]
pub enum VideoError {
    /// Failed to load video from URL
    LoadError(String),
    /// Invalid video format
    FormatError(String),
    /// Decoder error
    DecodeError(String),
    /// Seek error
    SeekError(String),
    /// Platform not supported
    UnsupportedPlatform,
    /// Video not loaded
    NotLoaded,
}

impl fmt::Display for VideoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoError::LoadError(msg) => write!(f, "Failed to load video: {}", msg),
            VideoError::FormatError(msg) => write!(f, "Invalid video format: {}", msg),
            VideoError::DecodeError(msg) => write!(f, "Decoder error: {}", msg),
            VideoError::SeekError(msg) => write!(f, "Seek error: {}", msg),
            VideoError::UnsupportedPlatform => write!(f, "Video not supported on this platform"),
            VideoError::NotLoaded => write!(f, "No video loaded"),
        }
    }
}

impl Error for VideoError {}

/// Trait for video decoders
pub trait VideoDecoder: Send {
    /// Get video metadata
    fn info(&self) -> &VideoInfo;

    /// Get the next frame (returns None if no frame ready or end of video)
    fn next_frame(&mut self) -> Option<VideoFrame>;

    /// Seek to a specific timestamp
    fn seek(&mut self, timestamp_ms: u64) -> Result<(), VideoError>;

    /// Check if decoder has more frames
    fn has_more_frames(&self) -> bool;

    /// Get current playback position
    fn current_time_ms(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_frame_black() {
        let frame = VideoFrame::black(100, 100);
        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert_eq!(frame.data.len(), 100 * 100 * 4);
        assert!(frame.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_playback_state_values() {
        assert_eq!(PlaybackState::Idle as i32, 0);
        assert_eq!(PlaybackState::Loading as i32, 1);
        assert_eq!(PlaybackState::Playing as i32, 2);
        assert_eq!(PlaybackState::Paused as i32, 3);
        assert_eq!(PlaybackState::Ended as i32, 4);
        assert_eq!(PlaybackState::Error as i32, 5);
    }
}
