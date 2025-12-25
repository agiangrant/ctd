//! Windows system tray support using Shell_NotifyIconW
//!
//! Provides system tray icon functionality for Windows.

use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::Mutex;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Custom message for tray icon callbacks
const WM_TRAY_CALLBACK: u32 = WM_USER + 1;

/// Global tray icon state
static TRAY_HWND: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TRAY_ICON_ID: AtomicU32 = AtomicU32::new(1);
static MENU_CALLBACK: Mutex<Option<Box<dyn Fn(u32) + Send + Sync>>> = Mutex::new(None);

/// Error type for tray operations
#[derive(Debug)]
pub enum TrayError {
    WindowCreation(String),
    IconCreation(String),
    ShellNotify(String),
    MenuCreation(String),
}

impl std::fmt::Display for TrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrayError::WindowCreation(e) => write!(f, "Window creation failed: {}", e),
            TrayError::IconCreation(e) => write!(f, "Icon creation failed: {}", e),
            TrayError::ShellNotify(e) => write!(f, "Shell notify failed: {}", e),
            TrayError::MenuCreation(e) => write!(f, "Menu creation failed: {}", e),
        }
    }
}

impl std::error::Error for TrayError {}

/// Menu item for the tray context menu
pub struct TrayMenuItem {
    pub id: u32,
    pub label: String,
    pub enabled: bool,
    pub checked: bool,
    pub is_separator: bool,
}

impl TrayMenuItem {
    pub fn new(id: u32, label: &str) -> Self {
        Self {
            id,
            label: label.to_string(),
            enabled: true,
            checked: false,
            is_separator: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: 0,
            label: String::new(),
            enabled: true,
            checked: false,
            is_separator: true,
        }
    }
}

/// Windows system tray icon wrapper
pub struct WindowsTrayIcon {
    hwnd: HWND,
    icon_id: u32,
    menu: Option<HMENU>,
    menu_items: Vec<TrayMenuItem>,
}

impl WindowsTrayIcon {
    /// Create a new tray icon from RGBA data
    pub fn new(icon_data: &[u8], width: u32, height: u32, tooltip: &str) -> Result<Self, TrayError> {
        unsafe {
            // Create hidden message window
            let hwnd = create_message_window()?;
            TRAY_HWND.store(hwnd.0 as *mut c_void, Ordering::SeqCst);

            let icon_id = TRAY_ICON_ID.fetch_add(1, Ordering::SeqCst);

            // Create icon from RGBA data
            let hicon = create_icon_from_rgba(icon_data, width, height)?;

            // Create tooltip string
            let mut tooltip_wide: [u16; 128] = [0; 128];
            for (i, ch) in tooltip.encode_utf16().take(127).enumerate() {
                tooltip_wide[i] = ch;
            }

            // Initialize NOTIFYICONDATAW
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = icon_id;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY_CALLBACK;
            nid.hIcon = hicon;
            nid.szTip = tooltip_wide;

            // Add tray icon
            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                return Err(TrayError::ShellNotify("Failed to add tray icon".to_string()));
            }

            // Set version for modern behavior
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);

            Ok(Self {
                hwnd,
                icon_id,
                menu: None,
                menu_items: Vec::new(),
            })
        }
    }

    /// Create a new tray icon from an image file
    pub fn from_file(path: &str, tooltip: &str) -> Result<Self, TrayError> {
        // Load image using the image crate
        let img = image::open(path)
            .map_err(|e| TrayError::IconCreation(format!("Failed to load image: {}", e)))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        Self::new(rgba.as_raw(), width, height, tooltip)
    }

    /// Set the tooltip text
    pub fn set_tooltip(&self, tooltip: &str) {
        unsafe {
            let mut tooltip_wide: [u16; 128] = [0; 128];
            for (i, ch) in tooltip.encode_utf16().take(127).enumerate() {
                tooltip_wide[i] = ch;
            }

            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = self.icon_id;
            nid.uFlags = NIF_TIP;
            nid.szTip = tooltip_wide;

            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Update the tray icon
    pub fn set_icon(&self, icon_data: &[u8], width: u32, height: u32) -> Result<(), TrayError> {
        unsafe {
            let hicon = create_icon_from_rgba(icon_data, width, height)?;

            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = self.icon_id;
            nid.uFlags = NIF_ICON;
            nid.hIcon = hicon;

            if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                return Err(TrayError::ShellNotify("Failed to modify tray icon".to_string()));
            }

            Ok(())
        }
    }

    /// Add a menu item
    pub fn add_menu_item(&mut self, item: TrayMenuItem) {
        self.menu_items.push(item);
        self.rebuild_menu();
    }

    /// Clear all menu items
    pub fn clear_menu(&mut self) {
        self.menu_items.clear();
        if let Some(menu) = self.menu.take() {
            unsafe {
                let _ = DestroyMenu(menu);
            }
        }
    }

    /// Set the menu callback
    pub fn set_callback<F: Fn(u32) + Send + Sync + 'static>(&self, callback: F) {
        let mut guard = MENU_CALLBACK.lock().unwrap();
        *guard = Some(Box::new(callback));
    }

    /// Show the tray icon
    pub fn show(&self) {
        unsafe {
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = self.icon_id;
            nid.uFlags = NIF_STATE;
            nid.dwState = NOTIFY_ICON_STATE(0);
            nid.dwStateMask = NIS_HIDDEN;

            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Hide the tray icon
    pub fn hide(&self) {
        unsafe {
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = self.icon_id;
            nid.uFlags = NIF_STATE;
            nid.dwState = NIS_HIDDEN;
            nid.dwStateMask = NIS_HIDDEN;

            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    fn rebuild_menu(&mut self) {
        unsafe {
            // Destroy old menu if exists
            if let Some(menu) = self.menu.take() {
                let _ = DestroyMenu(menu);
            }

            if self.menu_items.is_empty() {
                return;
            }

            // Create new popup menu
            let menu = CreatePopupMenu();
            if menu.is_err() {
                return;
            }
            let menu = menu.unwrap();

            for item in &self.menu_items {
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
                        item.id as usize,
                        PCWSTR::from_raw(label_wide.as_ptr()),
                    );
                }
            }

            self.menu = Some(menu);
        }
    }

    fn show_context_menu(&self) {
        if let Some(menu) = self.menu {
            unsafe {
                // Get cursor position
                let mut point = POINT::default();
                let _ = GetCursorPos(&mut point);

                // Set foreground window (required for menu to work properly)
                let _ = SetForegroundWindow(self.hwnd);

                // Show context menu
                let cmd = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY,
                    point.x,
                    point.y,
                    0,
                    self.hwnd,
                    None,
                );

                // Send dummy message to close menu properly
                let _ = PostMessageW(self.hwnd, WM_NULL, WPARAM(0), LPARAM(0));

                // Call callback with selected item
                if cmd.0 > 0 {
                    let guard = MENU_CALLBACK.lock().unwrap();
                    if let Some(callback) = guard.as_ref() {
                        callback(cmd.0 as u32);
                    }
                }
            }
        }
    }
}

