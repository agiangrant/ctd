//! Linux video decoder backend using GStreamer
//!
//! Uses GStreamer for hardware-accelerated video decoding on Linux.
//! Supports H.264, HEVC, VP8, VP9, and other formats.

use super::{VideoDecoder, VideoError, VideoFrame, VideoInfo};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::sync::{Arc, Mutex};

/// Linux video decoder using GStreamer
pub struct LinuxVideoDecoder {
    /// GStreamer pipeline
    pipeline: Option<gst::Pipeline>,
    /// Video info
    info: VideoInfo,
    /// Whether decoder is initialized
    initialized: bool,
    /// Latest frame from callback
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
    /// Whether playing
    is_playing: bool,
}

impl LinuxVideoDecoder {
    /// Create a new decoder from a file path
    pub fn from_file(path: &str) -> Result<Self, VideoError> {
        let absolute_path = if path.starts_with('/') {
            path.to_string()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path).to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string())
        };

        if !std::path::Path::new(&absolute_path).exists() {
            return Err(VideoError::LoadError(format!("File not found: {}", absolute_path)));
        }

        Self::create_decoder(&format!("file://{}", absolute_path))
    }

    /// Create a new decoder from a URL
    pub fn from_url(url: &str) -> Result<Self, VideoError> {
        if url.starts_with("http://") || url.starts_with("https://") {
            Self::create_decoder(url)
        } else if url.starts_with("file://") {
            Self::create_decoder(url)
        } else {
            Self::from_file(url)
        }
    }

    fn create_decoder(uri: &str) -> Result<Self, VideoError> {
        gst::init().map_err(|e| VideoError::DecodeError(format!("Failed to init GStreamer: {}", e)))?;

        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri)
            .build()
            .map_err(|e| VideoError::DecodeError(format!("Failed to create playbin: {}", e)))?;

        // Frame storage
        let latest_frame = Arc::new(Mutex::new(None::<VideoFrame>));
        let frame_clone = latest_frame.clone();

        // Create appsink with sync enabled for proper timing
        let appsink = gst_app::AppSink::builder()
            .caps(
                &gst_video::VideoCapsBuilder::new()
                    .format(gst_video::VideoFormat::Rgba)
                    .build(),
            )
            .max_buffers(2)
            .drop(true)      // Drop if we fall behind
            .sync(true)      // Sync to clock for proper playback speed
            .build();

        // Set up callback - this runs on GStreamer's thread at video frame rate
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    if let Ok(sample) = sink.pull_sample() {
                        if let (Some(buffer), Some(caps)) = (sample.buffer(), sample.caps()) {
                            if let Ok(video_info) = gst_video::VideoInfo::from_caps(caps) {
                                if let Ok(map) = buffer.map_readable() {
                                    let frame = VideoFrame {
                                        width: video_info.width(),
                                        height: video_info.height(),
                                        data: map.as_slice().to_vec(),
                                        timestamp_ms: buffer.pts().map(|t| t.mseconds()).unwrap_or(0),
                                    };
                                    // Store frame - don't block, just try
                                    if let Ok(mut guard) = frame_clone.try_lock() {
                                        *guard = Some(frame);
                                    }
                                }
                            }
                        }
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        playbin.set_property("video-sink", &appsink);

        // Disable audio
        let fakesink = gst::ElementFactory::make("fakesink")
            .property("sync", false)
            .build()
            .map_err(|e| VideoError::DecodeError(format!("Failed to create fakesink: {}", e)))?;
        playbin.set_property("audio-sink", &fakesink);

        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .map_err(|_| VideoError::DecodeError("Failed to cast to pipeline".to_string()))?;

        // Preroll in paused state
        pipeline.set_state(gst::State::Paused)
            .map_err(|e| VideoError::DecodeError(format!("Failed to set paused: {}", e)))?;

        let (result, state, _) = pipeline.state(gst::ClockTime::from_seconds(5));
        if result.is_err() || state != gst::State::Paused {
            if let Some(bus) = pipeline.bus() {
                while let Some(msg) = bus.pop() {
                    if let gst::MessageView::Error(err) = msg.view() {
                        let _ = pipeline.set_state(gst::State::Null);
                        return Err(VideoError::DecodeError(format!("Pipeline error: {}", err.error())));
                    }
                }
            }
            let _ = pipeline.set_state(gst::State::Null);
            return Err(VideoError::DecodeError("Pipeline failed to preroll".to_string()));
        }

        // Get video info
        let mut info = VideoInfo::default();
        if let Some(duration) = pipeline.query_duration::<gst::ClockTime>() {
            info.duration_ms = duration.mseconds();
        }

        // Wait a bit for first frame
        std::thread::sleep(std::time::Duration::from_millis(50));

        if let Ok(guard) = latest_frame.lock() {
            if let Some(frame) = guard.as_ref() {
                info.width = frame.width;
                info.height = frame.height;
            }
        }

        Ok(Self {
            pipeline: Some(pipeline),
            info,
            initialized: true,
            latest_frame,
            is_playing: false,
        })
    }

    fn ensure_playing(&mut self) {
        if !self.is_playing {
            if let Some(pipeline) = &self.pipeline {
                let _ = pipeline.set_state(gst::State::Playing);
                self.is_playing = true;
            }
        }
    }
}

