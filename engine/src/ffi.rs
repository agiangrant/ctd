//! FFI layer - C-compatible interface for Go interop
//!
//! Design principles:
//! - Single FFI call per frame (immediate mode)
//! - Minimal FFI calls on updates (retained mode)
//! - Safe memory management across boundary
//! - Efficient binary format for render commands (no JSON)

use crate::{
    Engine, EngineConfig,
    event::EventBatch,
    render::{RenderCommand, RenderMode},
    text::{FontDescriptor, FontSource, FontStyle, TextLayoutConfig, TextAlign, VerticalAlign, WordBreak, TextOverflow, WhiteSpace},
    widget::WidgetDelta,
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Mutex;

/// Opaque engine handle for FFI
pub type EngineHandle = *mut Engine;

/// Global engine storage (for simplicity, can be refactored later)
static ENGINE_MAP: Mutex<Option<Engine>> = Mutex::new(None);

/// Safe area insets (top, left, bottom, right) in logical pixels.
/// Updated on iOS/Android when window is created and on resize.
/// Returns (0, 0, 0, 0) on desktop platforms.
#[derive(Clone, Copy, Default)]
struct SafeAreaInsets {
    top: f32,
    left: f32,
    bottom: f32,
    right: f32,
}

static SAFE_AREA_INSETS: Mutex<SafeAreaInsets> = Mutex::new(SafeAreaInsets {
    top: 0.0,
    left: 0.0,
    bottom: 0.0,
    right: 0.0,
});

/// Initialize the engine with configuration JSON
///
/// # Safety
/// - config_json must be a valid null-terminated UTF-8 string
/// - caller must eventually call centered_engine_destroy
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_init(config_json: *const c_char) -> EngineHandle {
    if config_json.is_null() {
        return ptr::null_mut();
    }

    let config_str = match CStr::from_ptr(config_json).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let config: EngineConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(_) => EngineConfig::default(),
    };

    let engine = Engine::new(config);
    let mut map = ENGINE_MAP.lock().unwrap();
    *map = Some(engine);

    // Return a non-null pointer to indicate success
    // (we're using global storage for now)
    1 as EngineHandle
}

/// Destroy the engine and free resources
///
/// # Safety
/// - handle must be a valid handle from centered_engine_init
/// - handle must not be used after this call
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_destroy(_handle: EngineHandle) {
    let mut map = ENGINE_MAP.lock().unwrap();
    *map = None;
}

/// Submit a frame for immediate mode rendering
/// Returns a JSON string of events (caller must free with centered_free_string)
///
/// # Safety
/// - handle must be valid
/// - frame_json must be a valid null-terminated UTF-8 string containing widget tree
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_submit_frame(
    _handle: EngineHandle,
    frame_json: *const c_char,
) -> *mut c_char {
    if frame_json.is_null() {
        return ptr::null_mut();
    }

    let _frame_str = match CStr::from_ptr(frame_json).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    // Parse widget tree from JSON
    // TODO: Process widget tree and render

    // Return empty event batch for now
    let event_batch = EventBatch::default();
    let events_json = match serde_json::to_string(&event_batch) {
        Ok(json) => json,
        Err(_) => return ptr::null_mut(),
    };

    match CString::new(events_json) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Submit a delta update for retained mode
/// Returns a JSON string of events (caller must free with centered_free_string)
///
/// # Safety
/// - handle must be valid
/// - delta_json must be a valid null-terminated UTF-8 string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_submit_delta(
    _handle: EngineHandle,
    delta_json: *const c_char,
) -> *mut c_char {
    if delta_json.is_null() {
        return ptr::null_mut();
    }

    let delta_str = match CStr::from_ptr(delta_json).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let _delta: WidgetDelta = match serde_json::from_str(delta_str) {
        Ok(d) => d,
        Err(_) => return ptr::null_mut(),
    };

    // TODO: Apply delta to widget tree and re-render if needed

    // Return empty event batch for now
    let event_batch = EventBatch::default();
    let events_json = match serde_json::to_string(&event_batch) {
        Ok(json) => json,
        Err(_) => return ptr::null_mut(),
    };

    match CString::new(events_json) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Load styles from TOML configuration
/// Returns 0 on success, non-zero on error
///
/// # Safety
/// - handle must be valid
/// - toml must be a valid null-terminated UTF-8 string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_load_styles(
    _handle: EngineHandle,
    toml: *const c_char,
) -> i32 {
    if toml.is_null() {
        return -1;
    }

    let toml_str = match CStr::from_ptr(toml).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut map = ENGINE_MAP.lock().unwrap();
    if let Some(engine) = map.as_mut() {
        match engine.style_system.load_theme(toml_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Resize the rendering surface
///
/// # Safety
/// - handle must be valid
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_resize(
    _handle: EngineHandle,
    width: u32,
    height: u32,
) {
    let mut map = ENGINE_MAP.lock().unwrap();
    if let Some(engine) = map.as_mut() {
        engine.resize(width, height);
    }
}

/// Get the current rendering mode
///
/// # Safety
/// - handle must be valid
/// - Returns 0 for Immediate, 1 for Retained
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_get_mode(_handle: EngineHandle) -> i32 {
    let map = ENGINE_MAP.lock().unwrap();
    if let Some(engine) = map.as_ref() {
        match engine.mode() {
            RenderMode::Immediate => 0,
            RenderMode::Retained => 1,
        }
    } else {
        -1
    }
}

/// Free a string returned by the engine
///
/// # Safety
/// - ptr must be a string returned by the engine
/// - ptr must not be used after this call
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Get version string (for debugging)
/// Returns a static string (do NOT free)
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_engine_version() -> *const c_char {
    "0.1.0\0".as_ptr() as *const c_char
}

/// Get the app's internal files directory path (Android only).
/// Returns NULL on non-Android platforms or if not yet initialized.
/// The returned string is owned by the engine - do NOT free it.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_get_app_files_dir() -> *const c_char {
    #[cfg(target_os = "android")]
    {
        use crate::platform::android::get_app_files_dir;
        match get_app_files_dir() {
            Some(path) => {
                // Return as C string - caller must not free this
                // We need a static CString, so we'll use a lazy static
                use std::sync::OnceLock;
                static FILES_DIR_CSTRING: OnceLock<std::ffi::CString> = OnceLock::new();
                let cstr = FILES_DIR_CSTRING.get_or_init(|| {
                    std::ffi::CString::new(path).unwrap_or_default()
                });
                cstr.as_ptr()
            }
            None => std::ptr::null(),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        std::ptr::null()
    }
}

// ============================================================================
// FFI Render Command Structures (C-compatible)
// ============================================================================

/// C-compatible font source type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FFIFontSourceType {
    System = 0,
    Bundled = 1,
    Memory = 2,
}

/// C-compatible draw text command
#[repr(C)]
pub struct FFIDrawTextCommand {
    pub x: f32,
    pub y: f32,

    // Text content
    pub text_ptr: *const u8,
    pub text_len: usize,

    // Font source
    pub font_source_type: u8,     // FFIFontSourceType
    pub font_name_ptr: *const u8,  // Font name or path
    pub font_name_len: usize,
    pub font_data_hash: u64,       // For Memory fonts

    // Font properties
    pub font_weight: u16,          // 100-900
    pub font_style: u8,            // 0=Normal, 1=Italic
    pub font_size: f32,            // Points

    // Color
    pub color: u32,                // 0xRRGGBBAA

    // Layout
    pub max_width: f32,            // 0.0 = no constraint
    pub max_height: f32,           // 0.0 = no constraint
    pub line_height: f32,          // Multiplier
    pub letter_spacing: f32,       // em units
    pub word_spacing: f32,         // em units

    // Alignment
    pub alignment: u8,             // TextAlign
    pub vertical_align: u8,        // VerticalAlign

    // Behavior
    pub word_break: u8,            // WordBreak
    pub overflow: u8,              // TextOverflow
    pub white_space: u8,           // WhiteSpace
}

/// C-compatible draw rect command
#[repr(C)]
pub struct FFIDrawRectCommand {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: u32,
    pub border_radius: f32,
}

/// C-compatible render command type
#[repr(u8)]
pub enum FFIRenderCommandType {
    DrawRect = 0,
    DrawText = 1,
    PushClip = 2,
    PopClip = 3,
    SetOpacity = 4,
    Clear = 5,
}

/// C-compatible render command (tagged union)
#[repr(C)]
pub struct FFIRenderCommand {
    pub cmd_type: u8,  // FFIRenderCommandType
    pub padding: [u8; 7],  // Padding for alignment
    pub data: FFIRenderCommandData,
}

/// Union of all command data types
#[repr(C)]
pub union FFIRenderCommandData {
    pub draw_rect: std::mem::ManuallyDrop<FFIDrawRectCommand>,
    pub draw_text: std::mem::ManuallyDrop<FFIDrawTextCommand>,
    pub push_clip: std::mem::ManuallyDrop<FFIDrawRectCommand>,  // Same layout
    pub set_opacity: f32,
    pub clear_color: u32,
}

impl FFIDrawTextCommand {
    /// Convert FFI command to internal RenderCommand
    ///
    /// # Safety
    /// Caller must ensure all pointers are valid
    pub unsafe fn to_render_command(&self) -> RenderCommand {
        // Convert text
        let text = std::str::from_utf8_unchecked(
            std::slice::from_raw_parts(self.text_ptr, self.text_len)
        ).to_string();

        // Convert font name/path
        let font_name = std::str::from_utf8_unchecked(
            std::slice::from_raw_parts(self.font_name_ptr, self.font_name_len)
        ).to_string();

        // Create font source
        let source = match self.font_source_type {
            0 => FontSource::System(font_name),
            1 => FontSource::Bundled(font_name),
            2 => FontSource::Memory {
                name: font_name,
                data_hash: self.font_data_hash,
            },
            _ => FontSource::System("system".to_string()),
        };

        // Create font descriptor
        let font = FontDescriptor {
            source,
            weight: self.font_weight,
            style: FontStyle::from(self.font_style),
            size: self.font_size,
        };

        // Create layout config
        let layout = TextLayoutConfig {
            max_width: if self.max_width > 0.0 { Some(self.max_width) } else { None },
            max_height: if self.max_height > 0.0 { Some(self.max_height) } else { None },
            max_lines: None,  // Not exposed in FFI yet
            line_height: self.line_height,
            letter_spacing: self.letter_spacing,
            word_spacing: self.word_spacing,
            alignment: TextAlign::from(self.alignment),
            vertical_align: VerticalAlign::from(self.vertical_align),
            word_break: WordBreak::from(self.word_break),
            overflow: TextOverflow::from(self.overflow),
            white_space: WhiteSpace::from(self.white_space),
        };

        RenderCommand::DrawText {
            x: self.x,
            y: self.y,
            text,
            font,
            color: self.color,
            layout,
        }
    }
}

impl FFIRenderCommand {
    /// Convert FFI command to internal RenderCommand
    ///
    /// # Safety
    /// Caller must ensure all pointers in command data are valid
    pub unsafe fn to_render_command(&self) -> RenderCommand {
        match self.cmd_type {
            0 => {
                let rect = &*self.data.draw_rect;
                // Convert single border_radius to corner_radii array
                let r = rect.border_radius;
                RenderCommand::DrawRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    color: rect.color,
                    corner_radii: [r, r, r, r],
                    rotation: 0.0, // C FFI doesn't support rotation yet
                    border: None,
                    gradient: None,
                }
            },
            1 => {
                let text = &*self.data.draw_text;
                text.to_render_command()
            },
            2 => {
                let clip = &*self.data.push_clip;
                RenderCommand::PushClip {
                    x: clip.x,
                    y: clip.y,
                    width: clip.width,
                    height: clip.height,
                }
            },
            3 => RenderCommand::PopClip {},
            4 => RenderCommand::SetOpacity(self.data.set_opacity),
            5 => {
                let color_u32 = self.data.clear_color;
                let r = ((color_u32 >> 24) & 0xFF) as u8;
                let g = ((color_u32 >> 16) & 0xFF) as u8;
                let b = ((color_u32 >> 8) & 0xFF) as u8;
                let a = (color_u32 & 0xFF) as u8;
                RenderCommand::Clear(crate::style::Color { r, g, b, a })
            },
            _ => RenderCommand::PopClip {},  // Fallback
        }
    }
}

/// Render a batch of commands (optimized FFI call)
///
/// # Safety
/// - commands_ptr must point to valid FFIRenderCommand array
/// - All string pointers in commands must be valid UTF-8
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_engine_render_batch(
    _handle: EngineHandle,
    commands_ptr: *const FFIRenderCommand,
    commands_len: usize,
) -> i32 {
    if commands_ptr.is_null() || commands_len == 0 {
        return -1;
    }

    // Convert FFI commands to internal commands
    let ffi_commands = std::slice::from_raw_parts(commands_ptr, commands_len);
    let mut render_commands = Vec::with_capacity(commands_len);

    for ffi_cmd in ffi_commands {
        render_commands.push(ffi_cmd.to_render_command());
    }

    // TODO: Execute commands through platform backend
    // For now, just validate we can convert them
    #[cfg(debug_assertions)]
    println!("Received {} render commands via FFI", render_commands.len());

    0  // Success
}

// ============================================================================
// Platform Backend FFI - Window and Rendering Surface Management
// ============================================================================

// On wasm32, wgpu types don't implement Send/Sync (WebGPU is single-threaded).
// The C FFI backend functions are not used on wasm32 - web uses wasm-bindgen in platform/web.rs.
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::wgpu_backend::{SurfaceConfig, WgpuBackend};
use crate::platform::window_styling::{apply_window_style, WindowStyleOptions};
use std::sync::OnceLock;

#[cfg(not(target_arch = "wasm32"))]
/// Global backend storage (single instance for now)
static BACKEND: OnceLock<Mutex<Option<WgpuBackend>>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
/// Get the global backend storage
/// Used by FFI functions and iOS platform to access the shared backend
pub fn get_backend() -> &'static Mutex<Option<WgpuBackend>> {
    BACKEND.get_or_init(|| Mutex::new(None))
}

#[cfg(not(target_arch = "wasm32"))]
/// Set the global backend (used by iOS platform)
pub fn set_backend(backend: WgpuBackend) {
    let lock = get_backend();
    let mut guard = lock.lock().unwrap();
    *guard = Some(backend);
}

/// Global state for frameless window rendering
/// This allows the batch protocol render path to access window controls
/// without going through the App struct in the event callback
#[cfg(not(target_arch = "wasm32"))]
struct FramelessState {
    decorations: bool,
    show_native_controls: bool,
    dark_mode: bool,
    scale_factor: f64,
    #[cfg(target_os = "linux")]
    window_controls: Option<crate::platform::linux::WindowControls>,
    #[cfg(target_os = "windows")]
    window_controls: Option<crate::platform::windows::WindowControls>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for FramelessState {
    fn default() -> Self {
        Self {
            decorations: true,
            show_native_controls: false,
            dark_mode: false,
            scale_factor: 1.0,
            #[cfg(target_os = "linux")]
            window_controls: None,
            #[cfg(target_os = "windows")]
            window_controls: None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
static FRAMELESS_STATE: OnceLock<Mutex<FramelessState>> = OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn get_frameless_state() -> &'static Mutex<FramelessState> {
    FRAMELESS_STATE.get_or_init(|| Mutex::new(FramelessState::default()))
}

/// Create a rendering backend with a native window handle (macOS: NSView pointer)
///
/// This is the primary way to initialize rendering from Go/C.
/// The caller creates a window using their preferred windowing library (GLFW, SDL, etc.)
/// and passes the native view/window handle to Rust.
///
/// # Arguments
/// * `window_handle` - Platform-specific window handle:
///   - macOS: NSView pointer (from GLFW: glfwGetCocoaWindow, then contentView)
///   - Windows: HWND
///   - Linux/X11: Window (XID)
///   - Linux/Wayland: wl_surface pointer
/// * `width` - Width in physical pixels
/// * `height` - Height in physical pixels
/// * `scale_factor` - HiDPI scale factor (e.g., 2.0 for Retina)
///
/// # Returns
/// 0 on success, negative error code on failure
///
/// # Safety
/// - window_handle must be a valid native window/view pointer
/// - The window must remain valid for the lifetime of the backend
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_init(
    window_handle: *mut std::ffi::c_void,
    _display_handle: *mut std::ffi::c_void,  // Reserved for Linux/Wayland
    width: u32,
    height: u32,
    scale_factor: f64,
) -> i32 {
    if window_handle.is_null() {
        return -1;
    }

    // Create the backend
    let mut backend = WgpuBackend::new();

    // Create platform-specific window handle wrapper
    #[cfg(target_os = "macos")]
    let result = {
        use raw_window_handle::{AppKitWindowHandle, RawWindowHandle, HasWindowHandle, HasDisplayHandle, AppKitDisplayHandle, RawDisplayHandle};

        // Create a wrapper struct that implements HasWindowHandle
        // We use a raw pointer and mark it Send+Sync since we know it's valid
        struct MacOSWindowHandle {
            ns_view: *mut std::ffi::c_void,
        }

        // SAFETY: The NSView pointer is valid and we only use it from one thread
        unsafe impl Send for MacOSWindowHandle {}
        unsafe impl Sync for MacOSWindowHandle {}

        impl HasWindowHandle for MacOSWindowHandle {
            fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
                // SAFETY: We verified the pointer is non-null at construction
                let ns_view = unsafe { std::ptr::NonNull::new_unchecked(self.ns_view) };
                let handle = AppKitWindowHandle::new(ns_view);
                let raw = RawWindowHandle::AppKit(handle);
                // SAFETY: The handle is valid for the lifetime of self
                Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
            }
        }

        impl HasDisplayHandle for MacOSWindowHandle {
            fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
                let handle = AppKitDisplayHandle::new();
                let raw = RawDisplayHandle::AppKit(handle);
                // SAFETY: AppKit display handle is always valid on macOS
                Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw) })
            }
        }

        // SAFETY: Caller guarantees window_handle is a valid NSView
        let wrapper = MacOSWindowHandle {
            ns_view: window_handle,
        };

        let config = SurfaceConfig {
            width,
            height,
            scale_factor,
            vsync: true,
            low_power_gpu: false,
            allow_software_fallback: false,
        };

        // Initialize backend with window
        pollster::block_on(backend.init_with_window(&wrapper, config))
    };

    #[cfg(not(target_os = "macos"))]
    let result: Result<(), Box<dyn std::error::Error>> = {
        Err("Platform not yet supported for FFI window initialization".into())
    };

    match result {
        Ok(()) => {
            // Store the backend
            let backend_lock = get_backend();
            let mut guard = backend_lock.lock().unwrap();
            *guard = Some(backend);
            0
        }
        Err(e) => {
            eprintln!("Failed to initialize backend: {}", e);
            -2
        }
    }
}

/// Destroy the rendering backend and free resources
///
/// # Safety
/// - Must only be called once after centered_backend_init
/// - No rendering calls should be made after this
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_destroy() {
    let backend_lock = get_backend();
    let mut guard = backend_lock.lock().unwrap();
    *guard = None;
}

/// Resize the rendering surface
///
/// Call this when the window is resized.
///
/// # Arguments
/// * `width` - New width in physical pixels
/// * `height` - New height in physical pixels
/// * `scale_factor` - HiDPI scale factor
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_resize(width: u32, height: u32, scale_factor: f64) -> i32 {
    let backend_lock = get_backend();
    let mut guard = backend_lock.lock().unwrap();

    if let Some(backend) = guard.as_mut() {
        match backend.resize(width, height, scale_factor) {
            Ok(()) => 0,
            Err(_) => -2,
        }
    } else {
        -1
    }
}

/// Render a frame with the given commands (JSON format)
///
/// This is the main rendering entry point for immediate mode rendering.
/// Go builds a list of render commands, serializes to JSON, and calls this function.
///
/// # Arguments
/// * `commands_json` - JSON array of render commands
///
/// # Returns
/// 0 on success, negative error code on failure
///
/// # Safety
/// - commands_json must be a valid null-terminated UTF-8 string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_render_frame(
    commands_json: *const c_char,
) -> i32 {
    if commands_json.is_null() {
        return -1;
    }

    let json_str = match CStr::from_ptr(commands_json).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    // Parse commands from JSON
    let commands: Vec<RenderCommand> = match serde_json::from_str(json_str) {
        Ok(cmds) => cmds,
        Err(e) => {
            eprintln!("Failed to parse render commands: {}", e);
            return -3;
        }
    };

    // On iOS, use the thread-local backend
    #[cfg(target_os = "ios")]
    {
        match crate::platform::ios::render_frame(&commands) {
            Ok(()) => return 0,
            Err(e) => {
                eprintln!("iOS render error: {}", e);
                return -4;
            }
        }
    }

    // On other platforms, use the global backend
    #[cfg(not(target_os = "ios"))]
    {
        let backend_lock = get_backend();
        let mut guard = backend_lock.lock().unwrap();

        if let Some(backend) = guard.as_mut() {
            match backend.render_frame(&commands) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Render error: {}", e);
                    -4
                }
            }
        } else {
            eprintln!("Backend not initialized");
            -5
        }
    }
}

/// Begin a new frame (call before rendering commands)
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_begin_frame() -> i32 {
    // Currently a no-op, but reserved for future use (e.g., acquiring next swapchain image)
    0
}

/// End the current frame and present to screen
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_end_frame() -> i32 {
    // Currently handled within render_frame, but reserved for explicit control
    0
}

// ============================================================================
// Image/Texture Management FFI
// ============================================================================

use crate::image::LoadedImage;

/// Load an image from raw bytes and return a texture ID
///
/// Supports PNG and JPEG formats. The image is decoded and uploaded to the GPU.
///
/// # Arguments
/// * `data_ptr` - Pointer to image file data (PNG, JPEG, etc.)
/// * `data_len` - Length of data in bytes
///
/// # Returns
/// Positive texture ID on success, negative error code on failure:
/// - -1: Invalid parameters (null pointer or zero length)
/// - -2: Backend not initialized
/// - -3: Failed to decode image
/// - -4: Failed to upload to GPU
///
/// # Safety
/// - data_ptr must point to valid memory of at least data_len bytes
/// - The data is copied, so the caller can free data_ptr after this returns
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_load_image(
    data_ptr: *const u8,
    data_len: usize,
) -> i32 {
    if data_ptr.is_null() || data_len == 0 {
        return -1;
    }

    // Copy the data
    let data = std::slice::from_raw_parts(data_ptr, data_len);

    // Decode the image
    let loaded_image = match LoadedImage::from_bytes(data) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Failed to decode image: {}", e);
            return -3;
        }
    };

    // Get the backend and upload
    let backend_lock = get_backend();
    let mut guard = backend_lock.lock().unwrap();

    if let Some(backend) = guard.as_mut() {
        match backend.load_image(&loaded_image) {
            Ok(texture_id) => texture_id as i32,
            Err(e) => {
                eprintln!("Failed to upload image to GPU: {}", e);
                -4
            }
        }
    } else {
        eprintln!("Backend not initialized");
        -2
    }
}

/// Load an image from a file path and return a texture ID
///
/// Convenience wrapper around centered_backend_load_image for file paths.
///
/// # Arguments
/// * `path` - Null-terminated UTF-8 file path
///
/// # Returns
/// Positive texture ID on success, negative error code on failure
///
/// # Safety
/// - path must be a valid null-terminated UTF-8 string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_load_image_file(
    path: *const c_char,
) -> i32 {
    if path.is_null() {
        return -1;
    }

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Load from file
    let loaded_image = match LoadedImage::from_file(path_str) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Failed to load image file '{}': {}", path_str, e);
            return -3;
        }
    };

    // Get the backend and upload
    let backend_lock = get_backend();
    let mut guard = backend_lock.lock().unwrap();

    if let Some(backend) = guard.as_mut() {
        match backend.load_image(&loaded_image) {
            Ok(texture_id) => texture_id as i32,
            Err(e) => {
                eprintln!("Failed to upload image to GPU: {}", e);
                -4
            }
        }
    } else {
        eprintln!("Backend not initialized");
        -2
    }
}

/// Unload an image texture and free GPU resources
///
/// # Arguments
/// * `texture_id` - Texture ID returned by centered_backend_load_image
///
/// # Returns
/// 0 on success, negative error code on failure:
/// - -1: Invalid texture ID
/// - -2: Backend not initialized
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_unload_image(texture_id: u32) -> i32 {
    let backend_lock = get_backend();
    let mut guard = backend_lock.lock().unwrap();

    if let Some(backend) = guard.as_mut() {
        backend.unload_image(texture_id);
        0
    } else {
        -2
    }
}

/// Get texture dimensions for a loaded image
///
/// # Arguments
/// * `texture_id` - Texture ID returned by centered_backend_load_image
/// * `width_out` - Pointer to store width (in pixels)
/// * `height_out` - Pointer to store height (in pixels)
///
/// # Returns
/// 0 on success, negative error code on failure:
/// - -1: Invalid texture ID or texture not found
/// - -2: Backend not initialized
/// - -3: Null pointer for width_out or height_out
///
/// # Safety
/// - width_out and height_out must be valid pointers to u32
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_backend_get_texture_size(
    texture_id: u32,
    width_out: *mut u32,
    height_out: *mut u32,
) -> i32 {
    if width_out.is_null() || height_out.is_null() {
        return -3;
    }

    let backend_lock = get_backend();
    let guard = backend_lock.lock().unwrap();

    if let Some(backend) = guard.as_ref() {
        if let Some((width, height)) = backend.get_texture_size(texture_id) {
            *width_out = width;
            *height_out = height;
            0
        } else {
            -1
        }
    } else {
        -2
    }
}

// ============================================================================
// Video FFI
// ============================================================================
//
// Video playback API for loading and playing videos.
// Uses platform-native decoders (AVFoundation on macOS) for hardware-accelerated decoding.

use crate::video::player::VideoPlayer;
use crate::video::VideoFrame;

// Global video player storage
lazy_static::lazy_static! {
    static ref VIDEO_PLAYERS: std::sync::Mutex<std::collections::HashMap<u32, VideoPlayer>> = std::sync::Mutex::new(std::collections::HashMap::new());
    static ref NEXT_PLAYER_ID: std::sync::Mutex<u32> = std::sync::Mutex::new(1);
}

/// Create a new video player
///
/// # Returns
/// A unique player ID (always positive), or 0 on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_create() -> u32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    let mut next_id = NEXT_PLAYER_ID.lock().unwrap();

    let player_id = *next_id;
    *next_id += 1;

    players.insert(player_id, VideoPlayer::new());
    player_id
}

/// Destroy a video player and free resources
///
/// # Arguments
/// * `player_id` - Player ID from centered_video_create
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_destroy(player_id: u32) {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    players.remove(&player_id);
}

/// Load video from a URL (file:// or http://)
///
/// # Arguments
/// * `player_id` - Player ID from centered_video_create
/// * `url` - Null-terminated UTF-8 URL string
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_video_load_url(
    player_id: u32,
    url: *const c_char,
) -> i32 {
    if url.is_null() {
        return -1;
    }

    let url_str = match CStr::from_ptr(url).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.load_url(url_str) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Video load error: {}", e);
                -3
            }
        }
    } else {
        -2 // Player not found
    }
}

/// Load video from a file path
///
/// # Arguments
/// * `player_id` - Player ID from centered_video_create
/// * `path` - Null-terminated UTF-8 file path
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_video_load_file(
    player_id: u32,
    path: *const c_char,
) -> i32 {
    if path.is_null() {
        return -1;
    }

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.load_file(path_str) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Video load error: {}", e);
                -3
            }
        }
    } else {
        -2
    }
}

/// Initialize frame buffer for raw frame input (video streams)
///
/// # Arguments
/// * `player_id` - Player ID from centered_video_create
/// * `width` - Initial frame width
/// * `height` - Initial frame height
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_init_stream(
    player_id: u32,
    width: u32,
    height: u32,
) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.init_frame_buffer(width, height);
        0
    } else {
        -2
    }
}

/// Push a raw frame for video streams
///
/// # Arguments
/// * `player_id` - Player ID from centered_video_create
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `data` - RGBA pixel data (width * height * 4 bytes)
/// * `data_len` - Length of data in bytes
/// * `timestamp_ms` - Presentation timestamp in milliseconds
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_video_push_frame(
    player_id: u32,
    width: u32,
    height: u32,
    data: *const u8,
    data_len: usize,
    timestamp_ms: u64,
) -> i32 {
    if data.is_null() || data_len == 0 {
        return -1;
    }

    let expected_len = (width * height * 4) as usize;
    if data_len < expected_len {
        return -1;
    }

    let frame_data = std::slice::from_raw_parts(data, data_len).to_vec();
    let frame = VideoFrame::new(width, height, frame_data, timestamp_ms);

    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.push_frame(frame);
        0
    } else {
        -2
    }
}

/// Start or resume video playback
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_play(player_id: u32) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.play();
        0
    } else {
        -2
    }
}

/// Pause video playback
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_pause(player_id: u32) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.pause();
        0
    } else {
        -2
    }
}

/// Seek to a specific position
///
/// # Arguments
/// * `player_id` - Player ID
/// * `timestamp_ms` - Target position in milliseconds
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_seek(player_id: u32, timestamp_ms: u64) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.seek(timestamp_ms) {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Set looping behavior
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_set_looping(player_id: u32, looping: bool) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.set_looping(looping);
        0
    } else {
        -2
    }
}

/// Set muted state
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_set_muted(player_id: u32, muted: bool) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.set_muted(muted);
        0
    } else {
        -2
    }
}

/// Set volume (0.0 - 1.0)
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_set_volume(player_id: u32, volume: f32) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.set_volume(volume);
        0
    } else {
        -2
    }
}

/// Get current playback state
///
/// # Returns
/// PlaybackState as i32:
/// - 0: Idle
/// - 1: Loading
/// - 2: Playing
/// - 3: Paused
/// - 4: Ended
/// - 5: Error
/// - Negative: Player not found
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_get_state(player_id: u32) -> i32 {
    let players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        player.state() as i32
    } else {
        -2
    }
}

/// Get current playback position in milliseconds
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_get_time(player_id: u32) -> u64 {
    let players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        player.current_time_ms()
    } else {
        0
    }
}

/// Get video info
///
/// # Arguments
/// * `player_id` - Player ID
/// * `width_out` - Pointer to store video width
/// * `height_out` - Pointer to store video height
/// * `duration_ms_out` - Pointer to store duration in milliseconds
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_video_get_info(
    player_id: u32,
    width_out: *mut u32,
    height_out: *mut u32,
    duration_ms_out: *mut u64,
) -> i32 {
    if width_out.is_null() || height_out.is_null() || duration_ms_out.is_null() {
        return -1;
    }

    let players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        if let Some(info) = player.info() {
            *width_out = info.width;
            *height_out = info.height;
            *duration_ms_out = info.duration_ms;
            0
        } else {
            -3 // No video loaded
        }
    } else {
        -2
    }
}

/// Update video player and get texture ID for current frame
///
/// This should be called once per frame. It advances the video playback
/// and uploads new frames to the GPU as needed.
///
/// # Arguments
/// * `player_id` - Player ID
///
/// # Returns
/// Texture ID if a frame is available (use with DrawImage), 0 if no frame, negative on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_update(player_id: u32) -> i32 {
    let mut players = VIDEO_PLAYERS.lock().unwrap();
    let player = match players.get_mut(&player_id) {
        Some(p) => p,
        None => {
            eprintln!("VideoUpdate: player {} not found", player_id);
            return -2;
        }
    };

    // Update playback timing
    let has_new_frame = player.update();

    // Get or create texture
    if has_new_frame {
        if let Some(frame) = player.take_frame() {
            // Get backend for texture upload
            let backend_lock = get_backend();
            let mut backend_guard = backend_lock.lock().unwrap();

            if let Some(backend) = backend_guard.as_mut() {
                // Get or create texture ID
                let texture_id = match player.texture_id() {
                    Some(id) => id,
                    None => {
                        // Create new video texture
                        match backend.create_video_texture(frame.width, frame.height) {
                            Ok(id) => {
                                player.set_texture_id(id);
                                id
                            }
                            Err(e) => {
                                eprintln!("Failed to create video texture: {}", e);
                                return -4;
                            }
                        }
                    }
                };

                // Update texture with new frame
                if let Err(e) = backend.update_video_texture(
                    texture_id,
                    frame.width,
                    frame.height,
                    &frame.data,
                ) {
                    eprintln!("Failed to update video texture: {}", e);
                    return -5;
                }

                return texture_id as i32;
            } else {
                return -3; // Backend not initialized
            }
        }
    }

    // Return existing texture ID if we have one
    if let Some(id) = player.texture_id() {
        id as i32
    } else {
        0 // No texture yet
    }
}

/// Get texture ID for current video frame (without updating)
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_get_texture_id(player_id: u32) -> u32 {
    let players = VIDEO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        player.texture_id().unwrap_or(0)
    } else {
        0
    }
}

// ============================================================================
// Application Lifecycle FFI - Rust Owns Window
// ============================================================================
//
// This API is for the "Rust owns window" model where:
// - Rust creates and manages the window via winit
// - Rust owns the event loop and rendering
// - Go provides a callback that receives events and returns render commands
//
// Benefits:
// - Smooth rendering during Go GC pauses
// - Proper window resize handling
// - Simpler FFI (no native handle passing)
// - Consistent cross-platform behavior

use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Fullscreen, Window, WindowId},
    dpi::LogicalSize,
};

// Scancode extension is only available on desktop platforms
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use winit::platform::scancode::PhysicalKeyExtScancode;

