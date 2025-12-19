//! iOS platform backend using UIKit directly
//!
//! This bypasses winit and manages UIKit lifecycle directly for proper
//! rotation handling and touch event delivery.

#![cfg(target_os = "ios")]

use std::cell::RefCell;
use std::error::Error;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
use objc2::{class, declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use std::ffi::CStr;
use objc2_foundation::{
    CGFloat, CGRect, CGSize, MainThreadMarker, NSDictionary, NSNotification,
    NSNotificationCenter, NSNotificationName, NSNumber, NSObject, NSObjectProtocol, NSSet,
    NSString, NSValue,
};
use block2::RcBlock;
use std::ptr::NonNull;
use objc2_ui_kit::{
    UIApplication, UIApplicationDelegate, UIEvent, UIInterfaceOrientationMask, UIRectEdge,
    UIResponder, UIScreen, UITouch, UITouchPhase, UIView, UIViewController, UIWindow,
};

use super::backend::{AppConfig as BackendAppConfig, EventResponse, PlatformEvent, SafeAreaInsets};
use super::wgpu_backend::{SurfaceConfig, WgpuBackend};

// Thread-local state for iOS (everything runs on main thread)
thread_local! {
    static IOS_CALLBACK: RefCell<Option<Box<dyn FnMut(PlatformEvent) -> EventResponse>>> = RefCell::new(None);
    // NOTE: Backend is now stored in the global BACKEND in ffi.rs to share with video/audio/image loading
    static IOS_VIEW: RefCell<Option<Retained<MetalView>>> = RefCell::new(None);
    static IOS_WINDOW: RefCell<Option<Retained<UIWindow>>> = RefCell::new(None);
    static IOS_DISPLAY_LINK: RefCell<Option<Retained<AnyObject>>> = RefCell::new(None);
    static IOS_DISPLAY_LINK_HANDLER: RefCell<Option<Retained<DisplayLinkHandler>>> = RefCell::new(None);
    static SAFE_AREA: RefCell<SafeAreaInsets> = RefCell::new(SafeAreaInsets::default());
    static SCALE_FACTOR: RefCell<f64> = RefCell::new(1.0);
    static APP_READY: RefCell<bool> = RefCell::new(false);
    // Keyboard notification observers (must be kept alive)
    static KEYBOARD_SHOW_OBSERVER: RefCell<Option<Retained<NSObject>>> = RefCell::new(None);
    static KEYBOARD_HIDE_OBSERVER: RefCell<Option<Retained<NSObject>>> = RefCell::new(None);
    // Timer for delayed redraws (cursor blink, etc.)
    static IOS_REDRAW_TIMER: RefCell<Option<Retained<AnyObject>>> = RefCell::new(None);
    // Whether continuous rendering is active (display link running)
    static CONTINUOUS_RENDER: RefCell<bool> = RefCell::new(true);
    // Track if we've rendered at least one frame (prevents going idle before first render)
    static HAS_RENDERED_FRAME: RefCell<bool> = RefCell::new(false);
    // Grace period - keep rendering until this time (allows async ops like video to start)
    static RENDER_UNTIL: RefCell<Option<std::time::Instant>> = RefCell::new(None);
    // Target frames per second (configured at app init)
    static TARGET_FPS: RefCell<u32> = RefCell::new(60);
}

/// Set the target FPS for the render loop
pub fn set_target_fps(fps: u32) {
    let fps = fps.max(1); // Minimum 1 FPS
    TARGET_FPS.with(|f| *f.borrow_mut() = fps);

    // Update the display link's preferred frame rate if it exists
    IOS_DISPLAY_LINK.with(|dl| {
        if let Some(ref display_link) = *dl.borrow() {
            unsafe {
                // preferredFramesPerSecond is available on iOS 10+
                let _: () = msg_send![&**display_link, setPreferredFramesPerSecond: fps as i64];
            }
        }
    });
}

/// Get the target FPS
pub fn get_target_fps() -> u32 {
    TARGET_FPS.with(|f| *f.borrow())
}

// C callback type for Go's ready handler
type GoReadyCallback = unsafe extern "C" fn();
static mut GO_READY_CALLBACK: Option<GoReadyCallback> = None;

/// Register the callback that Rust will call when the iOS app is ready.
/// Go should call this before the app starts.
///
/// # Safety
/// Must be called from main thread before app starts.
#[no_mangle]
pub unsafe extern "C" fn centered_ios_set_ready_callback(callback: GoReadyCallback) {
    GO_READY_CALLBACK = Some(callback);
}

/// Called by Go to register its event callback after the app is ready.
/// This should be called from within the ready callback.
pub fn register_callback(callback: Box<dyn FnMut(PlatformEvent) -> EventResponse>) {
    IOS_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

// Atomic flag for exit request (can be set from any thread)
static REQUEST_EXIT: AtomicBool = AtomicBool::new(false);

/// Send an event to the callback and handle the response
/// Uses try_borrow_mut to handle re-entrant calls safely (e.g., when callback triggers another event)
fn send_event(event: PlatformEvent) -> EventResponse {
    IOS_CALLBACK.with(|cb| {
        // Use try_borrow_mut to handle re-entrant calls
        // If the callback is already borrowed (e.g., we're inside a callback that triggered another event),
        // return a default response instead of panicking
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
                // This can happen when the callback does something that triggers another event
                // (e.g., camera permission request, video input operations, etc.)
                EventResponse::default()
            }
        }
    })
}

