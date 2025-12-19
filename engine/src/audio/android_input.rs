//! Android audio input (microphone) implementation using JNI
//!
//! Uses Android's AudioRecord API via JNI for microphone capture.
//! Permission handling is done through the activity.

#![cfg(target_os = "android")]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::JNIEnv;
use log::info;

use super::input::{AudioInputBackend, AudioInputConfig, AudioInputDevice, AudioInputState, AudioSampleCallback};
use super::{AudioError, AudioInfo};

/// Get the JavaVM from the Android platform module
fn get_java_vm() -> Option<&'static jni::JavaVM> {
    unsafe { crate::platform::android::JAVA_VM.as_ref() }
}

/// Android audio input implementation
pub struct AndroidAudioInput {
    state: AudioInputState,
    has_permission: bool,
    config: Option<AudioInputConfig>,
    info: Option<AudioInfo>,
    callback: Option<AudioSampleCallback>,
    level: Arc<Mutex<f32>>,
    is_capturing: Arc<AtomicBool>,
}

impl AndroidAudioInput {
    pub fn new() -> Self {
        Self {
            state: AudioInputState::Idle,
            has_permission: false,
            config: None,
            info: None,
            callback: None,
            level: Arc::new(Mutex::new(0.0)),
            is_capturing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if microphone permission is granted via JNI
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

        // Check RECORD_AUDIO permission
        // checkSelfPermission returns PackageManager.PERMISSION_GRANTED (0) if granted
        let permission_name = match env.new_string("android.permission.RECORD_AUDIO") {
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

    /// Request microphone permission via JNI
    fn request_permission_jni(&mut self) -> Result<(), AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        let activity_ptr = crate::platform::android::get_activity_ptr();
        if activity_ptr.is_null() {
            return Err(AudioError::UnsupportedPlatform);
        }

        let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

        // Request RECORD_AUDIO permission
        // requestPermissions(String[], int)
        let permission_name = env.new_string("android.permission.RECORD_AUDIO")
            .map_err(|e| AudioError::Other(format!("JNI error: {:?}", e)))?;

        // Create String[] array
        let string_class = env.find_class("java/lang/String")
            .map_err(|e| AudioError::Other(format!("JNI error: {:?}", e)))?;
        let permissions_array = env.new_object_array(1, string_class, &permission_name)
            .map_err(|e| AudioError::Other(format!("JNI error: {:?}", e)))?;

        let result = env.call_method(
            &*activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[JValue::Object(&permissions_array), JValue::Int(1001)], // Request code 1001
        );

        let _ = env.exception_clear();

        match result {
            Ok(_) => {
                self.state = AudioInputState::RequestingPermission;
                Ok(())
            }
            Err(e) => Err(AudioError::Other(format!("Failed to request permission: {:?}", e))),
        }
    }
}

impl AudioInputBackend for AndroidAudioInput {
    fn request_permission(&mut self) -> Result<(), AudioError> {
        // First check if we already have permission
        if self.check_permission_jni() {
            self.has_permission = true;
            self.state = AudioInputState::Ready;
            return Ok(());
        }

        // Request permission
        self.request_permission_jni()
    }

    fn has_permission(&self) -> bool {
        self.check_permission_jni()
    }

    fn list_devices(&self) -> Result<Vec<AudioInputDevice>, AudioError> {
        // On Android, we typically just use the default microphone
        // More advanced device enumeration would require AudioManager
        Ok(vec![AudioInputDevice {
            id: "default".to_string(),
            name: "Default Microphone".to_string(),
            is_default: true,
        }])
    }

    fn open(&mut self, _device_id: Option<&str>, config: &AudioInputConfig) -> Result<(), AudioError> {
        if !self.has_permission() {
            return Err(AudioError::PermissionDenied);
        }

        self.config = Some(config.clone());
        self.info = Some(AudioInfo {
            duration_ms: 0,
            sample_rate: config.sample_rate,
            channels: config.channels,
            is_stream: true,
        });

        self.state = AudioInputState::Ready;
        info!("Android audio input opened: {}Hz, {} channels", config.sample_rate, config.channels);
        Ok(())
    }

    fn start(&mut self) -> Result<(), AudioError> {
        if self.state != AudioInputState::Ready && self.state != AudioInputState::Stopped {
            return Err(AudioError::Other("Invalid state for start".to_string()));
        }

        // TODO: Start AudioRecord via JNI
        // For now, we'll just set the state
        self.is_capturing.store(true, Ordering::SeqCst);
        self.state = AudioInputState::Capturing;
        info!("Android audio input started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        if self.state != AudioInputState::Capturing {
            return Ok(());
        }

        // TODO: Stop AudioRecord via JNI
        self.is_capturing.store(false, Ordering::SeqCst);
        self.state = AudioInputState::Stopped;
        info!("Android audio input stopped");
        Ok(())
    }

    fn close(&mut self) {
        let _ = self.stop();
        self.config = None;
        self.info = None;
        self.state = AudioInputState::Idle;
        info!("Android audio input closed");
    }

    fn state(&self) -> AudioInputState {
        self.state
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn set_sample_callback(&mut self, callback: Option<AudioSampleCallback>) {
        self.callback = callback;
    }

    fn level(&self) -> f32 {
        *self.level.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for AndroidAudioInput {
    fn default() -> Self {
        Self::new()
    }
}

// JNI callback for receiving audio samples from Kotlin
// This would be called from the Kotlin AudioRecord wrapper
#[no_mangle]
pub extern "system" fn Java_com_centered_demo_CenteredActivity_nativeOnAudioSamples(
    _env: JNIEnv,
    _class: JClass,
    _samples: jni::objects::JFloatArray,
    _timestamp_ns: jni::sys::jlong,
) {
    // TODO: Process audio samples and forward to callback
    info!("Audio samples received from Android");
}
