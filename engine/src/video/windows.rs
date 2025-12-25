//! Windows video decoder using Media Foundation
//!
//! Uses IMFSourceReader for video decoding with hardware acceleration
//! via DXVA (DirectX Video Acceleration) for H.264, HEVC, and other codecs.

use super::{VideoDecoder, VideoError, VideoFrame, VideoInfo};
use std::ptr;
use std::sync::OnceLock;

use windows::core::{GUID, PCWSTR, PROPVARIANT};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::System::Com::*;

/// Global Media Foundation initialization flag
static MF_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Ensure Media Foundation is initialized
fn ensure_mf_initialized() -> std::result::Result<(), VideoError> {
    MF_INITIALIZED.get_or_init(|| {
        unsafe {
            // Initialize COM
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // Initialize Media Foundation
            if MFStartup(MF_VERSION, MFSTARTUP_FULL).is_err() {
                return false;
            }
            true
        }
    });

    if *MF_INITIALIZED.get().unwrap_or(&false) {
        Ok(())
    } else {
        Err(VideoError::LoadError("Failed to initialize Media Foundation".into()))
    }
}

/// Windows video decoder using Media Foundation
pub struct WindowsVideoDecoder {
    /// Source reader for video decoding
    source_reader: Option<IMFSourceReader>,
    /// Video info
    info: VideoInfo,
    /// Current playback position in milliseconds
    current_time_ms: u64,
    /// Whether we've reached the end
    ended: bool,
}

unsafe impl Send for WindowsVideoDecoder {}

impl WindowsVideoDecoder {
    /// Create decoder from a URL (file:// or http://)
    pub fn from_url(url: &str) -> std::result::Result<Self, VideoError> {
        ensure_mf_initialized()?;

        // For remote URLs, download first (Media Foundation can handle some URLs directly,
        // but local files are more reliable)
        if url.starts_with("http://") || url.starts_with("https://") {
            let temp_path = Self::download_to_temp(url)?;
            return Self::from_file(&temp_path);
        }

        // Handle file:// URLs
        let path = if url.starts_with("file://") {
            &url[7..]
        } else {
            url
        };

        Self::from_file(path)
    }

    /// Download a remote URL to a temporary file
    fn download_to_temp(url: &str) -> std::result::Result<String, VideoError> {
        use std::io::Write;

        // Extract extension from URL for proper temp file naming
        let extension = url
            .split('?')
            .next()
            .and_then(|path| path.rsplit('.').next())
            .filter(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "webm" | "mov" | "avi" | "mkv"))
            .unwrap_or("mp4");

        // Create temp file path with timestamp for uniqueness
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let file_name = format!("centered_video_{}_{}.{}", std::process::id(), timestamp, extension);
        let temp_path = temp_dir.join(file_name);
        let temp_path_str = temp_path.to_string_lossy().to_string();

