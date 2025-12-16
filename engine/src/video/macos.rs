//! macOS video decoder using AVFoundation
//!
//! Uses AVAsset, AVAssetReader, and AVAssetReaderTrackOutput for video decoding.
//! This provides hardware-accelerated decoding via VideoToolbox for H.264, HEVC, etc.

use super::{VideoDecoder, VideoError, VideoFrame, VideoInfo};
use core_foundation::base::CFRelease;
use objc::runtime::{Object, BOOL, NO};
use objc::{msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::ptr;

// AVFoundation types
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    // AVMediaTypeVideo string constant
    static AVMediaTypeVideo: *mut Object;
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMTimeMake(value: i64, timescale: i32) -> CMTime;
    fn CMTimeGetSeconds(time: CMTime) -> f64;
    fn CMSampleBufferGetPresentationTimeStamp(sbuf: *const c_void) -> CMTime;
    fn CMSampleBufferGetImageBuffer(sbuf: *const c_void) -> *const c_void;
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferGetWidth(pixelBuffer: *const c_void) -> usize;
    fn CVPixelBufferGetHeight(pixelBuffer: *const c_void) -> usize;
    fn CVPixelBufferLockBaseAddress(pixelBuffer: *const c_void, lockFlags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixelBuffer: *const c_void, lockFlags: u64) -> i32;
    fn CVPixelBufferGetBaseAddress(pixelBuffer: *const c_void) -> *const u8;
    fn CVPixelBufferGetBytesPerRow(pixelBuffer: *const c_void) -> usize;
}

/// Core Media time structure
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

impl CMTime {
    fn invalid() -> Self {
        CMTime {
            value: 0,
            timescale: 0,
            flags: 0,
            epoch: 0,
        }
    }

    fn to_milliseconds(&self) -> u64 {
        if self.timescale == 0 {
            return 0;
        }
        ((self.value as f64 / self.timescale as f64) * 1000.0) as u64
    }
}

/// macOS video decoder using AVFoundation
pub struct MacOSVideoDecoder {
    /// AVAsset object
    asset: *mut Object,
    /// AVAssetReader object
    asset_reader: *mut Object,
    /// AVAssetReaderTrackOutput for video
    video_output: *mut Object,
    /// Video track
    video_track: *mut Object,
    /// Video metadata
    info: VideoInfo,
    /// Current playback time
    current_time_ms: u64,
    /// Whether we've reached the end
    ended: bool,
}

// Safety: The Objective-C objects are thread-safe when accessed properly
unsafe impl Send for MacOSVideoDecoder {}

impl MacOSVideoDecoder {
    /// Create decoder from a URL (file:// or http://)
    pub fn from_url(url: &str) -> Result<Self, VideoError> {
        unsafe {
            // For remote URLs, we need to download the video first
            // AVAssetReader doesn't support streaming - it needs a local file
            if url.starts_with("http://") || url.starts_with("https://") {
                // Download the video to a temp file
                // AVAssetReader doesn't support streaming - it needs a local file
                let temp_path = Self::download_to_temp(url)?;
                let ns_url = Self::create_file_url(&temp_path)?;
                return Self::from_nsurl(ns_url);
            }

            // Local file URL
            let path = if url.starts_with("file://") {
                &url[7..]
            } else {
                url
            };
            let ns_url = Self::create_file_url(path)?;

            if ns_url.is_null() {
                return Err(VideoError::LoadError(format!("Failed to create URL: {}", url)));
            }

            Self::from_nsurl(ns_url)
        }
    }