/// Custom user events for cross-thread communication
#[derive(Debug, Clone)]
enum UserEvent {
    /// Request a redraw from outside the event loop
    RequestRedraw,
    /// Minimize the window
    Minimize,
    /// Maximize/unmaximize the window (toggles)
    ToggleMaximize,
    /// Enter fullscreen mode (borderless on primary monitor)
    EnterFullscreen,
    /// Exit fullscreen mode
    ExitFullscreen,
    /// Toggle fullscreen mode
    ToggleFullscreen,
    /// Close the window (request exit)
    Close,
    /// Set window title
    SetTitle(String),
    /// System theme changed (Linux only) - true = dark mode
    #[cfg(target_os = "linux")]
    SystemThemeChanged(bool),
}

/// Global event loop proxy for requesting redraws from any thread
static EVENT_LOOP_PROXY: std::sync::OnceLock<std::sync::Mutex<Option<EventLoopProxy<UserEvent>>>> = std::sync::OnceLock::new();

fn get_event_loop_proxy() -> &'static std::sync::Mutex<Option<EventLoopProxy<UserEvent>>> {
    EVENT_LOOP_PROXY.get_or_init(|| std::sync::Mutex::new(None))
}

/// Application configuration passed to centered_app_run
#[repr(C)]
pub struct AppConfig {
    /// Window title (null-terminated UTF-8 string)
    pub title: *const c_char,
    /// Initial window width in logical pixels
    pub width: u32,
    /// Initial window height in logical pixels
    pub height: u32,
    /// Enable VSync
    pub vsync: bool,
    /// Prefer low power GPU (integrated over discrete)
    pub low_power_gpu: bool,
    /// Allow software rendering fallback (false = fail on devices without proper GPU)
    pub allow_software_fallback: bool,
    /// Target frames per second (default: 60)
    /// Use lower values (e.g., 30) for lighter apps to save battery
    /// Use higher values (e.g., 120) for games on high refresh rate displays
    pub target_fps: u32,
    /// User data pointer passed to callbacks
    pub user_data: *mut std::ffi::c_void,

    // Window appearance options
    /// Show window decorations (title bar, close button, etc). false = frameless window
    pub decorations: bool,
    /// Transparent window background (requires compositing window manager)
    pub transparent: bool,
    /// Allow window to be resized
    pub resizable: bool,
    /// Keep window above all others
    pub always_on_top: bool,
    /// Minimum window width (0 = no minimum)
    pub min_width: u32,
    /// Minimum window height (0 = no minimum)
    pub min_height: u32,
    /// Maximum window width (0 = no maximum)
    pub max_width: u32,
    /// Maximum window height (0 = no maximum)
    pub max_height: u32,
    /// Initial X position (i32::MIN = system default/centered)
    pub x: i32,
    /// Initial Y position (i32::MIN = system default/centered)
    pub y: i32,

    // Frameless window styling options
    /// Corner radius for frameless windows (in points). 0 = no rounding.
    /// Only applies when decorations = false.
    pub corner_radius: f32,
    /// Show native window controls (close/minimize/maximize buttons) on frameless windows.
    /// On macOS, these are the traffic light buttons.
    pub show_native_controls: bool,
    /// Enable the minimize button (only used if show_native_controls = true)
    pub enable_minimize: bool,
    /// Enable the maximize/zoom button (only used if show_native_controls = true)
    pub enable_maximize: bool,
    /// Dark mode for window controls: 0 = light, 1 = dark, 2 = auto/system
    pub dark_mode: u8,
}

/// Event type for FFI
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum AppEventType {
    /// Window ready for first render
    Ready = 0,
    /// Window needs redraw
    RedrawRequested = 1,
    /// Window resized (data: width, height in physical pixels)
    Resized = 2,
    /// Window close requested
    CloseRequested = 3,
    /// Mouse moved (data: x, y in logical pixels)
    MouseMoved = 4,
    /// Mouse button pressed (data: button index)
    MousePressed = 5,
    /// Mouse button released (data: button index)
    MouseReleased = 6,
    /// Key pressed (data: keycode)
    KeyPressed = 7,
    /// Key released (data: keycode)
    KeyReleased = 8,
    /// Character input (data: UTF-32 codepoint)
    CharInput = 9,
    /// Mouse wheel scrolled (data: delta_x, delta_y)
    MouseWheel = 10,
    /// App suspended (iOS backgrounded)
    Suspended = 11,
    /// App resumed (iOS foregrounded)
    Resumed = 12,
    /// Keyboard frame changed (data1: height in logical points, 0 if hidden; data2: animation duration in seconds)
    KeyboardFrameChanged = 13,
}

/// Event data passed to callback
#[repr(C)]
pub struct AppEvent {
    pub event_type: AppEventType,
    /// Width for resize, x for mouse, keycode for key, etc.
    pub data1: f64,
    /// Height for resize, y for mouse, etc.
    pub data2: f64,
    /// Scale factor (for resize events)
    pub scale_factor: f64,
}

/// Frame response from Go callback
/// Supports both immediate mode (raw commands) and retained mode (widget deltas)
#[repr(C)]
pub struct FrameResponse {
    /// Immediate mode: JSON array of RenderCommands (rendered on top of retained widgets)
    /// Set to null if no immediate rendering needed
    pub immediate_commands: *mut c_char,
    /// Retained mode: JSON of WidgetDelta (updates to widget tree)
    /// Set to null if no widget updates
    pub widget_delta: *mut c_char,
    /// Request another frame immediately (for animations)
    /// If false, waits for next event before redrawing
    pub request_redraw: bool,
    /// Schedule a redraw after N milliseconds (0 = no delayed redraw)
    /// Used for cursor blink, delayed animations, etc.
    pub redraw_after_ms: u32,
    /// Dark mode for window controls: 0 = light, 1 = dark, 2 = auto/system
    /// Updated each frame to allow runtime changes
    pub dark_mode: u8,
    /// Layer-based rendering: JSON array of LayerInfo for regional caching
    /// When set, enables per-layer texture caching and compositing
    /// Set to null to use immediate_commands instead
    pub layers: *mut c_char,
    /// Dirty region for scissor-based partial rendering
    /// JSON of DirtyRegion. If set, Rust applies scissor rect to skip pixels outside.
    /// Set to null for full screen redraw.
    pub dirty_region: *mut c_char,
}

/// Dirty region for scissor-based partial rendering
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DirtyRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Layer information for regional caching
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LayerInfo {
    /// Unique layer identifier
    pub id: u32,
    /// Screen-space X position
    pub x: f32,
    /// Screen-space Y position
    pub y: f32,
    /// Layer width
    pub width: f32,
    /// Layer height
    pub height: f32,
    /// Compositing order (lower = further back)
    pub z_order: i32,
    /// True if fully opaque (no blending needed)
    pub opaque: bool,
    /// True if layer needs re-rendering
    pub dirty: bool,
    /// Commands for this layer (only present if dirty)
    #[serde(default)]
    pub commands: Vec<RenderCommand>,
}

/// Callback function type for the application loop
///
/// Called by Rust for each event. Go should:
/// 1. Process the event (update state)
/// 2. Fill in the FrameResponse with any updates
///
/// The callback receives a pointer to a FrameResponse struct to fill in.
/// Any non-null string pointers in the response must be allocated with C.CString
/// and will be freed by Rust after processing.
///
/// # Frame Response Fields:
/// - `immediate_commands`: JSON array of RenderCommands for immediate mode rendering
/// - `widget_delta`: JSON WidgetDelta for retained mode updates
/// - `request_redraw`: Set to true for continuous animation (60fps), false for event-driven
///
/// # Usage Patterns:
///
/// **Pure Immediate Mode (game loop):**
/// ```c
/// response->immediate_commands = build_frame_commands();
/// response->request_redraw = true;  // Always request next frame
/// ```
///
/// **Pure Retained Mode (UI app):**
/// ```c
/// if (state_changed) {
///     response->widget_delta = build_delta();
/// }
/// response->request_redraw = false;  // Only redraw on events
/// ```
///
/// **Hybrid (game with UI):**
/// ```c
/// response->widget_delta = ui_delta;        // Menu/HUD updates
/// response->immediate_commands = game_frame; // Game rendering
/// response->request_redraw = true;
/// ```
pub type AppCallback = unsafe extern "C" fn(
    event: *const AppEvent,
    response: *mut FrameResponse,
    user_data: *mut std::ffi::c_void,
);

/// Application state for the event loop
struct App {
    window: Option<Window>,
    backend: Option<WgpuBackend>,
    callback: AppCallback,
    user_data: *mut std::ffi::c_void,
    config: AppConfig,
    should_exit: bool,
    // Keyboard modifier state
    modifiers: winit::keyboard::ModifiersState,
    // Scheduled redraw time (for cursor blink, etc.)
    next_redraw_at: Option<std::time::Instant>,
    // Linux-specific: window controls and resize handling
    #[cfg(target_os = "linux")]
    mouse_position: (f64, f64),
    #[cfg(target_os = "linux")]
    resize_direction: Option<winit::window::ResizeDirection>,
    #[cfg(target_os = "linux")]
    window_controls: Option<crate::platform::linux::WindowControls>,
    #[cfg(target_os = "linux")]
    current_dark_mode: u8,
    // Windows-specific: window controls and resize handling
    #[cfg(target_os = "windows")]
    mouse_position: (f64, f64),
    #[cfg(target_os = "windows")]
    resize_direction: Option<winit::window::ResizeDirection>,
    #[cfg(target_os = "windows")]
    window_controls: Option<crate::platform::windows::WindowControls>,
    #[cfg(target_os = "windows")]
    current_dark_mode: u8,
}

// Modifier flags for keyboard events (passed in data2)
// These match common conventions and can be combined with bitwise OR
const MOD_SHIFT: u32 = 1;
const MOD_CTRL: u32 = 2;
const MOD_ALT: u32 = 4;
const MOD_SUPER: u32 = 8; // Cmd on macOS, Win on Windows

/// Convert winit KeyCode to a stable integer value for FFI
/// These values are stable and cross-platform, matching the Go constants
fn keycode_to_u32(key: winit::keyboard::KeyCode) -> u32 {
    use winit::keyboard::KeyCode::*;
    match key {
        // Letters A-Z = 0-25
        KeyA => 0, KeyB => 1, KeyC => 2, KeyD => 3, KeyE => 4,
        KeyF => 5, KeyG => 6, KeyH => 7, KeyI => 8, KeyJ => 9,
        KeyK => 10, KeyL => 11, KeyM => 12, KeyN => 13, KeyO => 14,
        KeyP => 15, KeyQ => 16, KeyR => 17, KeyS => 18, KeyT => 19,
        KeyU => 20, KeyV => 21, KeyW => 22, KeyX => 23, KeyY => 24,
        KeyZ => 25,

        // Numbers 0-9 = 26-35
        Digit0 => 26, Digit1 => 27, Digit2 => 28, Digit3 => 29, Digit4 => 30,
        Digit5 => 31, Digit6 => 32, Digit7 => 33, Digit8 => 34, Digit9 => 35,

        // Function keys F1-F12 = 36-47
        F1 => 36, F2 => 37, F3 => 38, F4 => 39, F5 => 40, F6 => 41,
        F7 => 42, F8 => 43, F9 => 44, F10 => 45, F11 => 46, F12 => 47,

        // Navigation = 48-55
        ArrowUp => 48, ArrowDown => 49, ArrowLeft => 50, ArrowRight => 51,
        Home => 52, End => 53, PageUp => 54, PageDown => 55,

        // Editing = 56-62
        Backspace => 56, Delete => 57, Insert => 58, Enter => 59,
        Tab => 60, Escape => 61, Space => 62,

        // Punctuation = 63-73
        Minus => 63, Equal => 64, BracketLeft => 65, BracketRight => 66,
        Backslash => 67, Semicolon => 68, Quote => 69, Backquote => 70,
        Comma => 71, Period => 72, Slash => 73,

        // Numpad = 100-119
        Numpad0 => 100, Numpad1 => 101, Numpad2 => 102, Numpad3 => 103,
        Numpad4 => 104, Numpad5 => 105, Numpad6 => 106, Numpad7 => 107,
        Numpad8 => 108, Numpad9 => 109,
        NumpadAdd => 110, NumpadSubtract => 111, NumpadMultiply => 112,
        NumpadDivide => 113, NumpadDecimal => 114, NumpadEnter => 115,

        // Modifiers = 200-207 (sent as keycodes, not just modifiers)
        ShiftLeft => 200, ShiftRight => 201,
        ControlLeft => 202, ControlRight => 203,
        AltLeft => 204, AltRight => 205,
        SuperLeft => 206, SuperRight => 207,

        // Other = 300+
        CapsLock => 300, NumLock => 301, ScrollLock => 302,
        PrintScreen => 303, Pause => 304, ContextMenu => 305,

        // Unknown or unmapped keys
        _ => 999,
    }
}

/// Helper to get window size - uses outer_size on iOS for full screen rendering,
/// inner_size on other platforms for safe area rendering.
#[inline]
fn get_window_size(window: &winit::window::Window) -> winit::dpi::PhysicalSize<u32> {
    #[cfg(target_os = "ios")]
    {
        window.outer_size()
    }
    #[cfg(not(target_os = "ios"))]
    {
        window.inner_size()
    }
}

