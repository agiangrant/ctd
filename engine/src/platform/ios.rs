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
use objc2::runtime::AnyObject;
use objc2::{class, declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_foundation::{
    CGFloat, CGPoint, CGRect, CGSize, MainThreadMarker, NSObject, NSObjectProtocol, NSSet,
    NSString,
};
use objc2_ui_kit::{
    UIApplication, UIApplicationDelegate, UIEvent, UIInterfaceOrientationMask, UIRectEdge,
    UIResponder, UIScreen, UITouch, UITouchPhase, UIView, UIViewController, UIWindow,
};

use super::backend::{AppConfig as BackendAppConfig, EventResponse, PlatformEvent, SafeAreaInsets};
use super::wgpu_backend::{SurfaceConfig, WgpuBackend};

// Thread-local state for iOS (everything runs on main thread)
thread_local! {
    static IOS_CALLBACK: RefCell<Option<Box<dyn FnMut(PlatformEvent) -> EventResponse>>> = RefCell::new(None);
    static IOS_BACKEND: RefCell<Option<WgpuBackend>> = RefCell::new(None);
    static IOS_VIEW: RefCell<Option<Retained<MetalView>>> = RefCell::new(None);
    static IOS_WINDOW: RefCell<Option<Retained<UIWindow>>> = RefCell::new(None);
    static IOS_DISPLAY_LINK: RefCell<Option<Retained<AnyObject>>> = RefCell::new(None);
    static IOS_DISPLAY_LINK_HANDLER: RefCell<Option<Retained<DisplayLinkHandler>>> = RefCell::new(None);
    static SAFE_AREA: RefCell<SafeAreaInsets> = RefCell::new(SafeAreaInsets::default());
    static SCALE_FACTOR: RefCell<f64> = RefCell::new(1.0);
    static APP_READY: RefCell<bool> = RefCell::new(false);
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
fn send_event(event: PlatformEvent) -> EventResponse {
    IOS_CALLBACK.with(|cb| {
        if let Some(ref mut callback) = *cb.borrow_mut() {
            callback(event)
        } else {
            EventResponse::default()
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
            println!("[MetalView] displayLayer called");

            // Send RedrawRequested event to Go
            let response = send_event(PlatformEvent::RedrawRequested);

            // Get render commands from response and render them
            // The commands come back through the callback mechanism
            // For now, just trigger a redraw cycle

            // Render using wgpu backend
            IOS_BACKEND.with(|backend| {
                if let Some(ref mut b) = *backend.borrow_mut() {
                    // The render commands should have been collected by send_event
                    // We need to execute them here
                    // For now, just present an empty frame
                    println!("[MetalView] Rendering frame via wgpu backend");
                }
            });
        }

        #[method(layoutSubviews)]
        fn layout_subviews(&self) {
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };

            let bounds = self.bounds();
            let scale = self.contentScaleFactor();

            println!("[MetalView] layoutSubviews: {}x{} @ {}x",
                bounds.size.width, bounds.size.height, scale);

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

            // Resize wgpu backend
            let physical_width = (bounds.size.width * scale) as u32;
            let physical_height = (bounds.size.height * scale) as u32;

            IOS_BACKEND.with(|backend| {
                if let Some(ref mut b) = *backend.borrow_mut() {
                    let _ = b.resize(physical_width, physical_height, scale);
                }
            });

            SCALE_FACTOR.with(|sf| *sf.borrow_mut() = scale);

            // Update safe area insets
            update_safe_area_insets(self);

            // Send resize event
            let response = send_event(PlatformEvent::Resized {
                width: bounds.size.width,
                height: bounds.size.height,
                scale_factor: scale,
            });

            if response.request_redraw {
                self.set_needs_display();
            }
        }

        #[method(touchesBegan:withEvent:)]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            for touch in touches.iter() {
                let location = touch.locationInView(Some(self));
                let touch_id = touch as *const UITouch as u64;
                let response = send_event(PlatformEvent::TouchBegan {
                    id: touch_id,
                    x: location.x,
                    y: location.y,
                });
                if response.request_redraw {
                    self.set_needs_display();
                }
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
                if response.request_redraw {
                    self.set_needs_display();
                }
            }
        }

        #[method(touchesEnded:withEvent:)]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            for touch in touches.iter() {
                let location = touch.locationInView(Some(self));
                let touch_id = touch as *const UITouch as u64;
                let response = send_event(PlatformEvent::TouchEnded {
                    id: touch_id,
                    x: location.x,
                    y: location.y,
                });
                if response.request_redraw {
                    self.set_needs_display();
                }
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
                if response.request_redraw {
                    self.set_needs_display();
                }
            }
        }

        #[method(canBecomeFirstResponder)]
        fn can_become_first_responder(&self) -> bool {
            true
        }

        // CADisplayLink target method
        #[method(displayLinkFired:)]
        fn display_link_fired(&self, _display_link: *mut AnyObject) {
            // Send RedrawRequested event to Go callback
            let _response = send_event(PlatformEvent::RedrawRequested);
        }
    }

    unsafe impl NSObjectProtocol for MetalView {}
);

