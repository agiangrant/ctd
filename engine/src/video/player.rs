//! Video player - manages playback timing and texture updates
//!
//! The player coordinates between the decoder (which extracts frames) and
//! the GPU texture (which displays frames). It handles:
//! - Frame timing and synchronization
//! - Play/pause/seek controls
//! - Looping behavior
//! - Texture management

use super::decoder::{create_decoder_from_file, create_decoder_from_url, FrameBufferDecoder};
use super::{PlaybackState, VideoDecoder, VideoError, VideoFrame, VideoInfo};
use std::time::Instant;

/// Video player that manages playback and texture streaming
pub struct VideoPlayer {
    /// The video decoder (platform-specific or frame buffer)
    decoder: Option<Box<dyn VideoDecoder>>,

    /// Frame buffer decoder for raw frame input
    frame_buffer: Option<FrameBufferDecoder>,

    /// GPU texture ID for the current frame
    texture_id: Option<u32>,

    /// Current playback state
    state: PlaybackState,

    /// Current playback position in milliseconds
    current_time_ms: u64,

    /// Time when playback started (for calculating elapsed time)
    playback_start: Option<Instant>,

    /// Position when playback started (for calculating current position)
    playback_start_pos: u64,

    /// Whether to loop the video
    looping: bool,

    /// Whether audio is muted
    muted: bool,

    /// Audio volume (0.0 - 1.0)
    volume: f32,

    /// The most recent decoded frame
    current_frame: Option<VideoFrame>,

    /// Whether we need to upload a new frame to GPU
    frame_dirty: bool,

    /// Error message if state is Error
    error_message: Option<String>,
}

impl VideoPlayer {
    /// Create a new video player
    pub fn new() -> Self {
        Self {
            decoder: None,
            frame_buffer: None,
            texture_id: None,
            state: PlaybackState::Idle,
            current_time_ms: 0,
            playback_start: None,
            playback_start_pos: 0,
            looping: false,
            muted: false,
            volume: 1.0,
            current_frame: None,
            frame_dirty: false,
            error_message: None,
        }
    }

