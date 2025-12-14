//! macOS audio input using AVFoundation
//!
//! Uses AVCaptureSession with AVCaptureAudioDataOutput for microphone capture.
//! Supports device enumeration and selection.

use super::input::{AudioInputBackend, AudioInputConfig, AudioInputDevice, AudioInputState, AudioSampleCallback};
use super::{AudioError, AudioInfo};
use block::ConcreteBlock;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::{msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Once};

/// Registered delegate class for audio sample buffer callbacks
static AUDIO_DELEGATE_CLASS: Once = Once::new();
static mut AUDIO_DELEGATE_CLASS_PTR: *const Class = std::ptr::null();

/// Context passed to the delegate
struct AudioDelegateContext {
    level: Arc<Mutex<f32>>,
}

/// Register our custom delegate class (called once)
fn get_audio_delegate_class() -> &'static Class {
    AUDIO_DELEGATE_CLASS.call_once(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("CenteredAudioSampleDelegate", superclass)
            .expect("Failed to create audio delegate class");

        // Add instance variable for context pointer
        decl.add_ivar::<*mut c_void>("_context");

        // Add the delegate method
        unsafe {
            decl.add_method(
                sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
                capture_output_did_output_sample_buffer as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object),
            );
        }

        let cls = decl.register();
        unsafe {
            AUDIO_DELEGATE_CLASS_PTR = cls;
        }
    });

    unsafe { &*AUDIO_DELEGATE_CLASS_PTR }
}

/// The delegate callback for processing audio sample buffers
extern "C" fn capture_output_did_output_sample_buffer(
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
        let context = &*(context_ptr as *const AudioDelegateContext);

        // Get the audio buffer from the sample buffer
        // CMSampleBufferGetDataBuffer returns a CMBlockBuffer
        let block_buffer: *mut Object = CMSampleBufferGetDataBuffer(sample_buffer as *mut c_void) as *mut Object;
        if block_buffer.is_null() {
            return;
        }

        // Get buffer length
        let length = CMBlockBufferGetDataLength(block_buffer as *mut c_void);
        if length == 0 {
            return;
        }

        // Get pointer to audio data
        let mut data_ptr: *mut c_char = std::ptr::null_mut();
        let mut length_out: usize = 0;
        let status = CMBlockBufferGetDataPointer(
            block_buffer as *mut c_void,
            0,
            &mut length_out,
            std::ptr::null_mut(),
            &mut data_ptr,
        );

        if status != 0 || data_ptr.is_null() {
            return;
        }

        // The audio data is typically 16-bit PCM
        // Calculate RMS level
        let samples = std::slice::from_raw_parts(data_ptr as *const i16, length_out / 2);
        let mut sum_squares: f64 = 0.0;
        for &sample in samples {
            let normalized = sample as f64 / 32768.0;
            sum_squares += normalized * normalized;
        }
        let rms = (sum_squares / samples.len() as f64).sqrt() as f32;

        // Update level (with some smoothing)
        if let Ok(mut level) = context.level.lock() {
            // Simple exponential smoothing: new = 0.3 * measured + 0.7 * old
            *level = 0.3 * rms + 0.7 * (*level);
        }
    }
}

// CoreMedia framework functions
#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMSampleBufferGetDataBuffer(sbuf: *mut c_void) -> *mut c_void;
    fn CMBlockBufferGetDataLength(bbuf: *mut c_void) -> usize;
    fn CMBlockBufferGetDataPointer(
        bbuf: *mut c_void,
        offset: usize,
        length_at_offset_out: *mut usize,
        total_length_out: *mut usize,
        data_pointer_out: *mut *mut c_char,
    ) -> i32;
}

// Dispatch queue functions
#[link(name = "System")]
extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
}

