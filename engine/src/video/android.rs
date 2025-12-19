//! Android video decoder using MediaExtractor and MediaCodec via JNI
//!
//! Uses Android's MediaExtractor for demuxing and MediaCodec for decoding.
//! This provides hardware-accelerated decoding for H.264, HEVC, VP9, etc.

#![cfg(target_os = "android")]

use super::{VideoDecoder, VideoError, VideoFrame, VideoInfo};
use jni::objects::{GlobalRef, JObject, JValue};
use log::{error, info};
use std::time::Duration;

/// Get the JavaVM from the Android platform module
fn get_java_vm() -> Option<&'static jni::JavaVM> {
    unsafe { crate::platform::android::JAVA_VM.as_ref() }
}

/// Android video decoder using MediaExtractor and MediaCodec
pub struct AndroidVideoDecoder {
    /// MediaExtractor object (GlobalRef)
    extractor: Option<GlobalRef>,
    /// MediaCodec object (GlobalRef)
    codec: Option<GlobalRef>,
    /// Video track index
    track_index: i32,
    /// Video metadata
    info: VideoInfo,
    /// Current playback time in milliseconds
    current_time_ms: u64,
    /// Whether we've reached the end
    ended: bool,
    /// Whether the codec is configured
    configured: bool,
    /// Whether input has ended
    input_ended: bool,
}

// Safety: The JNI GlobalRefs are thread-safe
unsafe impl Send for AndroidVideoDecoder {}

impl AndroidVideoDecoder {
    /// Create decoder from a URL (file:// or http://)
    pub fn from_url(url: &str) -> Result<Self, VideoError> {
        Self::create_decoder(url, true)
    }

    /// Create decoder from a file path
    pub fn from_file(path: &str) -> Result<Self, VideoError> {
        Self::create_decoder(path, false)
    }

    fn create_decoder(source: &str, is_url: bool) -> Result<Self, VideoError> {
        let vm = get_java_vm().ok_or(VideoError::UnsupportedPlatform)?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|_| VideoError::UnsupportedPlatform)?;

        // Create MediaExtractor
        let extractor_class = env
            .find_class("android/media/MediaExtractor")
            .map_err(|e| VideoError::LoadError(format!("Failed to find MediaExtractor: {:?}", e)))?;

        let extractor: JObject = env
            .new_object(extractor_class, "()V", &[])
            .map_err(|e| {
                VideoError::LoadError(format!("Failed to create MediaExtractor: {:?}", e))
            })?;

