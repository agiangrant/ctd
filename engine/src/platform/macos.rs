//! macOS platform backend using AppKit directly
//!
//! This provides native window management for macOS without winit.

#![cfg(target_os = "macos")]

use std::cell::Cell;
use std::error::Error;
use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{class, declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_foundation::{
    CGFloat, CGPoint, CGRect, CGSize, MainThreadMarker, NSObject, NSObjectProtocol,
    NSPoint, NSRect, NSSize, NSString,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSEvent, NSEventModifierFlags, NSEventType, NSResponder, NSView, NSWindow,
    NSWindowDelegate, NSWindowStyleMask,
};

use super::backend::{AppConfig, EventCallback, EventResponse, PlatformEvent, SafeAreaInsets};
use super::wgpu_backend::{SurfaceConfig, WgpuBackend};

// Global state for the macOS backend
static MACOS_STATE: OnceLock<Mutex<MacosState>> = OnceLock::new();

struct MacosState {
    callback: Option<EventCallback>,
    backend: Option<WgpuBackend>,
    window: Option<Retained<NSWindow>>,
    view: Option<Retained<MetalView>>,
    scale_factor: f64,
    request_exit: bool,
    running: bool,
}

impl Default for MacosState {
    fn default() -> Self {
        Self {
            callback: None,
            backend: None,
            window: None,
            view: None,
            scale_factor: 1.0,
            request_exit: false,
            running: false,
        }
    }
}

fn get_state() -> &'static Mutex<MacosState> {
    MACOS_STATE.get_or_init(|| Mutex::new(MacosState::default()))
}

/// Send an event to the callback and handle the response
fn send_event(event: PlatformEvent) -> EventResponse {
    let mut state = get_state().lock().unwrap();
    if let Some(ref mut callback) = state.callback {
        callback(event)
    } else {
        EventResponse::default()
    }
}

// ============================================================================
// MetalView - NSView subclass with CAMetalLayer
// ============================================================================

struct MetalViewState {
    tracking_area: Cell<*mut AnyObject>,
}

declare_class!(
    pub struct MetalView;

    unsafe impl ClassType for MetalView {
        #[inherits(NSResponder, NSObject)]
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CenteredMetalView";
    }

    impl DeclaredClass for MetalView {
        type Ivars = MetalViewState;
    }

    unsafe impl MetalView {
        // Use CAMetalLayer as the backing layer
        #[method(wantsLayer)]
        fn wants_layer(&self) -> bool {
            true
        }

        #[method(makeBackingLayer)]
        fn make_backing_layer(&self) -> *mut AnyObject {
            unsafe { msg_send![class!(CAMetalLayer), layer] }
        }

        #[method(viewDidChangeBackingProperties)]
        fn view_did_change_backing_properties(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeBackingProperties] };
            self.update_layer_size();
        }

        #[method(setFrameSize:)]
        fn set_frame_size(&self, new_size: NSSize) {
            let _: () = unsafe { msg_send![super(self), setFrameSize: new_size] };
            self.update_layer_size();
        }

        #[method(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[method(mouseDown:)]
        fn mouse_down(&self, event: &NSEvent) {
            let location = self.convert_point(event);
            let response = send_event(PlatformEvent::PointerPressed {
                x: location.0,
                y: location.1,
                button: 0,
            });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(mouseUp:)]
        fn mouse_up(&self, event: &NSEvent) {
            let location = self.convert_point(event);
            let response = send_event(PlatformEvent::PointerReleased {
                x: location.0,
                y: location.1,
                button: 0,
            });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(mouseMoved:)]
        fn mouse_moved(&self, event: &NSEvent) {
            let location = self.convert_point(event);
            let response = send_event(PlatformEvent::PointerMoved {
                x: location.0,
                y: location.1,
            });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(mouseDragged:)]
        fn mouse_dragged(&self, event: &NSEvent) {
            let location = self.convert_point(event);
            let response = send_event(PlatformEvent::PointerMoved {
                x: location.0,
                y: location.1,
            });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(rightMouseDown:)]
        fn right_mouse_down(&self, event: &NSEvent) {
            let location = self.convert_point(event);
            let response = send_event(PlatformEvent::PointerPressed {
                x: location.0,
                y: location.1,
                button: 1,
            });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(rightMouseUp:)]
        fn right_mouse_up(&self, event: &NSEvent) {
            let location = self.convert_point(event);
            let response = send_event(PlatformEvent::PointerReleased {
                x: location.0,
                y: location.1,
                button: 1,
            });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(scrollWheel:)]
        fn scroll_wheel(&self, event: &NSEvent) {
            let dx = unsafe { event.scrollingDeltaX() };
            let dy = unsafe { event.scrollingDeltaY() };
            let response = send_event(PlatformEvent::Scroll { dx, dy });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(keyDown:)]
        fn key_down(&self, event: &NSEvent) {
            let keycode = unsafe { event.keyCode() } as u32;
            let modifiers = unsafe { event.modifierFlags() }.bits() as u32;

            let response = send_event(PlatformEvent::KeyPressed { keycode, modifiers });

            // Also send text input if applicable
            if let Some(chars) = unsafe { event.characters() } {
                let text = chars.to_string();
                if !text.is_empty() {
                    send_event(PlatformEvent::TextInput { text });
                }
            }

            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }

        #[method(keyUp:)]
        fn key_up(&self, event: &NSEvent) {
            let keycode = unsafe { event.keyCode() } as u32;
            let modifiers = unsafe { event.modifierFlags() }.bits() as u32;
            let response = send_event(PlatformEvent::KeyReleased { keycode, modifiers });
            if response.request_redraw {
                self.setNeedsDisplay(true);
            }
        }
    }

    unsafe impl NSObjectProtocol for MetalView {}
);

