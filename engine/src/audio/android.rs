//! Android audio playback implementation using JNI
//!
//! Uses Android's MediaPlayer API via JNI for audio playback.
//! For network URLs, uses prepareAsync() to avoid blocking.

#![cfg(target_os = "android")]

use jni::objects::{GlobalRef, JObject, JValue};
use log::{info, error};

use super::{AudioBackend, AudioError, AudioInfo, PlaybackState};

/// Get the JavaVM from the Android platform module
fn get_java_vm() -> Option<&'static jni::JavaVM> {
    unsafe { crate::platform::android::JAVA_VM.as_ref() }
}

/// Android audio player implementation
pub struct AndroidAudioPlayer {
    state: PlaybackState,
    info: Option<AudioInfo>,
    volume: f32,
    looping: bool,
    current_time_ms: u64,
    media_player: Option<GlobalRef>,
}

impl AndroidAudioPlayer {
    pub fn new() -> Self {
        Self {
            state: PlaybackState::Idle,
            info: None,
            volume: 1.0,
            looping: false,
            current_time_ms: 0,
            media_player: None,
        }
    }

    /// Create a MediaPlayer instance via JNI
    fn create_media_player(&mut self) -> Result<GlobalRef, AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        // Create new MediaPlayer instance
        let media_player_class = env.find_class("android/media/MediaPlayer")
            .map_err(|e| AudioError::DeviceError(format!("Failed to find MediaPlayer class: {:?}", e)))?;

        let media_player = env.new_object(media_player_class, "()V", &[])
            .map_err(|e| AudioError::DeviceError(format!("Failed to create MediaPlayer: {:?}", e)))?;

        let global_ref = env.new_global_ref(&media_player)
            .map_err(|e| AudioError::DeviceError(format!("Failed to create global ref: {:?}", e)))?;