impl Drop for WindowsTrayIcon {
    fn drop(&mut self) {
        unsafe {
            // Remove tray icon
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = self.icon_id;

            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);

            // Destroy menu
            if let Some(menu) = self.menu.take() {
                let _ = DestroyMenu(menu);
            }

            // Destroy message window
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Create a hidden message window for tray icon callbacks
unsafe fn create_message_window() -> Result<HWND, TrayError> {
    let class_name = w!("CenteredTrayMessageWindow");

    // Register window class
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(tray_window_proc),
        hInstance: HINSTANCE::default(),
        lpszClassName: class_name,
        ..Default::default()
    };

    let atom = RegisterClassExW(&wc);
    if atom == 0 {
        // Class might already be registered, try to use it anyway
    }

    // Create message-only window
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class_name,
        w!("Centered Tray"),
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
        Ok(_) => Err(TrayError::WindowCreation("Failed to create message window".to_string())),
        Err(e) => Err(TrayError::WindowCreation(format!("Failed to create message window: {:?}", e))),
    }
}

/// Create an HICON from RGBA data
unsafe fn create_icon_from_rgba(rgba: &[u8], width: u32, height: u32) -> Result<HICON, TrayError> {
    if rgba.len() != (width * height * 4) as usize {
        return Err(TrayError::IconCreation("Invalid RGBA data size".to_string()));
    }

    // Create DIB section for color bitmap
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // Top-down DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0, // BI_RGB = 0
            ..Default::default()
        },
        ..Default::default()
    };

    let hdc = GetDC(None);
    let mut bits_ptr: *mut c_void = std::ptr::null_mut();

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
            return Err(TrayError::IconCreation("Failed to create color bitmap".to_string()));
        }
    };

    // Copy RGBA data (convert RGBA to BGRA)
    let bits = std::slice::from_raw_parts_mut(bits_ptr as *mut u8, rgba.len());
    for i in (0..rgba.len()).step_by(4) {
        bits[i] = rgba[i + 2];     // B
        bits[i + 1] = rgba[i + 1]; // G
        bits[i + 2] = rgba[i];     // R
        bits[i + 3] = rgba[i + 3]; // A
    }

    // Create mask bitmap (all zeros for full opacity)
    let mask_bitmap = CreateBitmap(width as i32, height as i32, 1, 1, None);
    if mask_bitmap.is_invalid() {
        let _ = DeleteObject(color_bitmap);
        ReleaseDC(None, hdc);
        return Err(TrayError::IconCreation("Failed to create mask bitmap".to_string()));
    }

    // Create icon
    let icon_info = ICONINFO {
        fIcon: BOOL(1), // TRUE
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask_bitmap,
        hbmColor: color_bitmap,
    };

    let hicon = CreateIconIndirect(&icon_info);

    // Cleanup
    let _ = DeleteObject(color_bitmap);
    let _ = DeleteObject(mask_bitmap);
    ReleaseDC(None, hdc);

    hicon.map_err(|_| TrayError::IconCreation("Failed to create icon".to_string()))
}

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

            match event {
                // Right-click or context menu key
                x if x == WM_RBUTTONUP || x == WM_CONTEXTMENU => {
                    // Find the tray icon and show its menu
                    // This is a simplified version - in production you'd track icons properly
                }
                _ => {}
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

#[cfg(test)]
mod tests {
    // Tray icon tests require a display and are interactive,
    // so they're not run in CI. Manual testing is required.
}
