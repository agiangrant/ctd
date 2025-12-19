//! Video input (camera) capture
//!
//! Provides cross-platform video input capture using platform-specific backends.
//! On macOS/iOS, uses AVFoundation's AVCaptureSession for camera access.
//! Supports multiple simultaneous cameras.

/// Video input state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoInputState {
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

impl VideoInputState {
    pub fn as_i32(self) -> i32 {
        match self {
            VideoInputState::Idle => 0,
            VideoInputState::RequestingPermission => 1,
            VideoInputState::Ready => 2,
            VideoInputState::Capturing => 3,
            VideoInputState::Stopped => 4,
            VideoInputState::Error => 5,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => VideoInputState::Idle,
            1 => VideoInputState::RequestingPermission,
            2 => VideoInputState::Ready,
            3 => VideoInputState::Capturing,
            4 => VideoInputState::Stopped,
            _ => VideoInputState::Error,
        }
    }
}

/// Camera position (for devices with multiple cameras)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraPosition {
    /// Unspecified position
    Unspecified,
    /// Back-facing camera (typically on mobile devices)
    Back,
    /// Front-facing camera (selfie camera)
    Front,
    /// External camera (USB, etc.)
    External,
}

impl CameraPosition {
    pub fn as_i32(self) -> i32 {
        match self {
            CameraPosition::Unspecified => 0,
            CameraPosition::Back => 1,
            CameraPosition::Front => 2,
            CameraPosition::External => 3,
        }
    }
}

/// Video input configuration
#[derive(Debug, Clone)]
pub struct VideoInputConfig {
    /// Preferred width in pixels (0 for default)
    pub width: u32,
    /// Preferred height in pixels (0 for default)
    pub height: u32,
    /// Preferred frame rate (0 for default)
    pub frame_rate: u32,
    /// Preferred pixel format
    pub pixel_format: PixelFormat,
}

impl Default for VideoInputConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            frame_rate: 30,
            pixel_format: PixelFormat::BGRA,
        }
    }
}

/// Pixel format for video frames
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32-bit BGRA (compatible with most rendering)
    BGRA,
    /// 32-bit RGBA
    RGBA,
    /// YUV 4:2:0 (common camera format)
    YUV420,
    /// JPEG compressed
    JPEG,
}

/// Video input device information
#[derive(Debug, Clone)]
pub struct VideoInputDevice {
    /// Unique device identifier
    pub id: String,
    /// Human-readable device name
    pub name: String,
    /// Camera position (front, back, external)
    pub position: CameraPosition,
    /// Whether this is the default device
    pub is_default: bool,
    /// Supported resolutions
    pub resolutions: Vec<(u32, u32)>,
}

/// Video frame data
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Frame data (BGRA format by default)
    pub data: Vec<u8>,
    /// Timestamp in nanoseconds
    pub timestamp_ns: u64,
    /// Pixel format
    pub pixel_format: PixelFormat,
}

/// Video input error types
#[derive(Debug)]
pub enum VideoInputError {
    /// Permission denied
    PermissionDenied,
    /// Device not found
    DeviceNotFound,
    /// Device in use
    DeviceInUse,
    /// Invalid configuration
    InvalidConfig(String),
    /// Platform not supported
    UnsupportedPlatform,
    /// Other error
    Other(String),
}

impl std::fmt::Display for VideoInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoInputError::PermissionDenied => write!(f, "Camera permission denied"),
            VideoInputError::DeviceNotFound => write!(f, "Camera device not found"),
            VideoInputError::DeviceInUse => write!(f, "Camera device is in use"),
            VideoInputError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            VideoInputError::UnsupportedPlatform => write!(f, "Platform not supported"),
            VideoInputError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for VideoInputError {}

/// Callback for receiving video frames
pub type VideoFrameCallback = Box<dyn Fn(VideoFrame) + Send + 'static>;

/// Video input backend trait
pub trait VideoInputBackend: Send {
    /// Request camera permission (async on some platforms)
    fn request_permission(&mut self) -> Result<(), VideoInputError>;

    /// Check if permission was granted
    fn has_permission(&self) -> bool;

    /// List available video input devices (cameras)
    fn list_devices(&self) -> Result<Vec<VideoInputDevice>, VideoInputError>;

    /// Open a specific device (or default if None)
    fn open(&mut self, device_id: Option<&str>, config: &VideoInputConfig) -> Result<(), VideoInputError>;

    /// Start capturing video
    fn start(&mut self) -> Result<(), VideoInputError>;

    /// Stop capturing video
    fn stop(&mut self) -> Result<(), VideoInputError>;

    /// Close the device
    fn close(&mut self);

    /// Get current state
    fn state(&self) -> VideoInputState;

    /// Get current frame dimensions
    fn dimensions(&self) -> Option<(u32, u32)>;

    /// Set callback for video frames
    fn set_frame_callback(&mut self, callback: Option<VideoFrameCallback>);

    /// Get the latest frame (if available)
    fn latest_frame(&self) -> Option<VideoFrame>;
}

/// Cross-platform video input
pub struct VideoInput {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    backend: super::macos_input::MacOSVideoInput,

    #[cfg(target_os = "android")]
    backend: super::android_input::AndroidVideoInput,

    // Placeholder for unsupported platforms
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
    _phantom: std::marker::PhantomData<()>,
}

impl VideoInput {
    /// Create a new video input instance
    pub fn new() -> Self {
        Self {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            backend: super::macos_input::MacOSVideoInput::new(),
            #[cfg(target_os = "android")]
            backend: super::android_input::AndroidVideoInput::new(),
            #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
            _phantom: std::marker::PhantomData,
        }
    }

    /// Request camera permission
    pub fn request_permission(&mut self) -> Result<(), VideoInputError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.request_permission();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        Err(VideoInputError::UnsupportedPlatform)
    }

    /// Check if permission was granted
    pub fn has_permission(&self) -> bool {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.has_permission();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        false
    }

    /// List available video input devices
    pub fn list_devices(&self) -> Result<Vec<VideoInputDevice>, VideoInputError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.list_devices();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        Err(VideoInputError::UnsupportedPlatform)
    }

    /// Open a specific device (or default if None)
    pub fn open(&mut self, device_id: Option<&str>, config: &VideoInputConfig) -> Result<(), VideoInputError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.open(device_id, config);
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        {
            let _ = (device_id, config);
            Err(VideoInputError::UnsupportedPlatform)
        }
    }

    /// Start capturing video
    pub fn start(&mut self) -> Result<(), VideoInputError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.start();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        Err(VideoInputError::UnsupportedPlatform)
    }

    /// Stop capturing video
    pub fn stop(&mut self) -> Result<(), VideoInputError> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.stop();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        Err(VideoInputError::UnsupportedPlatform)
    }

    /// Close the device
    pub fn close(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        self.backend.close();
    }

    /// Get current state
    pub fn state(&self) -> VideoInputState {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.state();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        VideoInputState::Idle
    }

    /// Get current frame dimensions
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.dimensions();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        None
    }

    /// Set callback for video frames
    pub fn set_frame_callback(&mut self, callback: Option<VideoFrameCallback>) {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        self.backend.set_frame_callback(callback);
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        let _ = callback;
    }

    /// Get the latest frame
    pub fn latest_frame(&self) -> Option<VideoFrame> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        return self.backend.latest_frame();
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
        None
    }
}

impl Default for VideoInput {
    fn default() -> Self {
        Self::new()
    }
}