        // Set data source
        if is_url {
            let url_str = env
                .new_string(source)
                .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;

            let result = env.call_method(
                &extractor,
                "setDataSource",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&url_str)],
            );

            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
                return Err(VideoError::LoadError(format!(
                    "Failed to set URL data source: {}",
                    source
                )));
            }

            result.map_err(|e| {
                VideoError::LoadError(format!("Failed to set URL data source: {:?}", e))
            })?;
        } else {
            // For local files, use FileDescriptor
            // First open the file using FileInputStream
            let path_str = env
                .new_string(source)
                .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;

            let fis_class = env
                .find_class("java/io/FileInputStream")
                .map_err(|e| {
                    VideoError::LoadError(format!("Failed to find FileInputStream: {:?}", e))
                })?;

            let fis: JObject = env
                .new_object(
                    fis_class,
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&path_str)],
                )
                .map_err(|e| {
                    VideoError::LoadError(format!("Failed to create FileInputStream: {:?}", e))
                })?;

            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
                return Err(VideoError::LoadError(format!("File not found: {}", source)));
            }

            // Get FileDescriptor
            let fd: JObject = env
                .call_method(&fis, "getFD", "()Ljava/io/FileDescriptor;", &[])
                .map_err(|e| {
                    VideoError::LoadError(format!("Failed to get FileDescriptor: {:?}", e))
                })?
                .l()
                .map_err(|e| {
                    VideoError::LoadError(format!("Failed to get FD object: {:?}", e))
                })?;

            // Set data source with FileDescriptor
            let result = env.call_method(
                &extractor,
                "setDataSource",
                "(Ljava/io/FileDescriptor;)V",
                &[JValue::Object(&fd)],
            );

            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
                // Close the FileInputStream
                let _ = env.call_method(&fis, "close", "()V", &[]);
                return Err(VideoError::LoadError(format!(
                    "Failed to set file data source: {}",
                    source
                )));
            }

            result.map_err(|e| {
                VideoError::LoadError(format!("Failed to set file data source: {:?}", e))
            })?;

            // Close FileInputStream (we don't need it anymore)
            let _ = env.call_method(&fis, "close", "()V", &[]);
            let _ = env.exception_clear();
        }

        // Find video track
        let track_count: i32 = env
            .call_method(&extractor, "getTrackCount", "()I", &[])
            .map_err(|_| VideoError::LoadError("Failed to get track count".to_string()))?
            .i()
            .unwrap_or(0);

        let _ = env.exception_clear();

        let mut video_track_index = -1i32;
        let mut video_format: Option<JObject> = None;

        for i in 0..track_count {
            let format: JObject = env
                .call_method(
                    &extractor,
                    "getTrackFormat",
                    "(I)Landroid/media/MediaFormat;",
                    &[JValue::Int(i)],
                )
                .map_err(|_| VideoError::LoadError("Failed to get track format".to_string()))?
                .l()
                .map_err(|_| VideoError::LoadError("Failed to get format object".to_string()))?;

            let _ = env.exception_clear();

            // Get MIME type
            let mime_key = env
                .new_string("mime")
                .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;

            let mime: JObject = env
                .call_method(
                    &format,
                    "getString",
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    &[JValue::Object(&mime_key)],
                )
                .ok()
                .and_then(|v| v.l().ok())
                .unwrap_or(JObject::null());

            let _ = env.exception_clear();

            if !mime.is_null() {
                let mime_str: String = env
                    .get_string((&mime).into())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                if mime_str.starts_with("video/") {
                    video_track_index = i;
                    video_format = Some(format);
                    info!("Found video track {} with MIME: {}", i, mime_str);
                    break;
                }
            }
        }

        if video_track_index < 0 || video_format.is_none() {
            return Err(VideoError::FormatError("No video track found".to_string()));
        }

        let format = video_format.unwrap();

        // Select the video track
        let result = env.call_method(
            &extractor,
            "selectTrack",
            "(I)V",
            &[JValue::Int(video_track_index)],
        );
        let _ = env.exception_clear();
        result.map_err(|e| VideoError::LoadError(format!("Failed to select track: {:?}", e)))?;

        // Get video dimensions
        let width_key = env
            .new_string("width")
            .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;
        let height_key = env
            .new_string("height")
            .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;
        let duration_key = env
            .new_string("durationUs")
            .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;
        let frame_rate_key = env
            .new_string("frame-rate")
            .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;

        let width: i32 = env
            .call_method(
                &format,
                "getInteger",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&width_key)],
            )
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(1280);
        let _ = env.exception_clear();

        let height: i32 = env
            .call_method(
                &format,
                "getInteger",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&height_key)],
            )
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(720);
        let _ = env.exception_clear();

        let duration_us: i64 = env
            .call_method(
                &format,
                "getLong",
                "(Ljava/lang/String;)J",
                &[JValue::Object(&duration_key)],
            )
            .ok()
            .and_then(|v| v.j().ok())
            .unwrap_or(0);
        let _ = env.exception_clear();

        let frame_rate: f32 = env
            .call_method(
                &format,
                "getInteger",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&frame_rate_key)],
            )
            .ok()
            .and_then(|v| v.i().ok())
            .map(|v| v as f32)
            .unwrap_or(30.0);
        let _ = env.exception_clear();

        info!(
            "Video: {}x{}, duration: {}us, frame_rate: {}",
            width, height, duration_us, frame_rate
        );

        // Get MIME type for codec creation
        let mime_key = env
            .new_string("mime")
            .map_err(|e| VideoError::LoadError(format!("Failed to create string: {:?}", e)))?;
        let mime: JObject = env
            .call_method(
                &format,
                "getString",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&mime_key)],
            )
            .ok()
            .and_then(|v| v.l().ok())
            .unwrap_or(JObject::null());
        let _ = env.exception_clear();

        if mime.is_null() {
            return Err(VideoError::FormatError("No MIME type in format".to_string()));
        }

        // Create MediaCodec decoder
        let codec_class = env.find_class("android/media/MediaCodec").map_err(|e| {
            VideoError::LoadError(format!("Failed to find MediaCodec class: {:?}", e))
        })?;

        let codec: JObject = env
            .call_static_method(
                codec_class,
                "createDecoderByType",
                "(Ljava/lang/String;)Landroid/media/MediaCodec;",
                &[JValue::Object(&mime)],
            )
            .map_err(|e| VideoError::LoadError(format!("Failed to create decoder: {:?}", e)))?
            .l()
            .map_err(|e| VideoError::LoadError(format!("Failed to get codec object: {:?}", e)))?;

        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return Err(VideoError::LoadError("Failed to create video decoder".to_string()));
        }

        // Configure codec (null surface = SW decoding to ByteBuffer)
        let result = env.call_method(
            &codec,
            "configure",
            "(Landroid/media/MediaFormat;Landroid/view/Surface;Landroid/media/MediaCrypto;I)V",
            &[
                JValue::Object(&format),
                JValue::Object(&JObject::null()),
                JValue::Object(&JObject::null()),
                JValue::Int(0), // flags = 0 for decoder
            ],
        );

        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            return Err(VideoError::LoadError("Failed to configure codec".to_string()));
        }

        result.map_err(|e| VideoError::LoadError(format!("Failed to configure codec: {:?}", e)))?;

        // Start codec
        let result = env.call_method(&codec, "start", "()V", &[]);
        let _ = env.exception_clear();
        result.map_err(|e| VideoError::LoadError(format!("Failed to start codec: {:?}", e)))?;

        // Create global refs
        let extractor_ref = env
            .new_global_ref(&extractor)
            .map_err(|e| VideoError::LoadError(format!("Failed to create global ref: {:?}", e)))?;

        let codec_ref = env
            .new_global_ref(&codec)
            .map_err(|e| VideoError::LoadError(format!("Failed to create global ref: {:?}", e)))?;

        info!("Android video decoder created successfully");

        Ok(Self {
            extractor: Some(extractor_ref),
            codec: Some(codec_ref),
            track_index: video_track_index,
            info: VideoInfo {
                width: width as u32,
                height: height as u32,
                duration_ms: (duration_us / 1000) as u64,
                frame_rate,
                is_live: false,
            },
            current_time_ms: 0,
            ended: false,
            configured: true,
            input_ended: false,
        })
    }

    /// Feed input data to the codec
    fn feed_input(&mut self) -> Result<bool, VideoError> {
        if self.input_ended {
            return Ok(false);
        }

        let vm = get_java_vm().ok_or(VideoError::UnsupportedPlatform)?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|_| VideoError::UnsupportedPlatform)?;

        let extractor = self
            .extractor
            .as_ref()
            .ok_or(VideoError::DecodeError("No extractor".to_string()))?;
        let codec = self
            .codec
            .as_ref()
            .ok_or(VideoError::DecodeError("No codec".to_string()))?;

        // Try to get an input buffer with timeout
        let input_index: i32 = env
            .call_method(
                codec.as_obj(),
                "dequeueInputBuffer",
                "(J)I",
                &[JValue::Long(0)], // 0 = non-blocking
            )
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(-1);

        let _ = env.exception_clear();

        if input_index < 0 {
            return Ok(false); // No buffer available
        }

        // Get input buffer
        let input_buffers: JObject = env
            .call_method(
                codec.as_obj(),
                "getInputBuffer",
                "(I)Ljava/nio/ByteBuffer;",
                &[JValue::Int(input_index)],
            )
            .ok()
            .and_then(|v| v.l().ok())
            .unwrap_or(JObject::null());

        let _ = env.exception_clear();

        if input_buffers.is_null() {
            return Ok(false);
        }

        // Read sample data from extractor
        let sample_size: i32 = env
            .call_method(
                extractor.as_obj(),
                "readSampleData",
                "(Ljava/nio/ByteBuffer;I)I",
                &[JValue::Object(&input_buffers), JValue::Int(0)],
            )
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(-1);

        let _ = env.exception_clear();

        if sample_size < 0 {
            // End of stream
            let _ = env.call_method(
                codec.as_obj(),
                "queueInputBuffer",
                "(IIIJI)V",
                &[
                    JValue::Int(input_index),
                    JValue::Int(0),
                    JValue::Int(0),
                    JValue::Long(0),
                    JValue::Int(4), // BUFFER_FLAG_END_OF_STREAM
                ],
            );
            let _ = env.exception_clear();
            self.input_ended = true;
            return Ok(false);
        }

        // Get presentation time
        let pts: i64 = env
            .call_method(extractor.as_obj(), "getSampleTime", "()J", &[])
            .ok()
            .and_then(|v| v.j().ok())
            .unwrap_or(0);

        let _ = env.exception_clear();

        // Queue the input buffer
        let _ = env.call_method(
            codec.as_obj(),
            "queueInputBuffer",
            "(IIIJI)V",
            &[
                JValue::Int(input_index),
                JValue::Int(0),
                JValue::Int(sample_size),
                JValue::Long(pts),
                JValue::Int(0), // flags
            ],
        );
        let _ = env.exception_clear();

        // Advance extractor to next sample
        let _ = env.call_method(extractor.as_obj(), "advance", "()Z", &[]);
        let _ = env.exception_clear();

        Ok(true)
    }

    /// Get decoded output frame
    fn get_output(&mut self) -> Option<VideoFrame> {
        let vm = get_java_vm()?;
        let mut env = vm.attach_current_thread().ok()?;

        let codec = self.codec.as_ref()?;

        // Create BufferInfo object
        let buffer_info_class = env.find_class("android/media/MediaCodec$BufferInfo").ok()?;
        let buffer_info: JObject = env.new_object(buffer_info_class, "()V", &[]).ok()?;

        // Try to dequeue output buffer
        let output_index: i32 = env
            .call_method(
                codec.as_obj(),
                "dequeueOutputBuffer",
                "(Landroid/media/MediaCodec$BufferInfo;J)I",
                &[JValue::Object(&buffer_info), JValue::Long(10000)], // 10ms timeout
            )
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(-1);

        let _ = env.exception_clear();

        if output_index < 0 {
            // Handle special return values
            if output_index == -1 {
                // INFO_TRY_AGAIN_LATER
                return None;
            }
            // Other negative values are info changes, not errors
            return None;
        }

        // Get output buffer
        let output_buffer: JObject = env
            .call_method(
                codec.as_obj(),
                "getOutputBuffer",
                "(I)Ljava/nio/ByteBuffer;",
                &[JValue::Int(output_index)],
            )
            .ok()
            .and_then(|v| v.l().ok())
            .unwrap_or(JObject::null());

        let _ = env.exception_clear();

        let frame = if !output_buffer.is_null() {
            // Get buffer info fields
            let size: i32 = env
                .get_field(&buffer_info, "size", "I")
                .ok()
                .and_then(|v| v.i().ok())
                .unwrap_or(0);

            let offset: i32 = env
                .get_field(&buffer_info, "offset", "I")
                .ok()
                .and_then(|v| v.i().ok())
                .unwrap_or(0);

            let pts: i64 = env
                .get_field(&buffer_info, "presentationTimeUs", "J")
                .ok()
                .and_then(|v| v.j().ok())
                .unwrap_or(0);

            let flags: i32 = env
                .get_field(&buffer_info, "flags", "I")
                .ok()
                .and_then(|v| v.i().ok())
                .unwrap_or(0);

            // Check for end of stream
            if flags & 4 != 0 {
                // BUFFER_FLAG_END_OF_STREAM
                self.ended = true;
            }

            self.current_time_ms = (pts / 1000) as u64;

            // Get the raw pixel data
            // Note: MediaCodec outputs in various formats (usually YUV), we need to convert to RGBA
            if size > 0 {
                // Position the buffer at the offset
                let _ = env.call_method(
                    &output_buffer,
                    "position",
                    "(I)Ljava/nio/Buffer;",
                    &[JValue::Int(offset)],
                );
                let _ = env.exception_clear();

                // Create a byte array to hold the data
                let byte_array = env.new_byte_array(size).ok();

                let frame = if let Some(arr) = byte_array {
                    // Copy from ByteBuffer to byte array using get(byte[])
                    let _ = env.call_method(
                        &output_buffer,
                        "get",
                        "([B)Ljava/nio/ByteBuffer;",
                        &[JValue::Object(&arr)],
                    );
                    let _ = env.exception_clear();

                    // Get the byte array data
                    let mut data_vec = vec![0i8; size as usize];
                    let _ = env.get_byte_array_region(&arr, 0, &mut data_vec);
                    let _ = env.exception_clear();

                    // Convert to unsigned bytes
                    let data_slice: Vec<u8> = data_vec.iter().map(|&b| b as u8).collect();

                    // Convert YUV to RGBA
                    let rgba_data = if data_slice.len() == (self.info.width * self.info.height * 4) as usize {
                        // Already RGBA
                        data_slice
                    } else {
                        // Likely YUV, convert to RGBA
                        self.yuv_to_rgba(&data_slice)
                    };

                    Some(VideoFrame {
                        width: self.info.width,
                        height: self.info.height,
                        data: rgba_data,
                        timestamp_ms: self.current_time_ms,
                    })
                } else {
                    None
                };

                frame
            } else {
                None
            }
        } else {
            None
        };

        // Release the output buffer
        let _ = env.call_method(
            codec.as_obj(),
            "releaseOutputBuffer",
            "(IZ)V",
            &[JValue::Int(output_index), JValue::Bool(0)], // don't render
        );
        let _ = env.exception_clear();

        frame
    }

    /// Convert YUV420 to RGBA (simplified conversion)
    fn yuv_to_rgba(&self, yuv_data: &[u8]) -> Vec<u8> {
        let width = self.info.width as usize;
        let height = self.info.height as usize;
        let mut rgba = vec![255u8; width * height * 4];

        let y_size = width * height;
        let uv_size = y_size / 4;

        // Handle various YUV formats
        if yuv_data.len() >= y_size + uv_size * 2 {
            // YUV420 planar (I420)
            let y_plane = &yuv_data[0..y_size];
            let u_plane = &yuv_data[y_size..y_size + uv_size];
            let v_plane = &yuv_data[y_size + uv_size..];

            for row in 0..height {
                for col in 0..width {
                    let y_idx = row * width + col;
                    let uv_idx = (row / 2) * (width / 2) + (col / 2);

                    let y = y_plane.get(y_idx).copied().unwrap_or(0) as i32;
                    let u = u_plane.get(uv_idx).copied().unwrap_or(128) as i32 - 128;
                    let v = v_plane.get(uv_idx).copied().unwrap_or(128) as i32 - 128;

                    // YUV to RGB conversion
                    let r = (y + ((351 * v) >> 8)).clamp(0, 255) as u8;
                    let g = (y - ((179 * v + 86 * u) >> 8)).clamp(0, 255) as u8;
                    let b = (y + ((443 * u) >> 8)).clamp(0, 255) as u8;

                    let rgba_idx = (row * width + col) * 4;
                    rgba[rgba_idx] = r;
                    rgba[rgba_idx + 1] = g;
                    rgba[rgba_idx + 2] = b;
                    rgba[rgba_idx + 3] = 255;
                }
            }
        } else if yuv_data.len() >= y_size {
            // Grayscale only
            for i in 0..y_size.min(yuv_data.len()) {
                let rgba_idx = i * 4;
                let y = yuv_data[i];
                rgba[rgba_idx] = y;
                rgba[rgba_idx + 1] = y;
                rgba[rgba_idx + 2] = y;
                rgba[rgba_idx + 3] = 255;
            }
        }

        rgba
    }
}

