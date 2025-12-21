//! Linux system tray support using tray-icon
//!
//! Provides system tray icon functionality for Linux desktops.

use tray_icon::{TrayIcon, TrayIconBuilder, Icon};
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

/// Linux system tray icon wrapper
pub struct LinuxTrayIcon {
    tray: TrayIcon,
}

/// Error type for tray operations
#[derive(Debug)]
pub enum TrayError {
    BadIcon(tray_icon::BadIcon),
    TrayIcon(tray_icon::Error),
    Image(image::ImageError),
}

impl std::fmt::Display for TrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrayError::BadIcon(e) => write!(f, "Bad icon: {:?}", e),
            TrayError::TrayIcon(e) => write!(f, "Tray error: {}", e),
            TrayError::Image(e) => write!(f, "Image error: {}", e),
        }
    }
}

impl std::error::Error for TrayError {}

impl From<tray_icon::BadIcon> for TrayError {
    fn from(e: tray_icon::BadIcon) -> Self {
        TrayError::BadIcon(e)
    }
}

impl From<tray_icon::Error> for TrayError {
    fn from(e: tray_icon::Error) -> Self {
        TrayError::TrayIcon(e)
    }
}

impl From<image::ImageError> for TrayError {
    fn from(e: image::ImageError) -> Self {
        TrayError::Image(e)
    }
}

impl LinuxTrayIcon {
    /// Create a new tray icon from RGBA data
    ///
    /// # Arguments
    /// * `icon_data` - RGBA pixel data
    /// * `width` - Icon width in pixels
    /// * `height` - Icon height in pixels
    /// * `tooltip` - Tooltip text shown on hover
    pub fn new(icon_data: &[u8], width: u32, height: u32, tooltip: &str) -> Result<Self, TrayError> {
        let icon = Icon::from_rgba(icon_data.to_vec(), width, height)?;

        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(tooltip)
            .build()?;

        Ok(Self { tray })
    }

    /// Create a new tray icon from an image file
    pub fn from_file(path: &str, tooltip: &str) -> Result<Self, TrayError> {
        let img = image::open(path)?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let icon = Icon::from_rgba(rgba.into_raw(), width, height)?;

        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(tooltip)
            .build()?;

        Ok(Self { tray })
    }

    /// Set the tray icon menu
    pub fn set_menu(&self, menu: Menu) {
        self.tray.set_menu(Some(Box::new(menu)));
    }

    /// Set the tooltip text
    pub fn set_tooltip(&self, tooltip: &str) {
        let _ = self.tray.set_tooltip(Some(tooltip));
    }

    /// Update the tray icon
    pub fn set_icon(&self, icon_data: &[u8], width: u32, height: u32) -> Result<(), TrayError> {
        let icon = Icon::from_rgba(icon_data.to_vec(), width, height)?;
        self.tray.set_icon(Some(icon)).map_err(TrayError::TrayIcon)
    }

    /// Show the tray icon (it's visible by default)
    pub fn show(&self) {
        let _ = self.tray.set_visible(true);
    }

    /// Hide the tray icon
    pub fn hide(&self) {
        let _ = self.tray.set_visible(false);
    }

    /// Get the tray icon ID for event handling
    pub fn id(&self) -> tray_icon::TrayIconId {
        self.tray.id().clone()
    }
}

/// Helper to create a simple menu with items
pub fn create_menu(items: &[(&str, u32)]) -> Menu {
    let menu = Menu::new();

    for (label, id) in items {
        let item = MenuItem::with_id(*id, *label, true, None);
        let _ = menu.append(&item);
    }

    menu
}

/// Helper to create a menu with a quit option
pub fn create_menu_with_quit(items: &[(&str, u32)], quit_id: u32) -> Menu {
    let menu = Menu::new();

    for (label, id) in items {
        let item = MenuItem::with_id(*id, *label, true, None);
        let _ = menu.append(&item);
    }

    // Add separator and quit
    let _ = menu.append(&PredefinedMenuItem::separator());
    let quit = MenuItem::with_id(quit_id, "Quit", true, None);
    let _ = menu.append(&quit);

    menu
}

#[cfg(test)]
mod tests {
    // Tray icon tests require a display and are interactive,
    // so they're not run in CI. Manual testing is required.
}
