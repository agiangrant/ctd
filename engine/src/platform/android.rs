//! Android platform backend using android-activity
//!
//! This provides native Android lifecycle management, touch handling,
//! software/hardware keyboard integration, and wgpu rendering via Vulkan.

#![cfg(target_os = "android")]

use std::cell::RefCell;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use android_activity::{
    input::{InputEvent, KeyAction, KeyEvent, Keycode, MotionAction, MotionEvent},
    AndroidApp, InputStatus, MainEvent, PollEvent,
};
use jni::objects::{GlobalRef, JClass, JMethodID, JObject, JValue};
use jni::signature::{Primitive, ReturnType};
use jni::{JNIEnv, JavaVM};
use log::info;

use super::backend::{AppConfig, EventCallback, EventResponse, NativeHandle, PlatformBackend, PlatformEvent, SafeAreaInsets};
use super::wgpu_backend::{SurfaceConfig, WgpuBackend};

// Thread-local state for Android (main thread only)
thread_local! {
    static ANDROID_CALLBACK: RefCell<Option<Box<dyn FnMut(PlatformEvent) -> EventResponse>>> = RefCell::new(None);
    static ANDROID_APP: RefCell<Option<AndroidApp>> = RefCell::new(None);
    static SAFE_AREA: RefCell<SafeAreaInsets> = RefCell::new(SafeAreaInsets::default());
    static SCALE_FACTOR: RefCell<f64> = RefCell::new(1.0);
    static APP_READY: RefCell<bool> = RefCell::new(false);
}

// Global JNI state (set during JNI_OnLoad)
pub static mut JAVA_VM: Option<JavaVM> = None;
static mut ACTIVITY_CLASS: Option<GlobalRef> = None;
static mut GO_ANDROID_MAIN_CLASS: Option<GlobalRef> = None;

/// Get the activity pointer for JNI calls from other modules.
/// Returns the raw activity pointer or null if not available.
pub fn get_activity_ptr() -> *mut std::ffi::c_void {
    ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr()).unwrap_or(std::ptr::null_mut())
    })
}

// Cached JNI method IDs for performance
static mut METHOD_SHOW_KEYBOARD: Option<JMethodID> = None;
static mut METHOD_HIDE_KEYBOARD: Option<JMethodID> = None;
static mut METHOD_GET_SAFE_AREA: Option<JMethodID> = None;
static mut METHOD_HAPTIC_FEEDBACK: Option<JMethodID> = None;
static mut METHOD_GET_SCALE_FACTOR: Option<JMethodID> = None;
static mut METHOD_GO_ANDROID_MAIN: Option<jni::objects::JStaticMethodID> = None;

// Atomic flag for exit request (can be set from any thread)
static REQUEST_EXIT: AtomicBool = AtomicBool::new(false);
static REQUEST_REDRAW: AtomicBool = AtomicBool::new(false);

// Thread-safe queue for text input from JNI callbacks (UI thread -> main thread)
// Software keyboard input comes from the Android UI thread via JNI, but our callback
// is registered on the main Rust thread. This queue bridges the two threads.
lazy_static::lazy_static! {
    static ref PENDING_TEXT_INPUT: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref PENDING_KEY_EVENTS: Mutex<Vec<(i32, i32)>> = Mutex::new(Vec::new()); // (keycode, action)
    static ref PENDING_KEYBOARD_HEIGHT: Mutex<Option<(f32, i32)>> = Mutex::new(None); // (height in dp, duration in ms)
}

// C callback type for Go's ready handler
type GoReadyCallback = unsafe extern "C" fn();
static mut GO_READY_CALLBACK: Option<GoReadyCallback> = None;

/// Register the callback that Rust will call when the Android app is ready.
/// Go should call this before the app starts.
///
/// # Safety
/// Must be called from main thread before app starts.
#[no_mangle]
pub unsafe extern "C" fn centered_android_set_ready_callback(callback: GoReadyCallback) {
    GO_READY_CALLBACK = Some(callback);
}

/// Called by Go to register its event callback after the app is ready.
/// This should be called from within the ready callback.
pub fn register_callback(callback: Box<dyn FnMut(PlatformEvent) -> EventResponse>) {
    ANDROID_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

/// JNI_OnLoad - called when the native library is loaded
/// Stores the JavaVM for later JNI calls
/// Note: With GameActivity, the Kotlin code loads both libraries (centered_engine and gojni)
/// so we don't need to load gojni here. The Go class caching happens in android_main.
#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: jni::JavaVM, _reserved: *mut std::ffi::c_void) -> jni::sys::jint {
    // Initialize Android logger
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("Centered"),
    );

    info!("JNI_OnLoad: Centered engine loaded");

    // Store JavaVM for later use
    unsafe {
        JAVA_VM = Some(vm);
    }

    jni::sys::JNI_VERSION_1_6
}

/// Load a native library via JNI System.loadLibrary()
fn load_library_via_jni(env: &mut JNIEnv, lib_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let system_class = env.find_class("java/lang/System")?;
    let lib_jstring = env.new_string(lib_name)?;
    env.call_static_method(
        system_class,
        "loadLibrary",
        "(Ljava/lang/String;)V",
        &[(&lib_jstring).into()],
    )?;
    Ok(())
}

/// Cache JNI method IDs for the activity class
/// Called once when we have access to the activity
fn cache_jni_method_ids(env: &mut JNIEnv, activity_class: &JClass) {
    unsafe {
        // Cache method IDs (these are stable for the lifetime of the class)
        METHOD_SHOW_KEYBOARD = env
            .get_method_id(activity_class, "showSoftKeyboard", "()V")
            .ok();
        METHOD_HIDE_KEYBOARD = env
            .get_method_id(activity_class, "hideSoftKeyboard", "()V")
            .ok();
        METHOD_GET_SAFE_AREA = env
            .get_method_id(activity_class, "getSafeAreaInsets", "()[F")
            .ok();
        METHOD_HAPTIC_FEEDBACK = env
            .get_method_id(activity_class, "hapticFeedback", "(I)V")
            .ok();
        METHOD_GET_SCALE_FACTOR = env
            .get_method_id(activity_class, "getScaleFactor", "()F")
            .ok();
    }
}

/// Send an event to the callback and handle the response
/// Uses try_borrow_mut to handle re-entrant calls safely
fn send_event(event: PlatformEvent) -> EventResponse {
    ANDROID_CALLBACK.with(|cb| {
        match cb.try_borrow_mut() {
            Ok(mut guard) => {
                if let Some(ref mut callback) = *guard {
                    callback(event)
                } else {
                    EventResponse::default()
                }
            }
            Err(_) => {
                // Re-entrant call - callback is already borrowed
                EventResponse::default()
            }
        }
    })
}

/// Process queued text input and key events from the JNI thread.
/// This must be called from the main Rust thread where the callback is registered.
fn process_queued_input() -> bool {
    let mut had_events = false;

    // Process queued text input
    if let Ok(mut queue) = PENDING_TEXT_INPUT.lock() {
        for text in queue.drain(..) {
            info!("Processing queued text input: '{}'", text);
            let event = PlatformEvent::TextInput { text };
            let response = send_event(event);
            if response.exit {
                REQUEST_EXIT.store(true, Ordering::SeqCst);
            }
            if response.request_redraw {
                REQUEST_REDRAW.store(true, Ordering::SeqCst);
            }
            had_events = true;
        }
    }

    // Process queued key events (backspace, enter, etc.)
    if let Ok(mut queue) = PENDING_KEY_EVENTS.lock() {
        for (key_code, _action) in queue.drain(..) {
            let ffi_keycode = android_keycode_to_ffi(key_code);
            info!("Processing queued key event: keycode={} -> ffi={}", key_code, ffi_keycode);

            let event = PlatformEvent::KeyPressed {
                keycode: ffi_keycode,
                modifiers: 0,
            };
            let response = send_event(event);
            if response.exit {
                REQUEST_EXIT.store(true, Ordering::SeqCst);
            }
            if response.request_redraw {
                REQUEST_REDRAW.store(true, Ordering::SeqCst);
            }
            had_events = true;
        }
    }

    // Process queued keyboard height changes
    if let Ok(mut pending) = PENDING_KEYBOARD_HEIGHT.lock() {
        if let Some((height, duration_ms)) = pending.take() {
            info!("Processing keyboard height change: height={}, duration_ms={}", height, duration_ms);

            // Convert duration from ms to seconds
            let animation_duration = duration_ms as f64 / 1000.0;

            let event = PlatformEvent::KeyboardFrameChanged {
                height: height as f64,
                animation_duration,
            };
            let response = send_event(event);
            if response.exit {
                REQUEST_EXIT.store(true, Ordering::SeqCst);
            }
            if response.request_redraw {
                REQUEST_REDRAW.store(true, Ordering::SeqCst);
            }
            had_events = true;
        }
    }

    had_events
}