/// Update safe area insets from the window (iOS only, no-op on other platforms)
///
/// This function queries the system's safe area insets and transforms them based on the
/// device orientation. When UIKit hasn't performed its rotation transition (e.g., during
/// manual frame adjustments), the safe area insets reported by the system are in the
/// interface orientation, not the device orientation. We need to rotate them to match
/// the actual device orientation.
#[inline]
fn update_safe_area_from_window(window: &winit::window::Window) {
    #[cfg(target_os = "ios")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        // Get the UIView pointer from raw-window-handle
        let handle = match window.window_handle() {
            Ok(h) => h,
            Err(_) => return,
        };

        let ui_view_ptr = match handle.as_raw() {
            RawWindowHandle::UiKit(uikit_handle) => uikit_handle.ui_view.as_ptr(),
            _ => return,
        };

        if ui_view_ptr.is_null() {
            return;
        }

        // UIEdgeInsets is a struct with { top, left, bottom, right } as CGFloat (f64)
        #[repr(C)]
        struct UIEdgeInsets {
            top: f64,
            left: f64,
            bottom: f64,
            right: f64,
        }

        // UIDeviceOrientation values (from UIDevice.h)
        // Note: We don't handle PortraitUpsideDown (2) because iOS phones don't support it
        const UI_DEVICE_ORIENTATION_PORTRAIT: i64 = 1;
        const UI_DEVICE_ORIENTATION_LANDSCAPE_LEFT: i64 = 3;
        const UI_DEVICE_ORIENTATION_LANDSCAPE_RIGHT: i64 = 4;

        unsafe {
            use objc::{msg_send, sel, sel_impl, runtime::Object, class};

            let ui_view = ui_view_ptr as *mut Object;
            // Get the UIWindow from the UIView
            let ui_window: *mut Object = msg_send![ui_view, window];
            if ui_window.is_null() {
                return;
            }

            let insets: UIEdgeInsets = msg_send![ui_window, safeAreaInsets];

            // Get device orientation to determine if we need to rotate insets
            let ui_device_class = class!(UIDevice);
            let current_device: *mut Object = msg_send![ui_device_class, currentDevice];
            let device_orientation: i64 = msg_send![current_device, orientation];

            // Get window bounds to determine interface orientation
            #[repr(C)]
            struct CGRect {
                origin_x: f64,
                origin_y: f64,
                width: f64,
                height: f64,
            }
            let window_bounds: CGRect = msg_send![ui_window, bounds];
            let interface_is_portrait = window_bounds.height > window_bounds.width;

            let scale_factor = window.scale_factor() as f32;

            // Convert to logical pixels (these are in interface orientation)
            let raw_top = insets.top as f32 / scale_factor;
            let raw_left = insets.left as f32 / scale_factor;
            let raw_bottom = insets.bottom as f32 / scale_factor;
            let raw_right = insets.right as f32 / scale_factor;

            // Determine if we need to transform the insets based on orientation mismatch
            // Note: We only handle regular portrait (not upside-down) because iOS doesn't
            // support upside-down portrait on iPhones
            let device_is_portrait = device_orientation == UI_DEVICE_ORIENTATION_PORTRAIT;
            let device_is_landscape = device_orientation == UI_DEVICE_ORIENTATION_LANDSCAPE_LEFT
                || device_orientation == UI_DEVICE_ORIENTATION_LANDSCAPE_RIGHT;

            // Calculate effective insets based on device orientation
            let (top, left, bottom, right) = if device_is_landscape && interface_is_portrait {
                // Device is landscape but interface is portrait - rotate insets 90°
                if device_orientation == UI_DEVICE_ORIENTATION_LANDSCAPE_LEFT {
                    println!("Safe area: Rotating insets for landscape left (from portrait)");
                    (raw_left, raw_bottom, raw_right, raw_top)
                } else {
                    println!("Safe area: Rotating insets for landscape right (from portrait)");
                    (raw_right, raw_top, raw_left, raw_bottom)
                }
            } else if device_is_portrait && !interface_is_portrait {
                // Device is portrait but interface is landscape - rotate insets back
                println!("Safe area: Rotating insets for portrait (from landscape)");
                (raw_left, raw_bottom, raw_right, raw_top)
            } else {
                // No transformation needed (orientations match or unknown orientation)
                (raw_top, raw_left, raw_bottom, raw_right)
            };

            println!("Safe area insets (logical): top={}, left={}, bottom={}, right={} (device_orientation={}, interface_portrait={})",
                     top, left, bottom, right, device_orientation, interface_is_portrait);
            update_safe_area_insets(top, left, bottom, right);
        }
    }

    #[cfg(not(target_os = "ios"))]
    {
        // Desktop platforms have no unsafe areas
        let _ = window;
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        // Check if we woke up due to a scheduled redraw
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            if let Some(wake_time) = self.next_redraw_at {
                if wake_time <= std::time::Instant::now() {
                    // Time has arrived, request a redraw
                    self.next_redraw_at = None;
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }
        }

        // Linux: Process GTK events and tray icon menu events
        #[cfg(target_os = "linux")]
        {
            // Pump GTK events to allow tray icon to appear and respond
            while gtk::events_pending() {
                gtk::main_iteration();
            }
            tray_icon::process_events();
        }

        // Reset to Wait by default, will be updated by event handlers
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::RequestRedraw => {
                // Directly trigger a redraw with current state
                // This is more reliable than window.request_redraw() which queues
                // an event that might be processed with stale state
                let scale_factor = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
                let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();

                let logical_width = size.width as f64 / scale_factor;
                let logical_height = size.height as f64 / scale_factor;
                let app_event = AppEvent {
                    event_type: AppEventType::RedrawRequested,
                    data1: logical_width,
                    data2: logical_height,
                    scale_factor,
                };

                // Call Go callback and render
                let response = self.call_callback(&app_event);

                // Linux: update window controls theme if dark mode changed
                #[cfg(target_os = "linux")]
                self.update_dark_mode(response.dark_mode);

                // Render frame
                {
                    let backend_lock = get_backend();
                    let mut guard = backend_lock.lock().unwrap();
                    if let Some(ref mut backend) = *guard {
                        let mut all_commands = Vec::new();

                        // Check for layer-based rendering first
                        if let Some(ref json) = response.layers {
                            match serde_json::from_str::<Vec<LayerInfo>>(json) {
                                Ok(layers) => {
                                    // Sort layers by z_order (lower = further back)
                                    let mut sorted_layers = layers;
                                    sorted_layers.sort_by_key(|l| l.z_order);

                                    // Collect commands from all layers in z-order
                                    for layer in &sorted_layers {
                                        all_commands.extend(layer.commands.clone());
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse layers: {}", e);
                                }
                            }
                        } else if let Some(ref json) = response.immediate_commands {
                            // Fall back to immediate commands if no layers
                            match serde_json::from_str::<Vec<RenderCommand>>(json) {
                                Ok(commands) => {
                                    all_commands.extend(commands);
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse immediate commands: {}", e);
                                }
                            }
                        }

                        // Linux frameless window: add rounded corner clipping, window controls, and border
                        #[cfg(target_os = "linux")]
                        {
                            if !all_commands.is_empty() && !self.config.decorations {
                                let window_radius = crate::platform::linux::WINDOW_CORNER_RADIUS;

                                // Extract the background color from Clear command and replace with transparent
                                // This is needed because the render pass clear happens BEFORE stencil clipping,
                                // so we need to draw the background as a rect INSIDE the stencil clip instead.
                                let mut bg_color: Option<crate::style::Color> = None;
                                for cmd in all_commands.iter_mut() {
                                    if let RenderCommand::Clear(color) = cmd {
                                        bg_color = Some(*color);
                                        // Replace with transparent clear
                                        *color = crate::style::Color { r: 0, g: 0, b: 0, a: 0 };
                                        break;
                                    }
                                }

                                // Insert rounded corner clipping at the beginning (after Clear)
                                let rounded_clip = RenderCommand::PushRoundedClip {
                                    x: 0.0,
                                    y: 0.0,
                                    width: logical_width as f32,
                                    height: logical_height as f32,
                                    corner_radii: [window_radius, window_radius, window_radius, window_radius],
                                };

                                // Find the position after Clear command (if any)
                                let insert_pos = all_commands.iter()
                                    .position(|cmd| !matches!(cmd, RenderCommand::Clear(_)))
                                    .unwrap_or(0);
                                all_commands.insert(insert_pos, rounded_clip);

                                // If we had a background color, draw it as a fullscreen rect right after PushRoundedClip
                                // This rect will be clipped to the rounded corners by the stencil
                                if let Some(color) = bg_color {
                                    let bg_rect = RenderCommand::DrawRect {
                                        x: 0.0,
                                        y: 0.0,
                                        width: logical_width as f32,
                                        height: logical_height as f32,
                                        color: ((color.r as u32) << 24) | ((color.g as u32) << 16) | ((color.b as u32) << 8) | (color.a as u32),
                                        corner_radii: [0.0, 0.0, 0.0, 0.0], // No corner radius needed, stencil handles it
                                        rotation: 0.0,
                                        border: None,
                                        gradient: None,
                                    };
                                    // Insert right after the PushRoundedClip
                                    all_commands.insert(insert_pos + 1, bg_rect);
                                }

                                // Add window controls (inside the clipped area)
                                if let Some(ref controls) = self.window_controls {
                                    let control_commands = controls.to_render_commands(logical_width as f32);
                                    all_commands.extend(control_commands);
                                }

                                // End rounded corner clipping before drawing border
                                all_commands.push(RenderCommand::PopClip {});

                                // Add window border (rendered last, on top as outline, outside clip)
                                let is_dark = self.current_dark_mode == 1 ||
                                    (self.current_dark_mode == 2 && crate::platform::linux::is_dark_mode());
                                let border_cmd = crate::platform::linux::window_border_command(
                                    logical_width as f32,
                                    logical_height as f32,
                                    is_dark,
                                );
                                all_commands.push(border_cmd);
                            }
                        }

                        // Windows frameless window: add rounded corner clipping, window controls, and border
                        #[cfg(target_os = "windows")]
                        {
                            if !all_commands.is_empty() && !self.config.decorations {
                                let window_radius = crate::platform::windows::WINDOW_CORNER_RADIUS;

                                // Extract the background color from Clear command and replace with transparent
                                let mut bg_color: Option<crate::style::Color> = None;
                                for cmd in all_commands.iter_mut() {
                                    if let RenderCommand::Clear(color) = cmd {
                                        bg_color = Some(*color);
                                        *color = crate::style::Color { r: 0, g: 0, b: 0, a: 0 };
                                        break;
                                    }
                                }

                                // Insert rounded corner clipping at the beginning (after Clear)
                                let rounded_clip = RenderCommand::PushRoundedClip {
                                    x: 0.0,
                                    y: 0.0,
                                    width: logical_width as f32,
                                    height: logical_height as f32,
                                    corner_radii: [window_radius, window_radius, window_radius, window_radius],
                                };

                                let insert_pos = all_commands.iter()
                                    .position(|cmd| !matches!(cmd, RenderCommand::Clear(_)))
                                    .unwrap_or(0);
                                all_commands.insert(insert_pos, rounded_clip);

                                // Draw background rect inside the stencil clip
                                if let Some(color) = bg_color {
                                    let bg_rect = RenderCommand::DrawRect {
                                        x: 0.0,
                                        y: 0.0,
                                        width: logical_width as f32,
                                        height: logical_height as f32,
                                        color: ((color.r as u32) << 24) | ((color.g as u32) << 16) | ((color.b as u32) << 8) | (color.a as u32),
                                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                                        rotation: 0.0,
                                        border: None,
                                        gradient: None,
                                    };
                                    all_commands.insert(insert_pos + 1, bg_rect);
                                }

                                // Add window controls (inside the clipped area, like Linux)
                                if let Some(ref controls) = self.window_controls {
                                    let control_commands = controls.to_render_commands(logical_width as f32);
                                    all_commands.extend(control_commands);
                                }

                                // End rounded corner clipping
                                all_commands.push(RenderCommand::PopClip {});

                                // Add window border
                                let is_dark = self.current_dark_mode == 1;
                                let border_cmd = crate::platform::windows::window_border_command(
                                    logical_width as f32,
                                    logical_height as f32,
                                    is_dark,
                                );
                                all_commands.push(border_cmd);
                            }
                        }

                        if !all_commands.is_empty() {
                            // Get scissor rect from dirty region (if any)
                            let scissor = response.get_scissor_rect(scale_factor);
                            if let Err(e) = backend.render_frame_with_scissor(&all_commands, scissor) {
                                eprintln!("Render error: {}", e);
                            }
                        }
                    }
                }

                // If response wants continuous redraw, schedule another
                if response.request_redraw {
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }
            UserEvent::Minimize => {
                if let Some(ref window) = self.window {
                    window.set_minimized(true);
                }
            }
            UserEvent::ToggleMaximize => {
                if let Some(ref window) = self.window {
                    let is_maximized = window.is_maximized();
                    window.set_maximized(!is_maximized);
                }
            }
            UserEvent::EnterFullscreen => {
                if let Some(ref window) = self.window {
                    // Use borderless fullscreen on primary monitor
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                }
            }
            UserEvent::ExitFullscreen => {
                if let Some(ref window) = self.window {
                    window.set_fullscreen(None);
                }
            }
            UserEvent::ToggleFullscreen => {
                if let Some(ref window) = self.window {
                    let is_fullscreen = window.fullscreen().is_some();
                    if is_fullscreen {
                        window.set_fullscreen(None);
                    } else {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                }
            }
            UserEvent::Close => {
                self.should_exit = true;
                // The actual exit will be handled in the next event loop iteration
            }
            UserEvent::SetTitle(title) => {
                if let Some(ref window) = self.window {
                    window.set_title(&title);
                }
            }
            #[cfg(target_os = "linux")]
            UserEvent::SystemThemeChanged(is_dark) => {
                // Update window controls based on system theme change
                // Only applies when app is in "auto" mode (dark_mode == 2)
                if self.current_dark_mode == 2 {
                    let new_mode = if is_dark { 1 } else { 0 };
                    if let Some(ref mut controls) = self.window_controls {
                        controls.update_theme_from_app(new_mode);
                    }
                    // Request a redraw to show updated controls
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Get window title
        let title = if self.config.title.is_null() {
            "Centered App".to_string()
        } else {
            unsafe {
                CStr::from_ptr(self.config.title)
                    .to_str()
                    .unwrap_or("Centered App")
                    .to_string()
            }
        };

        // Create window with all config options
        // On Linux, frameless windows need transparency for rounded corners
        #[cfg(target_os = "linux")]
        let needs_transparent = self.config.transparent || !self.config.decorations;
        #[cfg(not(target_os = "linux"))]
        let needs_transparent = self.config.transparent;

        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height))
            .with_decorations(self.config.decorations)
            .with_transparent(needs_transparent)
            .with_resizable(self.config.resizable);

        // Set min/max size constraints if specified
        if self.config.min_width > 0 || self.config.min_height > 0 {
            window_attrs = window_attrs.with_min_inner_size(LogicalSize::new(
                self.config.min_width.max(1),
                self.config.min_height.max(1),
            ));
        }
        if self.config.max_width > 0 || self.config.max_height > 0 {
            window_attrs = window_attrs.with_max_inner_size(LogicalSize::new(
                if self.config.max_width > 0 { self.config.max_width } else { u32::MAX },
                if self.config.max_height > 0 { self.config.max_height } else { u32::MAX },
            ));
        }

        // Set initial position if specified (i32::MIN means use system default)
        if self.config.x != i32::MIN && self.config.y != i32::MIN {
            window_attrs = window_attrs.with_position(winit::dpi::LogicalPosition::new(
                self.config.x,
                self.config.y,
            ));
        }

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create window: {}", e);
                event_loop.exit();
                return;
            }
        };

        // Get physical size and scale factor
        // On iOS, use outer_size for full screen rendering
        let size = get_window_size(&window);
        let scale_factor = window.scale_factor();

        // Initialize wgpu backend
        let mut backend = WgpuBackend::new();
        let config = SurfaceConfig {
            width: size.width,
            height: size.height,
            scale_factor,
            vsync: self.config.vsync,
            low_power_gpu: self.config.low_power_gpu,
            allow_software_fallback: self.config.allow_software_fallback,
        };

        if let Err(e) = pollster::block_on(backend.init_with_window(&window, config)) {
            eprintln!("Failed to initialize backend: {}", e);
            event_loop.exit();
            return;
        }

        // Apply platform-specific window styling for frameless windows
        // This must happen after window creation but before we move the window
        if !self.config.decorations {
            let style_options = WindowStyleOptions {
                corner_radius: self.config.corner_radius,
                show_native_controls: self.config.show_native_controls,
                enable_minimize: self.config.enable_minimize,
                enable_maximize: self.config.enable_maximize,
            };
            if let Err(e) = apply_window_style(&window, style_options) {
                eprintln!("Failed to apply window style: {}", e);
            }
        }

        // Update safe area insets before storing window (iOS only)
        update_safe_area_from_window(&window);

        self.window = Some(window);

        // Store backend in global storage for FFI access (image loading, rendering, etc.)
        // The App will access it through get_backend() instead of self.backend
        {
            let backend_lock = get_backend();
            let mut guard = backend_lock.lock().unwrap();
            *guard = Some(backend);
        }

        // Send Ready event to Go with logical pixels
        let logical_width = size.width as f64 / scale_factor;
        let logical_height = size.height as f64 / scale_factor;
        println!("Window Ready: physical={}x{}, logical={}x{}, scale_factor={}",
            size.width, size.height, logical_width, logical_height, scale_factor);
        let event = AppEvent {
            event_type: AppEventType::Ready,
            data1: logical_width,
            data2: logical_height,
            scale_factor,
        };
        self.call_callback(&event);

        // Request first redraw
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let event = AppEvent {
                    event_type: AppEventType::CloseRequested,
                    data1: 0.0,
                    data2: 0.0,
                    scale_factor: 1.0,
                };
                self.call_callback(&event);
                self.should_exit = true;
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                let scale_factor = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);

                // Update safe area insets (may change on orientation change - iOS)
                if let Some(ref window) = self.window {
                    update_safe_area_from_window(window);
                }

                // Use the size from the event directly. On iOS, this comes from layoutSubviews
                // which has the correct window bounds. Don't use window.outer_size() because
                // it may return stale data during rotation transitions.
                println!("[FFI] WindowEvent::Resized - size: {:?}x{:?}, scale: {}",
                    size.width, size.height, scale_factor);

                // Resize backend (uses physical pixels)
                {
                    let backend_lock = get_backend();
                    let mut guard = backend_lock.lock().unwrap();
                    if let Some(ref mut backend) = *guard {
                        println!("[FFI] Calling backend.resize({}, {}, {})", size.width, size.height, scale_factor);
                        if let Err(e) = backend.resize(size.width, size.height, scale_factor) {
                            eprintln!("Resize error: {}", e);
                        }
                    }
                }

                // Notify Go with LOGICAL pixels (physical / scale_factor)
                // This ensures Go's coordinate system matches mouse events
                let logical_width = size.width as f64 / scale_factor;
                let logical_height = size.height as f64 / scale_factor;
                println!("[FFI] Sending EventResized to Go: logical {}x{}", logical_width, logical_height);
                let event = AppEvent {
                    event_type: AppEventType::Resized,
                    data1: logical_width,
                    data2: logical_height,
                    scale_factor,
                };
                self.call_callback(&event);

                // Linux/Windows: update frameless state in a single lock acquisition
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                {
                    if let Ok(mut state) = get_frameless_state().lock() {
                        // Update scale factor
                        state.scale_factor = scale_factor;

                        // Update maximize button state if we have window controls
                        #[cfg(target_os = "linux")]
                        {
                            if let Some(ref mut controls) = self.window_controls {
                                if let Some(ref window) = self.window {
                                    controls.maximize_state.maximized = window.is_maximized();
                                    state.window_controls = Some(controls.clone());
                                }
                            }
                        }

                        #[cfg(target_os = "windows")]
                        {
                            if let Some(ref mut controls) = self.window_controls {
                                if let Some(ref window) = self.window {
                                    controls.maximize_state.maximized = window.is_maximized();
                                    state.window_controls = Some(controls.clone());
                                }
                            }
                        }
                    }
                }

                // Request redraw after resize
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let scale_factor = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
                let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();

                // Report logical pixels to Go (physical / scale_factor)
                let logical_width = size.width as f64 / scale_factor;
                let logical_height = size.height as f64 / scale_factor;
                let event = AppEvent {
                    event_type: AppEventType::RedrawRequested,
                    data1: logical_width,
                    data2: logical_height,
                    scale_factor,
                };

                // Call Go callback and get response
                let response = self.call_callback(&event);

                // Linux: update window controls theme if dark mode changed
                #[cfg(target_os = "linux")]
                self.update_dark_mode(response.dark_mode);

                // Process retained mode widget delta (if any)
                // TODO: Apply widget_delta to internal widget tree
                // For now, we just acknowledge it
                if let Some(ref _delta_json) = response.widget_delta {
                    // let delta: WidgetDelta = serde_json::from_str(&delta_json)?;
                    // self.widget_tree.apply_delta(delta);
                    // This marks affected widgets dirty for re-render
                }

                // Render frame
                // In hybrid mode, retained widgets render first, then immediate commands on top
                {
                    let backend_lock = get_backend();
                    let mut guard = backend_lock.lock().unwrap();
                    if let Some(ref mut backend) = *guard {
                        let mut all_commands = Vec::new();

                        // Check for layer-based rendering first
                        if let Some(ref json) = response.layers {
                            match serde_json::from_str::<Vec<LayerInfo>>(json) {
                                Ok(layers) => {
                                    // Sort layers by z_order (lower = further back)
                                    let mut sorted_layers = layers;
                                    sorted_layers.sort_by_key(|l| l.z_order);

                                    // Collect commands from all layers in z-order
                                    for layer in &sorted_layers {
                                        all_commands.extend(layer.commands.clone());
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse layers: {}", e);
                                }
                            }
                        } else if let Some(ref json) = response.immediate_commands {
                            // Fall back to immediate commands if no layers
                            match serde_json::from_str::<Vec<RenderCommand>>(json) {
                                Ok(commands) => {
                                    all_commands.extend(commands);
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse immediate commands: {}", e);
                                }
                            }
                        }

                        // Linux frameless window: add rounded corner clipping, window controls, and border
                        // IMPORTANT: Only add if Go sent commands via JSON (not binary path)
                        // If Go used RenderFrameBinary, it already rendered and we'd cause a double-clear
                        #[cfg(target_os = "linux")]
                        {
                            if !all_commands.is_empty() && !self.config.decorations {
                                let window_radius = crate::platform::linux::WINDOW_CORNER_RADIUS;

                                // Extract the background color from Clear command and replace with transparent
                                // This is needed because the render pass clear happens BEFORE stencil clipping,
                                // so we need to draw the background as a rect INSIDE the stencil clip instead.
                                let mut bg_color: Option<crate::style::Color> = None;
                                for cmd in all_commands.iter_mut() {
                                    if let RenderCommand::Clear(color) = cmd {
                                        bg_color = Some(*color);
                                        // Replace with transparent clear
                                        *color = crate::style::Color { r: 0, g: 0, b: 0, a: 0 };
                                        break;
                                    }
                                }

                                // Insert rounded corner clipping at the beginning (after Clear)
                                let rounded_clip = RenderCommand::PushRoundedClip {
                                    x: 0.0,
                                    y: 0.0,
                                    width: logical_width as f32,
                                    height: logical_height as f32,
                                    corner_radii: [window_radius, window_radius, window_radius, window_radius],
                                };

                                // Find the position after Clear command (if any)
                                let insert_pos = all_commands.iter()
                                    .position(|cmd| !matches!(cmd, RenderCommand::Clear(_)))
                                    .unwrap_or(0);
                                all_commands.insert(insert_pos, rounded_clip);

                                // If we had a background color, draw it as a fullscreen rect right after PushRoundedClip
                                // This rect will be clipped to the rounded corners by the stencil
                                if let Some(color) = bg_color {
                                    let bg_rect = RenderCommand::DrawRect {
                                        x: 0.0,
                                        y: 0.0,
                                        width: logical_width as f32,
                                        height: logical_height as f32,
                                        color: ((color.r as u32) << 24) | ((color.g as u32) << 16) | ((color.b as u32) << 8) | (color.a as u32),
                                        corner_radii: [0.0, 0.0, 0.0, 0.0], // No corner radius needed, stencil handles it
                                        rotation: 0.0,
                                        border: None,
                                        gradient: None,
                                    };
                                    // Insert right after the PushRoundedClip
                                    all_commands.insert(insert_pos + 1, bg_rect);
                                }

                                // Add window controls (inside the clipped area)
                                if let Some(ref controls) = self.window_controls {
                                    let control_commands = controls.to_render_commands(logical_width as f32);
                                    all_commands.extend(control_commands);
                                }

                                // End rounded corner clipping before drawing border
                                all_commands.push(RenderCommand::PopClip {});

                                // Add window border (rendered last, on top as outline, outside clip)
                                let is_dark = self.current_dark_mode == 1 ||
                                    (self.current_dark_mode == 2 && crate::platform::linux::is_dark_mode());
                                let border_cmd = crate::platform::linux::window_border_command(
                                    logical_width as f32,
                                    logical_height as f32,
                                    is_dark,
                                );
                                all_commands.push(border_cmd);
                            }
                        }

                        // Windows frameless window: add rounded corner clipping, window controls, and border
                        // Only process if Go sent commands - if empty, Go rendered via different path
                        #[cfg(target_os = "windows")]
                        {
                            if !all_commands.is_empty() && !self.config.decorations {
                                let window_radius = crate::platform::windows::WINDOW_CORNER_RADIUS;

                                let mut bg_color: Option<crate::style::Color> = None;
                                for cmd in all_commands.iter_mut() {
                                    if let RenderCommand::Clear(color) = cmd {
                                        bg_color = Some(*color);
                                        *color = crate::style::Color { r: 0, g: 0, b: 0, a: 0 };
                                        break;
                                    }
                                }

                                let rounded_clip = RenderCommand::PushRoundedClip {
                                    x: 0.0,
                                    y: 0.0,
                                    width: logical_width as f32,
                                    height: logical_height as f32,
                                    corner_radii: [window_radius, window_radius, window_radius, window_radius],
                                };

                                let insert_pos = all_commands.iter()
                                    .position(|cmd| !matches!(cmd, RenderCommand::Clear(_)))
                                    .unwrap_or(0);
                                all_commands.insert(insert_pos, rounded_clip);

                                if let Some(color) = bg_color {
                                    let bg_rect = RenderCommand::DrawRect {
                                        x: 0.0,
                                        y: 0.0,
                                        width: logical_width as f32,
                                        height: logical_height as f32,
                                        color: ((color.r as u32) << 24) | ((color.g as u32) << 16) | ((color.b as u32) << 8) | (color.a as u32),
                                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                                        rotation: 0.0,
                                        border: None,
                                        gradient: None,
                                    };
                                    all_commands.insert(insert_pos + 1, bg_rect);
                                }

                                // Add window controls (inside the clipped area, like Linux)
                                if let Some(ref controls) = self.window_controls {
                                    let control_commands = controls.to_render_commands(logical_width as f32);
                                    all_commands.extend(control_commands);
                                }

                                all_commands.push(RenderCommand::PopClip {});

                                let is_dark = self.current_dark_mode == 1;
                                let border_cmd = crate::platform::windows::window_border_command(
                                    logical_width as f32,
                                    logical_height as f32,
                                    is_dark,
                                );
                                all_commands.push(border_cmd);
                            }
                        }

                        // Execute all commands
                        if !all_commands.is_empty() {
                            // Get scissor rect from dirty region (if any)
                            let scissor = response.get_scissor_rect(scale_factor);
                            if let Err(e) = backend.render_frame_with_scissor(&all_commands, scissor) {
                                eprintln!("Render error: {}", e);
                            }
                        }
                    }
                }

                // Handle redraw scheduling
                self.update_scheduled_redraw(&response);

                if response.request_redraw {
                    // Immediate redraw requested (animations, scrolling, etc.)
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                    // Clear scheduled redraw since we're doing immediate
                    self.next_redraw_at = None;
                    event_loop.set_control_flow(ControlFlow::Poll);
                } else if let Some(wake_time) = self.next_redraw_at {
                    // Delayed redraw scheduled (cursor blink, etc.)
                    if wake_time <= std::time::Instant::now() {
                        // Time already passed, request immediate redraw
                        if let Some(ref window) = self.window {
                            window.request_redraw();
                        }
                        self.next_redraw_at = None;
                    } else {
                        // Schedule wakeup at the specified time
                        event_loop.set_control_flow(ControlFlow::WaitUntil(wake_time));
                    }
                } else {
                    // No redraw needed, wait for events
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let scale_factor = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
                // Convert to logical pixels to match our coordinate system
                let logical_x = position.x / scale_factor;
                let logical_y = position.y / scale_factor;

                // Linux: track mouse position for window controls and resize
                #[cfg(target_os = "linux")]
                {
                    self.mouse_position = (logical_x, logical_y);

                    // Update window control hover states
                    if let Some(ref mut controls) = self.window_controls {
                        let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();
                        let window_width = size.width as f32 / scale_factor as f32;
                        if controls.update_hover(logical_x as f32, logical_y as f32, window_width) {
                            // Sync to global frameless state for batch rendering
                            if let Ok(mut state) = get_frameless_state().lock() {
                                state.window_controls = Some(controls.clone());
                            }
                            // Hover state changed, request redraw
                            if let Some(ref window) = self.window {
                                window.request_redraw();
                            }
                        }
                    }

                    // Update cursor for resize edges on frameless windows
                    if !self.config.decorations && self.config.resizable {
                        use crate::platform::linux::window_controls::{detect_resize_edge, HEADER_HEIGHT};
                        let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();
                        let window_width = size.width as f32 / scale_factor as f32;
                        let window_height = size.height as f32 / scale_factor as f32;

                        // Don't show resize cursor in header area (where controls are)
                        let edge = if logical_y as f32 > HEADER_HEIGHT || self.window_controls.is_none() {
                            detect_resize_edge(logical_x as f32, logical_y as f32, window_width, window_height)
                        } else {
                            None
                        };

                        // Update cursor based on edge
                        if let Some(ref window) = self.window {
                            use winit::window::CursorIcon;
                            let cursor = match edge {
                                Some(crate::platform::linux::window_controls::ResizeEdge::Top) |
                                Some(crate::platform::linux::window_controls::ResizeEdge::Bottom) => CursorIcon::NsResize,
                                Some(crate::platform::linux::window_controls::ResizeEdge::Left) |
                                Some(crate::platform::linux::window_controls::ResizeEdge::Right) => CursorIcon::EwResize,
                                Some(crate::platform::linux::window_controls::ResizeEdge::TopLeft) |
                                Some(crate::platform::linux::window_controls::ResizeEdge::BottomRight) => CursorIcon::NwseResize,
                                Some(crate::platform::linux::window_controls::ResizeEdge::TopRight) |
                                Some(crate::platform::linux::window_controls::ResizeEdge::BottomLeft) => CursorIcon::NeswResize,
                                None => CursorIcon::Default,
                            };
                            window.set_cursor(cursor);
                        }

                        self.resize_direction = edge.map(|e| e.to_resize_direction());
                    }
                }

                // Windows: track mouse position for window controls and resize
                #[cfg(target_os = "windows")]
                {
                    self.mouse_position = (logical_x, logical_y);

                    // Update window control hover states
                    if let Some(ref mut controls) = self.window_controls {
                        let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();
                        let window_width = size.width as f32 / scale_factor as f32;
                        if controls.update_hover(logical_x as f32, logical_y as f32, window_width) {
                            // Sync to global frameless state for batch rendering
                            if let Ok(mut state) = get_frameless_state().lock() {
                                state.window_controls = Some(controls.clone());
                            }
                            if let Some(ref window) = self.window {
                                window.request_redraw();
                            }
                        }
                    }

                    // Update cursor for resize edges on frameless windows
                    if !self.config.decorations && self.config.resizable {
                        use crate::platform::windows::window_controls::{detect_resize_edge, HEADER_HEIGHT};
                        let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();
                        let window_width = size.width as f32 / scale_factor as f32;
                        let window_height = size.height as f32 / scale_factor as f32;

                        let edge = if logical_y as f32 > HEADER_HEIGHT || self.window_controls.is_none() {
                            detect_resize_edge(logical_x as f32, logical_y as f32, window_width, window_height)
                        } else {
                            None
                        };

                        if let Some(ref window) = self.window {
                            use winit::window::CursorIcon;
                            let cursor = match edge {
                                Some(crate::platform::windows::window_controls::ResizeEdge::Top) |
                                Some(crate::platform::windows::window_controls::ResizeEdge::Bottom) => CursorIcon::NsResize,
                                Some(crate::platform::windows::window_controls::ResizeEdge::Left) |
                                Some(crate::platform::windows::window_controls::ResizeEdge::Right) => CursorIcon::EwResize,
                                Some(crate::platform::windows::window_controls::ResizeEdge::TopLeft) |
                                Some(crate::platform::windows::window_controls::ResizeEdge::BottomRight) => CursorIcon::NwseResize,
                                Some(crate::platform::windows::window_controls::ResizeEdge::TopRight) |
                                Some(crate::platform::windows::window_controls::ResizeEdge::BottomLeft) => CursorIcon::NeswResize,
                                None => CursorIcon::Default,
                            };
                            window.set_cursor(cursor);
                        }

                        self.resize_direction = edge.map(|e| e.to_resize_direction());
                    }
                }

                let event = AppEvent {
                    event_type: AppEventType::MouseMoved,
                    data1: logical_x,
                    data2: logical_y,
                    scale_factor,
                };
                let response = self.call_callback(&event);
                // Input events can trigger state changes that need redraw
                if response.request_redraw {
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::CursorLeft { .. } => {
                // Linux: clear hover states when cursor leaves window
                #[cfg(target_os = "linux")]
                {
                    if let Some(ref mut controls) = self.window_controls {
                        controls.clear_hover();
                        // Sync to global frameless state for batch rendering
                        if let Ok(mut state) = get_frameless_state().lock() {
                            state.window_controls = Some(controls.clone());
                        }
                        if let Some(ref window) = self.window {
                            window.request_redraw();
                        }
                    }
                }
                // Windows: clear hover states when cursor leaves window
                #[cfg(target_os = "windows")]
                {
                    if let Some(ref mut controls) = self.window_controls {
                        controls.clear_hover();
                        // Sync to global frameless state for batch rendering
                        if let Ok(mut state) = get_frameless_state().lock() {
                            state.window_controls = Some(controls.clone());
                        }
                        if let Some(ref window) = self.window {
                            window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // Linux: handle window control clicks and resize
                #[cfg(target_os = "linux")]
                {
                    if button == winit::event::MouseButton::Left && state == ElementState::Pressed {
                        let scale_factor = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
                        let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();
                        let window_width = size.width as f32 / scale_factor as f32;

                        // Check for window control button clicks
                        if let Some(ref controls) = self.window_controls {
                            use crate::platform::linux::window_controls::ButtonKind;
                            if let Some(kind) = controls.hit_test(self.mouse_position.0 as f32, self.mouse_position.1 as f32, window_width) {
                                match kind {
                                    ButtonKind::Close => {
                                        // Send close event to Go callback
                                        let close_event = AppEvent {
                                            event_type: AppEventType::CloseRequested,
                                            data1: 0.0,
                                            data2: 0.0,
                                            scale_factor: 1.0,
                                        };
                                        let _ = self.call_callback(&close_event);
                                        self.should_exit = true;
                                        event_loop.exit();
                                        return;
                                    }
                                    ButtonKind::Minimize => {
                                        if let Some(ref window) = self.window {
                                            window.set_minimized(true);
                                        }
                                        return; // Don't pass to Go
                                    }
                                    ButtonKind::Maximize => {
                                        if let Some(ref window) = self.window {
                                            if window.is_maximized() {
                                                window.set_maximized(false);
                                            } else {
                                                window.set_maximized(true);
                                            }
                                            // Update maximized state in controls
                                            if let Some(ref mut controls) = self.window_controls {
                                                controls.maximize_state.maximized = window.is_maximized();
                                                // Sync to global frameless state for batch rendering
                                                if let Ok(mut state) = get_frameless_state().lock() {
                                                    state.window_controls = Some(controls.clone());
                                                }
                                            }
                                            window.request_redraw();
                                        }
                                        return; // Don't pass to Go
                                    }
                                }
                            }
                        }

                        // Check for resize edge drag
                        if let Some(direction) = self.resize_direction {
                            if let Some(ref window) = self.window {
                                let _ = window.drag_resize_window(direction);
                            }
                            return; // Don't pass to Go
                        }

                        // Check for title bar drag (header area, excluding buttons)
                        if !self.config.decorations {
                            use crate::platform::linux::window_controls::HEADER_HEIGHT;
                            let (mx, my) = self.mouse_position;
                            if my < HEADER_HEIGHT as f64 {
                                // In header area - check if not on a button
                                let on_button = self.window_controls.as_ref()
                                    .map(|c| c.hit_test(mx as f32, my as f32, window_width).is_some())
                                    .unwrap_or(false);
                                if !on_button {
                                    if let Some(ref window) = self.window {
                                        let _ = window.drag_window();
                                    }
                                    return; // Don't pass to Go
                                }
                            }
                        }
                    }
                }

                // Windows: handle window control clicks and resize
                #[cfg(target_os = "windows")]
                {
                    if button == winit::event::MouseButton::Left && state == ElementState::Pressed {
                        let scale_factor = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
                        let size = self.window.as_ref().map(|w| get_window_size(w)).unwrap_or_default();
                        let window_width = size.width as f32 / scale_factor as f32;

                        // Check for window control button clicks
                        if let Some(ref controls) = self.window_controls {
                            use crate::platform::windows::window_controls::ButtonKind;
                            if let Some(kind) = controls.hit_test(self.mouse_position.0 as f32, self.mouse_position.1 as f32, window_width) {
                                match kind {
                                    ButtonKind::Close => {
                                        let close_event = AppEvent {
                                            event_type: AppEventType::CloseRequested,
                                            data1: 0.0,
                                            data2: 0.0,
                                            scale_factor: 1.0,
                                        };
                                        let _ = self.call_callback(&close_event);
                                        self.should_exit = true;
                                        event_loop.exit();
                                        return;
                                    }
                                    ButtonKind::Minimize => {
                                        if let Some(ref window) = self.window {
                                            window.set_minimized(true);
                                        }
                                        return;
                                    }
                                    ButtonKind::Maximize => {
                                        if let Some(ref window) = self.window {
                                            if window.is_maximized() {
                                                window.set_maximized(false);
                                            } else {
                                                window.set_maximized(true);
                                            }
                                            if let Some(ref mut controls) = self.window_controls {
                                                controls.maximize_state.maximized = window.is_maximized();
                                                // Sync to global frameless state for batch rendering
                                                if let Ok(mut state) = get_frameless_state().lock() {
                                                    state.window_controls = Some(controls.clone());
                                                }
                                            }
                                            window.request_redraw();
                                        }
                                        return;
                                    }
                                }
                            }
                        }

                        // Check for resize edge drag
                        if let Some(direction) = self.resize_direction {
                            if let Some(ref window) = self.window {
                                let _ = window.drag_resize_window(direction);
                            }
                            return;
                        }

                        // Check for title bar drag (header area, excluding buttons)
                        if !self.config.decorations {
                            use crate::platform::windows::window_controls::HEADER_HEIGHT;
                            let (mx, my) = self.mouse_position;
                            if my < HEADER_HEIGHT as f64 {
                                let on_button = self.window_controls.as_ref()
                                    .map(|c| c.hit_test(mx as f32, my as f32, window_width).is_some())
                                    .unwrap_or(false);
                                if !on_button {
                                    if let Some(ref window) = self.window {
                                        let _ = window.drag_window();
                                    }
                                    return;
                                }
                            }
                        }
                    }
                }

                let event_type = match state {
                    ElementState::Pressed => AppEventType::MousePressed,
                    ElementState::Released => AppEventType::MouseReleased,
                };
                let button_idx = match button {
                    winit::event::MouseButton::Left => 0.0,
                    winit::event::MouseButton::Right => 1.0,
                    winit::event::MouseButton::Middle => 2.0,
                    winit::event::MouseButton::Back => 3.0,
                    winit::event::MouseButton::Forward => 4.0,
                    winit::event::MouseButton::Other(n) => n as f64,
                };
                let event = AppEvent {
                    event_type,
                    data1: button_idx,
                    data2: 0.0,
                    scale_factor: 1.0,
                };
                let response = self.call_callback(&event);
                // Click events often trigger hover/active state animations
                if response.request_redraw {
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (mut dx, mut dy, is_line_delta) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x as f64 * 20.0, y as f64 * 20.0, true),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y, false),
                };

                // On Linux, winit gives us "natural" scroll deltas.
                // Flip to traditional if the user has natural scrolling disabled.
                #[cfg(target_os = "linux")]
                {
                    use crate::platform::linux::is_natural_scrolling;
                    if !is_natural_scrolling() {
                        dx = -dx;
                        dy = -dy;
                    }
                }

                // On Windows, winit gives us traditional scroll deltas (opposite of macOS).
                // We flip to match macOS convention (natural scrolling), checking the
                // appropriate setting based on input device type:
                // - LineDelta = mouse wheel (discrete notches) -> check FlipFlopWheel
                // - PixelDelta = touchpad (smooth scroll) -> check ScrollDirection
                #[cfg(target_os = "windows")]
                {
                    use crate::platform::windows::{is_mouse_natural_scrolling, is_touchpad_natural_scrolling};
                    let is_natural = if is_line_delta {
                        is_mouse_natural_scrolling()
                    } else {
                        is_touchpad_natural_scrolling()
                    };
                    if !is_natural {
                        // Windows default: flip to match macOS natural scrolling convention
                        dy = -dy;
                    }
                }

                // Suppress unused variable warning on non-Windows platforms
                let _ = is_line_delta;

                let event = AppEvent {
                    event_type: AppEventType::MouseWheel,
                    data1: dx,
                    data2: dy,
                    scale_factor: 1.0,
                };
                let response = self.call_callback(&event);
                // Scroll typically needs immediate redraw
                if response.request_redraw {
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let event_type = match event.state {
                    ElementState::Pressed => AppEventType::KeyPressed,
                    ElementState::Released => AppEventType::KeyReleased,
                };
                // Convert physical keycode to stable cross-platform identifier
                let keycode = match event.physical_key {
                    winit::keyboard::PhysicalKey::Code(code) => keycode_to_u32(code) as f64,
                    _ => 999.0, // Unknown key
                };

                // Pack modifier flags into data2
                let mut mods: u32 = 0;
                if self.modifiers.shift_key() {
                    mods |= MOD_SHIFT;
                }
                if self.modifiers.control_key() {
                    mods |= MOD_CTRL;
                }
                if self.modifiers.alt_key() {
                    mods |= MOD_ALT;
                }
                if self.modifiers.super_key() {
                    mods |= MOD_SUPER;
                }

                let app_event = AppEvent {
                    event_type,
                    data1: keycode,
                    data2: mods as f64,
                    scale_factor: 1.0,
                };
                let response = self.call_callback(&app_event);

                // Also send character input if it's a press with text
                if event.state == ElementState::Pressed {
                    if let Some(text) = &event.text {
                        for c in text.chars() {
                            let char_event = AppEvent {
                                event_type: AppEventType::CharInput,
                                data1: c as u32 as f64,
                                data2: mods as f64, // Include modifiers for char input too
                                scale_factor: 1.0,
                            };
                            self.call_callback(&char_event);
                        }
                    }
                }

                if response.request_redraw {
                    if let Some(ref window) = self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::Touch(touch) => {
                let scale_factor = self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor())
                    .unwrap_or(1.0);

                println!(
                    "[FFI] Touch event: phase={:?}, location=({:.1}, {:.1}), scale={:.1}",
                    touch.phase, touch.location.x, touch.location.y, scale_factor
                );

                match touch.phase {
                    winit::event::TouchPhase::Started => {
                        // First send mouse move to update position (touch includes location)
                        let move_event = AppEvent {
                            event_type: AppEventType::MouseMoved,
                            data1: touch.location.x / scale_factor,
                            data2: touch.location.y / scale_factor,
                            scale_factor,
                        };
                        self.call_callback(&move_event);

                        // Then send mouse press (like left click)
                        let press_event = AppEvent {
                            event_type: AppEventType::MousePressed,
                            data1: 0.0, // Button 0 = left mouse button
                            data2: 0.0,
                            scale_factor,
                        };
                        let response = self.call_callback(&press_event);
                        if response.request_redraw {
                            if let Some(ref window) = self.window {
                                window.request_redraw();
                            }
                        }
                    }
                    winit::event::TouchPhase::Moved => {
                        // Convert touch move to mouse move (for drag support)
                        let event = AppEvent {
                            event_type: AppEventType::MouseMoved,
                            data1: touch.location.x / scale_factor,
                            data2: touch.location.y / scale_factor,
                            scale_factor,
                        };
                        let response = self.call_callback(&event);
                        if response.request_redraw {
                            if let Some(ref window) = self.window {
                                window.request_redraw();
                            }
                        }
                    }
                    winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                        // Send final position update
                        let move_event = AppEvent {
                            event_type: AppEventType::MouseMoved,
                            data1: touch.location.x / scale_factor,
                            data2: touch.location.y / scale_factor,
                            scale_factor,
                        };
                        self.call_callback(&move_event);

                        // Then send mouse release
                        let release_event = AppEvent {
                            event_type: AppEventType::MouseReleased,
                            data1: 0.0, // Button 0 = left mouse button
                            data2: 0.0,
                            scale_factor,
                        };
                        let response = self.call_callback(&release_event);
                        if response.request_redraw {
                            if let Some(ref window) = self.window {
                                window.request_redraw();
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }
}

/// Processed response from callback
struct ProcessedResponse {
    immediate_commands: Option<String>,
    widget_delta: Option<String>,
    request_redraw: bool,
    redraw_after_ms: u32,
    dark_mode: u8,
    layers: Option<String>,
    dirty_region: Option<String>,
}

impl ProcessedResponse {
    /// Parse dirty_region JSON and convert to physical pixel scissor rect.
    /// Returns None if no dirty region (meaning full redraw).
    fn get_scissor_rect(&self, scale_factor: f64) -> Option<(u32, u32, u32, u32)> {
        let json = self.dirty_region.as_ref()?;
        match serde_json::from_str::<DirtyRegion>(json) {
            Ok(region) => {
                // Convert logical pixels to physical pixels
                let x = (region.x as f64 * scale_factor) as u32;
                let y = (region.y as f64 * scale_factor) as u32;
                let w = (region.width as f64 * scale_factor).ceil() as u32;
                let h = (region.height as f64 * scale_factor).ceil() as u32;
                Some((x, y, w.max(1), h.max(1)))
            }
            Err(e) => {
                eprintln!("Failed to parse dirty_region: {}", e);
                None
            }
        }
    }
}

impl App {
    fn call_callback(&self, event: &AppEvent) -> ProcessedResponse {
        // Create response struct for callback to fill
        let mut response = FrameResponse {
            immediate_commands: ptr::null_mut(),
            widget_delta: ptr::null_mut(),
            request_redraw: false,
            redraw_after_ms: 0,
            dark_mode: 2, // Default to auto/system
            layers: ptr::null_mut(),
            dirty_region: ptr::null_mut(),
        };

        // Call the Go callback
        unsafe {
            (self.callback)(
                event as *const AppEvent,
                &mut response as *mut FrameResponse,
                self.user_data,
            );
        }

        // Process the response - read strings WITHOUT taking ownership
        // Go allocated this memory, so we must NOT free it
        let immediate_commands = if response.immediate_commands.is_null() {
            None
        } else {
            let c_str = unsafe { CStr::from_ptr(response.immediate_commands) };
            c_str.to_str().ok().map(String::from)
        };

        let widget_delta = if response.widget_delta.is_null() {
            None
        } else {
            let c_str = unsafe { CStr::from_ptr(response.widget_delta) };
            c_str.to_str().ok().map(String::from)
        };

        let layers = if response.layers.is_null() {
            None
        } else {
            let c_str = unsafe { CStr::from_ptr(response.layers) };
            c_str.to_str().ok().map(String::from)
        };

        let dirty_region = if response.dirty_region.is_null() {
            None
        } else {
            let c_str = unsafe { CStr::from_ptr(response.dirty_region) };
            c_str.to_str().ok().map(String::from)
        };

        ProcessedResponse {
            immediate_commands,
            widget_delta,
            request_redraw: response.request_redraw,
            redraw_after_ms: response.redraw_after_ms,
            dark_mode: response.dark_mode,
            layers,
            dirty_region,
        }
    }

    /// Update window controls theme if dark mode changed (Linux only)
    #[cfg(target_os = "linux")]
    fn update_dark_mode(&mut self, new_dark_mode: u8) {
        if new_dark_mode != self.current_dark_mode {
            self.current_dark_mode = new_dark_mode;
            if let Some(ref mut controls) = self.window_controls {
                controls.update_theme_from_app(new_dark_mode);
            }
        }
    }

    /// Update window controls theme if dark mode changed (Windows)
    #[cfg(target_os = "windows")]
    fn update_dark_mode(&mut self, new_dark_mode: u8) {
        if new_dark_mode != self.current_dark_mode {
            self.current_dark_mode = new_dark_mode;
            if let Some(ref mut controls) = self.window_controls {
                controls.update_theme_from_app(new_dark_mode);
            }
        }
    }

    /// Update the scheduled redraw time based on response
    fn update_scheduled_redraw(&mut self, response: &ProcessedResponse) {
        if response.redraw_after_ms > 0 {
            let new_time = std::time::Instant::now()
                + std::time::Duration::from_millis(response.redraw_after_ms as u64);
            // Keep the earliest scheduled time
            self.next_redraw_at = Some(match self.next_redraw_at {
                Some(existing) if existing < new_time => existing,
                _ => new_time,
            });
        }
    }
}

/// Run the application with Rust-owned window
///
/// This function does not return until the application exits.
/// Rust owns the window and event loop; Go provides a callback for events.
///
/// # Arguments
/// * `config` - Application configuration
/// * `callback` - Function called for each event, returns render commands JSON
///
/// # Returns
/// 0 on success, negative error code on failure
///
/// # Safety
/// - config.title must be a valid null-terminated UTF-8 string (or null for default)
/// - callback must be a valid function pointer
/// - user_data lifetime must exceed the application run
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_app_run(
    config: *const AppConfig,
    callback: AppCallback,
) -> i32 {
    if config.is_null() {
        return -1;
    }

    let config = &*config;

    // On iOS, use our native UIKit backend instead of winit
    #[cfg(target_os = "ios")]
    {
        return run_ios_app(config, callback);
    }

    // On Android, use our native android-activity backend
    #[cfg(target_os = "android")]
    {
        return run_android_app(config, callback);
    }

    // On desktop platforms, use winit
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        return run_winit_app(config, callback);
    }
}

/// iOS main entry point - call this from main.m instead of UIApplicationMain
/// This allows Rust to own the entire iOS app lifecycle.
///
/// Usage in main.m:
/// ```objc
/// extern int centered_ios_main(int argc, char * argv[]);
/// int main(int argc, char * argv[]) {
///     return centered_ios_main(argc, argv);
/// }
/// ```
#[cfg(target_os = "ios")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_ios_main(argc: i32, argv: *mut *mut i8) -> i32 {
    println!("[FFI] centered_ios_main called");

    // CRITICAL: Register the AppDelegate class with the ObjC runtime BEFORE
    // calling UIApplicationMain. Without this, UIApplicationMain can't find
    // the class by name and will crash.
    crate::platform::ios::register_app_delegate_class();

    // Flow:
    // 1. main.m (or Go) calls centered_ios_main
    // 2. We register CenteredAppDelegate with the ObjC runtime
    // 3. UIApplicationMain starts with our delegate
    // 4. AppDelegate.didFinishLaunching creates the window/view
    // 5. It calls Go's ready callback (registered via centered_ios_set_ready_callback)
    // 6. Go registers its event handler via centered_app_run

    unsafe {
        extern "C" {
            fn UIApplicationMain(
                argc: i32,
                argv: *mut *mut i8,
                principal_class_name: *const objc2::runtime::AnyObject,
                delegate_class_name: *const objc2::runtime::AnyObject,
            ) -> i32;
        }

        let delegate_class = objc2_foundation::NSString::from_str("CenteredAppDelegate");

        println!("[FFI] Calling UIApplicationMain with delegate: CenteredAppDelegate");

        UIApplicationMain(
            argc,
            argv,
            std::ptr::null(),
            &*delegate_class as *const _ as *const objc2::runtime::AnyObject,
        )
    }
}

/// iOS-specific app runner using native UIKit
/// On iOS, this is called from Go's ready handler (after UIApplicationMain is already running).
/// It just registers the callback - the event loop is already running.
#[cfg(target_os = "ios")]
unsafe fn run_ios_app(config: &AppConfig, callback: AppCallback) -> i32 {
    use crate::platform::ios::{register_callback, set_target_fps};
    use crate::platform::backend::{PlatformEvent, EventResponse};

    // Set the target FPS from config (default 60 if not specified or 0)
    let fps = if config.target_fps == 0 { 60 } else { config.target_fps };
    set_target_fps(fps);

    println!("[FFI] run_ios_app: registering callback (target FPS: {})", fps);

    // Wrap the C callback in a Rust closure that translates events
    let user_data = config.user_data;
    let c_callback = callback;

    let rust_callback = move |event: PlatformEvent| -> EventResponse {
        // Translate PlatformEvent to AppEvent
        let app_event = match event {
            PlatformEvent::Ready { width, height, scale_factor } => AppEvent {
                event_type: AppEventType::Ready,
                data1: width,
                data2: height,
                scale_factor,
            },
            PlatformEvent::RedrawRequested => AppEvent {
                event_type: AppEventType::RedrawRequested,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::Resized { width, height, scale_factor } => AppEvent {
                event_type: AppEventType::Resized,
                data1: width,
                data2: height,
                scale_factor,
            },
            PlatformEvent::CloseRequested => AppEvent {
                event_type: AppEventType::CloseRequested,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchBegan { id: _, x, y } => AppEvent {
                event_type: AppEventType::MousePressed,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchMoved { id: _, x, y } => AppEvent {
                event_type: AppEventType::MouseMoved,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchEnded { id: _, x, y } => AppEvent {
                event_type: AppEventType::MouseReleased,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchCancelled { id: _, x, y } => AppEvent {
                event_type: AppEventType::MouseReleased,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::Resumed => AppEvent {
                event_type: AppEventType::Resumed,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::Suspended => AppEvent {
                event_type: AppEventType::Suspended,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::KeyPressed { keycode, modifiers } => AppEvent {
                event_type: AppEventType::KeyPressed,
                data1: keycode as f64,
                data2: modifiers as f64,
                scale_factor: 1.0,
            },
            PlatformEvent::KeyReleased { keycode, modifiers } => AppEvent {
                event_type: AppEventType::KeyReleased,
                data1: keycode as f64,
                data2: modifiers as f64,
                scale_factor: 1.0,
            },
            PlatformEvent::TextInput { text } => {
                // Send each character as a CharInput event
                for c in text.chars() {
                    let char_event = AppEvent {
                        event_type: AppEventType::CharInput,
                        data1: c as u32 as f64,
                        data2: 0.0, // no modifiers for text input
                        scale_factor: 1.0,
                    };
                    let mut temp_response = FrameResponse {
                        immediate_commands: std::ptr::null_mut(),
                        widget_delta: std::ptr::null_mut(),
                        request_redraw: false,
                        redraw_after_ms: 0,
                        dark_mode: 2,
                        layers: std::ptr::null_mut(),
                    };
                    c_callback(&char_event, &mut temp_response, user_data);
                }
                return EventResponse::default();
            },
            PlatformEvent::Scroll { dx, dy } => AppEvent {
                event_type: AppEventType::MouseWheel,
                data1: dx,
                data2: dy,
                scale_factor: 1.0,
            },
            PlatformEvent::KeyboardFrameChanged { height, animation_duration } => AppEvent {
                event_type: AppEventType::KeyboardFrameChanged,
                data1: height,
                data2: animation_duration,
                scale_factor: 1.0,
            },
            _ => return EventResponse::default(),
        };

        // Call the C callback
        let mut frame_response = FrameResponse {
            immediate_commands: std::ptr::null_mut(),
            widget_delta: std::ptr::null_mut(),
            request_redraw: false,
            redraw_after_ms: 0,
            dark_mode: 2,
            layers: std::ptr::null_mut(),
        };

        c_callback(&app_event, &mut frame_response, user_data);

        // iOS-specific: Process immediate commands if provided
        // In retained mode, commands are only sent when widgets are dirty
        if !frame_response.immediate_commands.is_null() {
            let json_str = match std::ffi::CStr::from_ptr(frame_response.immediate_commands).to_str() {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("[iOS] Invalid UTF-8 in immediate commands");
                    return EventResponse {
                        request_redraw: frame_response.request_redraw,
                        exit: false,
                        redraw_after_ms: frame_response.redraw_after_ms,
                    };
                }
            };

            // Skip rendering if JSON is empty array "[]"
            if json_str.len() > 2 {
                let commands: Vec<RenderCommand> = match serde_json::from_str(json_str) {
                    Ok(cmds) => cmds,
                    Err(e) => {
                        eprintln!("[iOS] Failed to parse render commands: {}", e);
                        return EventResponse {
                            request_redraw: frame_response.request_redraw,
                            exit: false,
                            redraw_after_ms: frame_response.redraw_after_ms,
                        };
                    }
                };

                if !commands.is_empty() {
                    if let Err(e) = crate::platform::ios::render_frame(&commands) {
                        eprintln!("[iOS] Render error: {}", e);
                    }
                }
            }
        }

        EventResponse {
            request_redraw: frame_response.request_redraw,
            exit: false, // FrameResponse doesn't have exit
            redraw_after_ms: frame_response.redraw_after_ms,
        }
    };

    // On iOS, just register the callback - UIApplicationMain is already running
    register_callback(Box::new(rust_callback));

    println!("[FFI] run_ios_app: callback registered, returning");

    // Return 0 - the event loop is already running via UIApplicationMain
    // This function returns immediately on iOS (unlike desktop where it blocks)
    0
}

/// Android app runner using android-activity
/// On Android, this is called from Go's ready handler (after the native activity is running).
/// Similar to iOS - it registers the callback, the event loop is already managed by android-activity.
#[cfg(target_os = "android")]
unsafe fn run_android_app(config: &AppConfig, callback: AppCallback) -> i32 {
    use crate::platform::android::{register_callback, set_target_fps};
    use crate::platform::backend::{PlatformEvent, EventResponse};

    // Set the target FPS from config (default 60 if not specified or 0)
    let fps = if config.target_fps == 0 { 60 } else { config.target_fps };
    set_target_fps(fps);

    log::info!("[FFI] run_android_app: registering callback (target FPS: {})", fps);

    // Wrap the C callback in a Rust closure that translates events
    let user_data = config.user_data;
    let c_callback = callback;

    let rust_callback = move |event: PlatformEvent| -> EventResponse {
        // Translate PlatformEvent to AppEvent
        let app_event = match event {
            PlatformEvent::Ready { width, height, scale_factor } => AppEvent {
                event_type: AppEventType::Ready,
                data1: width,
                data2: height,
                scale_factor,
            },
            PlatformEvent::RedrawRequested => AppEvent {
                event_type: AppEventType::RedrawRequested,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::Resized { width, height, scale_factor } => AppEvent {
                event_type: AppEventType::Resized,
                data1: width,
                data2: height,
                scale_factor,
            },
            PlatformEvent::CloseRequested => AppEvent {
                event_type: AppEventType::CloseRequested,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchBegan { id: _, x, y } => AppEvent {
                event_type: AppEventType::MousePressed,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchMoved { id: _, x, y } => AppEvent {
                event_type: AppEventType::MouseMoved,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchEnded { id: _, x, y } => AppEvent {
                event_type: AppEventType::MouseReleased,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::TouchCancelled { id: _, x, y } => AppEvent {
                event_type: AppEventType::MouseReleased,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::KeyPressed { keycode, modifiers } => AppEvent {
                event_type: AppEventType::KeyPressed,
                data1: keycode as f64,
                data2: modifiers as f64,
                scale_factor: 1.0,
            },
            PlatformEvent::KeyReleased { keycode, modifiers } => AppEvent {
                event_type: AppEventType::KeyReleased,
                data1: keycode as f64,
                data2: modifiers as f64,
                scale_factor: 1.0,
            },
            PlatformEvent::TextInput { text } => {
                // For text input, we need to return characters through the callback
                // Store text globally and return a special event type
                // For now, send each character as a CharInput event
                for c in text.chars() {
                    let char_event = AppEvent {
                        event_type: AppEventType::CharInput,
                        data1: c as u32 as f64,
                        data2: 0.0,
                        scale_factor: 1.0,
                    };
                    let mut temp_response = FrameResponse {
                        immediate_commands: std::ptr::null_mut(),
                        widget_delta: std::ptr::null_mut(),
                        request_redraw: false,
                        redraw_after_ms: 0,
                        dark_mode: 2,
                        layers: std::ptr::null_mut(),
                    };
                    c_callback(&char_event, &mut temp_response, user_data);
                }
                // Return a placeholder event (the actual char events were already sent)
                return EventResponse::default();
            },
            PlatformEvent::Suspended => AppEvent {
                event_type: AppEventType::Suspended,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::Resumed => AppEvent {
                event_type: AppEventType::Resumed,
                data1: 0.0,
                data2: 0.0,
                scale_factor: 1.0,
            },
            PlatformEvent::MemoryWarning => {
                // No direct equivalent in AppEventType, just log it
                log::warn!("Memory warning received");
                return EventResponse::default();
            },
            PlatformEvent::KeyboardFrameChanged { height, animation_duration } => AppEvent {
                event_type: AppEventType::KeyboardFrameChanged,
                data1: height,
                data2: animation_duration,
                scale_factor: 1.0,
            },
            // Mouse events (desktop) - shouldn't happen on Android but handle anyway
            PlatformEvent::PointerMoved { x, y } => AppEvent {
                event_type: AppEventType::MouseMoved,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::PointerPressed { x, y, button: _ } => AppEvent {
                event_type: AppEventType::MousePressed,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::PointerReleased { x, y, button: _ } => AppEvent {
                event_type: AppEventType::MouseReleased,
                data1: x,
                data2: y,
                scale_factor: 1.0,
            },
            PlatformEvent::Scroll { dx, dy } => AppEvent {
                event_type: AppEventType::MouseWheel,
                data1: dx,
                data2: dy,
                scale_factor: 1.0,
            },
        };

        // Call the C callback
        let mut frame_response = FrameResponse {
            immediate_commands: std::ptr::null_mut(),
            widget_delta: std::ptr::null_mut(),
            request_redraw: false,
            redraw_after_ms: 0,
            dark_mode: 2,
            layers: std::ptr::null_mut(),
        };

        c_callback(&app_event, &mut frame_response, user_data);

        // Process immediate commands if provided
        if !frame_response.immediate_commands.is_null() {
            let json_str = match std::ffi::CStr::from_ptr(frame_response.immediate_commands).to_str() {
                Ok(s) => s,
                Err(_) => return EventResponse::default(),
            };

            if let Ok(commands) = serde_json::from_str::<Vec<RenderCommand>>(json_str) {
                // Use thread-local backend for Android (similar to iOS)
                if let Err(e) = crate::platform::android::render_frame(&commands) {
                    log::error!("[Android] Render error: {}", e);
                }
            }
        }

        EventResponse {
            request_redraw: frame_response.request_redraw,
            exit: false,
            redraw_after_ms: frame_response.redraw_after_ms,
        }
    };

    // On Android, register the callback - the event loop is already running via android-activity
    register_callback(Box::new(rust_callback));

    log::info!("[FFI] run_android_app: callback registered, returning");

    // Return 0 - the event loop is already running
    0
}

/// Desktop app runner using winit
#[cfg(not(any(target_os = "ios", target_os = "android")))]
unsafe fn run_winit_app(config: &AppConfig, callback: AppCallback) -> i32 {
    // Initialize GTK on Linux (required for tray icon menus)
    // Must be done on main thread before any GTK widgets are created
    #[cfg(target_os = "linux")]
    {
        match gtk::init() {
            Ok(()) => eprintln!("[Rust] GTK initialized successfully"),
            Err(e) => eprintln!("[Rust] Warning: Failed to initialize GTK: {}", e),
        }
    }

    // Create event loop with custom user event type for cross-thread signaling
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("Failed to create event loop: {}", e);
            return -2;
        }
    };

    // Store the proxy globally so Go can request redraws from any thread
    {
        let proxy = event_loop.create_proxy();
        let mut guard = get_event_loop_proxy().lock().unwrap();
        *guard = Some(proxy);
    }

    // Start listening for system theme changes (Linux only)
    #[cfg(target_os = "linux")]
    {
        let theme_proxy = event_loop.create_proxy();
        crate::platform::linux::start_theme_listener(move |is_dark| {
            let _ = theme_proxy.send_event(UserEvent::SystemThemeChanged(is_dark));
        });
    }

    // Set control flow to wait for events (saves CPU)
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create app state
    let mut app = App {
        window: None,
        backend: None,
        callback,
        user_data: config.user_data,
        config: AppConfig {
            title: config.title,
            width: config.width,
            height: config.height,
            vsync: config.vsync,
            low_power_gpu: config.low_power_gpu,
            allow_software_fallback: config.allow_software_fallback,
            user_data: config.user_data,
            // Window appearance options
            decorations: config.decorations,
            transparent: config.transparent,
            resizable: config.resizable,
            always_on_top: config.always_on_top,
            min_width: config.min_width,
            min_height: config.min_height,
            max_width: config.max_width,
            max_height: config.max_height,
            x: config.x,
            y: config.y,
            // Frameless window styling
            corner_radius: config.corner_radius,
            show_native_controls: config.show_native_controls,
            enable_minimize: config.enable_minimize,
            enable_maximize: config.enable_maximize,
            target_fps: config.target_fps,
            dark_mode: config.dark_mode,
        },
        should_exit: false,
        modifiers: winit::keyboard::ModifiersState::empty(),
        next_redraw_at: None,
        #[cfg(target_os = "linux")]
        mouse_position: (0.0, 0.0),
        #[cfg(target_os = "linux")]
        resize_direction: None,
        #[cfg(target_os = "linux")]
        window_controls: if !config.decorations && config.show_native_controls {
            Some(crate::platform::linux::WindowControls::with_dark_mode(
                true, // close
                config.enable_minimize,
                config.enable_maximize,
                config.dark_mode,
            ))
        } else {
            None
        },
        #[cfg(target_os = "linux")]
        current_dark_mode: config.dark_mode,
        // Windows window controls initialization
        #[cfg(target_os = "windows")]
        mouse_position: (0.0, 0.0),
        #[cfg(target_os = "windows")]
        resize_direction: None,
        #[cfg(target_os = "windows")]
        window_controls: if !config.decorations && config.show_native_controls {
            Some(crate::platform::windows::WindowControls::with_dark_mode(
                true, // close
                config.enable_minimize,
                config.enable_maximize,
                config.dark_mode,
            ))
        } else {
            None
        },
        #[cfg(target_os = "windows")]
        current_dark_mode: config.dark_mode,
    };

    // Also update global frameless state for batch protocol access
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if let Ok(mut state) = get_frameless_state().lock() {
            state.decorations = config.decorations;
            state.show_native_controls = config.show_native_controls;
            state.dark_mode = config.dark_mode == 1;
            #[cfg(target_os = "linux")]
            {
                state.window_controls = app.window_controls.clone();
            }
            #[cfg(target_os = "windows")]
            {
                state.window_controls = app.window_controls.clone();
            }
        }
    }

    // Run the event loop (blocks until exit)
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("Event loop error: {}", e);
        return -3;
    }

    0
}

/// Request the application to exit
/// Call this from within the callback to trigger a clean shutdown
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_app_request_exit() {
    // This is a hint; actual exit happens via CloseRequested handling
    // For now, we rely on the callback returning and the window closing
}

/// Request a redraw from any thread
/// This is safe to call from background threads (e.g., Go goroutines).
/// It wakes up the event loop and triggers a redraw on the next tick.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_app_request_redraw() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::RequestRedraw) {
            Ok(()) => 0,
            Err(_) => -1, // Event loop closed
        }
    } else {
        -1 // No event loop running
    }
}

// ============================================================================
// Window Control FFI
// ============================================================================

/// Minimize the window
/// Safe to call from any thread.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_window_minimize() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::Minimize) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Toggle maximize state (maximize if not maximized, restore if maximized)
/// Safe to call from any thread.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_window_toggle_maximize() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::ToggleMaximize) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Enter fullscreen mode (borderless fullscreen on primary monitor)
/// Safe to call from any thread.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_window_enter_fullscreen() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::EnterFullscreen) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Exit fullscreen mode
/// Safe to call from any thread.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_window_exit_fullscreen() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::ExitFullscreen) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Toggle fullscreen mode
/// Safe to call from any thread.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_window_toggle_fullscreen() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::ToggleFullscreen) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Request window close (triggers clean shutdown)
/// Safe to call from any thread.
///
/// # Returns
/// 0 on success, -1 if no event loop is running
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_window_close() -> i32 {
    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::Close) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

/// Set the window title
/// Safe to call from any thread.
///
/// # Safety
/// - title must be a valid null-terminated UTF-8 string
///
/// # Returns
/// 0 on success, -1 if no event loop is running, -2 if title is invalid
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_window_set_title(title: *const c_char) -> i32 {
    if title.is_null() {
        return -2;
    }

    let title_str = match CStr::from_ptr(title).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -2,
    };

    let guard = get_event_loop_proxy().lock().unwrap();
    if let Some(ref proxy) = *guard {
        match proxy.send_event(UserEvent::SetTitle(title_str)) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

// ============================================================================
// Safe Area Insets FFI
// ============================================================================

/// C-compatible struct for safe area insets
#[repr(C)]
pub struct SafeAreaInsetsFFI {
    /// Top inset (e.g., status bar, notch on iOS)
    pub top: f32,
    /// Left inset
    pub left: f32,
    /// Bottom inset (e.g., home indicator on iOS)
    pub bottom: f32,
    /// Right inset
    pub right: f32,
}

/// Get the current safe area insets in logical pixels.
///
/// On iOS, this returns the areas occupied by the notch, status bar, and home indicator.
/// On Android, this returns the areas occupied by system UI (status bar, navigation bar, cutouts).
/// On desktop platforms, this returns (0, 0, 0, 0) as there are no unsafe areas.
///
/// Apps should use these values to position content that needs to avoid system UI:
/// - Title bars and navigation should be offset by `top`
/// - Bottom toolbars should be offset by `bottom`
/// - Content in landscape should respect `left` and `right` for notches
///
/// # Returns
/// SafeAreaInsetsFFI struct with top, left, bottom, right values in logical pixels
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_get_safe_area_insets() -> SafeAreaInsetsFFI {
    let insets = SAFE_AREA_INSETS.lock().unwrap();
    SafeAreaInsetsFFI {
        top: insets.top,
        left: insets.left,
        bottom: insets.bottom,
        right: insets.right,
    }
}

/// Get safe area insets via output pointer (iOS-compatible version).
/// This version writes to an output pointer instead of returning a struct,
/// which is required for purego on iOS where struct returns are not supported.
///
/// # Safety
/// `out` must be a valid pointer to a SafeAreaInsetsFFI struct
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_get_safe_area_insets_ptr(out: *mut SafeAreaInsetsFFI) -> i32 {
    if out.is_null() {
        return -1;
    }
    let insets = SAFE_AREA_INSETS.lock().unwrap();
    unsafe {
        (*out).top = insets.top;
        (*out).left = insets.left;
        (*out).bottom = insets.bottom;
        (*out).right = insets.right;
    }
    0
}

/// Internal function to update safe area insets (called from window setup on iOS/Android)
fn update_safe_area_insets(top: f32, left: f32, bottom: f32, right: f32) {
    let mut insets = SAFE_AREA_INSETS.lock().unwrap();
    insets.top = top;
    insets.left = left;
    insets.bottom = bottom;
    insets.right = right;
}

// ============================================================================
// System Preferences FFI
// ============================================================================

/// Check if the operating system is currently in dark mode
///
/// Returns:
/// - 1 if dark mode is enabled
/// - 0 if light mode is enabled
/// - -1 if unable to determine (error or unsupported platform)
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_system_dark_mode() -> i32 {
    #[cfg(target_os = "macos")]
    {
        // Use NSUserDefaults to check AppleInterfaceStyle
        // This is a simpler approach than using NSApplication.effectiveAppearance
        use cocoa::base::{id, nil};
        use cocoa::foundation::NSString;

        unsafe {
            // Get [NSUserDefaults standardUserDefaults]
            let defaults: id = msg_send![class!(NSUserDefaults), standardUserDefaults];
            let key = NSString::alloc(nil).init_str("AppleInterfaceStyle");
            let value: id = msg_send![defaults, stringForKey: key];

            if value == nil {
                return 0; // Light mode (no AppleInterfaceStyle means light)
            }

            // Get the string value
            let utf8: *const i8 = msg_send![value, UTF8String];
            if utf8.is_null() {
                return 0;
            }

            let style = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
            if style.to_lowercase().contains("dark") {
                return 1; // Dark mode
            }
            0 // Light mode
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Check Windows registry for dark mode setting
        // HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize
        // AppsUseLightTheme = 0 means dark mode, 1 means light mode
        use windows::Win32::System::Registry::*;
        use windows::core::*;

        unsafe {
            let key_path = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
            let value_name = w!("AppsUseLightTheme");

            let mut hkey = HKEY::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_path,
                0,
                KEY_READ,
                &mut hkey,
            );

            if result.is_err() {
                return -1; // Unable to open registry key
            }

            let mut value: u32 = 1; // Default to light mode
            let mut value_size = std::mem::size_of::<u32>() as u32;
            let mut value_type = REG_NONE;

            let query_result = RegQueryValueExW(
                hkey,
                value_name,
                None,
                Some(&mut value_type),
                Some(&mut value as *mut u32 as *mut u8),
                Some(&mut value_size),
            );

            let _ = RegCloseKey(hkey);

            if query_result.is_err() {
                return -1; // Unable to query registry value
            }

            // AppsUseLightTheme: 0 = dark mode, 1 = light mode
            if value == 0 { 1 } else { 0 }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Use the XDG Desktop Portal for accurate dark mode detection
        // This is what libadwaita and modern GNOME apps use
        // The portal reflects the actual appearance, not just user preference
        if crate::platform::linux::is_dark_mode() {
            return 1;
        }
        0
    }

    #[cfg(target_os = "ios")]
    {
        // Check UITraitCollection.currentTraitCollection.userInterfaceStyle
        // UIUserInterfaceStyleUnspecified = 0, Light = 1, Dark = 2
        unsafe {
            let trait_collection: *mut objc::runtime::Object =
                msg_send![class!(UITraitCollection), currentTraitCollection];
            if trait_collection.is_null() {
                return -1;
            }
            let style: i64 = msg_send![trait_collection, userInterfaceStyle];
            match style {
                2 => 1,  // UIUserInterfaceStyleDark -> return 1 (dark mode)
                1 => 0,  // UIUserInterfaceStyleLight -> return 0 (light mode)
                _ => 0,  // Unspecified defaults to light
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux", target_os = "ios")))]
    {
        -1 // Unsupported platform
    }
}

// ============================================================================
// Clipboard FFI
// ============================================================================

/// Global storage for clipboard string returned to Go
/// We need to keep the CString alive until the next call
static CLIPBOARD_STRING: Mutex<Option<CString>> = Mutex::new(None);

/// Get the clipboard contents as a null-terminated string
/// Returns null if clipboard is empty or contains non-text data
/// The returned string is valid until the next call to centered_clipboard_get
///
/// # Safety
/// - Returns a pointer to internally managed memory
/// - Caller must not free the returned pointer
/// - Pointer is valid only until next centered_clipboard_get call
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_clipboard_get() -> *const c_char {
    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::NSPasteboard;
        use cocoa::base::nil;

        unsafe {
            let pasteboard: *mut objc::runtime::Object = NSPasteboard::generalPasteboard(nil);
            let nsstring_class = class!(NSString);
            let string_type: *mut objc::runtime::Object =
                msg_send![nsstring_class, stringWithUTF8String: "public.utf8-plain-text\0".as_ptr()];
            let content: *mut objc::runtime::Object = msg_send![pasteboard, stringForType: string_type];

            if content.is_null() {
                return ptr::null();
            }

            let c_str: *const i8 = msg_send![content, UTF8String];
            if c_str.is_null() {
                return ptr::null();
            }

            let rust_str = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
            match CString::new(rust_str) {
                Ok(cstring) => {
                    let ptr = cstring.as_ptr();
                    // Store to keep alive
                    if let Ok(mut guard) = CLIPBOARD_STRING.lock() {
                        *guard = Some(cstring);
                    }
                    ptr
                }
                Err(_) => ptr::null(),
            }
        }
    }

    #[cfg(target_os = "ios")]
    {
        unsafe {
            // Get general pasteboard (UIPasteboard.generalPasteboard)
            let pasteboard: *mut objc::runtime::Object = msg_send![class!(UIPasteboard), generalPasteboard];
            if pasteboard.is_null() {
                return ptr::null();
            }

            // Get string property
            let content: *mut objc::runtime::Object = msg_send![pasteboard, string];
            if content.is_null() {
                return ptr::null();
            }

            let c_str: *const i8 = msg_send![content, UTF8String];
            if c_str.is_null() {
                return ptr::null();
            }

            let rust_str = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
            match CString::new(rust_str) {
                Ok(cstring) => {
                    let ptr = cstring.as_ptr();
                    // Store to keep alive
                    if let Ok(mut guard) = CLIPBOARD_STRING.lock() {
                        *guard = Some(cstring);
                    }
                    ptr
                }
                Err(_) => ptr::null(),
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::*;
        use windows::Win32::System::Memory::*;

        // CF_UNICODETEXT = 13
        const CF_UNICODETEXT: u32 = 13;

        unsafe {
            if OpenClipboard(None).is_err() {
                return ptr::null();
            }

            let handle = GetClipboardData(CF_UNICODETEXT);
            if handle.is_err() {
                let _ = CloseClipboard();
                return ptr::null();
            }
            let handle = handle.unwrap();

            // Convert HANDLE to HGLOBAL for GlobalLock
            let hglobal: HGLOBAL = std::mem::transmute(handle);
            let data = GlobalLock(hglobal);
            if data.is_null() {
                let _ = CloseClipboard();
                return ptr::null();
            }

            // Read UTF-16 string
            let wide_ptr = data as *const u16;
            let mut len = 0;
            while *wide_ptr.add(len) != 0 {
                len += 1;
            }
            let wide_slice = std::slice::from_raw_parts(wide_ptr, len);
            let rust_str = String::from_utf16_lossy(wide_slice);

            let _ = GlobalUnlock(hglobal);
            let _ = CloseClipboard();

            match CString::new(rust_str) {
                Ok(cstring) => {
                    let ptr = cstring.as_ptr();
                    if let Ok(mut guard) = CLIPBOARD_STRING.lock() {
                        *guard = Some(cstring);
                    }
                    ptr
                }
                Err(_) => ptr::null(),
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::LinuxClipboard;

        match LinuxClipboard::new() {
            Ok(mut clipboard) => {
                if let Some(text) = clipboard.get_text() {
                    match CString::new(text) {
                        Ok(cstring) => {
                            let ptr = cstring.as_ptr();
                            if let Ok(mut guard) = CLIPBOARD_STRING.lock() {
                                *guard = Some(cstring);
                            }
                            ptr
                        }
                        Err(_) => ptr::null(),
                    }
                } else {
                    ptr::null()
                }
            }
            Err(_) => ptr::null(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "linux")))]
    {
        ptr::null()
    }
}

/// Set the clipboard contents from a null-terminated string
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string, or null
/// - If text is null, this function does nothing
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_clipboard_set(text: *const c_char) {
    if text.is_null() {
        return;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return,
    };

    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::NSPasteboard;
        use cocoa::base::nil;
        use cocoa::foundation::NSString;

        let pasteboard: *mut objc::runtime::Object = NSPasteboard::generalPasteboard(nil);
        let _: () = msg_send![pasteboard, clearContents];

        let ns_string = NSString::alloc(nil).init_str(text_str);
        let nsstring_class = class!(NSString);
        let string_type: *mut objc::runtime::Object =
            msg_send![nsstring_class, stringWithUTF8String: "public.utf8-plain-text\0".as_ptr()];
        let _: bool = msg_send![pasteboard, setString: ns_string forType: string_type];
    }

    #[cfg(target_os = "ios")]
    {
        // Get general pasteboard (UIPasteboard.generalPasteboard)
        let pasteboard: *mut objc::runtime::Object = msg_send![class!(UIPasteboard), generalPasteboard];
        if pasteboard.is_null() {
            return;
        }

        // Create NSString from text
        let ns_string: *mut objc::runtime::Object = msg_send![class!(NSString), alloc];
        let ns_string: *mut objc::runtime::Object = msg_send![ns_string,
            initWithBytes: text_str.as_ptr()
            length: text_str.len()
            encoding: 4u64]; // NSUTF8StringEncoding

        if ns_string.is_null() {
            return;
        }

        // Set string on pasteboard
        let _: () = msg_send![pasteboard, setString: ns_string];
        let _: () = msg_send![ns_string, release];
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::DataExchange::*;
        use windows::Win32::System::Memory::*;

        // CF_UNICODETEXT = 13
        const CF_UNICODETEXT: u32 = 13;

        // Convert UTF-8 to UTF-16
        let wide: Vec<u16> = text_str.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2; // Size in bytes

        if OpenClipboard(None).is_err() {
            return;
        }

        let _ = EmptyClipboard();

        // Allocate global memory for the clipboard
        let hmem = GlobalAlloc(GMEM_MOVEABLE, size);
        if hmem.is_err() {
            let _ = CloseClipboard();
            return;
        }
        let hmem = hmem.unwrap();

        let dest = GlobalLock(hmem);
        if dest.is_null() {
            // Can't free hmem here easily, but this is rare error case
            let _ = CloseClipboard();
            return;
        }

        // Copy the UTF-16 string
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, dest as *mut u8, size);
        let _ = GlobalUnlock(hmem);

        // Set clipboard data - clipboard takes ownership of hmem on success
        // Convert HGLOBAL to HANDLE for SetClipboardData
        let handle: HANDLE = std::mem::transmute(hmem);
        let _ = SetClipboardData(CF_UNICODETEXT, handle);

        let _ = CloseClipboard();
    }

    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::LinuxClipboard;

        if let Ok(mut clipboard) = LinuxClipboard::new() {
            let _ = clipboard.set_text(text_str);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "linux")))]
    {
        let _ = text_str; // Suppress unused variable warning
    }
}

// ============================================================================
// Keyboard FFI
// ============================================================================

/// Show the software keyboard (iOS only)
/// The view must be able to become first responder
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_keyboard_show() {
    #[cfg(target_os = "ios")]
    {
        crate::platform::ios::show_keyboard();
    }
    #[cfg(target_os = "android")]
    {
        crate::platform::android::show_keyboard();
    }
}

/// Hide the software keyboard
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_keyboard_hide() {
    #[cfg(target_os = "ios")]
    {
        crate::platform::ios::hide_keyboard();
    }
    #[cfg(target_os = "android")]
    {
        crate::platform::android::hide_keyboard();
    }
}

/// Check if keyboard is currently visible
/// Returns 1 if visible, 0 if hidden
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_keyboard_is_visible() -> i32 {
    #[cfg(target_os = "ios")]
    {
        return if crate::platform::ios::is_keyboard_visible() { 1 } else { 0 };
    }

    #[cfg(target_os = "android")]
    {
        return if crate::platform::android::is_keyboard_visible() { 1 } else { 0 };
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        0
    }
}

// ============================================================================
// Haptic Feedback FFI
// ============================================================================

/// Trigger haptic feedback (iOS only)
///
/// # Arguments
/// * `style` - Feedback style:
///   - 0: Light impact
///   - 1: Medium impact
///   - 2: Heavy impact
///   - 3: Soft impact (iOS 13+)
///   - 4: Rigid impact (iOS 13+)
///   - 10: Selection changed
///   - 20: Notification success
///   - 21: Notification warning
///   - 22: Notification error
///
/// On non-iOS platforms, this function does nothing.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_haptic_feedback(style: i32) {
    #[cfg(target_os = "ios")]
    {
        unsafe {
            match style {
                // Impact feedback (0-4)
                0..=4 => {
                    // UIImpactFeedbackStyle: light=0, medium=1, heavy=2, soft=3, rigid=4
                    let generator: *mut objc::runtime::Object = msg_send![
                        class!(UIImpactFeedbackGenerator),
                        alloc
                    ];
                    let generator: *mut objc::runtime::Object = msg_send![
                        generator,
                        initWithStyle: style as i64
                    ];
                    let _: () = msg_send![generator, prepare];
                    let _: () = msg_send![generator, impactOccurred];
                    let _: () = msg_send![generator, release];
                }
                // Selection feedback (10)
                10 => {
                    let generator: *mut objc::runtime::Object = msg_send![
                        class!(UISelectionFeedbackGenerator),
                        new
                    ];
                    let _: () = msg_send![generator, prepare];
                    let _: () = msg_send![generator, selectionChanged];
                    let _: () = msg_send![generator, release];
                }
                // Notification feedback (20-22)
                20..=22 => {
                    // UINotificationFeedbackType: success=0, warning=1, error=2
                    let notification_type = style - 20;
                    let generator: *mut objc::runtime::Object = msg_send![
                        class!(UINotificationFeedbackGenerator),
                        new
                    ];
                    let _: () = msg_send![generator, prepare];
                    let _: () = msg_send![generator, notificationOccurred: notification_type as i64];
                    let _: () = msg_send![generator, release];
                }
                _ => {}
            }
        }
    }

    #[cfg(target_os = "android")]
    {
        // Map iOS style codes to Android style codes
        // Android: 0=Light, 1=Medium, 2=Heavy, 3=Selection, 4=Success, 5=Warning, 6=Error
        let android_style = match style {
            0 => 0,  // Light impact
            1 => 1,  // Medium impact
            2 => 2,  // Heavy impact
            3 => 0,  // Soft -> Light
            4 => 2,  // Rigid -> Heavy
            10 => 3, // Selection changed
            20 => 4, // Success
            21 => 5, // Warning
            22 => 6, // Error
            _ => 1,  // Default to medium
        };
        crate::platform::android::haptic_feedback(android_style);
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let _ = style; // Suppress unused variable warning
    }
}

// ============================================================================
// System Preferences FFI
// ============================================================================

/// Check if natural scrolling is enabled
/// Returns 1 if natural scrolling is enabled, 0 if disabled
/// - macOS: Checks NSUserDefaults for com.apple.swipescrolldirection
/// - Linux: Checks GNOME gsettings and KDE kreadconfig5
/// - iOS/Android: Always returns 1 (touch devices use natural scrolling)
///
/// # Safety
/// This function is safe to call from any thread
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_get_natural_scrolling() -> i32 {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::{id, nil};
        use cocoa::foundation::NSString;

        unsafe {
            let defaults: id = msg_send![class!(NSUserDefaults), standardUserDefaults];
            // com.apple.swipescrolldirection is the key for natural scrolling
            // Returns true (1) when natural scrolling is ON (default)
            let key = NSString::alloc(nil).init_str("com.apple.swipescrolldirection");
            let enabled: bool = msg_send![defaults, boolForKey: key];
            if enabled { 1 } else { 0 }
        }
    }

    #[cfg(target_os = "ios")]
    {
        // iOS always uses natural scrolling (touch-based)
        1
    }

    #[cfg(target_os = "linux")]
    {
        // We handle scroll direction in the Rust event handler, so tell Go
        // the deltas are already correct (return 1 = no additional flipping needed)
        1
    }

    #[cfg(target_os = "android")]
    {
        // Android uses natural scrolling (touch-based)
        1
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux", target_os = "android")))]
    {
        // Default to natural scrolling on other platforms (Windows, etc.)
        1
    }
}

// ============================================================================
// File Dialog FFI
// ============================================================================

/// Open a file dialog for selecting files
///
/// # Arguments
/// * `title` - Dialog title (null-terminated string, or null for default)
/// * `directory` - Initial directory (null-terminated string, or null for default)
/// * `filters` - Comma-separated file extensions (e.g., "png,jpg,jpeg"), or null for all files
/// * `multiple` - 1 to allow multiple selection, 0 for single file
///
/// # Returns
/// Pointer to a JSON string containing an array of selected paths, or null on cancel/error.
/// Caller must free with `centered_file_dialog_result_free`.
///
/// # Safety
/// - All string parameters must be null-terminated UTF-8 strings or null
/// - Returned pointer must be freed with `centered_file_dialog_result_free`
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_file_dialog_open(
    title: *const c_char,
    directory: *const c_char,
    filters: *const c_char,
    multiple: i32,
) -> *mut c_char {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::{id, nil, BOOL, YES, NO};
        use cocoa::foundation::NSString;

        // Parse parameters
        let title_str = if title.is_null() {
            None
        } else {
            CStr::from_ptr(title).to_str().ok()
        };

        let directory_str = if directory.is_null() {
            None
        } else {
            CStr::from_ptr(directory).to_str().ok()
        };

        let filters_str = if filters.is_null() {
            None
        } else {
            CStr::from_ptr(filters).to_str().ok()
        };

        let allow_multiple = multiple != 0;

        // Create NSOpenPanel
        let panel: id = msg_send![class!(NSOpenPanel), openPanel];

        // Set title
        if let Some(t) = title_str {
            if !t.is_empty() {
                let ns_title = NSString::alloc(nil).init_str(t);
                let _: () = msg_send![panel, setTitle: ns_title];
            }
        }

        // Set initial directory
        if let Some(d) = directory_str {
            if !d.is_empty() {
                let ns_dir = NSString::alloc(nil).init_str(d);
                let ns_url: id = msg_send![class!(NSURL), fileURLWithPath: ns_dir];
                let _: () = msg_send![panel, setDirectoryURL: ns_url];
            }
        }

        // Set allowed file types
        if let Some(f) = filters_str {
            if !f.is_empty() {
                let ns_array: id = msg_send![class!(NSMutableArray), array];
                for ext in f.split(',') {
                    let ext = ext.trim();
                    if !ext.is_empty() {
                        let ns_ext = NSString::alloc(nil).init_str(ext);
                        let _: () = msg_send![ns_array, addObject: ns_ext];
                    }
                }
                let _: () = msg_send![panel, setAllowedFileTypes: ns_array];
            }
        }

        // Configure panel
        let _: () = msg_send![panel, setAllowsMultipleSelection: if allow_multiple { YES } else { NO }];
        let _: () = msg_send![panel, setCanChooseFiles: YES];
        let _: () = msg_send![panel, setCanChooseDirectories: NO];

        // Run modal
        let response: i64 = msg_send![panel, runModal];

        // NSModalResponseOK = 1
        if response == 1 {
            let urls: id = msg_send![panel, URLs];
            let count: usize = msg_send![urls, count];

            let mut paths: Vec<String> = Vec::with_capacity(count);
            for i in 0..count {
                let url: id = msg_send![urls, objectAtIndex: i];
                let path: id = msg_send![url, path];
                let utf8: *const i8 = msg_send![path, UTF8String];
                if !utf8.is_null() {
                    if let Ok(s) = CStr::from_ptr(utf8).to_str() {
                        paths.push(s.to_string());
                    }
                }
            }

            // Return as JSON array
            match serde_json::to_string(&paths) {
                Ok(json) => {
                    match CString::new(json) {
                        Ok(cstring) => cstring.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        } else {
            ptr::null_mut()
        }
    }

    #[cfg(target_os = "linux")]
    {
        use rfd::FileDialog;

        eprintln!("[Rust] centered_file_dialog_open called");

        // Parse parameters
        let title_str = if title.is_null() {
            "Open File"
        } else {
            match CStr::from_ptr(title).to_str() {
                Ok(s) if !s.is_empty() => s,
                _ => "Open File",
            }
        };

        let directory_str = if directory.is_null() {
            None
        } else {
            CStr::from_ptr(directory).to_str().ok().filter(|s| !s.is_empty())
        };

        let filters_str = if filters.is_null() {
            None
        } else {
            CStr::from_ptr(filters).to_str().ok().filter(|s| !s.is_empty())
        };

        let allow_multiple = multiple != 0;

        eprintln!("[Rust] File dialog: title='{}', multiple={}", title_str, allow_multiple);

        // Build dialog
        let mut dialog = FileDialog::new().set_title(title_str);

        if let Some(dir) = directory_str {
            dialog = dialog.set_directory(dir);
        }

        // Parse comma-separated extensions
        if let Some(f) = filters_str {
            let exts: Vec<&str> = f.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !exts.is_empty() {
                dialog = dialog.add_filter("Files", &exts);
            }
        }

        eprintln!("[Rust] Showing file dialog...");

        // Show dialog
        let paths = if allow_multiple {
            dialog.pick_files()
        } else {
            dialog.pick_file().map(|p| vec![p])
        };

        eprintln!("[Rust] File dialog returned: {:?}", paths.is_some());

        match paths {
            Some(paths) => {
                let path_strings: Vec<String> = paths.iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                match serde_json::to_string(&path_strings) {
                    Ok(json) => {
                        match CString::new(json) {
                            Ok(cstring) => cstring.into_raw(),
                            Err(_) => ptr::null_mut(),
                        }
                    }
                    Err(_) => ptr::null_mut(),
                }
            }
            None => ptr::null_mut(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (title, directory, filters, multiple);
        ptr::null_mut()
    }
}

/// Open a save file dialog
///
/// # Arguments
/// * `title` - Dialog title (null-terminated string, or null for default)
/// * `directory` - Initial directory (null-terminated string, or null for default)
/// * `filters` - Comma-separated file extensions (e.g., "png,jpg,jpeg"), or null for all files
///
/// # Returns
/// Pointer to the selected path as a null-terminated string, or null on cancel/error.
/// Caller must free with `centered_file_dialog_result_free`.
///
/// # Safety
/// - All string parameters must be null-terminated UTF-8 strings or null
/// - Returned pointer must be freed with `centered_file_dialog_result_free`
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_file_dialog_save(
    title: *const c_char,
    directory: *const c_char,
    filters: *const c_char,
) -> *mut c_char {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::{id, nil};
        use cocoa::foundation::NSString;

        // Parse parameters
        let title_str = if title.is_null() {
            None
        } else {
            CStr::from_ptr(title).to_str().ok()
        };

        let directory_str = if directory.is_null() {
            None
        } else {
            CStr::from_ptr(directory).to_str().ok()
        };

        let filters_str = if filters.is_null() {
            None
        } else {
            CStr::from_ptr(filters).to_str().ok()
        };

        // Create NSSavePanel
        let panel: id = msg_send![class!(NSSavePanel), savePanel];

        // Set title
        if let Some(t) = title_str {
            if !t.is_empty() {
                let ns_title = NSString::alloc(nil).init_str(t);
                let _: () = msg_send![panel, setTitle: ns_title];
            }
        }

        // Set initial directory
        if let Some(d) = directory_str {
            if !d.is_empty() {
                let ns_dir = NSString::alloc(nil).init_str(d);
                let ns_url: id = msg_send![class!(NSURL), fileURLWithPath: ns_dir];
                let _: () = msg_send![panel, setDirectoryURL: ns_url];
            }
        }

        // Set allowed file types
        if let Some(f) = filters_str {
            if !f.is_empty() {
                let ns_array: id = msg_send![class!(NSMutableArray), array];
                for ext in f.split(',') {
                    let ext = ext.trim();
                    if !ext.is_empty() {
                        let ns_ext = NSString::alloc(nil).init_str(ext);
                        let _: () = msg_send![ns_array, addObject: ns_ext];
                    }
                }
                let _: () = msg_send![panel, setAllowedFileTypes: ns_array];
            }
        }

        // Run modal
        let response: i64 = msg_send![panel, runModal];

        // NSModalResponseOK = 1
        if response == 1 {
            let url: id = msg_send![panel, URL];
            if !url.is_null() {
                let path: id = msg_send![url, path];
                let utf8: *const i8 = msg_send![path, UTF8String];
                if !utf8.is_null() {
                    if let Ok(s) = CStr::from_ptr(utf8).to_str() {
                        match CString::new(s) {
                            Ok(cstring) => return cstring.into_raw(),
                            Err(_) => return ptr::null_mut(),
                        }
                    }
                }
            }
        }
        ptr::null_mut()
    }

    #[cfg(target_os = "linux")]
    {
        use rfd::FileDialog;

        // Parse parameters
        let title_str = if title.is_null() {
            "Save File"
        } else {
            match CStr::from_ptr(title).to_str() {
                Ok(s) if !s.is_empty() => s,
                _ => "Save File",
            }
        };

        let directory_str = if directory.is_null() {
            None
        } else {
            CStr::from_ptr(directory).to_str().ok().filter(|s| !s.is_empty())
        };

        let filters_str = if filters.is_null() {
            None
        } else {
            CStr::from_ptr(filters).to_str().ok().filter(|s| !s.is_empty())
        };

        // Build dialog
        let mut dialog = FileDialog::new().set_title(title_str);

        if let Some(dir) = directory_str {
            dialog = dialog.set_directory(dir);
        }

        // Parse comma-separated extensions
        if let Some(f) = filters_str {
            let exts: Vec<&str> = f.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !exts.is_empty() {
                dialog = dialog.add_filter("Files", &exts);
            }
        }

        // Show dialog
        match dialog.save_file() {
            Some(path) => {
                let path_str = path.to_string_lossy().to_string();
                match CString::new(path_str) {
                    Ok(cstring) => cstring.into_raw(),
                    Err(_) => ptr::null_mut(),
                }
            }
            None => ptr::null_mut(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (title, directory, filters);
        ptr::null_mut()
    }
}

/// Free a file dialog result
///
/// # Safety
/// - `result` must be a pointer returned by `centered_file_dialog_open` or `centered_file_dialog_save`
/// - `result` must not be used after this call
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_file_dialog_result_free(result: *mut c_char) {
    if !result.is_null() {
        drop(CString::from_raw(result));
    }
}

// ============================================================================
// Tray Icon FFI
// ============================================================================

#[cfg(target_os = "macos")]
mod tray_icon {
    use cocoa::base::{id, nil, BOOL, YES, NO};
    use cocoa::foundation::NSString;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::Mutex;
    use std::os::raw::c_char;
    use std::ffi::CStr;

    /// Tray icon state
    struct TrayState {
        status_bar: id,
        status_item: id,
        menu: id,
        visible: bool,
        callback: Option<extern "C" fn(i32)>,
    }

    unsafe impl Send for TrayState {}

    impl Default for TrayState {
        fn default() -> Self {
            Self {
                status_bar: nil,
                status_item: nil,
                menu: nil,
                visible: true,
                callback: None,
            }
        }
    }

    static TRAY_STATE: Mutex<Option<TrayState>> = Mutex::new(None);

    /// Create the tray icon
    pub fn create() -> i32 {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        if guard.is_some() {
            return 1; // Already created
        }

        unsafe {
            // Get system status bar
            let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
            if status_bar.is_null() {
                return -1;
            }

            // Create status item with variable length (-1.0)
            let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0f64];
            if status_item.is_null() {
                return -2;
            }

            // Retain the status item
            let _: () = msg_send![status_item, retain];

            // Set default title
            let button: id = msg_send![status_item, button];
            if !button.is_null() {
                let default_title = NSString::alloc(nil).init_str("App");
                let _: () = msg_send![button, setTitle: default_title];
            }

            *guard = Some(TrayState {
                status_bar,
                status_item,
                menu: nil,
                visible: true,
                callback: None,
            });
        }

        0
    }

    /// Destroy the tray icon
    pub fn destroy() {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if let Some(state) = guard.take() {
            unsafe {
                if !state.status_item.is_null() && !state.status_bar.is_null() {
                    let _: () = msg_send![state.status_bar, removeStatusItem: state.status_item];
                    let _: () = msg_send![state.status_item, release];
                }
                if !state.menu.is_null() {
                    let _: () = msg_send![state.menu, release];
                }
            }
        }
    }

    /// Set icon from file path
    pub unsafe fn set_icon_file(path: *const c_char) -> i32 {
        if path.is_null() {
            return -3;
        }

        let path_str = match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => return -3,
        };

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        if state.status_item.is_null() {
            return -1;
        }

        let button: id = msg_send![state.status_item, button];
        if button.is_null() {
            return -2;
        }

        let ns_path = NSString::alloc(nil).init_str(path_str);
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithContentsOfFile: ns_path];

        if image.is_null() {
            return -3;
        }

        // Set template mode for dark/light mode support
        let _: () = msg_send![image, setTemplate: YES];

        // Resize to 18x18 (standard menu bar size)
        #[repr(C)]
        struct NSSize {
            width: f64,
            height: f64,
        }
        let size = NSSize { width: 18.0, height: 18.0 };
        let _: () = msg_send![image, setSize: size];

        let _: () = msg_send![button, setImage: image];

        // Clear title when we have an icon
        let empty = NSString::alloc(nil).init_str("");
        let _: () = msg_send![button, setTitle: empty];

        0
    }

    /// Set icon from raw data
    pub unsafe fn set_icon_data(data: *const u8, length: usize) -> i32 {
        if data.is_null() || length == 0 {
            return -3;
        }

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        if state.status_item.is_null() {
            return -1;
        }

        let button: id = msg_send![state.status_item, button];
        if button.is_null() {
            return -2;
        }

        // Create NSData from bytes
        let ns_data: id = msg_send![class!(NSData), dataWithBytes: data length: length];
        if ns_data.is_null() {
            return -3;
        }

        // Create NSImage from data
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithData: ns_data];

        if image.is_null() {
            return -3;
        }

        // Set template mode
        let _: () = msg_send![image, setTemplate: YES];

        // Resize
        #[repr(C)]
        struct NSSize {
            width: f64,
            height: f64,
        }
        let size = NSSize { width: 18.0, height: 18.0 };
        let _: () = msg_send![image, setSize: size];

        let _: () = msg_send![button, setImage: image];

        // Clear title
        let empty = NSString::alloc(nil).init_str("");
        let _: () = msg_send![button, setTitle: empty];

        0
    }

    /// Set tooltip
    pub unsafe fn set_tooltip(tooltip: *const c_char) {
        if tooltip.is_null() {
            return;
        }

        let tooltip_str = match CStr::from_ptr(tooltip).to_str() {
            Ok(s) => s,
            Err(_) => return,
        };

        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if state.status_item.is_null() {
            return;
        }

        let button: id = msg_send![state.status_item, button];
        if button.is_null() {
            return;
        }

        let ns_tooltip = NSString::alloc(nil).init_str(tooltip_str);
        let _: () = msg_send![button, setToolTip: ns_tooltip];
    }

    /// Set title
    pub unsafe fn set_title(title: *const c_char) {
        if title.is_null() {
            return;
        }

        let title_str = match CStr::from_ptr(title).to_str() {
            Ok(s) => s,
            Err(_) => return,
        };

        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if state.status_item.is_null() {
            return;
        }

        let button: id = msg_send![state.status_item, button];
        if button.is_null() {
            return;
        }

        let ns_title = NSString::alloc(nil).init_str(title_str);
        let _: () = msg_send![button, setTitle: ns_title];
    }

    /// Clear menu
    pub fn clear_menu() {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if !state.menu.is_null() {
            unsafe {
                let _: () = msg_send![state.menu, removeAllItems];
            }
        }
    }

    /// Ensure menu exists
    fn ensure_menu(state: &mut TrayState) {
        if state.menu.is_null() {
            unsafe {
                let menu: id = msg_send![class!(NSMenu), alloc];
                let menu: id = msg_send![menu, init];
                state.menu = menu;

                if !state.status_item.is_null() {
                    let _: () = msg_send![state.status_item, setMenu: menu];
                }
            }
        }
    }

    /// Add menu item
    pub unsafe fn add_menu_item(
        label: *const c_char,
        enabled: i32,
        checked: i32,
        is_separator: i32,
    ) -> i32 {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        ensure_menu(state);

        let menu_item: id;

        if is_separator != 0 {
            menu_item = msg_send![class!(NSMenuItem), separatorItem];
        } else {
            let label_str = if label.is_null() {
                ""
            } else {
                CStr::from_ptr(label).to_str().unwrap_or("")
            };

            let ns_label = NSString::alloc(nil).init_str(label_str);
            let key_equiv = NSString::alloc(nil).init_str("");

            menu_item = msg_send![class!(NSMenuItem), alloc];
            // Note: Without action handler, menu items won't trigger callbacks
            // For now, we create items without actions (callbacks not yet implemented in Rust)
            let menu_item: id = msg_send![menu_item, initWithTitle: ns_label action: nil keyEquivalent: key_equiv];

            let _: () = msg_send![menu_item, setEnabled: if enabled != 0 { YES } else { NO }];

            if checked != 0 {
                let _: () = msg_send![menu_item, setState: 1i64]; // NSControlStateValueOn
            }
        }

        // Get current count for index
        let count: i64 = msg_send![state.menu, numberOfItems];

        // Set tag for identification
        if is_separator == 0 {
            let _: () = msg_send![menu_item, setTag: count];
        }

        // Add to menu
        let _: () = msg_send![state.menu, addItem: menu_item];

        count as i32
    }

    /// Set menu item enabled
    pub fn set_menu_item_enabled(index: i32, enabled: i32) {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if state.menu.is_null() {
            return;
        }

        unsafe {
            let menu_item: id = msg_send![state.menu, itemAtIndex: index as i64];
            if !menu_item.is_null() {
                let _: () = msg_send![menu_item, setEnabled: if enabled != 0 { YES } else { NO }];
            }
        }
    }

    /// Set menu item checked
    pub fn set_menu_item_checked(index: i32, checked: i32) {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if state.menu.is_null() {
            return;
        }

        unsafe {
            let menu_item: id = msg_send![state.menu, itemAtIndex: index as i64];
            if !menu_item.is_null() {
                let _: () = msg_send![menu_item, setState: if checked != 0 { 1i64 } else { 0i64 }];
            }
        }
    }

    /// Set menu item label
    pub unsafe fn set_menu_item_label(index: i32, label: *const c_char) {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if state.menu.is_null() {
            return;
        }

        let label_str = if label.is_null() {
            ""
        } else {
            CStr::from_ptr(label).to_str().unwrap_or("")
        };

        let menu_item: id = msg_send![state.menu, itemAtIndex: index as i64];
        if !menu_item.is_null() {
            let ns_label = NSString::alloc(nil).init_str(label_str);
            let _: () = msg_send![menu_item, setTitle: ns_label];
        }
    }

    /// Set visibility
    pub fn set_visible(visible: i32) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        if state.status_bar.is_null() {
            return;
        }

        state.visible = visible != 0;

        unsafe {
            if visible != 0 {
                if state.status_item.is_null() {
                    let status_item: id = msg_send![state.status_bar, statusItemWithLength: -1.0f64];
                    if !status_item.is_null() {
                        let _: () = msg_send![status_item, retain];
                        state.status_item = status_item;

                        if !state.menu.is_null() {
                            let _: () = msg_send![status_item, setMenu: state.menu];
                        }
                    }
                }
            } else if !state.status_item.is_null() {
                let _: () = msg_send![state.status_bar, removeStatusItem: state.status_item];
                let _: () = msg_send![state.status_item, release];
                state.status_item = nil;
            }
        }
    }

    /// Get visibility
    pub fn is_visible() -> i32 {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        match guard.as_ref() {
            Some(state) => if state.visible { 1 } else { 0 },
            None => 0,
        }
    }

    /// Set callback (stored but not yet fully wired up)
    pub fn set_callback(callback: extern "C" fn(i32)) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if let Some(state) = guard.as_mut() {
            state.callback = Some(callback);
        }
    }
}

#[cfg(target_os = "windows")]
mod tray_icon {
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::sync::Mutex;
    use std::ptr;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::Shell::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Custom message for tray icon callbacks
    const WM_TRAY_CALLBACK: u32 = WM_USER + 1;

    /// Menu item info
    struct MenuItem {
        label: String,
        enabled: bool,
        checked: bool,
        is_separator: bool,
    }

    /// Tray icon state
    struct TrayState {
        hwnd: HWND,
        icon_id: u32,
        hicon: HICON,
        tooltip: String,
        menu: Option<HMENU>,
        menu_items: Vec<MenuItem>,
        visible: bool,
        callback: Option<extern "C" fn(i32)>,
    }

    unsafe impl Send for TrayState {}

    impl Default for TrayState {
        fn default() -> Self {
            Self {
                hwnd: HWND::default(),
                icon_id: 1,
                hicon: HICON::default(),
                tooltip: String::new(),
                menu: None,
                menu_items: Vec::new(),
                visible: true,
                callback: None,
            }
        }
    }

    static TRAY_STATE: Mutex<Option<TrayState>> = Mutex::new(None);

    /// Window procedure for the message window
    unsafe extern "system" fn tray_window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAY_CALLBACK => {
                let event = (lparam.0 & 0xFFFF) as u32;

                // Right-click shows context menu
                if event == WM_RBUTTONUP {
                    show_context_menu(hwnd);
                }

                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Show the context menu at cursor position
    unsafe fn show_context_menu(hwnd: HWND) {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if let Some(menu) = state.menu {
            let mut point = POINT::default();
            let _ = GetCursorPos(&mut point);

            // Required for menu to work properly
            let _ = SetForegroundWindow(hwnd);

            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                0,
                hwnd,
                None,
            );

            // Send dummy message to close menu properly
            let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));

            // Call callback with selected item index
            if cmd.0 > 0 {
                if let Some(callback) = state.callback {
                    drop(guard); // Release lock before callback
                    callback((cmd.0 - 1) as i32); // Convert to 0-based index
                }
            }
        }
    }

    /// Create hidden message window for tray callbacks
    unsafe fn create_message_window() -> Result<HWND, i32> {
        let class_name_wide: Vec<u16> = "CenteredTrayWindow\0".encode_utf16().collect();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(tray_window_proc),
            hInstance: HINSTANCE::default(),
            lpszClassName: PCWSTR::from_raw(class_name_wide.as_ptr()),
            ..Default::default()
        };

        // Register class (may already be registered)
        let _ = RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR::from_raw(class_name_wide.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            None,
            None,
        );

        match hwnd {
            Ok(h) if h != HWND::default() => Ok(h),
            _ => Err(-1),
        }
    }

    /// Create the tray icon
    pub fn create() -> i32 {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        if guard.is_some() {
            return 1; // Already created
        }

        unsafe {
            let hwnd = match create_message_window() {
                Ok(h) => h,
                Err(e) => return e,
            };

            // Create a default icon (app icon or system default)
            let hicon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

            let mut tooltip_wide: [u16; 128] = [0; 128];
            let default_tooltip = "App";
            for (i, ch) in default_tooltip.encode_utf16().take(127).enumerate() {
                tooltip_wide[i] = ch;
            }

            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY_CALLBACK;
            nid.hIcon = hicon;
            nid.szTip = tooltip_wide;

            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                let _ = DestroyWindow(hwnd);
                return -2;
            }

            // Set version for modern behavior
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);

            *guard = Some(TrayState {
                hwnd,
                icon_id: 1,
                hicon,
                tooltip: default_tooltip.to_string(),
                menu: None,
                menu_items: Vec::new(),
                visible: true,
                callback: None,
            });
        }

        0
    }

    /// Destroy the tray icon
    pub fn destroy() {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if let Some(state) = guard.take() {
            unsafe {
                let mut nid = NOTIFYICONDATAW::default();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = state.hwnd;
                nid.uID = state.icon_id;

                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);

                if let Some(menu) = state.menu {
                    let _ = DestroyMenu(menu);
                }

                let _ = DestroyWindow(state.hwnd);
            }
        }
    }

    /// Create HICON from RGBA data
    unsafe fn create_icon_from_rgba(rgba: &[u8], width: u32, height: u32) -> Result<HICON, i32> {
        if rgba.len() != (width * height * 4) as usize {
            return Err(-3);
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // Top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                ..Default::default()
            },
            ..Default::default()
        };

        let hdc = GetDC(None);
        let mut bits_ptr: *mut std::ffi::c_void = ptr::null_mut();

        let color_bitmap = match CreateDIBSection(
            hdc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            None,
            0,
        ) {
            Ok(bmp) if !bmp.is_invalid() && !bits_ptr.is_null() => bmp,
            _ => {
                ReleaseDC(None, hdc);
                return Err(-3);
            }
        };

        // Copy RGBA to BGRA
        let bits = std::slice::from_raw_parts_mut(bits_ptr as *mut u8, rgba.len());
        for i in (0..rgba.len()).step_by(4) {
            bits[i] = rgba[i + 2];     // B
            bits[i + 1] = rgba[i + 1]; // G
            bits[i + 2] = rgba[i];     // R
            bits[i + 3] = rgba[i + 3]; // A
        }

        let mask_bitmap = CreateBitmap(width as i32, height as i32, 1, 1, None);
        if mask_bitmap.is_invalid() {
            let _ = DeleteObject(color_bitmap);
            ReleaseDC(None, hdc);
            return Err(-3);
        }

        let icon_info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
        };

        let hicon = CreateIconIndirect(&icon_info);

        let _ = DeleteObject(color_bitmap);
        let _ = DeleteObject(mask_bitmap);
        ReleaseDC(None, hdc);

        hicon.map_err(|_| -3)
    }

    /// Set icon from file path
    pub unsafe fn set_icon_file(path: *const c_char) -> i32 {
        if path.is_null() {
            return -3;
        }

        let path_str = match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => return -3,
        };

        // Load image using the image crate
        let img = match image::open(path_str) {
            Ok(i) => i,
            Err(_) => return -3,
        };

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        // Create icon from RGBA
        let hicon = match create_icon_from_rgba(rgba.as_raw(), width, height) {
            Ok(h) => h,
            Err(e) => return e,
        };

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        // Update the icon
        state.hicon = hicon;

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = state.hwnd;
        nid.uID = state.icon_id;
        nid.uFlags = NIF_ICON;
        nid.hIcon = hicon;

        if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
            return -2;
        }

        0
    }

    /// Set icon from raw image data (PNG/JPEG bytes)
    pub unsafe fn set_icon_data(data: *const u8, length: usize) -> i32 {
        if data.is_null() || length == 0 {
            return -3;
        }

        let bytes = std::slice::from_raw_parts(data, length);

        // Decode image using the image crate
        let img = match image::load_from_memory(bytes) {
            Ok(i) => i,
            Err(_) => return -3,
        };

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let hicon = match create_icon_from_rgba(rgba.as_raw(), width, height) {
            Ok(h) => h,
            Err(e) => return e,
        };

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        state.hicon = hicon;

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = state.hwnd;
        nid.uID = state.icon_id;
        nid.uFlags = NIF_ICON;
        nid.hIcon = hicon;

        if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
            return -2;
        }

        0
    }

    /// Set tooltip
    pub unsafe fn set_tooltip(tooltip: *const c_char) {
        if tooltip.is_null() {
            return;
        }

        let tooltip_str = match CStr::from_ptr(tooltip).to_str() {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        state.tooltip = tooltip_str.to_string();

        let mut tooltip_wide: [u16; 128] = [0; 128];
        for (i, ch) in tooltip_str.encode_utf16().take(127).enumerate() {
            tooltip_wide[i] = ch;
        }

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = state.hwnd;
        nid.uID = state.icon_id;
        nid.uFlags = NIF_TIP;
        nid.szTip = tooltip_wide;

        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }

    /// Set title (Windows uses tooltip, no separate title)
    pub unsafe fn set_title(title: *const c_char) {
        // Windows tray icons don't have a separate title, use tooltip
        set_tooltip(title);
    }

    /// Clear menu
    pub fn clear_menu() {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        if let Some(menu) = state.menu.take() {
            unsafe {
                let _ = DestroyMenu(menu);
            }
        }
        state.menu_items.clear();
    }

    /// Rebuild the popup menu from menu_items
    unsafe fn rebuild_menu(state: &mut TrayState) {
        if let Some(menu) = state.menu.take() {
            let _ = DestroyMenu(menu);
        }

        if state.menu_items.is_empty() {
            return;
        }

        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };

        for (i, item) in state.menu_items.iter().enumerate() {
            if item.is_separator {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            } else {
                let mut flags = MF_STRING;
                if !item.enabled {
                    flags |= MF_GRAYED;
                }
                if item.checked {
                    flags |= MF_CHECKED;
                }

                let label_wide: Vec<u16> = item.label.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = AppendMenuW(
                    menu,
                    flags,
                    (i + 1) as usize, // 1-based ID for TrackPopupMenu
                    PCWSTR::from_raw(label_wide.as_ptr()),
                );
            }
        }

        state.menu = Some(menu);
    }

    /// Add menu item
    pub unsafe fn add_menu_item(
        label: *const c_char,
        enabled: i32,
        checked: i32,
        is_separator: i32,
    ) -> i32 {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        let label_str = if is_separator != 0 || label.is_null() {
            String::new()
        } else {
            CStr::from_ptr(label).to_str().unwrap_or("").to_string()
        };

        let index = state.menu_items.len() as i32;

        state.menu_items.push(MenuItem {
            label: label_str,
            enabled: enabled != 0,
            checked: checked != 0,
            is_separator: is_separator != 0,
        });

        rebuild_menu(state);

        index
    }

    /// Set menu item enabled state
    pub fn set_menu_item_enabled(index: i32, enabled: i32) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        if let Some(item) = state.menu_items.get_mut(index as usize) {
            item.enabled = enabled != 0;
            unsafe { rebuild_menu(state); }
        }
    }

    /// Set menu item checked state
    pub fn set_menu_item_checked(index: i32, checked: i32) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        if let Some(item) = state.menu_items.get_mut(index as usize) {
            item.checked = checked != 0;
            unsafe { rebuild_menu(state); }
        }
    }

    /// Set menu item label
    pub unsafe fn set_menu_item_label(index: i32, label: *const c_char) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        let label_str = if label.is_null() {
            String::new()
        } else {
            CStr::from_ptr(label).to_str().unwrap_or("").to_string()
        };

        if let Some(item) = state.menu_items.get_mut(index as usize) {
            item.label = label_str;
            rebuild_menu(state);
        }
    }

    /// Set visibility
    pub fn set_visible(visible: i32) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        let was_visible = state.visible;
        state.visible = visible != 0;

        if was_visible == state.visible {
            return;
        }

        unsafe {
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = state.hwnd;
            nid.uID = state.icon_id;

            if visible != 0 {
                // Re-add the icon
                nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
                nid.uCallbackMessage = WM_TRAY_CALLBACK;
                nid.hIcon = state.hicon;

                let mut tooltip_wide: [u16; 128] = [0; 128];
                for (i, ch) in state.tooltip.encode_utf16().take(127).enumerate() {
                    tooltip_wide[i] = ch;
                }
                nid.szTip = tooltip_wide;

                let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            } else {
                // Remove the icon
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            }
        }
    }

    /// Get visibility
    pub fn is_visible() -> i32 {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        match guard.as_ref() {
            Some(state) => if state.visible { 1 } else { 0 },
            None => 0,
        }
    }

    /// Set callback
    pub fn set_callback(callback: extern "C" fn(i32)) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if let Some(state) = guard.as_mut() {
            state.callback = Some(callback);
        }
    }
}