    /// Download a remote URL to a temporary file
    fn download_to_temp(url: &str) -> Result<String, VideoError> {
        use std::io::Write;

        // Create temp file path
        let temp_dir = std::env::temp_dir();
        let file_name = format!("centered_video_{}.mp4", std::process::id());
        let temp_path = temp_dir.join(file_name);
        let temp_path_str = temp_path.to_string_lossy().to_string();

        unsafe {
            let ns_url = Self::create_http_url(url)?;

            // Use NSData's dataWithContentsOfURL - synchronous but works on both macOS and iOS
            // This is simpler than NSURLSession with blocks and works reliably
            let data: *mut Object = msg_send![class!(NSData), dataWithContentsOfURL: ns_url];

            if data.is_null() {
                return Err(VideoError::LoadError(format!("Failed to download video from: {}", url)));
            }

            // Get data bytes
            let length: usize = msg_send![data, length];
            let bytes: *const u8 = msg_send![data, bytes];

            if length == 0 || bytes.is_null() {
                return Err(VideoError::LoadError("Downloaded empty data".into()));
            }

            // Write to temp file
            let data_slice = std::slice::from_raw_parts(bytes, length);
            let mut file = std::fs::File::create(&temp_path)
                .map_err(|e| VideoError::LoadError(format!("Failed to create temp file: {}", e)))?;
            file.write_all(data_slice)
                .map_err(|e| VideoError::LoadError(format!("Failed to write temp file: {}", e)))?;

            Ok(temp_path_str)
        }
    }

    unsafe fn create_http_url(url: &str) -> Result<*mut Object, VideoError> {
        let ns_string: *mut Object = msg_send![class!(NSString), alloc];
        let ns_string: *mut Object = msg_send![ns_string, initWithBytes:url.as_ptr()
                                                                 length:url.len()
                                                               encoding:4u64]; // NSUTF8StringEncoding

        if ns_string.is_null() {
            return Err(VideoError::LoadError("Failed to create NSString".into()));
        }

        let ns_url: *mut Object = msg_send![class!(NSURL), URLWithString: ns_string];
        let _: () = msg_send![ns_string, release];

        if ns_url.is_null() {
            return Err(VideoError::LoadError(format!("Failed to create URL: {}", url)));
        }

        Ok(ns_url)
    }

    /// Create decoder from a file path
    pub fn from_file(path: &str) -> Result<Self, VideoError> {
        unsafe {
            let ns_url = Self::create_file_url(path)?;
            Self::from_nsurl(ns_url)
        }
    }

    unsafe fn create_file_url(path: &str) -> Result<*mut Object, VideoError> {
        let ns_string: *mut Object = msg_send![class!(NSString), alloc];
        let ns_string: *mut Object = msg_send![ns_string, initWithBytes:path.as_ptr()
                                                                 length:path.len()
                                                               encoding:4u64]; // NSUTF8StringEncoding

        if ns_string.is_null() {
            return Err(VideoError::LoadError("Failed to create NSString".into()));
        }

        let ns_url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_string];
        let _: () = msg_send![ns_string, release];

        if ns_url.is_null() {
            return Err(VideoError::LoadError(format!("Failed to create file URL: {}", path)));
        }

