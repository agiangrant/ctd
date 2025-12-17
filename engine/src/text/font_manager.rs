//! Font Manager - Platform-specific font loading and caching
//!
//! Provides a unified interface for loading fonts from different sources
//! (system, bundled files, embedded data) and caching them for performance.
//!
//! # Platform Implementation Guide
//!
//! To add support for a new platform, implement the [`PlatformFontManagerTrait`] trait:
//!
//! 1. Create a new module (e.g., `windows.rs` for Windows/DirectWrite)
//! 2. Implement a font struct that implements the [`Font`] trait
//! 3. Implement a font manager struct that implements [`PlatformFontManagerTrait`]
//! 4. Add the appropriate `#[cfg(...)]` gates in this file
//!
//! See `macos.rs` for a reference implementation using Core Text.

use super::{FontDescriptor, FontSource, FontStyle};
use std::collections::HashMap;

// Platform-specific font manager implementations
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use macos::MacOSFontManager as PlatformFontManager;

// Stub font manager for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub use stub::StubFontManager as PlatformFontManager;

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod stub {
    use super::*;

    /// Stub font manager for platforms without text rendering support.
    /// Returns errors for all operations.
    pub struct StubFontManager;

    impl PlatformFontManagerTrait for StubFontManager {
        fn new() -> Self {
            Self
        }

        fn load_system_font(
            &mut self,
            name: &str,
            _weight: u16,
            _style: FontStyle,
            _size: f32,
        ) -> Result<Box<dyn Font>, FontError> {
            Err(FontError::Unsupported(format!(
                "Text rendering not supported on this platform. Cannot load font '{}'",
                name
            )))
        }

        fn load_font_from_data(
            &mut self,
            _data: &[u8],
            _weight: u16,
            _style: FontStyle,
            _size: f32,
        ) -> Result<Box<dyn Font>, FontError> {
            Err(FontError::Unsupported(
                "Text rendering not supported on this platform".to_string()
            ))
        }
    }
}

/// Trait for platform-specific font loading implementations.
///
/// Each platform (macOS, Windows, Linux, Web) implements this trait using
/// native text rendering APIs:
/// - macOS/iOS: Core Text
/// - Windows: DirectWrite
/// - Linux: FreeType + HarfBuzz
/// - Web/WASM: Canvas 2D API
///
/// # Implementation Notes
///
/// - Font weight uses the CSS numeric scale (100-900, where 400 = normal, 700 = bold)
/// - Font style is either Normal or Italic
/// - Returned fonts must be thread-safe (`Send + Sync`)
/// - Font data for `load_font_from_data` is raw TTF/OTF bytes
pub trait PlatformFontManagerTrait {
    /// Create a new platform font manager.
    fn new() -> Self where Self: Sized;

    /// Load a system font by name.
    ///
    /// # Arguments
    /// - `name`: Font family name (e.g., "Helvetica", "Arial") or "system" for platform default
    /// - `weight`: Font weight (100-900 scale, 400 = normal, 700 = bold)
    /// - `style`: Font style (Normal or Italic)
    /// - `size`: Font size in points
    ///
    /// # Platform-Specific Behavior
    /// - "system" or empty string should return the platform's default UI font
    /// - Weight mapping may vary by platform (some fonts only support regular/bold)
    fn load_system_font(
        &mut self,
        name: &str,
        weight: u16,
        style: FontStyle,
        size: f32,
    ) -> Result<Box<dyn Font>, FontError>;

    /// Load a font from raw TTF/OTF data.
    ///
    /// # Arguments
    /// - `data`: Raw font file bytes (TTF or OTF format)
    /// - `weight`: Requested font weight (may be ignored if font doesn't support variants)
    /// - `style`: Requested font style (may be ignored if font doesn't support variants)
    /// - `size`: Font size in points
    fn load_font_from_data(
        &mut self,
        data: &[u8],
        weight: u16,
        style: FontStyle,
        size: f32,
    ) -> Result<Box<dyn Font>, FontError>;
}

/// Glyph metrics for a single character
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    /// Glyph ID in the font
    pub glyph_id: u32,

    /// Horizontal advance (space to next character)
    pub advance: f32,

    /// Glyph width in pixels
    pub width: f32,

    /// Glyph height in pixels
    pub height: f32,

    /// Horizontal bearing (offset from origin)
    pub bearing_x: f32,

    /// Vertical bearing (offset from baseline)
    pub bearing_y: f32,
}

/// Font handle - platform-specific font object
pub trait Font: Send + Sync {
    /// Get metrics for a specific character
    fn glyph_metrics(&self, character: char) -> Option<GlyphMetrics>;

    /// Get the font's ascent (height above baseline)
    fn ascent(&self) -> f32;

    /// Get the font's descent (height below baseline)
    fn descent(&self) -> f32;

    /// Get the font's line height (recommended line spacing)
    fn line_height(&self) -> f32;

    /// Get the font's cap height (height of capital letters)
    fn cap_height(&self) -> f32;

