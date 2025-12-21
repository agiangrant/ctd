//! Audio player - manages playback across platform backends
//!
//! The AudioPlayer provides a unified interface for audio playback,
//! delegating to platform-specific backends while managing:
//! - Volume control (with master volume support)
//! - Looping behavior
//! - Playback state
//! - Time tracking

use super::{AudioBackend, AudioError, AudioInfo, PlaybackState};

#[cfg(any(target_os = "macos", target_os = "ios"))]
use super::macos::MacOSAudioBackend;

#[cfg(target_os = "android")]
use super::android::AndroidAudioPlayer;

#[cfg(target_os = "linux")]
use super::linux::LinuxAudioBackend;

/// Audio player that manages playback through platform backends
pub struct AudioPlayer {
    /// Platform-specific audio backend
    backend: Option<Box<dyn AudioBackend>>,

    /// Current playback state (cached for quick access)
    state: PlaybackState,

    /// Volume (0.0 - 1.0)
    volume: f32,

    /// Whether audio loops
    looping: bool,

    /// Error message if state is Error
    error_message: Option<String>,
}

impl AudioPlayer {
    /// Create a new audio player
    pub fn new() -> Self {
        Self {
            backend: None,
            state: PlaybackState::Idle,
            volume: 1.0,
            looping: false,
            error_message: None,
        }
    }

    /// Load audio from a file path
    pub fn load_file(&mut self, path: &str) -> Result<(), AudioError> {
        self.reset();
        self.state = PlaybackState::Loading;

        // Create platform-specific backend
        let mut backend = Self::create_backend()?;

        match backend.load_file(path) {
            Ok(()) => {
                backend.set_volume(self.volume);
                backend.set_looping(self.looping);
                self.backend = Some(backend);
                self.state = PlaybackState::Paused;
                Ok(())
            }
            Err(e) => {
                self.state = PlaybackState::Error;
                self.error_message = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Load audio from a URL
    pub fn load_url(&mut self, url: &str) -> Result<(), AudioError> {
        self.reset();
        self.state = PlaybackState::Loading;

        // Create platform-specific backend
        let mut backend = Self::create_backend()?;

        match backend.load_url(url) {
            Ok(()) => {
                backend.set_volume(self.volume);
                backend.set_looping(self.looping);
                self.backend = Some(backend);
                self.state = PlaybackState::Paused;
                Ok(())
            }
            Err(e) => {
                self.state = PlaybackState::Error;
                self.error_message = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Create platform-specific backend
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn create_backend() -> Result<Box<dyn AudioBackend>, AudioError> {
        Ok(Box::new(MacOSAudioBackend::new()))
    }

    #[cfg(target_os = "android")]
    fn create_backend() -> Result<Box<dyn AudioBackend>, AudioError> {
        Ok(Box::new(AndroidAudioPlayer::new()))
    }

    #[cfg(target_os = "linux")]
    fn create_backend() -> Result<Box<dyn AudioBackend>, AudioError> {
        Ok(Box::new(LinuxAudioBackend::new()))
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
    fn create_backend() -> Result<Box<dyn AudioBackend>, AudioError> {
        Err(AudioError::UnsupportedPlatform)
    }

    /// Start or resume playback
    pub fn play(&mut self) -> Result<(), AudioError> {
        if let Some(backend) = &mut self.backend {
            backend.play()?;
            self.state = PlaybackState::Playing;
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    /// Pause playback
    pub fn pause(&mut self) -> Result<(), AudioError> {
        if let Some(backend) = &mut self.backend {
            backend.pause()?;
            self.state = PlaybackState::Paused;
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    /// Stop playback and reset to beginning
    pub fn stop(&mut self) -> Result<(), AudioError> {
        if let Some(backend) = &mut self.backend {
            backend.stop()?;
            self.state = PlaybackState::Paused;
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    /// Seek to a specific position in milliseconds
    pub fn seek(&mut self, timestamp_ms: u64) -> Result<(), AudioError> {
        if let Some(backend) = &mut self.backend {
            backend.seek(timestamp_ms)
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    /// Set volume (0.0 - 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(backend) = &mut self.backend {
            backend.set_volume(self.volume);
        }
    }

    /// Get current volume
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Set whether audio should loop
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
        if let Some(backend) = &mut self.backend {
            backend.set_looping(looping);
        }
    }

    /// Check if audio is looping
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// Get current playback state
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Get audio metadata
    pub fn info(&self) -> Option<&AudioInfo> {
        self.backend.as_ref().and_then(|b| b.info())
    }

    /// Get current playback position in milliseconds
    pub fn current_time_ms(&self) -> u64 {
        self.backend
            .as_ref()
            .map(|b| b.current_time_ms())
            .unwrap_or(0)
    }

    /// Get error message if in error state
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Update playback state (call periodically)
    /// Returns true if state changed
    pub fn update(&mut self) -> bool {
        if let Some(backend) = &mut self.backend {
            backend.update();
            let new_state = backend.state();
            if new_state != self.state {
                self.state = new_state;
                return true;
            }
        }
        false
    }

    /// Reset player state
    fn reset(&mut self) {
        self.backend = None;
        self.state = PlaybackState::Idle;
        self.error_message = None;
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