// ============================================================================
// UIEdgeInsets - C struct for safe area insets
// ============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct UIEdgeInsets {
    top: CGFloat,
    left: CGFloat,
    bottom: CGFloat,
    right: CGFloat,
}

// Make it safe for objc2 msg_send
unsafe impl objc2::Encode for UIEdgeInsets {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct(
        "UIEdgeInsets",
        &[
            CGFloat::ENCODING,
            CGFloat::ENCODING,
            CGFloat::ENCODING,
            CGFloat::ENCODING,
        ],
    );
}

// ============================================================================
// MetalView - UIView subclass with CAMetalLayer
// ============================================================================

declare_class!(
    pub struct MetalView;

    unsafe impl ClassType for MetalView {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CenteredMetalView";
    }

    impl DeclaredClass for MetalView {
        type Ivars = ();
    }

    unsafe impl MetalView {
        // Use CAMetalLayer as the backing layer
        #[method(layerClass)]
        fn layer_class() -> &'static AnyObject {
            unsafe { msg_send![class!(CAMetalLayer), class] }
        }

        // CALayerDelegate method - called when layer needs to display
        #[method(displayLayer:)]
        fn display_layer(&self, _layer: *mut AnyObject) {

            // Send RedrawRequested event to Go
            let response = send_event(PlatformEvent::RedrawRequested);

            // Get render commands from response and render them
            // The commands come back through the callback mechanism
            // For now, just trigger a redraw cycle

            // Render using wgpu backend (use global backend from ffi.rs)
            {
                let backend_lock = crate::ffi::get_backend();
                if let Ok(mut guard) = backend_lock.lock() {
                    if let Some(ref mut _b) = *guard {
                        // The render commands should have been collected by send_event
                        // We need to execute them here
                        // For now, just present an empty frame
                        // println!("[MetalView] Rendering frame via wgpu backend");
                    }
                }
            }
        }

        #[method(layoutSubviews)]
        fn layout_subviews(&self) {
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };

            let bounds = self.bounds();
            let scale = self.contentScaleFactor();

            // Update CAMetalLayer drawable size
            let layer: *mut AnyObject = unsafe { msg_send![self, layer] };
            if !layer.is_null() {
                let drawable_size = CGSize {
                    width: bounds.size.width * scale,
                    height: bounds.size.height * scale,
                };
                let _: () = unsafe { msg_send![layer, setDrawableSize: drawable_size] };
                let _: () = unsafe { msg_send![layer, setContentsScale: scale] };
            }

            // Resize wgpu backend (use global backend from ffi.rs)
            let physical_width = (bounds.size.width * scale) as u32;
            let physical_height = (bounds.size.height * scale) as u32;

            {
                let backend_lock = crate::ffi::get_backend();
                if let Ok(mut guard) = backend_lock.lock() {
                    if let Some(ref mut b) = *guard {
                        let _ = b.resize(physical_width, physical_height, scale);
                    }
                }
            }

            SCALE_FACTOR.with(|sf| *sf.borrow_mut() = scale);

            // Update safe area insets
            update_safe_area_insets(self);

            // Send resize event
            let response = send_event(PlatformEvent::Resized {
                width: bounds.size.width,
                height: bounds.size.height,
                scale_factor: scale,
            });

            handle_event_response(&response);
        }

        #[method(touchesBegan:withEvent:)]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            // Extend grace period for touch events - allows async ops like video to start
            let grace_duration = std::time::Duration::from_millis(500);
            RENDER_UNTIL.with(|r| {
                *r.borrow_mut() = Some(std::time::Instant::now() + grace_duration);
            });

            for touch in touches.iter() {
                let location = touch.locationInView(Some(self));
                let touch_id = touch as *const UITouch as u64;
                let response = send_event(PlatformEvent::TouchBegan {
                    id: touch_id,
                    x: location.x,
                    y: location.y,
                });
                handle_event_response(&response);
            }
        }

        #[method(touchesMoved:withEvent:)]
        fn touches_moved(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            for touch in touches.iter() {
                let location = touch.locationInView(Some(self));
                let touch_id = touch as *const UITouch as u64;
                let response = send_event(PlatformEvent::TouchMoved {
                    id: touch_id,
                    x: location.x,
                    y: location.y,
                });
                handle_event_response(&response);
            }
        }

        #[method(touchesEnded:withEvent:)]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            // Extend grace period for touch end events (button releases, etc.)
            let grace_duration = std::time::Duration::from_millis(500);
            RENDER_UNTIL.with(|r| {
                *r.borrow_mut() = Some(std::time::Instant::now() + grace_duration);
            });

            for touch in touches.iter() {
                let location = touch.locationInView(Some(self));
                let touch_id = touch as *const UITouch as u64;
                let response = send_event(PlatformEvent::TouchEnded {
                    id: touch_id,
                    x: location.x,
                    y: location.y,
                });
                handle_event_response(&response);
            }
        }

        #[method(touchesCancelled:withEvent:)]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            for touch in touches.iter() {
                let location = touch.locationInView(Some(self));
                let touch_id = touch as *const UITouch as u64;
                let response = send_event(PlatformEvent::TouchCancelled {
                    id: touch_id,
                    x: location.x,
                    y: location.y,
                });
                handle_event_response(&response);
            }
        }

        #[method(canBecomeFirstResponder)]
        fn can_become_first_responder(&self) -> bool {
            true
        }

        // UIKeyInput protocol methods for keyboard input
        #[method(hasText)]
        fn has_text(&self) -> bool {
            // Return true to indicate we can receive text
            // This is required for UIKeyInput compliance
            true
        }

        #[method(insertText:)]
        fn insert_text(&self, text: &objc2_foundation::NSString) {
            // Convert NSString to Rust String
            let text_str = text.to_string();

            // Send TextInput event to Go
            send_event(PlatformEvent::TextInput { text: text_str });
        }

        #[method(deleteBackward)]
        fn delete_backward(&self) {
            // Send backspace as a KeyPressed event
            // Use FFI keycode 56 for backspace (not macOS keycode 0x33)
            send_event(PlatformEvent::KeyPressed {
                keycode: 56, // FFI KeyBackspace
                modifiers: 0,
            });
        }

        // CADisplayLink target method
        #[method(displayLinkFired:)]
        fn display_link_fired(&self, _display_link: *mut AnyObject) {
            // Check if we've already rendered a frame BEFORE this render
            // This ensures we don't go idle until after at least one full frame
            let was_rendered_before = HAS_RENDERED_FRAME.with(|h| *h.borrow());

            // Check if we're within a grace period (keeps rendering for async ops)
            let in_grace_period = RENDER_UNTIL.with(|r| {
                r.borrow().map(|until| std::time::Instant::now() < until).unwrap_or(false)
            });

            // Send RedrawRequested event to Go callback (this triggers rendering)
            let response = send_event(PlatformEvent::RedrawRequested);

            // Mark that we've now rendered at least one frame
            HAS_RENDERED_FRAME.with(|h| *h.borrow_mut() = true);

            // Handle rendering mode based on response
            // Only allow going idle if:
            // - We've rendered at least one frame before
            // - We're not within a grace period (for async ops like video)
            if !response.request_redraw && was_rendered_before && !in_grace_period {
                // No continuous rendering needed
                if response.redraw_after_ms > 0 {
                    // Schedule a delayed redraw for cursor blink, etc.
                    pause_display_link();
                    schedule_delayed_redraw(response.redraw_after_ms);
                } else {
                    // Nothing to do, pause rendering
                    pause_display_link();
                }
            }
            // If request_redraw is true, OR first frame, OR in grace period, keep display link running
        }

        // Timer callback for delayed redraws (cursor blink)
        #[method(timerFired:)]
        fn timer_fired(&self, _timer: *mut AnyObject) {
            // Clear the timer reference
            IOS_REDRAW_TIMER.with(|t| *t.borrow_mut() = None);

            // Send RedrawRequested event
            let response = send_event(PlatformEvent::RedrawRequested);

            // Handle response like display_link_fired
            if response.request_redraw {
                // Need continuous rendering, resume display link
                resume_display_link();
            } else if response.redraw_after_ms > 0 {
                // Schedule another delayed redraw
                schedule_delayed_redraw(response.redraw_after_ms);
            }
            // Otherwise, stay paused until an event triggers a redraw
        }

        // Hardware keyboard support - pressesBegan
        #[method(pressesBegan:withEvent:)]
        fn presses_began(&self, presses: &NSSet<AnyObject>, _event: Option<&UIEvent>) {
            for press in presses.iter() {
                unsafe {
                    // Get the key from UIPress
                    let key: *mut AnyObject = msg_send![&*press, key];
                    if key.is_null() {
                        continue;
                    }

                    // Get keyCode (UIKeyboardHIDUsage)
                    let key_code: i64 = msg_send![key, keyCode];

                    // Get characters for text input
                    let characters: *mut AnyObject = msg_send![key, characters];
                    if !characters.is_null() {
                        let chars_str: *const std::ffi::c_char = msg_send![characters, UTF8String];
                        if !chars_str.is_null() {
                            let text = std::ffi::CStr::from_ptr(chars_str).to_string_lossy().to_string();
                            if !text.is_empty() && !text.chars().all(|c| c.is_control()) {
                                send_event(PlatformEvent::TextInput { text });
                            }
                        }
                    }

                    // Convert HID key code to FFI keycode
                    let ffi_keycode = hid_to_ffi_keycode(key_code);
                    if ffi_keycode != 0 {
                        send_event(PlatformEvent::KeyPressed {
                            keycode: ffi_keycode,
                            modifiers: 0,
                        });
                    }
                }
            }
        }

        // Hardware keyboard support - pressesEnded
        #[method(pressesEnded:withEvent:)]
        fn presses_ended(&self, presses: &NSSet<AnyObject>, _event: Option<&UIEvent>) {
            for press in presses.iter() {
                unsafe {
                    let key: *mut AnyObject = msg_send![&*press, key];
                    if key.is_null() {
                        continue;
                    }

                    let key_code: i64 = msg_send![key, keyCode];
                    let ffi_keycode = hid_to_ffi_keycode(key_code);
                    if ffi_keycode != 0 {
                        send_event(PlatformEvent::KeyReleased {
                            keycode: ffi_keycode,
                            modifiers: 0,
                        });
                    }
                }
            }
        }

        // Hardware keyboard support - pressesCancelled
        #[method(pressesCancelled:withEvent:)]
        fn presses_cancelled(&self, presses: &NSSet<AnyObject>, _event: Option<&UIEvent>) {
            // Same as pressesEnded - send KeyReleased for each key
            for press in presses.iter() {
                unsafe {
                    let key: *mut AnyObject = msg_send![&*press, key];
                    if key.is_null() {
                        continue;
                    }

                    let key_code: i64 = msg_send![key, keyCode];
                    let ffi_keycode = hid_to_ffi_keycode(key_code);
                    if ffi_keycode != 0 {
                        send_event(PlatformEvent::KeyReleased {
                            keycode: ffi_keycode,
                            modifiers: 0,
                        });
                    }
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for MetalView {}
);

impl MetalView {
    fn new(mtm: MainThreadMarker, frame: CGRect) -> Retained<Self> {
        // Add UIKeyInput protocol conformance to the class at runtime
        // This is needed so iOS knows our view can receive keyboard input
        unsafe {
            extern "C" {
                fn objc_getProtocol(name: *const std::ffi::c_char) -> *const std::ffi::c_void;
                fn class_addProtocol(cls: *const std::ffi::c_void, protocol: *const std::ffi::c_void) -> bool;
            }

            let class_ptr = Self::class() as *const _ as *const std::ffi::c_void;
            let protocol_name = b"UIKeyInput\0";
            let protocol = objc_getProtocol(protocol_name.as_ptr() as *const std::ffi::c_char);
            if !protocol.is_null() {
                class_addProtocol(class_ptr, protocol);
            }

            // Also add UITextInputTraits which is required for keyboard customization
            let traits_name = b"UITextInputTraits\0";
            let traits_protocol = objc_getProtocol(traits_name.as_ptr() as *const std::ffi::c_char);
            if !traits_protocol.is_null() {
                class_addProtocol(class_ptr, traits_protocol);
            }
        }

        let this = mtm.alloc().set_ivars(());
        let this: Retained<Self> = unsafe { msg_send_id![super(this), initWithFrame: frame] };

        // Enable multi-touch
        this.setMultipleTouchEnabled(true);

        // Set autoresizing to fill parent
        let autoresizing_mask: usize = 0x12; // FlexibleWidth | FlexibleHeight
        let _: () = unsafe { msg_send![&*this, setAutoresizingMask: autoresizing_mask] };

        // Get screen scale and set contentScaleFactor explicitly
        // (the default may be 1.0 before the view is attached to a window)
        let screen = UIScreen::mainScreen(mtm);
        let screen_scale = screen.scale();
        let _: () = unsafe { msg_send![&*this, setContentScaleFactor: screen_scale] };

        // Configure CAMetalLayer
        let layer: *mut AnyObject = unsafe { msg_send![&*this, layer] };
        if !layer.is_null() {
            let pixel_format: usize = 80; // MTLPixelFormatBGRA8Unorm
            let _: () = unsafe { msg_send![layer, setPixelFormat: pixel_format] };
            let _: () = unsafe { msg_send![layer, setContentsScale: screen_scale] };

            let drawable_size = CGSize {
                width: frame.size.width * screen_scale,
                height: frame.size.height * screen_scale,
            };
            let _: () = unsafe { msg_send![layer, setDrawableSize: drawable_size] };

            // Set the view as the layer's delegate so displayLayer: gets called
            let _: () = unsafe { msg_send![layer, setDelegate: &*this] };
        }

        this
    }

    /// Get the CAMetalLayer pointer for wgpu surface creation
    fn metal_layer(&self) -> *mut c_void {
        unsafe { msg_send![self, layer] }
    }

    fn set_needs_display(&self) {
        let _: () = unsafe { msg_send![self, setNeedsDisplay] };
    }
}

/// Handle event response - resume display link if continuous rendering needed
fn handle_event_response(response: &EventResponse) {
    if response.request_redraw {
        // Need continuous rendering for animations, scrolling, etc.
        resume_display_link();
    } else if response.redraw_after_ms > 0 {
        // Need a delayed redraw (cursor blink, etc.)
        // If display link is paused, schedule a timer
        CONTINUOUS_RENDER.with(|cr| {
            if !*cr.borrow() {
                schedule_delayed_redraw(response.redraw_after_ms);
            }
        });
    }
}

/// Extend the render grace period - keeps rendering for a short while
/// to allow async operations (video playback, network loads) to start
fn extend_render_grace_period() {
    let grace_duration = std::time::Duration::from_millis(500); // 500ms grace period
    let new_until = std::time::Instant::now() + grace_duration;
    RENDER_UNTIL.with(|r| {
        let current = r.borrow().unwrap_or(std::time::Instant::now());
        // Only extend if new time is later
        if new_until > current {
            *r.borrow_mut() = Some(new_until);
        }
    });
}

/// Check if we're within the render grace period
fn is_within_grace_period() -> bool {
    RENDER_UNTIL.with(|r| {
        r.borrow().map(|until| std::time::Instant::now() < until).unwrap_or(false)
    })
}

// ============================================================================
// DisplayLinkHandler - receives CADisplayLink callbacks
// ============================================================================

declare_class!(
    pub struct DisplayLinkHandler;

    unsafe impl ClassType for DisplayLinkHandler {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CenteredDisplayLinkHandler";
    }

    impl DeclaredClass for DisplayLinkHandler {
        type Ivars = ();
    }

    unsafe impl DisplayLinkHandler {
        #[method(render:)]
        fn render(&self, _display_link: *mut AnyObject) {
            // Send RedrawRequested event directly to Go callback
            send_event(PlatformEvent::RedrawRequested);
        }
    }

    unsafe impl NSObjectProtocol for DisplayLinkHandler {}
);

impl DisplayLinkHandler {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(());
        unsafe { msg_send_id![super(this), init] }
    }
}

/// Start the display link for continuous rendering
fn start_display_link(_mtm: MainThreadMarker) {
    // Use the MetalView as the target (it's already stored and retained)
    IOS_VIEW.with(|v| {
        if let Some(ref view) = *v.borrow() {
            // Create CADisplayLink targeting MetalView's displayLinkFired: method
            let display_link: Retained<AnyObject> = unsafe {
                msg_send_id![
                    class!(CADisplayLink),
                    displayLinkWithTarget: &**view,
                    selector: sel!(displayLinkFired:)
                ]
            };

            // Set the preferred frame rate based on target FPS config
            let target_fps = TARGET_FPS.with(|f| *f.borrow());
            unsafe {
                let _: () = msg_send![&*display_link, setPreferredFramesPerSecond: target_fps as i64];
            }

            // Add to main run loop - use currentRunLoop and common modes
            let current_run_loop: *mut AnyObject = unsafe { msg_send![class!(NSRunLoop), currentRunLoop] };

            // NSRunLoopCommonModes
            extern "C" {
                static NSRunLoopCommonModes: *const AnyObject;
            }
            let common_modes: *const AnyObject = unsafe { NSRunLoopCommonModes };

            let _: () = unsafe { msg_send![&*display_link, addToRunLoop: current_run_loop, forMode: common_modes] };

            // Store to keep alive
            IOS_DISPLAY_LINK.with(|dl| *dl.borrow_mut() = Some(display_link));
        }
    });
}

/// Pause the display link to stop continuous rendering
fn pause_display_link() {
    CONTINUOUS_RENDER.with(|cr| {
        if *cr.borrow() {
            *cr.borrow_mut() = false;
            IOS_DISPLAY_LINK.with(|dl| {
                if let Some(ref display_link) = *dl.borrow() {
                    let _: () = unsafe { msg_send![&**display_link, setPaused: true] };
                }
            });
        }
    });
}

/// Resume the display link for continuous rendering
fn resume_display_link() {
    CONTINUOUS_RENDER.with(|cr| {
        if !*cr.borrow() {
            *cr.borrow_mut() = true;
            // Cancel any pending timer
            cancel_delayed_redraw();
            IOS_DISPLAY_LINK.with(|dl| {
                if let Some(ref display_link) = *dl.borrow() {
                    let _: () = unsafe { msg_send![&**display_link, setPaused: false] };
                }
            });
        }
    });
}

/// Schedule a one-shot redraw after the specified milliseconds
fn schedule_delayed_redraw(ms: u32) {
    // Cancel any existing timer
    cancel_delayed_redraw();

    // Create an NSTimer for the delayed redraw
    let seconds = ms as f64 / 1000.0;

    IOS_VIEW.with(|v| {
        if let Some(ref view) = *v.borrow() {
            // Create timer targeting the view's timerFired: method
            let timer: Retained<AnyObject> = unsafe {
                msg_send_id![
                    class!(NSTimer),
                    scheduledTimerWithTimeInterval: seconds,
                    target: &**view,
                    selector: sel!(timerFired:),
                    userInfo: std::ptr::null::<AnyObject>(),
                    repeats: false
                ]
            };
            IOS_REDRAW_TIMER.with(|t| *t.borrow_mut() = Some(timer));
        }
    });
}

/// Cancel any pending delayed redraw timer
fn cancel_delayed_redraw() {
    IOS_REDRAW_TIMER.with(|t| {
        if let Some(ref timer) = *t.borrow() {
            let _: () = unsafe { msg_send![&**timer, invalidate] };
        }
        *t.borrow_mut() = None;
    });
}

// ============================================================================
// ViewController - handles rotation
// ============================================================================

declare_class!(
    pub struct RootViewController;

    unsafe impl ClassType for RootViewController {
        #[inherits(UIResponder, NSObject)]
        type Super = UIViewController;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CenteredRootViewController";
    }

    impl DeclaredClass for RootViewController {
        type Ivars = ();
    }

    unsafe impl RootViewController {
        #[method(shouldAutorotate)]
        fn should_autorotate(&self) -> bool {
            true
        }

        #[method(supportedInterfaceOrientations)]
        fn supported_interface_orientations(&self) -> UIInterfaceOrientationMask {
            UIInterfaceOrientationMask::All
        }

        #[method(prefersStatusBarHidden)]
        fn prefers_status_bar_hidden(&self) -> bool {
            false
        }

        #[method(prefersHomeIndicatorAutoHidden)]
        fn prefers_home_indicator_auto_hidden(&self) -> bool {
            true
        }

        #[method(viewWillTransitionToSize:withTransitionCoordinator:)]
        fn view_will_transition_to_size(
            &self,
            size: CGSize,
            coordinator: *mut AnyObject,
        ) {
            // Call super - UIKit will call layoutSubviews automatically
            let _: () = unsafe {
                msg_send![super(self), viewWillTransitionToSize: size, withTransitionCoordinator: coordinator]
            };
        }
    }

    unsafe impl NSObjectProtocol for RootViewController {}
);

impl RootViewController {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(());
        unsafe { msg_send_id![super(this), init] }
    }
}

