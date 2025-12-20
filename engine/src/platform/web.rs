//! Web platform backend using wasm-bindgen
//!
//! This provides web browser support via:
//! - Canvas element for rendering (WebGL/WebGPU via wgpu)
//! - requestAnimationFrame for the render loop
//! - DOM events for input handling
//! - Configurable FPS via frame skipping

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, HtmlCanvasElement, KeyboardEvent, MouseEvent, TouchEvent, WheelEvent,
    Window,
};

use super::backend::{AppConfig, EventCallback, EventResponse, PlatformEvent, SafeAreaInsets};

/// Convert JsValue to a boxed error for use with `?` operator
fn js_err(val: JsValue) -> Box<dyn Error> {
    let msg = val.as_string().unwrap_or_else(|| format!("{:?}", val));
    msg.into()
}

// Thread-local state for the web platform
thread_local! {
    static CALLBACK: RefCell<Option<Box<dyn FnMut(PlatformEvent) -> EventResponse>>> = RefCell::new(None);
    static CANVAS: RefCell<Option<HtmlCanvasElement>> = RefCell::new(None);
    static REQUEST_REDRAW: RefCell<bool> = RefCell::new(false);
    static REQUEST_EXIT: RefCell<bool> = RefCell::new(false);
    static CONTINUOUS_RENDER: RefCell<bool> = RefCell::new(true);
    static HAS_RENDERED_FRAME: RefCell<bool> = RefCell::new(false);
    static RENDER_UNTIL: RefCell<Option<f64>> = RefCell::new(None); // timestamp in ms
    static TARGET_FPS: RefCell<u32> = RefCell::new(60);
    static LAST_FRAME_TIME: RefCell<f64> = RefCell::new(0.0);
    static FRAME_COUNT: RefCell<u32> = RefCell::new(0);
    static NEXT_REDRAW_AT: RefCell<Option<f64>> = RefCell::new(None); // scheduled redraw timestamp
}

/// Set the target FPS for the web platform
pub fn set_target_fps(fps: u32) {
    TARGET_FPS.with(|f| *f.borrow_mut() = fps.max(1));
}

/// Get the current target FPS
pub fn get_target_fps() -> u32 {
    TARGET_FPS.with(|f| *f.borrow())
}

/// Get frame interval in milliseconds
fn get_frame_interval_ms() -> f64 {
    TARGET_FPS.with(|f| {
        let fps = *f.borrow();
        if fps == 0 {
            16.67
        } else {
            1000.0 / fps as f64
        }
    })
}

/// Extend the render grace period (for async operations like video playback)
pub fn extend_render_grace_period(duration_ms: u32) {
    let window = web_sys::window().expect("no window");
    let now = window.performance().expect("no performance").now();
    let until = now + duration_ms as f64;
    RENDER_UNTIL.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(current) = *r {
            if until > current {
                *r = Some(until);
            }
        } else {
            *r = Some(until);
        }
    });
}

/// Check if we should render this frame based on FPS limiting and idle state
fn should_render_frame(now: f64) -> bool {
    // Always render if we haven't rendered at least one frame
    if !HAS_RENDERED_FRAME.with(|h| *h.borrow()) {
        return true;
    }

    // Render if in continuous mode (animations, etc.)
    if CONTINUOUS_RENDER.with(|c| *c.borrow()) {
        return true;
    }

    // Render if we're in a grace period
    if let Some(until) = RENDER_UNTIL.with(|r| *r.borrow()) {
        if now < until {
            return true;
        }
    }

    // Render if we have a scheduled redraw
    if let Some(when) = NEXT_REDRAW_AT.with(|r| *r.borrow()) {
        if now >= when {
            return true;
        }
    }

    // Render if explicitly requested
    if REQUEST_REDRAW.with(|r| *r.borrow()) {
        return true;
    }

    false
}

/// Check if enough time has passed for the next frame (FPS limiting)
fn should_throttle_frame(now: f64) -> bool {
    let last = LAST_FRAME_TIME.with(|l| *l.borrow());
    let interval = get_frame_interval_ms();

    // Allow first frame
    if last == 0.0 {
        return false;
    }

    // Throttle if not enough time has passed
    now - last < interval
}

/// Web platform backend
pub struct WebBackend;

