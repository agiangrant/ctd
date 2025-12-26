//! Linux audio input (microphone) backend using cpal
//!
//! Uses cpal for cross-platform audio I/O, which supports ALSA, PulseAudio,
//! and PipeWire on Linux.

use super::input::{AudioInputBackend, AudioInputConfig, AudioInputDevice, AudioInputState, AudioSampleCallback};
use super::{AudioError, AudioInfo};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Linux audio input using cpal
pub struct LinuxAudioInput {
    /// cpal host
    host: Host,
    /// Selected device
    device: Option<Device>,
    /// Active input stream
    stream: Option<Stream>,
    /// Current state
    state: AudioInputState,
    /// Audio info
    info: Option<AudioInfo>,
    /// Sample callback
    callback: Arc<Mutex<Option<AudioSampleCallback>>>,
    /// Current audio level (RMS, 0-1000 scaled to avoid floats in atomic)
    level: Arc<AtomicU32>,
    /// Stream configuration
    config: Option<StreamConfig>,
}

impl LinuxAudioInput {
    pub fn new() -> Self {
        let host = cpal::default_host();

        Self {
            host,
            device: None,
            stream: None,
            state: AudioInputState::Idle,
            info: None,
            callback: Arc::new(Mutex::new(None)),
            level: Arc::new(AtomicU32::new(0)),
            config: None,
        }
    }
}

impl Default for LinuxAudioInput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioInputBackend for LinuxAudioInput {
    fn request_permission(&mut self) -> Result<(), AudioError> {
        // Linux doesn't have a permission system for audio like iOS/Android
        // If we can access the audio devices, we have permission
        match self.host.default_input_device() {
            Some(_) => {
                self.state = AudioInputState::Ready;
                Ok(())
            }
            None => {
                self.state = AudioInputState::Error;
                Err(AudioError::DeviceError("No audio input devices available".to_string()))
            }
        }
    }

    fn has_permission(&self) -> bool {
        // On Linux, permission is implicit if devices are accessible
        self.host.default_input_device().is_some()
    }

    fn list_devices(&self) -> Result<Vec<AudioInputDevice>, AudioError> {
        let default_device = self.host.default_input_device();
        let default_name = default_device.as_ref().and_then(|d| d.name().ok());

        let devices = self.host.input_devices()
            .map_err(|e| AudioError::DeviceError(format!("Failed to enumerate devices: {}", e)))?;

        let mut result = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                let is_default = default_name.as_ref().map(|d| d == &name).unwrap_or(false);
                result.push(AudioInputDevice {
                    id: name.clone(),
                    name,
                    is_default,
                });
            }
        }

        Ok(result)
    }

    fn open(&mut self, device_id: Option<&str>, config: &AudioInputConfig) -> Result<(), AudioError> {
        // Find the device
        let device = if let Some(id) = device_id {
            // Find specific device by name
            let devices = self.host.input_devices()
                .map_err(|e| AudioError::DeviceError(format!("Failed to enumerate devices: {}", e)))?;

            let mut found = None;
            for d in devices {
                if let Ok(name) = d.name() {
                    if name == id {
                        found = Some(d);
                        break;
                    }
                }
            }
            found.ok_or_else(|| AudioError::DeviceError(format!("Device '{}' not found", id)))?
        } else {
            // Use default device
            self.host.default_input_device()
                .ok_or_else(|| AudioError::DeviceError("No default input device".to_string()))?
        };

        // Configure the stream
        let stream_config = StreamConfig {
            channels: config.channels as u16,
            sample_rate: cpal::SampleRate(config.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(config.buffer_size),
        };

        self.device = Some(device);
        self.config = Some(stream_config);
        self.info = Some(AudioInfo {
            duration_ms: 0, // Live input
            sample_rate: config.sample_rate,
            channels: config.channels,
            is_stream: true,
        });
        self.state = AudioInputState::Ready;

        Ok(())
    }

    fn start(&mut self) -> Result<(), AudioError> {
        let device = self.device.as_ref()
            .ok_or_else(|| AudioError::DeviceError("No device opened".to_string()))?;

        let config = self.config.clone()
            .ok_or_else(|| AudioError::DeviceError("No configuration set".to_string()))?;

        let callback = self.callback.clone();
        let level = self.level.clone();

        // Create the input stream
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Calculate RMS level
                let sum: f32 = data.iter().map(|&s| s * s).sum();
                let rms = (sum / data.len() as f32).sqrt();
                // Scale to 0-1000 for atomic storage
                let level_scaled = (rms.min(1.0) * 1000.0) as u32;
                level.store(level_scaled, Ordering::Relaxed);

                // Get timestamp (approximate)
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0);

                // Call the user callback
                if let Ok(guard) = callback.lock() {
                    if let Some(cb) = guard.as_ref() {
                        cb(data, timestamp);
                    }
                }
            },
            move |err| {
                eprintln!("Audio input error: {}", err);
            },
            None, // No timeout
        ).map_err(|e| AudioError::DeviceError(format!("Failed to build stream: {}", e)))?;

        stream.play()
            .map_err(|e| AudioError::DeviceError(format!("Failed to start stream: {}", e)))?;

        self.stream = Some(stream);
        self.state = AudioInputState::Capturing;

        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        if let Some(stream) = &self.stream {
            stream.pause()
                .map_err(|e| AudioError::DeviceError(format!("Failed to pause stream: {}", e)))?;
        }
        self.state = AudioInputState::Stopped;
        Ok(())
    }

    fn close(&mut self) {
        self.stream = None;
        self.device = None;
        self.config = None;
        self.state = AudioInputState::Idle;
        self.level.store(0, Ordering::Relaxed);
    }

    fn state(&self) -> AudioInputState {
        self.state
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn set_sample_callback(&mut self, callback: Option<AudioSampleCallback>) {
        if let Ok(mut guard) = self.callback.lock() {
            *guard = callback;
        }
    }

    fn level(&self) -> f32 {
        self.level.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

impl LinuxAudioInput {
    /// Update method for compatibility with polling-based platforms.
    /// Linux uses cpal callbacks so this is a no-op, but must exist for the trait.
    pub fn update(&mut self) {
        // cpal uses callbacks, no polling needed
    }
}

// cpal Stream is Send but not Sync - we handle this carefully
unsafe impl Send for LinuxAudioInput {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_creation() {
        let input = LinuxAudioInput::new();
        assert_eq!(input.state(), AudioInputState::Idle);
    }

    #[test]
    fn test_list_devices() {
        let input = LinuxAudioInput::new();
        // This may return empty list in CI without audio devices
        let _ = input.list_devices();
    }
}