// ============================================================================
// AppDelegate
// ============================================================================

declare_class!(
    pub struct AppDelegate;

    unsafe impl ClassType for AppDelegate {
        #[inherits(NSObject)]
        type Super = UIResponder;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CenteredAppDelegate";
    }

    impl DeclaredClass for AppDelegate {
        type Ivars = ();
    }

    unsafe impl AppDelegate {
        #[method(application:didFinishLaunchingWithOptions:)]
        fn did_finish_launching(
            &self,
            _application: &UIApplication,
            _options: *mut AnyObject,
        ) -> bool {
            let mtm = MainThreadMarker::new().unwrap();

            // Get screen bounds
            let screen = UIScreen::mainScreen(mtm);
            let screen_bounds = screen.bounds();
            let scale = screen.scale();

            // Create window
            let window: Retained<UIWindow> = unsafe {
                msg_send_id![mtm.alloc::<UIWindow>(), initWithFrame: screen_bounds]
            };

            // Create view controller
            let view_controller = RootViewController::new(mtm);

            // Create metal view
            let metal_view = MetalView::new(mtm, screen_bounds);

            // Set up view hierarchy
            view_controller.setView(Some(&metal_view));
            window.setRootViewController(Some(&view_controller));
            window.makeKeyAndVisible();

            // Store in thread-local state
            IOS_WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
            IOS_VIEW.with(|v| *v.borrow_mut() = Some(metal_view.clone()));
            SCALE_FACTOR.with(|sf| *sf.borrow_mut() = scale);

            // Update safe area insets
            update_safe_area_insets(&metal_view);

            // Initialize wgpu backend with the metal view
            let physical_width = (screen_bounds.size.width * scale) as u32;
            let physical_height = (screen_bounds.size.height * scale) as u32;

            let view_ptr = &*metal_view as *const MetalView as *mut c_void;
            let native_handle = super::backend::NativeHandle {
                ns_view: view_ptr,
            };

            let mut backend = WgpuBackend::new();
            let config = SurfaceConfig {
                width: physical_width,
                height: physical_height,
                scale_factor: scale,
                vsync: true,
                low_power_gpu: false,
                allow_software_fallback: false,
            };

            match pollster::block_on(backend.init_with_window(&native_handle, config)) {
                Ok(()) => {
                    // Store in global backend (shared with FFI for video/audio/image loading)
                    crate::ffi::set_backend(backend);
                }
                Err(e) => {
                    eprintln!("[iOS] Failed to initialize wgpu backend: {}", e);
                }
            }

            // Mark app as ready
            APP_READY.with(|ready| *ready.borrow_mut() = true);

            // Call Go's ready callback if registered
            // This allows Go to set up its event handler
            unsafe {
                if let Some(callback) = GO_READY_CALLBACK {
                    callback();
                }
            }

            // Send ready event (Go callback should be registered now)
            let response = send_event(PlatformEvent::Ready {
                width: screen_bounds.size.width,
                height: screen_bounds.size.height,
                scale_factor: scale,
            });

            handle_event_response(&response);

            // Start the display link for continuous rendering
            start_display_link(mtm);

            // Set a longer grace period on startup (1 second) to allow
            // videos and other async content to load
            RENDER_UNTIL.with(|r| {
                *r.borrow_mut() = Some(std::time::Instant::now() + std::time::Duration::from_secs(1));
            });

            // Set up keyboard show/hide observers for keyboard avoidance
            setup_keyboard_observers();

            true
        }

        #[method(applicationDidBecomeActive:)]
        fn did_become_active(&self, _application: &UIApplication) {
            let response = send_event(PlatformEvent::Resumed);
            // Always resume display link when app becomes active
            if response.request_redraw {
                resume_display_link();
            }
        }

        #[method(applicationWillResignActive:)]
        fn will_resign_active(&self, _application: &UIApplication) {
            send_event(PlatformEvent::Suspended);
        }

        #[method(applicationDidReceiveMemoryWarning:)]
        fn did_receive_memory_warning(&self, _application: &UIApplication) {
            send_event(PlatformEvent::MemoryWarning);
        }
    }

    unsafe impl NSObjectProtocol for AppDelegate {}
    unsafe impl UIApplicationDelegate for AppDelegate {}
);

