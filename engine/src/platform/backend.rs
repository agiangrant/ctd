//! Platform backend trait for cross-platform window/event management
//!
//! Each platform (macOS, iOS, Windows, Linux, Android) implements this trait to handle:
//! - Window/view creation and lifecycle
//! - Event loop management
//! - Surface creation for wgpu rendering
//! - Input events (touch, mouse, keyboard)
//!
//! This replaces winit with direct platform APIs for better control,
//! especially on mobile where native lifecycle management is critical.

use std::error::Error;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub always_on_top: bool,
    pub fullscreen: bool,
    /// Target frames per second (default: 60)
    /// Use lower values (e.g., 30) for lighter apps to save battery
    /// Use higher values (e.g., 120) for games on high refresh rate displays
    pub target_fps: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Centered".to_string(),
            width: 800,
            height: 600,
            resizable: true,
            decorations: true,
            transparent: false,
            always_on_top: false,
            fullscreen: false,
            target_fps: 60,
        }
    }
}

/// Events sent from platform to application
#[derive(Debug, Clone)]
pub enum PlatformEvent {
    /// Application is ready, window created
    Ready {
        width: f64,
        height: f64,
        scale_factor: f64,
    },
    /// Window needs redraw
    RedrawRequested,
    /// Window resized
    Resized {
        width: f64,
        height: f64,
        scale_factor: f64,
    },
    /// Window close requested
    CloseRequested,
    /// Mouse/touch moved
    PointerMoved { x: f64, y: f64 },
    /// Mouse button or touch started
    PointerPressed { x: f64, y: f64, button: u8 },
    /// Mouse button or touch ended
    PointerReleased { x: f64, y: f64, button: u8 },
    /// Touch-specific events with touch ID for multi-touch
    TouchBegan { id: u64, x: f64, y: f64 },
    TouchMoved { id: u64, x: f64, y: f64 },
    TouchEnded { id: u64, x: f64, y: f64 },
    TouchCancelled { id: u64, x: f64, y: f64 },
    /// Scroll/wheel event
    Scroll { dx: f64, dy: f64 },
    /// Key pressed
    KeyPressed { keycode: u32, modifiers: u32 },
    /// Key released
    KeyReleased { keycode: u32, modifiers: u32 },
    /// Text input
    TextInput { text: String },
    /// Application suspended (mobile)
    Suspended,
    /// Application resumed (mobile)
    Resumed,
    /// Memory warning (mobile)
    MemoryWarning,
    /// Keyboard visibility changed (mobile)
    KeyboardFrameChanged {
        /// Height of keyboard in logical points (0 if hidden)
        height: f64,
        /// Animation duration in seconds
        animation_duration: f64,
    },
}

/// Response from application to platform
#[derive(Debug, Clone, Default)]
pub struct EventResponse {
    /// Request another frame immediately (for animations, scrolling)
    pub request_redraw: bool,
    /// Request exit
    pub exit: bool,
    /// Schedule a redraw after N milliseconds (for cursor blink, etc.)
    /// Only used if request_redraw is false
    pub redraw_after_ms: u32,
}

/// Safe area insets (for notched devices, status bars, etc.)
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeAreaInsets {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

/// Callback type for event handling
pub type EventCallback = Box<dyn FnMut(PlatformEvent) -> EventResponse>;

/// Platform backend trait
///
/// Each platform implements this to provide native window/event management.
/// The backend owns the event loop and calls the provided callback for each event.
pub trait PlatformBackend: Sized {
    /// Run the application with the given config and event callback.
    /// This function blocks until the application exits.
    fn run(config: AppConfig, callback: EventCallback) -> Result<(), Box<dyn Error>>;

    /// Request a redraw from any thread.
    /// This is used to wake up the event loop when Go wants to update the UI.
    fn request_redraw();

    /// Request application exit.
    fn request_exit();

    /// Get current safe area insets.
    fn safe_area_insets() -> SafeAreaInsets;
}

/// Handle to the native window/view for wgpu surface creation.
///
/// On macOS: NSWindow/NSView
/// On iOS: UIWindow/UIView with CAMetalLayer
/// On Windows: HWND
/// On Linux: X11 Window or Wayland surface
/// On Android: ANativeWindow
/// On Web: Canvas element ID
pub struct NativeHandle {
    /// Raw window handle for wgpu
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub ns_view: *mut std::ffi::c_void,

    #[cfg(target_os = "windows")]
    pub hwnd: *mut std::ffi::c_void,

    #[cfg(target_os = "linux")]
    pub window: u64,
    #[cfg(target_os = "linux")]
    pub display: *mut std::ffi::c_void,

    #[cfg(target_os = "android")]
    pub a_native_window: *mut std::ffi::c_void,

    /// Canvas element pointer (from wasm-bindgen)
    #[cfg(target_arch = "wasm32")]
    pub canvas_ptr: *mut std::ffi::c_void,
}

// SAFETY: NativeHandle contains raw pointers that are only used to create wgpu surfaces.
// The pointers remain valid for the lifetime of the window, and we ensure single-threaded
// access during surface creation.
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}

// Implement raw-window-handle traits for wgpu compatibility
impl raw_window_handle::HasWindowHandle for NativeHandle {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        #[cfg(target_os = "macos")]
        {
            let handle = raw_window_handle::AppKitWindowHandle::new(
                std::ptr::NonNull::new(self.ns_view).unwrap()
            );
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "ios")]
        {
            let handle = raw_window_handle::UiKitWindowHandle::new(
                std::ptr::NonNull::new(self.ns_view).unwrap()
            );
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "windows")]
        {
            use std::num::NonZeroIsize;
            let handle = raw_window_handle::Win32WindowHandle::new(
                NonZeroIsize::new(self.hwnd as isize).unwrap()
            );
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "linux")]
        {
            let handle = raw_window_handle::XlibWindowHandle::new(self.window);
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "android")]
        {
            let handle = raw_window_handle::AndroidNdkWindowHandle::new(
                std::ptr::NonNull::new(self.a_native_window).unwrap()
            );
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_arch = "wasm32")]
        {
            let handle = raw_window_handle::WebCanvasWindowHandle::new(
                std::ptr::NonNull::new(self.canvas_ptr).expect("canvas_ptr must be non-null")
            );
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
        }
    }
}

impl raw_window_handle::HasDisplayHandle for NativeHandle {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        #[cfg(target_os = "macos")]
        {
            let handle = raw_window_handle::AppKitDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "ios")]
        {
            let handle = raw_window_handle::UiKitDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "windows")]
        {
            let handle = raw_window_handle::WindowsDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "linux")]
        {
            let handle = raw_window_handle::XlibDisplayHandle::new(
                std::ptr::NonNull::new(self.display),
                0, // screen number
            );
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_os = "android")]
        {
            let handle = raw_window_handle::AndroidDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
        }

        #[cfg(target_arch = "wasm32")]
        {
            let handle = raw_window_handle::WebDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
        }
    }
}