#[cfg(target_os = "linux")]
mod tray_icon {
    use std::sync::Mutex;
    use std::os::raw::c_char;
    use std::ffi::CStr;
    use tray_icon::menu::{Menu, MenuItem, MenuId};

    /// Menu item info for tracking
    struct MenuItemInfo {
        id: MenuId,
        item: MenuItem,
    }

    /// Tray icon state
    struct TrayState {
        tray: Option<tray_icon::TrayIcon>,
        menu: Option<Menu>,
        menu_items: Vec<MenuItemInfo>,
        visible: bool,
        callback: Option<extern "C" fn(i32)>,
    }

    unsafe impl Send for TrayState {}

    impl Default for TrayState {
        fn default() -> Self {
            Self {
                tray: None,
                menu: None,
                menu_items: Vec::new(),
                visible: true,
                callback: None,
            }
        }
    }

    static TRAY_STATE: Mutex<Option<TrayState>> = Mutex::new(None);

    /// Create the tray icon
    /// Note: GTK must be initialized before calling this (done in run_winit_app)
    pub fn create() -> i32 {
        eprintln!("[Rust] tray_icon::create() called");

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("[Rust] Failed to lock TRAY_STATE: {:?}", e);
                return -1;
            }
        };

        if guard.is_some() {
            eprintln!("[Rust] Tray icon already created");
            return 1; // Already created
        }

        // Create a default icon - many Linux DEs won't show tray icons without one
        eprintln!("[Rust] Creating default icon...");
        let default_icon = match create_default_icon() {
            Some(icon) => {
                eprintln!("[Rust] Default icon created successfully");
                icon
            },
            None => {
                eprintln!("[Rust] Failed to create default tray icon");
                return -3;
            }
        };

        // Create a basic tray icon with default icon
        eprintln!("[Rust] Building tray icon...");
        let tray = match tray_icon::TrayIconBuilder::new()
            .with_tooltip("App")
            .with_icon(default_icon)
            .build()
        {
            Ok(t) => {
                eprintln!("[Rust] Tray icon built successfully");
                t
            },
            Err(e) => {
                eprintln!("[Rust] Failed to create tray icon: {}", e);
                return -2;
            }
        };

        *guard = Some(TrayState {
            tray: Some(tray),
            menu: None,
            menu_items: Vec::new(),
            visible: true,
            callback: None,
        });

        eprintln!("[Rust] Tray icon creation complete");
        0
    }

    /// Destroy the tray icon
    pub fn destroy() {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Just drop the state - TrayIcon will clean up on drop
        *guard = None;
    }

    /// Set icon from file path
    pub unsafe fn set_icon_file(path: *const c_char) -> i32 {
        if path.is_null() {
            return -3;
        }

        let path_str = match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => return -3,
        };

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        let tray = match state.tray.as_ref() {
            Some(t) => t,
            None => return -1,
        };

        // Load image and convert to icon
        let img = match image::open(path_str) {
            Ok(i) => i,
            Err(_) => return -3,
        };

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let icon = match tray_icon::Icon::from_rgba(rgba.into_raw(), width, height) {
            Ok(i) => i,
            Err(_) => return -3,
        };

        if tray.set_icon(Some(icon)).is_err() {
            return -4;
        }

        0
    }

    /// Set icon from raw data (PNG encoded)
    pub unsafe fn set_icon_data(data: *const u8, length: usize) -> i32 {
        if data.is_null() || length == 0 {
            return -3;
        }

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        let tray = match state.tray.as_ref() {
            Some(t) => t,
            None => return -1,
        };

        // Load image from bytes
        let bytes = std::slice::from_raw_parts(data, length);
        let img = match image::load_from_memory(bytes) {
            Ok(i) => i,
            Err(_) => return -3,
        };

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let icon = match tray_icon::Icon::from_rgba(rgba.into_raw(), width, height) {
            Ok(i) => i,
            Err(_) => return -3,
        };

        if tray.set_icon(Some(icon)).is_err() {
            return -4;
        }

        0
    }

    /// Set tooltip
    pub unsafe fn set_tooltip(tooltip: *const c_char) {
        if tooltip.is_null() {
            return;
        }

        let tooltip_str = match CStr::from_ptr(tooltip).to_str() {
            Ok(s) => s,
            Err(_) => return,
        };

        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if let Some(tray) = &state.tray {
            let _ = tray.set_tooltip(Some(tooltip_str));
        }
    }

    /// Set title (Linux tray icons don't typically show titles, but we'll use tooltip)
    pub unsafe fn set_title(title: *const c_char) {
        // On Linux, we use the tooltip for the title
        set_tooltip(title);
    }

    /// Clear menu
    pub fn clear_menu() {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return,
        };

        state.menu = None;
        state.menu_items.clear();

        if let Some(tray) = &state.tray {
            tray.set_menu(None);
        }
    }

    /// Add menu item
    pub unsafe fn add_menu_item(
        label: *const c_char,
        enabled: i32,
        _checked: i32,
        is_separator: i32,
    ) -> i32 {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return -1,
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => return -1,
        };

        // Create menu if it doesn't exist
        if state.menu.is_none() {
            state.menu = Some(Menu::new());
        }

        let menu = state.menu.as_ref().unwrap();
        let index = state.menu_items.len() as i32;

        if is_separator != 0 {
            use tray_icon::menu::PredefinedMenuItem;
            let _ = menu.append(&PredefinedMenuItem::separator());
        } else {
            let label_str = if label.is_null() {
                ""
            } else {
                match CStr::from_ptr(label).to_str() {
                    Ok(s) => s,
                    Err(_) => "",
                }
            };

            let item = MenuItem::with_id(index as u32, label_str, enabled != 0, None);
            let id = item.id().clone();
            let _ = menu.append(&item);
            state.menu_items.push(MenuItemInfo { id, item });
        }

        // Update tray menu
        if let Some(tray) = &state.tray {
            if let Some(menu) = &state.menu {
                tray.set_menu(Some(Box::new(menu.clone())));
            }
        }

        index
    }

    /// Set menu item enabled state
    pub fn set_menu_item_enabled(index: i32, enabled: i32) {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if let Some(info) = state.menu_items.get(index as usize) {
            info.item.set_enabled(enabled != 0);
        }
    }

    /// Set menu item checked state (not well supported on Linux)
    pub fn set_menu_item_checked(_index: i32, _checked: i32) {
        // Linux tray menus don't typically support checkmarks in the same way
        // This is a no-op for now
    }

    /// Set menu item label
    pub unsafe fn set_menu_item_label(index: i32, label: *const c_char) {
        if label.is_null() {
            return;
        }

        let label_str = match CStr::from_ptr(label).to_str() {
            Ok(s) => s,
            Err(_) => return,
        };

        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        if let Some(info) = state.menu_items.get(index as usize) {
            info.item.set_text(label_str);
        }
    }

    /// Set visibility
    pub fn set_visible(visible: i32) {
        eprintln!("[Rust] tray_icon::set_visible({}) called", visible);

        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => {
                eprintln!("[Rust] Failed to lock TRAY_STATE in set_visible");
                return;
            }
        };

        let state = match guard.as_mut() {
            Some(s) => s,
            None => {
                eprintln!("[Rust] No tray state in set_visible");
                return;
            }
        };

        state.visible = visible != 0;
        eprintln!("[Rust] Setting tray visible to: {}", state.visible);

        if let Some(tray) = &state.tray {
            match tray.set_visible(state.visible) {
                Ok(()) => eprintln!("[Rust] Tray set_visible succeeded"),
                Err(e) => eprintln!("[Rust] Tray set_visible failed: {:?}", e),
            }
        } else {
            eprintln!("[Rust] No tray icon in state");
        }
    }

    /// Get visibility
    pub fn is_visible() -> i32 {
        let guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        match guard.as_ref() {
            Some(state) => if state.visible { 1 } else { 0 },
            None => 0,
        }
    }

    /// Set callback
    pub fn set_callback(callback: extern "C" fn(i32)) {
        let mut guard = match TRAY_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if let Some(state) = guard.as_mut() {
            state.callback = Some(callback);
        }
    }

    /// Process pending menu events
    /// Should be called from the event loop to handle menu item clicks
    pub fn process_events() {
        use tray_icon::menu::MenuEvent;

        // Try to receive all pending menu events
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            // Find the menu item index that was clicked
            let guard = match TRAY_STATE.lock() {
                Ok(g) => g,
                Err(_) => return,
            };

            if let Some(state) = guard.as_ref() {
                // Find the index of the clicked menu item
                for (index, item_info) in state.menu_items.iter().enumerate() {
                    if item_info.id == event.id {
                        // Call the callback with the index
                        if let Some(callback) = state.callback {
                            drop(guard); // Release lock before calling callback
                            callback(index as i32);
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Create a simple default icon (22x22 blue circle)
    fn create_default_icon() -> Option<tray_icon::Icon> {
        // Create a 22x22 icon with a blue circle (common Linux tray icon size)
        let size = 22u32;
        let center = size as f32 / 2.0;
        let radius = (size as f32 / 2.0) - 1.0;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);

        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist <= radius {
                    // Blue color inside circle
                    rgba.extend_from_slice(&[59, 130, 246, 255]); // Tailwind blue-500
                } else {
                    // Transparent outside circle
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }

        tray_icon::Icon::from_rgba(rgba, size, size).ok()
    }
}

/// Create a system tray icon
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_create() -> i32 {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::create()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        -1
    }
}

/// Destroy the tray icon
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_destroy() {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::destroy();
    }
}

/// Set tray icon from file
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_tray_icon_set_icon_file(path: *const c_char) -> i32 {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_icon_file(path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = path;
        -1
    }
}

/// Set tray icon from raw image data
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_tray_icon_set_icon_data(data: *const u8, length: u64) -> i32 {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_icon_data(data, length as usize)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (data, length);
        -1
    }
}

