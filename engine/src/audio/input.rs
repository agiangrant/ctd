//! Audio input (microphone) capture
//!
//! Provides cross-platform audio input capture using platform-specific backends.
//! On macOS/iOS, uses AVFoundation's AVCaptureSession for microphone access.

use super::{AudioError, AudioInfo};

/// Audio input state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioInputState {
    /// Not initialized
    Idle,
    /// Requesting permissions
    RequestingPermission,
    /// Ready to capture
    Ready,
    /// Currently capturing
    Capturing,
    /// Stopped
    Stopped,
    /// Error occurred
    Error,
}

impl AudioInputState {
    pub fn as_i32(self) -> i32 {
        match self {
            AudioInputState::Idle => 0,
            AudioInputState::RequestingPermission => 1,
            AudioInputState::Ready => 2,
            AudioInputState::Capturing => 3,
            AudioInputState::Stopped => 4,
            AudioInputState::Error => 5,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => AudioInputState::Idle,
            1 => AudioInputState::RequestingPermission,
            2 => AudioInputState::Ready,
            3 => AudioInputState::Capturing,
            4 => AudioInputState::Stopped,
            _ => AudioInputState::Error,
        }
    }
}

/// Audio input configuration
#[derive(Debug, Clone)]
pub struct AudioInputConfig {
    /// Sample rate in Hz (default: 44100)
    pub sample_rate: u32,
    /// Number of channels (default: 1 for mono)
    pub channels: u32,
    /// Bits per sample (default: 16)
    pub bits_per_sample: u32,
    /// Buffer size in samples (default: 1024)
    pub buffer_size: u32,
}

impl Default for AudioInputConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 1,
            bits_per_sample: 16,
            buffer_size: 1024,
        }
    }
}

/// Audio input device information
#[derive(Debug, Clone)]
pub struct AudioInputDevice {
    /// Unique device identifier
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Whether this is the default device
    pub is_default: bool,
}

/// Callback for receiving audio samples
pub type AudioSampleCallback = Box<dyn Fn(&[f32], u64) + Send + 'static>;

/// Audio input backend trait
pub trait AudioInputBackend: Send {
    /// Request microphone permission (shows system dialog if needed)
    /// Returns Ok(()) if permission is granted.
    /// Returns Err with appropriate error if denied or restricted.
    fn request_permission(&mut self) -> Result<(), AudioError>;

    /// Check if permission was granted
    fn has_permission(&self) -> bool;

    /// List available audio input devices
    fn list_devices(&self) -> Result<Vec<AudioInputDevice>, AudioError>;

    /// Open a specific device (or default if None)
    fn open(&mut self, device_id: Option<&str>, config: &AudioInputConfig) -> Result<(), AudioError>;

    /// Start capturing audio
    fn start(&mut self) -> Result<(), AudioError>;

    /// Stop capturing audio
    fn stop(&mut self) -> Result<(), AudioError>;

    /// Close the device
    fn close(&mut self);

    /// Get current state
    fn state(&self) -> AudioInputState;

    /// Get audio info (after opening)
    fn info(&self) -> Option<&AudioInfo>;

    /// Set callback for audio samples
    fn set_sample_callback(&mut self, callback: Option<AudioSampleCallback>);

    /// Get current audio level (0.0 - 1.0, RMS)
    fn level(&self) -> f32;
}

/// Cross-platform audio input
pub struct AudioInput {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    backend: super::macos_input::MacOSAudioInput,

    #[cfg(target_os = "android")]
    backend: super::android_input::AndroidAudioInput,

    #[cfg(target_os = "linux")]
    backend: super::linux_input::LinuxAudioInput,

    // Placeholder for unsupported platforms
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
    _phantom: std::marker::PhantomData<()>,
}

impl AudioInput {
    /// Create a new audio input instance
    pub fn new() -> Self {
        Self {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            backend: super::macos_input::MacOSAudioInput::new(),
            #[cfg(target_os = "android")]
            backend: super::android_input::AndroidAudioInput::new(),
            #[cfg(target_os = "linux")]
            backend: super::linux_input::LinuxAudioInput::new(),
            #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
            _phantom: std::marker::PhantomData,
        }
    }

    /// Request microphone permission (shows system dialog if needed)
    pub fn request_permission(&mut self) -> Result<(), AudioError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.request_permission();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        Err(AudioError::UnsupportedPlatform)
    }

    /// Check if permission was granted
    pub fn has_permission(&self) -> bool {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.has_permission();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        false
    }

    /// List available audio input devices
    pub fn list_devices(&self) -> Result<Vec<AudioInputDevice>, AudioError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.list_devices();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        Err(AudioError::UnsupportedPlatform)
    }

    /// Open a specific device (or default if None)
    pub fn open(&mut self, device_id: Option<&str>, config: &AudioInputConfig) -> Result<(), AudioError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.open(device_id, config);
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        {
            let _ = (device_id, config);
            Err(AudioError::UnsupportedPlatform)
        }
    }

    /// Start capturing audio
    pub fn start(&mut self) -> Result<(), AudioError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.start();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        Err(AudioError::UnsupportedPlatform)
    }

    /// Stop capturing audio
    pub fn stop(&mut self) -> Result<(), AudioError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.stop();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        Err(AudioError::UnsupportedPlatform)
    }

    /// Close the device
    pub fn close(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        self.backend.close();
    }

    /// Get current state
    pub fn state(&self) -> AudioInputState {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.state();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        AudioInputState::Idle
    }

    /// Get audio info
    pub fn info(&self) -> Option<&AudioInfo> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.info();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        None
    }

    /// Set callback for audio samples
    pub fn set_sample_callback(&mut self, callback: Option<AudioSampleCallback>) {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        self.backend.set_sample_callback(callback);
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        let _ = callback;
    }

    /// Get current audio level (0.0 - 1.0)
    pub fn level(&self) -> f32 {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux"))]
        return self.backend.level();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux")))]
        0.0
    }
}

impl Default for AudioInput {
    fn default() -> Self {
        Self::new()
    }
}
