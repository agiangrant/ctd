//! Linux clipboard support using arboard
//!
//! Provides clipboard read/write functionality that works on both X11 and Wayland.

use arboard::Clipboard;

/// Linux clipboard wrapper
pub struct LinuxClipboard {
    clipboard: Clipboard,
}

impl LinuxClipboard {
    /// Create a new clipboard instance
    pub fn new() -> Result<Self, arboard::Error> {
        Ok(Self {
            clipboard: Clipboard::new()?,
        })
    }

    /// Get text from the clipboard
    pub fn get_text(&mut self) -> Option<String> {
        self.clipboard.get_text().ok()
    }

    /// Set text to the clipboard
    pub fn set_text(&mut self, text: &str) -> bool {
        self.clipboard.set_text(text).is_ok()
    }

    /// Get image from the clipboard (returns RGBA data, width, height)
    pub fn get_image(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        self.clipboard.get_image().ok().map(|img| {
            (img.bytes.into_owned(), img.width as u32, img.height as u32)
        })
    }

    /// Set image to the clipboard (RGBA data, width, height)
    pub fn set_image(&mut self, data: &[u8], width: u32, height: u32) -> bool {
        use arboard::ImageData;

        let img = ImageData {
            width: width as usize,
            height: height as usize,
            bytes: data.into(),
        };
        self.clipboard.set_image(img).is_ok()
    }

    /// Clear the clipboard
    pub fn clear(&mut self) -> bool {
        self.clipboard.clear().is_ok()
    }
}

impl Default for LinuxClipboard {
    fn default() -> Self {
        Self::new().expect("Failed to initialize clipboard")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_creation() {
        // This may fail in CI without display
        if let Ok(clipboard) = LinuxClipboard::new() {
            let _ = clipboard;
        }
    }

    #[test]
    fn test_clipboard_text() {
        if let Ok(mut clipboard) = LinuxClipboard::new() {
            // Set and get text
            let test_text = "Hello from Centered!";
            if clipboard.set_text(test_text) {
                if let Some(text) = clipboard.get_text() {
                    assert_eq!(text, test_text);
                }
            }
        }
    }
}