/// Set tray icon tooltip
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_tray_icon_set_tooltip(tooltip: *const c_char) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_tooltip(tooltip);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = tooltip;
    }
}

/// Set tray icon title
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_tray_icon_set_title(title: *const c_char) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_title(title);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = title;
    }
}

/// Clear tray menu
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_clear_menu() {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::clear_menu();
    }
}

/// Add menu item to tray
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_tray_icon_add_menu_item(
    label: *const c_char,
    enabled: i32,
    checked: i32,
    is_separator: i32,
) -> i32 {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::add_menu_item(label, enabled, checked, is_separator)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (label, enabled, checked, is_separator);
        -1
    }
}

/// Set menu item enabled state
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_set_menu_item_enabled(index: i32, enabled: i32) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_menu_item_enabled(index, enabled);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (index, enabled);
    }
}

/// Set menu item checked state
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_set_menu_item_checked(index: i32, checked: i32) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_menu_item_checked(index, checked);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (index, checked);
    }
}

/// Set menu item label
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_tray_icon_set_menu_item_label(index: i32, label: *const c_char) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_menu_item_label(index, label);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (index, label);
    }
}

/// Set tray icon visibility
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_set_visible(visible: i32) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_visible(visible);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = visible;
    }
}

/// Get tray icon visibility
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_is_visible() -> i32 {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::is_visible()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        0
    }
}

