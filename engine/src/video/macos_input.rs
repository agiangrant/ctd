//! macOS video input using AVFoundation
//!
//! Uses AVCaptureSession with AVCaptureVideoDataOutput for camera capture.
//! Supports device enumeration and selection.

use super::input::{
    CameraPosition, PixelFormat, VideoFrame, VideoFrameCallback, VideoInputBackend,
    VideoInputConfig, VideoInputDevice, VideoInputError, VideoInputState,
};
use block::ConcreteBlock;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::{msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};

/// Registered delegate class for video sample buffer callbacks
static VIDEO_DELEGATE_CLASS: Once = Once::new();
static mut VIDEO_DELEGATE_CLASS_PTR: *const Class = std::ptr::null();

/// Context passed to the video delegate
struct VideoDelegateContext {
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
}

/// Register our custom video delegate class (called once)
fn get_video_delegate_class() -> &'static Class {
    VIDEO_DELEGATE_CLASS.call_once(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("CenteredVideoSampleDelegate", superclass)
            .expect("Failed to create video delegate class");

        // Add instance variable for context pointer
        decl.add_ivar::<*mut c_void>("_context");

        // Add the delegate method
        unsafe {
            decl.add_method(
                sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
                capture_output_did_output_video_sample_buffer
                    as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object),
            );
        }

        let cls = decl.register();
        unsafe {
            VIDEO_DELEGATE_CLASS_PTR = cls;
        }
    });

    unsafe { &*VIDEO_DELEGATE_CLASS_PTR }
}

/// The delegate callback for processing video sample buffers
extern "C" fn capture_output_did_output_video_sample_buffer(
    this: &Object,
    _sel: Sel,
    _output: *mut Object,
    sample_buffer: *mut Object,
    _connection: *mut Object,
) {
    unsafe {
        // Get context from ivar
        let context_ptr: *mut c_void = *this.get_ivar("_context");
        if context_ptr.is_null() {
            return;
        }
        let context = &*(context_ptr as *const VideoDelegateContext);

        // Get the image buffer (CVPixelBuffer) from the sample buffer
        let pixel_buffer = CMSampleBufferGetImageBuffer(sample_buffer as *mut c_void);
        if pixel_buffer.is_null() {
            return;
        }

        // Lock the base address for reading
        let lock_result = CVPixelBufferLockBaseAddress(pixel_buffer, 1); // kCVPixelBufferLock_ReadOnly = 1
        if lock_result != 0 {
            return;
        }

        // Get dimensions
        let width = CVPixelBufferGetWidth(pixel_buffer) as u32;
        let height = CVPixelBufferGetHeight(pixel_buffer) as u32;
        let bytes_per_row = CVPixelBufferGetBytesPerRow(pixel_buffer);

        // Get pointer to pixel data
        let base_address = CVPixelBufferGetBaseAddress(pixel_buffer);
        if base_address.is_null() {
            CVPixelBufferUnlockBaseAddress(pixel_buffer, 1);
            return;
        }

        // Copy the pixel data (BGRA format)
        let data_len = bytes_per_row * height as usize;
        let data = std::slice::from_raw_parts(base_address as *const u8, data_len);

        // Create VideoFrame
        let frame = VideoFrame {
            width,
            height,
            data: data.to_vec(),
            timestamp_ns: 0, // TODO: get actual timestamp from sample buffer
            pixel_format: PixelFormat::BGRA,
        };

        // Unlock the buffer
        CVPixelBufferUnlockBaseAddress(pixel_buffer, 1);

        // Store the frame
        if let Ok(mut latest) = context.latest_frame.lock() {
            *latest = Some(frame);
        }
    }
}

// CoreMedia framework functions for video
#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMSampleBufferGetImageBuffer(sbuf: *mut c_void) -> *mut c_void;
}

// CoreVideo framework functions
#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferLockBaseAddress(pixelBuffer: *mut c_void, lockFlags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixelBuffer: *mut c_void, unlockFlags: u64) -> i32;
    fn CVPixelBufferGetWidth(pixelBuffer: *mut c_void) -> usize;
    fn CVPixelBufferGetHeight(pixelBuffer: *mut c_void) -> usize;
    fn CVPixelBufferGetBytesPerRow(pixelBuffer: *mut c_void) -> usize;
    fn CVPixelBufferGetBaseAddress(pixelBuffer: *mut c_void) -> *mut c_void;
}

// Dispatch queue functions
#[link(name = "System")]
extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
}