/// macOS audio input using AVCaptureSession
pub struct MacOSAudioInput {
    /// AVCaptureSession instance
    session: *mut Object,
    /// AVCaptureDeviceInput
    device_input: *mut Object,
    /// AVCaptureAudioDataOutput
    audio_output: *mut Object,
    /// Delegate for sample buffer callbacks
    delegate: *mut Object,
    /// Dispatch queue for callbacks
    callback_queue: *mut Object,
    /// Delegate context (boxed to ensure stable address)
    delegate_context: Option<Box<AudioDelegateContext>>,
    /// Current state
    state: AudioInputState,
    /// Permission granted
    permission_granted: Arc<AtomicBool>,
    /// Audio info
    info: Option<AudioInfo>,
    /// Sample callback
    sample_callback: Option<AudioSampleCallback>,
    /// Current audio level (RMS)
    level: Arc<Mutex<f32>>,
    /// Sample buffer for callback
    sample_buffer: Arc<Mutex<Vec<f32>>>,
}

// Safety: We ensure thread safety through proper synchronization
unsafe impl Send for MacOSAudioInput {}

impl MacOSAudioInput {
    /// Create a new macOS audio input
    pub fn new() -> Self {
        Self {
            session: ptr::null_mut(),
            device_input: ptr::null_mut(),
            audio_output: ptr::null_mut(),
            delegate: ptr::null_mut(),
            callback_queue: ptr::null_mut(),
            delegate_context: None,
            state: AudioInputState::Idle,
            permission_granted: Arc::new(AtomicBool::new(false)),
            info: None,
            sample_callback: None,
            level: Arc::new(Mutex::new(0.0)),
            sample_buffer: Arc::new(Mutex::new(Vec::new())),
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
}

impl Default for MacOSAudioInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MacOSAudioInput {
    fn drop(&mut self) {
        self.close();
    }
}

impl AudioInputBackend for MacOSAudioInput {
    fn request_permission(&mut self) -> Result<(), AudioError> {
        if self.permission_granted.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.state = AudioInputState::RequestingPermission;

        unsafe {
            // Get AVCaptureDevice class
            let device_class = class!(AVCaptureDevice);

            // Check authorization status for audio
            // AVMediaTypeAudio = "soun"
            let media_type = Self::create_nsstring("soun");
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
                            self.state = AudioInputState::Error;
                            return Err(AudioError::Other("Permission request timed out".into()));
                        }
                        // Run the main run loop briefly to allow the permission dialog to work
                        let run_loop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
                        let date: *mut Object = msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: 0.1f64];
                        let _: () = msg_send![run_loop, runUntilDate: date];
                    }

                    let _: () = msg_send![media_type, release];

                    if granted.load(Ordering::SeqCst) {
                        self.permission_granted.store(true, Ordering::SeqCst);
                        self.state = AudioInputState::Ready;
                        Ok(())
                    } else {
                        self.state = AudioInputState::Error;
                        Err(AudioError::PermissionDenied)
                    }
                }
                1 => {
                    // AVAuthorizationStatusRestricted
                    let _: () = msg_send![media_type, release];
                    self.state = AudioInputState::Error;
                    Err(AudioError::Other("Microphone access restricted by system policy".into()))
                }
                2 => {
                    // AVAuthorizationStatusDenied
                    let _: () = msg_send![media_type, release];
                    self.state = AudioInputState::Error;
                    Err(AudioError::PermissionDenied)
                }
                3 => {
                    // AVAuthorizationStatusAuthorized
                    let _: () = msg_send![media_type, release];
                    self.permission_granted.store(true, Ordering::SeqCst);
                    self.state = AudioInputState::Ready;
                    Ok(())
                }
                _ => {
                    let _: () = msg_send![media_type, release];
                    self.state = AudioInputState::Error;
                    Err(AudioError::Other("Unknown authorization status".into()))
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
            let media_type = Self::create_nsstring("soun");
            let status: i64 = msg_send![device_class, authorizationStatusForMediaType: media_type];
            let _: () = msg_send![media_type, release];

            status == 3 // AVAuthorizationStatusAuthorized
        }
    }

    fn list_devices(&self) -> Result<Vec<AudioInputDevice>, AudioError> {
        let mut devices = Vec::new();

        unsafe {
            // Get AVCaptureDevice class
            let device_class = class!(AVCaptureDevice);

            // Get default audio device
            let media_type = Self::create_nsstring("soun");
            let default_device: *mut Object = msg_send![device_class, defaultDeviceWithMediaType: media_type];
            let _: () = msg_send![media_type, release];

            let default_id = if !default_device.is_null() {
                let uid: *mut Object = msg_send![default_device, uniqueID];
                Self::nsstring_to_string(uid)
            } else {
                None
            };

            // Create discovery session for audio devices
            // Include both built-in and external microphone types
            let builtin_mic = Self::create_nsstring("AVCaptureDeviceTypeBuiltInMicrophone");
            let external = Self::create_nsstring("AVCaptureDeviceTypeExternal");

            // Create array with multiple device types
            let types_array: [*mut Object; 2] = [builtin_mic, external];
            let device_types: *mut Object = msg_send![class!(NSArray), arrayWithObjects:types_array.as_ptr() count:2usize];
            let _: () = msg_send![builtin_mic, release];
            let _: () = msg_send![external, release];

            let media_type = Self::create_nsstring("soun");
            // Use discovery session to find all audio devices
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

                    if let (Some(id), Some(device_name)) = (Self::nsstring_to_string(uid), Self::nsstring_to_string(name)) {
                        let is_default = default_id.as_ref().map(|d| d == &id).unwrap_or(false);
                        devices.push(AudioInputDevice {
                            id,
                            name: device_name,
                            is_default,
                        });
                    }
                }
            }

            // If discovery session didn't find devices, try getting the default directly
            if devices.is_empty() && !default_device.is_null() {
                let uid: *mut Object = msg_send![default_device, uniqueID];
                let name: *mut Object = msg_send![default_device, localizedName];

                if let (Some(id), Some(device_name)) = (Self::nsstring_to_string(uid), Self::nsstring_to_string(name)) {
                    devices.push(AudioInputDevice {
                        id,
                        name: device_name,
                        is_default: true,
                    });
                }
            }
        }

        Ok(devices)
    }

    fn open(&mut self, device_id: Option<&str>, config: &AudioInputConfig) -> Result<(), AudioError> {
        if !self.has_permission() {
            return Err(AudioError::Other("Microphone permission not granted".into()));
        }

        // Close any existing session
        self.close();

        unsafe {
            // Create capture session
            let session: *mut Object = msg_send![class!(AVCaptureSession), alloc];
            let session: *mut Object = msg_send![session, init];

            if session.is_null() {
                return Err(AudioError::Other("Failed to create capture session".into()));
            }

            // Get the device
            let device: *mut Object = if let Some(id) = device_id {
                let ns_id = Self::create_nsstring(id);
                let device: *mut Object = msg_send![class!(AVCaptureDevice), deviceWithUniqueID: ns_id];
                let _: () = msg_send![ns_id, release];
                device
            } else {
                let media_type = Self::create_nsstring("soun");
                let device: *mut Object = msg_send![class!(AVCaptureDevice), defaultDeviceWithMediaType: media_type];
                let _: () = msg_send![media_type, release];
                device
            };

            if device.is_null() {
                let _: () = msg_send![session, release];
                return Err(AudioError::Other("No audio input device found".into()));
            }

            // Create device input
            let mut error: *mut Object = ptr::null_mut();
            let device_input: *mut Object = msg_send![class!(AVCaptureDeviceInput), deviceInputWithDevice:device error:&mut error];

            if device_input.is_null() || !error.is_null() {
                let _: () = msg_send![session, release];
                if !error.is_null() {
                    let desc: *mut Object = msg_send![error, localizedDescription];
                    if let Some(err_str) = Self::nsstring_to_string(desc) {
                        return Err(AudioError::Other(format!("Failed to create device input: {}", err_str)));
                    }
                }
                return Err(AudioError::Other("Failed to create device input".into()));
            }

            // Add input to session
            let can_add: BOOL = msg_send![session, canAddInput: device_input];
            if can_add == NO {
                let _: () = msg_send![session, release];
                return Err(AudioError::Other("Cannot add input to session".into()));
            }
            let _: () = msg_send![session, addInput: device_input];

            // Create audio data output
            let audio_output: *mut Object = msg_send![class!(AVCaptureAudioDataOutput), alloc];
            let audio_output: *mut Object = msg_send![audio_output, init];

            if audio_output.is_null() {
                let _: () = msg_send![session, release];
                return Err(AudioError::Other("Failed to create audio output".into()));
            }

            // Add output to session
            let can_add: BOOL = msg_send![session, canAddOutput: audio_output];
            if can_add == NO {
                let _: () = msg_send![audio_output, release];
                let _: () = msg_send![session, release];
                return Err(AudioError::Other("Cannot add output to session".into()));
            }
            let _: () = msg_send![session, addOutput: audio_output];

            // Create dispatch queue for sample buffer callbacks
            let queue_label = std::ffi::CString::new("com.centered.audio.input").unwrap();
            let callback_queue = dispatch_queue_create(queue_label.as_ptr(), std::ptr::null());
            if callback_queue.is_null() {
                let _: () = msg_send![audio_output, release];
                let _: () = msg_send![session, release];
                return Err(AudioError::Other("Failed to create dispatch queue".into()));
            }

            // Create delegate context with reference to our level Arc
            let context = Box::new(AudioDelegateContext {
                level: self.level.clone(),
            });

            // Create delegate instance
            let delegate_class = get_audio_delegate_class();
            let delegate: *mut Object = msg_send![delegate_class, alloc];
            let delegate: *mut Object = msg_send![delegate, init];
            if delegate.is_null() {
                dispatch_release(callback_queue);
                let _: () = msg_send![audio_output, release];
                let _: () = msg_send![session, release];
                return Err(AudioError::Other("Failed to create delegate".into()));
            }

            // Store context pointer in delegate's ivar
            let context_ptr = Box::into_raw(context) as *mut c_void;
            (*delegate).set_ivar("_context", context_ptr);

            // Set delegate on audio output
            let _: () = msg_send![audio_output, setSampleBufferDelegate:delegate queue:callback_queue];

            self.session = session;
            self.device_input = device_input;
            self.audio_output = audio_output;
            self.delegate = delegate;
            self.callback_queue = callback_queue as *mut Object;
            // Re-box the context so we can manage its lifetime
            self.delegate_context = Some(unsafe { Box::from_raw(context_ptr as *mut AudioDelegateContext) });
            self.info = Some(AudioInfo {
                duration_ms: 0, // Continuous capture
                sample_rate: config.sample_rate,
                channels: config.channels,
                is_stream: true,
            });
            self.state = AudioInputState::Ready;

            Ok(())
        }
    }

    fn start(&mut self) -> Result<(), AudioError> {
        if self.session.is_null() {
            return Err(AudioError::Other("Session not opened".into()));
        }

        unsafe {
            let is_running: BOOL = msg_send![self.session, isRunning];
            if is_running == YES {
                return Ok(()); // Already running
            }

            let _: () = msg_send![self.session, startRunning];
            self.state = AudioInputState::Capturing;
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        if self.session.is_null() {
            return Ok(());
        }

        unsafe {
            let is_running: BOOL = msg_send![self.session, isRunning];
            if is_running == YES {
                let _: () = msg_send![self.session, stopRunning];
            }
            self.state = AudioInputState::Stopped;
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
                if !self.audio_output.is_null() {
                    // Clear the delegate before removing
                    let null: *mut Object = std::ptr::null_mut();
                    let _: () = msg_send![self.audio_output, setSampleBufferDelegate:null queue:null];
                    let _: () = msg_send![self.session, removeOutput: self.audio_output];
                    let _: () = msg_send![self.audio_output, release];
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
        self.audio_output = ptr::null_mut();
        self.delegate = ptr::null_mut();
        self.callback_queue = ptr::null_mut();
        self.delegate_context = None;
        self.info = None;
        self.state = AudioInputState::Idle;
    }

    fn state(&self) -> AudioInputState {
        self.state
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn set_sample_callback(&mut self, callback: Option<AudioSampleCallback>) {
        self.sample_callback = callback;
    }

    fn level(&self) -> f32 {
        *self.level.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_input_creation() {
        let input = MacOSAudioInput::new();
        assert_eq!(input.state(), AudioInputState::Idle);
    }
}