// ============================================================================
// Helper functions
// ============================================================================

fn update_safe_area_insets(view: &MetalView) {
    // Get safe area insets from the view
    // On iOS 11+, this returns the notch/home indicator insets
    let insets: UIEdgeInsets = unsafe { msg_send![view, safeAreaInsets] };

    SAFE_AREA.with(|sa| {
        *sa.borrow_mut() = SafeAreaInsets {
            top: insets.top,
            left: insets.left,
            bottom: insets.bottom,
            right: insets.right,
        };
    });
}

/// Set up observers for keyboard show/hide notifications.
/// This allows the app to know when the keyboard appears and its height,
/// so it can scroll content to keep text inputs visible.
fn setup_keyboard_observers() {
    let center = unsafe { NSNotificationCenter::defaultCenter() };

    // UIKeyboardWillShowNotification
    let show_name: &NSNotificationName = unsafe {
        &*NSString::from_str("UIKeyboardWillShowNotification")
    };

    // UIKeyboardWillHideNotification
    let hide_name: &NSNotificationName = unsafe {
        &*NSString::from_str("UIKeyboardWillHideNotification")
    };

    // Handler for keyboard will show
    let show_block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        let notification = unsafe { notification.as_ref() };
        let (height, duration) = extract_keyboard_info(notification);

        let response = send_event(PlatformEvent::KeyboardFrameChanged {
            height,
            animation_duration: duration,
        });

        // Keyboard show typically triggers scroll animations
        if response.request_redraw {
            resume_display_link();
        }
    });

    // Handler for keyboard will hide
    let hide_block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        let notification = unsafe { notification.as_ref() };
        let (_, duration) = extract_keyboard_info(notification);

        let response = send_event(PlatformEvent::KeyboardFrameChanged {
            height: 0.0,
            animation_duration: duration,
        });

        // Keyboard hide typically triggers scroll animations
        if response.request_redraw {
            resume_display_link();
        }
    });

    // Register observers and store them to keep alive
    let show_observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(show_name),
            None,
            None,
            &show_block,
        )
    };

    let hide_observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(hide_name),
            None,
            None,
            &hide_block,
        )
    };

    // Store observers in thread-local state to keep them alive
    KEYBOARD_SHOW_OBSERVER.with(|o| *o.borrow_mut() = Some(show_observer));
    KEYBOARD_HIDE_OBSERVER.with(|o| *o.borrow_mut() = Some(hide_observer));
}