impl MetalView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(MetalViewState {
            tracking_area: Cell::new(std::ptr::null_mut()),
        });
        let this: Retained<Self> = unsafe { msg_send_id![super(this), initWithFrame: frame] };

        // Set up layer
        this.setWantsLayer(true);

        // Configure CAMetalLayer
        let layer: *mut AnyObject = unsafe { msg_send![&*this, layer] };
        if !layer.is_null() {
            let pixel_format: usize = 80; // MTLPixelFormatBGRA8Unorm
            let _: () = unsafe { msg_send![layer, setPixelFormat: pixel_format] };
        }

        // Set up tracking area for mouse moved events
        this.setup_tracking_area();

        this
    }

    fn setup_tracking_area(&self) {
        let bounds = self.bounds();

        // NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved | NSTrackingActiveInKeyWindow | NSTrackingInVisibleRect
        let options: usize = 0x01 | 0x02 | 0x20 | 0x200;

        let tracking_area: *mut AnyObject = unsafe {
            let area: *mut AnyObject = msg_send![class!(NSTrackingArea), alloc];
            msg_send![area, initWithRect: bounds, options: options, owner: self as *const _ as *mut AnyObject, userInfo: std::ptr::null::<AnyObject>()]
        };

        let _: () = unsafe { msg_send![self, addTrackingArea: tracking_area] };
        self.ivars().tracking_area.set(tracking_area);
    }

    fn convert_point(&self, event: &NSEvent) -> (f64, f64) {
        let window_point = unsafe { event.locationInWindow() };
        let view_point: NSPoint = unsafe { msg_send![self, convertPoint: window_point, fromView: std::ptr::null::<NSView>()] };

        // Flip Y coordinate (AppKit has origin at bottom-left)
        let bounds = self.bounds();
        (view_point.x, bounds.size.height - view_point.y)
    }

    fn update_layer_size(&self) {
        let bounds = self.bounds();
        let scale: CGFloat = unsafe {
            if let Some(window) = self.window() {
                window.backingScaleFactor()
            } else {
                1.0
            }
        };

        let layer: *mut AnyObject = unsafe { msg_send![self, layer] };
        if !layer.is_null() {
            let drawable_size = CGSize {
                width: bounds.size.width * scale,
                height: bounds.size.height * scale,
            };
            let _: () = unsafe { msg_send![layer, setDrawableSize: drawable_size] };
            let _: () = unsafe { msg_send![layer, setContentsScale: scale] };
        }

        // Resize wgpu backend
        let physical_width = (bounds.size.width * scale) as u32;
        let physical_height = (bounds.size.height * scale) as u32;

        {
            let mut state = get_state().lock().unwrap();
            if let Some(ref mut backend) = state.backend {
                let _ = backend.resize(physical_width, physical_height, scale);
            }
            state.scale_factor = scale;
        }

        // Send resize event
        let response = send_event(PlatformEvent::Resized {
            width: bounds.size.width,
            height: bounds.size.height,
            scale_factor: scale,
        });

        if response.request_redraw {
            self.setNeedsDisplay(true);
        }
    }

    fn metal_layer(&self) -> *mut std::ffi::c_void {
        unsafe { msg_send![self, layer] }
    }
}

// ============================================================================
// WindowDelegate - handles window events
// ============================================================================

declare_class!(
    pub struct WindowDelegate;

    unsafe impl ClassType for WindowDelegate {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CenteredWindowDelegate";
    }

    impl DeclaredClass for WindowDelegate {
        type Ivars = ();
    }

    unsafe impl WindowDelegate {
        #[method(windowWillClose:)]
        fn window_will_close(&self, _notification: *mut AnyObject) {
            println!("[WindowDelegate] windowWillClose");
            let response = send_event(PlatformEvent::CloseRequested);
            if response.exit {
                let mut state = get_state().lock().unwrap();
                state.request_exit = true;
            }
        }

        #[method(windowDidResize:)]
        fn window_did_resize(&self, _notification: *mut AnyObject) {
            // The view's setFrameSize handles this
        }

        #[method(windowDidBecomeKey:)]
        fn window_did_become_key(&self, _notification: *mut AnyObject) {
            send_event(PlatformEvent::Resumed);
        }

        #[method(windowDidResignKey:)]
        fn window_did_resign_key(&self, _notification: *mut AnyObject) {
            send_event(PlatformEvent::Suspended);
        }
    }

    unsafe impl NSObjectProtocol for WindowDelegate {}
    unsafe impl NSWindowDelegate for WindowDelegate {}
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(());
        unsafe { msg_send_id![super(this), init] }
    }
}

