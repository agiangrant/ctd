//! Platform-specific window styling for frameless windows
//!
//! This module handles:
//! - Border radius (rounded corners) for frameless windows
//! - Native window control buttons (traffic lights on macOS)
//! - Title bar transparency and appearance

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Options for window styling
#[derive(Debug, Clone, Copy)]
pub struct WindowStyleOptions {
    /// Corner radius for the window (in points). 0 = no rounding.
    pub corner_radius: f32,
    /// Show native window controls (close/minimize/maximize buttons)
    pub show_native_controls: bool,
    /// Enable the minimize button (only used if show_native_controls is true)
    pub enable_minimize: bool,
    /// Enable the maximize/zoom button (only used if show_native_controls is true)
    pub enable_maximize: bool,
}

impl Default for WindowStyleOptions {
    fn default() -> Self {
        Self {
            corner_radius: 10.0, // macOS default
            show_native_controls: true,
            enable_minimize: true,
            enable_maximize: true,
        }
    }
}

/// Apply platform-specific window styling
///
/// This should be called after window creation to apply frameless window
/// customizations like border radius and window control visibility.
pub fn apply_window_style<W: HasWindowHandle>(
    window: &W,
    options: WindowStyleOptions,
) -> Result<(), String> {
    let handle = window
        .window_handle()
        .map_err(|e| format!("Failed to get window handle: {}", e))?;

    match handle.as_raw() {
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(appkit_handle) => {
            macos::apply_style(appkit_handle, options)
        }
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(win32_handle) => {
            windows::apply_style(win32_handle, options)
        }
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) | RawWindowHandle::Wayland(_) => {
            // Linux compositors handle rounding automatically
            Ok(())
        }
        _ => {
            // Unsupported platform - silently succeed
            Ok(())
        }
    }
}

// Platform-specific implementations
#[cfg(target_os = "macos")]
mod macos {
    use super::WindowStyleOptions;
    use raw_window_handle::AppKitWindowHandle;