        Ok(ns_url)
    }

    unsafe fn from_nsurl(ns_url: *mut Object) -> Result<Self, VideoError> {
        // Create AVURLAsset (better for remote URLs)
        let asset: *mut Object = msg_send![class!(AVURLAsset), URLAssetWithURL:ns_url options:ptr::null::<Object>()];
        if asset.is_null() {
            return Err(VideoError::LoadError("Failed to create AVURLAsset".into()));
        }
        let _: () = msg_send![asset, retain];

        // For remote URLs, we need to explicitly request loading of several keys
        // tracks - for video/audio track access
        // playable - to ensure the asset can be played
        // duration - for timeline info
        let tracks_key: *mut Object = msg_send![class!(NSString), stringWithUTF8String: b"tracks\0".as_ptr()];
        let playable_key: *mut Object = msg_send![class!(NSString), stringWithUTF8String: b"playable\0".as_ptr()];
        let duration_key: *mut Object = msg_send![class!(NSString), stringWithUTF8String: b"duration\0".as_ptr()];

        // Create array with all keys to load
        let keys: [*mut Object; 3] = [tracks_key, playable_key, duration_key];
        let keys_array: *mut Object = msg_send![class!(NSArray), arrayWithObjects:keys.as_ptr() count:3usize];

        // Request asynchronous loading - this triggers the actual network request
        // for remote URLs. We pass nil for completionHandler and poll the status instead,
        // since creating ObjC blocks from Rust is complex.
        let _: () = msg_send![asset, loadValuesAsynchronouslyForKeys:keys_array completionHandler:ptr::null::<Object>()];

        // Poll for completion with timeout - wait for ALL keys to be loaded
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(30);

        loop {
            let mut all_loaded = true;

            for key in &[tracks_key, playable_key, duration_key] {
                let mut load_error: *mut Object = ptr::null_mut();
                let status: i64 = msg_send![asset, statusOfValueForKey:*key error:&mut load_error];

                match status {
                    2 => { /* Loaded - good */ }
                    3 | 4 => {
                        // Failed or Cancelled
                        eprintln!("Asset key loading failed with status {}", status);
                        let _: () = msg_send![asset, release];
                        return Err(VideoError::LoadError("Failed to load asset".into()));
                    }
                    _ => {
                        all_loaded = false;
                    }
                }
            }

            if all_loaded {
                break;
            }

            if start.elapsed() > timeout {
                let _: () = msg_send![asset, release];
                return Err(VideoError::LoadError("Timeout loading asset".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Check if asset is playable
        let is_playable: BOOL = msg_send![asset, isPlayable];
        if is_playable == NO {
            let _: () = msg_send![asset, release];
            return Err(VideoError::FormatError("Asset is not playable".into()));
        }

        // Get video tracks
        let tracks: *mut Object = msg_send![asset, tracksWithMediaType: AVMediaTypeVideo];
        let track_count: usize = msg_send![tracks, count];

        if track_count == 0 {
            let _: () = msg_send![asset, release];
            return Err(VideoError::FormatError("No video tracks found".into()));
        }

        let video_track: *mut Object = msg_send![tracks, objectAtIndex: 0usize];
        let _: () = msg_send![video_track, retain];

        // Get video properties
        let natural_size: CGSize = msg_send![video_track, naturalSize];
        let duration: CMTime = msg_send![asset, duration];
        let nominal_frame_rate: f32 = msg_send![video_track, nominalFrameRate];

        let info = VideoInfo {
            width: natural_size.width as u32,
            height: natural_size.height as u32,
            duration_ms: duration.to_milliseconds(),
            frame_rate: if nominal_frame_rate > 0.0 {
                nominal_frame_rate
            } else {
                30.0
            },
            is_live: false,
        };

        // Create asset reader
        let mut error: *mut Object = ptr::null_mut();
        let asset_reader: *mut Object = msg_send![class!(AVAssetReader), alloc];
        let asset_reader: *mut Object = msg_send![asset_reader, initWithAsset:asset error:&mut error];

        if asset_reader.is_null() || !error.is_null() {
            let _: () = msg_send![asset, release];
            let _: () = msg_send![video_track, release];
            return Err(VideoError::LoadError("Failed to create AVAssetReader".into()));
        }

        // Create video output with BGRA format for easy conversion
        let pixel_format_key: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: b"PixelFormatType\0".as_ptr()];
        let pixel_format_value: *mut Object =
            msg_send![class!(NSNumber), numberWithInt: 0x42475241i32]; // kCVPixelFormatType_32BGRA

        let output_settings: *mut Object = msg_send![class!(NSDictionary),
            dictionaryWithObject:pixel_format_value
            forKey:pixel_format_key];

        let video_output: *mut Object = msg_send![class!(AVAssetReaderTrackOutput), alloc];
        let video_output: *mut Object = msg_send![video_output,
            initWithTrack:video_track
            outputSettings:output_settings];

        if video_output.is_null() {
            let _: () = msg_send![asset_reader, release];
            let _: () = msg_send![asset, release];
            let _: () = msg_send![video_track, release];
            return Err(VideoError::LoadError("Failed to create video output".into()));
        }

        // Add output to reader
        let can_add: BOOL = msg_send![asset_reader, canAddOutput: video_output];
        if can_add == NO {
            let _: () = msg_send![video_output, release];
            let _: () = msg_send![asset_reader, release];
            let _: () = msg_send![asset, release];
            let _: () = msg_send![video_track, release];
            return Err(VideoError::LoadError("Cannot add video output to reader".into()));
        }

        let _: () = msg_send![asset_reader, addOutput: video_output];

        // Start reading
        let started: BOOL = msg_send![asset_reader, startReading];
        if started == NO {
            // Get error from reader
            let reader_error: *mut Object = msg_send![asset_reader, error];
            let error_desc = if !reader_error.is_null() {
                let desc: *mut Object = msg_send![reader_error, localizedDescription];
                if !desc.is_null() {
                    let utf8: *const i8 = msg_send![desc, UTF8String];
                    if !utf8.is_null() {
                        std::ffi::CStr::from_ptr(utf8).to_string_lossy().to_string()
                    } else {
                        "Unknown error".to_string()
                    }
                } else {
                    "Unknown error".to_string()
                }
            } else {
                "No error object".to_string()
            };
            eprintln!("[MacOSVideoDecoder] Failed to start reading: {}", error_desc);
            let _: () = msg_send![video_output, release];
            let _: () = msg_send![asset_reader, release];
            let _: () = msg_send![asset, release];
            let _: () = msg_send![video_track, release];
            return Err(VideoError::LoadError(format!("Failed to start reading: {}", error_desc)));
        }

        Ok(Self {
            asset,
            asset_reader,
            video_output,
            video_track,
            info,
            current_time_ms: 0,
            ended: false,
        })
    }

    /// Convert BGRA pixel buffer to RGBA
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
                    rgba[dst_idx + 3] = bgra[src_idx + 3]; // A
                }
            }
        }

        rgba
    }
}