    /// Load video from a URL
    pub fn load_url(&mut self, url: &str) -> Result<(), VideoError> {
        self.reset();
        self.state = PlaybackState::Loading;

        match create_decoder_from_url(url) {
            Ok(decoder) => {
                self.decoder = Some(decoder);
                self.state = PlaybackState::Paused;
                // Decode first frame for thumbnail
                self.decode_next_frame();
                Ok(())
            }
            Err(e) => {
                self.state = PlaybackState::Error;
                self.error_message = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Load video from a file path
    pub fn load_file(&mut self, path: &str) -> Result<(), VideoError> {
        self.reset();
        self.state = PlaybackState::Loading;

        match create_decoder_from_file(path) {
            Ok(decoder) => {
                self.decoder = Some(decoder);
                self.state = PlaybackState::Paused;
                // Decode first frame for thumbnail
                self.decode_next_frame();
                Ok(())
            }
            Err(e) => {
                self.state = PlaybackState::Error;
                self.error_message = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Initialize for raw frame input (video meetings)
    pub fn init_frame_buffer(&mut self, width: u32, height: u32) {
        self.reset();
        self.frame_buffer = Some(FrameBufferDecoder::new(width, height));
        self.state = PlaybackState::Playing;
    }

    /// Push a raw frame (for video meetings)
    pub fn push_frame(&mut self, frame: VideoFrame) {
        if let Some(fb) = &mut self.frame_buffer {
            fb.push_frame(frame.clone());
            self.current_frame = Some(frame);
            self.frame_dirty = true;
            self.state = PlaybackState::Playing;
        }
    }

    /// Start or resume playback
    pub fn play(&mut self) {
        if self.decoder.is_some() || self.frame_buffer.is_some() {
            if self.state == PlaybackState::Ended && self.looping {
                // Restart from beginning
                if let Err(e) = self.seek(0) {
                    eprintln!("Failed to seek to start: {}", e);
                }
            }
            self.state = PlaybackState::Playing;
            self.playback_start = Some(Instant::now());
            self.playback_start_pos = self.current_time_ms;
        }
    }

    /// Pause playback
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
            // Update current time before stopping
            if let Some(start) = self.playback_start {
                self.current_time_ms =
                    self.playback_start_pos + start.elapsed().as_millis() as u64;
            }
            self.playback_start = None;
        }
    }

    /// Seek to a specific position
    pub fn seek(&mut self, timestamp_ms: u64) -> Result<(), VideoError> {
        if let Some(decoder) = &mut self.decoder {
            decoder.seek(timestamp_ms)?;
            self.current_time_ms = timestamp_ms;
            self.playback_start_pos = timestamp_ms;
            if self.state == PlaybackState::Playing {
                self.playback_start = Some(Instant::now());
            }
            // Decode frame at new position
            self.decode_next_frame();
            Ok(())
        } else {
            Err(VideoError::NotLoaded)
        }
    }

    /// Set looping behavior
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    /// Set muted state
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Set volume (0.0 - 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Update playback state (call each frame)
    ///
    /// Returns true if a new frame is ready for upload
    pub fn update(&mut self) -> bool {
        if self.state != PlaybackState::Playing {
            return self.frame_dirty;
        }

        // Calculate current playback position
        if let Some(start) = self.playback_start {
            self.current_time_ms = self.playback_start_pos + start.elapsed().as_millis() as u64;
        }

        // Check if we need a new frame
        if let Some(decoder) = &self.decoder {
            let frame_time = if decoder.info().frame_rate > 0.0 {
                (1000.0 / decoder.info().frame_rate) as u64
            } else {
                33 // ~30fps default
            };

            // Decode frames until we catch up to current time
            let current_frame_time = self
                .current_frame
                .as_ref()
                .map(|f| f.timestamp_ms)
                .unwrap_or(0);
            if self.current_time_ms >= current_frame_time + frame_time {
                self.decode_next_frame();
            }
        }

        // Handle frame buffer (live streams)
        if let Some(fb) = &mut self.frame_buffer {
            if fb.has_frame() {
                if let Some(frame) = fb.next_frame() {
                    self.current_frame = Some(frame);
                    self.frame_dirty = true;
                }
            }
        }

        self.frame_dirty
    }

    /// Get the current frame if one is ready
    pub fn take_frame(&mut self) -> Option<VideoFrame> {
        if self.frame_dirty {
            self.frame_dirty = false;
            self.current_frame.clone()
        } else {
            None
        }
    }

    /// Get the current texture ID
    pub fn texture_id(&self) -> Option<u32> {
        self.texture_id
    }

    /// Set the texture ID (called by renderer after upload)
    pub fn set_texture_id(&mut self, id: u32) {
        self.texture_id = Some(id);
    }

    /// Get playback state
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Get video info
    pub fn info(&self) -> Option<VideoInfo> {
        if let Some(decoder) = &self.decoder {
            Some(decoder.info().clone())
        } else if let Some(fb) = &self.frame_buffer {
            Some(fb.info().clone())
        } else {
            None
        }
    }

    /// Get current playback position
    pub fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }

    /// Get error message
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Check if looping
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// Check if muted
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Get volume
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Decode the next frame from the decoder
    fn decode_next_frame(&mut self) {
        if let Some(decoder) = &mut self.decoder {
            if let Some(frame) = decoder.next_frame() {
                self.current_frame = Some(frame);
                self.frame_dirty = true;
            } else if !decoder.has_more_frames() {
                // End of video
                if self.looping {
                    // Seek to beginning and continue
                    if decoder.seek(0).is_ok() {
                        self.current_time_ms = 0;
                        self.playback_start_pos = 0;
                        self.playback_start = Some(Instant::now());
                        // Try to get first frame
                        if let Some(frame) = decoder.next_frame() {
                            self.current_frame = Some(frame);
                            self.frame_dirty = true;
                        }
                    }
                } else {
                    self.state = PlaybackState::Ended;
                    self.playback_start = None;
                }
            }
        }
    }

    /// Reset player state
    fn reset(&mut self) {
        self.decoder = None;
        self.frame_buffer = None;
        self.texture_id = None;
        self.state = PlaybackState::Idle;
        self.current_time_ms = 0;
        self.playback_start = None;
        self.playback_start_pos = 0;
        self.current_frame = None;
        self.frame_dirty = false;
        self.error_message = None;
    }
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}
