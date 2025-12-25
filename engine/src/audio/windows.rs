//! Windows audio playback using WASAPI and Media Foundation
//!
//! Uses Windows Audio Session API (WASAPI) for audio playback and
//! Media Foundation for decoding compressed audio formats (MP3, M4A, AAC, etc.).

use super::{AudioBackend, AudioError, AudioInfo, PlaybackState};
use std::path::Path;
use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::*;

/// Global COM initialization flag
static COM_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Global Media Foundation initialization flag
static MF_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Ensure COM is initialized
fn ensure_com_initialized() {
    COM_INITIALIZED.get_or_init(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        true
    });
}

/// Ensure Media Foundation is initialized
fn ensure_mf_initialized() {
    ensure_com_initialized();
    MF_INITIALIZED.get_or_init(|| {
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        }
        true
    });
}

/// Windows audio backend using WASAPI
pub struct WindowsAudioBackend {
    /// Audio info
    info: Option<AudioInfo>,
    /// Current playback state
    state: PlaybackState,
    /// Volume (0.0 - 1.0)
    volume: f32,
    /// Whether audio should loop
    looping: bool,
    /// Current playback position in milliseconds
    current_time_ms: u64,
    /// Audio client
    audio_client: Option<IAudioClient>,
    /// Render client
    render_client: Option<IAudioRenderClient>,
    /// Audio data
    audio_data: Option<Vec<f32>>,
    /// Sample rate
    sample_rate: u32,
    /// Number of channels
    channels: u32,
    /// Buffer frame count
    buffer_frame_count: u32,
    /// Current sample position
    current_sample: usize,
}

// SAFETY: WASAPI objects are thread-safe when used correctly
unsafe impl Send for WindowsAudioBackend {}

impl WindowsAudioBackend {
    pub fn new() -> Self {
        ensure_com_initialized();

        Self {
            info: None,
            state: PlaybackState::Idle,
            volume: 1.0,
            looping: false,
            current_time_ms: 0,
            audio_client: None,
            render_client: None,
            audio_data: None,
            sample_rate: 44100,
            channels: 2,
            buffer_frame_count: 0,
            current_sample: 0,
        }
    }

    /// Initialize WASAPI audio client
    fn init_audio_client(&mut self) -> std::result::Result<(), AudioError> {
        unsafe {
            // Get default audio endpoint
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(
                &MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            ).map_err(|e| AudioError::DeviceError(format!("Failed to create device enumerator: {:?}", e)))?;

            // eRender = 0, eConsole = 0
            let device: IMMDevice = enumerator
                .GetDefaultAudioEndpoint(EDataFlow(0), ERole(0))
                .map_err(|e| AudioError::DeviceError(format!("Failed to get default audio endpoint: {:?}", e)))?;

            // Activate audio client
            let audio_client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| AudioError::DeviceError(format!("Failed to activate audio client: {:?}", e)))?;

            // Get mix format
            let mix_format = audio_client
                .GetMixFormat()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get mix format: {:?}", e)))?;

            // Store format info
            self.sample_rate = (*mix_format).nSamplesPerSec;
            self.channels = (*mix_format).nChannels as u32;

            // Initialize audio client in shared mode
            // AUDCLNT_SHAREMODE_SHARED = 0
            // AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM = 0x80000000
            // AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY = 0x08000000
            let buffer_duration = 10_000_000i64; // 1 second in 100-nanosecond units
            let stream_flags = 0x80000000u32 | 0x08000000u32;

            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                stream_flags,
                buffer_duration,
                0,
                mix_format,
                None,
            ).map_err(|e| AudioError::DeviceError(format!("Failed to initialize audio client: {:?}", e)))?;

            // Get buffer size
            self.buffer_frame_count = audio_client
                .GetBufferSize()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get buffer size: {:?}", e)))?;

            // Get render client
            let render_client: IAudioRenderClient = audio_client
                .GetService()
                .map_err(|e| AudioError::DeviceError(format!("Failed to get render client: {:?}", e)))?;

            // Free the mix format
            CoTaskMemFree(Some(mix_format as *const _ as *const std::ffi::c_void));

            self.audio_client = Some(audio_client);
            self.render_client = Some(render_client);