/// Extract keyboard height and animation duration from notification userInfo.
fn extract_keyboard_info(notification: &NSNotification) -> (f64, f64) {
    let mut height = 0.0f64;
    let mut duration = 0.25f64; // Default animation duration

    unsafe {
        if let Some(user_info) = notification.userInfo() {
            // Get keyboard frame from UIKeyboardFrameEndUserInfoKey
            let frame_key = NSString::from_str("UIKeyboardFrameEndUserInfoKey");
            if let Some(frame_value) = user_info.objectForKey(&*frame_key) {
                // The value is an NSValue containing a CGRect
                let frame_value_ptr = &*frame_value as *const AnyObject as *const NSValue;
                let frame: CGRect = msg_send![frame_value_ptr, CGRectValue];
                height = frame.size.height;
            }

            // Get animation duration from UIKeyboardAnimationDurationUserInfoKey
            let duration_key = NSString::from_str("UIKeyboardAnimationDurationUserInfoKey");
            if let Some(duration_value) = user_info.objectForKey(&*duration_key) {
                let duration_value_ptr = &*duration_value as *const AnyObject as *const NSNumber;
                duration = msg_send![duration_value_ptr, doubleValue];
            }
        }
    }

    (height, duration)
}

// ============================================================================
// Class Registration
// ============================================================================

