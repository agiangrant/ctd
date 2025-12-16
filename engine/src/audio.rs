//! Audio playback and input support
//!
//! This module provides audio playback and input functionality using platform-native APIs:
//! - macOS/iOS: AVFoundation / AVAudioEngine (hardware-accelerated, respects system devices)
//! - Linux: PulseAudio or PipeWire (planned)
//! - Windows: WASAPI (planned)
//!
//! The audio system supports:
//! - Background music and sound effects (playback)
//! - Microphone capture (input)
//! - Volume control (per-player and master)
//! - Looping playback
//! - Playback position seeking
//! - System default output/input device (automatic routing)
//! - Multiple simultaneous input devices

pub mod player;
pub mod input;

// macOS and iOS share AVFoundation for audio
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod macos;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod macos_input;

use std::error::Error;
use std::fmt;

/// Audio metadata
#[derive(Clone, Debug, Default)]
pub struct AudioInfo {
    /// Duration in milliseconds (0 for streams)
    pub duration_ms: u64,
    /// Sample rate in Hz (e.g., 44100, 48000)
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo)
    pub channels: u32,
    /// Whether this is a live/streaming source
    pub is_stream: bool,
}

/// Audio playback state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PlaybackState {
    /// No audio loaded
    Idle = 0,
    /// Audio is loading/buffering
    Loading = 1,
    /// Audio is playing
    Playing = 2,
    /// Audio is paused
    Paused = 3,
    /// Audio has finished playing
    Ended = 4,
    /// Error occurred
    Error = 5,
}

/// Audio error types
#[derive(Debug)]
pub enum AudioError {
    /// Failed to load audio from file/URL
    LoadError(String),
    /// Invalid audio format
    FormatError(String),
    /// Decoder error
    DecodeError(String),
    /// Seek error
    SeekError(String),
    /// Output device error
    DeviceError(String),
    /// Platform not supported
    UnsupportedPlatform,
    /// Audio not loaded
    NotLoaded,
    /// Permission denied
    PermissionDenied,
    /// Other error
    Other(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::LoadError(msg) => write!(f, "Failed to load audio: {}", msg),
            AudioError::FormatError(msg) => write!(f, "Invalid audio format: {}", msg),
            AudioError::DecodeError(msg) => write!(f, "Decoder error: {}", msg),
            AudioError::SeekError(msg) => write!(f, "Seek error: {}", msg),
            AudioError::DeviceError(msg) => write!(f, "Audio device error: {}", msg),
            AudioError::UnsupportedPlatform => write!(f, "Audio not supported on this platform"),
            AudioError::NotLoaded => write!(f, "No audio loaded"),
            AudioError::PermissionDenied => write!(f, "Permission denied"),
            AudioError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for AudioError {}

/// Trait for platform-specific audio backends
pub trait AudioBackend: Send {
    /// Load audio from a file path
    fn load_file(&mut self, path: &str) -> Result<(), AudioError>;

    /// Load audio from a URL
    fn load_url(&mut self, url: &str) -> Result<(), AudioError>;

    /// Get audio metadata (available after loading)
    fn info(&self) -> Option<&AudioInfo>;

    /// Start or resume playback
    fn play(&mut self) -> Result<(), AudioError>;

    /// Pause playback
    fn pause(&mut self) -> Result<(), AudioError>;

    /// Stop playback and reset position to beginning
    fn stop(&mut self) -> Result<(), AudioError>;

    /// Seek to a specific position in milliseconds
    fn seek(&mut self, timestamp_ms: u64) -> Result<(), AudioError>;

    /// Set whether the audio should loop
    fn set_looping(&mut self, looping: bool);

    /// Set the volume (0.0 = silent, 1.0 = full volume)
    fn set_volume(&mut self, volume: f32);

    /// Get current volume
    fn volume(&self) -> f32;

    /// Check if audio is looping
    fn is_looping(&self) -> bool;

    /// Get current playback state
    fn state(&self) -> PlaybackState;

    /// Get current playback position in milliseconds
    fn current_time_ms(&self) -> u64;

    /// Update playback state (call periodically, handles state transitions)
    fn update(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_state_values() {
        assert_eq!(PlaybackState::Idle as i32, 0);
        assert_eq!(PlaybackState::Loading as i32, 1);
        assert_eq!(PlaybackState::Playing as i32, 2);
        assert_eq!(PlaybackState::Paused as i32, 3);
        assert_eq!(PlaybackState::Ended as i32, 4);
        assert_eq!(PlaybackState::Error as i32, 5);
    }

    #[test]
    fn test_audio_info_default() {
        let info = AudioInfo::default();
        assert_eq!(info.duration_ms, 0);
        assert_eq!(info.sample_rate, 0);
        assert_eq!(info.channels, 0);
        assert!(!info.is_stream);
    }
}