        // Download using WinHTTP
        let response = winhttp_download(url)
            .map_err(|e| VideoError::LoadError(format!("Failed to download: {}", e)))?;

        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| VideoError::LoadError(format!("Failed to create temp file: {}", e)))?;
        file.write_all(&response)
            .map_err(|e| VideoError::LoadError(format!("Failed to write temp file: {}", e)))?;

        Ok(temp_path_str)
    }

    /// Create decoder from a file path
    pub fn from_file(path: &str) -> std::result::Result<Self, VideoError> {
        ensure_mf_initialized()?;

        unsafe {
            // Create source reader from URL
            let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

            // Create attributes for the source reader
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes as *mut _, 2)
                .map_err(|e| VideoError::LoadError(format!("Failed to create attributes: {:?}", e)))?;
            let attributes = attributes.ok_or_else(|| VideoError::LoadError("No attributes returned".to_string()))?;

            // Enable hardware acceleration
            attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .ok();

            // Enable video processing for format conversion
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .ok();

            // Create source reader
            let source_reader = MFCreateSourceReaderFromURL(
                PCWSTR::from_raw(wide_path.as_ptr()),
                &attributes,
            )
            .map_err(|e| VideoError::LoadError(format!("Failed to create source reader: {:?}", e)))?;

            // First, select the video stream
            source_reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .ok();
            source_reader
                .SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true)
                .map_err(|e| VideoError::LoadError(format!("Failed to select video stream: {:?}", e)))?;

            // Try to configure output to RGB32 format (preferred for direct GPU upload)
            // MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING allows format conversion
            let media_type: IMFMediaType = MFCreateMediaType()
                .map_err(|e| VideoError::LoadError(format!("Failed to create media type: {:?}", e)))?;

            media_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| VideoError::FormatError(format!("Failed to set major type: {:?}", e)))?;

            media_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                .map_err(|e| VideoError::FormatError(format!("Failed to set subtype: {:?}", e)))?;

            // Try RGB32 first, fall back to other formats if not supported
            let format_set = source_reader
                .SetCurrentMediaType(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    None,
                    &media_type,
                )
                .is_ok();

            if !format_set {
                // Try ARGB32
                media_type
                    .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32)
                    .ok();
                let argb_set = source_reader
                    .SetCurrentMediaType(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        None,
                        &media_type,
                    )
                    .is_ok();

                if !argb_set {
                    // Just use the native format and we'll convert later
                    // Get native format info for dimensions
                }
            }

            // Get the actual output media type
            let output_type = source_reader
                .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
                .map_err(|e| VideoError::FormatError(format!("Failed to get output type: {:?}", e)))?;

            // Get video dimensions (packed as UINT64: width << 32 | height)
            let frame_size = output_type
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .map_err(|e| VideoError::FormatError(format!("Failed to get frame size: {:?}", e)))?;
            let width = (frame_size >> 32) as u32;
            let height = (frame_size & 0xFFFFFFFF) as u32;

            // Get frame rate (packed as UINT64: numerator << 32 | denominator)
            let frame_rate = match output_type.GetUINT64(&MF_MT_FRAME_RATE) {
                Ok(rate) => {
                    let num = (rate >> 32) as u32;
                    let den = (rate & 0xFFFFFFFF) as u32;
                    if den > 0 { num as f32 / den as f32 } else { 30.0 }
                }
                Err(_) => 30.0,
            };

            // Get duration using PROPVARIANT
            let duration_100ns = source_reader
                .GetPresentationAttribute(
                    MF_SOURCE_READER_MEDIASOURCE.0 as u32,
                    &MF_PD_DURATION,
                )
                .ok()
                .and_then(|prop| {
                    // Try to extract u64 value from PROPVARIANT
                    // VT_UI8 (unsigned 64-bit integer)
                    if let Ok(val) = u64::try_from(&prop) {
                        Some(val / 10_000) // Convert 100ns to ms
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            let info = VideoInfo {
                width,
                height,
                duration_ms: duration_100ns,
                frame_rate,
                is_live: false,
            };

            Ok(Self {
                source_reader: Some(source_reader),
                info,
                current_time_ms: 0,
                ended: false,
            })
        }
    }

    /// Convert BGRA to RGBA
    fn bgra_to_rgba(bgra: &[u8], width: usize, height: usize, stride: usize) -> Vec<u8> {
        let mut rgba = vec![0u8; width * height * 4];

        for y in 0..height {
            for x in 0..width {
                let src_idx = y * stride + x * 4;
                let dst_idx = (y * width + x) * 4;

                if src_idx + 3 < bgra.len() {
                    rgba[dst_idx] = bgra[src_idx + 2]; // R from B
                    rgba[dst_idx + 1] = bgra[src_idx + 1]; // G
                    rgba[dst_idx + 2] = bgra[src_idx]; // B from R
                    rgba[dst_idx + 3] = 255; // A (RGB32 doesn't have alpha)
                }
            }
        }

        rgba
    }
}

impl Drop for WindowsVideoDecoder {
    fn drop(&mut self) {
        // IMFSourceReader will be released automatically via Drop
        self.source_reader = None;
    }
}

impl VideoDecoder for WindowsVideoDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_frame(&mut self) -> Option<VideoFrame> {
        if self.ended {
            return None;
        }

        let source_reader = self.source_reader.as_ref()?;

        unsafe {
            let mut stream_index = 0u32;
            let mut flags = 0u32;
            let mut timestamp = 0i64;
            let mut sample: Option<IMFSample> = None;

            let result = source_reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                Some(&mut stream_index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            );

            if result.is_err() {
                self.ended = true;
                return None;
            }

            // Check for end of stream
            if (flags as i32) & MF_SOURCE_READERF_ENDOFSTREAM.0 != 0 {
                self.ended = true;
                return None;
            }

            // Check for stream change
            if (flags as i32) & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 != 0 {
                // Handle format change if needed
            }

            let sample = sample?;

            // Get the media buffer from the sample
            let buffer = sample.ConvertToContiguousBuffer().ok()?;

            // Lock the buffer
            let mut data_ptr: *mut u8 = ptr::null_mut();
            let mut max_length = 0u32;
            let mut current_length = 0u32;

            buffer
                .Lock(&mut data_ptr, Some(&mut max_length), Some(&mut current_length))
                .ok()?;

            if data_ptr.is_null() || current_length == 0 {
                buffer.Unlock().ok();
                return None;
            }

            // Copy and convert the data
            let width = self.info.width as usize;
            let height = self.info.height as usize;
            let stride = width * 4; // RGB32 is 4 bytes per pixel

            let data_slice = std::slice::from_raw_parts(data_ptr, current_length as usize);

            // RGB32 from Media Foundation with video processing is top-down, no flip needed
            // Just convert BGRA to RGBA
            let rgba = Self::bgra_to_rgba(data_slice, width, height, stride);

            // Unlock the buffer
            buffer.Unlock().ok();

            // Update timestamp (convert from 100ns to ms)
            self.current_time_ms = (timestamp / 10_000) as u64;

            Some(VideoFrame {
                width: self.info.width,
                height: self.info.height,
                data: rgba,
                timestamp_ms: self.current_time_ms,
            })
        }
    }

    fn seek(&mut self, timestamp_ms: u64) -> std::result::Result<(), VideoError> {
        let source_reader = self.source_reader.as_ref().ok_or(VideoError::NotLoaded)?;

        unsafe {
            // Convert ms to 100ns units
            let position_100ns = (timestamp_ms * 10_000) as i64;

            // Create PROPVARIANT with the position using from() conversion
            let prop = PROPVARIANT::from(position_100ns);

            source_reader
                .SetCurrentPosition(&GUID::zeroed(), &prop)
                .map_err(|e| VideoError::SeekError(format!("Seek failed: {:?}", e)))?;

            self.current_time_ms = timestamp_ms;
            self.ended = false;

            Ok(())
        }
    }

    fn has_more_frames(&self) -> bool {
        !self.ended
    }

    fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }
}