        Ok(global_ref)
    }

    /// Set the data source (file path or URL)
    fn set_data_source(&mut self, path: &str, is_url: bool) -> Result<(), AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        let media_player = self.media_player.as_ref()
            .ok_or(AudioError::NotLoaded)?;

        let path_str = env.new_string(path)
            .map_err(|e| AudioError::LoadError(format!("Failed to create string: {:?}", e)))?;

        // Reset the player first
        let _ = env.call_method(media_player.as_obj(), "reset", "()V", &[]);
        let _ = env.exception_clear();

        if is_url {
            // For HTTP/HTTPS URLs, use setDataSource(String) directly
            // This avoids ContentResolver issues and handles network URLs properly
            info!("Setting URL data source: {}", path);

            let result = env.call_method(
                media_player.as_obj(),
                "setDataSource",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&path_str)],
            );

            if env.exception_check().unwrap_or(false) {
                let exception = env.exception_occurred().ok();
                let _ = env.exception_clear();
                if let Some(exc) = exception {
                    let msg: JObject = env.call_method(&exc, "getMessage", "()Ljava/lang/String;", &[])
                        .ok()
                        .and_then(|v| v.l().ok())
                        .unwrap_or(JObject::null());
                    if !msg.is_null() {
                        if let Ok(s) = env.get_string((&msg).into()) {
                            error!("setDataSource exception: {}", s.to_string_lossy());
                        }
                    }
                }
                return Err(AudioError::LoadError(format!("Failed to set data source: {}", path)));
            }

            result.map_err(|e| AudioError::LoadError(format!("Failed to set data source: {:?}", e)))?;

            info!("Set URL data source successfully: {}", path);

            // Use synchronous prepare() - this blocks until ready but is reliable
            // (prepareAsync requires OnPreparedListener which is complex via JNI)
            info!("Preparing media player (this may take a moment for network URLs)...");
            let prepare_result = env.call_method(media_player.as_obj(), "prepare", "()V", &[]);

            if env.exception_check().unwrap_or(false) {
                let exception = env.exception_occurred().ok();
                let _ = env.exception_clear();
                if let Some(exc) = exception {
                    let msg: JObject = env.call_method(&exc, "getMessage", "()Ljava/lang/String;", &[])
                        .ok()
                        .and_then(|v| v.l().ok())
                        .unwrap_or(JObject::null());
                    if !msg.is_null() {
                        if let Ok(s) = env.get_string((&msg).into()) {
                            error!("prepare() exception: {}", s.to_string_lossy());
                        }
                    }
                }
                return Err(AudioError::LoadError("Failed to prepare media from URL".to_string()));
            }

            prepare_result.map_err(|e| AudioError::LoadError(format!("Failed to prepare: {:?}", e)))?;

            // Get duration now that we're prepared
            let duration = env.call_method(media_player.as_obj(), "getDuration", "()I", &[])
                .map_err(|_| AudioError::LoadError("Failed to get duration".to_string()))?
                .i()
                .unwrap_or(0) as u64;

            let _ = env.exception_clear();

            self.info = Some(AudioInfo {
                duration_ms: duration,
                sample_rate: 44100,
                channels: 2,
                is_stream: true,
            });
            self.state = PlaybackState::Paused;
            info!("Audio prepared from URL, duration: {}ms", duration);
        } else {
            // For local files, use simple setDataSource
            let result = env.call_method(
                media_player.as_obj(),
                "setDataSource",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&path_str)],
            );

            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
                return Err(AudioError::LoadError(format!("Failed to set data source: {}", path)));
            }

            result.map_err(|e| AudioError::LoadError(format!("Failed to set data source: {:?}", e)))?;

            // Prepare the player (synchronous for local files)
            let prepare_result = env.call_method(media_player.as_obj(), "prepare", "()V", &[]);

            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
                return Err(AudioError::LoadError("Failed to prepare media".to_string()));
            }

            prepare_result.map_err(|e| AudioError::LoadError(format!("Failed to prepare: {:?}", e)))?;

            // Get duration
            let duration = env.call_method(media_player.as_obj(), "getDuration", "()I", &[])
                .map_err(|_| AudioError::LoadError("Failed to get duration".to_string()))?
                .i()
                .unwrap_or(0) as u64;

            let _ = env.exception_clear();

            self.info = Some(AudioInfo {
                duration_ms: duration,
                sample_rate: 44100,
                channels: 2,
                is_stream: false,
            });

            self.state = PlaybackState::Paused;
            info!("Audio prepared from file, duration: {}ms", duration);
        }

        Ok(())
    }
}

impl AudioBackend for AndroidAudioPlayer {
    fn load_file(&mut self, path: &str) -> Result<(), AudioError> {
        self.state = PlaybackState::Loading;

        // Create MediaPlayer if needed
        if self.media_player.is_none() {
            self.media_player = Some(self.create_media_player()?);
        }

        self.set_data_source(path, false)?;
        info!("Android audio loaded from file: {}", path);
        Ok(())
    }

    fn load_url(&mut self, url: &str) -> Result<(), AudioError> {
        self.state = PlaybackState::Loading;
        info!("Android audio loading from URL: {}", url);

        // Create MediaPlayer if needed
        if self.media_player.is_none() {
            self.media_player = Some(self.create_media_player()?);
        }

        self.set_data_source(url, true)?;
        info!("Android audio loaded from URL: {}", url);
        Ok(())
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn play(&mut self) -> Result<(), AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        let media_player = self.media_player.as_ref()
            .ok_or(AudioError::NotLoaded)?;

        let result = env.call_method(media_player.as_obj(), "start", "()V", &[]);
        let _ = env.exception_clear();

        result.map_err(|e| AudioError::Other(format!("Failed to play: {:?}", e)))?;
        self.state = PlaybackState::Playing;
        info!("Android audio playing");
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        let media_player = self.media_player.as_ref()
            .ok_or(AudioError::NotLoaded)?;

        let result = env.call_method(media_player.as_obj(), "pause", "()V", &[]);
        let _ = env.exception_clear();

        result.map_err(|e| AudioError::Other(format!("Failed to pause: {:?}", e)))?;
        self.state = PlaybackState::Paused;
        info!("Android audio paused");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        let media_player = self.media_player.as_ref()
            .ok_or(AudioError::NotLoaded)?;

        let result = env.call_method(media_player.as_obj(), "stop", "()V", &[]);
        let _ = env.exception_clear();

        result.map_err(|e| AudioError::Other(format!("Failed to stop: {:?}", e)))?;
        self.current_time_ms = 0;
        self.state = PlaybackState::Ended;
        info!("Android audio stopped");
        Ok(())
    }