/// Convert Android MotionEvent to PlatformEvent(s)
/// Converts physical pixel coordinates to logical pixels for Go's layout system
fn handle_motion_event(event: &MotionEvent) -> Vec<PlatformEvent> {
    let mut events = Vec::new();
    let action = event.action();

    // Get scale factor to convert physical to logical pixels
    let scale = SCALE_FACTOR.with(|s| *s.borrow());

    // Get the pointer that triggered this event
    let pointer_index = event.pointer_index();
    let pointer_count = event.pointer_count();

    match action {
        MotionAction::Down => {
            // Primary pointer down - use index 0 for primary touch
            let pointer = event.pointer_at_index(0);
            let x = pointer.x() as f64 / scale;
            let y = pointer.y() as f64 / scale;
            info!("TouchBegan: physical=({}, {}), logical=({}, {}), scale={}",
                  pointer.x(), pointer.y(), x, y, scale);
            events.push(PlatformEvent::TouchBegan {
                id: pointer.pointer_id() as u64,
                x,
                y,
            });
        }
        MotionAction::PointerDown => {
            // Secondary pointer down
            let pointer = event.pointer_at_index(pointer_index);
            events.push(PlatformEvent::TouchBegan {
                id: pointer.pointer_id() as u64,
                x: pointer.x() as f64 / scale,
                y: pointer.y() as f64 / scale,
            });
        }
        MotionAction::Move => {
            // All pointers moved - report all of them
            for i in 0..pointer_count {
                let pointer = event.pointer_at_index(i);
                events.push(PlatformEvent::TouchMoved {
                    id: pointer.pointer_id() as u64,
                    x: pointer.x() as f64 / scale,
                    y: pointer.y() as f64 / scale,
                });
            }
        }
        MotionAction::Up => {
            // Primary pointer up - use index 0 for primary touch
            let pointer = event.pointer_at_index(0);
            let x = pointer.x() as f64 / scale;
            let y = pointer.y() as f64 / scale;
            info!("TouchEnded: physical=({}, {}), logical=({}, {}), scale={}",
                  pointer.x(), pointer.y(), x, y, scale);
            events.push(PlatformEvent::TouchEnded {
                id: pointer.pointer_id() as u64,
                x,
                y,
            });
        }
        MotionAction::PointerUp => {
            // Secondary pointer up
            let pointer = event.pointer_at_index(pointer_index);
            events.push(PlatformEvent::TouchEnded {
                id: pointer.pointer_id() as u64,
                x: pointer.x() as f64 / scale,
                y: pointer.y() as f64 / scale,
            });
        }
        MotionAction::Cancel => {
            // All pointers cancelled
            for i in 0..pointer_count {
                let pointer = event.pointer_at_index(i);
                events.push(PlatformEvent::TouchCancelled {
                    id: pointer.pointer_id() as u64,
                    x: pointer.x() as f64 / scale,
                    y: pointer.y() as f64 / scale,
                });
            }
        }
        _ => {}
    }

    events
}

/// Convert Android KeyEvent to PlatformEvent(s)
/// Returns both KeyPressed event and optionally TextInput for printable characters
fn handle_key_event(event: &KeyEvent) -> Vec<PlatformEvent> {
    let mut events = Vec::new();

    // Get the Android keycode enum
    let android_keycode = event.key_code();
    let keycode = keycode_to_ffi(android_keycode);

    // Get modifiers from meta state
    // The meta_state() returns a MetaState which we need to convert to raw flags
    // For now, we'll access it through the underlying representation
    let modifiers = 0u32;  // TODO: Extract modifiers when android-activity exposes MetaState flags

    match event.action() {
        KeyAction::Down => {
            // Always send the key press event
            events.push(PlatformEvent::KeyPressed { keycode, modifiers });

            // For printable characters, also send a TextInput event
            // This mimics iOS behavior where both key events and text input are sent
            if let Some(text) = keycode_to_char(android_keycode) {
                events.push(PlatformEvent::TextInput { text });
            }
        }
        KeyAction::Up => {
            events.push(PlatformEvent::KeyReleased { keycode, modifiers });
        }
        _ => {}
    }

    events
}

/// Convert Android Keycode enum to printable character (if applicable)
/// Returns None for non-printable keys
fn keycode_to_char(keycode: Keycode) -> Option<String> {
    match keycode {
        // Letters A-Z
        Keycode::A => Some("a".to_string()),
        Keycode::B => Some("b".to_string()),
        Keycode::C => Some("c".to_string()),
        Keycode::D => Some("d".to_string()),
        Keycode::E => Some("e".to_string()),
        Keycode::F => Some("f".to_string()),
        Keycode::G => Some("g".to_string()),
        Keycode::H => Some("h".to_string()),
        Keycode::I => Some("i".to_string()),
        Keycode::J => Some("j".to_string()),
        Keycode::K => Some("k".to_string()),
        Keycode::L => Some("l".to_string()),
        Keycode::M => Some("m".to_string()),
        Keycode::N => Some("n".to_string()),
        Keycode::O => Some("o".to_string()),
        Keycode::P => Some("p".to_string()),
        Keycode::Q => Some("q".to_string()),
        Keycode::R => Some("r".to_string()),
        Keycode::S => Some("s".to_string()),
        Keycode::T => Some("t".to_string()),
        Keycode::U => Some("u".to_string()),
        Keycode::V => Some("v".to_string()),
        Keycode::W => Some("w".to_string()),
        Keycode::X => Some("x".to_string()),
        Keycode::Y => Some("y".to_string()),
        Keycode::Z => Some("z".to_string()),
        // Numbers 0-9
        Keycode::Keycode0 => Some("0".to_string()),
        Keycode::Keycode1 => Some("1".to_string()),
        Keycode::Keycode2 => Some("2".to_string()),
        Keycode::Keycode3 => Some("3".to_string()),
        Keycode::Keycode4 => Some("4".to_string()),
        Keycode::Keycode5 => Some("5".to_string()),
        Keycode::Keycode6 => Some("6".to_string()),
        Keycode::Keycode7 => Some("7".to_string()),
        Keycode::Keycode8 => Some("8".to_string()),
        Keycode::Keycode9 => Some("9".to_string()),
        // Punctuation and symbols
        Keycode::Space => Some(" ".to_string()),
        Keycode::Comma => Some(",".to_string()),
        Keycode::Period => Some(".".to_string()),
        Keycode::Slash => Some("/".to_string()),
        Keycode::Semicolon => Some(";".to_string()),
        Keycode::Apostrophe => Some("'".to_string()),
        Keycode::LeftBracket => Some("[".to_string()),
        Keycode::RightBracket => Some("]".to_string()),
        Keycode::Backslash => Some("\\".to_string()),
        Keycode::Minus => Some("-".to_string()),
        Keycode::Equals => Some("=".to_string()),
        Keycode::Grave => Some("`".to_string()),
        _ => None,
    }
}