/// Register the AppDelegate class with the Objective-C runtime.
/// This must be called before UIApplicationMain so the class can be found by name.
pub fn register_app_delegate_class() {
    // Accessing the class triggers registration with the ObjC runtime
    let _ = AppDelegate::class();
}

// ============================================================================
// PlatformBackend implementation
// ============================================================================

pub struct IosBackend;

impl super::backend::PlatformBackend for IosBackend {
    fn run(
        _config: BackendAppConfig,
        callback: super::backend::EventCallback,
    ) -> Result<(), Box<dyn Error>> {
        // Store the callback in thread-local state
        IOS_CALLBACK.with(|cb| {
            *cb.borrow_mut() = Some(callback);
        });

        // Run UIApplicationMain - this never returns on iOS
        unsafe {
            extern "C" {
                fn UIApplicationMain(
                    argc: i32,
                    argv: *mut *mut i8,
                    principal_class_name: *const AnyObject,
                    delegate_class_name: *const AnyObject,
                ) -> i32;
            }

            let delegate_class = NSString::from_str("CenteredAppDelegate");

            UIApplicationMain(
                0,
                std::ptr::null_mut(),
                std::ptr::null(),
                &*delegate_class as *const NSString as *const AnyObject,
            );
        }

        Ok(())
    }

