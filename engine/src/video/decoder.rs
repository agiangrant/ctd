//! Video decoder abstraction
//!
//! Provides a common interface for platform-specific video decoders.

use super::{VideoDecoder, VideoError, VideoFrame, VideoInfo};

/// Create a decoder for the current platform
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn create_decoder_from_url(url: &str) -> Result<Box<dyn VideoDecoder>, VideoError> {
    use super::macos::MacOSVideoDecoder;
    let decoder = MacOSVideoDecoder::from_url(url)?;
    Ok(Box::new(decoder))
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn create_decoder_from_file(path: &str) -> Result<Box<dyn VideoDecoder>, VideoError> {
    use super::macos::MacOSVideoDecoder;
    let decoder = MacOSVideoDecoder::from_file(path)?;
    Ok(Box::new(decoder))
}

/// Create a decoder for Android
#[cfg(target_os = "android")]
pub fn create_decoder_from_url(url: &str) -> Result<Box<dyn VideoDecoder>, VideoError> {
    use super::android::AndroidVideoDecoder;
    let decoder = AndroidVideoDecoder::from_url(url)?;
    Ok(Box::new(decoder))
}

#[cfg(target_os = "android")]
pub fn create_decoder_from_file(path: &str) -> Result<Box<dyn VideoDecoder>, VideoError> {
    use super::android::AndroidVideoDecoder;
    let decoder = AndroidVideoDecoder::from_file(path)?;
    Ok(Box::new(decoder))
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
pub fn create_decoder_from_url(_url: &str) -> Result<Box<dyn VideoDecoder>, VideoError> {
    Err(VideoError::UnsupportedPlatform)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
pub fn create_decoder_from_file(_path: &str) -> Result<Box<dyn VideoDecoder>, VideoError> {
    Err(VideoError::UnsupportedPlatform)
}

/// A simple frame buffer decoder for raw frame input (video meetings, etc.)
pub struct FrameBufferDecoder {
    info: VideoInfo,
    frames: Vec<VideoFrame>,
    current_index: usize,
    current_time: u64,
}

impl FrameBufferDecoder {
    /// Create a new frame buffer decoder
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            info: VideoInfo {
                width,
                height,
                duration_ms: 0,
                frame_rate: 30.0,
                is_live: true,
            },
            frames: Vec::new(),
            current_index: 0,
            current_time: 0,
        }
    }

    /// Push a new frame into the buffer
    pub fn push_frame(&mut self, frame: VideoFrame) {
        // Update dimensions if they changed
        if frame.width != self.info.width || frame.height != self.info.height {
            self.info.width = frame.width;
            self.info.height = frame.height;
        }
        self.current_time = frame.timestamp_ms;

        // Keep only the latest frame for live streams
        self.frames.clear();
        self.frames.push(frame);
        self.current_index = 0;
    }

    /// Check if there's a frame available
    pub fn has_frame(&self) -> bool {
        !self.frames.is_empty()
    }
}

impl VideoDecoder for FrameBufferDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_frame(&mut self) -> Option<VideoFrame> {
        if self.current_index < self.frames.len() {
            let frame = self.frames[self.current_index].clone();
            self.current_index += 1;
            Some(frame)
        } else {
            None
        }
    }

    fn seek(&mut self, _timestamp_ms: u64) -> Result<(), VideoError> {
        // Seeking doesn't make sense for live streams
        Ok(())
    }

    fn has_more_frames(&self) -> bool {
        self.current_index < self.frames.len()
    }

    fn current_time_ms(&self) -> u64 {
        self.current_time
    }
}