    /// Get the font's x-height (height of lowercase 'x')
    fn x_height(&self) -> f32;

    /// Get the font size in points
    fn size(&self) -> f32;

    /// Measure the width of a string
    fn measure_text(&self, text: &str) -> f32 {
        text.chars()
            .filter_map(|c| self.glyph_metrics(c))
            .map(|m| m.advance)
            .sum()
    }
}

/// Font Manager - loads and caches fonts
pub struct FontManager {
    /// Platform-specific font manager
    platform: PlatformFontManager,

    /// Font cache (cache_key → Font)
    cache: HashMap<String, Box<dyn Font>>,

    /// Font data cache for bundled/memory fonts
    font_data_cache: HashMap<u64, Vec<u8>>,
}

impl FontManager {
    /// Create a new font manager
    pub fn new() -> Self {
        Self {
            platform: PlatformFontManager::new(),
            cache: HashMap::new(),
            font_data_cache: HashMap::new(),
        }
    }

    /// Load a font (with caching)
    pub fn load_font(&mut self, descriptor: &FontDescriptor) -> Result<&dyn Font, FontError> {
        let cache_key = descriptor.cache_key();

        // Check cache first
        if self.cache.contains_key(&cache_key) {
            return Ok(self.cache.get(&cache_key).unwrap().as_ref());
        }

        // Load font based on source
        let font: Box<dyn Font> = match &descriptor.source {
            FontSource::System(name) => {
                self.platform.load_system_font(name, descriptor.weight, descriptor.style, descriptor.size)?
            }

            FontSource::Bundled(path) => {
                // Load font file
                let font_data = std::fs::read(path)
                    .map_err(|e| FontError::LoadFailed(format!("Failed to read font file: {}", e)))?;

                self.platform.load_font_from_data(&font_data, descriptor.weight, descriptor.style, descriptor.size)?
            }

            FontSource::Memory { data_hash, .. } => {
                // Get font data from cache
                let font_data = self.font_data_cache.get(data_hash)
                    .ok_or_else(|| FontError::LoadFailed("Font data not found in cache".to_string()))?;

                self.platform.load_font_from_data(font_data, descriptor.weight, descriptor.style, descriptor.size)?
            }
        };

        // Cache the font
        self.cache.insert(cache_key.clone(), font);

        Ok(self.cache.get(&cache_key).unwrap().as_ref())
    }

    /// Register embedded font data (for Memory fonts)
    pub fn register_font_data(&mut self, name: &str, data: Vec<u8>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        data.hash(&mut hasher);
        let hash = hasher.finish();

        self.font_data_cache.insert(hash, data);
        hash
    }

    /// Clear the font cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> FontCacheStats {
        FontCacheStats {
            cached_fonts: self.cache.len(),
            embedded_fonts: self.font_data_cache.len(),
        }
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Font cache statistics
#[derive(Debug, Clone)]
pub struct FontCacheStats {
    pub cached_fonts: usize,
    pub embedded_fonts: usize,
}

/// Font loading errors
#[derive(Debug, Clone)]
pub enum FontError {
    /// Font not found
    NotFound(String),

    /// Failed to load font
    LoadFailed(String),

    /// Invalid font data
    InvalidData(String),

    /// Platform error
    PlatformError(String),

    /// Feature not yet supported
    Unsupported(String),
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::NotFound(msg) => write!(f, "Font not found: {}", msg),
            FontError::LoadFailed(msg) => write!(f, "Failed to load font: {}", msg),
            FontError::InvalidData(msg) => write!(f, "Invalid font data: {}", msg),
            FontError::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            FontError::Unsupported(msg) => write!(f, "Unsupported feature: {}", msg),
        }
    }
}

impl std::error::Error for FontError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_creation() {
        let manager = FontManager::new();
        let stats = manager.cache_stats();
        assert_eq!(stats.cached_fonts, 0);
        assert_eq!(stats.embedded_fonts, 0);
    }

    #[test]
    fn test_register_font_data() {
        let mut manager = FontManager::new();
        let font_data = vec![0u8; 1024]; // Dummy font data

        let hash = manager.register_font_data("TestFont", font_data);
        assert!(hash > 0);

        let stats = manager.cache_stats();
        assert_eq!(stats.embedded_fonts, 1);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_load_system_font() {
        let mut manager = FontManager::new();
        let descriptor = FontDescriptor::system("Helvetica", 400, FontStyle::Normal, 16.0);

        let result = manager.load_font(&descriptor);
        assert!(result.is_ok(), "Failed to load system font: {:?}", result.err());

        // Should be cached now
        let stats = manager.cache_stats();
        assert_eq!(stats.cached_fonts, 1);

        // Loading again should use cache
        let result2 = manager.load_font(&descriptor);
        assert!(result2.is_ok());

        // Cache size should still be 1
        let stats2 = manager.cache_stats();
        assert_eq!(stats2.cached_fonts, 1);
    }
}