    fn request_redraw() {
        IOS_VIEW.with(|v| {
            if let Some(ref view) = *v.borrow() {
                view.set_needs_display();
            }
        });
    }

    fn request_exit() {
        // iOS doesn't support programmatic exit
        REQUEST_EXIT.store(true, Ordering::SeqCst);
    }

    fn safe_area_insets() -> SafeAreaInsets {
        SAFE_AREA.with(|sa| *sa.borrow())
    }
}

/// Initialize the wgpu backend with the metal view
/// Call this after the view is created
pub fn init_wgpu_backend(_config: SurfaceConfig) -> Result<(), Box<dyn Error>> {
    let backend = WgpuBackend::new();

    // Store in global backend (shared with FFI)
    crate::ffi::set_backend(backend);

    // TODO: Initialize surface with metal layer from IOS_VIEW
    // This requires adding init_with_metal_layer to WgpuBackend

    Ok(())
}

/// Get the current scale factor
pub fn scale_factor() -> f64 {
    SCALE_FACTOR.with(|sf| *sf.borrow())
}

/// Request a redraw from any context
pub fn request_redraw() {
    IOS_VIEW.with(|v| {
        if let Some(ref view) = *v.borrow() {
            view.set_needs_display();
        }
    });
}

/// Render a frame using the iOS backend
/// Called from FFI when Go submits render commands on iOS
pub fn render_frame(commands: &[crate::render::RenderCommand]) -> Result<(), Box<dyn Error>> {
    let backend_lock = crate::ffi::get_backend();
    let mut guard = backend_lock.lock().map_err(|e| format!("Lock error: {}", e))?;
    if let Some(ref mut b) = *guard {
        b.render_frame(commands)
    } else {
        Err("iOS backend not initialized".into())
    }
}