impl Default for LinuxVideoDecoder {
    fn default() -> Self {
        Self {
            pipeline: None,
            info: VideoInfo::default(),
            initialized: false,
            latest_frame: Arc::new(Mutex::new(None)),
            is_playing: false,
        }
    }
}

impl Drop for LinuxVideoDecoder {
    fn drop(&mut self) {
        if let Some(pipeline) = &self.pipeline {
            let _ = pipeline.set_state(gst::State::Null);
        }
    }
}

impl VideoDecoder for LinuxVideoDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_frame(&mut self) -> Option<VideoFrame> {
        if !self.initialized {
            return None;
        }

        // Start playing on first frame request
        self.ensure_playing();

        // Take the latest frame (non-blocking)
        if let Ok(mut guard) = self.latest_frame.try_lock() {
            guard.take()
        } else {
            None
        }
    }

    fn seek(&mut self, timestamp_ms: u64) -> Result<(), VideoError> {
        let pipeline = self.pipeline.as_ref().ok_or(VideoError::NotLoaded)?;

        pipeline.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            gst::ClockTime::from_mseconds(timestamp_ms),
        ).map_err(|e| VideoError::SeekError(format!("Seek failed: {}", e)))?;

        Ok(())
    }

    fn has_more_frames(&self) -> bool {
        if !self.initialized {
            return false;
        }

        if let Some(pipeline) = &self.pipeline {
            // Check pipeline state - if it's null, we're done
            let (_, state, _) = pipeline.state(gst::ClockTime::ZERO);
            if state == gst::State::Null {
                return false;
            }

            // Check for EOS by peeking at bus (non-destructive)
            if let Some(bus) = pipeline.bus() {
                // Pop and check messages - we need to handle them anyway
                while let Some(msg) = bus.pop() {
                    match msg.view() {
                        gst::MessageView::Eos(_) => {
                            return false;
                        }
                        gst::MessageView::Error(err) => {
                            eprintln!("GStreamer error: {}", err.error());
                            return false;
                        }
                        _ => {}
                    }
                }
            }

            return true;
        }

        false
    }

    fn current_time_ms(&self) -> u64 {
        if let Some(pipeline) = &self.pipeline {
            if let Some(pos) = pipeline.query_position::<gst::ClockTime>() {
                return pos.mseconds();
            }
        }
        0
    }
}

unsafe impl Send for LinuxVideoDecoder {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_default() {
        let decoder = LinuxVideoDecoder::default();
        assert!(!decoder.initialized);
        assert!(!decoder.has_more_frames());
    }
}