impl Drop for AndroidVideoDecoder {
    fn drop(&mut self) {
        if let Some(vm) = get_java_vm() {
            if let Ok(mut env) = vm.attach_current_thread() {
                // Stop and release codec
                if let Some(ref codec) = self.codec {
                    let _ = env.call_method(codec.as_obj(), "stop", "()V", &[]);
                    let _ = env.exception_clear();
                    let _ = env.call_method(codec.as_obj(), "release", "()V", &[]);
                    let _ = env.exception_clear();
                }

                // Release extractor
                if let Some(ref extractor) = self.extractor {
                    let _ = env.call_method(extractor.as_obj(), "release", "()V", &[]);
                    let _ = env.exception_clear();
                }
            }
        }
    }
}

impl VideoDecoder for AndroidVideoDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_frame(&mut self) -> Option<VideoFrame> {
        if self.ended {
            return None;
        }

        // Feed input to keep the pipeline full
        for _ in 0..5 {
            if self.feed_input().unwrap_or(false) {
                break;
            }
        }

        // Try to get output
        for _ in 0..10 {
            if let Some(frame) = self.get_output() {
                return Some(frame);
            }
            // Feed more input
            let _ = self.feed_input();
            std::thread::sleep(Duration::from_millis(1));
        }

        None
    }

    fn seek(&mut self, timestamp_ms: u64) -> Result<(), VideoError> {
        let vm = get_java_vm().ok_or(VideoError::UnsupportedPlatform)?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|_| VideoError::UnsupportedPlatform)?;

        let extractor = self
            .extractor
            .as_ref()
            .ok_or(VideoError::DecodeError("No extractor".to_string()))?;
        let codec = self
            .codec
            .as_ref()
            .ok_or(VideoError::DecodeError("No codec".to_string()))?;

        // Flush the codec
        let _ = env.call_method(codec.as_obj(), "flush", "()V", &[]);
        let _ = env.exception_clear();

        // Seek the extractor
        let timestamp_us = (timestamp_ms * 1000) as i64;
        let _ = env.call_method(
            extractor.as_obj(),
            "seekTo",
            "(JI)V",
            &[
                JValue::Long(timestamp_us),
                JValue::Int(0), // SEEK_TO_PREVIOUS_SYNC
            ],
        );
        let _ = env.exception_clear();

        self.current_time_ms = timestamp_ms;
        self.ended = false;
        self.input_ended = false;

        Ok(())
    }

    fn has_more_frames(&self) -> bool {
        !self.ended
    }

    fn current_time_ms(&self) -> u64 {
        self.current_time_ms
    }
}
