//! Windows video input (camera) using Media Foundation
//!
//! Uses IMFSourceReader for camera capture with support for device enumeration
//! and multiple camera selection. Capture runs on a background thread to avoid
//! blocking the UI thread.

use super::input::{
    CameraPosition, PixelFormat, VideoFrame, VideoFrameCallback, VideoInputBackend,
    VideoInputConfig, VideoInputDevice, VideoInputError, VideoInputState,
};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use windows::core::{Interface, PWSTR};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::*;

/// Ensure COM and Media Foundation are initialized for this thread.
/// These are safe to call multiple times per thread.
fn ensure_mf_initialized() -> std::result::Result<(), VideoInputError> {
    unsafe {
        // Initialize COM - safe to call multiple times per thread
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        // Initialize Media Foundation - also safe to call multiple times
        // It maintains a reference count internally
        MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| {
            VideoInputError::Other(format!("Failed to initialize Media Foundation: {:?}", e))
        })?;
    }
    Ok(())
}

/// State constants for atomic state
const STATE_IDLE: u8 = 0;
const STATE_READY: u8 = 1;
const STATE_CAPTURING: u8 = 2;
const STATE_STOPPED: u8 = 3;

/// Device info needed to open a camera on the capture thread
#[derive(Clone)]
struct DeviceOpenRequest {
    device_id: Option<String>,
}

/// Device info that persists across stop/start cycles
#[derive(Clone)]
struct DeviceConfig {
    device_id: Option<String>,
}

/// Windows video input using Media Foundation with background capture thread
pub struct WindowsVideoInput {
    /// Current state (atomic for thread-safe access)
    state: Arc<AtomicU8>,
    /// Permission granted (Windows doesn't require explicit permission for cameras)
    permission_granted: Arc<AtomicBool>,
    /// Current dimensions
    dimensions: Arc<Mutex<Option<(u32, u32)>>>,
    /// Frame callback
    frame_callback: Option<VideoFrameCallback>,
    /// Latest captured frame (shared with capture thread)
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
    /// Signal to stop the capture thread
    stop_signal: Arc<AtomicBool>,
    /// Capture thread handle
    capture_thread: Option<JoinHandle<()>>,
    /// Request to open a device (set before starting thread)
    open_request: Arc<Mutex<Option<DeviceOpenRequest>>>,
    /// Persistent device config (survives stop/start cycles)
    device_config: Option<DeviceConfig>,
}

// Note: We use Arc for all shared state between threads
unsafe impl Send for WindowsVideoInput {}

impl WindowsVideoInput {
    pub fn new() -> Self {
        let _ = ensure_mf_initialized();

        Self {
            state: Arc::new(AtomicU8::new(STATE_IDLE)),
            permission_granted: Arc::new(AtomicBool::new(true)), // Windows doesn't require explicit camera permission
            dimensions: Arc::new(Mutex::new(None)),
            frame_callback: None,
            latest_frame: Arc::new(Mutex::new(None)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            capture_thread: None,
            open_request: Arc::new(Mutex::new(None)),
            device_config: None,
        }
    }

    /// Called from main thread - just returns the latest frame without blocking
    pub fn update(&mut self) {
        // No-op on main thread - capture happens on background thread
        // The latest_frame is updated by the capture thread
    }

    /// Background capture thread function
    fn capture_thread_fn(
        state: Arc<AtomicU8>,
        dimensions: Arc<Mutex<Option<(u32, u32)>>>,
        latest_frame: Arc<Mutex<Option<VideoFrame>>>,
        stop_signal: Arc<AtomicBool>,
        open_request: Arc<Mutex<Option<DeviceOpenRequest>>>,
    ) {
        // Initialize COM/MF for this thread
        if ensure_mf_initialized().is_err() {
            state.store(STATE_IDLE, Ordering::SeqCst);
            return;
        }

        // Get the open request
        let request = match open_request.lock().ok().and_then(|mut r| r.take()) {
            Some(r) => r,
            None => {
                state.store(STATE_IDLE, Ordering::SeqCst);
                return;
            }
        };

        // Open the camera
        let (source_reader, dims) = match unsafe { Self::open_camera_on_thread(&request.device_id) } {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Failed to open camera: {:?}", e);
                state.store(STATE_IDLE, Ordering::SeqCst);
                return;
            }
        };

        // Store dimensions
        if let Ok(mut d) = dimensions.lock() {
            *d = Some(dims);
        }

        // Update state to capturing
        state.store(STATE_CAPTURING, Ordering::SeqCst);

        // Reusable RGBA buffer
        let mut rgba_buffer: Vec<u8> = Vec::with_capacity((dims.0 * dims.1 * 4) as usize);
        rgba_buffer.resize((dims.0 * dims.1 * 4) as usize, 0);

        // Capture loop
        while !stop_signal.load(Ordering::SeqCst) {
            unsafe {
                let mut stream_index = 0u32;
                let mut flags = 0u32;
                let mut timestamp = 0i64;
                let mut sample: Option<IMFSample> = None;

                // Read one frame - this blocks until a frame is available
                // But that's OK since we're on a background thread
                let result = source_reader.ReadSample(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    0,
                    Some(&mut stream_index),
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                );

                if result.is_err() {
                    // Small sleep on error to avoid spinning
                    thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }

                // Check for end of stream
                if (flags as i32) & MF_SOURCE_READERF_ENDOFSTREAM.0 != 0 {
                    break;
                }

                if let Some(sample) = sample {
                    if let Some(frame) = Self::process_sample_static(&sample, dims, &mut rgba_buffer) {
                        // Store latest frame
                        if let Ok(mut latest) = latest_frame.lock() {
                            *latest = Some(frame);
                        }
                    }
                }
            }
        }

        // Clean up
        state.store(STATE_STOPPED, Ordering::SeqCst);
    }

