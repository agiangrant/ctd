# Video Widget Implementation Plan

## Overview

Add video playback support to the centered framework, enabling video widgets that can:
1. Load videos from URLs (async streaming)
2. Load videos from local files
3. Accept raw video frame data (for video meetings/WebRTC)
4. Support Tailwind-style classes (object-fit, object-position, rounded corners)

## Architecture Decision: Platform-Native Video Decoding

For macOS, use **AVFoundation** (via `av-foundation` crate or raw FFI) rather than FFmpeg because:
- Zero additional dependencies (ships with macOS)
- Hardware-accelerated decoding (VideoToolbox)
- Better codec support for common formats (H.264, HEVC, VP9)
- Cleaner integration with the existing Core* framework usage

For cross-platform support later, we can add:
- Linux: GStreamer via `gstreamer` crate
- Windows: Media Foundation via `windows` crate

## Implementation Phases

### Phase 1: Video Module Structure (Rust)

Create `engine/src/video.rs` and `engine/src/video/` with:

```rust
// engine/src/video.rs
pub mod decoder;
pub mod player;

/// Video frame ready for GPU upload
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,       // RGBA pixel data
    pub timestamp_ms: u64,   // Presentation timestamp
}

/// Video metadata
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub frame_rate: f32,
}

/// Video playback state
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    Ended,
    Error(String),
}
```

### Phase 2: macOS Video Decoder

Create `engine/src/video/decoder/macos.rs`:

```rust
// Uses AVFoundation for decoding
// AVAssetReader + AVAssetReaderTrackOutput for frame extraction
// CVPixelBuffer -> RGBA conversion for GPU upload

pub struct MacOSVideoDecoder {
    asset_reader: AVAssetReader,
    video_output: AVAssetReaderTrackOutput,
    audio_output: Option<AVAssetReaderTrackOutput>,
    info: VideoInfo,
}

impl MacOSVideoDecoder {
    pub fn from_url(url: &str) -> Result<Self, VideoError>;
    pub fn from_file(path: &str) -> Result<Self, VideoError>;
    pub fn next_frame(&mut self) -> Option<VideoFrame>;
    pub fn seek(&mut self, timestamp_ms: u64) -> Result<(), VideoError>;
    pub fn info(&self) -> &VideoInfo;
}
```

### Phase 3: Video Player (frame timing & texture streaming)

Create `engine/src/video/player.rs`:

```rust
pub struct VideoPlayer {
    decoder: Box<dyn VideoDecoder>,
    texture_id: Option<u32>,
    state: PlaybackState,
    current_time_ms: u64,
    looping: bool,
    muted: bool,
    volume: f32,
}

impl VideoPlayer {
    pub fn new() -> Self;
    pub fn load_url(&mut self, url: &str) -> Result<(), VideoError>;
    pub fn load_file(&mut self, path: &str) -> Result<(), VideoError>;

    // For video meetings - accept raw frame data
    pub fn push_frame(&mut self, frame: VideoFrame);

    pub fn play(&mut self);
    pub fn pause(&mut self);
    pub fn seek(&mut self, timestamp_ms: u64);
    pub fn set_looping(&mut self, looping: bool);
    pub fn set_volume(&mut self, volume: f32);

    // Called each frame to get current texture
    pub fn update(&mut self, delta_ms: u64) -> Option<u32>;
    pub fn texture_id(&self) -> Option<u32>;
    pub fn state(&self) -> PlaybackState;
    pub fn info(&self) -> Option<&VideoInfo>;
}
```

### Phase 4: GPU Texture Streaming

Extend `WgpuBackend` with video texture support:

```rust
impl WgpuBackend {
    // Create a texture that can be updated each frame
    pub fn create_video_texture(&mut self, width: u32, height: u32) -> u32;

    // Update texture with new frame data (fast path)
    pub fn update_video_texture(&mut self, texture_id: u32, frame: &VideoFrame) -> Result<(), Error>;

    // Existing DrawImage command works for video frames
}
```

### Phase 5: FFI Layer

Add to `engine/src/ffi.rs`:

```rust
// Video player management
pub extern "C" fn centered_video_create() -> u32;  // Returns player_id
pub extern "C" fn centered_video_destroy(player_id: u32);

// Loading
pub extern "C" fn centered_video_load_url(player_id: u32, url: *const c_char) -> i32;
pub extern "C" fn centered_video_load_file(player_id: u32, path: *const c_char) -> i32;

// For video meetings - push raw frame data
pub extern "C" fn centered_video_push_frame(
    player_id: u32,
    width: u32,
    height: u32,
    data: *const u8,
    data_len: usize,
    timestamp_ms: u64,
) -> i32;

// Playback control
pub extern "C" fn centered_video_play(player_id: u32) -> i32;
pub extern "C" fn centered_video_pause(player_id: u32) -> i32;
pub extern "C" fn centered_video_seek(player_id: u32, timestamp_ms: u64) -> i32;
pub extern "C" fn centered_video_set_looping(player_id: u32, looping: bool) -> i32;
pub extern "C" fn centered_video_set_volume(player_id: u32, volume: f32) -> i32;

// State queries
pub extern "C" fn centered_video_get_state(player_id: u32) -> i32;  // PlaybackState as int
pub extern "C" fn centered_video_get_texture_id(player_id: u32) -> u32;
pub extern "C" fn centered_video_get_info(
    player_id: u32,
    width: *mut u32,
    height: *mut u32,
    duration_ms: *mut u64,
) -> i32;

// Frame update (call each frame)
pub extern "C" fn centered_video_update(player_id: u32, delta_ms: u64) -> i32;
```