/// Set tray icon menu callback
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_tray_icon_set_callback(callback: extern "C" fn(i32)) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        tray_icon::set_callback(callback);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = callback;
    }
}

// ============================================================================
// Text Measurement FFI
// ============================================================================

use crate::text::font_manager::FontManager;

/// Global font manager for text measurement
static FONT_MANAGER: OnceLock<Mutex<FontManager>> = OnceLock::new();

fn get_font_manager() -> &'static Mutex<FontManager> {
    FONT_MANAGER.get_or_init(|| Mutex::new(FontManager::new()))
}

/// Get the current backend scale factor (for HiDPI displays)
/// Returns 1.0 if backend is not initialized
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_get_scale_factor() -> f64 {
    let backend_lock = get_backend();
    let guard = match backend_lock.lock() {
        Ok(g) => g,
        Err(_) => return 1.0,
    };

    if let Some(backend) = guard.as_ref() {
        backend.scale_factor()
    } else {
        1.0
    }
}

/// Text measurement result
#[repr(C)]
pub struct TextMeasurement {
    /// Total width of the text in pixels
    pub width: f32,
    /// Total height of the text in pixels (based on font metrics, not bounding box)
    pub height: f32,
    /// Font ascent (distance from baseline to top)
    pub ascent: f32,
    /// Font descent (distance from baseline to bottom, positive value)
    pub descent: f32,
}

/// Measure text dimensions with a specific font
///
/// This function measures the pixel dimensions of the given text string
/// using the specified font. Useful for cursor positioning, layout calculations,
/// and text editing.
///
/// # Arguments
/// * `text` - The text to measure (null-terminated UTF-8)
/// * `font_name` - System font name (null-terminated UTF-8), e.g., "Helvetica", "San Francisco"
/// * `font_size` - Font size in points
///
/// # Returns
/// TextMeasurement with width, height, ascent, and descent.
/// On error, returns all zeros.
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_name must be a valid null-terminated UTF-8 string
#[cfg(not(target_os = "android"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text(
    text: *const c_char,
    font_name: *const c_char,
    font_size: f32,
) -> TextMeasurement {
    let error_result = TextMeasurement {
        width: 0.0,
        height: 0.0,
        ascent: 0.0,
        descent: 0.0,
    };

    if text.is_null() || font_name.is_null() {
        return error_result;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    let font_name_str = match CStr::from_ptr(font_name).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    // Load font and measure
    let font_manager = get_font_manager();
    let mut manager = match font_manager.lock() {
        Ok(m) => m,
        Err(_) => return error_result,
    };

    let descriptor = FontDescriptor::system(font_name_str, 400, FontStyle::Normal, font_size);

    match manager.load_font(&descriptor) {
        Ok(font) => {
            let width = font.measure_text(text_str);
            let ascent = font.ascent();
            let descent = font.descent().abs(); // descent is typically negative
            let height = ascent + descent;

            TextMeasurement {
                width,
                height,
                ascent,
                descent,
            }
        }
        Err(e) => {
            eprintln!("Failed to load font '{}' for measurement: {}", font_name_str, e);
            error_result
        }
    }
}

/// Android implementation using JNI Canvas API
#[cfg(target_os = "android")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text(
    text: *const c_char,
    font_name: *const c_char,
    font_size: f32,
) -> TextMeasurement {
    // Fallback result using character-count heuristic
    // This ensures layout still works even if JNI measurement fails
    let make_fallback = |text_str: &str| {
        // Approximate average character width as 0.5 * font_size for proportional fonts
        // This is a rough estimate but better than 0
        let char_count = text_str.chars().count() as f32;
        let width = char_count * font_size * 0.5;
        let ascent = font_size * 0.8;
        let descent = font_size * 0.2;
        TextMeasurement {
            width,
            height: ascent + descent,
            ascent,
            descent,
        }
    };

    if text.is_null() || font_name.is_null() {
        return TextMeasurement {
            width: 0.0,
            height: font_size,
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
        };
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return make_fallback(""),
    };

    let font_name_str = match CStr::from_ptr(font_name).to_str() {
        Ok(s) => s,
        Err(_) => return make_fallback(text_str),
    };

    // Measure at logical font size - rendering scales everything proportionally
    // (positions AND font size), so measurement at logical size gives logical width
    let descriptor = FontDescriptor::system(font_name_str, 400, FontStyle::Normal, font_size);

    // Use Android text measurement via JNI
    let width = match crate::text::atlas::android::measure_text_width(text_str, &descriptor) {
        Some(w) if w > 0.0 => w,
        _ => {
            // JNI measurement failed - use fallback
            log::warn!("Android text measurement failed for '{}', using fallback", text_str);
            return make_fallback(text_str);
        }
    };

    // Approximate height based on font size (proper metrics would require more JNI calls)
    let ascent = font_size * 0.8;
    let descent = font_size * 0.2;
    let height = ascent + descent;

    TextMeasurement {
        width,
        height,
        ascent,
        descent,
    }
}

/// Measure text dimensions - pointer-based version for iOS compatibility
///
/// This version writes the result to an output pointer instead of returning by value,
/// which is needed for purego compatibility on iOS where struct returns aren't supported.
///
/// # Arguments
/// * `text` - The text to measure (null-terminated UTF-8)
/// * `font_name` - System font name (null-terminated UTF-8)
/// * `font_size` - Font size in points
/// * `out` - Pointer to TextMeasurement struct to write result into
///
/// # Returns
/// 0 on success, -1 on error
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_name must be a valid null-terminated UTF-8 string
/// - out must be a valid pointer to a TextMeasurement struct
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_ptr(
    text: *const c_char,
    font_name: *const c_char,
    font_size: f32,
    out: *mut TextMeasurement,
) -> i32 {
    if out.is_null() {
        return -1;
    }

    let result = centered_measure_text(text, font_name, font_size);
    *out = result;
    0
}

/// Measure text width only (simpler API for common use case)
///
/// # Arguments
/// * `text` - The text to measure (null-terminated UTF-8)
/// * `font_name` - System font name (null-terminated UTF-8)
/// * `font_size` - Font size in points
///
/// # Returns
/// Width of the text in pixels. Returns 0.0 on error.
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_name must be a valid null-terminated UTF-8 string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_width(
    text: *const c_char,
    font_name: *const c_char,
    font_size: f32,
) -> f32 {
    centered_measure_text(text, font_name, font_size).width
}

/// Measure a substring's width for cursor positioning
///
/// Measures the width of text[0..char_index]. Useful for calculating
/// cursor X position in a text field.
///
/// This function sums up individual glyph advances to match how text rendering
/// positions characters. This ensures the cursor position matches the actual
/// rendered text position exactly.
///
/// # Arguments
/// * `text` - The full text (null-terminated UTF-8)
/// * `char_index` - Character index (0-based, counts Unicode characters not bytes)
/// * `font_name` - System font name (null-terminated UTF-8)
/// * `font_size` - Font size in points
///
/// # Returns
/// Width of text up to char_index in pixels. Returns 0.0 on error.
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_name must be a valid null-terminated UTF-8 string
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_to_cursor(
    text: *const c_char,
    char_index: u32,
    font_name: *const c_char,
    font_size: f32,
) -> f32 {
    if text.is_null() || font_name.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Get substring up to char_index
    let substring: String = text_str.chars().take(char_index as usize).collect();

    if substring.is_empty() {
        return 0.0;
    }

    let font_name_str = match CStr::from_ptr(font_name).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Get scale factor from backend (same as rendering uses)
    // Rendering scales font_size by scale_factor, so we must too for accurate measurement
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return 0.0,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size just like rendering does (see wgpu_backend.rs draw_text)
    let scaled_font_size = font_size * scale_factor;

    // Use CTLine to measure the entire string at once (fast path - no rasterization)
    let mut rasterizer = crate::text::atlas::MacOSGlyphRasterizer::new();
    let descriptor = FontDescriptor::system(font_name_str, 400, FontStyle::Normal, scaled_font_size);

    // Measure the whole substring at once using CTLine
    let total_width = rasterizer.measure_string(&substring, &descriptor);

    // Convert back to logical pixels (divide by scale factor)
    // Go works in logical pixels, rendering works in physical pixels
    total_width / scale_factor
}

/// Measure text width with a full font descriptor (supports bundled fonts)
///
/// This function supports both system fonts and bundled fonts by taking
/// a JSON-encoded FontDescriptor.
///
/// # Arguments
/// * `text` - The text to measure (null-terminated UTF-8)
/// * `font_json` - JSON-encoded FontDescriptor (null-terminated UTF-8)
///
/// # Returns
/// Width of the text in logical pixels. Returns 0.0 on error.
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> f32 {
    if text.is_null() || font_json.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    if text_str.is_empty() {
        return 0.0;
    }

    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse font descriptor JSON: {}", e);
            return 0.0;
        }
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return 0.0,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size for physical pixels
    let scaled_descriptor = FontDescriptor {
        source: descriptor.source,
        weight: descriptor.weight,
        style: descriptor.style,
        size: descriptor.size * scale_factor,
    };

    // Use the rasterizer's measure_string which handles bundled fonts
    let mut rasterizer = crate::text::atlas::MacOSGlyphRasterizer::new();
    let width = rasterizer.measure_string(text_str, &scaled_descriptor);

    // Convert back to logical pixels
    width / scale_factor
}

/// Measure text dimensions with a full font descriptor (supports bundled fonts)
///
/// This function returns full text metrics including height, ascent, and descent.
/// It supports both system fonts and bundled fonts via the FontDescriptor.
///
/// # Arguments
/// * `text` - The text to measure (null-terminated UTF-8)
/// * `font_json` - JSON-encoded FontDescriptor (null-terminated UTF-8)
///
/// # Returns
/// TextMeasurement with width, height, ascent, and descent in logical pixels.
/// On error, returns all zeros.
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_metrics_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> TextMeasurement {
    let error_result = TextMeasurement {
        width: 0.0,
        height: 0.0,
        ascent: 0.0,
        descent: 0.0,
    };

    if text.is_null() || font_json.is_null() {
        return error_result;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    // Empty text still has font metrics (height based on font)
    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse font descriptor JSON: {}", e);
            return error_result;
        }
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return error_result,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size for physical pixels
    let scaled_descriptor = FontDescriptor {
        source: descriptor.source,
        weight: descriptor.weight,
        style: descriptor.style,
        size: descriptor.size * scale_factor,
    };

    // Use font manager to get font metrics
    let font_manager = get_font_manager();
    let mut manager = match font_manager.lock() {
        Ok(m) => m,
        Err(_) => return error_result,
    };

    match manager.load_font(&scaled_descriptor) {
        Ok(font) => {
            let width = if text_str.is_empty() {
                0.0
            } else {
                font.measure_text(text_str)
            };
            let ascent = font.ascent();
            let descent = font.descent().abs();
            let height = ascent + descent;

            TextMeasurement {
                width: width / scale_factor,
                height: height / scale_factor,
                ascent: ascent / scale_factor,
                descent: descent / scale_factor,
            }
        }
        Err(e) => {
            eprintln!("Failed to load font for measurement: {}", e);
            error_result
        }
    }
}

/// Pointer-based version of centered_measure_text_metrics_with_font for iOS compatibility.
/// iOS with purego doesn't support returning structs directly, so we write to an output pointer.
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
/// - out must be a valid pointer to a TextMeasurement struct
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_metrics_with_font_ptr(
    text: *const c_char,
    font_json: *const c_char,
    out: *mut TextMeasurement,
) -> i32 {
    if out.is_null() {
        return -1;
    }

    let result = centered_measure_text_metrics_with_font(text, font_json);
    *out = result;
    0
}

/// Windows implementation: Measure text with font and return metrics
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
#[cfg(target_os = "windows")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_metrics_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> TextMeasurement {
    use crate::text::FontDescriptor;

    let error_result = TextMeasurement {
        width: 0.0,
        height: 0.0,
        ascent: 0.0,
        descent: 0.0,
    };

    if text.is_null() || font_json.is_null() {
        return error_result;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse font descriptor JSON: {}", e);
            return error_result;
        }
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return error_result,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size for physical pixels
    let scaled_descriptor = FontDescriptor {
        source: descriptor.source,
        weight: descriptor.weight,
        style: descriptor.style,
        size: descriptor.size * scale_factor,
    };

    // Use the backend's public methods to measure text
    let backend_lock = get_backend();
    let mut guard = match backend_lock.lock() {
        Ok(g) => g,
        Err(_) => return error_result,
    };

    if let Some(backend) = guard.as_mut() {
        let width = if text_str.is_empty() {
            0.0
        } else {
            backend.measure_string(text_str, &scaled_descriptor)
        };

        let (ascent, descent) = backend.get_font_metrics(&scaled_descriptor);
        let height = ascent + descent;

        TextMeasurement {
            width: width / scale_factor,
            height: height / scale_factor,
            ascent: ascent / scale_factor,
            descent: descent / scale_factor,
        }
    } else {
        error_result
    }
}

/// Windows implementation: Pointer-based version
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
/// - out must be a valid pointer to a TextMeasurement struct
#[cfg(target_os = "windows")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_metrics_with_font_ptr(
    text: *const c_char,
    font_json: *const c_char,
    out: *mut TextMeasurement,
) -> i32 {
    if out.is_null() {
        return -1;
    }

    let result = centered_measure_text_metrics_with_font(text, font_json);
    *out = result;
    0
}

// Android implementations for text measurement using JNI Canvas API
#[cfg(target_os = "android")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_to_cursor(
    text: *const c_char,
    char_index: u32,
    font_name: *const c_char,
    font_size: f32,
) -> f32 {
    if text.is_null() || font_name.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    let font_name_str = match CStr::from_ptr(font_name).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Measure at logical font size - rendering scales everything proportionally
    let descriptor = FontDescriptor::system(font_name_str, 400, FontStyle::Normal, font_size);

    // Use Android text measurement via JNI
    crate::text::atlas::android::measure_text_to_cursor(text_str, char_index as usize, &descriptor)
        .unwrap_or(0.0)
}

#[cfg(target_os = "android")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> f32 {
    if text.is_null() || font_json.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    if text_str.is_empty() {
        return 0.0;
    }

    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to parse font descriptor JSON: {}", e);
            return 0.0;
        }
    };

    // Measure at logical font size - rendering scales everything proportionally
    crate::text::atlas::android::measure_text_width(text_str, &descriptor)
        .unwrap_or(0.0)
}

// Linux implementations for text measurement using FreeType

/// Global Linux glyph rasterizer for FFI text measurement (preserves font caches across calls)
#[cfg(target_os = "linux")]
static LINUX_RASTERIZER: OnceLock<Mutex<crate::text::atlas::LinuxGlyphRasterizer>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn get_linux_rasterizer() -> &'static Mutex<crate::text::atlas::LinuxGlyphRasterizer> {
    LINUX_RASTERIZER.get_or_init(|| Mutex::new(crate::text::atlas::LinuxGlyphRasterizer::new()))
}

#[cfg(target_os = "linux")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_to_cursor(
    text: *const c_char,
    char_index: u32,
    font_name: *const c_char,
    font_size: f32,
) -> f32 {
    if text.is_null() || font_name.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Get substring up to char_index
    let substring: String = text_str.chars().take(char_index as usize).collect();

    if substring.is_empty() {
        return 0.0;
    }

    let font_name_str = match CStr::from_ptr(font_name).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return 0.0,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size just like rendering does
    let scaled_font_size = font_size * scale_factor;

    // Use global LinuxGlyphRasterizer (preserves font caches across calls)
    let rasterizer = get_linux_rasterizer();
    let mut rasterizer = match rasterizer.lock() {
        Ok(r) => r,
        Err(_) => return 0.0,
    };
    let descriptor = FontDescriptor::system(font_name_str, 400, FontStyle::Normal, scaled_font_size);

    // Measure the whole substring at once
    let total_width = rasterizer.measure_string(&substring, &descriptor);

    // Convert back to logical pixels
    total_width / scale_factor
}

#[cfg(target_os = "linux")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> f32 {
    if text.is_null() || font_json.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    if text_str.is_empty() {
        return 0.0;
    }

    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse font descriptor JSON: {}", e);
            return 0.0;
        }
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return 0.0,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size for physical pixels
    let scaled_descriptor = FontDescriptor {
        source: descriptor.source,
        weight: descriptor.weight,
        style: descriptor.style,
        size: descriptor.size * scale_factor,
    };

    // Use global LinuxGlyphRasterizer (preserves font caches across calls)
    let rasterizer = get_linux_rasterizer();
    let mut rasterizer = match rasterizer.lock() {
        Ok(r) => r,
        Err(_) => return 0.0,
    };
    let width = rasterizer.measure_string(text_str, &scaled_descriptor);

    // Convert back to logical pixels
    width / scale_factor
}

/// Linux implementation: Measure text with font and return metrics
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
#[cfg(target_os = "linux")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_metrics_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> TextMeasurement {
    let error_result = TextMeasurement {
        width: 0.0,
        height: 0.0,
        ascent: 0.0,
        descent: 0.0,
    };

    if text.is_null() || font_json.is_null() {
        return error_result;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return error_result,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse font descriptor JSON: {}", e);
            return error_result;
        }
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return error_result,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size for physical pixels
    let scaled_descriptor = FontDescriptor {
        source: descriptor.source,
        weight: descriptor.weight,
        style: descriptor.style,
        size: descriptor.size * scale_factor,
    };

    // Use global LinuxGlyphRasterizer (preserves font caches across calls)
    let rasterizer = get_linux_rasterizer();
    let mut rasterizer = match rasterizer.lock() {
        Ok(r) => r,
        Err(_) => return error_result,
    };

    let width = if text_str.is_empty() {
        0.0
    } else {
        rasterizer.measure_string(text_str, &scaled_descriptor)
    };

    let (ascent, descent) = rasterizer.get_font_metrics(&scaled_descriptor);
    let height = ascent + descent;

    TextMeasurement {
        width: width / scale_factor,
        height: height / scale_factor,
        ascent: ascent / scale_factor,
        descent: descent / scale_factor,
    }
}

/// Linux implementation: Pointer-based version for purego compatibility
///
/// # Safety
/// - text must be a valid null-terminated UTF-8 string
/// - font_json must be a valid null-terminated UTF-8 JSON string
/// - out must be a valid pointer to a TextMeasurement struct
#[cfg(target_os = "linux")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_metrics_with_font_ptr(
    text: *const c_char,
    font_json: *const c_char,
    out: *mut TextMeasurement,
) -> i32 {
    if out.is_null() {
        return -1;
    }

    let result = centered_measure_text_metrics_with_font(text, font_json);
    *out = result;
    0
}

// Windows implementations for text measurement using DirectWrite
#[cfg(target_os = "windows")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_to_cursor(
    text: *const c_char,
    char_index: u32,
    font_name: *const c_char,
    font_size: f32,
) -> f32 {
    if text.is_null() || font_name.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Get substring up to char_index
    let substring: String = text_str.chars().take(char_index as usize).collect();

    if substring.is_empty() {
        return 0.0;
    }

    let font_name_str = match CStr::from_ptr(font_name).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return 0.0,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size just like rendering does
    let scaled_font_size = font_size * scale_factor;

    // Use WindowsGlyphRasterizer to measure the substring
    let mut rasterizer = crate::text::atlas::WindowsGlyphRasterizer::new();
    let descriptor = FontDescriptor::system(font_name_str, 400, FontStyle::Normal, scaled_font_size);

    // Measure the whole substring at once
    let total_width = rasterizer.measure_string(&substring, &descriptor);

    // Convert back to logical pixels
    total_width / scale_factor
}

#[cfg(target_os = "windows")]
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_measure_text_with_font(
    text: *const c_char,
    font_json: *const c_char,
) -> f32 {
    if text.is_null() || font_json.is_null() {
        return 0.0;
    }

    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    if text_str.is_empty() {
        return 0.0;
    }

    let font_json_str = match CStr::from_ptr(font_json).to_str() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };

    // Parse the font descriptor from JSON
    let descriptor: FontDescriptor = match serde_json::from_str(font_json_str) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse font descriptor JSON: {}", e);
            return 0.0;
        }
    };

    // Get scale factor from backend
    let scale_factor = {
        let backend_lock = get_backend();
        let guard = match backend_lock.lock() {
            Ok(g) => g,
            Err(_) => return 0.0,
        };
        if let Some(backend) = guard.as_ref() {
            backend.scale_factor() as f32
        } else {
            1.0f32
        }
    };

    // Scale font size for physical pixels
    let scaled_descriptor = FontDescriptor {
        source: descriptor.source,
        weight: descriptor.weight,
        style: descriptor.style,
        size: descriptor.size * scale_factor,
    };

    // Use the WindowsGlyphRasterizer's measure_string which handles bundled fonts
    let mut rasterizer = crate::text::atlas::WindowsGlyphRasterizer::new();
    let width = rasterizer.measure_string(text_str, &scaled_descriptor);

    // Convert back to logical pixels
    width / scale_factor
}

// ============================================================================
// Audio FFI
// ============================================================================
//
// Audio playback API for loading and playing audio files.
// Uses platform-native APIs (AVFoundation on macOS) for optimal quality
// and to respect system output device preferences.

use crate::audio::player::AudioPlayer;

// Global audio player storage
lazy_static::lazy_static! {
    static ref AUDIO_PLAYERS: std::sync::Mutex<std::collections::HashMap<u32, AudioPlayer>> = std::sync::Mutex::new(std::collections::HashMap::new());
    static ref NEXT_AUDIO_PLAYER_ID: std::sync::Mutex<u32> = std::sync::Mutex::new(1);
}

/// Create a new audio player
///
/// # Returns
/// A unique player ID (always positive), or 0 on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_create() -> u32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    let mut next_id = NEXT_AUDIO_PLAYER_ID.lock().unwrap();

    let player_id = *next_id;
    *next_id += 1;

    players.insert(player_id, AudioPlayer::new());
    player_id
}

/// Destroy an audio player and free resources
///
/// # Arguments
/// * `player_id` - Player ID from centered_audio_create
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_destroy(player_id: u32) {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    players.remove(&player_id);
}

/// Load audio from a URL (file:// or http://)
///
/// # Arguments
/// * `player_id` - Player ID from centered_audio_create
/// * `url` - Null-terminated UTF-8 URL string
///
/// # Returns
/// 0 on success, negative error code on failure:
/// - -1: Invalid parameters
/// - -2: Player not found
/// - -3: Load failed
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_audio_load_url(
    player_id: u32,
    url: *const c_char,
) -> i32 {
    if url.is_null() {
        return -1;
    }

    let url_str = match CStr::from_ptr(url).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.load_url(url_str) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Audio load error: {}", e);
                -3
            }
        }
    } else {
        -2
    }
}

/// Load audio from a file path
///
/// # Arguments
/// * `player_id` - Player ID from centered_audio_create
/// * `path` - Null-terminated UTF-8 file path
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_audio_load_file(
    player_id: u32,
    path: *const c_char,
) -> i32 {
    if path.is_null() {
        return -1;
    }

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.load_file(path_str) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Audio load error: {}", e);
                -3
            }
        }
    } else {
        -2
    }
}