/// Convert Android Keycode enum to FFI keycode
/// Follows same mapping as iOS HID codes for consistency
fn keycode_to_ffi(keycode: Keycode) -> u32 {
    match keycode {
        // Letters A-Z → FFI 0-25
        Keycode::A => 0,
        Keycode::B => 1,
        Keycode::C => 2,
        Keycode::D => 3,
        Keycode::E => 4,
        Keycode::F => 5,
        Keycode::G => 6,
        Keycode::H => 7,
        Keycode::I => 8,
        Keycode::J => 9,
        Keycode::K => 10,
        Keycode::L => 11,
        Keycode::M => 12,
        Keycode::N => 13,
        Keycode::O => 14,
        Keycode::P => 15,
        Keycode::Q => 16,
        Keycode::R => 17,
        Keycode::S => 18,
        Keycode::T => 19,
        Keycode::U => 20,
        Keycode::V => 21,
        Keycode::W => 22,
        Keycode::X => 23,
        Keycode::Y => 24,
        Keycode::Z => 25,
        // Numbers 1-9 → FFI 26-34, 0 → FFI 35
        Keycode::Keycode1 => 26,
        Keycode::Keycode2 => 27,
        Keycode::Keycode3 => 28,
        Keycode::Keycode4 => 29,
        Keycode::Keycode5 => 30,
        Keycode::Keycode6 => 31,
        Keycode::Keycode7 => 32,
        Keycode::Keycode8 => 33,
        Keycode::Keycode9 => 34,
        Keycode::Keycode0 => 35,
        // Function keys F1-F12 → FFI 36-47
        Keycode::F1 => 36,
        Keycode::F2 => 37,
        Keycode::F3 => 38,
        Keycode::F4 => 39,
        Keycode::F5 => 40,
        Keycode::F6 => 41,
        Keycode::F7 => 42,
        Keycode::F8 => 43,
        Keycode::F9 => 44,
        Keycode::F10 => 45,
        Keycode::F11 => 46,
        Keycode::F12 => 47,
        // Arrow keys
        Keycode::DpadUp => 48,
        Keycode::DpadDown => 49,
        Keycode::DpadLeft => 50,
        Keycode::DpadRight => 51,
        // Special keys
        Keycode::Escape => 52,
        Keycode::Tab => 53,
        Keycode::Space => 54,
        Keycode::Enter => 59,
        Keycode::Del => 56,        // Backspace
        Keycode::ForwardDel => 57, // Delete
        // Modifiers
        Keycode::ShiftLeft | Keycode::ShiftRight => 58,
        Keycode::CtrlLeft | Keycode::CtrlRight => 60,
        Keycode::AltLeft | Keycode::AltRight => 61,
        Keycode::MetaLeft | Keycode::MetaRight => 62,
        _ => 0, // Unknown key
    }
}

/// Convert Android keycode to FFI keycode
/// Follows same mapping as iOS HID codes for consistency
fn android_keycode_to_ffi(keycode: i32) -> u32 {
    // Android keycodes from android.view.KeyEvent
    const KEYCODE_A: i32 = 29;
    const KEYCODE_Z: i32 = 54;
    const KEYCODE_0: i32 = 7;
    const KEYCODE_9: i32 = 16;
    const KEYCODE_ENTER: i32 = 66;
    const KEYCODE_DEL: i32 = 67;  // Backspace
    const KEYCODE_FORWARD_DEL: i32 = 112;
    const KEYCODE_TAB: i32 = 61;
    const KEYCODE_SPACE: i32 = 62;
    const KEYCODE_DPAD_UP: i32 = 19;
    const KEYCODE_DPAD_DOWN: i32 = 20;
    const KEYCODE_DPAD_LEFT: i32 = 21;
    const KEYCODE_DPAD_RIGHT: i32 = 22;
    const KEYCODE_ESCAPE: i32 = 111;
    const KEYCODE_F1: i32 = 131;
    const KEYCODE_F12: i32 = 142;
    const KEYCODE_SHIFT_LEFT: i32 = 59;
    const KEYCODE_SHIFT_RIGHT: i32 = 60;
    const KEYCODE_CTRL_LEFT: i32 = 113;
    const KEYCODE_CTRL_RIGHT: i32 = 114;
    const KEYCODE_ALT_LEFT: i32 = 57;
    const KEYCODE_ALT_RIGHT: i32 = 58;
    const KEYCODE_META_LEFT: i32 = 117;  // Super/Windows/Command
    const KEYCODE_META_RIGHT: i32 = 118;

    match keycode {
        // Letters A-Z → FFI 0-25
        k if k >= KEYCODE_A && k <= KEYCODE_Z => (k - KEYCODE_A) as u32,

        // Numbers 1-9 → FFI 26-34, 0 → FFI 35
        k if k >= KEYCODE_0 + 1 && k <= KEYCODE_9 => (k - KEYCODE_0 - 1 + 26) as u32,
        KEYCODE_0 => 35,

        // Function keys F1-F12 → FFI 36-47
        k if k >= KEYCODE_F1 && k <= KEYCODE_F12 => (k - KEYCODE_F1 + 36) as u32,

        // Arrow keys
        KEYCODE_DPAD_UP => 48,
        KEYCODE_DPAD_DOWN => 49,
        KEYCODE_DPAD_LEFT => 50,
        KEYCODE_DPAD_RIGHT => 51,

        // Special keys
        KEYCODE_ESCAPE => 52,
        KEYCODE_TAB => 53,
        KEYCODE_SPACE => 54,
        KEYCODE_ENTER => 59,
        KEYCODE_DEL => 56,  // Backspace
        KEYCODE_FORWARD_DEL => 57,  // Delete

        // Modifiers (for completeness, though usually handled via meta state)
        KEYCODE_SHIFT_LEFT | KEYCODE_SHIFT_RIGHT => 58,
        KEYCODE_CTRL_LEFT | KEYCODE_CTRL_RIGHT => 60,
        KEYCODE_ALT_LEFT | KEYCODE_ALT_RIGHT => 61,
        KEYCODE_META_LEFT | KEYCODE_META_RIGHT => 62,

        _ => 0,  // Unknown key
    }
}

/// Convert Android meta state to FFI modifier flags
fn android_meta_to_modifiers(meta: u32) -> u32 {
    // Android meta state flags
    const META_SHIFT_ON: u32 = 0x00000001;
    const META_ALT_ON: u32 = 0x00000002;
    const META_CTRL_ON: u32 = 0x00001000;
    const META_META_ON: u32 = 0x00010000;  // Super/Command

    // FFI modifier flags (same as iOS)
    const MOD_SHIFT: u32 = 1 << 0;
    const MOD_CTRL: u32 = 1 << 1;
    const MOD_ALT: u32 = 1 << 2;
    const MOD_SUPER: u32 = 1 << 3;

    let mut modifiers = 0u32;
    if meta & META_SHIFT_ON != 0 { modifiers |= MOD_SHIFT; }
    if meta & META_CTRL_ON != 0 { modifiers |= MOD_CTRL; }
    if meta & META_ALT_ON != 0 { modifiers |= MOD_ALT; }
    if meta & META_META_ON != 0 { modifiers |= MOD_SUPER; }

    modifiers
}

/// Android platform backend implementation
pub struct AndroidBackend;

