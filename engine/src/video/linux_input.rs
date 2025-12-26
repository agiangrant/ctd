//! Linux video input (camera) implementation using Video4Linux2
//!
//! Uses the v4l crate for camera capture on Linux.
//! Supports device enumeration, resolution configuration, and frame capture.

#![cfg(target_os = "linux")]

use super::input::{
    CameraPosition, PixelFormat, VideoFrame, VideoFrameCallback, VideoInputBackend,
    VideoInputConfig, VideoInputDevice, VideoInputError, VideoInputState,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::FourCC;

/// Linux video input using Video4Linux2
pub struct LinuxVideoInput {
    /// Current state
    state: VideoInputState,
    /// Device path (e.g., /dev/video0)
    device_path: Option<String>,
    /// Current configuration
    config: Option<VideoInputConfig>,
    /// Capture dimensions
    dimensions: Option<(u32, u32)>,
    /// Frame callback
    callback: Arc<Mutex<Option<VideoFrameCallback>>>,
    /// Latest captured frame
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
    /// Whether capture is active
    is_capturing: Arc<AtomicBool>,
    /// Stop signal for capture thread
    stop_signal: Arc<AtomicBool>,
    /// Capture thread handle
    capture_thread: Option<thread::JoinHandle<()>>,
}

impl LinuxVideoInput {
    pub fn new() -> Self {
        Self {
            state: VideoInputState::Idle,
            device_path: None,
            config: None,
            dimensions: None,
            callback: Arc::new(Mutex::new(None)),
            latest_frame: Arc::new(Mutex::new(None)),
            is_capturing: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            capture_thread: None,
        }
    }

    /// Enumerate available v4l2 devices
    fn enumerate_devices() -> Vec<VideoInputDevice> {
        let mut devices = Vec::new();
        let mut first = true;

        // Check /dev/video* devices
        for i in 0..16 {
            let path = format!("/dev/video{}", i);
            if Path::new(&path).exists() {
                if let Ok(dev) = Device::with_path(&path) {
                    // Try to get device capabilities
                    if let Ok(caps) = dev.query_caps() {
                        // Only include capture devices
                        if caps.capabilities.contains(v4l::capability::Flags::VIDEO_CAPTURE) {
                            let name = caps.card.clone();

                            // Try to detect camera position from name
                            let position = if name.to_lowercase().contains("front") {
                                CameraPosition::Front
                            } else if name.to_lowercase().contains("back") || name.to_lowercase().contains("rear") {
                                CameraPosition::Back
                            } else {
                                CameraPosition::External
                            };

                            // Get supported resolutions
                            let resolutions = Self::get_supported_resolutions(&dev);

                            devices.push(VideoInputDevice {
                                id: path.clone(),
                                name,
                                position,
                                is_default: first,
                                resolutions,
                            });
                            first = false;
                        }
                    }
                }
            }
        }

        devices
    }

    /// Get supported resolutions for a device
    fn get_supported_resolutions(dev: &Device) -> Vec<(u32, u32)> {
        let mut resolutions = Vec::new();

        // Common resolutions to try
        let common_resolutions = [
            (1920, 1080),
            (1280, 720),
            (800, 600),
            (640, 480),
            (352, 288),
            (320, 240),
        ];

        // Try YUYV format (most common for webcams)
        if let Ok(mut fmt) = dev.format() {
            for (width, height) in common_resolutions.iter() {
                fmt.width = *width;
                fmt.height = *height;
                if dev.set_format(&fmt).is_ok() {
                    if let Ok(actual) = dev.format() {
                        if actual.width == *width && actual.height == *height {
                            resolutions.push((*width, *height));
                        }
                    }
                }
            }
        }

        // If no resolutions found, return common defaults
        if resolutions.is_empty() {
            resolutions = vec![(1280, 720), (640, 480)];
        }

        resolutions
    }

    /// Convert YUYV (YUY2) to RGBA using fast integer math
    /// Uses fixed-point arithmetic (16.16) for ~5-10x speedup over floating point
    fn yuyv_to_rgba(yuyv: &[u8], width: u32, height: u32) -> Vec<u8> {
        let pixel_count = (width * height) as usize;
        let mut rgba = vec![0u8; pixel_count * 4];

        // Fixed-point coefficients (scaled by 65536 = 2^16)
        // R = Y + 1.402 * V  -> 1.402 * 65536 = 91881
        // G = Y - 0.344 * U - 0.714 * V -> 0.344 * 65536 = 22544, 0.714 * 65536 = 46793
        // B = Y + 1.772 * U  -> 1.772 * 65536 = 116129
        const V_TO_R: i32 = 91881;
        const U_TO_G: i32 = 22544;
        const V_TO_G: i32 = 46793;
        const U_TO_B: i32 = 116129;

        let yuyv_ptr = yuyv.as_ptr();
        let rgba_ptr = rgba.as_mut_ptr();

        // Process 2 pixels at a time (YUYV = 4 bytes -> 2 RGBA pixels = 8 bytes)
        let pair_count = pixel_count / 2;

        for i in 0..pair_count {
            // SAFETY: We know the indices are in bounds
            unsafe {
                let src = yuyv_ptr.add(i * 4);
                let dst = rgba_ptr.add(i * 8);

                let y0 = *src as i32;
                let u = *src.add(1) as i32 - 128;
                let y1 = *src.add(2) as i32;
                let v = *src.add(3) as i32 - 128;

                // Pre-calculate chroma contributions (shared between both pixels)
                let v_r = (V_TO_R * v) >> 16;
                let uv_g = ((U_TO_G * u) + (V_TO_G * v)) >> 16;
                let u_b = (U_TO_B * u) >> 16;

                // First pixel
                let r0 = (y0 + v_r).clamp(0, 255) as u8;
                let g0 = (y0 - uv_g).clamp(0, 255) as u8;
                let b0 = (y0 + u_b).clamp(0, 255) as u8;

                // Second pixel
                let r1 = (y1 + v_r).clamp(0, 255) as u8;
                let g1 = (y1 - uv_g).clamp(0, 255) as u8;
                let b1 = (y1 + u_b).clamp(0, 255) as u8;

                *dst = r0;
                *dst.add(1) = g0;
                *dst.add(2) = b0;
                *dst.add(3) = 255;
                *dst.add(4) = r1;
                *dst.add(5) = g1;
                *dst.add(6) = b1;
                *dst.add(7) = 255;
            }
        }

        rgba
    }

    /// Convert MJPEG to RGBA (using image crate)
    fn mjpeg_to_rgba(jpeg_data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
        use image::ImageDecoder;

        let decoder = match image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(jpeg_data)) {
            Ok(d) => d,
            Err(_) => return None,
        };

        let (width, height) = decoder.dimensions();

        let img = match image::load_from_memory_with_format(jpeg_data, image::ImageFormat::Jpeg) {
            Ok(img) => img,
            Err(_) => return None,
        };

        let rgba = img.to_rgba8();
        Some((rgba.into_raw(), width, height))
    }
}

