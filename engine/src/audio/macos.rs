//! macOS audio backend using AVFoundation
//!
//! Uses AVAudioPlayer for simple audio playback, which:
//! - Automatically uses the system default output device
//! - Respects system volume and output routing preferences
//! - Supports common audio formats (MP3, AAC, WAV, M4A, etc.)
//! - Handles audio session management automatically

use super::{AudioBackend, AudioError, AudioInfo, PlaybackState};
use objc::runtime::{Object, BOOL, NO, YES};
use objc::{msg_send, sel, sel_impl};
use std::ptr;

/// macOS audio backend using AVAudioPlayer
pub struct MacOSAudioBackend {
    /// AVAudioPlayer instance
    player: *mut Object,

    /// Audio metadata
    info: Option<AudioInfo>,

    /// Current playback state
    state: PlaybackState,

    /// Volume (0.0 - 1.0)
    volume: f32,

    /// Whether audio loops
    looping: bool,
}

// Safety: AVAudioPlayer is thread-safe when accessed properly
unsafe impl Send for MacOSAudioBackend {}

impl MacOSAudioBackend {
    /// Create a new macOS audio backend
    pub fn new() -> Self {
        Self {
            player: ptr::null_mut(),
            info: None,
            state: PlaybackState::Idle,
            volume: 1.0,
            looping: false,
        }
    }

    /// Create an NSString from a Rust string
    unsafe fn create_nsstring(s: &str) -> *mut Object {
        let ns_string: *mut Object = msg_send![class!(NSString), alloc];
        let ns_string: *mut Object = msg_send![ns_string, initWithBytes:s.as_ptr()
                                                                 length:s.len()
                                                               encoding:4u64]; // NSUTF8StringEncoding
        ns_string
    }

    /// Create NSURL from file path
    unsafe fn create_file_url(path: &str) -> Result<*mut Object, AudioError> {
        let ns_string = Self::create_nsstring(path);
        if ns_string.is_null() {
            return Err(AudioError::LoadError("Failed to create NSString".into()));
        }

        let ns_url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_string];
        let _: () = msg_send![ns_string, release];

        if ns_url.is_null() {
            return Err(AudioError::LoadError(format!(
                "Failed to create file URL: {}",
                path
            )));
        }