impl Drop for MacOSVideoDecoder {
    fn drop(&mut self) {
        unsafe {
            if !self.video_output.is_null() {
                let _: () = msg_send![self.video_output, release];
            }
            if !self.asset_reader.is_null() {
                let _: () = msg_send![self.asset_reader, cancelReading];
                let _: () = msg_send![self.asset_reader, release];
            }
            if !self.video_track.is_null() {
                let _: () = msg_send![self.video_track, release];
            }
            if !self.asset.is_null() {
                let _: () = msg_send![self.asset, release];
            }
        }
    }
}

impl VideoDecoder for MacOSVideoDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_frame(&mut self) -> Option<VideoFrame> {
        if self.ended {
            return None;
        }

        unsafe {
            // Check reader status first
            let status: i64 = msg_send![self.asset_reader, status];
            // AVAssetReaderStatus: 0=unknown, 1=reading, 2=completed, 3=failed, 4=cancelled
            if status != 1 {
                if status == 3 {
                    // Failed - get error
                    let error: *mut Object = msg_send![self.asset_reader, error];
                    if !error.is_null() {
                        let desc: *mut Object = msg_send![error, localizedDescription];
                        if !desc.is_null() {
                            let utf8: *const i8 = msg_send![desc, UTF8String];
                            if !utf8.is_null() {
                                let error_str = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
                                eprintln!("[VideoDecoder] Reader error: {}", error_str);
                            }
                        }
                    }
                }
                self.ended = true;
                return None;
            }

            // Get next sample buffer
            let sample_buffer: *mut Object = msg_send![self.video_output, copyNextSampleBuffer];

            if sample_buffer.is_null() {
                self.ended = true;
                return None;
            }

            // Get presentation time using C function
            let pts: CMTime = CMSampleBufferGetPresentationTimeStamp(sample_buffer as *const c_void);
            self.current_time_ms = pts.to_milliseconds();

            // Get image buffer (CVPixelBuffer) using C function
            let image_buffer: *const c_void = CMSampleBufferGetImageBuffer(sample_buffer as *const c_void);

            if image_buffer.is_null() {
                // Release sample buffer
                CFRelease(sample_buffer as *const c_void);
                return None;
            }

            // Lock the pixel buffer
            let lock_result = CVPixelBufferLockBaseAddress(image_buffer, 0);
            if lock_result != 0 {
                CFRelease(sample_buffer as *const c_void);
                return None;
            }

            // Get pixel data
            let width = CVPixelBufferGetWidth(image_buffer);
            let height = CVPixelBufferGetHeight(image_buffer);
            let bytes_per_row = CVPixelBufferGetBytesPerRow(image_buffer);
            let base_address = CVPixelBufferGetBaseAddress(image_buffer);

            let frame = if !base_address.is_null() {
                let data_len = bytes_per_row * height;
                let bgra_data = std::slice::from_raw_parts(base_address, data_len);
                let rgba_data = Self::bgra_to_rgba(bgra_data, width, height, bytes_per_row);

                Some(VideoFrame {
                    width: width as u32,
                    height: height as u32,
                    data: rgba_data,
                    timestamp_ms: self.current_time_ms,
                })
            } else {
                None
            };

            // Unlock and release
            CVPixelBufferUnlockBaseAddress(image_buffer, 0);
            CFRelease(sample_buffer as *const c_void);

            frame
        }
    }

    fn seek(&mut self, timestamp_ms: u64) -> Result<(), VideoError> {
        // AVAssetReader doesn't support seeking directly
        // We need to recreate the reader at the new position
        // For now, we only support seeking to the beginning
        if timestamp_ms == 0 {
            // Cancel current reader
            unsafe {
                let _: () = msg_send![self.asset_reader, cancelReading];
                let _: () = msg_send![self.video_output, release];
                let _: () = msg_send![self.asset_reader, release];
            }

            // Recreate reader
            unsafe {
                let mut error: *mut Object = ptr::null_mut();
                let asset_reader: *mut Object = msg_send![class!(AVAssetReader), alloc];
                let asset_reader: *mut Object =
                    msg_send![asset_reader, initWithAsset:self.asset error:&mut error];

                if asset_reader.is_null() {
                    return Err(VideoError::SeekError("Failed to recreate reader".into()));
                }

                // Recreate output
                let pixel_format_key: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: b"PixelFormatType\0".as_ptr()];
                let pixel_format_value: *mut Object =
                    msg_send![class!(NSNumber), numberWithInt: 0x42475241i32];

                let output_settings: *mut Object = msg_send![class!(NSDictionary),
                    dictionaryWithObject:pixel_format_value
                    forKey:pixel_format_key];

                let video_output: *mut Object = msg_send![class!(AVAssetReaderTrackOutput), alloc];
                let video_output: *mut Object = msg_send![video_output,
                    initWithTrack:self.video_track
                    outputSettings:output_settings];

                let _: () = msg_send![asset_reader, addOutput: video_output];
                let _: BOOL = msg_send![asset_reader, startReading];

                self.asset_reader = asset_reader;
                self.video_output = video_output;
                self.current_time_ms = 0;
                self.ended = false;
            }

            Ok(())
        } else {
            Err(VideoError::SeekError(
                "Seeking to arbitrary positions not yet supported".into(),
            ))
        }
    }

    fn has_more_frames(&self) -> bool {
        !self.ended
    }

    fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }
}

#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
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
        let rgba = MacOSVideoDecoder::bgra_to_rgba(&bgra, 2, 2, 8);

        assert_eq!(rgba[0], 0); // R (was B)
        assert_eq!(rgba[1], 0); // G
        assert_eq!(rgba[2], 255); // B (was R)
        assert_eq!(rgba[3], 255); // A
    }
}