impl PlatformBackend for AndroidBackend {
    fn run(config: AppConfig, callback: EventCallback) -> Result<(), Box<dyn Error>> {
        // Store callback in thread-local
        register_callback(callback);

        // Get the AndroidApp from thread-local (set by android_main)
        let app = ANDROID_APP.with(|a| a.borrow().clone())
            .ok_or("AndroidApp not initialized - ensure android_main was called")?;

        let mut backend: Option<WgpuBackend> = None;
        let mut has_window = false;
        let mut window_size = (config.width, config.height);

        info!("AndroidBackend::run starting event loop");

        // Main event loop
        loop {
            // Check exit request
            if REQUEST_EXIT.load(Ordering::SeqCst) {
                info!("Exit requested, breaking event loop");
                break;
            }

            // Poll for events
            app.poll_events(Some(std::time::Duration::from_millis(16)), |event| {
                match event {
                    PollEvent::Wake => {
                        // Check if redraw was requested
                        if REQUEST_REDRAW.swap(false, Ordering::SeqCst) {
                            let response = send_event(PlatformEvent::RedrawRequested);
                            if response.exit {
                                REQUEST_EXIT.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    PollEvent::Timeout => {
                        // Frame tick - request redraw if we have a window
                        if has_window {
                            let response = send_event(PlatformEvent::RedrawRequested);
                            if response.exit {
                                REQUEST_EXIT.store(true, Ordering::SeqCst);
                            }
                            // Note: Rendering is done in the callback via render_frame()
                        }
                    }
                    PollEvent::Main(main_event) => {
                        match main_event {
                            MainEvent::InitWindow { .. } => {
                                info!("InitWindow received");

                                // Get the native window
                                if let Some(window) = app.native_window() {
                                    let width = window.width() as u32;
                                    let height = window.height() as u32;
                                    window_size = (width, height);

                                    // Get scale factor from JNI
                                    let scale = get_scale_factor_jni().unwrap_or(1.0) as f64;
                                    SCALE_FACTOR.with(|s| *s.borrow_mut() = scale);

                                    info!("Window size: {}x{}, scale: {}", width, height, scale);

                                    // Initialize wgpu backend
                                    let mut new_backend = WgpuBackend::new();
                                    let native_handle = NativeHandle {
                                        a_native_window: window.ptr().as_ptr() as *mut std::ffi::c_void,
                                    };

                                    // Same configuration as iOS
                                    let surface_config = SurfaceConfig {
                                        width,
                                        height,
                                        scale_factor: scale,
                                        vsync: true,
                                        low_power_gpu: false,  // Prefer performance GPU
                                        allow_software_fallback: false,
                                    };

                                    // Initialize with window (blocking on async)
                                    if let Err(e) = pollster::block_on(
                                        new_backend.init_with_window(&native_handle, surface_config)
                                    ) {
                                        info!("Failed to init wgpu: {:?}", e);
                                    } else {
                                        // Store backend globally for FFI access
                                        {
                                            let backend_lock = crate::ffi::get_backend();
                                            if let Ok(mut guard) = backend_lock.lock() {
                                                *guard = Some(new_backend);
                                            }
                                        }

                                        // Get backend reference for local use
                                        backend = {
                                            let backend_lock = crate::ffi::get_backend();
                                            if let Ok(guard) = backend_lock.lock() {
                                                // Note: We can't move out of the mutex, so we'll use the global
                                                None
                                            } else {
                                                None
                                            }
                                        };

                                        has_window = true;

                                        // Update safe area insets
                                        update_safe_area_insets();

                                        // Notify Go that we're ready
                                        unsafe {
                                            if let Some(callback) = GO_READY_CALLBACK {
                                                callback();
                                            }
                                        }

                                        // Send Ready event
                                        let response = send_event(PlatformEvent::Ready {
                                            width: width as f64,
                                            height: height as f64,
                                            scale_factor: scale,
                                        });

                                        if response.exit {
                                            REQUEST_EXIT.store(true, Ordering::SeqCst);
                                        }

                                        APP_READY.with(|r| *r.borrow_mut() = true);
                                    }
                                }
                            }
                            MainEvent::TerminateWindow { .. } => {
                                info!("TerminateWindow received");
                                has_window = false;

                                // Clear the global backend
                                {
                                    let backend_lock = crate::ffi::get_backend();
                                    if let Ok(mut guard) = backend_lock.lock() {
                                        *guard = None;
                                    }
                                }
                                backend = None;
                            }
                            MainEvent::WindowResized { .. } => {
                                if let Some(window) = app.native_window() {
                                    let physical_width = window.width() as u32;
                                    let physical_height = window.height() as u32;
                                    window_size = (physical_width, physical_height);

                                    let scale = SCALE_FACTOR.with(|s| *s.borrow());

                                    // Convert to logical pixels for Go
                                    let logical_width = physical_width as f64 / scale;
                                    let logical_height = physical_height as f64 / scale;

                                    info!("Window resized: {}x{} physical, {}x{} logical",
                                          physical_width, physical_height, logical_width, logical_height);

                                    // Resize wgpu backend (uses physical pixels)
                                    {
                                        let backend_lock = crate::ffi::get_backend();
                                        if let Ok(mut guard) = backend_lock.lock() {
                                            if let Some(ref mut b) = *guard {
                                                let _ = b.resize(physical_width, physical_height, scale);
                                            }
                                        }
                                    }

                                    // Update safe area insets
                                    update_safe_area_insets();

                                    // Send logical dimensions to Go
                                    let response = send_event(PlatformEvent::Resized {
                                        width: logical_width,
                                        height: logical_height,
                                        scale_factor: scale,
                                    });

                                    if response.exit {
                                        REQUEST_EXIT.store(true, Ordering::SeqCst);
                                    }
                                }
                            }
                            MainEvent::GainedFocus => {
                                info!("App gained focus (Resumed)");
                                let response = send_event(PlatformEvent::Resumed);
                                if response.exit {
                                    REQUEST_EXIT.store(true, Ordering::SeqCst);
                                }
                            }
                            MainEvent::LostFocus => {
                                info!("App lost focus (Suspended)");
                                let response = send_event(PlatformEvent::Suspended);
                                if response.exit {
                                    REQUEST_EXIT.store(true, Ordering::SeqCst);
                                }
                            }
                            MainEvent::LowMemory => {
                                info!("Low memory warning");
                                let _ = send_event(PlatformEvent::MemoryWarning);
                            }
                            MainEvent::Destroy => {
                                info!("App destroy requested");
                                REQUEST_EXIT.store(true, Ordering::SeqCst);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            });

            // Handle input events
            if let Ok(mut input_iter) = app.input_events_iter() {
                loop {
                    if !input_iter.next(|event| {
                        match event {
                            InputEvent::MotionEvent(motion_event) => {
                                let events = handle_motion_event(&motion_event);
                                for event in events {
                                    let response = send_event(event);
                                    if response.exit {
                                        REQUEST_EXIT.store(true, Ordering::SeqCst);
                                    }
                                    if response.request_redraw {
                                        REQUEST_REDRAW.store(true, Ordering::SeqCst);
                                    }
                                }
                                InputStatus::Handled
                            }
                            InputEvent::KeyEvent(key_event) => {
                                // handle_key_event returns multiple events (KeyPressed + optional TextInput)
                                for event in handle_key_event(&key_event) {
                                    let response = send_event(event);
                                    if response.exit {
                                        REQUEST_EXIT.store(true, Ordering::SeqCst);
                                    }
                                    if response.request_redraw {
                                        REQUEST_REDRAW.store(true, Ordering::SeqCst);
                                    }
                                }
                                InputStatus::Handled
                            }
                            _ => InputStatus::Unhandled,
                        }
                    }) {
                        break;
                    }
                }
            }
        }

        info!("AndroidBackend::run exiting");
        Ok(())
    }

    fn request_redraw() {
        REQUEST_REDRAW.store(true, Ordering::SeqCst);
        // Event loop will pick this up on next poll timeout
    }

    fn request_exit() {
        REQUEST_EXIT.store(true, Ordering::SeqCst);
        // Event loop will pick this up on next poll timeout
    }

    fn safe_area_insets() -> SafeAreaInsets {
        SAFE_AREA.with(|s| *s.borrow())
    }
}

/// Entry point for android-activity
/// This is called by the android-activity crate when the app starts.
/// Unlike iOS where UIApplicationMain runs the event loop, on Android
/// we must run the event loop ourselves here.
#[no_mangle]
fn android_main(app: AndroidApp) {
    // Initialize Android logger
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("Centered"),
    );

    info!("android_main: Centered engine starting");

    // Initialize the global JavaVM from AndroidApp
    // This is needed because android_main runs on a different thread than JNI_OnLoad
    // and the android-activity crate manages the JVM connection
    unsafe {
        let vm_ptr = app.vm_as_ptr();
        if !vm_ptr.is_null() {
            // Create a JavaVM from the raw pointer
            match JavaVM::from_raw(vm_ptr as *mut jni::sys::JavaVM) {
                Ok(vm) => {
                    info!("android_main: Got JavaVM from AndroidApp");

                    // Try to load Go library and cache class reference here
                    // since JNI_OnLoad may not be called with NativeActivity
                    if let Ok(mut env) = vm.attach_current_thread() {
                        // Clear any pending exceptions
                        let _ = env.exception_clear();

                        // Get the activity to use its class loader
                        let activity_ptr = app.activity_as_ptr();
                        if !activity_ptr.is_null() {
                            // Use ManuallyDrop to prevent JObject from deleting the reference we don't own
                            let activity = std::mem::ManuallyDrop::new(JObject::from_raw(activity_ptr as *mut _));

                            // Get the activity's class loader
                            if let Ok(class_loader_val) = env.call_method(&*activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[]) {
                                if let Ok(class_loader) = class_loader_val.l() {
                                    info!("android_main: Got activity class loader");

                                    // First load the Go library if not already loaded
                                    if GO_ANDROID_MAIN_CLASS.is_none() {
                                        // Use ClassLoader.loadClass to find the Go class
                                        let class_name = env.new_string("android_demo.Android_demo").ok();
                                        if let Some(class_name_str) = class_name {
                                            match env.call_method(
                                                &class_loader,
                                                "loadClass",
                                                "(Ljava/lang/String;)Ljava/lang/Class;",
                                                &[(&class_name_str).into()],
                                            ) {
                                                Ok(class_val) => {
                                                    if let Ok(class_obj) = class_val.l() {
                                                        // Convert JObject to JClass
                                                        let class: JClass = JClass::from_raw(class_obj.as_raw());

                                                        if let Ok(global_ref) = env.new_global_ref(&class) {
                                                            GO_ANDROID_MAIN_CLASS = Some(global_ref);
                                                            info!("android_main: Cached Go class reference");

                                                            if let Ok(method_id) = env.get_static_method_id(&class, "androidMain", "()V") {
                                                                METHOD_GO_ANDROID_MAIN = Some(method_id);
                                                                info!("android_main: Cached Go androidMain method ID");
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    info!("android_main: Could not find Go class via class loader: {:?}", e);
                                                    let _ = env.exception_clear();
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                let _ = env.exception_clear();
                                info!("android_main: Could not get class loader from activity");
                            }
                        }
                    }

                    // Store the JavaVM after we're done using the reference
                    JAVA_VM = Some(vm);
                }
                Err(e) => {
                    info!("android_main: Failed to create JavaVM: {:?}", e);
                }
            }
        } else {
            info!("android_main: vm_as_ptr returned null");
        }
    }

    // Store AndroidApp in thread-local for later access
    ANDROID_APP.with(|a| *a.borrow_mut() = Some(app.clone()));

    // Run the event loop directly - this is required by android-activity
    // The AndroidBackend::run method expects a callback to be registered,
    // but Go won't register it until after we send the Ready event.
    // So we run the loop here and call Go's ready callback when the window is ready.
    run_android_event_loop(app);
}

/// Main Android event loop - runs until the app exits
fn run_android_event_loop(app: AndroidApp) {
    let mut has_window = false;

    info!("android_main: starting event loop");

    loop {
        // Check exit request
        if REQUEST_EXIT.load(Ordering::SeqCst) {
            info!("Exit requested, breaking event loop");
            break;
        }

        // Poll for events with 16ms timeout (~60fps)
        app.poll_events(Some(std::time::Duration::from_millis(16)), |event| {
            match event {
                PollEvent::Wake => {
                    // Check if redraw was requested
                    if REQUEST_REDRAW.swap(false, Ordering::SeqCst) {
                        let response = send_event(PlatformEvent::RedrawRequested);
                        if response.exit {
                            REQUEST_EXIT.store(true, Ordering::SeqCst);
                        }
                    }
                }
                PollEvent::Timeout => {
                    // Frame tick - request redraw if we have a window and app is ready
                    if has_window && APP_READY.with(|r| *r.borrow()) {
                        let response = send_event(PlatformEvent::RedrawRequested);
                        if response.exit {
                            REQUEST_EXIT.store(true, Ordering::SeqCst);
                        }
                    }
                }
                PollEvent::Main(main_event) => {
                    match main_event {
                        MainEvent::InitWindow { .. } => {
                            info!("InitWindow received");
                            handle_init_window(&app);
                            has_window = true;
                        }
                        MainEvent::TerminateWindow { .. } => {
                            info!("TerminateWindow received");
                            has_window = false;
                            // Clear the global backend
                            {
                                let backend_lock = crate::ffi::get_backend();
                                if let Ok(mut guard) = backend_lock.lock() {
                                    *guard = None;
                                }
                            }
                        }
                        MainEvent::WindowResized { .. } => {
                            if let Some(window) = app.native_window() {
                                let physical_width = window.width() as u32;
                                let physical_height = window.height() as u32;
                                let scale = SCALE_FACTOR.with(|s| *s.borrow());

                                // Convert to logical pixels for Go
                                let logical_width = physical_width as f64 / scale;
                                let logical_height = physical_height as f64 / scale;

                                info!("Window resized: {}x{} physical, {}x{} logical",
                                      physical_width, physical_height, logical_width, logical_height);

                                // Resize wgpu backend (uses physical pixels)
                                {
                                    let backend_lock = crate::ffi::get_backend();
                                    if let Ok(mut guard) = backend_lock.lock() {
                                        if let Some(ref mut b) = *guard {
                                            let _ = b.resize(physical_width, physical_height, scale);
                                        }
                                    }
                                }

                                update_safe_area_insets();

                                // Send logical dimensions to Go
                                let response = send_event(PlatformEvent::Resized {
                                    width: logical_width,
                                    height: logical_height,
                                    scale_factor: scale,
                                });
                                if response.exit {
                                    REQUEST_EXIT.store(true, Ordering::SeqCst);
                                }
                            }
                        }
                        MainEvent::GainedFocus => {
                            info!("App gained focus (Resumed)");
                            let response = send_event(PlatformEvent::Resumed);
                            if response.exit {
                                REQUEST_EXIT.store(true, Ordering::SeqCst);
                            }
                        }
                        MainEvent::LostFocus => {
                            info!("App lost focus (Suspended)");
                            let response = send_event(PlatformEvent::Suspended);
                            if response.exit {
                                REQUEST_EXIT.store(true, Ordering::SeqCst);
                            }
                        }
                        MainEvent::LowMemory => {
                            info!("Low memory warning");
                            let _ = send_event(PlatformEvent::MemoryWarning);
                        }
                        MainEvent::Destroy => {
                            info!("Destroy received, exiting");
                            REQUEST_EXIT.store(true, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        });

        // Handle input events separately (touch, keyboard)
        if let Ok(mut input_iter) = app.input_events_iter() {
            loop {
                if !input_iter.next(|event| {
                    match event {
                        InputEvent::MotionEvent(motion_event) => {
                            let events = handle_motion_event(&motion_event);
                            for e in events {
                                let response = send_event(e);
                                if response.exit {
                                    REQUEST_EXIT.store(true, Ordering::SeqCst);
                                }
                                if response.request_redraw {
                                    REQUEST_REDRAW.store(true, Ordering::SeqCst);
                                }
                            }
                            InputStatus::Handled
                        }
                        InputEvent::KeyEvent(key_event) => {
                            // handle_key_event returns multiple events (KeyPressed + optional TextInput)
                            for e in handle_key_event(&key_event) {
                                let response = send_event(e);
                                if response.exit {
                                    REQUEST_EXIT.store(true, Ordering::SeqCst);
                                }
                                if response.request_redraw {
                                    REQUEST_REDRAW.store(true, Ordering::SeqCst);
                                }
                            }
                            InputStatus::Handled
                        }
                        _ => InputStatus::Unhandled,
                    }
                }) {
                    break;
                }
            }
        }

        // Process queued input from JNI callbacks (software keyboard)
        // This must be done on the main thread where ANDROID_CALLBACK is registered
        if process_queued_input() {
            // If we processed input, request a redraw
            REQUEST_REDRAW.store(true, Ordering::SeqCst);
        }
    }

    info!("android_main: event loop exited");
}

/// Handle window initialization
fn handle_init_window(app: &AndroidApp) {
    if let Some(window) = app.native_window() {
        // Enable edge-to-edge rendering first (before we measure window size)
        // This allows rendering behind system bars and into notch/cutout area
        enable_edge_to_edge();

        // Get physical pixel dimensions from the native window
        let physical_width = window.width() as u32;
        let physical_height = window.height() as u32;

        // Get scale factor from JNI (display density)
        let scale = get_scale_factor_jni().unwrap_or(1.0) as f64;
        SCALE_FACTOR.with(|s| *s.borrow_mut() = scale);

        // Convert to logical pixels for Go's layout system
        // (iOS UIKit returns logical points, Android NDK returns physical pixels)
        let logical_width = physical_width as f64 / scale;
        let logical_height = physical_height as f64 / scale;

        info!("Window size: {}x{} physical, {}x{} logical, scale: {}",
              physical_width, physical_height, logical_width, logical_height, scale);

        // Initialize wgpu backend
        let mut new_backend = WgpuBackend::new();
        let native_handle = NativeHandle {
            a_native_window: window.ptr().as_ptr() as *mut std::ffi::c_void,
        };

        // Surface config uses physical pixels for GPU rendering
        let surface_config = SurfaceConfig {
            width: physical_width,
            height: physical_height,
            scale_factor: scale,
            vsync: true,
            low_power_gpu: false,
            allow_software_fallback: false,
        };

        if let Err(e) = pollster::block_on(new_backend.init_with_window(&native_handle, surface_config)) {
            info!("Failed to init wgpu: {:?}", e);
            return;
        }

        // Store backend globally for FFI access
        {
            let backend_lock = crate::ffi::get_backend();
            if let Ok(mut guard) = backend_lock.lock() {
                *guard = Some(new_backend);
            }
        }

        // Update safe area insets
        update_safe_area_insets();

        // Call Go's AndroidMain to start the Go app
        // This is the gomobile-generated entry point
        // Flow:
        // 1. call_go_android_main spawns a thread that calls Go's AndroidMain
        // 2. Go's AndroidMain → ffi.Run → runAndroid
        // 3. runAndroid calls fnAndroidSetReadyCallback (sets GO_READY_CALLBACK)
        // 4. runAndroid blocks with select {}
        // 5. We wait briefly, then call GO_READY_CALLBACK
        // 6. GO_READY_CALLBACK (androidReadyCallback) calls fnAppRun
        // 7. fnAppRun calls run_android_app which calls register_callback
        // 8. Now ANDROID_CALLBACK is set and we can send events
        if let Err(e) = call_go_android_main() {
            info!("Failed to call Go AndroidMain: {:?}", e);
        }

        // Wait for Go to register the ready callback with retry loop
        // Go's AndroidMain runs in a separate thread and may take 50-100ms to initialize
        let mut callback_found = false;
        for i in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            unsafe {
                if GO_READY_CALLBACK.is_some() {
                    info!("GO_READY_CALLBACK found after {}ms", (i + 1) * 50);
                    callback_found = true;
                    break;
                }
            }
        }

        if !callback_found {
            info!("Warning: GO_READY_CALLBACK not set after 1s, Go may not have initialized");
        }

        // Call Go's ready callback - this triggers androidReadyCallback which
        // calls fnAppRun to register the event handler
        unsafe {
            if let Some(callback) = GO_READY_CALLBACK {
                info!("Calling Go ready callback (androidReadyCallback)");
                callback();
                // Wait briefly for fnAppRun to complete and register ANDROID_CALLBACK
                std::thread::sleep(std::time::Duration::from_millis(50));
            } else {
                info!("Warning: GO_READY_CALLBACK not set, cannot call ready callback");
            }
        }

        // Send Ready event to the registered callback with logical dimensions
        // (Go's layout system works in logical pixels, like iOS points)
        let response = send_event(PlatformEvent::Ready {
            width: logical_width,
            height: logical_height,
            scale_factor: scale,
        });

        if response.exit {
            REQUEST_EXIT.store(true, Ordering::SeqCst);
        }

        APP_READY.with(|r| *r.borrow_mut() = true);
    }
}

// ============================================================================
// JNI Helper Functions
// ============================================================================

/// Call Go's AndroidMain function via JNI in a separate thread
/// This is the gomobile-generated entry point for the Go app.
/// The class name is android_demo/Android_demo (from package android_demo, func AndroidMain)
///
/// We spawn a new thread because Go's ffi.Run blocks (select {}),
/// and we don't want to block the Rust event loop.
fn call_go_android_main() -> Result<(), Box<dyn std::error::Error>> {
    let vm = unsafe { JAVA_VM.as_ref() }.ok_or("JavaVM not available")?;

    // Check if we have cached the Go class
    let has_cached = unsafe { GO_ANDROID_MAIN_CLASS.is_some() && METHOD_GO_ANDROID_MAIN.is_some() };

    // Clone the JavaVM pointer for the thread
    let vm_ptr = vm as *const jni::JavaVM as usize;

    info!("Spawning thread to call Go AndroidMain (cached: {})", has_cached);

    std::thread::spawn(move || {
        // Reinterpret the pointer as JavaVM reference
        let vm = unsafe { &*(vm_ptr as *const jni::JavaVM) };

        // Attach this thread to the JVM
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                info!("Failed to attach Go thread to JVM: {:?}", e);
                return;
            }
        };

        info!("Calling Go AndroidMain via JNI");

        // Use cached class and method if available
        unsafe {
            if let (Some(class_ref), Some(method_id)) = (&GO_ANDROID_MAIN_CLASS, METHOD_GO_ANDROID_MAIN) {
                // Convert GlobalRef to JClass - the GlobalRef wraps a JClass object
                let class: JClass = JClass::from_raw(class_ref.as_raw());

                // Call using cached references
                match env.call_static_method_unchecked(
                    &class,
                    method_id,
                    ReturnType::Primitive(Primitive::Void),
                    &[],
                ) {
                    Ok(_) => info!("Go AndroidMain returned (unexpected)"),
                    Err(e) => info!("Failed to call Go AndroidMain: {:?}", e),
                }
                return;
            }
        }

        // Fall back to finding class (won't work from native thread, but try anyway)
        let class = match env.find_class("android_demo/Android_demo") {
            Ok(c) => c,
            Err(e) => {
                info!("Failed to find Go class: {:?}", e);
                let _ = env.exception_clear();
                return;
            }
        };

        // Call the static androidMain method
        // This blocks in Go (select {}), so it never returns
        match env.call_static_method(
            class,
            "androidMain",
            "()V",
            &[],
        ) {
            Ok(_) => info!("Go AndroidMain returned (unexpected)"),
            Err(e) => info!("Failed to call Go AndroidMain: {:?}", e),
        }
    });

    Ok(())
}

/// Update safe area insets from JNI
fn update_safe_area_insets() {
    if let Some(insets) = get_safe_area_insets_jni() {
        SAFE_AREA.with(|s| *s.borrow_mut() = insets);
    }
}

/// Enable edge-to-edge rendering (render behind system bars and into notch area)
/// This should be called early during window initialization
fn enable_edge_to_edge() {
    let vm = match unsafe { JAVA_VM.as_ref() } {
        Some(vm) => vm,
        None => {
            info!("enable_edge_to_edge: JavaVM not available");
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            info!("enable_edge_to_edge: Failed to attach thread: {:?}", e);
            return;
        }
    };

    let activity = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    });

    let activity = match activity {
        Some(ptr) if !ptr.is_null() => unsafe { JObject::from_raw(ptr as *mut _) },
        _ => {
            info!("enable_edge_to_edge: Activity not available");
            return;
        }
    };

    // Get the Window from the Activity
    let window = match env.call_method(&activity, "getWindow", "()Landroid/view/Window;", &[]) {
        Ok(w) => match w.l() {
            Ok(obj) => obj,
            Err(_) => {
                info!("enable_edge_to_edge: Failed to get Window object");
                return;
            }
        },
        Err(e) => {
            info!("enable_edge_to_edge: getWindow failed: {:?}", e);
            let _ = env.exception_clear();
            return;
        }
    };

    // Try setDecorFitsSystemWindows(false) for API 30+ (modern edge-to-edge)
    // This method may not exist on older Android versions
    let result = env.call_method(&window, "setDecorFitsSystemWindows", "(Z)V", &[JValue::Bool(0)]);
    if result.is_err() || env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        info!("enable_edge_to_edge: setDecorFitsSystemWindows not available (pre-API 30)");

        // Fallback for older Android: use system UI flags
        // SYSTEM_UI_FLAG_LAYOUT_STABLE | SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
        let flags: i32 = 0x00000100 | 0x00000400 | 0x00000200;

        // Get DecorView
        if let Ok(decor_view_result) = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[]) {
            if let Ok(decor_view) = decor_view_result.l() {
                let _ = env.call_method(&decor_view, "setSystemUiVisibility", "(I)V", &[JValue::Int(flags)]);
                let _ = env.exception_clear();
            }
        }
    } else {
        info!("enable_edge_to_edge: setDecorFitsSystemWindows(false) succeeded");
    }

    // Set display cutout mode for notch support (API 28+)
    // LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES = 1
    // Need to get WindowManager.LayoutParams and set layoutInDisplayCutoutMode
    if let Ok(attrs_result) = env.call_method(&window, "getAttributes", "()Landroid/view/WindowManager$LayoutParams;", &[]) {
        if let Ok(attrs) = attrs_result.l() {
            // Try to set the field directly - layoutInDisplayCutoutMode = 1 (SHORT_EDGES)
            let field_result = env.set_field(&attrs, "layoutInDisplayCutoutMode", "I", JValue::Int(1));
            if field_result.is_err() || env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
                info!("enable_edge_to_edge: layoutInDisplayCutoutMode field not available (pre-API 28)");
            } else {
                // Apply the modified attributes back to the window
                let _ = env.call_method(&window, "setAttributes", "(Landroid/view/WindowManager$LayoutParams;)V", &[JValue::Object(&attrs)]);
                let _ = env.exception_clear();
                info!("enable_edge_to_edge: Set layoutInDisplayCutoutMode=SHORT_EDGES");
            }
        }
    }

    // Make status bar and navigation bar transparent
    // setStatusBarColor(Color.TRANSPARENT) and setNavigationBarColor(Color.TRANSPARENT)
    let transparent: i32 = 0; // Color.TRANSPARENT = 0
    let _ = env.call_method(&window, "setStatusBarColor", "(I)V", &[JValue::Int(transparent)]);
    let _ = env.exception_clear();
    let _ = env.call_method(&window, "setNavigationBarColor", "(I)V", &[JValue::Int(transparent)]);
    let _ = env.exception_clear();

    info!("enable_edge_to_edge: Configuration complete");
}

/// Get safe area insets via JNI call to activity
/// Falls back to standard Android APIs if custom method not available
fn get_safe_area_insets_jni() -> Option<SafeAreaInsets> {
    let vm = unsafe { JAVA_VM.as_ref()? };
    let mut env = vm.attach_current_thread().ok()?;

    // Get activity instance
    let activity = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    })?;

    if activity.is_null() {
        return Some(SafeAreaInsets::default());
    }

    let activity_obj = unsafe { JObject::from_raw(activity as *mut _) };

    // Try custom method first (if using CenteredActivity)
    let result = env.call_method(&activity_obj, "getSafeAreaInsets", "()[F", &[]);

    if result.is_ok() && !env.exception_check().unwrap_or(true) {
        let array = match result.ok()?.l() {
            Ok(a) => a,
            Err(_) => return get_safe_area_fallback(&mut env, &activity_obj),
        };

        // Get the float array values
        let mut insets = [0.0f32; 4];
        unsafe {
            let float_array = jni::objects::JFloatArray::from_raw(array.as_raw());
            if env.get_float_array_region(&float_array, 0, &mut insets).is_ok() {
                return Some(SafeAreaInsets {
                    top: insets[0] as f64,
                    left: insets[1] as f64,
                    bottom: insets[2] as f64,
                    right: insets[3] as f64,
                });
            }
        }
    }

    let _ = env.exception_clear();
    get_safe_area_fallback(&mut env, &activity_obj)
}

/// Fallback safe area calculation using standard Android APIs
fn get_safe_area_fallback(env: &mut JNIEnv, activity: &JObject) -> Option<SafeAreaInsets> {
    // Get density for converting pixels to dp
    let density = {
        let resources = env.call_method(activity, "getResources", "()Landroid/content/res/Resources;", &[]).ok()?.l().ok()?;
        let metrics = env.call_method(&resources, "getDisplayMetrics", "()Landroid/util/DisplayMetrics;", &[]).ok()?.l().ok()?;
        env.get_field(&metrics, "density", "F").ok()?.f().ok().unwrap_or(1.0)
    };

    let mut top: f32 = 0.0;
    let mut bottom: f32 = 0.0;

    // Try to get status bar height from resources
    // getResources().getIdentifier("status_bar_height", "dimen", "android")
    // getResources().getDimensionPixelSize(id)
    let resources = match env.call_method(activity, "getResources", "()Landroid/content/res/Resources;", &[]) {
        Ok(r) => match r.l() {
            Ok(obj) => obj,
            Err(_) => return Some(SafeAreaInsets::default()),
        },
        Err(_) => {
            let _ = env.exception_clear();
            return Some(SafeAreaInsets::default());
        }
    };

    // Get status bar height
    let status_bar_id = {
        let name = env.new_string("status_bar_height").ok()?;
        let dimen = env.new_string("dimen").ok()?;
        let android = env.new_string("android").ok()?;
        match env.call_method(
            &resources,
            "getIdentifier",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
            &[JValue::Object(&name), JValue::Object(&dimen), JValue::Object(&android)],
        ) {
            Ok(id) => id.i().ok().unwrap_or(0),
            Err(_) => {
                let _ = env.exception_clear();
                0
            }
        }
    };

    if status_bar_id > 0 {
        if let Ok(height) = env.call_method(&resources, "getDimensionPixelSize", "(I)I", &[JValue::Int(status_bar_id)]) {
            if let Ok(h) = height.i() {
                top = h as f32 / density;
            }
        }
        let _ = env.exception_clear();
    }

    // Get navigation bar height
    let nav_bar_id = {
        let name = env.new_string("navigation_bar_height").ok()?;
        let dimen = env.new_string("dimen").ok()?;
        let android = env.new_string("android").ok()?;
        match env.call_method(
            &resources,
            "getIdentifier",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
            &[JValue::Object(&name), JValue::Object(&dimen), JValue::Object(&android)],
        ) {
            Ok(id) => id.i().ok().unwrap_or(0),
            Err(_) => {
                let _ = env.exception_clear();
                0
            }
        }
    };

    if nav_bar_id > 0 {
        if let Ok(height) = env.call_method(&resources, "getDimensionPixelSize", "(I)I", &[JValue::Int(nav_bar_id)]) {
            if let Ok(h) = height.i() {
                bottom = h as f32 / density;
            }
        }
        let _ = env.exception_clear();
    }

    info!("Safe area fallback: top={}, bottom={}, density={}", top, bottom, density);

    Some(SafeAreaInsets {
        top: top as f64,
        left: 0.0,
        bottom: bottom as f64,
        right: 0.0,
    })
}

/// Get scale factor via JNI using standard Android APIs
/// Falls back to 1.0 if unable to determine
fn get_scale_factor_jni() -> Option<f32> {
    let vm = unsafe { JAVA_VM.as_ref()? };
    let mut env = vm.attach_current_thread().ok()?;

    let activity = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    })?;

    if activity.is_null() {
        return Some(1.0);
    }

    let activity_obj = unsafe { JObject::from_raw(activity as *mut _) };

    // First try custom method (if using CenteredActivity)
    let result = env.call_method(&activity_obj, "getScaleFactor", "()F", &[]);

    // Clear any pending exception and try standard API
    if result.is_err() || env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();

        // Use standard Android API: activity.getResources().getDisplayMetrics().density
        let resources = match env.call_method(&activity_obj, "getResources", "()Landroid/content/res/Resources;", &[]) {
            Ok(r) => r,
            Err(_) => {
                let _ = env.exception_clear();
                return Some(1.0);
            }
        };

        let resources_obj = match resources.l() {
            Ok(r) => r,
            Err(_) => return Some(1.0),
        };

        let metrics = match env.call_method(&resources_obj, "getDisplayMetrics", "()Landroid/util/DisplayMetrics;", &[]) {
            Ok(m) => m,
            Err(_) => {
                let _ = env.exception_clear();
                return Some(1.0);
            }
        };

        let metrics_obj = match metrics.l() {
            Ok(m) => m,
            Err(_) => return Some(1.0),
        };

        let density = match env.get_field(&metrics_obj, "density", "F") {
            Ok(d) => d,
            Err(_) => {
                let _ = env.exception_clear();
                return Some(1.0);
            }
        };

        return density.f().ok().or(Some(1.0));
    }

    result.ok()?.f().ok()
}

// ============================================================================
// Public JNI functions called from Rust FFI layer
// ============================================================================

/// Show soft keyboard
pub fn show_keyboard() {
    let vm = match unsafe { JAVA_VM.as_ref() } {
        Some(vm) => vm,
        None => {
            info!("show_keyboard: JavaVM not available");
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            info!("show_keyboard: Failed to attach thread: {:?}", e);
            return;
        }
    };

    // Clear any pending exceptions before making JNI calls
    let _ = env.exception_clear();

    let activity_ptr = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    });

    let activity_ptr = match activity_ptr {
        Some(ptr) if !ptr.is_null() => ptr,
        _ => {
            info!("show_keyboard: Activity not available");
            return;
        }
    };

    // Use ManuallyDrop to prevent JObject from deleting the reference we don't own
    let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

    let result = env.call_method(&*activity, "showSoftKeyboard", "()V", &[]);
    // Check for and log any Java exceptions
    if env.exception_check().unwrap_or(false) {
        info!("show_keyboard: Java exception occurred");
        let _ = env.exception_describe(); // Print exception to logcat
        let _ = env.exception_clear();
    }
    if let Err(e) = result {
        info!("show_keyboard: JNI call failed: {:?}", e);
    } else {
        info!("show_keyboard: JNI call succeeded");
    }
}

/// Hide soft keyboard
pub fn hide_keyboard() {
    let vm = match unsafe { JAVA_VM.as_ref() } {
        Some(vm) => vm,
        None => {
            info!("hide_keyboard: JavaVM not available");
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            info!("hide_keyboard: Failed to attach thread: {:?}", e);
            return;
        }
    };

    // Clear any pending exceptions before making JNI calls
    let _ = env.exception_clear();

    let activity_ptr = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    });

    let activity_ptr = match activity_ptr {
        Some(ptr) if !ptr.is_null() => ptr,
        _ => {
            info!("hide_keyboard: Activity not available");
            return;
        }
    };

    // Use ManuallyDrop to prevent JObject from deleting the reference we don't own
    let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

    let result = env.call_method(&*activity, "hideSoftKeyboard", "()V", &[]);
    // Check for and log any Java exceptions
    if env.exception_check().unwrap_or(false) {
        info!("hide_keyboard: Java exception occurred");
        let _ = env.exception_describe(); // Print exception to logcat
        let _ = env.exception_clear();
    }
    if let Err(e) = result {
        info!("hide_keyboard: JNI call failed: {:?}", e);
    } else {
        info!("hide_keyboard: JNI call succeeded");
    }
}

/// Check if keyboard is currently visible
pub fn is_keyboard_visible() -> bool {
    let vm = match unsafe { JAVA_VM.as_ref() } {
        Some(vm) => vm,
        None => return false,
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(_) => return false,
    };

    // Clear any pending exceptions before making JNI calls
    let _ = env.exception_clear();

    let activity_ptr = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    });