    /// Open camera and return source reader - called on capture thread
    unsafe fn open_camera_on_thread(
        device_id: &Option<String>,
    ) -> Result<(IMFSourceReader, (u32, u32)), VideoInputError> {
        // Create attributes for video capture device enumeration
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes as *mut _, 1).map_err(|e| {
            VideoInputError::Other(format!("Failed to create attributes: {:?}", e))
        })?;
        let attributes = attributes.ok_or_else(|| {
            VideoInputError::Other("No attributes returned".to_string())
        })?;

        attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .map_err(|e| {
                VideoInputError::Other(format!("Failed to set source type: {:?}", e))
            })?;

        // Enumerate all video capture devices
        let mut count = 0u32;
        let mut device_sources: *mut Option<IMFActivate> = ptr::null_mut();

        MFEnumDeviceSources(&attributes, &mut device_sources, &mut count).map_err(|e| {
            VideoInputError::Other(format!("Failed to enumerate devices: {:?}", e))
        })?;

        if count == 0 || device_sources.is_null() {
            return Err(VideoInputError::DeviceNotFound);
        }

        // Find and open the requested device
        let mut last_error = VideoInputError::DeviceNotFound;
        let mut result: Option<(IMFSourceReader, (u32, u32))> = None;

        for i in 0..count {
            let activate_opt = &*device_sources.add(i as usize);
            let activate = match activate_opt.as_ref() {
                Some(a) => a,
                None => continue,
            };

            // If a specific device ID was requested, check if this is the right one
            if let Some(requested_id) = device_id {
                if let Some(this_id) = Self::get_device_id(activate) {
                    if this_id != *requested_id {
                        continue;
                    }
                }
            }

            // Try to activate this device
            match Self::try_open_device_static(activate) {
                Ok((reader, dims)) => {
                    result = Some((reader, dims));
                    break;
                }
                Err(e) => {
                    eprintln!("Failed to open camera device {}: {:?}", i, e);
                    last_error = e;
                    continue;
                }
            }
        }

        // Free the device array
        CoTaskMemFree(Some(device_sources as *const _));