// ============================================================================
// Keyboard Functions
// ============================================================================

// Thread-local to track keyboard visibility
thread_local! {
    static KEYBOARD_VISIBLE: RefCell<bool> = RefCell::new(false);
}

/// Show the software keyboard
pub fn show_keyboard() {
    IOS_VIEW.with(|v| {
        if let Some(ref view) = *v.borrow() {
            unsafe {
                let result: bool = msg_send![&**view, becomeFirstResponder];
                if result {
                    KEYBOARD_VISIBLE.with(|kv| *kv.borrow_mut() = true);
                }
            }
        }
    });
}

/// Hide the software keyboard
pub fn hide_keyboard() {
    IOS_VIEW.with(|v| {
        if let Some(ref view) = *v.borrow() {
            unsafe {
                let _: bool = msg_send![&**view, resignFirstResponder];
                KEYBOARD_VISIBLE.with(|kv| *kv.borrow_mut() = false);
            }
        }
    });
}

/// Check if keyboard is visible
pub fn is_keyboard_visible() -> bool {
    KEYBOARD_VISIBLE.with(|kv| *kv.borrow())
}

/// Convert USB HID keyboard usage codes to FFI keycodes
/// See: https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf (page 53)
fn hid_to_ffi_keycode(hid_code: i64) -> u32 {
    match hid_code {
        // Letters A-Z (HID 0x04-0x1D -> FFI 0-25)
        0x04..=0x1D => (hid_code - 0x04) as u32,

        // Numbers 1-9, 0 (HID 0x1E-0x27 -> FFI 26-35)
        0x1E..=0x26 => (hid_code - 0x1E + 26) as u32, // 1-9
        0x27 => 35, // 0

        // Function keys F1-F12 (HID 0x3A-0x45 -> FFI 36-47)
        0x3A..=0x45 => (hid_code - 0x3A + 36) as u32,

        // Navigation
        0x52 => 48, // Up Arrow
        0x51 => 49, // Down Arrow
        0x50 => 50, // Left Arrow
        0x4F => 51, // Right Arrow
        0x4A => 52, // Home
        0x4D => 53, // End
        0x4B => 54, // Page Up
        0x4E => 55, // Page Down

        // Editing
        0x2A => 56, // Backspace/Delete
        0x4C => 57, // Delete Forward
        0x49 => 58, // Insert
        0x28 => 59, // Return/Enter
        0x2B => 60, // Tab
        0x29 => 61, // Escape
        0x2C => 62, // Space

        // Punctuation
        0x2D => 63, // Minus
        0x2E => 64, // Equal
        0x2F => 65, // Left Bracket
        0x30 => 66, // Right Bracket
        0x31 => 67, // Backslash
        0x33 => 68, // Semicolon
        0x34 => 69, // Quote
        0x35 => 70, // Grave/Backtick
        0x36 => 71, // Comma
        0x37 => 72, // Period
        0x38 => 73, // Slash

        _ => 0, // Unknown key
    }
}