    let activity_ptr = match activity_ptr {
        Some(ptr) if !ptr.is_null() => ptr,
        _ => return false,
    };

    // Use ManuallyDrop to prevent JObject from deleting the reference we don't own
    let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

    let result = env.call_method(&*activity, "isKeyboardVisible", "()Z", &[]);
    // Always clear exceptions after JNI calls
    let _ = env.exception_clear();
    match result {
        Ok(r) => r.z().unwrap_or(false),
        Err(e) => {
            info!("is_keyboard_visible: JNI call failed: {:?}", e);
            false
        }
    }
}

/// Trigger haptic feedback
/// style: 0=Light, 1=Medium, 2=Heavy, 3=Selection, 4=Success, 5=Warning, 6=Error
pub fn haptic_feedback(style: i32) {
    let vm = match unsafe { JAVA_VM.as_ref() } {
        Some(vm) => vm,
        None => {
            info!("haptic_feedback: JavaVM not available");
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            info!("haptic_feedback: Failed to attach thread: {:?}", e);
            return;
        }
    };

    // Clear any pending exceptions before making JNI calls
    let _ = env.exception_clear();

    let activity_ptr = ANDROID_APP.with(|a| {
        a.borrow().as_ref().map(|app| app.activity_as_ptr())
    });

    let activity_ptr = match activity_ptr {
        Some(ptr) if !ptr.is_null() => ptr,
        _ => {
            info!("haptic_feedback: Activity not available");
            return;
        }
    };

    // Create a JObject from the raw pointer
    // We use ManuallyDrop to prevent the JObject from trying to delete the reference
    // since we don't own this reference
    let activity = std::mem::ManuallyDrop::new(unsafe { JObject::from_raw(activity_ptr as *mut _) });

    let result = env.call_method(&*activity, "hapticFeedback", "(I)V", &[JValue::Int(style)]);
    // Always clear exceptions after JNI calls
    let _ = env.exception_clear();
    if let Err(e) = result {
        info!("haptic_feedback: JNI call failed: {:?}", e);
    }
}

