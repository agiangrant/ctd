//! Windows audio input using WASAPI
//!
//! Uses Windows Audio Session API (WASAPI) for audio capture.
//! Supports device enumeration and selection.

use super::input::{AudioInputBackend, AudioInputConfig, AudioInputDevice, AudioInputState, AudioSampleCallback};
use super::{AudioError, AudioInfo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::ptr;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

/// WAVE_FORMAT_IEEE_FLOAT format tag
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;

/// WAVE_FORMAT_EXTENSIBLE format tag
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// KSDATAFORMAT_SUBTYPE_IEEE_FLOAT GUID
const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

/// Ensure COM is initialized for this thread.
/// CoInitializeEx is safe to call multiple times per thread - it will return
/// S_FALSE if already initialized, which we ignore.
fn ensure_com_initialized() {
    unsafe {
        // Call CoInitializeEx on every invocation - it's safe to call multiple times
        // per thread and will return S_FALSE if already initialized.
        // This is necessary because Go may call FFI functions from different OS threads.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

/// Windows audio input using WASAPI
pub struct WindowsAudioInput {
    /// Audio client for capture
    audio_client: Option<IAudioClient>,
    /// Capture client
    capture_client: Option<IAudioCaptureClient>,
    /// Current device endpoint
    device: Option<IMMDevice>,
    /// Current state
    state: AudioInputState,
    /// Permission granted (Windows doesn't require explicit permission)
    permission_granted: Arc<AtomicBool>,
    /// Audio info
    info: Option<AudioInfo>,
    /// Sample callback
    sample_callback: Option<AudioSampleCallback>,
    /// Current audio level (RMS)
    level: Arc<Mutex<f32>>,
    /// Sample rate
    sample_rate: u32,
    /// Number of channels
    channels: u32,
    /// Bits per sample
    bits_per_sample: u32,
    /// Whether the format is float (vs int16)
    is_float_format: bool,
    /// Buffer frame count
    buffer_frame_count: u32,
    /// Whether capture is running
    is_capturing: bool,
}

// Safety: We ensure thread safety through proper synchronization
unsafe impl Send for WindowsAudioInput {}

impl WindowsAudioInput {
    pub fn new() -> Self {
        ensure_com_initialized();

        Self {
            audio_client: None,
            capture_client: None,
            device: None,
            state: AudioInputState::Idle,
            permission_granted: Arc::new(AtomicBool::new(true)), // Windows doesn't require explicit permission
            info: None,
            sample_callback: None,
            level: Arc::new(Mutex::new(0.0)),
            sample_rate: 44100,
            channels: 1,
            bits_per_sample: 16,
            is_float_format: false,
            buffer_frame_count: 0,
            is_capturing: false,
        }
    }

    /// Get the device enumerator
    fn get_enumerator() -> std::result::Result<IMMDeviceEnumerator, AudioError> {
        unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| AudioError::DeviceError(format!("Failed to create device enumerator: {:?}", e)))
        }
    }

    /// Get property value as string
    unsafe fn get_device_name(device: &IMMDevice) -> Option<String> {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;

        // PKEY_Device_FriendlyName
        let key = PROPERTYKEY {
            fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
            pid: 14,
        };

        let value = store.GetValue(&key).ok()?;

        // Get the string from PROPVARIANT using the to_string() method
        // which handles VT_LPWSTR and other string types
        let s = value.to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    /// Get device ID as string
    unsafe fn get_device_id(device: &IMMDevice) -> Option<String> {
        let id = device.GetId().ok()?;
        let len = (0..).take_while(|&i| *id.0.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(id.0, len);
        let result = String::from_utf16_lossy(slice);
        CoTaskMemFree(Some(id.0 as *const _));
        Some(result)
    }

    /// Process available audio data and update level meter.
    /// Call this periodically (e.g., every frame) while capturing.
    pub fn update(&mut self) {
        if !self.is_capturing {
            return;
        }

        // Ensure COM is initialized for this thread (Go may call from different threads)
        ensure_com_initialized();

        let channels = self.channels as usize;
        let is_float = self.is_float_format;
        let bits_per_sample = self.bits_per_sample;

        if let Some(capture_client) = &self.capture_client {
            unsafe {
                // Process all available packets
                loop {
                    // Get next packet size
                    let packet_length = match capture_client.GetNextPacketSize() {
                        Ok(len) => len,
                        Err(_) => break,
                    };

                    if packet_length == 0 {
                        break;
                    }

                    // Get buffer
                    let mut data: *mut u8 = ptr::null_mut();
                    let mut num_frames = 0u32;
                    let mut flags = 0u32;

                    if capture_client
                        .GetBuffer(&mut data, &mut num_frames, &mut flags, None, None)
                        .is_ok()
                    {
                        // Check for AUDCLNT_BUFFERFLAGS_SILENT (0x2)
                        let is_silent = (flags & 0x2) != 0;

                        if !data.is_null() && num_frames > 0 && channels > 0 && !is_silent {
                            let num_samples = (num_frames as usize) * channels;

                            // Calculate RMS level based on sample format
                            let rms = if is_float {
                                // Float format (32-bit)
                                let samples = std::slice::from_raw_parts(
                                    data as *const f32,
                                    num_samples,
                                );

                                let mut sum_squares: f64 = 0.0;
                                for &sample in samples {
                                    let s = sample as f64;
                                    sum_squares += s * s;
                                }
                                (sum_squares / num_samples as f64).sqrt() as f32
                            } else if bits_per_sample == 16 {
                                // Int16 format
                                let samples = std::slice::from_raw_parts(
                                    data as *const i16,
                                    num_samples,
                                );

                                let mut sum_squares: f64 = 0.0;
                                for &sample in samples {
                                    // Normalize to -1.0 to 1.0 range
                                    let s = (sample as f64) / 32768.0;
                                    sum_squares += s * s;
                                }
                                (sum_squares / num_samples as f64).sqrt() as f32
                            } else if bits_per_sample == 24 {
                                // Int24 format (packed as 3 bytes per sample)
                                let byte_count = num_samples * 3;
                                let bytes = std::slice::from_raw_parts(data, byte_count);

                                let mut sum_squares: f64 = 0.0;
                                for i in 0..num_samples {
                                    let offset = i * 3;
                                    // Little-endian 24-bit signed integer
                                    let sample = ((bytes[offset] as i32) |
                                                  ((bytes[offset + 1] as i32) << 8) |
                                                  ((bytes[offset + 2] as i32) << 16)) << 8 >> 8;
                                    // Normalize to -1.0 to 1.0 range
                                    let s = (sample as f64) / 8388608.0;
                                    sum_squares += s * s;
                                }
                                (sum_squares / num_samples as f64).sqrt() as f32
                            } else if bits_per_sample == 32 {
                                // Int32 format
                                let samples = std::slice::from_raw_parts(
                                    data as *const i32,
                                    num_samples,
                                );

                                let mut sum_squares: f64 = 0.0;
                                for &sample in samples {
                                    // Normalize to -1.0 to 1.0 range
                                    let s = (sample as f64) / 2147483648.0;
                                    sum_squares += s * s;
                                }
                                (sum_squares / num_samples as f64).sqrt() as f32
                            } else {
                                // Unknown format - assume silence
                                0.0
                            };

                            // Update level with smoothing, and amplify for visibility
                            // Microphone levels can be very quiet (1e-6 to 1e-3 range typically)
                            // We need significant amplification to show a visible level
                            if let Ok(mut lvl) = self.level.lock() {
                                // Amplify significantly - typical speaking is around 0.01-0.1 RMS
                                // but WASAPI returns values in the 1e-6 to 1e-3 range
                                let amplified = (rms * 500.0).min(1.0);
                                *lvl = 0.3 * amplified + 0.7 * (*lvl);
                            }
                        }

                        // Release buffer
                        let _ = capture_client.ReleaseBuffer(num_frames);
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

impl Default for WindowsAudioInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsAudioInput {
    fn drop(&mut self) {
        self.close();
    }
}

impl AudioInputBackend for WindowsAudioInput {
    fn request_permission(&mut self) -> std::result::Result<(), AudioError> {
        // Windows doesn't require explicit microphone permission in desktop apps
        // The permission is implicitly granted when accessing the device
        // (UWP apps do require it, but this is for Win32)
        self.permission_granted.store(true, Ordering::SeqCst);
        self.state = AudioInputState::Ready;
        Ok(())
    }

    fn has_permission(&self) -> bool {
        // Windows desktop apps don't require explicit permission
        self.permission_granted.load(Ordering::SeqCst)
    }

    fn list_devices(&self) -> std::result::Result<Vec<AudioInputDevice>, AudioError> {
        let mut devices = Vec::new();

        unsafe {
            let enumerator = Self::get_enumerator()?;

            // Get default capture device ID
            // eCapture = 1, eConsole = 0
            let default_device = enumerator.GetDefaultAudioEndpoint(EDataFlow(1), ERole(0)).ok();
            let default_id = default_device.as_ref().and_then(|d| Self::get_device_id(d));

            // Enumerate all capture devices
            let collection = enumerator
                .EnumAudioEndpoints(EDataFlow(1), DEVICE_STATE_ACTIVE)
                .map_err(|e| AudioError::DeviceError(format!("Failed to enumerate devices: {:?}", e)))?;

            let count = collection
                .GetCount()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get device count: {:?}", e)))?;

            for i in 0..count {
                if let Ok(device) = collection.Item(i) {
                    let id = Self::get_device_id(&device);
                    let name = Self::get_device_name(&device);

                    if let (Some(id), Some(name)) = (id, name) {
                        let is_default = default_id.as_ref().map(|d| d == &id).unwrap_or(false);
                        devices.push(AudioInputDevice {
                            id,
                            name,
                            is_default,
                        });
                    }
                }
            }
        }

        Ok(devices)
    }

    fn open(&mut self, device_id: Option<&str>, config: &AudioInputConfig) -> std::result::Result<(), AudioError> {
        // Close any existing capture
        self.close();

        unsafe {
            let enumerator = Self::get_enumerator()?;

            // Get the device
            let device = if let Some(id) = device_id {
                // Convert device ID to wide string
                let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
                enumerator
                    .GetDevice(PCWSTR::from_raw(wide.as_ptr()))
                    .map_err(|e| AudioError::DeviceError(format!("Failed to get device: {:?}", e)))?
            } else {
                // eCapture = 1, eConsole = 0
                enumerator
                    .GetDefaultAudioEndpoint(EDataFlow(1), ERole(0))
                    .map_err(|e| AudioError::DeviceError(format!("Failed to get default capture device: {:?}", e)))?
            };

            // Activate audio client
            let audio_client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| AudioError::DeviceError(format!("Failed to activate audio client: {:?}", e)))?;

            // Get the mix format
            let mix_format = audio_client
                .GetMixFormat()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get mix format: {:?}", e)))?;

            // Store format info
            self.sample_rate = (*mix_format).nSamplesPerSec;
            self.channels = (*mix_format).nChannels as u32;
            self.bits_per_sample = (*mix_format).wBitsPerSample as u32;

            // Determine if this is a float format
            let format_tag = (*mix_format).wFormatTag;
            self.is_float_format = if format_tag == WAVE_FORMAT_IEEE_FLOAT {
                true
            } else if format_tag == WAVE_FORMAT_EXTENSIBLE {
                // For extensible format, check the subformat GUID
                // The WAVEFORMATEXTENSIBLE struct follows WAVEFORMATEX
                // Use read_unaligned because the struct is packed
                let ext_ptr = mix_format as *const WAVEFORMATEXTENSIBLE;
                let subformat_ptr = std::ptr::addr_of!((*ext_ptr).SubFormat);
                let subformat = std::ptr::read_unaligned(subformat_ptr);
                subformat == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                false
            };

            // Initialize audio client for capture in shared mode
            // AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM = 0x80000000
            // AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY = 0x08000000
            let buffer_duration = 10_000_000i64; // 1 second in 100-nanosecond units
            let stream_flags = 0x80000000u32 | 0x08000000u32;
            audio_client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    stream_flags,
                    buffer_duration,
                    0,
                    mix_format,
                    None,
                )
                .map_err(|e| AudioError::DeviceError(format!("Failed to initialize audio client: {:?}", e)))?;

            // Get buffer size
            self.buffer_frame_count = audio_client
                .GetBufferSize()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get buffer size: {:?}", e)))?;

            // Get capture client
            let capture_client: IAudioCaptureClient = audio_client
                .GetService()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get capture client: {:?}", e)))?;

            // Free the mix format
            CoTaskMemFree(Some(mix_format as *const _ as *const std::ffi::c_void));

            self.audio_client = Some(audio_client);
            self.capture_client = Some(capture_client);
            self.device = Some(device);
            self.info = Some(AudioInfo {
                duration_ms: 0, // Continuous capture
                sample_rate: self.sample_rate,
                channels: self.channels,
                is_stream: true,
            });
            self.state = AudioInputState::Ready;

            Ok(())
        }
    }

    fn start(&mut self) -> std::result::Result<(), AudioError> {
        if self.audio_client.is_none() {
            return Err(AudioError::Other("Device not opened".into()));
        }

        if self.is_capturing {
            return Ok(()); // Already capturing
        }

        unsafe {
            if let Some(audio_client) = &self.audio_client {
                audio_client
                    .Start()
                    .map_err(|e| AudioError::Other(format!("Failed to start capture: {:?}", e)))?;
            }
        }

        self.is_capturing = true;
        self.state = AudioInputState::Capturing;

        // Note: Audio processing is done in the update() method which should be called
        // periodically from the main thread. This avoids threading issues with COM objects.

        Ok(())
    }

    fn stop(&mut self) -> std::result::Result<(), AudioError> {
        if !self.is_capturing {
            return Ok(());
        }

        unsafe {
            if let Some(audio_client) = &self.audio_client {
                let _ = audio_client.Stop();
            }
        }

        self.is_capturing = false;
        self.state = AudioInputState::Stopped;

        Ok(())
    }

    fn close(&mut self) {
        self.stop().ok();

        self.audio_client = None;
        self.capture_client = None;
        self.device = None;
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
        let input = WindowsAudioInput::new();
        assert_eq!(input.state(), AudioInputState::Idle);
    }

    #[test]
    fn test_has_permission() {
        let input = WindowsAudioInput::new();
        // Windows desktop apps don't require explicit permission
        assert!(input.has_permission());
    }
}