            Ok(())
        }
    }

    /// Load audio data from file using a simple WAV parser
    fn load_wav(&mut self, path: &str) -> std::result::Result<(), AudioError> {
        let data = std::fs::read(path)
            .map_err(|e| AudioError::LoadError(format!("Failed to read file: {}", e)))?;

        // Very simple WAV parser - just check for basic format
        if data.len() < 44 {
            return Err(AudioError::FormatError("File too small for WAV".to_string()));
        }

        // Check RIFF header
        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(AudioError::FormatError("Not a valid WAV file".to_string()));
        }

        // Parse fmt chunk
        let channels = u16::from_le_bytes([data[22], data[23]]) as u32;
        let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let bits_per_sample = u16::from_le_bytes([data[34], data[35]]);

        // Find data chunk
        let mut data_offset = 36;
        let mut data_size = 0u32;
        while data_offset + 8 < data.len() {
            let chunk_id = &data[data_offset..data_offset + 4];
            let chunk_size = u32::from_le_bytes([
                data[data_offset + 4],
                data[data_offset + 5],
                data[data_offset + 6],
                data[data_offset + 7],
            ]);

            if chunk_id == b"data" {
                data_size = chunk_size;
                data_offset += 8;
                break;
            }
            data_offset += 8 + chunk_size as usize;
        }

        if data_size == 0 {
            return Err(AudioError::FormatError("No data chunk found".to_string()));
        }

        // Convert audio data to f32
        let samples: Vec<f32> = match bits_per_sample {
            16 => {
                data[data_offset..data_offset + data_size as usize]
                    .chunks(2)
                    .map(|b| {
                        let sample = i16::from_le_bytes([b[0], b.get(1).copied().unwrap_or(0)]);
                        sample as f32 / 32768.0
                    })
                    .collect()
            }
            8 => {
                data[data_offset..data_offset + data_size as usize]
                    .iter()
                    .map(|&b| (b as f32 - 128.0) / 128.0)
                    .collect()
            }
            _ => return Err(AudioError::FormatError(format!("Unsupported bit depth: {}", bits_per_sample))),
        };

        let duration_ms = (samples.len() as u64 * 1000) / (sample_rate as u64 * channels as u64);

        self.audio_data = Some(samples);
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.info = Some(AudioInfo {
            duration_ms,
            sample_rate,
            channels,
            is_stream: false,
        });

        Ok(())
    }

    /// Load audio using Media Foundation (supports MP3, M4A, AAC, FLAC, etc.)
    fn load_with_media_foundation(&mut self, path: &str) -> std::result::Result<(), AudioError> {
        ensure_mf_initialized();

        unsafe {
            // Convert path to wide string
            let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

            // Create source reader from file
            let source_reader: IMFSourceReader = MFCreateSourceReaderFromURL(
                PCWSTR(wide_path.as_ptr()),
                None,
            ).map_err(|e| AudioError::LoadError(format!("Failed to create source reader: {:?}", e)))?;

            // Configure source reader to output PCM audio
            let output_type: IMFMediaType = MFCreateMediaType()
                .map_err(|e| AudioError::FormatError(format!("Failed to create media type: {:?}", e)))?;

            output_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(|e| AudioError::FormatError(format!("Failed to set major type: {:?}", e)))?;

            output_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)
                .map_err(|e| AudioError::FormatError(format!("Failed to set subtype: {:?}", e)))?;

            source_reader.SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                None,
                &output_type,
            ).map_err(|e| AudioError::FormatError(format!("Failed to set output type: {:?}", e)))?;

            // Get the actual output type (with sample rate, channels, etc.)
            let actual_type = source_reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32)
                .map_err(|e| AudioError::FormatError(format!("Failed to get current type: {:?}", e)))?;

            let channels = actual_type.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)
                .unwrap_or(2);
            let sample_rate = actual_type.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
                .unwrap_or(44100);

            // Read all audio samples
            let mut samples: Vec<f32> = Vec::new();
            loop {
                let mut flags: u32 = 0;
                let mut sample: Option<IMFSample> = None;
                source_reader.ReadSample(
                    MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                    0,
                    None,
                    Some(&mut flags),
                    None,
                    Some(&mut sample),
                ).map_err(|e| AudioError::DecodeError(format!("Failed to read sample: {:?}", e)))?;

                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }

                if let Some(sample) = sample {
                    // Get buffer from sample
                    let buffer = sample.ConvertToContiguousBuffer()
                        .map_err(|e| AudioError::DecodeError(format!("Failed to get buffer: {:?}", e)))?;

                    let mut data_ptr: *mut u8 = std::ptr::null_mut();
                    let mut data_len: u32 = 0;
                    buffer.Lock(&mut data_ptr, None, Some(&mut data_len))
                        .map_err(|e| AudioError::DecodeError(format!("Failed to lock buffer: {:?}", e)))?;

                    // Copy float samples
                    let float_slice = std::slice::from_raw_parts(
                        data_ptr as *const f32,
                        data_len as usize / 4,
                    );
                    samples.extend_from_slice(float_slice);

                    buffer.Unlock()
                        .map_err(|e| AudioError::DecodeError(format!("Failed to unlock buffer: {:?}", e)))?;
                }
            }

            if samples.is_empty() {
                return Err(AudioError::DecodeError("No audio data decoded".to_string()));
            }

            let duration_ms = (samples.len() as u64 * 1000) / (sample_rate as u64 * channels as u64);

            self.audio_data = Some(samples);
            self.sample_rate = sample_rate;
            self.channels = channels;
            self.info = Some(AudioInfo {
                duration_ms,
                sample_rate,
                channels,
                is_stream: false,
            });

            Ok(())
        }
    }
}