/// Download a file using WinHTTP
fn winhttp_download(url: &str) -> std::result::Result<Vec<u8>, String> {
    unsafe {
        // Parse the URL to extract host, path, and whether it's HTTPS
        let is_https = url.starts_with("https://");
        let url_without_scheme = if is_https {
            &url[8..]
        } else if url.starts_with("http://") {
            &url[7..]
        } else {
            return Err("Invalid URL scheme".to_string());
        };

        // Split host and path
        let (host, path) = match url_without_scheme.find('/') {
            Some(idx) => (&url_without_scheme[..idx], &url_without_scheme[idx..]),
            None => (url_without_scheme, "/"),
        };

        // Convert to wide strings
        let user_agent: Vec<u16> = "CenteredEngine/1.0\0".encode_utf16().collect();
        let host_wide: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        // Open WinHTTP session
        let session = WinHttpOpen(
            PCWSTR(user_agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return Err("Failed to open WinHTTP session".to_string());
        }

        // Connect to server
        let port = if is_https {
            INTERNET_DEFAULT_HTTPS_PORT
        } else {
            INTERNET_DEFAULT_HTTP_PORT
        };
        let connection = WinHttpConnect(session, PCWSTR(host_wide.as_ptr()), port, 0);
        if connection.is_null() {
            WinHttpCloseHandle(session);
            return Err("Failed to connect to server".to_string());
        }

        // Open request
        let get_wide: Vec<u16> = "GET\0".encode_utf16().collect();
        let flags = if is_https { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) };
        let request = WinHttpOpenRequest(
            connection,
            PCWSTR(get_wide.as_ptr()),
            PCWSTR(path_wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        );
        if request.is_null() {
            WinHttpCloseHandle(connection);
            WinHttpCloseHandle(session);
            return Err("Failed to open request".to_string());
        }

        // Send request
        if WinHttpSendRequest(request, None, None, 0, 0, 0).is_err() {
            WinHttpCloseHandle(request);
            WinHttpCloseHandle(connection);
            WinHttpCloseHandle(session);
            return Err("Failed to send request".to_string());
        }

        // Receive response
        if WinHttpReceiveResponse(request, std::ptr::null_mut()).is_err() {
            WinHttpCloseHandle(request);
            WinHttpCloseHandle(connection);
            WinHttpCloseHandle(session);
            return Err("Failed to receive response".to_string());
        }

        // Read data
        let mut data = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let mut bytes_read = 0u32;
            if WinHttpReadData(
                request,
                buffer.as_mut_ptr() as *mut _,
                buffer.len() as u32,
                &mut bytes_read,
            )
            .is_err()
            {
                break;
            }
            if bytes_read == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..bytes_read as usize]);
        }

        // Cleanup
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connection);
        WinHttpCloseHandle(session);

        if data.is_empty() {
            return Err("No data received".to_string());
        }

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgra_to_rgba() {
        let bgra = vec![
            255, 0, 0, 255, // BGRA blue -> RGBA red
            0, 255, 0, 255, // BGRA green -> RGBA green
            0, 0, 255, 255, // BGRA red -> RGBA blue
            255, 255, 255, 255, // BGRA white -> RGBA white
        ];
        let rgba = WindowsVideoDecoder::bgra_to_rgba(&bgra, 2, 2, 8);

        assert_eq!(rgba[0], 0); // R (was B)
        assert_eq!(rgba[1], 0); // G
        assert_eq!(rgba[2], 255); // B (was R)
        assert_eq!(rgba[3], 255); // A
    }
}