/// Get current scale factor
pub fn scale_factor() -> f64 {
    SCALE_FACTOR.with(|s| *s.borrow())
}

// ============================================================================
// FFI exports for Go
// ============================================================================

/// Show software keyboard (called from Go via FFI)
#[no_mangle]
pub extern "C" fn centered_android_keyboard_show() {
    show_keyboard();
}

/// Hide software keyboard (called from Go via FFI)
#[no_mangle]
pub extern "C" fn centered_android_keyboard_hide() {
    hide_keyboard();
}

/// Trigger haptic feedback (called from Go via FFI)
#[no_mangle]
pub extern "C" fn centered_android_haptic_feedback(style: i32) {
    haptic_feedback(style);
}

/// Get safe area insets (called from Go via FFI)
/// Returns a pointer to a static SafeAreaInsets struct
#[no_mangle]
pub extern "C" fn centered_android_get_safe_area_insets(out: *mut SafeAreaInsets) {
    if !out.is_null() {
        let insets = AndroidBackend::safe_area_insets();
        unsafe {
            *out = insets;
        }
    }
}

/// Get scale factor (called from Go via FFI)
#[no_mangle]
pub extern "C" fn centered_android_get_scale_factor() -> f64 {
    scale_factor()
}

/// Render a frame using the Android backend
/// Called from FFI when Go submits render commands on Android
pub fn render_frame(commands: &[crate::render::RenderCommand]) -> Result<(), Box<dyn Error>> {
    let backend_lock = crate::ffi::get_backend();
    let mut guard = backend_lock.lock().map_err(|e| format!("Lock error: {}", e))?;
    if let Some(ref mut b) = *guard {
        b.render_frame(commands)
    } else {
        Err("Android backend not initialized".into())
    }
}