impl Default for LinuxVideoInput {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoInputBackend for LinuxVideoInput {
    fn request_permission(&mut self) -> Result<(), VideoInputError> {
        // Linux doesn't have a permission system like iOS/Android
        // If we can access the video devices, we have permission
        let devices = Self::enumerate_devices();
        if devices.is_empty() {
            self.state = VideoInputState::Error;
            Err(VideoInputError::DeviceNotFound)
        } else {
            self.state = VideoInputState::Ready;
            Ok(())
        }
    }

    fn has_permission(&self) -> bool {
        // On Linux, permission is implicit if devices are accessible
        !Self::enumerate_devices().is_empty()
    }

    fn list_devices(&self) -> Result<Vec<VideoInputDevice>, VideoInputError> {
        Ok(Self::enumerate_devices())
    }

    fn open(&mut self, device_id: Option<&str>, config: &VideoInputConfig) -> Result<(), VideoInputError> {
        // Determine device path
        let device_path = if let Some(id) = device_id {
            id.to_string()
        } else {
            // Use first available device
            let devices = Self::enumerate_devices();
            devices
                .first()
                .ok_or(VideoInputError::DeviceNotFound)?
                .id
                .clone()
        };

        // Open device to verify it works
        let dev = Device::with_path(&device_path)
            .map_err(|e| VideoInputError::Other(format!("Failed to open device: {}", e)))?;

        // Set format
        let mut fmt = dev
            .format()
            .map_err(|e| VideoInputError::Other(format!("Failed to get format: {}", e)))?;

        fmt.width = config.width;
        fmt.height = config.height;

        // Try MJPEG first (better quality, compressed)
        fmt.fourcc = FourCC::new(b"MJPG");
        if dev.set_format(&fmt).is_err() {
            // Fall back to YUYV
            fmt.fourcc = FourCC::new(b"YUYV");
            dev.set_format(&fmt)
                .map_err(|e| VideoInputError::Other(format!("Failed to set format: {}", e)))?;
        }

        // Get actual format
        let actual = dev
            .format()
            .map_err(|e| VideoInputError::Other(format!("Failed to get format: {}", e)))?;

        self.device_path = Some(device_path);
        self.config = Some(config.clone());
        self.dimensions = Some((actual.width, actual.height));
        self.state = VideoInputState::Ready;

        Ok(())
    }