impl super::backend::PlatformBackend for WebBackend {
    fn run(config: AppConfig, callback: EventCallback) -> Result<(), Box<dyn Error>> {
        // Set up panic hook for better error messages
        console_error_panic_hook::set_once();

        // Set target FPS
        set_target_fps(config.target_fps);

        // Get the window and document
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;

        // Create or find the canvas element
        let canvas = create_or_find_canvas(&document, &config)?;
        let canvas_clone = canvas.clone();

        // Store canvas and callback
        CANVAS.with(|c| *c.borrow_mut() = Some(canvas.clone()));
        CALLBACK.with(|c| *c.borrow_mut() = Some(callback));

        // Set up initial grace period for startup
        let now = window.performance().expect("no performance").now();
        RENDER_UNTIL.with(|r| *r.borrow_mut() = Some(now + 1000.0)); // 1 second grace period

        // Get initial dimensions
        let rect = canvas.get_bounding_client_rect();
        let width = rect.width();
        let height = rect.height();
        let scale_factor = window.device_pixel_ratio();

        // Set canvas resolution to match device pixels
        canvas.set_width((width * scale_factor) as u32);
        canvas.set_height((height * scale_factor) as u32);

        // Send Ready event
        dispatch_event(PlatformEvent::Ready {
            width,
            height,
            scale_factor,
        });

        // Set up event listeners
        setup_event_listeners(&window, &document, &canvas)?;

        // Set up resize observer
        setup_resize_observer(&canvas_clone)?;

        // Start the animation loop
        start_animation_loop(&window)?;

        Ok(())
    }

    fn request_redraw() {
        REQUEST_REDRAW.with(|r| *r.borrow_mut() = true);
        CONTINUOUS_RENDER.with(|c| *c.borrow_mut() = true);
    }

    fn request_exit() {
        REQUEST_EXIT.with(|r| *r.borrow_mut() = true);
    }

    fn safe_area_insets() -> SafeAreaInsets {
        // Web browsers handle safe areas via CSS env() variables
        // For now, return zeros - can be enhanced to read CSS custom properties
        SafeAreaInsets::default()
    }
}

/// Create or find the canvas element
fn create_or_find_canvas(
    document: &Document,
    config: &AppConfig,
) -> Result<HtmlCanvasElement, Box<dyn Error>> {
    // Try to find existing canvas with id "centered-canvas"
    if let Some(element) = document.get_element_by_id("centered-canvas") {
        if let Ok(canvas) = element.dyn_into::<HtmlCanvasElement>() {
            return Ok(canvas);
        }
    }

    // Create a new canvas
    let canvas = document
        .create_element("canvas").map_err(js_err)?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| "failed to create canvas")?;

    canvas.set_id("centered-canvas");

    // Set initial size
    let style = canvas.style();
    style.set_property("width", &format!("{}px", config.width)).map_err(js_err)?;
    style.set_property("height", &format!("{}px", config.height)).map_err(js_err)?;
    style.set_property("display", "block").map_err(js_err)?;

    // Make canvas focusable for keyboard events
    canvas.set_tab_index(0);

    // Append to body or a container
    if let Some(container) = document.get_element_by_id("centered-container") {
        container.append_child(&canvas).map_err(js_err)?;
    } else if let Some(body) = document.body() {
        body.append_child(&canvas).map_err(js_err)?;
    } else {
        return Err("no body element".into());
    }

    Ok(canvas)
}