/// Start or resume audio playback
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_play(player_id: u32) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.play() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Pause audio playback
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_pause(player_id: u32) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.pause() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Stop audio playback and reset to beginning
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_stop(player_id: u32) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.stop() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Seek to a specific position in milliseconds
///
/// # Arguments
/// * `player_id` - Player ID
/// * `timestamp_ms` - Target position in milliseconds
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_seek(player_id: u32, timestamp_ms: u64) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        match player.seek(timestamp_ms) {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Set looping behavior
///
/// # Arguments
/// * `player_id` - Player ID
/// * `looping` - Whether to loop playback
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_set_looping(player_id: u32, looping: bool) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.set_looping(looping);
        0
    } else {
        -2
    }
}

/// Set volume (0.0 - 1.0)
///
/// # Arguments
/// * `player_id` - Player ID
/// * `volume` - Volume level (0.0 = silent, 1.0 = full volume)
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_set_volume(player_id: u32, volume: f32) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        player.set_volume(volume);
        0
    } else {
        -2
    }
}

/// Get current playback state
///
/// # Returns
/// PlaybackState as i32:
/// - 0: Idle
/// - 1: Loading
/// - 2: Playing
/// - 3: Paused
/// - 4: Ended
/// - 5: Error
/// - Negative: Player not found
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_get_state(player_id: u32) -> i32 {
    let players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        player.state() as i32
    } else {
        -2
    }
}

/// Get current playback position in milliseconds
///
/// # Returns
/// Current position in milliseconds, or 0 if player not found
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_get_time(player_id: u32) -> u64 {
    let players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        player.current_time_ms()
    } else {
        0
    }
}

/// Get audio info (duration)
///
/// # Arguments
/// * `player_id` - Player ID
/// * `duration_ms_out` - Pointer to store duration in milliseconds
/// * `sample_rate_out` - Pointer to store sample rate (Hz)
/// * `channels_out` - Pointer to store channel count
///
/// # Returns
/// 0 on success, negative error code on failure
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_audio_get_info(
    player_id: u32,
    duration_ms_out: *mut u64,
    sample_rate_out: *mut u32,
    channels_out: *mut u32,
) -> i32 {
    if duration_ms_out.is_null() || sample_rate_out.is_null() || channels_out.is_null() {
        return -1;
    }

    let players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        if let Some(info) = player.info() {
            *duration_ms_out = info.duration_ms;
            *sample_rate_out = info.sample_rate;
            *channels_out = info.channels;
            0
        } else {
            -3 // No audio loaded
        }
    } else {
        -2
    }
}

/// Get current volume
///
/// # Returns
/// Volume (0.0 - 1.0), or 0.0 if player not found
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_get_volume(player_id: u32) -> f32 {
    let players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        player.volume()
    } else {
        0.0
    }
}

/// Check if audio is looping
///
/// # Returns
/// 1 if looping, 0 if not looping or player not found
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_is_looping(player_id: u32) -> i32 {
    let players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get(&player_id) {
        if player.is_looping() { 1 } else { 0 }
    } else {
        0
    }
}

/// Update audio player state
///
/// Should be called periodically (e.g., each frame) to update playback state.
/// Returns whether the state changed.
///
/// # Returns
/// 1 if state changed, 0 if not, negative on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_update(player_id: u32) -> i32 {
    let mut players = AUDIO_PLAYERS.lock().unwrap();
    if let Some(player) = players.get_mut(&player_id) {
        if player.update() { 1 } else { 0 }
    } else {
        -2
    }
}

// ============================================================================
// Audio Input (Microphone) FFI
// ============================================================================

use crate::audio::input::{AudioInput, AudioInputConfig, AudioInputState};

lazy_static::lazy_static! {
    /// Global audio input storage
    static ref AUDIO_INPUTS: std::sync::Mutex<std::collections::HashMap<u32, AudioInput>> = std::sync::Mutex::new(std::collections::HashMap::new());
    static ref NEXT_AUDIO_INPUT_ID: std::sync::Mutex<u32> = std::sync::Mutex::new(1);
}

/// Create a new audio input (microphone)
///
/// # Returns
/// A unique input ID (always positive), or 0 on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_create() -> u32 {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    let mut next_id = NEXT_AUDIO_INPUT_ID.lock().unwrap();

    let input_id = *next_id;
    *next_id += 1;

    inputs.insert(input_id, AudioInput::new());
    input_id
}

/// Destroy an audio input and free resources
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_destroy(input_id: u32) {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(mut input) = inputs.remove(&input_id) {
        input.close();
    }
}

/// Request microphone permission
///
/// # Returns
/// 0 on success, 1 if permission needs to be granted, negative on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_request_permission(input_id: u32) -> i32 {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.request_permission() {
            Ok(()) => 0,
            Err(_) => 1, // Permission needed or denied
        }
    } else {
        -2
    }
}

/// Check if microphone permission is granted
///
/// # Returns
/// 1 if granted, 0 if not
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_has_permission(input_id: u32) -> i32 {
    let inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        if input.has_permission() { 1 } else { 0 }
    } else {
        0
    }
}

/// List available audio input devices
/// Returns a JSON array of device info, or null on error
/// Caller must free the returned string with centered_free_string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_list_devices(input_id: u32) -> *mut c_char {
    let inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        match input.list_devices() {
            Ok(devices) => {
                let json = serde_json::json!(devices.iter().map(|d| {
                    serde_json::json!({
                        "id": d.id,
                        "name": d.name,
                        "is_default": d.is_default,
                    })
                }).collect::<Vec<_>>());
                match CString::new(json.to_string()) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null_mut(),
                }
            }
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Open an audio input device
///
/// # Arguments
/// * `input_id` - Input ID
/// * `device_id` - Device ID (null for default)
/// * `sample_rate` - Sample rate (0 for default)
/// * `channels` - Number of channels (0 for default)
///
/// # Returns
/// 0 on success, negative on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_audio_input_open(
    input_id: u32,
    device_id: *const c_char,
    sample_rate: u32,
    channels: u32,
) -> i32 {
    let device_str = if device_id.is_null() {
        None
    } else {
        match CStr::from_ptr(device_id).to_str() {
            Ok(s) => Some(s),
            Err(_) => return -1,
        }
    };

    let config = AudioInputConfig {
        sample_rate: if sample_rate == 0 { 44100 } else { sample_rate },
        channels: if channels == 0 { 1 } else { channels },
        ..Default::default()
    };

    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.open(device_str, &config) {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Start capturing audio
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_start(input_id: u32) -> i32 {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.start() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Stop capturing audio
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_stop(input_id: u32) -> i32 {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.stop() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Close the audio input device
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_close(input_id: u32) {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        input.close();
    }
}

/// Get audio input state
///
/// # Returns
/// 0=Idle, 1=RequestingPermission, 2=Ready, 3=Capturing, 4=Stopped, 5=Error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_get_state(input_id: u32) -> i32 {
    let inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        input.state().as_i32()
    } else {
        -2
    }
}

/// Get current audio input level (0.0 - 1.0 RMS)
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_audio_input_get_level(input_id: u32) -> f32 {
    let mut inputs = AUDIO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        // Call update() to read samples from the microphone
        input.update();
        input.level()
    } else {
        0.0
    }
}

// ============================================================================
// Video Input (Camera) FFI
// ============================================================================

use crate::video::input::{VideoInput, VideoInputConfig, VideoInputState};

lazy_static::lazy_static! {
    /// Global video input storage
    static ref VIDEO_INPUTS: std::sync::Mutex<std::collections::HashMap<u32, VideoInput>> = std::sync::Mutex::new(std::collections::HashMap::new());
    static ref NEXT_VIDEO_INPUT_ID: std::sync::Mutex<u32> = std::sync::Mutex::new(1);
}

/// Create a new video input (camera)
///
/// # Returns
/// A unique input ID (always positive), or 0 on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_create() -> u32 {
    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    let mut next_id = NEXT_VIDEO_INPUT_ID.lock().unwrap();

    let input_id = *next_id;
    *next_id += 1;

    inputs.insert(input_id, VideoInput::new());
    input_id
}

/// Destroy a video input and free resources
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_destroy(input_id: u32) {
    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(mut input) = inputs.remove(&input_id) {
        input.close();
    }
}

/// Request camera permission
///
/// # Returns
/// 0 on success, 1 if permission needs to be granted, negative on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_request_permission(input_id: u32) -> i32 {
    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.request_permission() {
            Ok(()) => 0,
            Err(_) => 1,
        }
    } else {
        -2
    }
}

/// Check if camera permission is granted
///
/// # Returns
/// 1 if granted, 0 if not
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_has_permission(input_id: u32) -> i32 {
    let inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        if input.has_permission() { 1 } else { 0 }
    } else {
        0
    }
}

/// List available video input devices (cameras)
/// Returns a JSON array of device info, or null on error
/// Caller must free the returned string with centered_free_string
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_list_devices(input_id: u32) -> *mut c_char {
    let inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        match input.list_devices() {
            Ok(devices) => {
                let json = serde_json::json!(devices.iter().map(|d| {
                    serde_json::json!({
                        "id": d.id,
                        "name": d.name,
                        "position": d.position.as_i32(),
                        "is_default": d.is_default,
                        "resolutions": d.resolutions,
                    })
                }).collect::<Vec<_>>());
                match CString::new(json.to_string()) {
                    Ok(s) => s.into_raw(),
                    Err(_) => ptr::null_mut(),
                }
            }
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Open a video input device (camera)
///
/// # Arguments
/// * `input_id` - Input ID
/// * `device_id` - Device ID (null for default)
/// * `width` - Preferred width (0 for default)
/// * `height` - Preferred height (0 for default)
/// * `frame_rate` - Preferred frame rate (0 for default)
///
/// # Returns
/// 0 on success, negative on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_video_input_open(
    input_id: u32,
    device_id: *const c_char,
    width: u32,
    height: u32,
    frame_rate: u32,
) -> i32 {
    let device_str = if device_id.is_null() {
        None
    } else {
        match CStr::from_ptr(device_id).to_str() {
            Ok(s) => Some(s),
            Err(_) => return -1,
        }
    };

    let config = VideoInputConfig {
        width: if width == 0 { 1280 } else { width },
        height: if height == 0 { 720 } else { height },
        frame_rate: if frame_rate == 0 { 30 } else { frame_rate },
        ..Default::default()
    };

    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.open(device_str, &config) {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Start capturing video
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_start(input_id: u32) -> i32 {
    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.start() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Stop capturing video
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_stop(input_id: u32) -> i32 {
    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        match input.stop() {
            Ok(()) => 0,
            Err(_) => -3,
        }
    } else {
        -2
    }
}

/// Close the video input device
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_close(input_id: u32) {
    let mut inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get_mut(&input_id) {
        input.close();
    }
}

/// Get video input state
///
/// # Returns
/// 0=Idle, 1=RequestingPermission, 2=Ready, 3=Capturing, 4=Stopped, 5=Error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_get_state(input_id: u32) -> i32 {
    let inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        input.state().as_i32()
    } else {
        -2
    }
}

/// Get video input dimensions
///
/// # Returns
/// Width in the high 16 bits, height in the low 16 bits, or 0 on error
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_get_dimensions(input_id: u32, width_out: *mut u32, height_out: *mut u32) -> i32 {
    let inputs = VIDEO_INPUTS.lock().unwrap();
    if let Some(input) = inputs.get(&input_id) {
        if let Some((w, h)) = input.dimensions() {
            unsafe {
                if !width_out.is_null() {
                    *width_out = w;
                }
                if !height_out.is_null() {
                    *height_out = h;
                }
            }
            0
        } else {
            -3
        }
    } else {
        -2
    }
}

/// Get latest video frame as a GPU texture
///
/// This function gets the latest frame from the video input, uploads it to a GPU
/// texture (creating one if needed), and returns the texture ID.
/// If an existing_texture_id is provided and valid, it will be reused/updated.
///
/// # Returns
/// Texture ID (positive), or negative error code:
/// - -1: Backend not initialized
/// - -2: Input not found
/// - -3: No frame available
/// - -4: Failed to upload to GPU
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn centered_video_input_get_frame_texture(input_id: u32, existing_texture_id: u32) -> i32 {
    // First update the input to capture new frames, then get the latest frame
    let frame = {
        let mut inputs = VIDEO_INPUTS.lock().unwrap();
        if let Some(input) = inputs.get_mut(&input_id) {
            // Call update() to read frames from the camera
            input.update();
            input.latest_frame()
        } else {
            return -2; // Input not found
        }
    };

    let frame = match frame {
        Some(f) => f,
        None => return -3, // No frame available
    };

    // Only convert BGRA to RGBA if the frame is in BGRA format
    // Windows camera already outputs RGBA from process_sample()
    let rgba_data = if frame.pixel_format == crate::video::input::PixelFormat::BGRA {
        let mut data = frame.data.clone();
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // Swap B and R
        }
        data
    } else {
        // Don't clone - take ownership directly
        frame.data
    };

    // Create LoadedImage for the backend
    let loaded_image = crate::image::LoadedImage {
        width: frame.width,
        height: frame.height,
        data: rgba_data,
    };

    // Get backend and upload/update texture
    let mut backend_guard = get_backend().lock().unwrap();
    let backend = match backend_guard.as_mut() {
        Some(b) => b,
        None => return -1, // Backend not initialized
    };

    // If we have an existing texture, try to update it in-place for better performance
    // This avoids creating/destroying GPU textures every frame
    if existing_texture_id > 0 {
        match backend.update_texture(existing_texture_id, &loaded_image) {
            Ok(texture_id) => texture_id as i32,
            Err(_) => -4, // Upload failed
        }
    } else {
        // First frame - create new texture
        match backend.load_image(&loaded_image) {
            Ok(texture_id) => texture_id as i32,
            Err(_) => -4, // Upload failed
        }
    }
}

// ============================================================================
// Batched Binary Commands (SharedMemory Transport)
// ============================================================================

/// Command types for the binary protocol (must match Go side exactly).
/// Using u16 with 256-spacing between groups to allow room for growth.
/// Each category has 256 slots available.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchCommandType {
    // Text measurement commands (0x0000 - 0x00FF)
    MeasureText = 0x0000,
    MeasureTextBatch = 0x0001,
    MeasureTextToCursor = 0x0002,
    MeasureTextWithFont = 0x0003,

    // Image commands (0x0100 - 0x01FF)
    LoadImage = 0x0100,
    LoadImageFile = 0x0101,
    UnloadImage = 0x0102,
    GetTextureSize = 0x0103,

    // Render commands (0x0200 - 0x02FF)
    RenderFrame = 0x0200,

    // System queries (0x0300 - 0x03FF)
    GetScaleFactor = 0x0300,
    GetDarkMode = 0x0301,

    // Audio playback commands (0x0400 - 0x04FF)
    AudioCreate = 0x0400,
    AudioDestroy = 0x0401,
    AudioLoadURL = 0x0402,
    AudioLoadFile = 0x0403,
    AudioPlay = 0x0404,
    AudioPause = 0x0405,
    AudioStop = 0x0406,
    AudioSeek = 0x0407,
    AudioSetVolume = 0x0408,
    AudioSetLooping = 0x0409,
    AudioGetState = 0x040A,
    AudioGetTime = 0x040B,
    AudioGetInfo = 0x040C,
    AudioGetVolume = 0x040D,
    AudioIsLooping = 0x040E,
    AudioUpdate = 0x040F,

    // Audio input commands (0x0500 - 0x05FF)
    AudioInputCreate = 0x0500,
    AudioInputDestroy = 0x0501,
    AudioInputRequestPermission = 0x0502,
    AudioInputHasPermission = 0x0503,
    AudioInputListDevices = 0x0504,
    AudioInputOpen = 0x0505,
    AudioInputStart = 0x0506,
    AudioInputStop = 0x0507,
    AudioInputClose = 0x0508,
    AudioInputGetLevel = 0x0509,
    AudioInputGetState = 0x050A,

    // Video playback commands (0x0600 - 0x06FF)
    VideoCreate = 0x0600,
    VideoDestroy = 0x0601,
    VideoLoadURL = 0x0602,
    VideoLoadFile = 0x0603,
    VideoInitStream = 0x0604,
    VideoPushFrame = 0x0605,
    VideoPlay = 0x0606,
    VideoPause = 0x0607,
    VideoSeek = 0x0608,
    VideoSetLooping = 0x0609,
    VideoSetMuted = 0x060A,
    VideoSetVolume = 0x060B,
    VideoGetState = 0x060C,
    VideoGetTime = 0x060D,
    VideoGetInfo = 0x060E,
    VideoUpdate = 0x060F,
    VideoGetTextureID = 0x0610,

    // Video input commands (0x0700 - 0x07FF)
    VideoInputCreate = 0x0700,
    VideoInputDestroy = 0x0701,
    VideoInputRequestPermission = 0x0702,
    VideoInputHasPermission = 0x0703,
    VideoInputListDevices = 0x0704,
    VideoInputOpen = 0x0705,
    VideoInputStart = 0x0706,
    VideoInputStop = 0x0707,
    VideoInputClose = 0x0708,
    VideoInputGetState = 0x0709,
    VideoInputGetDimensions = 0x070A,
    VideoInputGetFrameTexture = 0x070B,

    // Clipboard commands (0x0800 - 0x08FF)
    ClipboardGet = 0x0800,
    ClipboardSet = 0x0801,

    // App lifecycle (0xFF00 - 0xFFFF)
    RequestRedraw = 0xFF00,
    RequestExit = 0xFF01,
}

/// Response types for the binary protocol.
/// Using u8 since we don't need as many response types.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchResponseType {
    Success = 0,
    Error = 1,
    Float32 = 2,
    Int32 = 3,
    Uint32 = 4,
    Uint64 = 5,
    String = 6,
    Bytes = 7,
    Bool = 8,
    Float32Array = 9,
    Uint32Pair = 10,     // For texture size, dimensions
    Uint32Triple = 11,   // For audio info (duration, sample_rate, channels)
    VideoInfo = 12,      // For video info (width, height, duration)
}

/// Execute a batch of binary commands.
///
/// Buffer format (request):
///   count: u32           - Number of commands
///   For each command:
///     cmd_type: u16      - Command type (little-endian)
///     payload_len: u32   - Length of payload
///     payload: [u8]      - Command-specific payload
///
/// Buffer format (response):
///   count: u32           - Number of responses
///   For each response:
///     resp_type: u8      - Response type
///     payload_len: u32   - Length of payload
///     payload: [u8]      - Response-specific payload
///
/// # Safety
/// - request_ptr must point to valid memory of at least request_len bytes
/// - response_ptr must point to valid memory of at least response_capacity bytes
/// - response_len_out must point to valid u32
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn centered_execute_batch(
    request_ptr: *const u8,
    request_len: usize,
    response_ptr: *mut u8,
    response_capacity: usize,
    response_len_out: *mut usize,
) -> i32 {
    if request_ptr.is_null() || response_ptr.is_null() || response_len_out.is_null() {
        return -1;
    }

    if request_len < 4 {
        return -1;
    }

    let request = std::slice::from_raw_parts(request_ptr, request_len);
    let response = std::slice::from_raw_parts_mut(response_ptr, response_capacity);

    // Read command count
    let cmd_count = u32::from_le_bytes([request[0], request[1], request[2], request[3]]) as usize;

    // Process commands and build responses
    let mut req_offset = 4;
    let mut resp_offset = 4; // Reserve space for response count

    for _ in 0..cmd_count {
        if req_offset + 6 > request_len {
            break;
        }

        // Read command type (u16)
        let cmd_type = u16::from_le_bytes([request[req_offset], request[req_offset + 1]]);
        req_offset += 2;

        let payload_len = u32::from_le_bytes([
            request[req_offset],
            request[req_offset + 1],
            request[req_offset + 2],
            request[req_offset + 3],
        ]) as usize;
        req_offset += 4;

        if req_offset + payload_len > request_len {
            break;
        }

        let payload = &request[req_offset..req_offset + payload_len];
        req_offset += payload_len;

        // Execute command and write response
        let (resp_type, resp_payload) = execute_single_command(cmd_type, payload);

        // Write response type
        if resp_offset + 5 + resp_payload.len() > response_capacity {
            // Response buffer full - return error
            *response_len_out = 0;
            return -2;
        }

        response[resp_offset] = resp_type as u8;
        resp_offset += 1;

        // Write response payload length
        let resp_payload_len = resp_payload.len() as u32;
        response[resp_offset..resp_offset + 4].copy_from_slice(&resp_payload_len.to_le_bytes());
        resp_offset += 4;

        // Write response payload
        response[resp_offset..resp_offset + resp_payload.len()].copy_from_slice(&resp_payload);
        resp_offset += resp_payload.len();
    }

    // Write response count at the beginning
    response[0..4].copy_from_slice(&(cmd_count as u32).to_le_bytes());

    *response_len_out = resp_offset;
    0
}

