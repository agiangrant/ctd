//! Linux audio playback backend using GStreamer
//!
//! Uses GStreamer for audio playback on Linux, which provides robust
//! format support and hardware acceleration.

use super::{AudioBackend, AudioError, AudioInfo, PlaybackState};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Linux audio backend using GStreamer
pub struct LinuxAudioBackend {
    /// GStreamer pipeline
    pipeline: Option<gst::Element>,
    /// Audio metadata
    info: Option<AudioInfo>,
    /// Current playback state
    state: PlaybackState,
    /// Volume (0.0 - 1.0)
    volume: f32,
    /// Whether to loop
    looping: bool,
    /// Duration in milliseconds
    duration_ms: u64,
    /// Track if we've reached EOS
    reached_eos: Arc<AtomicBool>,
}

impl LinuxAudioBackend {
    pub fn new() -> Self {
        // Initialize GStreamer (safe to call multiple times)
        let _ = gst::init();

        Self {
            pipeline: None,
            info: None,
            state: PlaybackState::Idle,
            volume: 1.0,
            looping: false,
            duration_ms: 0,
            reached_eos: Arc::new(AtomicBool::new(false)),
        }
    }

    fn create_pipeline(&mut self, uri: &str) -> Result<(), AudioError> {
        // Create playbin for audio
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri)
            .build()
            .map_err(|e| AudioError::DeviceError(format!("Failed to create playbin: {}", e)))?;

        // Set volume
        playbin.set_property("volume", self.volume as f64);

        // Set to paused to preroll
        playbin
            .set_state(gst::State::Paused)
            .map_err(|e| AudioError::LoadError(format!("Failed to set paused: {:?}", e)))?;

        // Wait for state change
        let (result, state, _) = playbin.state(gst::ClockTime::from_seconds(5));
        if result.is_err() || state != gst::State::Paused {
            // Check for errors
            if let Some(bus) = playbin.bus() {
                while let Some(msg) = bus.pop() {
                    if let gst::MessageView::Error(err) = msg.view() {
                        let _ = playbin.set_state(gst::State::Null);
                        return Err(AudioError::LoadError(format!(
                            "Pipeline error: {}",
                            err.error()
                        )));
                    }
                }
            }
            let _ = playbin.set_state(gst::State::Null);
            return Err(AudioError::LoadError("Failed to preroll audio".to_string()));
        }

        // Get duration
        if let Some(duration) = playbin.query_duration::<gst::ClockTime>() {
            self.duration_ms = duration.mseconds();
        }

        // Try to get audio info from caps
        // For now, use reasonable defaults
        self.info = Some(AudioInfo {
            duration_ms: self.duration_ms,
            sample_rate: 44100, // Default, actual rate handled by GStreamer
            channels: 2,
            is_stream: uri.starts_with("http://") || uri.starts_with("https://"),
        });

        self.pipeline = Some(playbin);
        self.state = PlaybackState::Paused;
        self.reached_eos.store(false, Ordering::SeqCst);

        Ok(())
    }
}

impl Default for LinuxAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LinuxAudioBackend {
    fn drop(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            let _ = pipeline.set_state(gst::State::Null);
        }
    }
}

impl AudioBackend for LinuxAudioBackend {
    fn load_file(&mut self, path: &str) -> Result<(), AudioError> {
        // Reset state
        if let Some(pipeline) = self.pipeline.take() {
            let _ = pipeline.set_state(gst::State::Null);
        }
        self.state = PlaybackState::Loading;

        // Convert to absolute path if needed
        let absolute_path = if path.starts_with('/') {
            path.to_string()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path).to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string())
        };

        // Check file exists
        if !std::path::Path::new(&absolute_path).exists() {
            self.state = PlaybackState::Error;
            return Err(AudioError::LoadError(format!(
                "File not found: {}",
                absolute_path
            )));
        }

        let uri = format!("file://{}", absolute_path);
        self.create_pipeline(&uri)
    }

    fn load_url(&mut self, url: &str) -> Result<(), AudioError> {
        // Reset state
        if let Some(pipeline) = self.pipeline.take() {
            let _ = pipeline.set_state(gst::State::Null);
        }
        self.state = PlaybackState::Loading;

        // Handle file:// URLs
        let uri = if url.starts_with("file://") {
            url.to_string()
        } else if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            // Assume it's a file path
            return self.load_file(url);
        };

        self.create_pipeline(&uri)
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn play(&mut self) -> Result<(), AudioError> {
        // If we reached EOS and not looping, restart from beginning
        if self.reached_eos.load(Ordering::SeqCst) && !self.looping {
            self.seek(0)?;
            self.reached_eos.store(false, Ordering::SeqCst);
        }

        if let Some(pipeline) = &self.pipeline {
            pipeline
                .set_state(gst::State::Playing)
                .map_err(|e| AudioError::DeviceError(format!("Failed to play: {:?}", e)))?;
            self.state = PlaybackState::Playing;
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    fn pause(&mut self) -> Result<(), AudioError> {
        if let Some(pipeline) = &self.pipeline {
            pipeline
                .set_state(gst::State::Paused)
                .map_err(|e| AudioError::DeviceError(format!("Failed to pause: {:?}", e)))?;
            self.state = PlaybackState::Paused;
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        if let Some(pipeline) = &self.pipeline {
            // Seek to beginning and pause
            pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::ZERO,
                )
                .map_err(|e| AudioError::SeekError(format!("Seek failed: {}", e)))?;

            pipeline
                .set_state(gst::State::Paused)
                .map_err(|e| AudioError::DeviceError(format!("Failed to pause: {:?}", e)))?;

            self.state = PlaybackState::Paused;
            self.reached_eos.store(false, Ordering::SeqCst);
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    fn seek(&mut self, timestamp_ms: u64) -> Result<(), AudioError> {
        if let Some(pipeline) = &self.pipeline {
            pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::from_mseconds(timestamp_ms),
                )
                .map_err(|e| AudioError::SeekError(format!("Seek failed: {}", e)))?;
            Ok(())
        } else {
            Err(AudioError::NotLoaded)
        }
    }

    fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(pipeline) = &self.pipeline {
            pipeline.set_property("volume", self.volume as f64);
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
        if let Some(pipeline) = &self.pipeline {
            if let Some(pos) = pipeline.query_position::<gst::ClockTime>() {
                return pos.mseconds();
            }
        }
        0
    }

    fn update(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            // Check for EOS
            if let Some(bus) = pipeline.bus() {
                while let Some(msg) = bus.pop() {
                    match msg.view() {
                        gst::MessageView::Eos(_) => {
                            if self.looping {
                                // Seek back to start and continue playing
                                let _ = pipeline.seek_simple(
                                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                    gst::ClockTime::ZERO,
                                );
                            } else {
                                self.state = PlaybackState::Ended;
                                self.reached_eos.store(true, Ordering::SeqCst);
                            }
                        }
                        gst::MessageView::Error(err) => {
                            eprintln!("GStreamer audio error: {}", err.error());
                            self.state = PlaybackState::Error;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

unsafe impl Send for LinuxAudioBackend {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = LinuxAudioBackend::new();
        assert_eq!(backend.state(), PlaybackState::Idle);
        assert_eq!(backend.volume(), 1.0);
    }
}