        result.ok_or(last_error)
    }

    /// Try to open a specific camera device - static version for thread
    unsafe fn try_open_device_static(
        activate: &IMFActivate,
    ) -> Result<(IMFSourceReader, (u32, u32)), VideoInputError> {
        // Create media source from the activation object
        let media_source: IMFMediaSource = activate.ActivateObject().map_err(|e| {
            VideoInputError::Other(format!("Failed to activate device: {:?}", e))
        })?;

        // Create source reader attributes with video processing and low latency enabled
        let mut reader_attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut reader_attributes as *mut _, 4).map_err(|e| {
            VideoInputError::Other(format!("Failed to create reader attributes: {:?}", e))
        })?;
        let reader_attributes = reader_attributes.ok_or_else(|| {
            VideoInputError::Other("No reader attributes returned".to_string())
        })?;

        // Enable video processing for format conversion
        reader_attributes
            .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
            .ok();

        // Enable low latency mode for real-time capture
        reader_attributes
            .SetUINT32(&MF_LOW_LATENCY, 1)
            .ok();

        // Disable throttling for real-time capture
        reader_attributes
            .SetUINT32(&MF_SOURCE_READER_DISCONNECT_MEDIASOURCE_ON_SHUTDOWN, 1)
            .ok();

        let source_reader =
            MFCreateSourceReaderFromMediaSource(&media_source, &reader_attributes).map_err(
                |e| {
                    VideoInputError::Other(format!("Failed to create source reader: {:?}", e))
                },
            )?;

        // Select the video stream
        source_reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .ok();
        source_reader
            .SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true)
            .ok();

        // Configure output to RGB32 format
        let media_type: IMFMediaType = MFCreateMediaType().map_err(|e| {
            VideoInputError::Other(format!("Failed to create media type: {:?}", e))
        })?;

        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| {
                VideoInputError::InvalidConfig(format!("Failed to set major type: {:?}", e))
            })?;

        media_type
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
            .map_err(|e| {
                VideoInputError::InvalidConfig(format!("Failed to set subtype: {:?}", e))
            })?;

        // Try to set RGB32, fall back if not supported
        let format_set = source_reader
            .SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                None,
                &media_type,
            )
            .is_ok();

        if !format_set {
            // Try NV12 (common camera format) - we'll convert it later
            media_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
                .ok();
            source_reader
                .SetCurrentMediaType(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    None,
                    &media_type,
                )
                .map_err(|e| {
                    VideoInputError::InvalidConfig(format!("Failed to set media type: {:?}", e))
                })?;
        }

        // Get actual output dimensions
        let output_type = source_reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .map_err(|e| {
                VideoInputError::Other(format!("Failed to get output type: {:?}", e))
            })?;

        // Get video dimensions (packed as UINT64: width << 32 | height)
        let frame_size = output_type.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|e| {
            VideoInputError::Other(format!("Failed to get frame size: {:?}", e))
        })?;
        let width = (frame_size >> 32) as u32;
        let height = (frame_size & 0xFFFFFFFF) as u32;

        Ok((source_reader, (width, height)))
    }

    /// Process a sample and convert to VideoFrame - static version for thread
    fn process_sample_static(
        sample: &IMFSample,
        dims: (u32, u32),
        rgba_buffer: &mut Vec<u8>,
    ) -> Option<VideoFrame> {
        unsafe {
            let buffer = sample.ConvertToContiguousBuffer().ok()?;
            let (width, height) = dims;
            let expected_size = (width * height * 4) as usize;
            let expected_row_bytes = (width * 4) as usize;

            // Try to get the buffer as a 2D buffer first (gives us proper stride/pitch)
            let buffer_2d: Option<IMF2DBuffer> = buffer.cast().ok();

            // Ensure our reusable buffer is the right size
            if rgba_buffer.len() != expected_size {
                rgba_buffer.resize(expected_size, 0);
            }

            let success = if let Some(buf_2d) = buffer_2d {
                // Use 2D buffer interface for proper stride
                let mut scanline0: *mut u8 = ptr::null_mut();
                let mut pitch: i32 = 0;
                if buf_2d.Lock2D(&mut scanline0, &mut pitch).is_err() {
                    return None;
                }

                if scanline0.is_null() {
                    let _ = buf_2d.Unlock2D();
                    return None;
                }

                // pitch can be negative (bottom-up image) - we use absolute value for stride
                let stride = pitch.unsigned_abs() as usize;

                // Fast path: if stride matches expected row bytes, no padding to handle
                let result = if stride == expected_row_bytes {
                    let total_len = stride * height as usize;
                    let data_slice = std::slice::from_raw_parts(scanline0, total_len);

                    // Check if frame is all zeros (camera warming up)
                    let has_nonzero = data_slice.iter().take(100).any(|&b| b != 0);
                    if !has_nonzero {
                        false
                    } else {
                        // Fast BGRX -> RGBA conversion
                        Self::convert_bgrx_to_rgba_fast(data_slice, rgba_buffer);
                        true
                    }
                } else {
                    // Slow path with stride handling
                    let total_len = stride * height as usize;
                    let data_slice = std::slice::from_raw_parts(scanline0, total_len);

                    let has_nonzero = data_slice.iter().take(100).any(|&b| b != 0);
                    if !has_nonzero {
                        false
                    } else {
                        Self::convert_bgrx_to_rgba_with_stride(
                            data_slice,
                            rgba_buffer,
                            width as usize,
                            height as usize,
                            stride,
                        );
                        true
                    }
                };

                let _ = buf_2d.Unlock2D();
                result
            } else {
                // Fall back to 1D buffer
                let mut data: *mut u8 = ptr::null_mut();
                let mut max_length = 0u32;
                let mut current_length = 0u32;
                if buffer
                    .Lock(&mut data, Some(&mut max_length), Some(&mut current_length))
                    .is_err()
                {
                    return None;
                }

                if data.is_null() || current_length == 0 {
                    let _ = buffer.Unlock();
                    return None;
                }

                let data_slice = std::slice::from_raw_parts(data, current_length as usize);

                // Check if frame is all zeros (camera warming up)
                let has_nonzero = data_slice.iter().take(100).any(|&b| b != 0);
                let result = if !has_nonzero {
                    false
                } else {
                    // Calculate stride from data length
                    let stride = current_length as usize / height as usize;

                    if stride == expected_row_bytes {
                        Self::convert_bgrx_to_rgba_fast(data_slice, rgba_buffer);
                    } else {
                        Self::convert_bgrx_to_rgba_with_stride(
                            data_slice,
                            rgba_buffer,
                            width as usize,
                            height as usize,
                            stride,
                        );
                    }
                    true
                };

                let _ = buffer.Unlock();
                result
            };

            if success {
                // Clone the buffer for the frame - we need to keep rgba_buffer for reuse
                Some(VideoFrame {
                    data: rgba_buffer.clone(),
                    width,
                    height,
                    pixel_format: PixelFormat::RGBA,
                    timestamp_ns: 0,
                })
            } else {
                None
            }
        }
    }

    /// Fast BGRX to RGBA conversion when stride equals row width (no padding)
    #[inline]
    fn convert_bgrx_to_rgba_fast(src: &[u8], dst: &mut [u8]) {
        // Process 4 pixels at a time for better cache efficiency
        let chunks = src.len() / 16;
        for i in 0..chunks {
            let src_offset = i * 16;
            let dst_offset = i * 16;

            // Pixel 0
            dst[dst_offset] = src[src_offset + 2]; // R
            dst[dst_offset + 1] = src[src_offset + 1]; // G
            dst[dst_offset + 2] = src[src_offset]; // B
            dst[dst_offset + 3] = 255; // A

            // Pixel 1
            dst[dst_offset + 4] = src[src_offset + 6];
            dst[dst_offset + 5] = src[src_offset + 5];
            dst[dst_offset + 6] = src[src_offset + 4];
            dst[dst_offset + 7] = 255;

            // Pixel 2
            dst[dst_offset + 8] = src[src_offset + 10];
            dst[dst_offset + 9] = src[src_offset + 9];
            dst[dst_offset + 10] = src[src_offset + 8];
            dst[dst_offset + 11] = 255;

            // Pixel 3
            dst[dst_offset + 12] = src[src_offset + 14];
            dst[dst_offset + 13] = src[src_offset + 13];
            dst[dst_offset + 14] = src[src_offset + 12];
            dst[dst_offset + 15] = 255;
        }

        // Handle remaining pixels
        let remaining_start = chunks * 16;
        for i in (remaining_start..src.len()).step_by(4) {
            if i + 3 < src.len() && i + 3 < dst.len() {
                dst[i] = src[i + 2]; // R
                dst[i + 1] = src[i + 1]; // G
                dst[i + 2] = src[i]; // B
                dst[i + 3] = 255; // A
            }
        }
    }

    /// BGRX to RGBA conversion with stride handling (for padded buffers)
    fn convert_bgrx_to_rgba_with_stride(
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
        stride: usize,
    ) {
        let row_bytes = width * 4;
        for y in 0..height {
            let src_row_start = y * stride;
            let dst_row_start = y * row_bytes;

            for x in 0..width {
                let src_idx = src_row_start + x * 4;
                let dst_idx = dst_row_start + x * 4;

                if src_idx + 3 < src.len() && dst_idx + 3 < dst.len() {
                    dst[dst_idx] = src[src_idx + 2]; // R
                    dst[dst_idx + 1] = src[src_idx + 1]; // G
                    dst[dst_idx + 2] = src[src_idx]; // B
                    dst[dst_idx + 3] = 255; // A
                }
            }
        }
    }

    /// Get device name from activation object
    unsafe fn get_device_name(activate: &IMFActivate) -> Option<String> {
        let mut name_ptr: PWSTR = PWSTR::null();
        let mut name_len = 0u32;

        if activate
            .GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, &mut name_ptr, &mut name_len)
            .is_ok()
        {
            if !name_ptr.is_null() && name_len > 0 {
                let slice = std::slice::from_raw_parts(name_ptr.0, name_len as usize);
                let result = String::from_utf16_lossy(slice);
                CoTaskMemFree(Some(name_ptr.0 as *const _));
                return Some(result);
            }
        }
        None
    }

    /// Get device symbolic link (ID) from activation object
    unsafe fn get_device_id(activate: &IMFActivate) -> Option<String> {
        let mut id_ptr: PWSTR = PWSTR::null();
        let mut id_len = 0u32;

        if activate
            .GetAllocatedString(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                &mut id_ptr,
                &mut id_len,
            )
            .is_ok()
        {
            if !id_ptr.is_null() && id_len > 0 {
                let slice = std::slice::from_raw_parts(id_ptr.0, id_len as usize);
                let result = String::from_utf16_lossy(slice);
                CoTaskMemFree(Some(id_ptr.0 as *const _));
                return Some(result);
            }
        }
        None
    }

    /// Convert atomic state to VideoInputState enum
    fn atomic_to_state(val: u8) -> VideoInputState {
        match val {
            STATE_READY => VideoInputState::Ready,
            STATE_CAPTURING => VideoInputState::Capturing,
            STATE_STOPPED => VideoInputState::Stopped,
            _ => VideoInputState::Idle,
        }
    }
}