/// macOS video input using AVCaptureSession
pub struct MacOSVideoInput {
    /// AVCaptureSession instance
    session: *mut Object,
    /// AVCaptureDeviceInput
    device_input: *mut Object,
    /// AVCaptureVideoDataOutput
    video_output: *mut Object,
    /// Delegate for sample buffer callbacks
    delegate: *mut Object,
    /// Dispatch queue for callbacks
    callback_queue: *mut Object,
    /// Delegate context (boxed to ensure stable address)
    delegate_context: Option<Box<VideoDelegateContext>>,
    /// Current state
    state: VideoInputState,
    /// Permission granted
    permission_granted: Arc<AtomicBool>,
    /// Current frame dimensions
    dimensions: Option<(u32, u32)>,
    /// Frame callback
    frame_callback: Option<VideoFrameCallback>,
    /// Latest frame
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
}

// Safety: We ensure thread safety through proper synchronization
unsafe impl Send for MacOSVideoInput {}

impl MacOSVideoInput {
    /// Create a new macOS video input
    pub fn new() -> Self {
        Self {
            session: ptr::null_mut(),
            device_input: ptr::null_mut(),
            video_output: ptr::null_mut(),
            delegate: ptr::null_mut(),
            callback_queue: ptr::null_mut(),
            delegate_context: None,
            state: VideoInputState::Idle,
            permission_granted: Arc::new(AtomicBool::new(false)),
            dimensions: None,
            frame_callback: None,
            latest_frame: Arc::new(Mutex::new(None)),
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

    /// Get string from NSString
    unsafe fn nsstring_to_string(ns_string: *mut Object) -> Option<String> {
        if ns_string.is_null() {
            return None;
        }
        let utf8: *const i8 = msg_send![ns_string, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }

    /// Convert AVCaptureDevicePosition to CameraPosition
    fn position_from_av(position: i64) -> CameraPosition {
        match position {
            1 => CameraPosition::Back,
            2 => CameraPosition::Front,
            _ => CameraPosition::Unspecified,
        }
    }
}

impl Default for MacOSVideoInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacOSVideoInput {
    fn drop(&mut self) {
        self.close();
    }
}

impl VideoInputBackend for MacOSVideoInput {
    fn request_permission(&mut self) -> Result<(), VideoInputError> {
        if self.permission_granted.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.state = VideoInputState::RequestingPermission;

        unsafe {
            // Get AVCaptureDevice class
            let device_class = class!(AVCaptureDevice);

            // Check authorization status for video
            // AVMediaTypeVideo = "vide"
            let media_type = Self::create_nsstring("vide");
            let status: i64 = msg_send![device_class, authorizationStatusForMediaType: media_type];

            match status {
                0 => {
                    // AVAuthorizationStatusNotDetermined - show system dialog
                    // Use a shared atomic to capture the result from the block
                    let granted = Arc::new(AtomicBool::new(false));
                    let granted_clone = granted.clone();
                    let done = Arc::new(AtomicBool::new(false));
                    let done_clone = done.clone();

                    // Create the completion block
                    let block = ConcreteBlock::new(move |result: BOOL| {
                        granted_clone.store(result == YES, Ordering::SeqCst);
                        done_clone.store(true, Ordering::SeqCst);
                    });
                    let block = block.copy();

                    // Call requestAccessForMediaType:completionHandler:
                    let _: () = msg_send![device_class, requestAccessForMediaType:media_type completionHandler:&*block];

                    // Wait for the callback (with timeout)
                    let start = std::time::Instant::now();
                    while !done.load(Ordering::SeqCst) {
                        if start.elapsed() > std::time::Duration::from_secs(120) {
                            let _: () = msg_send![media_type, release];
                            self.state = VideoInputState::Error;
                            return Err(VideoInputError::Other("Permission request timed out".into()));
                        }
                        // Run the main run loop briefly to allow the permission dialog to work
                        let run_loop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
                        let date: *mut Object = msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: 0.1f64];
                        let _: () = msg_send![run_loop, runUntilDate: date];
                    }

                    let _: () = msg_send![media_type, release];

                    if granted.load(Ordering::SeqCst) {
                        self.permission_granted.store(true, Ordering::SeqCst);
                        self.state = VideoInputState::Ready;
                        Ok(())
                    } else {
                        self.state = VideoInputState::Error;
                        Err(VideoInputError::PermissionDenied)
                    }
                }
                1 => {
                    // AVAuthorizationStatusRestricted
                    let _: () = msg_send![media_type, release];
                    self.state = VideoInputState::Error;
                    Err(VideoInputError::Other("Camera access restricted by system policy".into()))
                }
                2 => {
                    // AVAuthorizationStatusDenied
                    let _: () = msg_send![media_type, release];
                    self.state = VideoInputState::Error;
                    Err(VideoInputError::PermissionDenied)
                }
                3 => {
                    // AVAuthorizationStatusAuthorized
                    let _: () = msg_send![media_type, release];
                    self.permission_granted.store(true, Ordering::SeqCst);
                    self.state = VideoInputState::Ready;
                    Ok(())
                }
                _ => {
                    let _: () = msg_send![media_type, release];
                    self.state = VideoInputState::Error;
                    Err(VideoInputError::Other("Unknown authorization status".into()))
                }
            }
        }
    }

    fn has_permission(&self) -> bool {
        if self.permission_granted.load(Ordering::SeqCst) {
            return true;
        }

        // Check current status
        unsafe {
            let device_class = class!(AVCaptureDevice);
            let media_type = Self::create_nsstring("vide");
            let status: i64 = msg_send![device_class, authorizationStatusForMediaType: media_type];
            let _: () = msg_send![media_type, release];

            status == 3 // AVAuthorizationStatusAuthorized
        }
    }

    fn list_devices(&self) -> Result<Vec<VideoInputDevice>, VideoInputError> {
        let mut devices = Vec::new();

        unsafe {
            let device_class = class!(AVCaptureDevice);

            // Get default video device
            let media_type = Self::create_nsstring("vide");
            let default_device: *mut Object = msg_send![device_class, defaultDeviceWithMediaType: media_type];
            let _: () = msg_send![media_type, release];

            let default_id = if !default_device.is_null() {
                let uid: *mut Object = msg_send![default_device, uniqueID];
                Self::nsstring_to_string(uid)
            } else {
                None
            };

            // Create discovery session for video devices
            // Include built-in cameras and external devices (USB webcams, etc.)
            let wide_angle = Self::create_nsstring("AVCaptureDeviceTypeBuiltInWideAngleCamera");
            let external = Self::create_nsstring("AVCaptureDeviceTypeExternal");

            // Create array with multiple device types using arrayWithObjects:count:
            // This expects a C array pointer and count
            let types_array: [*mut Object; 2] = [wide_angle, external];
            let device_types: *mut Object = msg_send![class!(NSArray), arrayWithObjects:types_array.as_ptr() count:2usize];
            let _: () = msg_send![wide_angle, release];
            let _: () = msg_send![external, release];

            let media_type = Self::create_nsstring("vide");
            let discovery_session: *mut Object = msg_send![
                class!(AVCaptureDeviceDiscoverySession),
                discoverySessionWithDeviceTypes: device_types
                mediaType: media_type
                position: 0i64 // AVCaptureDevicePositionUnspecified
            ];
            let _: () = msg_send![media_type, release];

            if !discovery_session.is_null() {
                let discovered_devices: *mut Object = msg_send![discovery_session, devices];
                let count: usize = msg_send![discovered_devices, count];

                for i in 0..count {
                    let device: *mut Object = msg_send![discovered_devices, objectAtIndex: i];
                    if device.is_null() {
                        continue;
                    }

                    let uid: *mut Object = msg_send![device, uniqueID];
                    let name: *mut Object = msg_send![device, localizedName];
                    let position: i64 = msg_send![device, position];

                    if let (Some(id), Some(device_name)) = (Self::nsstring_to_string(uid), Self::nsstring_to_string(name)) {
                        let is_default = default_id.as_ref().map(|d| d == &id).unwrap_or(false);

                        // Get supported resolutions
                        let formats: *mut Object = msg_send![device, formats];
                        let format_count: usize = msg_send![formats, count];
                        let mut resolutions = Vec::new();

                        for j in 0..format_count {
                            let format: *mut Object = msg_send![formats, objectAtIndex: j];
                            let desc: *mut Object = msg_send![format, formatDescription];
                            if !desc.is_null() {
                                // Get dimensions from CMVideoFormatDescription
                                // This requires CoreMedia framework access
                                // For now, we'll add common resolutions
                            }
                        }

                        // Add common resolutions as fallback
                        if resolutions.is_empty() {
                            resolutions = vec![
                                (1920, 1080),
                                (1280, 720),
                                (640, 480),
                            ];
                        }

                        devices.push(VideoInputDevice {
                            id,
                            name: device_name,
                            position: Self::position_from_av(position),
                            is_default,
                            resolutions,
                        });
                    }
                }
            }

            // If discovery session didn't find devices, try getting the default directly
            if devices.is_empty() && !default_device.is_null() {
                let uid: *mut Object = msg_send![default_device, uniqueID];
                let name: *mut Object = msg_send![default_device, localizedName];
                let position: i64 = msg_send![default_device, position];

                if let (Some(id), Some(device_name)) = (Self::nsstring_to_string(uid), Self::nsstring_to_string(name)) {
                    devices.push(VideoInputDevice {
                        id,
                        name: device_name,
                        position: Self::position_from_av(position),
                        is_default: true,
                        resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
                    });
                }
            }
        }

        Ok(devices)
    }

    fn open(&mut self, device_id: Option<&str>, config: &VideoInputConfig) -> Result<(), VideoInputError> {
        if !self.has_permission() {
            return Err(VideoInputError::PermissionDenied);
        }

        // Close any existing session
        self.close();

        unsafe {
            // Create capture session
            let session: *mut Object = msg_send![class!(AVCaptureSession), alloc];
            let session: *mut Object = msg_send![session, init];

            if session.is_null() {
                return Err(VideoInputError::Other("Failed to create capture session".into()));
            }

            // Set session preset based on requested resolution
            let preset = if config.width >= 1920 {
                Self::create_nsstring("AVCaptureSessionPreset1920x1080")
            } else if config.width >= 1280 {
                Self::create_nsstring("AVCaptureSessionPreset1280x720")
            } else {
                Self::create_nsstring("AVCaptureSessionPreset640x480")
            };
            let _: () = msg_send![session, setSessionPreset: preset];
            let _: () = msg_send![preset, release];

            // Get the device
            let device: *mut Object = if let Some(id) = device_id {
                let ns_id = Self::create_nsstring(id);
                let device: *mut Object = msg_send![class!(AVCaptureDevice), deviceWithUniqueID: ns_id];
                let _: () = msg_send![ns_id, release];
                device
            } else {
                let media_type = Self::create_nsstring("vide");
                let device: *mut Object = msg_send![class!(AVCaptureDevice), defaultDeviceWithMediaType: media_type];
                let _: () = msg_send![media_type, release];
                device
            };

            if device.is_null() {
                let _: () = msg_send![session, release];
                return Err(VideoInputError::DeviceNotFound);
            }

            // Create device input
            let mut error: *mut Object = ptr::null_mut();
            let device_input: *mut Object = msg_send![class!(AVCaptureDeviceInput), deviceInputWithDevice:device error:&mut error];

            if device_input.is_null() || !error.is_null() {
                let _: () = msg_send![session, release];
                if !error.is_null() {
                    let desc: *mut Object = msg_send![error, localizedDescription];
                    if let Some(err_str) = Self::nsstring_to_string(desc) {
                        return Err(VideoInputError::Other(format!("Failed to create device input: {}", err_str)));
                    }
                }
                return Err(VideoInputError::Other("Failed to create device input".into()));
            }

            // Add input to session
            let can_add: BOOL = msg_send![session, canAddInput: device_input];
            if can_add == NO {
                let _: () = msg_send![session, release];
                return Err(VideoInputError::DeviceInUse);
            }
            let _: () = msg_send![session, addInput: device_input];

            // Create video data output
            let video_output: *mut Object = msg_send![class!(AVCaptureVideoDataOutput), alloc];
            let video_output: *mut Object = msg_send![video_output, init];

            if video_output.is_null() {
                let _: () = msg_send![session, release];
                return Err(VideoInputError::Other("Failed to create video output".into()));
            }

            // Set pixel format to BGRA for easy rendering
            let pixel_format_key = Self::create_nsstring("PixelFormatType");
            // kCVPixelFormatType_32BGRA = 'BGRA' = 0x42475241
            let pixel_format_value: *mut Object = msg_send![class!(NSNumber), numberWithUnsignedInt: 0x42475241u32];
            let settings: *mut Object = msg_send![class!(NSDictionary), dictionaryWithObject:pixel_format_value forKey:pixel_format_key];
            let _: () = msg_send![video_output, setVideoSettings: settings];
            let _: () = msg_send![pixel_format_key, release];

            // Add output to session
            let can_add: BOOL = msg_send![session, canAddOutput: video_output];
            if can_add == NO {
                let _: () = msg_send![video_output, release];
                let _: () = msg_send![session, release];
                return Err(VideoInputError::Other("Cannot add output to session".into()));
            }
            let _: () = msg_send![session, addOutput: video_output];

            // Create dispatch queue for sample buffer callbacks
            let queue_label = std::ffi::CString::new("com.centered.video.input").unwrap();
            let callback_queue = dispatch_queue_create(queue_label.as_ptr(), std::ptr::null());
            if callback_queue.is_null() {
                let _: () = msg_send![video_output, release];
                let _: () = msg_send![session, release];
                return Err(VideoInputError::Other("Failed to create dispatch queue".into()));
            }

            // Create delegate context with reference to our latest_frame Arc
            let context = Box::new(VideoDelegateContext {
                latest_frame: self.latest_frame.clone(),
            });

            // Create delegate instance
            let delegate_class = get_video_delegate_class();
            let delegate: *mut Object = msg_send![delegate_class, alloc];
            let delegate: *mut Object = msg_send![delegate, init];
            if delegate.is_null() {
                dispatch_release(callback_queue);
                let _: () = msg_send![video_output, release];
                let _: () = msg_send![session, release];
                return Err(VideoInputError::Other("Failed to create delegate".into()));
            }

            // Store context pointer in delegate's ivar
            let context_ptr = Box::into_raw(context) as *mut c_void;
            (*delegate).set_ivar("_context", context_ptr);

            // Set delegate on video output
            let _: () = msg_send![video_output, setSampleBufferDelegate:delegate queue:callback_queue];

            self.session = session;
            self.device_input = device_input;
            self.video_output = video_output;
            self.delegate = delegate;
            self.callback_queue = callback_queue as *mut Object;
            // Re-box the context so we can manage its lifetime
            self.delegate_context = Some(unsafe { Box::from_raw(context_ptr as *mut VideoDelegateContext) });
            self.dimensions = Some((config.width, config.height));
            self.state = VideoInputState::Ready;

            Ok(())
        }
    }

    fn start(&mut self) -> Result<(), VideoInputError> {
        if self.session.is_null() {
            return Err(VideoInputError::Other("Session not opened".into()));
        }

        unsafe {
            let is_running: BOOL = msg_send![self.session, isRunning];
            if is_running == YES {
                return Ok(()); // Already running
            }

            let _: () = msg_send![self.session, startRunning];
            self.state = VideoInputState::Capturing;
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), VideoInputError> {
        if self.session.is_null() {
            return Ok(());
        }

        unsafe {
            let is_running: BOOL = msg_send![self.session, isRunning];
            if is_running == YES {
                let _: () = msg_send![self.session, stopRunning];
            }
            self.state = VideoInputState::Stopped;
        }

        Ok(())
    }

    fn close(&mut self) {
        self.stop().ok();

        unsafe {
            if !self.session.is_null() {
                // Remove inputs and outputs
                if !self.device_input.is_null() {
                    let _: () = msg_send![self.session, removeInput: self.device_input];
                }
                if !self.video_output.is_null() {
                    // Clear the delegate before removing
                    let null: *mut Object = std::ptr::null_mut();
                    let _: () = msg_send![self.video_output, setSampleBufferDelegate:null queue:null];
                    let _: () = msg_send![self.session, removeOutput: self.video_output];
                    let _: () = msg_send![self.video_output, release];
                }
                let _: () = msg_send![self.session, release];
            }

            // Release delegate
            if !self.delegate.is_null() {
                let _: () = msg_send![self.delegate, release];
            }

            // Release dispatch queue
            if !self.callback_queue.is_null() {
                dispatch_release(self.callback_queue as *mut c_void);
            }
        }

        self.session = ptr::null_mut();
        self.device_input = ptr::null_mut();
        self.video_output = ptr::null_mut();
        self.delegate = ptr::null_mut();
        self.callback_queue = ptr::null_mut();
        self.delegate_context = None;
        self.dimensions = None;
        self.state = VideoInputState::Idle;
    }

    fn state(&self) -> VideoInputState {
        self.state
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }

    fn set_frame_callback(&mut self, callback: Option<VideoFrameCallback>) {
        self.frame_callback = callback;
    }

    fn latest_frame(&self) -> Option<VideoFrame> {
        self.latest_frame.lock().ok()?.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_input_creation() {
        let input = MacOSVideoInput::new();
        assert_eq!(input.state(), VideoInputState::Idle);
    }
}