    fn seek(&mut self, timestamp_ms: u64) -> Result<(), AudioError> {
        let vm = get_java_vm().ok_or(AudioError::UnsupportedPlatform)?;
        let mut env = vm.attach_current_thread().map_err(|_| AudioError::UnsupportedPlatform)?;

        let media_player = self.media_player.as_ref()
            .ok_or(AudioError::NotLoaded)?;

        let result = env.call_method(
            media_player.as_obj(),
            "seekTo",
            "(I)V",
            &[JValue::Int(timestamp_ms as i32)],
        );
        let _ = env.exception_clear();

        result.map_err(|e| AudioError::SeekError(format!("Failed to seek: {:?}", e)))?;
        self.current_time_ms = timestamp_ms;
        info!("Android audio seeked to {}ms", timestamp_ms);
        Ok(())
    }

    fn set_looping(&mut self, looping: bool) {
        self.looping = looping;

        if let Some(ref media_player) = self.media_player {
            if let Some(vm) = get_java_vm() {
                if let Ok(mut env) = vm.attach_current_thread() {
                    let _ = env.call_method(
                        media_player.as_obj(),
                        "setLooping",
                        "(Z)V",
                        &[JValue::Bool(looping as u8)],
                    );
                    let _ = env.exception_clear();
                }
            }
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);

        if let Some(ref media_player) = self.media_player {
            if let Some(vm) = get_java_vm() {
                if let Ok(mut env) = vm.attach_current_thread() {
                    let _ = env.call_method(
                        media_player.as_obj(),
                        "setVolume",
                        "(FF)V",
                        &[JValue::Float(self.volume), JValue::Float(self.volume)],
                    );
                    let _ = env.exception_clear();
                }
            }
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
        if let Some(ref media_player) = self.media_player {
            if let Some(vm) = get_java_vm() {
                if let Ok(mut env) = vm.attach_current_thread() {
                    if let Ok(result) = env.call_method(
                        media_player.as_obj(),
                        "getCurrentPosition",
                        "()I",
                        &[],
                    ) {
                        let _ = env.exception_clear();
                        if let Ok(pos) = result.i() {
                            return pos as u64;
                        }
                    }
                    let _ = env.exception_clear();
                }
            }
        }
        self.current_time_ms
    }

    fn update(&mut self) {
        // Check if playback has completed
        if self.state == PlaybackState::Playing {
            if let Some(ref media_player) = self.media_player {
                if let Some(vm) = get_java_vm() {
                    if let Ok(mut env) = vm.attach_current_thread() {
                        if let Ok(result) = env.call_method(
                            media_player.as_obj(),
                            "isPlaying",
                            "()Z",
                            &[],
                        ) {
                            let _ = env.exception_clear();
                            if let Ok(playing) = result.z() {
                                if !playing && !self.looping {
                                    self.state = PlaybackState::Ended;
                                }
                            }
                        }
                        let _ = env.exception_clear();
                    }
                }
            }
        }
    }
}

impl Default for AndroidAudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AndroidAudioPlayer {
    fn drop(&mut self) {
        // Release the MediaPlayer
        if let Some(ref media_player) = self.media_player {
            if let Some(vm) = get_java_vm() {
                if let Ok(mut env) = vm.attach_current_thread() {
                    let _ = env.call_method(media_player.as_obj(), "release", "()V", &[]);
                    let _ = env.exception_clear();
                }
            }
        }
    }
}