impl AudioBackend for WindowsAudioBackend {
    fn load_file(&mut self, path: &str) -> std::result::Result<(), AudioError> {
        self.state = PlaybackState::Loading;

        // Check file extension
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "wav" => self.load_wav(path)?,
            "mp3" | "m4a" | "aac" | "flac" | "wma" | "ogg" => {
                // Use Media Foundation for compressed formats
                self.load_with_media_foundation(path)?;
            }
            _ => {
                // Try Media Foundation first for unknown formats
                if self.load_with_media_foundation(path).is_err() {
                    // Fall back to WAV parser
                    self.load_wav(path).map_err(|_| {
                        AudioError::FormatError(format!("Unsupported audio format: {}", ext))
                    })?;
                }
            }
        }

        // Initialize audio client
        self.init_audio_client()?;

        self.state = PlaybackState::Paused;
        self.current_sample = 0;
        self.current_time_ms = 0;

        Ok(())
    }

    fn load_url(&mut self, url: &str) -> std::result::Result<(), AudioError> {
        // URL loading would require downloading the file first
        // or using Media Foundation's streaming capabilities
        Err(AudioError::LoadError(format!(
            "URL loading not yet implemented on Windows: {}",
            url
        )))
    }

    fn info(&self) -> Option<&AudioInfo> {
        self.info.as_ref()
    }

    fn play(&mut self) -> std::result::Result<(), AudioError> {
        if let Some(audio_client) = &self.audio_client {
            unsafe {
                audio_client.Start()
                    .map_err(|e| AudioError::Other(format!("Failed to start playback: {:?}", e)))?;
            }
            self.state = PlaybackState::Playing;
        }
        Ok(())
    }

    fn pause(&mut self) -> std::result::Result<(), AudioError> {
        if let Some(audio_client) = &self.audio_client {
            unsafe {
                audio_client.Stop()
                    .map_err(|e| AudioError::Other(format!("Failed to stop playback: {:?}", e)))?;
            }
            self.state = PlaybackState::Paused;
        }
        Ok(())
    }

    fn stop(&mut self) -> std::result::Result<(), AudioError> {
        if let Some(audio_client) = &self.audio_client {
            unsafe {
                audio_client.Stop()
                    .map_err(|e| AudioError::Other(format!("Failed to stop playback: {:?}", e)))?;
                audio_client.Reset()
                    .map_err(|e| AudioError::Other(format!("Failed to reset playback: {:?}", e)))?;
            }
        }
        self.current_sample = 0;
        self.current_time_ms = 0;
        self.state = PlaybackState::Paused;
        Ok(())
    }

    fn seek(&mut self, timestamp_ms: u64) -> std::result::Result<(), AudioError> {
        if let Some(info) = &self.info {
            if timestamp_ms > info.duration_ms {
                return Err(AudioError::SeekError("Seek position beyond duration".to_string()));
            }

            // Calculate sample position
            let sample_position = (timestamp_ms * self.sample_rate as u64 * self.channels as u64) / 1000;
            self.current_sample = sample_position as usize;
            self.current_time_ms = timestamp_ms;
        }
        Ok(())
    }

    fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
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
        self.current_time_ms
    }

    fn update(&mut self) {
        if self.state != PlaybackState::Playing {
            return;
        }

        // Fill audio buffer
        unsafe {
            if let (Some(audio_client), Some(render_client), Some(audio_data)) =
                (&self.audio_client, &self.render_client, &self.audio_data)
            {
                // Get current padding
                let padding = audio_client.GetCurrentPadding().unwrap_or(0);
                let frames_available = self.buffer_frame_count - padding;

                if frames_available > 0 {
                    // Get buffer
                    if let Ok(buffer) = render_client.GetBuffer(frames_available) {
                        let samples_per_frame = self.channels as usize;
                        let samples_to_write = (frames_available as usize) * samples_per_frame;
                        let buffer_slice = std::slice::from_raw_parts_mut(
                            buffer as *mut f32,
                            samples_to_write,
                        );

                        for i in 0..samples_to_write {
                            if self.current_sample < audio_data.len() {
                                buffer_slice[i] = audio_data[self.current_sample] * self.volume;
                                self.current_sample += 1;
                            } else if self.looping {
                                self.current_sample = 0;
                                buffer_slice[i] = audio_data[self.current_sample] * self.volume;
                                self.current_sample += 1;
                            } else {
                                buffer_slice[i] = 0.0;
                            }
                        }

                        let _ = render_client.ReleaseBuffer(frames_available, 0);

                        // Update time
                        self.current_time_ms = (self.current_sample as u64 * 1000)
                            / (self.sample_rate as u64 * self.channels as u64);

                        // Check if playback ended
                        if self.current_sample >= audio_data.len() && !self.looping {
                            self.state = PlaybackState::Ended;
                        }
                    }
                }
            }
        }
    }
}

impl Default for WindowsAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = WindowsAudioBackend::new();
        assert_eq!(backend.state(), PlaybackState::Idle);
        assert_eq!(backend.volume(), 1.0);
        assert!(!backend.is_looping());
    }
}