/// Set up DOM event listeners
fn setup_event_listeners(
    window: &Window,
    _document: &Document,
    canvas: &HtmlCanvasElement,
) -> Result<(), Box<dyn Error>> {
    // Mouse move
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            let rect = canvas_for_closure.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            extend_render_grace_period(500);
            dispatch_event(PlatformEvent::PointerMoved { x, y });
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Mouse down
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            let rect = canvas_for_closure.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            let button = event.button() as u8;
            extend_render_grace_period(500);
            dispatch_event(PlatformEvent::PointerPressed { x, y, button });
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Mouse up
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            let rect = canvas_for_closure.get_bounding_client_rect();
            let x = event.client_x() as f64 - rect.left();
            let y = event.client_y() as f64 - rect.top();
            let button = event.button() as u8;
            dispatch_event(PlatformEvent::PointerReleased { x, y, button });
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Touch start
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: TouchEvent| {
            event.prevent_default();
            let rect = canvas_for_closure.get_bounding_client_rect();
            let touches = event.changed_touches();
            for i in 0..touches.length() {
                if let Some(touch) = touches.get(i) {
                    let x = touch.client_x() as f64 - rect.left();
                    let y = touch.client_y() as f64 - rect.top();
                    let id = touch.identifier() as u64;
                    extend_render_grace_period(500);
                    dispatch_event(PlatformEvent::TouchBegan { id, x, y });
                }
            }
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("touchstart", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Touch move
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: TouchEvent| {
            event.prevent_default();
            let rect = canvas_for_closure.get_bounding_client_rect();
            let touches = event.changed_touches();
            for i in 0..touches.length() {
                if let Some(touch) = touches.get(i) {
                    let x = touch.client_x() as f64 - rect.left();
                    let y = touch.client_y() as f64 - rect.top();
                    let id = touch.identifier() as u64;
                    dispatch_event(PlatformEvent::TouchMoved { id, x, y });
                }
            }
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("touchmove", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Touch end
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: TouchEvent| {
            event.prevent_default();
            let rect = canvas_for_closure.get_bounding_client_rect();
            let touches = event.changed_touches();
            for i in 0..touches.length() {
                if let Some(touch) = touches.get(i) {
                    let x = touch.client_x() as f64 - rect.left();
                    let y = touch.client_y() as f64 - rect.top();
                    let id = touch.identifier() as u64;
                    dispatch_event(PlatformEvent::TouchEnded { id, x, y });
                }
            }
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("touchend", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Touch cancel
    {
        let canvas_for_closure = canvas.clone();
        let closure = Closure::wrap(Box::new(move |event: TouchEvent| {
            let rect = canvas_for_closure.get_bounding_client_rect();
            let touches = event.changed_touches();
            for i in 0..touches.length() {
                if let Some(touch) = touches.get(i) {
                    let x = touch.client_x() as f64 - rect.left();
                    let y = touch.client_y() as f64 - rect.top();
                    let id = touch.identifier() as u64;
                    dispatch_event(PlatformEvent::TouchCancelled { id, x, y });
                }
            }
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("touchcancel", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Wheel/scroll
    {
        let closure = Closure::wrap(Box::new(move |event: WheelEvent| {
            event.prevent_default();
            let dx = event.delta_x();
            let dy = event.delta_y();
            extend_render_grace_period(100);
            dispatch_event(PlatformEvent::Scroll { dx, dy });
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Keyboard events (need focus on canvas)
    {
        let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let keycode = event.key_code();
            let modifiers = get_keyboard_modifiers(&event);

            // Check for text input (printable characters)
            let key = event.key();
            if key.len() == 1 && !event.ctrl_key() && !event.meta_key() {
                dispatch_event(PlatformEvent::TextInput { text: key });
            }

            dispatch_event(PlatformEvent::KeyPressed { keycode, modifiers });
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    {
        let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let keycode = event.key_code();
            let modifiers = get_keyboard_modifiers(&event);
            dispatch_event(PlatformEvent::KeyReleased { keycode, modifiers });
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    // Visibility change (for suspend/resume)
    {
        let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let window = web_sys::window().expect("no window");
            let document = window.document().expect("no document");
            if let Ok(hidden) = js_sys::Reflect::get(&document, &"hidden".into()) {
                if hidden.as_bool().unwrap_or(false) {
                    dispatch_event(PlatformEvent::Suspended);
                } else {
                    dispatch_event(PlatformEvent::Resumed);
                    extend_render_grace_period(500);
                }
            }
        }) as Box<dyn FnMut(_)>);
        window
            .document()
            .expect("no document")
            .add_event_listener_with_callback("visibilitychange", closure.as_ref().unchecked_ref()).map_err(js_err)?;
        closure.forget();
    }

    Ok(())
}

/// Set up a resize observer on the canvas
fn setup_resize_observer(canvas: &HtmlCanvasElement) -> Result<(), Box<dyn Error>> {
    let canvas = canvas.clone();

    // Use ResizeObserver if available, otherwise fallback to window resize
    let window = web_sys::window().expect("no window");

    let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let window = web_sys::window().expect("no window");
        let rect = canvas.get_bounding_client_rect();
        let width = rect.width();
        let height = rect.height();
        let scale_factor = window.device_pixel_ratio();

        // Update canvas resolution
        canvas.set_width((width * scale_factor) as u32);
        canvas.set_height((height * scale_factor) as u32);

        dispatch_event(PlatformEvent::Resized {
            width,
            height,
            scale_factor,
        });
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref()).map_err(js_err)?;
    closure.forget();

    Ok(())
}

/// Get keyboard modifier flags
fn get_keyboard_modifiers(event: &KeyboardEvent) -> u32 {
    let mut modifiers = 0u32;
    if event.shift_key() {
        modifiers |= 1;
    }
    if event.ctrl_key() {
        modifiers |= 2;
    }
    if event.alt_key() {
        modifiers |= 4;
    }
    if event.meta_key() {
        modifiers |= 8;
    }
    modifiers
}

/// Dispatch an event to the callback
fn dispatch_event(event: PlatformEvent) {
    CALLBACK.with(|c| {
        if let Some(ref mut callback) = *c.borrow_mut() {
            let response = callback(event);
            handle_event_response(response);
        }
    });
}

/// Handle the response from the event callback
fn handle_event_response(response: EventResponse) {
    if response.exit {
        REQUEST_EXIT.with(|r| *r.borrow_mut() = true);
        return;
    }

    if response.request_redraw {
        CONTINUOUS_RENDER.with(|c| *c.borrow_mut() = true);
        REQUEST_REDRAW.with(|r| *r.borrow_mut() = true);
    } else {
        CONTINUOUS_RENDER.with(|c| *c.borrow_mut() = false);

        // Schedule a redraw after the specified delay
        if response.redraw_after_ms > 0 {
            let window = web_sys::window().expect("no window");
            let now = window.performance().expect("no performance").now();
            let when = now + response.redraw_after_ms as f64;
            NEXT_REDRAW_AT.with(|r| *r.borrow_mut() = Some(when));
        }
    }
}

/// Start the requestAnimationFrame loop
fn start_animation_loop(window: &Window) -> Result<(), Box<dyn Error>> {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let window_clone = window.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        // Check for exit request
        if REQUEST_EXIT.with(|r| *r.borrow()) {
            return;
        }

        // Check if we should render this frame
        let should_render = should_render_frame(timestamp);
        let should_throttle = should_throttle_frame(timestamp);

        if should_render && !should_throttle {
            // Update frame timing
            LAST_FRAME_TIME.with(|l| *l.borrow_mut() = timestamp);
            FRAME_COUNT.with(|c| *c.borrow_mut() += 1);

            // Clear the redraw request
            REQUEST_REDRAW.with(|r| *r.borrow_mut() = false);
            NEXT_REDRAW_AT.with(|r| *r.borrow_mut() = None);

            // Dispatch redraw event
            dispatch_event(PlatformEvent::RedrawRequested);

            // Mark that we've rendered at least one frame
            HAS_RENDERED_FRAME.with(|h| *h.borrow_mut() = true);
        }

        // Request next frame
        if let Some(ref closure) = *f.borrow() {
            window_clone
                .request_animation_frame(closure.as_ref().unchecked_ref())
                .expect("failed to request animation frame");
        }
    }) as Box<dyn FnMut(f64)>));

    // Start the loop
    if let Some(ref closure) = *g.borrow() {
        window
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .expect("failed to request animation frame");
    }

    Ok(())
}

// WASM exports for JavaScript interop

/// Initialize the web platform (called from JavaScript)
#[wasm_bindgen]
pub fn centered_web_init(
    canvas_id: Option<String>,
    width: u32,
    height: u32,
    target_fps: u32,
) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let config = AppConfig {
        title: "Centered".to_string(),
        width,
        height,
        target_fps: if target_fps == 0 { 60 } else { target_fps },
        ..Default::default()
    };

    // Store canvas ID if provided
    if let Some(id) = canvas_id {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        if let Some(element) = document.get_element_by_id(&id) {
            if let Ok(canvas) = element.dyn_into::<HtmlCanvasElement>() {
                CANVAS.with(|c| *c.borrow_mut() = Some(canvas));
            }
        }
    }

    set_target_fps(config.target_fps);

    Ok(())
}

/// Request a redraw from JavaScript
#[wasm_bindgen]
pub fn centered_web_request_redraw() {
    use super::backend::PlatformBackend;
    <WebBackend as PlatformBackend>::request_redraw();
}

/// Get the current FPS setting
#[wasm_bindgen]
pub fn centered_web_get_target_fps() -> u32 {
    get_target_fps()
}

/// Set the target FPS
#[wasm_bindgen]
pub fn centered_web_set_target_fps(fps: u32) {
    set_target_fps(fps);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_calculation() {
        set_target_fps(60);
        assert!((get_frame_interval_ms() - 16.67).abs() < 0.1);

        set_target_fps(30);
        assert!((get_frame_interval_ms() - 33.33).abs() < 0.1);

        set_target_fps(120);
        assert!((get_frame_interval_ms() - 8.33).abs() < 0.1);
    }

    #[test]
    fn test_keyboard_modifiers() {
        // Note: Can't easily test this without a real KeyboardEvent
        // This is a placeholder for documentation
    }
}