impl MetalView {
    fn new(mtm: MainThreadMarker, frame: CGRect) -> Retained<Self> {
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
            println!("[DisplayLink] render: called");

            // Send RedrawRequested event directly to Go callback
            let response = send_event(PlatformEvent::RedrawRequested);

            println!("[DisplayLink] response.request_redraw = {}", response.request_redraw);
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
    println!("[iOS] Starting CADisplayLink...");

    // Use the MetalView as the target (it's already stored and retained)
    IOS_VIEW.with(|v| {
        if let Some(ref view) = *v.borrow() {
            println!("[iOS] Using MetalView as display link target");

            // Create CADisplayLink targeting MetalView's displayLinkFired: method
            let display_link: Retained<AnyObject> = unsafe {
                msg_send_id![
                    class!(CADisplayLink),
                    displayLinkWithTarget: &**view,
                    selector: sel!(displayLinkFired:)
                ]
            };
            println!("[iOS] CADisplayLink created");

            // Add to main run loop - use currentRunLoop and common modes
            let current_run_loop: *mut AnyObject = unsafe { msg_send![class!(NSRunLoop), currentRunLoop] };
            println!("[iOS] Got current run loop: {:?}", current_run_loop);

            // NSRunLoopCommonModes
            extern "C" {
                static NSRunLoopCommonModes: *const AnyObject;
            }
            let common_modes: *const AnyObject = unsafe { NSRunLoopCommonModes };
            println!("[iOS] Common modes: {:?}", common_modes);

            let _: () = unsafe { msg_send![&*display_link, addToRunLoop: current_run_loop, forMode: common_modes] };
            println!("[iOS] CADisplayLink added to run loop");

            // Store to keep alive
            IOS_DISPLAY_LINK.with(|dl| *dl.borrow_mut() = Some(display_link));

            println!("[iOS] CADisplayLink started");
        } else {
            println!("[iOS] ERROR: MetalView not found for display link");
        }
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
            println!("[VC] viewWillTransitionToSize: {}x{}", size.width, size.height);

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
            println!("[AppDelegate] didFinishLaunchingWithOptions");

            let mtm = MainThreadMarker::new().unwrap();

            // Get screen bounds
            let screen = UIScreen::mainScreen(mtm);
            let screen_bounds = screen.bounds();
            let scale = screen.scale();

            println!("[AppDelegate] Screen: {}x{} @ {}x",
                screen_bounds.size.width, screen_bounds.size.height, scale);

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
            println!("[AppDelegate] Initializing wgpu backend");
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
                    println!("[AppDelegate] wgpu backend initialized successfully");
                    IOS_BACKEND.with(|b| *b.borrow_mut() = Some(backend));
                }
                Err(e) => {
                    println!("[AppDelegate] ERROR: Failed to initialize wgpu backend: {}", e);
                }
            }

            // Mark app as ready
            APP_READY.with(|ready| *ready.borrow_mut() = true);

            // Call Go's ready callback if registered
            // This allows Go to set up its event handler
            unsafe {
                if let Some(callback) = GO_READY_CALLBACK {
                    println!("[AppDelegate] Calling Go ready callback");
                    callback();
                } else {
                    println!("[AppDelegate] No Go ready callback registered");
                }
            }

            // Send ready event (Go callback should be registered now)
            let response = send_event(PlatformEvent::Ready {
                width: screen_bounds.size.width,
                height: screen_bounds.size.height,
                scale_factor: scale,
            });

            if response.request_redraw {
                metal_view.set_needs_display();
            }

            // Start the display link for continuous rendering
            start_display_link(mtm);

            true
        }

        #[method(applicationDidBecomeActive:)]
        fn did_become_active(&self, _application: &UIApplication) {
            println!("[AppDelegate] applicationDidBecomeActive");
            let response = send_event(PlatformEvent::Resumed);
            if response.request_redraw {
                IOS_VIEW.with(|v| {
                    if let Some(ref view) = *v.borrow() {
                        view.set_needs_display();
                    }
                });
            }
        }

        #[method(applicationWillResignActive:)]
        fn will_resign_active(&self, _application: &UIApplication) {
            println!("[AppDelegate] applicationWillResignActive");
            send_event(PlatformEvent::Suspended);
        }

        #[method(applicationDidReceiveMemoryWarning:)]
        fn did_receive_memory_warning(&self, _application: &UIApplication) {
            println!("[AppDelegate] applicationDidReceiveMemoryWarning");
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

    println!("[iOS] Safe area insets: top={}, left={}, bottom={}, right={}",
        insets.top, insets.left, insets.bottom, insets.right);
}

// ============================================================================
// Class Registration
// ============================================================================

/// Register the AppDelegate class with the Objective-C runtime.
/// This must be called before UIApplicationMain so the class can be found by name.
pub fn register_app_delegate_class() {
    // Accessing the class triggers registration with the ObjC runtime
    let _cls = AppDelegate::class();
    println!("[iOS] AppDelegate class registered: {:?}", _cls);
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
pub fn init_wgpu_backend(config: SurfaceConfig) -> Result<(), Box<dyn Error>> {
    let backend = WgpuBackend::new();

    IOS_BACKEND.with(|b| {
        *b.borrow_mut() = Some(backend);
    });

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
    IOS_BACKEND.with(|backend| {
        if let Some(ref mut b) = *backend.borrow_mut() {
            b.render_frame(commands)
        } else {
            Err("iOS backend not initialized".into())
        }
    })
}