impl Default for WindowsVideoInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsVideoInput {
    fn drop(&mut self) {
        self.close();
    }
}

impl VideoInputBackend for WindowsVideoInput {
    fn request_permission(&mut self) -> std::result::Result<(), VideoInputError> {
        // Windows desktop apps don't require explicit camera permission
        // (UWP apps do, but this is for Win32)
        self.permission_granted.store(true, Ordering::SeqCst);
        self.state.store(STATE_READY, Ordering::SeqCst);
        Ok(())
    }

    fn has_permission(&self) -> bool {
        self.permission_granted.load(Ordering::SeqCst)
    }

    fn list_devices(&self) -> std::result::Result<Vec<VideoInputDevice>, VideoInputError> {
        ensure_mf_initialized()?;

        let mut devices = Vec::new();

        unsafe {
            // Create attributes for video capture devices
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes as *mut _, 1).map_err(|e| {
                VideoInputError::Other(format!("Failed to create attributes: {:?}", e))
            })?;
            let attributes = attributes.ok_or_else(|| {
                VideoInputError::Other("No attributes returned".to_string())
            })?;

            attributes
                .SetGUID(
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                )
                .map_err(|e| {
                    VideoInputError::Other(format!("Failed to set source type: {:?}", e))
                })?;

            // Enumerate video capture devices
            let mut count = 0u32;
            let mut device_sources: *mut Option<IMFActivate> = ptr::null_mut();

            MFEnumDeviceSources(&attributes, &mut device_sources, &mut count).map_err(|e| {
                VideoInputError::Other(format!("Failed to enumerate devices: {:?}", e))
            })?;

            if count > 0 && !device_sources.is_null() {
                for i in 0..count {
                    let activate_opt = &*device_sources.add(i as usize);
                    if let Some(activate) = activate_opt {
                        let name = Self::get_device_name(activate);
                        let id = Self::get_device_id(activate);

                        if let (Some(name), Some(id)) = (name, id) {
                            devices.push(VideoInputDevice {
                                id,
                                name,
                                position: CameraPosition::External, // Windows doesn't distinguish
                                is_default: i == 0,
                                resolutions: vec![(1920, 1080), (1280, 720), (640, 480)], // Common resolutions
                            });
                        }
                    }
                }

                // Free the array
                CoTaskMemFree(Some(device_sources as *const _));
            }
        }

