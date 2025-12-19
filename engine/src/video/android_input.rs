//! Android video input (camera) implementation using JNI
//!
//! Uses Android's Camera2 API via JNI for camera capture.
//! Permission handling is done through the activity.

#![cfg(target_os = "android")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::JNIEnv;
use log::info;

use super::input::{
    CameraPosition, PixelFormat, VideoFrame, VideoFrameCallback, VideoInputBackend,
    VideoInputConfig, VideoInputDevice, VideoInputError, VideoInputState,
};

/// Get the JavaVM from the Android platform module
fn get_java_vm() -> Option<&'static jni::JavaVM> {
    unsafe { crate::platform::android::JAVA_VM.as_ref() }
}

/// Android video input implementation
pub struct AndroidVideoInput {
    state: VideoInputState,
    has_permission: bool,
    config: Option<VideoInputConfig>,
    dimensions: Option<(u32, u32)>,
    callback: Option<VideoFrameCallback>,
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
    is_capturing: Arc<AtomicBool>,
}

impl AndroidVideoInput {
    pub fn new() -> Self {
        Self {
            state: VideoInputState::Idle,
            has_permission: false,
            config: None,
            dimensions: None,
            callback: None,
            latest_frame: Arc::new(Mutex::new(None)),
            is_capturing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if camera permission is granted via JNI
    fn check_permission_jni(&self) -> bool {
        let vm = match get_java_vm() {
            Some(vm) => vm,
            None => return false,
        };

        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(_) => return false,
        };

        // Get the activity
        let activity_ptr = crate::platform::android::get_activity_ptr();
        if activity_ptr.is_null() {
            return false;
        }

        let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

        // Check CAMERA permission
        let permission_name = match env.new_string("android.permission.CAMERA") {
            Ok(s) => s,
            Err(_) => return false,
        };

        let result = env.call_method(
            &*activity,
            "checkSelfPermission",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&permission_name)],
        );

        let _ = env.exception_clear();

        match result {
            Ok(val) => val.i().unwrap_or(-1) == 0, // PERMISSION_GRANTED = 0
            Err(_) => false,
        }
    }

    /// Request camera permission via JNI
    fn request_permission_jni(&mut self) -> Result<(), VideoInputError> {
        let vm = get_java_vm().ok_or(VideoInputError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| VideoInputError::UnsupportedPlatform)?;

        let activity_ptr = crate::platform::android::get_activity_ptr();
        if activity_ptr.is_null() {
            return Err(VideoInputError::UnsupportedPlatform);
        }

        let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

        // Request CAMERA permission
        let permission_name = env.new_string("android.permission.CAMERA")
            .map_err(|e| VideoInputError::Other(format!("JNI error: {:?}", e)))?;

        let string_class = env.find_class("java/lang/String")
            .map_err(|e| VideoInputError::Other(format!("JNI error: {:?}", e)))?;
        let permissions_array = env.new_object_array(1, string_class, &permission_name)
            .map_err(|e| VideoInputError::Other(format!("JNI error: {:?}", e)))?;

        let result = env.call_method(
            &*activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[JValue::Object(&permissions_array), JValue::Int(1002)], // Request code 1002
        );

        let _ = env.exception_clear();

        match result {
            Ok(_) => {
                self.state = VideoInputState::RequestingPermission;
                Ok(())
            }
            Err(e) => Err(VideoInputError::Other(format!("Failed to request permission: {:?}", e))),
        }
    }

    /// List available cameras via JNI
    fn list_devices_jni(&self) -> Result<Vec<VideoInputDevice>, VideoInputError> {
        let vm = get_java_vm().ok_or(VideoInputError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| VideoInputError::UnsupportedPlatform)?;

        // Get the activity
        let activity_ptr = crate::platform::android::get_activity_ptr();
        if activity_ptr.is_null() {
            return Err(VideoInputError::UnsupportedPlatform);
        }

        let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

        // Get CameraManager service
        let camera_service = env.new_string("camera")
            .map_err(|e| VideoInputError::Other(format!("JNI error: {:?}", e)))?;

        let camera_manager = env.call_method(
            &*activity,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&camera_service)],
        );

        let _ = env.exception_clear();

        let camera_manager = match camera_manager {
            Ok(cm) => match cm.l() {
                Ok(obj) => obj,
                Err(_) => {
                    return Ok(vec![
                        VideoInputDevice {
                            id: "0".to_string(),
                            name: "Back Camera".to_string(),
                            position: CameraPosition::Back,
                            is_default: true,
                            resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
                        },
                        VideoInputDevice {
                            id: "1".to_string(),
                            name: "Front Camera".to_string(),
                            position: CameraPosition::Front,
                            is_default: false,
                            resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
                        },
                    ]);
                }
            },
            Err(_) => {
                // Fallback to default devices if we can't enumerate
                return Ok(vec![
                    VideoInputDevice {
                        id: "0".to_string(),
                        name: "Back Camera".to_string(),
                        position: CameraPosition::Back,
                        is_default: true,
                        resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
                    },
                    VideoInputDevice {
                        id: "1".to_string(),
                        name: "Front Camera".to_string(),
                        position: CameraPosition::Front,
                        is_default: false,
                        resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
                    },
                ]);
            }
        };

        // Get camera IDs
        let camera_ids = env.call_method(
            &camera_manager,
            "getCameraIdList",
            "()[Ljava/lang/String;",
            &[],
        );

        let _ = env.exception_clear();

        // For now, return default devices
        // Full implementation would enumerate camera IDs and get their characteristics
        Ok(vec![
            VideoInputDevice {
                id: "0".to_string(),
                name: "Back Camera".to_string(),
                position: CameraPosition::Back,
                is_default: true,
                resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
            },
            VideoInputDevice {
                id: "1".to_string(),
                name: "Front Camera".to_string(),
                position: CameraPosition::Front,
                is_default: false,
                resolutions: vec![(1920, 1080), (1280, 720), (640, 480)],
            },
        ])
    }
}