    pub fn apply_style(
        handle: AppKitWindowHandle,
        options: WindowStyleOptions,
    ) -> Result<(), String> {
        use objc::{msg_send, sel, sel_impl, runtime::Object};

        unsafe {
            // Get NSView from the handle
            let ns_view = handle.ns_view.as_ptr() as *mut Object;
            if ns_view.is_null() {
                return Err("NSView handle is null".to_string());
            }

            // Get NSWindow from NSView
            let ns_window: *mut Object = msg_send![ns_view, window];
            if ns_window.is_null() {
                return Err("NSWindow is null".to_string());
            }

            // For frameless windows with native controls, we need special configuration
            if options.show_native_controls {
                // Prevent window from being deallocated when closed
                // This allows the event loop to handle CloseRequested properly
                let _: () = msg_send![ns_window, setReleasedWhenClosed: false];

                // Don't hide window when app loses focus
                let _: () = msg_send![ns_window, setHidesOnDeactivate: false];

                // Get the current style mask and ensure we have the right flags
                let current_mask: u64 = msg_send![ns_window, styleMask];

                // NSWindowStyleMask values:
                // Titled = 1, Closable = 2, Miniaturizable = 4, Resizable = 8
                // FullSizeContentView = 32768 (1 << 15)
                let titled: u64 = 1;
                let closable: u64 = 2;
                let full_size_content_view: u64 = 1 << 15;

                let new_mask = current_mask | titled | closable | full_size_content_view;
                let _: () = msg_send![ns_window, setStyleMask: new_mask];

                // Make title bar transparent so content extends underneath
                let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: true];

                // Hide the title text
                let _: () = msg_send![ns_window, setTitleVisibility: 1i64]; // NSWindowTitleHidden = 1
            }

            // Configure window control buttons (traffic lights)
            configure_window_buttons(ns_window, &options)?;

            // Apply corner radius via Core Animation layer
            if options.corner_radius > 0.0 {
                apply_corner_radius(ns_window, options.corner_radius)?;
            }

            // If hiding controls, set titlebar height to 0
            if !options.show_native_controls {
                let _: () = msg_send![ns_window, setTitlebarHeight: 0.0f64];
            }

            Ok(())
        }
    }

    unsafe fn configure_window_buttons(
        ns_window: *mut objc::runtime::Object,
        options: &WindowStyleOptions,
    ) -> Result<(), String> {
        use objc::{msg_send, sel, sel_impl};

        // NSWindowButton values
        const CLOSE_BUTTON: i64 = 0;
        const MINIATURIZE_BUTTON: i64 = 1;
        const ZOOM_BUTTON: i64 = 2;

        // Get references to the standard window buttons
        let close_button: *mut objc::runtime::Object =
            msg_send![ns_window, standardWindowButton: CLOSE_BUTTON];
        let miniaturize_button: *mut objc::runtime::Object =
            msg_send![ns_window, standardWindowButton: MINIATURIZE_BUTTON];
        let zoom_button: *mut objc::runtime::Object =
            msg_send![ns_window, standardWindowButton: ZOOM_BUTTON];

        let hide_controls = !options.show_native_controls;

        // Show/hide the buttons
        if !close_button.is_null() {
            let _: () = msg_send![close_button, setHidden: hide_controls];
        }
        if !miniaturize_button.is_null() {
            let _: () = msg_send![miniaturize_button, setHidden: hide_controls];
        }
        if !zoom_button.is_null() {
            let _: () = msg_send![zoom_button, setHidden: hide_controls];
        }

        // If showing controls, configure which ones are enabled
        if options.show_native_controls {
            // Close button is always enabled when showing controls
            if !close_button.is_null() {
                let _: () = msg_send![close_button, setEnabled: true];
            }
            // Miniaturize (minimize) button
            if !miniaturize_button.is_null() {
                let _: () = msg_send![miniaturize_button, setEnabled: options.enable_minimize];
            }
            // Zoom (maximize) button
            if !zoom_button.is_null() {
                let _: () = msg_send![zoom_button, setEnabled: options.enable_maximize];
            }
        }

        Ok(())
    }

    unsafe fn apply_corner_radius(
        ns_window: *mut objc::runtime::Object,
        radius: f32,
    ) -> Result<(), String> {
        use objc::{msg_send, sel, sel_impl};

        // Get the content view
        let content_view: *mut objc::runtime::Object = msg_send![ns_window, contentView];
        if content_view.is_null() {
            return Err("Content view is null".to_string());
        }

        // Enable layer-backed view for Core Animation
        let _: () = msg_send![content_view, setWantsLayer: true];

        // Get the layer
        let layer: *mut objc::runtime::Object = msg_send![content_view, layer];
        if layer.is_null() {
            return Err("Layer is null".to_string());
        }

        // Set corner radius and mask to bounds
        let _: () = msg_send![layer, setCornerRadius: radius as f64];
        let _: () = msg_send![layer, setMasksToBounds: true];

        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::WindowStyleOptions;
    use raw_window_handle::Win32WindowHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::UI::Controls::MARGINS;

    /// DWM window corner preference values
    const DWMWCP_DEFAULT: i32 = 0;
    const DWMWCP_DONOTROUND: i32 = 1;
    const DWMWCP_ROUND: i32 = 2;
    const DWMWCP_ROUNDSMALL: i32 = 3;

    /// DWM system backdrop type values (Windows 11)
    const DWMSBT_AUTO: i32 = 0;
    const DWMSBT_NONE: i32 = 1;
    const DWMSBT_MAINWINDOW: i32 = 2;  // Mica
    const DWMSBT_TRANSIENTWINDOW: i32 = 3;  // Acrylic
    const DWMSBT_TABBEDWINDOW: i32 = 4;  // Tabbed Mica

    pub fn apply_style(
        handle: Win32WindowHandle,
        options: WindowStyleOptions,
    ) -> Result<(), String> {
        unsafe {
            let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);

            // Apply rounded corners (Windows 11+)
            if options.corner_radius > 0.0 {
                // Choose corner style based on radius
                let corner_preference = if options.corner_radius >= 8.0 {
                    DWMWCP_ROUND // Standard rounded corners
                } else {
                    DWMWCP_ROUNDSMALL // Small rounded corners
                };

                let result = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &corner_preference as *const i32 as *const std::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                );

                if result.is_err() {
                    // Windows 10 doesn't support this - that's OK, silently continue
                    eprintln!("Note: DWM rounded corners not supported (Windows 11+ required)");
                }
            }

            // Apply Mica material backdrop (Windows 11+)
            // This creates the frosted glass effect behind the window
            let backdrop_type = DWMSBT_MAINWINDOW; // Mica
            let result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop_type as *const i32 as *const std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );

            if result.is_err() {
                // Try the older USE_IMMERSIVE_DARK_MODE for dark title bar on Windows 10
                let use_dark_mode: i32 = 1;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                    &use_dark_mode as *const i32 as *const std::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                );
            }

            // For frameless windows, extend the frame into client area
            if !options.show_native_controls {
                let margins = MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                };

                let result = DwmExtendFrameIntoClientArea(hwnd, &margins);
                if result.is_err() {
                    return Err("Failed to extend frame into client area".to_string());
                }
            }

            Ok(())
        }
    }
}
