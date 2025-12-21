//! Linux platform integration
//!
//! Provides Linux-specific functionality for desktop integration:
//! - Clipboard (X11/Wayland via arboard)
//! - File dialogs (native via rfd)
//! - System tray (via tray-icon)
//! - Notifications (via notify-rust/D-Bus)
//! - XDG portal integration (settings, dark mode)

mod clipboard;
mod dialogs;
mod notifications;
mod portal;
mod tray;

pub use clipboard::LinuxClipboard;
pub use dialogs::{open_file_dialog, save_file_dialog, open_folder_dialog, show_message_dialog, MessageLevel};
pub use notifications::show_notification;
pub use portal::{is_dark_mode, get_accent_color, is_natural_scrolling};
pub use tray::LinuxTrayIcon;