    fn start(&mut self) -> Result<(), VideoInputError> {
        if self.state != VideoInputState::Ready && self.state != VideoInputState::Stopped {
            return Err(VideoInputError::Other("Invalid state for start".to_string()));
        }

        let device_path = self
            .device_path
            .clone()
            .ok_or(VideoInputError::DeviceNotFound)?;
        let config = self
            .config
            .clone()
            .ok_or(VideoInputError::InvalidConfig("No configuration set".to_string()))?;

        let is_capturing = self.is_capturing.clone();
        let stop_signal = self.stop_signal.clone();
        let latest_frame = self.latest_frame.clone();
        let callback = self.callback.clone();

        // Reset stop signal
        stop_signal.store(false, Ordering::SeqCst);
        is_capturing.store(true, Ordering::SeqCst);

        // Start capture thread
        let handle = thread::spawn(move || {
            let dev = match Device::with_path(&device_path) {
                Ok(d) => d,
                Err(_) => {
                    is_capturing.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Set format
            let mut fmt = match dev.format() {
                Ok(f) => f,
                Err(_) => {
                    is_capturing.store(false, Ordering::SeqCst);
                    return;
                }
            };

            fmt.width = config.width;
            fmt.height = config.height;

            // Try MJPEG first
            let use_mjpeg;
            fmt.fourcc = FourCC::new(b"MJPG");
            if dev.set_format(&fmt).is_ok() {
                use_mjpeg = true;
            } else {
                fmt.fourcc = FourCC::new(b"YUYV");
                if dev.set_format(&fmt).is_err() {
                    is_capturing.store(false, Ordering::SeqCst);
                    return;
                }
                use_mjpeg = false;
            }

            let actual = match dev.format() {
                Ok(f) => f,
                Err(_) => {
                    is_capturing.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Create stream
            let mut stream = match Stream::with_buffers(&dev, Type::VideoCapture, 4) {
                Ok(s) => s,
                Err(_) => {
                    is_capturing.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Capture loop
            while !stop_signal.load(Ordering::SeqCst) {
                match stream.next() {
                    Ok((buf, _meta)) => {
                        let (rgba_data, width, height) = if use_mjpeg {
                            match Self::mjpeg_to_rgba(buf) {
                                Some(result) => result,
                                None => continue,
                            }
                        } else {
                            let rgba = Self::yuyv_to_rgba(buf, actual.width, actual.height);
                            (rgba, actual.width, actual.height)
                        };

                        let frame = VideoFrame {
                            width,
                            height,
                            data: rgba_data,
                            timestamp_ns: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_nanos() as u64)
                                .unwrap_or(0),
                            pixel_format: PixelFormat::RGBA,
                        };

                        // Store latest frame
                        if let Ok(mut guard) = latest_frame.lock() {
                            *guard = Some(frame.clone());
                        }

                        // Call callback
                        if let Ok(guard) = callback.lock() {
                            if let Some(ref cb) = *guard {
                                cb(frame);
                            }
                        }
                    }
                    Err(_) => {
                        // Brief pause on error
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }

            is_capturing.store(false, Ordering::SeqCst);
        });

        self.capture_thread = Some(handle);
        self.state = VideoInputState::Capturing;

        Ok(())
    }

    fn stop(&mut self) -> Result<(), VideoInputError> {
        if self.state != VideoInputState::Capturing {
            return Ok(());
        }

        // Signal stop
        self.stop_signal.store(true, Ordering::SeqCst);

        // Wait for thread to finish
        if let Some(handle) = self.capture_thread.take() {
            let _ = handle.join();
        }

        self.state = VideoInputState::Stopped;
        Ok(())
    }

    fn close(&mut self) {
        let _ = self.stop();
        self.device_path = None;
        self.config = None;
        self.dimensions = None;
        self.state = VideoInputState::Idle;

        // Clear latest frame
        if let Ok(mut guard) = self.latest_frame.lock() {
            *guard = None;
        }
    }

    fn state(&self) -> VideoInputState {
        self.state
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }

    fn set_frame_callback(&mut self, callback: Option<VideoFrameCallback>) {
        if let Ok(mut guard) = self.callback.lock() {
            *guard = callback;
        }
    }

    fn latest_frame(&self) -> Option<VideoFrame> {
        self.latest_frame.lock().ok()?.clone()
    }
}

impl LinuxVideoInput {
    /// Update method for compatibility with polling-based platforms.
    /// Linux uses a background thread with callbacks so this is a no-op.
    pub fn update(&mut self) {
        // Linux uses a capture thread with callbacks, no polling needed
    }
}

// v4l Device is not thread-safe, but we handle it in our own thread
unsafe impl Send for LinuxVideoInput {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_creation() {
        let input = LinuxVideoInput::new();
        assert_eq!(input.state(), VideoInputState::Idle);
    }

    #[test]
    fn test_enumerate_devices() {
        // This will return empty in CI without video devices
        let devices = LinuxVideoInput::enumerate_devices();
        // Just ensure it doesn't panic
        let _ = devices;
    }

    #[test]
    fn test_yuyv_to_rgba() {
        // Simple 2x1 pixel test
        let yuyv = [128, 128, 128, 128]; // Gray pixels
        let rgba = LinuxVideoInput::yuyv_to_rgba(&yuyv, 2, 1);
        assert_eq!(rgba.len(), 8); // 2 pixels * 4 bytes
        // Alpha should be 255
        assert_eq!(rgba[3], 255);
        assert_eq!(rgba[7], 255);
    }
}
