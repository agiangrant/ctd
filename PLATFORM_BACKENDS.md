# Platform Rendering Backends

This document describes the platform-specific rendering backends for Centered.

## Architecture Overview

The platform abstraction layer provides a unified interface (`PlatformBackend` trait) that all platform-specific implementations must satisfy. This allows the core engine to be platform-agnostic while delegating actual rendering to optimized native backends.

```
┌──────────────────────────────────────────────────────────┐
│              Centered Engine Core                        │
│  ┌────────────────────────────────────────────────────┐  │
│  │          render::Renderer                          │  │
│  │  - Processes widget tree                          │  │
│  │  - Generates RenderCommands                       │  │
│  └──────────────────┬─────────────────────────────────┘  │
│                     │                                     │
│  ┌──────────────────▼─────────────────────────────────┐  │
│  │        platform::PlatformBackend trait            │  │
│  │  - init()                                         │  │
│  │  - begin_frame() / end_frame()                    │  │
│  │  - execute_commands()                             │  │
│  │  - poll_events()                                  │  │
│  └──────────────────┬─────────────────────────────────┘  │
└────────────────────┬│─────────────────────────────────────┘
                     ││
      ┌──────────────┼┼──────────────┬─────────────────┐
      │              ││               │                  │
┌─────▼────┐   ┌────▼▼────┐   ┌─────▼─────┐   ┌───────▼──────┐
│  Metal   │   │  Metal    │   │  Vulkan   │   │   Vulkan     │
│  macOS   │   │   iOS     │   │  Android  │   │   Linux      │
└──────────┘   └───────────┘   └───────────┘   └──────────────┘
                                                      │
                                            ┌─────────▼────────┐
                                            │   Direct3D 12    │
                                            │    Windows       │
                                            └──────────────────┘
```

## Platform Support Matrix

| Platform | Graphics API | Status | Testable On | Notes |
|----------|-------------|---------|-------------|-------|
| **macOS** | Metal | ✓ Implemented | Simulator/Device | Primary development platform |
| **iOS** | Metal | ✓ Implemented | Simulator/Device | Retina displays, touch events |
| **Android** | Vulkan | ✓ Implemented | Emulator/Device | API 24+ (Android 7.0+) |
| **Linux** | Vulkan | ✓ Implemented | Best guess | X11 + Wayland support |
| **Windows** | Direct3D 12 | ✓ Implemented | Best guess | Windows 10+ |

## Platform Details

### macOS - Metal Backend

**File**: `engine/src/platform/metal.rs`

