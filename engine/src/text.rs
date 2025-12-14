//! Text rendering types and utilities
//!
//! This module defines the core types for text rendering, including font descriptors,
//! layout configuration, and text styling. These types receive exact values from the
//! Go layer (already resolved from Tailwind classes).

pub mod atlas;
pub mod font_manager;
pub mod shaper;

use serde::{Deserialize, Serialize};

// Re-export atlas types
pub use atlas::{AtlasEntry, AtlasMetrics, GlyphAtlas, GlyphBitmap, GlyphKey, GlyphRasterizer, PlatformGlyphRasterizer};

// Re-export font manager types
pub use font_manager::{Font, FontError, FontManager, GlyphMetrics};

// Re-export shaper types
pub use shaper::{ShapedGlyph, ShapedLine, ShapedText, ShaperError, TextShaper, PlatformTextShaper};

/// Complete font specification with exact values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontDescriptor {
    /// Font source (system font name or bundled font path)
    pub source: FontSource,

    /// Font weight (100-900)
    pub weight: u16,

    /// Font style (normal or italic)
    pub style: FontStyle,

    /// Font size in points
    pub size: f32,
}

/// Font source - either system font or bundled font file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontSource {
    /// System font by name (e.g., "San Francisco", "Roboto", "Segoe UI")
    System(String),

    /// Bundled font from file path (e.g., "fonts/Inter-Regular.ttf")
    Bundled(String),

    /// Font loaded from memory (embedded in binary)
    /// Contains font name for caching and raw font data
    Memory {
        name: String,
        data_hash: u64,  // Hash of font data for cache key
    },
}

impl FontDescriptor {
    /// Create a new font descriptor with system font
    pub fn system(name: &str, weight: u16, style: FontStyle, size: f32) -> Self {
        Self {
            source: FontSource::System(name.to_string()),
            weight,
            style,
            size,
        }
    }

    /// Create a new font descriptor with bundled font
    pub fn bundled(path: &str, weight: u16, style: FontStyle, size: f32) -> Self {
        Self {
            source: FontSource::Bundled(path.to_string()),
            weight,
            style,
            size,
        }
    }

    /// Create a cache key for this font (for font manager cache)
    pub fn cache_key(&self) -> String {
        format!(
            "{:?}:{}:{}:{}",
            self.source,
            self.weight,
            self.style as u8,
            self.size
        )
    }
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self {
            source: FontSource::System("system".to_string()),
            weight: 400,
            style: FontStyle::Normal,
            size: 16.0,
        }
    }
}

/// Font style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum FontStyle {
    Normal = 0,
    Italic = 1,
}

impl From<u8> for FontStyle {
    fn from(value: u8) -> Self {
        match value {
            1 => FontStyle::Italic,
            _ => FontStyle::Normal,
        }
    }
}

/// Text layout configuration with exact values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutConfig {
    /// Maximum width in pixels (None = no constraint)
    pub max_width: Option<f32>,

    /// Maximum height in pixels (None = no constraint)
    pub max_height: Option<f32>,

    /// Maximum number of lines (None = no constraint)
    pub max_lines: Option<usize>,

    /// Line height multiplier (1.5 = 150% of font size)
    pub line_height: f32,

    /// Letter spacing in em units (0.05 = 5% of font size)
    pub letter_spacing: f32,

    /// Word spacing in em units
    pub word_spacing: f32,

    /// Horizontal text alignment
    pub alignment: TextAlign,

    /// Vertical text alignment
    pub vertical_align: VerticalAlign,

    /// Word breaking behavior
    pub word_break: WordBreak,

    /// Text overflow behavior
    pub overflow: TextOverflow,

    /// Whitespace handling
    pub white_space: WhiteSpace,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            max_lines: None,
            line_height: 1.5,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            alignment: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            word_break: WordBreak::Normal,
            overflow: TextOverflow::Wrap,
            white_space: WhiteSpace::Normal,
        }
    }
}