// ============================================================================
// JNI exports for Kotlin (keyboard text input)
// ============================================================================

/// Called from Kotlin when text is entered via the software keyboard.
/// This is called from the Android UI thread, so we queue the text and
/// request a redraw so it gets processed on the main Rust thread.
#[no_mangle]
pub extern "system" fn Java_com_centered_demo_CenteredActivity_nativeOnTextInput(
    mut env: JNIEnv,
    _class: JClass,
    text: jni::objects::JString,
) {
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    if text_str.is_empty() {
        return;
    }

    info!("Text input from keyboard: '{}'", text_str);

    // Queue the text input for processing on the main thread
    if let Ok(mut queue) = PENDING_TEXT_INPUT.lock() {
        queue.push(text_str);
    }
    // Request a redraw so the main event loop processes the queued input
    REQUEST_REDRAW.store(true, Ordering::SeqCst);
}

/// Called from Kotlin when a special key is pressed (backspace, enter, etc.)
/// This is called from the Android UI thread, so we queue the event for
/// processing on the main Rust thread.
#[no_mangle]
pub extern "system" fn Java_com_centered_demo_CenteredActivity_nativeOnKeyEvent(
    _env: JNIEnv,
    _class: JClass,
    key_code: jni::sys::jint,
    action: jni::sys::jint,
) -> jni::sys::jboolean {
    // Only handle key down events
    if action != 0 {  // ACTION_DOWN = 0
        return 0;
    }

    info!("Key event from keyboard: keycode={}, action={}", key_code, action);

    // Queue the key event for processing on the main thread
    if let Ok(mut queue) = PENDING_KEY_EVENTS.lock() {
        queue.push((key_code, action));
    }
    // Request a redraw so the main event loop processes the queued input
    REQUEST_REDRAW.store(true, Ordering::SeqCst);

    1 // true - we handled the event
}

/// Called from Kotlin when the keyboard height changes (appears/disappears).
/// This allows the app to scroll content to keep focused inputs visible.
/// This is called from the Android UI thread, so we queue the event for
/// processing on the main Rust thread.
#[no_mangle]
pub extern "system" fn Java_com_centered_demo_CenteredActivity_nativeOnKeyboardHeightChanged(
    _env: JNIEnv,
    _class: JClass,
    height: jni::sys::jfloat,
    animation_duration_ms: jni::sys::jint,
) {
    info!("Keyboard height changed: height={}, duration_ms={}", height, animation_duration_ms);

    // Queue the keyboard height change for processing on the main thread
    if let Ok(mut pending) = PENDING_KEYBOARD_HEIGHT.lock() {
        *pending = Some((height, animation_duration_ms));
    }
    // Request a redraw so the main event loop processes the queued event
    REQUEST_REDRAW.store(true, Ordering::SeqCst);
}