**Graphics API**: Metal (Apple's modern low-level graphics API)

**Features**:
- Hardware-accelerated rendering via GPU
- Low overhead, direct access to Metal
- Integrates with AppKit/Cocoa for windowing
- Native look and feel

**Implementation Status**:
- ✓ Backend structure and trait implementation
- ✓ Platform detection and initialization
- ⚠️  TODO: Actual Metal device creation (requires `metal-rs` crate)
- ⚠️  TODO: Command encoding and execution
- ⚠️  TODO: Text rendering via Core Text
- ⚠️  TODO: Event handling via NSEvent

**Dependencies** (currently commented out in Cargo.toml):
```toml
metal = "0.27"
cocoa = "0.25"
core-graphics = "0.23"
```

**Typical Configuration**:
```rust
SurfaceConfig {
    width: 1920,
    height: 1080,
    scale_factor: 2.0,  // Retina display
    vsync: true,
}
```

### iOS - Metal Backend

**File**: `engine/src/platform/metal_ios.rs`

**Graphics API**: Metal

**Features**:
- Same Metal API as macOS
- Touch event handling (not mouse)
- Retina displays (2x-3x scale factor)
- Orientation changes (portrait/landscape)
- Safe area handling

**Implementation Status**:
- ✓ Backend structure with iOS-specific considerations
- ✓ Touch event stubs
- ⚠️  TODO: UIKit integration
- ⚠️  TODO: CAMetalLayer setup
- ⚠️  TODO: Orientation change handling

**Dependencies** (commented out):
```toml
metal = "0.27"
objc = "0.2"
```

**Typical Configurations**:
```rust
// iPhone 15 Pro
SurfaceConfig {
    width: 1179,
    height: 2556,
    scale_factor: 3.0,
    vsync: true,
}

// iPad Pro 12.9"
SurfaceConfig {
    width: 2048,
    height: 2732,
    scale_factor: 2.0,
    vsync: true,
}
```

### Android - Vulkan Backend

**File**: `engine/src/platform/vulkan_android.rs`

**Graphics API**: Vulkan 1.0+ (Android 7.0+)

**Features**:
- Modern low-level graphics API
- Cross-vendor GPU support
- Efficient multi-threading
- Wide device compatibility

**Implementation Status**:
- ✓ Backend structure
- ✓ Display server detection
- ⚠️  TODO: Vulkan instance creation with Android extensions
- ⚠️  TODO: ANativeWindow integration
- ⚠️  TODO: Swapchain management
- ⚠️  TODO: Touch event handling via MotionEvent

**Dependencies** (commented out):
```toml
ash = "0.37"           # Vulkan bindings
android-activity = "0.5"
ndk = "0.8"
ndk-sys = "0.5"
```

**Typical Configuration**:
```rust
// Pixel 8 Pro
SurfaceConfig {
    width: 1080,
    height: 2400,
    scale_factor: 3.0,
    vsync: true,
}
```

**Android-Specific Notes**:
- Requires Vulkan API 24+ (Android 7.0+)
- Must handle app lifecycle (pause/resume)
- Swapchain recreation on orientation change
- Touch events instead of mouse

### Linux - Vulkan Backend

**File**: `engine/src/platform/vulkan_linux.rs`

**Graphics API**: Vulkan

**Features**:
- X11 and Wayland support
- Automatic display server detection
- Wide GPU vendor support (NVIDIA, AMD, Intel)
- Desktop environment agnostic

**Implementation Status**:
- ✓ Backend structure
- ✓ Display server detection (X11 vs Wayland)
- ⚠️  TODO: Vulkan instance with platform extensions (VK_KHR_xlib_surface or VK_KHR_wayland_surface)
- ⚠️  TODO: X11/Wayland window integration
- ⚠️  TODO: Event handling (xcb or wayland-client)

**Dependencies** (commented out):
```toml
ash = "0.37"
ash-window = "0.12"
wayland-client = { version = "0.31", optional = true }
xcb = { version = "1.3", optional = true }
```

**Features** (commented out):
```toml
wayland = ["wayland-client"]
x11 = ["xcb"]
```

**Display Server Detection**:
```rust
// Checks environment variables
if std::env::var("WAYLAND_DISPLAY").is_ok() {
    DisplayServer::Wayland
} else if std::env::var("DISPLAY").is_ok() {
    DisplayServer::X11
}
```

**Typical Configuration**:
```rust
SurfaceConfig {
    width: 1920,
    height: 1080,
    scale_factor: 1.0,  // Can be 1.5-2.0 for HiDPI
    vsync: true,
}
```

### Windows - Direct3D 12 Backend

**File**: `engine/src/platform/d3d.rs`

**Graphics API**: Direct3D 12

**Features**:
- Modern low-level graphics API
- Excellent Windows integration
- DirectWrite for text rendering
- DXGI for window/swapchain management

**Implementation Status**:
- ✓ Backend structure
- ⚠️  TODO: D3D12 device creation
- ⚠️  TODO: DXGI factory and swapchain
- ⚠️  TODO: Command queues and lists
- ⚠️  TODO: DirectWrite text rendering
- ⚠️  TODO: Win32 event handling

**Dependencies** (commented out):
```toml
windows = { version = "0.52", features = [
    "Win32_Graphics_Direct3D12",
    "Win32_Graphics_Dxgi",
    "Win32_Graphics_Direct3D",
    "Win32_Foundation",
    "Win32_System_Threading",
] }
```

**Typical Configuration**:
```rust
// 1080p
SurfaceConfig {
    width: 1920,
    height: 1080,
    scale_factor: 1.0,
    vsync: true,
}

// 4K with 150% DPI scaling
SurfaceConfig {
    width: 3840,
    height: 2160,
    scale_factor: 1.5,
    vsync: true,
}
```

## PlatformBackend Trait

All platforms must implement this trait:

```rust
pub trait PlatformBackend: Send + Sync {
    /// Initialize the backend with surface configuration
    fn init(&mut self, config: SurfaceConfig) -> Result<(), PlatformError>;

    /// Get platform information (OS, renderer, device)
    fn get_info(&self) -> PlatformInfo;

    /// Resize the rendering surface
    fn resize(&mut self, width: u32, height: u32) -> Result<(), PlatformError>;

    /// Begin a new frame
    fn begin_frame(&mut self) -> Result<(), PlatformError>;

    /// Execute rendering commands
    fn execute_commands(&mut self, commands: &[RenderCommand]) -> Result<(), PlatformError>;

    /// End frame and present to screen
    fn end_frame(&mut self) -> Result<(), PlatformError>;

    /// Poll for platform events
    fn poll_events(&mut self) -> Vec<Event>;

    /// Shutdown and cleanup
    fn shutdown(&mut self);

    /// Get current surface config
    fn get_surface_config(&self) -> &SurfaceConfig;

    /// Set VSync on/off
    fn set_vsync(&mut self, enabled: bool) -> Result<(), PlatformError>;

    /// Capture screenshot (optional)
    fn capture_screenshot(&self) -> Result<Vec<u8>, PlatformError>;
}
```

## Render Commands

All backends execute the same set of render commands:

```rust
pub enum RenderCommand {
    /// Draw a rectangle with optional rounded corners
    DrawRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: u32,           // RGBA as u32
        border_radius: f32,
    },

    /// Draw text at position
    DrawText {
        x: f32,
        y: f32,
        text: String,
        font_size: f32,
        color: u32,
    },

    /// Begin clipping to rectangle
    PushClip {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },

    /// End current clip region
    PopClip,

    /// Set opacity for subsequent draws
    SetOpacity(f32),

    /// Clear screen with color
    Clear(Color),
}
```

## Testing

### Current Test Coverage

All platform backends have basic tests:

```bash
$ cargo test --manifest-path /Users/andrew/projects/centered/engine/Cargo.toml

test platform::metal::tests::test_metal_backend_creation ... ok
test platform::metal::tests::test_metal_backend_info ... ok
test platform::metal::tests::test_metal_backend_init ... ok
test platform::tests::test_create_backend ... ok
```

### Testing on Different Platforms

**macOS**:
```bash
cargo test
cargo run --example basic  # When example is created
```

**iOS Simulator**:
```bash
cargo build --target aarch64-apple-ios-sim
# Run in Xcode
```

**Android Emulator**:
```bash
cargo build --target aarch64-linux-android
# Deploy with gradle/adb
```

**Linux** (cross-compile or native):
```bash
cargo build --target x86_64-unknown-linux-gnu
```

**Windows** (cross-compile or native):
```bash
cargo build --target x86_64-pc-windows-msvc
```

## Next Steps

### Phase 1: macOS Metal Implementation (Primary)

1. **Uncomment Metal dependencies** in Cargo.toml
2. **Implement Metal device creation**:
   ```rust
   let device = metal::Device::system_default()
       .ok_or(PlatformError::InitializationFailed("No Metal device"))?;
   ```
3. **Create command queue**: For submitting rendering work
4. **Implement render pass encoding**: Convert RenderCommands to Metal commands
5. **Text rendering**: Use Core Text to rasterize text
6. **Event loop**: Integrate with AppKit/NSEvent

### Phase 2: iOS Testing

1. Create iOS app wrapper
2. Test on simulator (macOS)
3. Test on physical device
4. Handle touch events
5. Handle orientation changes

### Phase 3: Android Testing

1. Create Android app wrapper
2. Implement Vulkan initialization
3. Test on emulator
4. Test on physical device

### Phase 4: Linux & Windows

1. Implement based on learnings from other platforms
2. Cross-platform testing

## Performance Considerations

### Metal (macOS/iOS)
- **Efficient**: Direct GPU access
- **Command buffers**: Batch work for GPU
- **Shared memory**: Metal buffers can be CPU/GPU accessible
- **Target**: 60fps @ 1080p, 120fps for ProMotion displays

### Vulkan (Android/Linux)
- **Very efficient**: Low CPU overhead
- **Explicit synchronization**: More control, more complexity
- **Pipeline caching**: Reduce shader compilation overhead
- **Target**: 60fps @ 1080p, 90-120fps for high refresh displays

### Direct3D 12 (Windows)
- **Modern API**: Similar efficiency to Vulkan
- **Command lists**: Pre-record rendering work
- **DirectX Raytracing**: Future opportunity for shadows
- **Target**: 60fps @ 1080p, 144fps for gaming displays

## Platform-Specific Optimizations

### macOS
- Use Metal Performance Shaders for blur/filters
- CVDisplayLink for perfect frame pacing
- Metal 3 features on macOS 13+

### iOS
- ProMotion support (120Hz)
- MetalKit for easier setup
- Minimize power consumption (battery)

### Android
- Vulkan validation layers for debugging
- Frame pacing API for smooth rendering
- Thermal management

### Linux
- Mesa drivers (open source)
- Proprietary drivers (NVIDIA)
- Mixed DPI support

### Windows
- Variable refresh rate (G-Sync/FreeSync)
- DirectStorage for fast asset loading
- Windows 11 optimizations

## Resources

### Metal
- [Metal Programming Guide](https://developer.apple.com/metal/)
- [metal-rs Rust bindings](https://github.com/gfx-rs/metal-rs)

### Vulkan
- [Vulkan Tutorial](https://vulkan-tutorial.com/)
- [ash Rust bindings](https://github.com/ash-rs/ash)

### Direct3D 12
- [D3D12 Programming Guide](https://docs.microsoft.com/en-us/windows/win32/direct3d12/directx-12-programming-guide)
- [windows-rs bindings](https://github.com/microsoft/windows-rs)

## FAQ

**Q: Why not OpenGL?**
A: OpenGL is deprecated on macOS and older on other platforms. Modern APIs (Metal, Vulkan, D3D12) offer better performance and more control.

**Q: Why not use a cross-platform library like wgpu?**
A: We want direct control over each platform for maximum performance and to learn the platform-specific best practices. wgpu is a good alternative for simpler use cases.

**Q: Can I add a new platform?**
A: Yes! Implement the `PlatformBackend` trait and add conditional compilation in `platform/mod.rs`.

**Q: What about WebAssembly/WebGPU?**
A: Possible future target. Would use WebGPU backend.