// ============================================================================
// PlatformBackend implementation
// ============================================================================

pub struct MacosBackend;

impl super::backend::PlatformBackend for MacosBackend {
    fn run(config: AppConfig, callback: EventCallback) -> Result<(), Box<dyn Error>> {
        // Store the callback in global state
        {
            let mut state = get_state().lock().unwrap();
            state.callback = Some(callback);
            state.running = true;
        }

        let mtm = MainThreadMarker::new().expect("Must run on main thread");

        // Initialize NSApplication
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        // Create window
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let frame = NSRect {
            origin: NSPoint { x: 100.0, y: 100.0 },
            size: NSSize {
                width: config.width as f64,
                height: config.height as f64,
            },
        };

        let window = unsafe {
            let window: Retained<NSWindow> = msg_send_id![
                mtm.alloc::<NSWindow>(),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ];
            window
        };

        // Set window title
        let title = NSString::from_str(&config.title);
        window.setTitle(&title);

        // Create window delegate
        let delegate = WindowDelegate::new(mtm);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        // Get content rect for view
        let content_rect = window.contentRectForFrameRect(frame);

        // Create metal view
        let view_frame = NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: content_rect.size,
        };
        let metal_view = MetalView::new(mtm, view_frame);

        // Set as content view
        window.setContentView(Some(&metal_view));

        // Get scale factor
        let scale = window.backingScaleFactor();

        // Store window and view
        {
            let mut state = get_state().lock().unwrap();
            state.window = Some(window.clone());
            state.view = Some(metal_view.clone());
            state.scale_factor = scale;
        }

        // Initialize wgpu backend
        {
            let mut backend = WgpuBackend::new();
            let wgpu_config = SurfaceConfig {
                width: (content_rect.size.width * scale) as u32,
                height: (content_rect.size.height * scale) as u32,
                scale_factor: scale,
                vsync: true,
                low_power_gpu: false,
                allow_software_fallback: false,
            };

            // TODO: Initialize backend with metal layer
            // For now, we'll need to implement init_with_metal_layer on WgpuBackend

            let mut state = get_state().lock().unwrap();
            state.backend = Some(backend);
        }

        // Show window
        window.makeKeyAndOrderFront(None);
        app.activate();

        // Send ready event
        let response = send_event(PlatformEvent::Ready {
            width: content_rect.size.width,
            height: content_rect.size.height,
            scale_factor: scale,
        });

        if response.request_redraw {
            metal_view.setNeedsDisplay(true);
        }

        // Run event loop
        // Note: This is a simplified event loop. For production, you'd want to use
        // NSApplication's run method or a proper run loop integration.
        loop {
            {
                let state = get_state().lock().unwrap();
                if state.request_exit || !state.running {
                    break;
                }
            }

            // Process events
            unsafe {
                let event: Option<Retained<NSEvent>> = msg_send_id![
                    &*app,
                    nextEventMatchingMask: NSEventType::Any.0 as u64,
                    untilDate: std::ptr::null::<AnyObject>(),
                    inMode: NSString::from_str("kCFRunLoopDefaultMode") as *const NSString,
                    dequeue: true
                ];

                if let Some(event) = event {
                    app.sendEvent(&event);
                }
            }

            // Check for redraw
            let needs_redraw = {
                let state = get_state().lock().unwrap();
                state.view.is_some()
            };

            if needs_redraw {
                // Render frame
                send_event(PlatformEvent::RedrawRequested);
            }

            // Small sleep to avoid spinning
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        Ok(())
    }

    fn request_redraw() {
        let state = get_state().lock().unwrap();
        if let Some(ref view) = state.view {
            view.setNeedsDisplay(true);
        }
    }

    fn request_exit() {
        let mut state = get_state().lock().unwrap();
        state.request_exit = true;
    }

    fn safe_area_insets() -> SafeAreaInsets {
        // macOS doesn't have safe area insets like iOS
        SafeAreaInsets::default()
    }
}

/// Get the wgpu backend for rendering
pub fn get_backend() -> Option<std::sync::MutexGuard<'static, MacosState>> {
    Some(get_state().lock().unwrap())
}

/// Request a redraw from any thread
pub fn request_redraw() {
    MacosBackend::request_redraw();
}