/// Execute a single command and return the response type and payload.
fn execute_single_command(cmd_type: u16, payload: &[u8]) -> (BatchResponseType, Vec<u8>) {
    match cmd_type {
        // MeasureText (0x0000)
        0x0000 => {
            // Payload: text_len(4) + text + font_len(4) + font + size(4)
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }

            let text_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            let mut offset = 4;

            if offset + text_len + 4 > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }

            let text = match std::str::from_utf8(&payload[offset..offset + text_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            offset += text_len;

            let font_len = u32::from_le_bytes([
                payload[offset],
                payload[offset + 1],
                payload[offset + 2],
                payload[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + font_len + 4 > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }

            let font_name = match std::str::from_utf8(&payload[offset..offset + font_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            offset += font_len;

            let size_bits = u32::from_le_bytes([
                payload[offset],
                payload[offset + 1],
                payload[offset + 2],
                payload[offset + 3],
            ]);
            let font_size = f32::from_bits(size_bits);

            // Measure text using font manager (same as centered_measure_text)
            let font_manager = get_font_manager();
            let width = match font_manager.lock() {
                Ok(mut manager) => {
                    let descriptor = FontDescriptor::system(font_name, 400, FontStyle::Normal, font_size);
                    match manager.load_font(&descriptor) {
                        Ok(font) => font.measure_text(text),
                        Err(_) => 0.0,
                    }
                }
                Err(_) => 0.0,
            };
            (BatchResponseType::Float32, width.to_bits().to_le_bytes().to_vec())
        }

        // MeasureTextBatch (0x0001) - Measure multiple text strings in one call
        0x0001 => {
            // Payload: count(4) + [text_len(4) + text + font_len(4) + font + size(4)]...
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }

            let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            let mut offset = 4;

            // Pre-allocate result buffer: count(4) + widths(count * 4)
            let mut result = vec![0u8; 4 + count * 4];
            result[0..4].copy_from_slice(&(count as u32).to_le_bytes());

            let font_manager = get_font_manager();
            let mut manager = match font_manager.lock() {
                Ok(m) => m,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };

            for i in 0..count {
                // Parse text
                if offset + 4 > payload.len() {
                    return (BatchResponseType::Error, vec![]);
                }
                let text_len = u32::from_le_bytes([
                    payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3],
                ]) as usize;
                offset += 4;

                if offset + text_len > payload.len() {
                    return (BatchResponseType::Error, vec![]);
                }
                let text = match std::str::from_utf8(&payload[offset..offset + text_len]) {
                    Ok(s) => s,
                    Err(_) => return (BatchResponseType::Error, vec![]),
                };
                offset += text_len;

                // Parse font name
                if offset + 4 > payload.len() {
                    return (BatchResponseType::Error, vec![]);
                }
                let font_len = u32::from_le_bytes([
                    payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3],
                ]) as usize;
                offset += 4;

                if offset + font_len > payload.len() {
                    return (BatchResponseType::Error, vec![]);
                }
                let font_name = match std::str::from_utf8(&payload[offset..offset + font_len]) {
                    Ok(s) => s,
                    Err(_) => return (BatchResponseType::Error, vec![]),
                };
                offset += font_len;

                // Parse font size
                if offset + 4 > payload.len() {
                    return (BatchResponseType::Error, vec![]);
                }
                let size_bits = u32::from_le_bytes([
                    payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3],
                ]);
                let font_size = f32::from_bits(size_bits);
                offset += 4;

                // Measure text
                let width = {
                    let descriptor = FontDescriptor::system(font_name, 400, FontStyle::Normal, font_size);
                    match manager.load_font(&descriptor) {
                        Ok(font) => font.measure_text(text),
                        Err(_) => 0.0,
                    }
                };

                // Store result
                let result_offset = 4 + i * 4;
                result[result_offset..result_offset + 4].copy_from_slice(&width.to_bits().to_le_bytes());
            }

            (BatchResponseType::Float32Array, result)
        }

        // GetScaleFactor (0x0300)
        0x0300 => {
            // Same implementation as centered_get_scale_factor
            let backend_lock = get_backend();
            let scale = match backend_lock.lock() {
                Ok(guard) => {
                    if let Some(backend) = guard.as_ref() {
                        backend.scale_factor()
                    } else {
                        1.0
                    }
                }
                Err(_) => 1.0,
            };
            let scale_f32 = scale as f32;
            (BatchResponseType::Float32, scale_f32.to_bits().to_le_bytes().to_vec())
        }

        // GetDarkMode (0x0301)
        0x0301 => {
            // Same implementation as centered_system_dark_mode
            let dark_mode = centered_system_dark_mode();
            (BatchResponseType::Bool, vec![if dark_mode == 1 { 1 } else { 0 }])
        }

        // LoadImage (0x0100) - payload is raw image bytes
        0x0100 => {
            // Decode the image first
            let loaded_image = match LoadedImage::from_bytes(payload) {
                Ok(img) => img,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };

            let backend_lock = get_backend();
            match backend_lock.lock() {
                Ok(mut guard) => {
                    if let Some(backend) = guard.as_mut() {
                        match backend.load_image(&loaded_image) {
                            Ok(id) => {
                                let mut resp = vec![0u8; 4];
                                resp[0..4].copy_from_slice(&id.to_le_bytes());
                                (BatchResponseType::Uint32, resp)
                            }
                            Err(_) => (BatchResponseType::Error, vec![]),
                        }
                    } else {
                        (BatchResponseType::Error, vec![])
                    }
                }
                Err(_) => (BatchResponseType::Error, vec![]),
            }
        }

        // LoadImageFile (0x0101) - payload is path string
        0x0101 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let path_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            if 4 + path_len > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let path = match std::str::from_utf8(&payload[4..4 + path_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };

            // Load the image from file
            let loaded_image = match LoadedImage::from_file(path) {
                Ok(img) => img,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };

            let backend_lock = get_backend();
            match backend_lock.lock() {
                Ok(mut guard) => {
                    if let Some(backend) = guard.as_mut() {
                        match backend.load_image(&loaded_image) {
                            Ok(id) => {
                                let mut resp = vec![0u8; 4];
                                resp[0..4].copy_from_slice(&id.to_le_bytes());
                                (BatchResponseType::Uint32, resp)
                            }
                            Err(_) => (BatchResponseType::Error, vec![]),
                        }
                    } else {
                        (BatchResponseType::Error, vec![])
                    }
                }
                Err(_) => (BatchResponseType::Error, vec![]),
            }
        }

        // UnloadImage (0x0102) - payload is texture_id (u32)
        0x0102 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let texture_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

            let backend_lock = get_backend();
            match backend_lock.lock() {
                Ok(mut guard) => {
                    if let Some(backend) = guard.as_mut() {
                        backend.unload_image(texture_id);
                    }
                }
                Err(_) => {}
            }
            (BatchResponseType::Success, vec![])
        }

        // GetTextureSize (0x0103) - payload is texture_id (u32)
        0x0103 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let texture_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

            let backend_lock = get_backend();
            match backend_lock.lock() {
                Ok(guard) => {
                    if let Some(backend) = guard.as_ref() {
                        if let Some((w, h)) = backend.get_texture_size(texture_id) {
                            let mut resp = vec![0u8; 8];
                            resp[0..4].copy_from_slice(&w.to_le_bytes());
                            resp[4..8].copy_from_slice(&h.to_le_bytes());
                            (BatchResponseType::Uint32Pair, resp)
                        } else {
                            (BatchResponseType::Error, vec![])
                        }
                    } else {
                        (BatchResponseType::Error, vec![])
                    }
                }
                Err(_) => (BatchResponseType::Error, vec![]),
            }
        }

        // ClipboardGet (0x0800)
        0x0800 => {
            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSPasteboard;
                use cocoa::base::nil;

                unsafe {
                    let pasteboard: *mut objc::runtime::Object = NSPasteboard::generalPasteboard(nil);
                    let nsstring_class = class!(NSString);
                    let string_type: *mut objc::runtime::Object = msg_send![nsstring_class, stringWithUTF8String: "public.utf8-plain-text\0".as_ptr()];
                    let content: *mut objc::runtime::Object = msg_send![pasteboard, stringForType: string_type];

                    if content.is_null() {
                        return (BatchResponseType::String, vec![0, 0, 0, 0]);
                    }

                    let c_str: *const i8 = msg_send![content, UTF8String];
                    if c_str.is_null() {
                        return (BatchResponseType::String, vec![0, 0, 0, 0]);
                    }

                    let rust_str = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
                    let mut resp = vec![0u8; 4 + rust_str.len()];
                    resp[0..4].copy_from_slice(&(rust_str.len() as u32).to_le_bytes());
                    resp[4..].copy_from_slice(rust_str.as_bytes());
                    (BatchResponseType::String, resp)
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                (BatchResponseType::String, vec![0, 0, 0, 0])
            }
        }

        // ClipboardSet (0x0801) - payload is string
        0x0801 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let text_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            if 4 + text_len > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let text = match std::str::from_utf8(&payload[4..4 + text_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };

            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSPasteboard;
                use cocoa::base::nil;
                use cocoa::foundation::NSString;

                unsafe {
                    let pasteboard: *mut objc::runtime::Object = NSPasteboard::generalPasteboard(nil);
                    let _: () = msg_send![pasteboard, clearContents];

                    let ns_string = NSString::alloc(nil).init_str(text);
                    let nsstring_class = class!(NSString);
                    let string_type: *mut objc::runtime::Object = msg_send![nsstring_class, stringWithUTF8String: "public.utf8-plain-text\0".as_ptr()];
                    let _: bool = msg_send![pasteboard, setString: ns_string forType: string_type];
                }
            }
            (BatchResponseType::Success, vec![])
        }

        // RequestRedraw (0xFF00)
        0xFF00 => {
            // Call the existing request_redraw function
            unsafe { centered_app_request_redraw(); }
            (BatchResponseType::Success, vec![])
        }

        // RequestExit (0xFF01)
        0xFF01 => {
            // Call the existing request_exit function
            unsafe { centered_app_request_exit(); }
            (BatchResponseType::Success, vec![])
        }

        // ========================================================================
        // Audio Playback Commands (0x0400 - 0x040F)
        // ========================================================================

        // AudioCreate (0x0400) - no payload, returns player_id
        0x0400 => {
            let player_id = centered_audio_create();
            (BatchResponseType::Uint32, player_id.to_le_bytes().to_vec())
        }

        // AudioDestroy (0x0401) - payload: player_id (u32)
        0x0401 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            centered_audio_destroy(player_id);
            (BatchResponseType::Success, vec![])
        }

        // AudioLoadURL (0x0402) - payload: player_id (u32) + url_len (u32) + url
        0x0402 => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let url_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            if 8 + url_len > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let url = match std::str::from_utf8(&payload[8..8 + url_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let url_cstring = match std::ffi::CString::new(url) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let result = unsafe { centered_audio_load_url(player_id, url_cstring.as_ptr()) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioLoadFile (0x0403) - payload: player_id (u32) + path_len (u32) + path
        0x0403 => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let path_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            if 8 + path_len > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let path = match std::str::from_utf8(&payload[8..8 + path_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let path_cstring = match std::ffi::CString::new(path) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let result = unsafe { centered_audio_load_file(player_id, path_cstring.as_ptr()) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioPlay (0x0404) - payload: player_id (u32)
        0x0404 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_play(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioPause (0x0405) - payload: player_id (u32)
        0x0405 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_pause(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioStop (0x0406) - payload: player_id (u32)
        0x0406 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_stop(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioSeek (0x0407) - payload: player_id (u32) + timestamp_ms (u64)
        0x0407 => {
            if payload.len() < 12 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let timestamp_ms = u64::from_le_bytes([
                payload[4], payload[5], payload[6], payload[7],
                payload[8], payload[9], payload[10], payload[11],
            ]);
            let result = centered_audio_seek(player_id, timestamp_ms);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioSetVolume (0x0408) - payload: player_id (u32) + volume (f32)
        0x0408 => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let volume = f32::from_bits(u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]));
            let result = centered_audio_set_volume(player_id, volume);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioSetLooping (0x0409) - payload: player_id (u32) + looping (u8)
        0x0409 => {
            if payload.len() < 5 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let looping = payload[4] != 0;
            let result = centered_audio_set_looping(player_id, looping);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioGetState (0x040A) - payload: player_id (u32)
        0x040A => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let state = centered_audio_get_state(player_id);
            (BatchResponseType::Int32, state.to_le_bytes().to_vec())
        }

        // AudioGetTime (0x040B) - payload: player_id (u32)
        0x040B => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let time = centered_audio_get_time(player_id);
            (BatchResponseType::Uint64, time.to_le_bytes().to_vec())
        }

        // AudioGetInfo (0x040C) - payload: player_id (u32)
        0x040C => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let mut duration_ms: u64 = 0;
            let mut sample_rate: u32 = 0;
            let mut channels: u32 = 0;
            let result = unsafe {
                centered_audio_get_info(player_id, &mut duration_ms, &mut sample_rate, &mut channels)
            };
            if result == 0 {
                let mut resp = vec![0u8; 16];
                resp[0..8].copy_from_slice(&duration_ms.to_le_bytes());
                resp[8..12].copy_from_slice(&sample_rate.to_le_bytes());
                resp[12..16].copy_from_slice(&channels.to_le_bytes());
                (BatchResponseType::Uint32Triple, resp)
            } else {
                (BatchResponseType::Error, vec![])
            }
        }

        // AudioGetVolume (0x040D) - payload: player_id (u32)
        0x040D => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let volume = centered_audio_get_volume(player_id);
            (BatchResponseType::Float32, volume.to_bits().to_le_bytes().to_vec())
        }

        // AudioIsLooping (0x040E) - payload: player_id (u32)
        0x040E => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let is_looping = centered_audio_is_looping(player_id);
            (BatchResponseType::Bool, vec![if is_looping == 1 { 1 } else { 0 }])
        }

        // AudioUpdate (0x040F) - payload: player_id (u32)
        0x040F => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_update(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // ========================================================================
        // Audio Input Commands (0x0500 - 0x050A)
        // ========================================================================

        // AudioInputCreate (0x0500) - no payload, returns input_id
        0x0500 => {
            let input_id = centered_audio_input_create();
            (BatchResponseType::Uint32, input_id.to_le_bytes().to_vec())
        }

        // AudioInputDestroy (0x0501) - payload: input_id (u32)
        0x0501 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            centered_audio_input_destroy(input_id);
            (BatchResponseType::Success, vec![])
        }

        // AudioInputRequestPermission (0x0502) - payload: input_id (u32)
        0x0502 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_input_request_permission(input_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioInputHasPermission (0x0503) - payload: input_id (u32)
        0x0503 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_input_has_permission(input_id);
            (BatchResponseType::Bool, vec![if result == 1 { 1 } else { 0 }])
        }

        // AudioInputListDevices (0x0504) - payload: input_id (u32)
        0x0504 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let devices_ptr = centered_audio_input_list_devices(input_id);
            if devices_ptr.is_null() {
                return (BatchResponseType::String, vec![0, 0, 0, 0]);
            }
            unsafe {
                let devices_str = std::ffi::CStr::from_ptr(devices_ptr).to_string_lossy().into_owned();
                centered_free_string(devices_ptr);
                let mut resp = vec![0u8; 4 + devices_str.len()];
                resp[0..4].copy_from_slice(&(devices_str.len() as u32).to_le_bytes());
                resp[4..].copy_from_slice(devices_str.as_bytes());
                (BatchResponseType::String, resp)
            }
        }

        // AudioInputOpen (0x0505) - payload: input_id (u32) + device_id_len (u32) + device_id + sample_rate (u32) + channels (u32)
        0x0505 => {
            if payload.len() < 16 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let device_id_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            if 8 + device_id_len + 8 > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let device_id = if device_id_len == 0 {
                None
            } else {
                match std::str::from_utf8(&payload[8..8 + device_id_len]) {
                    Ok(s) => Some(s),
                    Err(_) => return (BatchResponseType::Error, vec![]),
                }
            };
            let offset = 8 + device_id_len;
            let sample_rate = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]);
            let channels = u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]);

            let device_cstring = device_id.map(|s| std::ffi::CString::new(s).ok()).flatten();
            let device_ptr = device_cstring.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null());
            let result = unsafe { centered_audio_input_open(input_id, device_ptr, sample_rate, channels) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioInputStart (0x0506) - payload: input_id (u32)
        0x0506 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_input_start(input_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioInputStop (0x0507) - payload: input_id (u32)
        0x0507 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_audio_input_stop(input_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // AudioInputClose (0x0508) - payload: input_id (u32)
        0x0508 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            centered_audio_input_close(input_id);
            (BatchResponseType::Success, vec![])
        }

        // AudioInputGetLevel (0x0509) - payload: input_id (u32)
        0x0509 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let level = centered_audio_input_get_level(input_id);
            (BatchResponseType::Float32, level.to_bits().to_le_bytes().to_vec())
        }

        // AudioInputGetState (0x050A) - payload: input_id (u32)
        0x050A => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let state = centered_audio_input_get_state(input_id);
            (BatchResponseType::Int32, state.to_le_bytes().to_vec())
        }

        // ========================================================================
        // Video Playback Commands (0x0600 - 0x0610)
        // ========================================================================

        // VideoCreate (0x0600) - no payload, returns player_id
        0x0600 => {
            let player_id = centered_video_create();
            (BatchResponseType::Uint32, player_id.to_le_bytes().to_vec())
        }

        // VideoDestroy (0x0601) - payload: player_id (u32)
        0x0601 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            centered_video_destroy(player_id);
            (BatchResponseType::Success, vec![])
        }

        // VideoLoadURL (0x0602) - payload: player_id (u32) + url_len (u32) + url
        0x0602 => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let url_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            if 8 + url_len > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let url = match std::str::from_utf8(&payload[8..8 + url_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let url_cstring = match std::ffi::CString::new(url) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let result = unsafe { centered_video_load_url(player_id, url_cstring.as_ptr()) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoLoadFile (0x0603) - payload: player_id (u32) + path_len (u32) + path
        0x0603 => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let path_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            if 8 + path_len > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let path = match std::str::from_utf8(&payload[8..8 + path_len]) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let path_cstring = match std::ffi::CString::new(path) {
                Ok(s) => s,
                Err(_) => return (BatchResponseType::Error, vec![]),
            };
            let result = unsafe { centered_video_load_file(player_id, path_cstring.as_ptr()) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoInitStream (0x0604) - payload: player_id (u32) + width (u32) + height (u32)
        0x0604 => {
            if payload.len() < 12 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let width = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            let height = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
            let result = centered_video_init_stream(player_id, width, height);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoPushFrame (0x0605) - payload: player_id (u32) + width (u32) + height (u32) + timestamp_ms (u64) + frame_data
        0x0605 => {
            if payload.len() < 20 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let width = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            let height = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
            let timestamp_ms = u64::from_le_bytes([
                payload[12], payload[13], payload[14], payload[15],
                payload[16], payload[17], payload[18], payload[19],
            ]);
            let frame_data = &payload[20..];
            let result = unsafe { centered_video_push_frame(player_id, width, height, frame_data.as_ptr(), frame_data.len(), timestamp_ms) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoPlay (0x0606) - payload: player_id (u32)
        0x0606 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_play(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoPause (0x0607) - payload: player_id (u32)
        0x0607 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_pause(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoSeek (0x0608) - payload: player_id (u32) + timestamp_ms (u64)
        0x0608 => {
            if payload.len() < 12 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let timestamp_ms = u64::from_le_bytes([
                payload[4], payload[5], payload[6], payload[7],
                payload[8], payload[9], payload[10], payload[11],
            ]);
            let result = centered_video_seek(player_id, timestamp_ms);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoSetLooping (0x0609) - payload: player_id (u32) + looping (u8)
        0x0609 => {
            if payload.len() < 5 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let looping = payload[4] != 0;
            let result = centered_video_set_looping(player_id, looping);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoSetMuted (0x060A) - payload: player_id (u32) + muted (u8)
        0x060A => {
            if payload.len() < 5 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let muted = payload[4] != 0;
            let result = centered_video_set_muted(player_id, muted);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoSetVolume (0x060B) - payload: player_id (u32) + volume (f32)
        0x060B => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let volume = f32::from_bits(u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]));
            let result = centered_video_set_volume(player_id, volume);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoGetState (0x060C) - payload: player_id (u32)
        0x060C => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let state = centered_video_get_state(player_id);
            (BatchResponseType::Int32, state.to_le_bytes().to_vec())
        }

        // VideoGetTime (0x060D) - payload: player_id (u32)
        0x060D => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let time = centered_video_get_time(player_id);
            (BatchResponseType::Uint64, time.to_le_bytes().to_vec())
        }

        // VideoGetInfo (0x060E) - payload: player_id (u32)
        0x060E => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let mut width: u32 = 0;
            let mut height: u32 = 0;
            let mut duration_ms: u64 = 0;
            let result = unsafe {
                centered_video_get_info(player_id, &mut width, &mut height, &mut duration_ms)
            };
            if result == 0 {
                let mut resp = vec![0u8; 16];
                resp[0..4].copy_from_slice(&width.to_le_bytes());
                resp[4..8].copy_from_slice(&height.to_le_bytes());
                resp[8..16].copy_from_slice(&duration_ms.to_le_bytes());
                (BatchResponseType::VideoInfo, resp)
            } else {
                (BatchResponseType::Error, vec![])
            }
        }

        // VideoUpdate (0x060F) - payload: player_id (u32)
        0x060F => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_update(player_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoGetTextureID (0x0610) - payload: player_id (u32)
        0x0610 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let player_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let texture_id = centered_video_get_texture_id(player_id);
            (BatchResponseType::Uint32, texture_id.to_le_bytes().to_vec())
        }

        // ========================================================================
        // Video Input Commands (0x0700 - 0x070A)
        // ========================================================================

        // VideoInputCreate (0x0700) - no payload, returns input_id
        0x0700 => {
            let input_id = centered_video_input_create();
            (BatchResponseType::Uint32, input_id.to_le_bytes().to_vec())
        }

        // VideoInputDestroy (0x0701) - payload: input_id (u32)
        0x0701 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            centered_video_input_destroy(input_id);
            (BatchResponseType::Success, vec![])
        }

        // VideoInputRequestPermission (0x0702) - payload: input_id (u32)
        0x0702 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_input_request_permission(input_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoInputHasPermission (0x0703) - payload: input_id (u32)
        0x0703 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_input_has_permission(input_id);
            (BatchResponseType::Bool, vec![if result == 1 { 1 } else { 0 }])
        }

        // VideoInputListDevices (0x0704) - payload: input_id (u32)
        0x0704 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let devices_ptr = centered_video_input_list_devices(input_id);
            if devices_ptr.is_null() {
                return (BatchResponseType::String, vec![0, 0, 0, 0]);
            }
            unsafe {
                let devices_str = std::ffi::CStr::from_ptr(devices_ptr).to_string_lossy().into_owned();
                centered_free_string(devices_ptr);
                let mut resp = vec![0u8; 4 + devices_str.len()];
                resp[0..4].copy_from_slice(&(devices_str.len() as u32).to_le_bytes());
                resp[4..].copy_from_slice(devices_str.as_bytes());
                (BatchResponseType::String, resp)
            }
        }

        // VideoInputOpen (0x0705) - payload: input_id (u32) + device_id_len (u32) + device_id + width (u32) + height (u32) + fps (u32)
        0x0705 => {
            if payload.len() < 20 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let device_id_len = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]) as usize;
            if 8 + device_id_len + 12 > payload.len() {
                return (BatchResponseType::Error, vec![]);
            }
            let device_id = if device_id_len == 0 {
                None
            } else {
                match std::str::from_utf8(&payload[8..8 + device_id_len]) {
                    Ok(s) => Some(s),
                    Err(_) => return (BatchResponseType::Error, vec![]),
                }
            };
            let offset = 8 + device_id_len;
            let width = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]);
            let height = u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]);
            let fps = u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]);

            let device_cstring = device_id.map(|s| std::ffi::CString::new(s).ok()).flatten();
            let device_ptr = device_cstring.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null());
            let result = unsafe { centered_video_input_open(input_id, device_ptr, width, height, fps) };
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoInputStart (0x0706) - payload: input_id (u32)
        0x0706 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_input_start(input_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoInputStop (0x0707) - payload: input_id (u32)
        0x0707 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let result = centered_video_input_stop(input_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // VideoInputClose (0x0708) - payload: input_id (u32)
        0x0708 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            centered_video_input_close(input_id);
            (BatchResponseType::Success, vec![])
        }

        // VideoInputGetState (0x0709) - payload: input_id (u32)
        0x0709 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let state = centered_video_input_get_state(input_id);
            (BatchResponseType::Int32, state.to_le_bytes().to_vec())
        }

        // VideoInputGetDimensions (0x070A) - payload: input_id (u32)
        0x070A => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let mut width: u32 = 0;
            let mut height: u32 = 0;
            let result = unsafe { centered_video_input_get_dimensions(input_id, &mut width, &mut height) };
            if result == 0 {
                let mut resp = vec![0u8; 8];
                resp[0..4].copy_from_slice(&width.to_le_bytes());
                resp[4..8].copy_from_slice(&height.to_le_bytes());
                (BatchResponseType::Uint32Pair, resp)
            } else {
                (BatchResponseType::Error, vec![])
            }
        }

        // VideoInputGetFrameTexture (0x070B) - payload: input_id (u32) + existing_texture_id (u32)
        0x070B => {
            if payload.len() < 8 {
                return (BatchResponseType::Error, vec![]);
            }
            let input_id = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let existing_texture_id = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            let result = centered_video_input_get_frame_texture(input_id, existing_texture_id);
            (BatchResponseType::Int32, result.to_le_bytes().to_vec())
        }

        // ========================================================================
        // Render Commands (0x0200)
        // ========================================================================

        // RenderFrame (0x0200) - Binary render commands
        // Payload: command_count(4) + [command_type(1) + command_data]...
        //
        // Command types:
        //   0x00 - Clear: r(1) + g(1) + b(1) + a(1)
        //   0x01 - DrawRect: x(4) + y(4) + w(4) + h(4) + color(4) + radii(16) + rotation(4) + flags(1) + [border_w(4) + border_color(4) + border_style(1)] + [gradient_data]
        //   0x02 - DrawText: x(4) + y(4) + text_len(4) + text + font_data + color(4) + layout_data
        //   0x03 - DrawImage: x(4) + y(4) + w(4) + h(4) + texture_id(4) + flags(1) + [source_rect(16)] + radii(16)
        //   0x04 - DrawShadow: x(4) + y(4) + w(4) + h(4) + blur(4) + color(4) + offset_x(4) + offset_y(4) + radii(16)
        //   0x05 - PushClip: x(4) + y(4) + w(4) + h(4)
        //   0x06 - PopClip: (no data)
        //   0x07 - BeginScrollView: x(4) + y(4) + w(4) + h(4) + scroll_x(4) + scroll_y(4) + flags(1) + [content_w(4)] + [content_h(4)]
        //   0x08 - EndScrollView: (no data)
        //   0x09 - SetOpacity: opacity(4)
        0x0200 => {
            if payload.len() < 4 {
                return (BatchResponseType::Error, vec![]);
            }

            let command_count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            let mut offset = 4;
            let mut commands: Vec<RenderCommand> = Vec::with_capacity(command_count);

            for _ in 0..command_count {
                if offset >= payload.len() {
                    return (BatchResponseType::Error, b"unexpected end of payload".to_vec());
                }

                let cmd_type = payload[offset];
                offset += 1;

                match cmd_type {
                    // Clear: r(1) + g(1) + b(1) + a(1)
                    0x00 => {
                        if offset + 4 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let r = payload[offset];
                        let g = payload[offset + 1];
                        let b = payload[offset + 2];
                        let a = payload[offset + 3];
                        offset += 4;
                        commands.push(RenderCommand::Clear(crate::style::Color { r, g, b, a }));
                    }

                    // DrawRect: x(4) + y(4) + w(4) + h(4) + color(4) + radii(16) + rotation(4) + flags(1) + [border] + [gradient]
                    0x01 => {
                        if offset + 41 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let width = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let height = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                        let color = u32::from_le_bytes([payload[offset + 16], payload[offset + 17], payload[offset + 18], payload[offset + 19]]);
                        let r0 = f32::from_bits(u32::from_le_bytes([payload[offset + 20], payload[offset + 21], payload[offset + 22], payload[offset + 23]]));
                        let r1 = f32::from_bits(u32::from_le_bytes([payload[offset + 24], payload[offset + 25], payload[offset + 26], payload[offset + 27]]));
                        let r2 = f32::from_bits(u32::from_le_bytes([payload[offset + 28], payload[offset + 29], payload[offset + 30], payload[offset + 31]]));
                        let r3 = f32::from_bits(u32::from_le_bytes([payload[offset + 32], payload[offset + 33], payload[offset + 34], payload[offset + 35]]));
                        let rotation = f32::from_bits(u32::from_le_bytes([payload[offset + 36], payload[offset + 37], payload[offset + 38], payload[offset + 39]]));
                        let flags = payload[offset + 40];
                        offset += 41;

                        let has_border = (flags & 0x01) != 0;
                        let has_gradient = (flags & 0x02) != 0;

                        let border = if has_border {
                            if offset + 9 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let bw = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                            let bc = u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]);
                            let bs = match payload[offset + 8] {
                                1 => crate::render::BorderStyle::Dashed,
                                2 => crate::render::BorderStyle::Dotted,
                                _ => crate::render::BorderStyle::Solid,
                            };
                            offset += 9;
                            Some(crate::render::Border { width: bw, color: bc, style: bs })
                        } else {
                            None
                        };

                        let gradient = if has_gradient {
                            if offset + 1 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let grad_type = payload[offset];
                            offset += 1;

                            match grad_type {
                                // Linear gradient: angle(4) + stop_count(1) + stops(position(4) + color(4))...
                                0 => {
                                    if offset + 5 > payload.len() {
                                        return (BatchResponseType::Error, vec![]);
                                    }
                                    let angle = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                                    let stop_count = payload[offset + 4] as usize;
                                    offset += 5;

                                    if offset + stop_count * 8 > payload.len() {
                                        return (BatchResponseType::Error, vec![]);
                                    }
                                    let mut stops = Vec::with_capacity(stop_count);
                                    for _ in 0..stop_count {
                                        let pos = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                                        let col = u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]);
                                        offset += 8;
                                        stops.push(crate::render::GradientStop { position: pos, color: col });
                                    }
                                    Some(crate::render::Gradient::Linear { angle, stops })
                                }
                                // Radial gradient: center_x(4) + center_y(4) + stop_count(1) + stops...
                                1 => {
                                    if offset + 9 > payload.len() {
                                        return (BatchResponseType::Error, vec![]);
                                    }
                                    let center_x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                                    let center_y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                                    let stop_count = payload[offset + 8] as usize;
                                    offset += 9;

                                    if offset + stop_count * 8 > payload.len() {
                                        return (BatchResponseType::Error, vec![]);
                                    }
                                    let mut stops = Vec::with_capacity(stop_count);
                                    for _ in 0..stop_count {
                                        let pos = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                                        let col = u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]);
                                        offset += 8;
                                        stops.push(crate::render::GradientStop { position: pos, color: col });
                                    }
                                    Some(crate::render::Gradient::Radial { center_x, center_y, stops })
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };

                        commands.push(RenderCommand::DrawRect {
                            x, y, width, height, color,
                            corner_radii: [r0, r1, r2, r3],
                            rotation,
                            border,
                            gradient,
                        });
                    }

                    // DrawText: x(4) + y(4) + text_len(4) + text + font_data + color(4) + layout_data
                    0x02 => {
                        if offset + 12 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let text_len = u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]) as usize;
                        offset += 12;

                        if offset + text_len > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let text = match std::str::from_utf8(&payload[offset..offset + text_len]) {
                            Ok(s) => s.to_string(),
                            Err(_) => return (BatchResponseType::Error, vec![]),
                        };
                        offset += text_len;

                        // Font descriptor: source_type(1) + name_len(4) + name + weight(2) + style(1) + size(4)
                        if offset + 1 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let source_type = payload[offset];
                        offset += 1;

                        if offset + 4 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let font_name_len = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]) as usize;
                        offset += 4;

                        if offset + font_name_len > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let font_name = match std::str::from_utf8(&payload[offset..offset + font_name_len]) {
                            Ok(s) => s.to_string(),
                            Err(_) => return (BatchResponseType::Error, vec![]),
                        };
                        offset += font_name_len;

                        if offset + 7 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let weight = u16::from_le_bytes([payload[offset], payload[offset + 1]]);
                        let style = match payload[offset + 2] {
                            1 => FontStyle::Italic,
                            _ => FontStyle::Normal,
                        };
                        let size = f32::from_bits(u32::from_le_bytes([payload[offset + 3], payload[offset + 4], payload[offset + 5], payload[offset + 6]]));
                        offset += 7;

                        let source = match source_type {
                            1 => FontSource::Bundled(font_name),
                            _ => FontSource::System(font_name),
                        };
                        let font = FontDescriptor { source, weight, style, size };

                        // Color
                        if offset + 4 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let color = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]);
                        offset += 4;

                        // Layout config: flags(1) + [max_width(4)] + [max_height(4)] + [max_lines(4)] + line_height(4) + letter_spacing(4) + word_spacing(4) + alignment(1) + vertical_align(1) + word_break(1) + overflow(1) + white_space(1)
                        if offset + 1 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let layout_flags = payload[offset];
                        offset += 1;

                        let has_max_width = (layout_flags & 0x01) != 0;
                        let has_max_height = (layout_flags & 0x02) != 0;
                        let has_max_lines = (layout_flags & 0x04) != 0;

                        let max_width = if has_max_width {
                            if offset + 4 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let v = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                            offset += 4;
                            Some(v)
                        } else {
                            None
                        };

                        let max_height = if has_max_height {
                            if offset + 4 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let v = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                            offset += 4;
                            Some(v)
                        } else {
                            None
                        };

                        let max_lines = if has_max_lines {
                            if offset + 4 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let v = u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]) as usize;
                            offset += 4;
                            Some(v)
                        } else {
                            None
                        };

                        if offset + 17 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let line_height = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let letter_spacing = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let word_spacing = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let alignment = match payload[offset + 12] {
                            1 => TextAlign::Center,
                            2 => TextAlign::Right,
                            3 => TextAlign::Justify,
                            _ => TextAlign::Left,
                        };
                        let vertical_align = match payload[offset + 13] {
                            1 => VerticalAlign::Middle,
                            2 => VerticalAlign::Bottom,
                            3 => VerticalAlign::Baseline,
                            _ => VerticalAlign::Top,
                        };
                        let word_break = match payload[offset + 14] {
                            1 => WordBreak::BreakAll,
                            2 => WordBreak::KeepAll,
                            3 => WordBreak::BreakWord,
                            _ => WordBreak::Normal,
                        };
                        let overflow = match payload[offset + 15] {
                            1 => TextOverflow::Ellipsis,
                            2 => TextOverflow::Wrap,
                            _ => TextOverflow::Clip,
                        };
                        let white_space = match payload[offset + 16] {
                            1 => WhiteSpace::NoWrap,
                            2 => WhiteSpace::Pre,
                            3 => WhiteSpace::PreWrap,
                            _ => WhiteSpace::Normal,
                        };
                        offset += 17;

                        let layout = TextLayoutConfig {
                            max_width,
                            max_height,
                            max_lines,
                            line_height,
                            letter_spacing,
                            word_spacing,
                            alignment,
                            vertical_align,
                            word_break,
                            overflow,
                            white_space,
                        };

                        commands.push(RenderCommand::DrawText { x, y, text, font, color, layout });
                    }

                    // DrawImage: x(4) + y(4) + w(4) + h(4) + texture_id(4) + flags(1) + [source_rect(16)] + radii(16)
                    0x03 => {
                        if offset + 21 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let width = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let height = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                        let texture_id = u32::from_le_bytes([payload[offset + 16], payload[offset + 17], payload[offset + 18], payload[offset + 19]]);
                        let flags = payload[offset + 20];
                        offset += 21;

                        let has_source_rect = (flags & 0x01) != 0;

                        let source_rect = if has_source_rect {
                            if offset + 16 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let sx = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                            let sy = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                            let sw = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                            let sh = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                            offset += 16;
                            Some((sx, sy, sw, sh))
                        } else {
                            None
                        };

                        if offset + 16 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let r0 = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let r1 = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let r2 = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let r3 = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                        offset += 16;

                        commands.push(RenderCommand::DrawImage {
                            x, y, width, height, texture_id,
                            source_rect,
                            corner_radii: [r0, r1, r2, r3],
                        });
                    }

                    // DrawShadow: x(4) + y(4) + w(4) + h(4) + blur(4) + color(4) + offset_x(4) + offset_y(4) + radii(16)
                    0x04 => {
                        if offset + 48 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let width = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let height = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                        let blur = f32::from_bits(u32::from_le_bytes([payload[offset + 16], payload[offset + 17], payload[offset + 18], payload[offset + 19]]));
                        let color = u32::from_le_bytes([payload[offset + 20], payload[offset + 21], payload[offset + 22], payload[offset + 23]]);
                        let offset_x = f32::from_bits(u32::from_le_bytes([payload[offset + 24], payload[offset + 25], payload[offset + 26], payload[offset + 27]]));
                        let offset_y = f32::from_bits(u32::from_le_bytes([payload[offset + 28], payload[offset + 29], payload[offset + 30], payload[offset + 31]]));
                        let r0 = f32::from_bits(u32::from_le_bytes([payload[offset + 32], payload[offset + 33], payload[offset + 34], payload[offset + 35]]));
                        let r1 = f32::from_bits(u32::from_le_bytes([payload[offset + 36], payload[offset + 37], payload[offset + 38], payload[offset + 39]]));
                        let r2 = f32::from_bits(u32::from_le_bytes([payload[offset + 40], payload[offset + 41], payload[offset + 42], payload[offset + 43]]));
                        let r3 = f32::from_bits(u32::from_le_bytes([payload[offset + 44], payload[offset + 45], payload[offset + 46], payload[offset + 47]]));
                        offset += 48;

                        commands.push(RenderCommand::DrawShadow {
                            x, y, width, height, blur, color,
                            offset_x, offset_y,
                            corner_radii: [r0, r1, r2, r3],
                        });
                    }

                    // PushClip: x(4) + y(4) + w(4) + h(4)
                    0x05 => {
                        if offset + 16 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let width = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let height = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                        offset += 16;
                        commands.push(RenderCommand::PushClip { x, y, width, height });
                    }

                    // PopClip: (no data)
                    0x06 => {
                        commands.push(RenderCommand::PopClip {});
                    }

                    // BeginScrollView: x(4) + y(4) + w(4) + h(4) + scroll_x(4) + scroll_y(4) + flags(1) + [content_w(4)] + [content_h(4)]
                    0x07 => {
                        if offset + 25 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let x = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        let y = f32::from_bits(u32::from_le_bytes([payload[offset + 4], payload[offset + 5], payload[offset + 6], payload[offset + 7]]));
                        let width = f32::from_bits(u32::from_le_bytes([payload[offset + 8], payload[offset + 9], payload[offset + 10], payload[offset + 11]]));
                        let height = f32::from_bits(u32::from_le_bytes([payload[offset + 12], payload[offset + 13], payload[offset + 14], payload[offset + 15]]));
                        let scroll_x = f32::from_bits(u32::from_le_bytes([payload[offset + 16], payload[offset + 17], payload[offset + 18], payload[offset + 19]]));
                        let scroll_y = f32::from_bits(u32::from_le_bytes([payload[offset + 20], payload[offset + 21], payload[offset + 22], payload[offset + 23]]));
                        let flags = payload[offset + 24];
                        offset += 25;

                        let has_content_width = (flags & 0x01) != 0;
                        let has_content_height = (flags & 0x02) != 0;

                        let content_width = if has_content_width {
                            if offset + 4 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let v = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                            offset += 4;
                            Some(v)
                        } else {
                            None
                        };

                        let content_height = if has_content_height {
                            if offset + 4 > payload.len() {
                                return (BatchResponseType::Error, vec![]);
                            }
                            let v = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                            offset += 4;
                            Some(v)
                        } else {
                            None
                        };

                        commands.push(RenderCommand::BeginScrollView {
                            x, y, width, height, scroll_x, scroll_y, content_width, content_height,
                        });
                    }

                    // EndScrollView: (no data)
                    0x08 => {
                        commands.push(RenderCommand::EndScrollView {});
                    }

                    // SetOpacity: opacity(4)
                    0x09 => {
                        if offset + 4 > payload.len() {
                            return (BatchResponseType::Error, vec![]);
                        }
                        let opacity = f32::from_bits(u32::from_le_bytes([payload[offset], payload[offset + 1], payload[offset + 2], payload[offset + 3]]));
                        offset += 4;
                        commands.push(RenderCommand::SetOpacity(opacity));
                    }

                    // Unknown command type
                    _ => {
                        return (BatchResponseType::Error, format!("unknown render command type: {}", cmd_type).into_bytes());
                    }
                }
            }

            // Execute the render commands via the backend
            let backend_lock = get_backend();
            match backend_lock.lock() {
                Ok(mut guard) => {
                    if let Some(backend) = guard.as_mut() {
                        // Handle frameless window rendering (Linux/Windows)
                        #[cfg(any(target_os = "linux", target_os = "windows"))]
                        let final_commands = {
                            let mut all_commands = commands;

                            // Check frameless state and add window controls
                            if let Ok(state) = get_frameless_state().lock() {
                                if !state.decorations && state.show_native_controls && !all_commands.is_empty() {
                                    // Get window dimensions from backend (physical) and convert to logical
                                    let scale = state.scale_factor as f32;
                                    let logical_width = backend.get_width() as f32 / scale;
                                    let logical_height = backend.get_height() as f32 / scale;

                                    #[cfg(target_os = "linux")]
                                    let window_radius = crate::platform::linux::WINDOW_CORNER_RADIUS;
                                    #[cfg(target_os = "windows")]
                                    let window_radius = crate::platform::windows::WINDOW_CORNER_RADIUS;

                                    // Extract background color from Clear and replace with transparent
                                    let mut bg_color: Option<crate::style::Color> = None;
                                    for cmd in all_commands.iter_mut() {
                                        if let RenderCommand::Clear(color) = cmd {
                                            bg_color = Some(*color);
                                            *color = crate::style::Color { r: 0, g: 0, b: 0, a: 0 };
                                            break;
                                        }
                                    }

                                    // Insert rounded corner clipping at the beginning (after Clear)
                                    let rounded_clip = RenderCommand::PushRoundedClip {
                                        x: 0.0,
                                        y: 0.0,
                                        width: logical_width,
                                        height: logical_height,
                                        corner_radii: [window_radius, window_radius, window_radius, window_radius],
                                    };

                                    let insert_pos = all_commands.iter()
                                        .position(|cmd| !matches!(cmd, RenderCommand::Clear(_)))
                                        .unwrap_or(0);
                                    all_commands.insert(insert_pos, rounded_clip);

                                    // Draw background rect right after PushRoundedClip (inside stencil clip)
                                    if let Some(color) = bg_color {
                                        let bg_rect = RenderCommand::DrawRect {
                                            x: 0.0,
                                            y: 0.0,
                                            width: logical_width,
                                            height: logical_height,
                                            color: ((color.r as u32) << 24) | ((color.g as u32) << 16) | ((color.b as u32) << 8) | (color.a as u32),
                                            corner_radii: [0.0, 0.0, 0.0, 0.0],
                                            rotation: 0.0,
                                            border: None,
                                            gradient: None,
                                        };
                                        all_commands.insert(insert_pos + 1, bg_rect);
                                    }

                                    // Add window controls (inside the clipped area)
                                    if let Some(ref controls) = state.window_controls {
                                        let control_commands = controls.to_render_commands(logical_width);
                                        all_commands.extend(control_commands);
                                    }

                                    // End rounded corner clipping
                                    all_commands.push(RenderCommand::PopClip {});

                                    // Add window border
                                    #[cfg(target_os = "linux")]
                                    {
                                        let is_dark = state.dark_mode ||
                                            crate::platform::linux::is_dark_mode();
                                        let border_cmd = crate::platform::linux::window_border_command(
                                            logical_width,
                                            logical_height,
                                            is_dark,
                                        );
                                        all_commands.push(border_cmd);
                                    }
                                    #[cfg(target_os = "windows")]
                                    {
                                        let border_cmd = crate::platform::windows::window_border_command(
                                            logical_width,
                                            logical_height,
                                            state.dark_mode,
                                        );
                                        all_commands.push(border_cmd);
                                    }
                                }
                            }

                            all_commands
                        };

                        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                        let final_commands = commands;

                        match backend.render_frame(&final_commands) {
                            Ok(()) => (BatchResponseType::Success, vec![]),
                            Err(e) => (BatchResponseType::Error, format!("render error: {}", e).into_bytes()),
                        }
                    } else {
                        (BatchResponseType::Error, b"backend not initialized".to_vec())
                    }
                }
                Err(_) => (BatchResponseType::Error, b"failed to lock backend".to_vec()),
            }
        }

        // Unsupported command - return error
        _ => (BatchResponseType::Error, vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = centered_engine_version();
        assert!(!version.is_null());
        unsafe {
            let version_str = CStr::from_ptr(version).to_str().unwrap();
            assert_eq!(version_str, "0.1.0");
        }
    }

    #[test]
    fn test_engine_lifecycle() {
        let config = EngineConfig::default();
        let config_json = serde_json::to_string(&config).unwrap();
        let c_config = CString::new(config_json).unwrap();

        unsafe {
            let handle = centered_engine_init(c_config.as_ptr());
            assert!(!handle.is_null());

            let mode = centered_engine_get_mode(handle);
            assert_eq!(mode, 1); // Retained mode is default

            centered_engine_destroy(handle);
        }
    }
}