impl VideoInputBackend for AndroidVideoInput {
    fn request_permission(&mut self) -> Result<(), VideoInputError> {
        // First check if we already have permission
        if self.check_permission_jni() {
            self.has_permission = true;
            self.state = VideoInputState::Ready;
            return Ok(());
        }

        // Request permission
        self.request_permission_jni()
    }

    fn has_permission(&self) -> bool {
        self.check_permission_jni()
    }

    fn list_devices(&self) -> Result<Vec<VideoInputDevice>, VideoInputError> {
        self.list_devices_jni()
    }

    fn open(&mut self, device_id: Option<&str>, config: &VideoInputConfig) -> Result<(), VideoInputError> {
        if !self.has_permission() {
            return Err(VideoInputError::PermissionDenied);
        }

        let camera_id = device_id.unwrap_or("0");
        self.config = Some(config.clone());
        self.dimensions = Some((config.width, config.height));

        self.state = VideoInputState::Ready;
        info!("Android video input opened: camera {}, {}x{}", camera_id, config.width, config.height);
        Ok(())
    }

    fn start(&mut self) -> Result<(), VideoInputError> {
        if self.state != VideoInputState::Ready && self.state != VideoInputState::Stopped {
            return Err(VideoInputError::Other("Invalid state for start".to_string()));
        }

        // TODO: Start Camera2 capture via JNI
        self.is_capturing.store(true, Ordering::SeqCst);
        self.state = VideoInputState::Capturing;
        info!("Android video input started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), VideoInputError> {
        if self.state != VideoInputState::Capturing {
            return Ok(());
        }

        // TODO: Stop Camera2 capture via JNI
        self.is_capturing.store(false, Ordering::SeqCst);
        self.state = VideoInputState::Stopped;
        info!("Android video input stopped");
        Ok(())
    }

    fn close(&mut self) {
        let _ = self.stop();
        self.config = None;
        self.dimensions = None;
        self.state = VideoInputState::Idle;
        info!("Android video input closed");
    }

    fn state(&self) -> VideoInputState {
        self.state
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }

    fn set_frame_callback(&mut self, callback: Option<VideoFrameCallback>) {
        self.callback = callback;
    }

    fn latest_frame(&self) -> Option<VideoFrame> {
        self.latest_frame.lock().ok()?.clone()
    }
}

impl Default for AndroidVideoInput {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-safe storage for the video input callback
lazy_static::lazy_static! {
    static ref VIDEO_FRAME_CALLBACK: Mutex<Option<VideoFrameCallback>> = Mutex::new(None);
}

/// Set the video frame callback (called from Rust side)
pub fn set_video_frame_callback(callback: Option<VideoFrameCallback>) {
    if let Ok(mut guard) = VIDEO_FRAME_CALLBACK.lock() {
        *guard = callback;
    }
}

// JNI callback for receiving video frames from Kotlin
// This would be called from the Kotlin Camera2 wrapper
#[no_mangle]
pub extern "system" fn Java_com_centered_demo_CenteredActivity_nativeOnVideoFrame(
    mut env: JNIEnv,
    _class: JClass,
    data: jni::objects::JByteArray,
    width: jni::sys::jint,
    height: jni::sys::jint,
    timestamp_ns: jni::sys::jlong,
) {
    // Get the byte array data
    let data_len = match env.get_array_length(&data) {
        Ok(len) => len as usize,
        Err(_) => return,
    };

    let mut buffer = vec![0u8; data_len];
    if env.get_byte_array_region(&data, 0, unsafe {
        std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut i8, data_len)
    }).is_err() {
        return;
    }

    let frame = VideoFrame {
        width: width as u32,
        height: height as u32,
        data: buffer,
        timestamp_ns: timestamp_ns as u64,
        pixel_format: PixelFormat::RGBA,
    };

    // Call the callback if set
    if let Ok(guard) = VIDEO_FRAME_CALLBACK.lock() {
        if let Some(ref callback) = *guard {
            callback(frame);
        }
    }
}