        Ok(ns_url)
    }

    /// Create NSURL from URL string
    unsafe fn create_http_url(url: &str) -> Result<*mut Object, AudioError> {
        let ns_string = Self::create_nsstring(url);
        if ns_string.is_null() {
            return Err(AudioError::LoadError("Failed to create NSString".into()));
        }

        let ns_url: *mut Object = msg_send![class!(NSURL), URLWithString: ns_string];
        let _: () = msg_send![ns_string, release];

        if ns_url.is_null() {
            return Err(AudioError::LoadError(format!(
                "Failed to create URL: {}",
                url
            )));
        }

        Ok(ns_url)
    }

    /// Download remote URL to temp file (AVAudioPlayer needs local files)
    fn download_to_temp(url: &str) -> Result<String, AudioError> {
        use std::io::Write;

        // Extract file extension from URL (AVAudioPlayer uses extension to determine format)
        let extension = url
            .split('?')
            .next() // Remove query string
            .and_then(|path| path.rsplit('.').next())
            .filter(|ext| matches!(ext.to_lowercase().as_str(), "mp3" | "m4a" | "aac" | "wav" | "aiff" | "caf" | "ogg" | "flac"))
            .unwrap_or("mp3"); // Default to mp3 if no valid extension found

        // Create temp file path with correct extension
        // Use timestamp for uniqueness when loading multiple audio files
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let file_name = format!("centered_audio_{}_{}.{}", std::process::id(), timestamp, extension);
        let temp_path = temp_dir.join(file_name);
        let temp_path_str = temp_path.to_string_lossy().to_string();

        unsafe {
            let ns_url = Self::create_http_url(url)?;

            // Use NSData's dataWithContentsOfURL - synchronous but works on both macOS and iOS
            let data: *mut Object = msg_send![class!(NSData), dataWithContentsOfURL: ns_url];

            if data.is_null() {
                return Err(AudioError::LoadError(format!("Failed to download audio from: {}", url)));
            }

            // Get data bytes
            let length: usize = msg_send![data, length];
            let bytes: *const u8 = msg_send![data, bytes];

            if length == 0 || bytes.is_null() {
                return Err(AudioError::LoadError("Downloaded empty data".into()));
            }

            // Check if response looks like HTML (error page) instead of audio
            let data_slice = std::slice::from_raw_parts(bytes, length);
            if length > 15 && data_slice.starts_with(b"<!DOCTYPE") || data_slice.starts_with(b"<html") || data_slice.starts_with(b"<HTML") {
                return Err(AudioError::LoadError(format!(
                    "URL returned HTML instead of audio data (length: {} bytes)",
                    length
                )));
            }

            // Write to temp file
            let mut file = std::fs::File::create(&temp_path)
                .map_err(|e| AudioError::LoadError(format!("Failed to create temp file: {}", e)))?;
            file.write_all(data_slice)
                .map_err(|e| AudioError::LoadError(format!("Failed to write temp file: {}", e)))?;

            Ok(temp_path_str)
        }
    }

    /// Create AVAudioPlayer from NSURL
    unsafe fn create_player_from_url(&mut self, ns_url: *mut Object) -> Result<(), AudioError> {
        // Release existing player if any
        if !self.player.is_null() {
            let _: () = msg_send![self.player, stop];
            let _: () = msg_send![self.player, release];
            self.player = ptr::null_mut();
        }

        // Create AVAudioPlayer
        let mut error: *mut Object = ptr::null_mut();
        let player: *mut Object = msg_send![class!(AVAudioPlayer), alloc];
        let player: *mut Object = msg_send![player, initWithContentsOfURL:ns_url error:&mut error];

        if player.is_null() || !error.is_null() {
            if !error.is_null() {
                let desc: *mut Object = msg_send![error, localizedDescription];
                if !desc.is_null() {
                    let utf8: *const i8 = msg_send![desc, UTF8String];
                    if !utf8.is_null() {
                        let error_str = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
                        return Err(AudioError::LoadError(format!(
                            "Failed to create audio player: {}",
                            error_str
                        )));
                    }
                }
            }
            return Err(AudioError::LoadError(
                "Failed to create AVAudioPlayer".into(),
            ));
        }

        // Prepare to play (preload buffers)
        let prepared: BOOL = msg_send![player, prepareToPlay];
        if prepared == NO {
            let _: () = msg_send![player, release];
            return Err(AudioError::LoadError("Failed to prepare audio".into()));
        }

        // Get audio info
        let duration: f64 = msg_send![player, duration];
        // AVAudioPlayer doesn't directly expose format details, but we can get them from settings
        let settings: *mut Object = msg_send![player, settings];
        let sample_rate: u32 = if !settings.is_null() {
            let sr_key =
                Self::create_nsstring("AVSampleRateKey");
            let sr_value: *mut Object = msg_send![settings, objectForKey: sr_key];
            let _: () = msg_send![sr_key, release];
            if !sr_value.is_null() {
                msg_send![sr_value, unsignedIntValue]
            } else {
                44100 // Default
            }
        } else {
            44100
        };

        let channels: u32 = msg_send![player, numberOfChannels];

        self.info = Some(AudioInfo {
            duration_ms: (duration * 1000.0) as u64,
            sample_rate,
            channels,
            is_stream: false,
        });

        // Set initial volume and looping
        let _: () = msg_send![player, setVolume: self.volume];
        let loop_count: i64 = if self.looping { -1 } else { 0 };
        let _: () = msg_send![player, setNumberOfLoops: loop_count];

        self.player = player;
        self.state = PlaybackState::Paused;

        Ok(())
    }
}