/// Horizontal text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TextAlign {
    Left = 0,
    Center = 1,
    Right = 2,
    Justify = 3,
}

impl From<u8> for TextAlign {
    fn from(value: u8) -> Self {
        match value {
            1 => TextAlign::Center,
            2 => TextAlign::Right,
            3 => TextAlign::Justify,
            _ => TextAlign::Left,
        }
    }
}

/// Vertical text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum VerticalAlign {
    Top = 0,
    Middle = 1,
    Bottom = 2,
    Baseline = 3,
}

impl From<u8> for VerticalAlign {
    fn from(value: u8) -> Self {
        match value {
            1 => VerticalAlign::Middle,
            2 => VerticalAlign::Bottom,
            3 => VerticalAlign::Baseline,
            _ => VerticalAlign::Top,
        }
    }
}

/// Word breaking behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WordBreak {
    Normal = 0,      // Break at word boundaries
    BreakAll = 1,    // Break anywhere
    KeepAll = 2,     // No breaks (CJK)
    BreakWord = 3,   // Break long words if needed
}

impl From<u8> for WordBreak {
    fn from(value: u8) -> Self {
        match value {
            1 => WordBreak::BreakAll,
            2 => WordBreak::KeepAll,
            3 => WordBreak::BreakWord,
            _ => WordBreak::Normal,
        }
    }
}

/// Text overflow behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TextOverflow {
    Clip = 0,        // Cut off
    Ellipsis = 1,    // Add "..."
    Wrap = 2,        // Wrap to next line
}

impl From<u8> for TextOverflow {
    fn from(value: u8) -> Self {
        match value {
            1 => TextOverflow::Ellipsis,
            2 => TextOverflow::Wrap,
            _ => TextOverflow::Clip,
        }
    }
}

/// Whitespace handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WhiteSpace {
    Normal = 0,      // Collapse whitespace, wrap
    NoWrap = 1,      // Collapse whitespace, no wrap
    Pre = 2,         // Preserve whitespace, no wrap
    PreWrap = 3,     // Preserve whitespace, wrap
}

impl From<u8> for WhiteSpace {
    fn from(value: u8) -> Self {
        match value {
            1 => WhiteSpace::NoWrap,
            2 => WhiteSpace::Pre,
            3 => WhiteSpace::PreWrap,
            _ => WhiteSpace::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_descriptor_system() {
        let font = FontDescriptor::system("San Francisco", 700, FontStyle::Normal, 18.0);
        assert_eq!(font.weight, 700);
        assert_eq!(font.size, 18.0);
        assert!(matches!(font.source, FontSource::System(_)));
    }

    #[test]
    fn test_font_descriptor_bundled() {
        let font = FontDescriptor::bundled("fonts/Inter-Bold.ttf", 700, FontStyle::Normal, 18.0);
        assert!(matches!(font.source, FontSource::Bundled(_)));
    }

    #[test]
    fn test_font_cache_key() {
        let font1 = FontDescriptor::system("Arial", 400, FontStyle::Normal, 16.0);
        let font2 = FontDescriptor::system("Arial", 700, FontStyle::Normal, 16.0);

        // Different weights should have different cache keys
        assert_ne!(font1.cache_key(), font2.cache_key());
    }

    #[test]
    fn test_text_layout_defaults() {
        let layout = TextLayoutConfig::default();
        assert_eq!(layout.line_height, 1.5);
        assert_eq!(layout.alignment, TextAlign::Left);
        assert!(layout.max_width.is_none());
    }

    #[test]
    fn test_enum_conversions() {
        assert_eq!(FontStyle::from(0), FontStyle::Normal);
        assert_eq!(FontStyle::from(1), FontStyle::Italic);

        assert_eq!(TextAlign::from(0), TextAlign::Left);
        assert_eq!(TextAlign::from(1), TextAlign::Center);

        assert_eq!(WordBreak::from(3), WordBreak::BreakWord);
    }
}