        Ok(devices)
    }

    fn open(
        &mut self,
        device_id: Option<&str>,
        _config: &VideoInputConfig,
    ) -> std::result::Result<(), VideoInputError> {
        // Close any existing capture first
        self.close();

        // Store the device config persistently (survives stop/start cycles)
        self.device_config = Some(DeviceConfig {
            device_id: device_id.map(|s| s.to_string()),
        });

        // Store the open request for the capture thread
        if let Ok(mut req) = self.open_request.lock() {
            *req = Some(DeviceOpenRequest {
                device_id: device_id.map(|s| s.to_string()),
            });
        }

        self.state.store(STATE_READY, Ordering::SeqCst);
        Ok(())
    }

    fn start(&mut self) -> std::result::Result<(), VideoInputError> {
        let current_state = self.state.load(Ordering::SeqCst);
        if current_state == STATE_CAPTURING {
            return Ok(()); // Already capturing
        }

        if current_state != STATE_READY && current_state != STATE_STOPPED {
            return Err(VideoInputError::Other("Device not opened".into()));
        }

        // Restore open request from device config if it was consumed by a previous capture thread
        {
            let mut req_guard = self.open_request.lock().unwrap();
            if req_guard.is_none() {
                if let Some(config) = &self.device_config {
                    *req_guard = Some(DeviceOpenRequest {
                        device_id: config.device_id.clone(),
                    });
                } else {
                    return Err(VideoInputError::Other("No device configured".into()));
                }
            }
        }

        // Reset stop signal
        self.stop_signal.store(false, Ordering::SeqCst);

        // Clone the Arcs for the thread
        let state = Arc::clone(&self.state);
        let dimensions = Arc::clone(&self.dimensions);
        let latest_frame = Arc::clone(&self.latest_frame);
        let stop_signal = Arc::clone(&self.stop_signal);
        let open_request = Arc::clone(&self.open_request);

        // Spawn the capture thread
        self.capture_thread = Some(thread::spawn(move || {
            Self::capture_thread_fn(state, dimensions, latest_frame, stop_signal, open_request);
        }));

        Ok(())
    }

    fn stop(&mut self) -> std::result::Result<(), VideoInputError> {
        // Signal the capture thread to stop
        self.stop_signal.store(true, Ordering::SeqCst);

        // Wait for the thread to finish
        if let Some(handle) = self.capture_thread.take() {
            let _ = handle.join();
        }

        self.state.store(STATE_STOPPED, Ordering::SeqCst);
        Ok(())
    }

    fn close(&mut self) {
        self.stop().ok();

        // Clear dimensions and latest frame
        if let Ok(mut d) = self.dimensions.lock() {
            *d = None;
        }
        if let Ok(mut f) = self.latest_frame.lock() {
            *f = None;
        }

        // Clear device config (requires re-open to start again)
        self.device_config = None;

        self.state.store(STATE_IDLE, Ordering::SeqCst);
    }

    fn state(&self) -> VideoInputState {
        Self::atomic_to_state(self.state.load(Ordering::SeqCst))
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions.lock().ok().and_then(|d| *d)
    }

    fn set_frame_callback(&mut self, callback: Option<VideoFrameCallback>) {
        self.frame_callback = callback;
    }

    fn latest_frame(&self) -> Option<VideoFrame> {
        // Take the frame instead of cloning to avoid extra allocation
        self.latest_frame.lock().ok().and_then(|mut f| f.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_input_creation() {
        let input = WindowsVideoInput::new();
        assert_eq!(input.state(), VideoInputState::Idle);
    }

    #[test]
    fn test_has_permission() {
        let input = WindowsVideoInput::new();
        // Windows desktop apps don't require explicit camera permission
        assert!(input.has_permission());
    }
}
