//! Windows platform integration
//!
//! Provides Windows-specific functionality for desktop integration:
//! - System tray icons (Shell_NotifyIconW)
//! - Notifications (Toast notifications)
//! - Scroll direction detection
//! - Frameless window controls

pub mod tray;
pub mod window_controls;

pub use tray::WindowsTrayIcon;
pub use window_controls::{WindowControls, ButtonKind, ResizeEdge, detect_resize_edge, HEADER_HEIGHT, window_border_command, WINDOW_CORNER_RADIUS};

use std::sync::OnceLock;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD,
};
use windows::core::PCWSTR;

/// Cached scroll direction settings (mouse, touchpad)
static MOUSE_NATURAL_SCROLL: OnceLock<bool> = OnceLock::new();
static TOUCHPAD_NATURAL_SCROLL: OnceLock<bool> = OnceLock::new();

/// Check if mouse has natural scrolling enabled (FlipFlopWheel)
///
/// This is for discrete scroll wheels (LineDelta events).
/// Windows mice can have FlipFlopWheel set per-device in the registry.
pub fn is_mouse_natural_scrolling() -> bool {
    *MOUSE_NATURAL_SCROLL.get_or_init(|| {
        // Check HID device FlipFlopWheel settings
        if let Some(flip) = read_mouse_flip_flop_wheel() {
            return flip;
        }
        // Default: Windows mice use traditional scrolling
        false
    })
}

/// Check if touchpad has natural scrolling enabled
///
/// This is for smooth/pixel-based scrolling (PixelDelta events).
/// Precision Touchpads store this in PrecisionTouchPad registry key.
pub fn is_touchpad_natural_scrolling() -> bool {
    *TOUCHPAD_NATURAL_SCROLL.get_or_init(|| {
        if let Some(natural) = read_touchpad_scroll_direction() {
            return natural;
        }
        // Default: Windows touchpads use traditional scrolling
        false
    })
}

/// Read FlipFlopWheel setting for mouse devices from HID registry
fn read_mouse_flip_flop_wheel() -> Option<bool> {
    unsafe {
        // Mouse FlipFlopWheel is stored per-device under:
        // HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Enum\HID\<VID_xxxx&PID_xxxx>\<instance>\Device Parameters
        let key_path: Vec<u16> = "SYSTEM\\CurrentControlSet\\Enum\\HID"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut hid_key: HKEY = std::mem::zeroed();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR::from_raw(key_path.as_ptr()),
            0,
            KEY_READ,
            &mut hid_key,
        ).is_err() {
            return None;
        }

        // Enumerate HID device classes
        let mut index = 0u32;
        let mut name_buf = [0u16; 256];

        loop {
            let mut name_len = name_buf.len() as u32;
            let result = RegEnumKeyExW(
                hid_key,
                index,
                windows::core::PWSTR::from_raw(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                windows::core::PWSTR::null(),
                None,
                None,
            );

            if result.is_err() {
                break;
            }

            // Open this device class
            let mut device_class_key: HKEY = std::mem::zeroed();
            if RegOpenKeyExW(
                hid_key,
                PCWSTR::from_raw(name_buf.as_ptr()),
                0,
                KEY_READ,
                &mut device_class_key,
            ).is_ok() {
                // Enumerate device instances
                let mut inst_index = 0u32;
                loop {
                    let mut inst_name_len = name_buf.len() as u32;
                    let inst_result = RegEnumKeyExW(
                        device_class_key,
                        inst_index,
                        windows::core::PWSTR::from_raw(name_buf.as_mut_ptr()),
                        &mut inst_name_len,
                        None,
                        windows::core::PWSTR::null(),
                        None,
                        None,
                    );

                    if inst_result.is_err() {
                        break;
                    }

                    // Try to open Device Parameters
                    let mut instance_key: HKEY = std::mem::zeroed();
                    if RegOpenKeyExW(
                        device_class_key,
                        PCWSTR::from_raw(name_buf.as_ptr()),
                        0,
                        KEY_READ,
                        &mut instance_key,
                    ).is_ok() {
                        let params_path: Vec<u16> = "Device Parameters"
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();

                        let mut params_key: HKEY = std::mem::zeroed();
                        if RegOpenKeyExW(
                            instance_key,
                            PCWSTR::from_raw(params_path.as_ptr()),
                            0,
                            KEY_READ,
                            &mut params_key,
                        ).is_ok() {
                            // Check for FlipFlopWheel value
                            let value_name: Vec<u16> = "FlipFlopWheel"
                                .encode_utf16()
                                .chain(std::iter::once(0))
                                .collect();

                            let mut data: u32 = 0;
                            let mut data_size = std::mem::size_of::<u32>() as u32;
                            let mut value_type = REG_DWORD;

                            if RegQueryValueExW(
                                params_key,
                                PCWSTR::from_raw(value_name.as_ptr()),
                                None,
                                Some(&mut value_type),
                                Some(&mut data as *mut u32 as *mut u8),
                                Some(&mut data_size),
                            ).is_ok() {
                                let _ = RegCloseKey(params_key);
                                let _ = RegCloseKey(instance_key);
                                let _ = RegCloseKey(device_class_key);
                                let _ = RegCloseKey(hid_key);
                                // FlipFlopWheel: 1 = natural scrolling
                                return Some(data == 1);
                            }
                            let _ = RegCloseKey(params_key);
                        }
                        let _ = RegCloseKey(instance_key);
                    }
                    inst_index += 1;
                }
                let _ = RegCloseKey(device_class_key);
            }
            index += 1;
        }

        let _ = RegCloseKey(hid_key);
        None
    }
}

/// Read ScrollDirection setting for Precision Touchpad
fn read_touchpad_scroll_direction() -> Option<bool> {
    unsafe {
        // Precision Touchpad settings
        let key_path: Vec<u16> = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey: HKEY = std::mem::zeroed();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(key_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );

        if result.is_err() {
            return None;
        }

        // Check ScrollDirection (0 = down motion scrolls up = natural, 1 = traditional)
        // Note: Windows inverts the meaning - 0 is actually natural scrolling for touchpads
        let value_name: Vec<u16> = "ScrollDirection"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut data: u32 = 0;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        let mut value_type = REG_DWORD;

        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_name.as_ptr()),
            None,
            Some(&mut value_type),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        if result.is_ok() {
            // ScrollDirection: 0 = natural (down motion scrolls down), 1 = traditional
            return Some(data == 0);
        }

        None
    }
}
