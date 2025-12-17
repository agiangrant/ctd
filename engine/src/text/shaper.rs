//! Text shaping and layout
//!
//! This module handles text shaping - the process of converting text strings
//! into positioned glyphs ready for rendering. It handles:
//! - Line breaking and word wrapping
//! - Text alignment (left, center, right, justify)
//! - Line spacing and letter spacing
//! - Bidirectional text (TODO)
//! - Complex scripts (handled by platform - Core Text, HarfBuzz, DirectWrite)

use super::{FontDescriptor, TextLayoutConfig, TextAlign, WordBreak, TextOverflow};
use crate::text::font_manager::{Font, FontError, FontManager};

// Core Text is available on both macOS and iOS
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use macos::MacOSTextShaper;

/// A positioned glyph ready for rendering
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// Glyph ID (for atlas lookup)
    pub glyph_id: u32,

    /// Character this glyph represents (for debugging/fallback)
    pub character: char,

    /// X position relative to text origin
    pub x: f32,

    /// Y position relative to text origin (baseline)
    pub y: f32,

    /// Glyph advance width
    pub advance: f32,

    /// Visual width of glyph
    pub width: f32,

    /// Visual height of glyph
    pub height: f32,
}

/// A shaped line of text with positioned glyphs
#[derive(Debug, Clone)]
pub struct ShapedLine {
    /// Glyphs in this line
    pub glyphs: Vec<ShapedGlyph>,

    /// Total width of the line
    pub width: f32,

    /// Height of the line (ascent + descent)
    pub height: f32,

    /// Ascent (distance from baseline to top)
    pub ascent: f32,

    /// Descent (distance from baseline to bottom)
    pub descent: f32,

    /// Y position of this line's baseline
    pub baseline_y: f32,
}

/// Complete shaped text ready for rendering
#[derive(Debug, Clone)]
pub struct ShapedText {
    /// Shaped lines
    pub lines: Vec<ShapedLine>,

    /// Total bounding box width
    pub width: f32,

    /// Total bounding box height
    pub height: f32,
}

impl ShapedText {
    /// Create an empty shaped text
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            width: 0.0,
            height: 0.0,
        }
    }
}

/// Text shaping errors
#[derive(Debug, Clone)]
pub enum ShaperError {
    /// Font error
    FontError(String),

    /// Layout error
    LayoutError(String),
}

impl From<FontError> for ShaperError {
    fn from(err: FontError) -> Self {
        ShaperError::FontError(err.to_string())
    }
}

impl std::fmt::Display for ShaperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaperError::FontError(msg) => write!(f, "Font error: {}", msg),
            ShaperError::LayoutError(msg) => write!(f, "Layout error: {}", msg),
        }
    }
}

impl std::error::Error for ShaperError {}

/// Text shaper trait - platform-specific implementation
pub trait TextShaper {
    /// Shape text into positioned glyphs
    fn shape_text(
        &self,
        text: &str,
        font: &dyn Font,
        config: &TextLayoutConfig,
    ) -> Result<ShapedText, ShaperError>;
}

/// Platform-specific text shaper
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub type PlatformTextShaper = MacOSTextShaper;

// Stub shaper for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub use stub::StubTextShaper as PlatformTextShaper;

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod stub {
    use super::*;
    use crate::text::font_manager::Font;

    /// Stub text shaper for platforms without text rendering support.
    /// Returns empty shaped text for all operations.
    pub struct StubTextShaper;

    impl StubTextShaper {
        pub fn new() -> Self {
            Self
        }
    }

    impl TextShaper for StubTextShaper {
        fn shape_text(
            &self,
            _text: &str,
            _font: &dyn Font,
            _config: &TextLayoutConfig,
        ) -> Result<ShapedText, ShaperError> {
            // Return empty text - rendering is not supported on this platform
            Ok(ShapedText::empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_shaped_text() {
        let shaped = ShapedText::empty();
        assert_eq!(shaped.lines.len(), 0);
        assert_eq!(shaped.width, 0.0);
        assert_eq!(shaped.height, 0.0);
    }

    #[test]
    fn test_shaped_glyph_creation() {
        let glyph = ShapedGlyph {
            glyph_id: 42,
            character: 'A',
            x: 10.0,
            y: 20.0,
            advance: 12.0,
            width: 11.0,
            height: 16.0,
        };

        assert_eq!(glyph.glyph_id, 42);
        assert_eq!(glyph.character, 'A');
        assert_eq!(glyph.x, 10.0);
    }
}