impl Default for MacOSAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacOSAudioBackend {
    fn drop(&mut self) {
        unsafe {
            if !self.player.is_null() {
                let _: () = msg_send![self.player, stop];
                let _: () = msg_send![self.player, release];
            }
        }
    }
}

impl AudioBackend for MacOSAudioBackend {
    fn load_file(&mut self, path: &str) -> Result<(), AudioError> {
        unsafe {
            let ns_url = Self::create_file_url(path)?;
            self.create_player_from_url(ns_url)
        }
    }

    fn load_url(&mut self, url: &str) -> Result<(), AudioError> {
        // AVAudioPlayer requires local files, so download remote URLs first
        if url.starts_with("http://") || url.starts_with("https://") {
            let temp_path = Self::download_to_temp(url)?;
            self.load_file(&temp_path)
        } else if url.starts_with("file://") {
            self.load_file(&url[7..])
        } else {
            // Assume it's a local path
            self.load_file(url)
        }
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn play(&mut self) -> Result<(), AudioError> {
        if self.player.is_null() {
            return Err(AudioError::NotLoaded);
        }

        unsafe {
            let success: BOOL = msg_send![self.player, play];
            if success == YES {
                self.state = PlaybackState::Playing;
                Ok(())
            } else {
                Err(AudioError::DecodeError("Failed to start playback".into()))
            }
        }
    }

    fn pause(&mut self) -> Result<(), AudioError> {
        if self.player.is_null() {
            return Err(AudioError::NotLoaded);
        }

        unsafe {
            let _: () = msg_send![self.player, pause];
            self.state = PlaybackState::Paused;
            Ok(())
        }
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        if self.player.is_null() {
            return Err(AudioError::NotLoaded);
        }

        unsafe {
            let _: () = msg_send![self.player, stop];
            // Reset to beginning
            let _: () = msg_send![self.player, setCurrentTime: 0.0f64];
            self.state = PlaybackState::Paused;
            Ok(())
        }
    }

    fn seek(&mut self, timestamp_ms: u64) -> Result<(), AudioError> {
        if self.player.is_null() {
            return Err(AudioError::NotLoaded);
        }

        unsafe {
            let time_seconds = timestamp_ms as f64 / 1000.0;
            let _: () = msg_send![self.player, setCurrentTime: time_seconds];
            Ok(())
        }
    }

    fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
        if !self.player.is_null() {
            unsafe {
                // -1 = infinite loops, 0 = no loops
                let loop_count: i64 = if looping { -1 } else { 0 };
                let _: () = msg_send![self.player, setNumberOfLoops: loop_count];
            }
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if !self.player.is_null() {
            unsafe {
                let _: () = msg_send![self.player, setVolume: self.volume];
            }
        }
    }

    fn volume(&self) -> f32 {
        self.volume
    }

    fn is_looping(&self) -> bool {
        self.looping
    }

    fn state(&self) -> PlaybackState {
        self.state
    }

    fn current_time_ms(&self) -> u64 {
        if self.player.is_null() {
            return 0;
        }

        unsafe {
            let current_time: f64 = msg_send![self.player, currentTime];
            (current_time * 1000.0) as u64
        }
    }

    fn update(&mut self) {
        if self.player.is_null() {
            return;
        }

        unsafe {
            let is_playing: BOOL = msg_send![self.player, isPlaying];

            // Check if we were playing but audio finished
            if self.state == PlaybackState::Playing && is_playing == NO {
                // Check if we've reached the end
                let current_time: f64 = msg_send![self.player, currentTime];
                let duration: f64 = msg_send![self.player, duration];

                // If we're at (or near) the end and not looping, we've finished
                if !self.looping && current_time >= duration - 0.1 {
                    self.state = PlaybackState::Ended;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = MacOSAudioBackend::new();
        assert_eq!(backend.state(), PlaybackState::Idle);
        assert_eq!(backend.volume(), 1.0);
        assert!(!backend.is_looping());
    }
}