### Phase 6: Go Widget API

Add to `retained/widget.go`:

```go
// Widget fields for video
type Widget struct {
    // ... existing fields ...
    videoPlayerID   uint32
    videoSource     string
    videoAutoplay   bool
    videoLoop       bool
    videoMuted      bool
    videoVolume     float32
    videoState      VideoState
}

// VideoState enum
type VideoState int
const (
    VideoStateIdle VideoState = iota
    VideoStateLoading
    VideoStatePlaying
    VideoStatePaused
    VideoStateEnded
    VideoStateError
)
```

Add to `retained/builders.go`:

```go
// Video creates a video widget from a URL or file path
func Video(source string, classes string) *Widget {
    w := NewWidget(KindVideo)
    w.SetVideoSource(source)
    if classes != "" {
        w.SetClasses(classes)
    }
    return w
}

// VideoFromURL creates a video widget that streams from a URL
func VideoFromURL(url string, classes string) *Widget {
    return Video(url, classes)
}

// VideoFromFile creates a video widget from a local file
func VideoFromFile(path string, classes string) *Widget {
    return Video(path, classes)
}

// VideoStream creates a video widget for raw frame input (video meetings)
func VideoStream(classes string) *Widget {
    w := NewWidget(KindVideo)
    // No source - frames will be pushed via PushFrame()
    if classes != "" {
        w.SetClasses(classes)
    }
    return w
}
```

Widget methods:

```go
func (w *Widget) Play()
func (w *Widget) Pause()
func (w *Widget) Seek(timestampMs uint64)
func (w *Widget) SetLooping(loop bool) *Widget
func (w *Widget) SetMuted(muted bool) *Widget
func (w *Widget) SetVolume(volume float32) *Widget
func (w *Widget) SetAutoplay(autoplay bool) *Widget
func (w *Widget) OnEnded(fn func()) *Widget
func (w *Widget) OnError(fn func(error)) *Widget
func (w *Widget) OnTimeUpdate(fn func(currentMs, durationMs uint64)) *Widget

// For video meetings
func (w *Widget) PushFrame(width, height uint32, rgbaData []byte, timestampMs uint64)
```

### Phase 7: Rendering Integration

Update `retained/loop.go` to handle video widgets:

```go
case KindVideo:
    // Update video player
    if w.videoPlayerID != 0 {
        ffi.VideoUpdate(w.videoPlayerID, deltaMs)
        textureID := ffi.VideoGetTextureID(w.videoPlayerID)
        if textureID != 0 {
            // Use existing image rendering with object-fit support
            imgX, imgY, imgW, imgH := calculateObjectFit(...)
            commands = append(commands, ffi.Image(textureID, imgX, imgY, imgW, imgH))
        }
    }
```

## Dependencies to Add

In `engine/Cargo.toml`:

```toml
# macOS video decoding
[target.'cfg(target_os = "macos")'.dependencies]
# ... existing deps ...
block = "0.1"  # For Objective-C blocks (AVFoundation callbacks)
```

We'll use raw FFI to AVFoundation (similar to how we use Core Text) rather than adding a heavy crate.

## Test Video

Use the Big Buck Bunny test video:
- URL: https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4
- Format: H.264 MP4
- Size: 360p, 10 seconds, ~1MB

## Example Usage

```go
// examples/video/main.go
func buildUI() *retained.Widget {
    return retained.VStack("flex-1 bg-gray-900 p-8",
        retained.Text("Video Widget Example", "text-4xl font-bold text-white mb-8"),

        // Video from URL with controls
        retained.VStack("gap-4",
            retained.VideoFromURL(
                "https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4",
                "w-full max-w-2xl rounded-xl object-contain",
            ).SetAutoplay(true).SetLooping(true),

            retained.HStack("gap-2",
                retained.Button("Play", "px-4 py-2 bg-green-600 rounded"),
                retained.Button("Pause", "px-4 py-2 bg-yellow-600 rounded"),
            ),
        ),

        // Video stream for meetings
        retained.HStack("gap-4 mt-8",
            retained.VideoStream("w-48 h-36 rounded-lg bg-gray-800"),
            retained.VideoStream("w-48 h-36 rounded-lg bg-gray-800"),
        ),
    )
}
```

## Timeline Estimate

1. Phase 1-2 (Video module + macOS decoder): Core video decoding
2. Phase 3-4 (Player + GPU streaming): Frame timing and texture updates
3. Phase 5-6 (FFI + Go API): Widget integration
4. Phase 7 (Rendering): Visual integration with object-fit support

## Future Enhancements

- Audio playback (requires audio backend integration)
- Subtitle support
- Playback speed control
- Picture-in-picture support
- HLS/DASH streaming support
- Linux (GStreamer) and Windows (Media Foundation) backends
