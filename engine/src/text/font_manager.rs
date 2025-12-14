//! Font Manager - Platform-specific font loading and caching
//!
//! Provides a unified interface for loading fonts from different sources
//! (system, bundled files, embedded data) and caching them for performance.

use super::{FontDescriptor, FontSource, FontStyle};
use std::collections::HashMap;

// Core Text is available on both macOS and iOS
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use macos::MacOSFontManager as PlatformFontManager;

// Stub font manager for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub struct StubFontManager;

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
impl StubFontManager {
    pub fn new() -> Self { Self }
    pub fn load_font(&mut self, _desc: &FontDescriptor) -> Result<Font, FontError> {
        Err(FontError::NotFound("Platform not supported".into()))
    }
    pub fn get_or_load_font(&mut self, _desc: &FontDescriptor) -> Result<Font, FontError> {
        Err(FontError::NotFound("Platform not supported".into()))
    }
    pub fn measure_text(&mut self, _text: &str, _desc: &FontDescriptor) -> f32 {
        0.0
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub type PlatformFontManager = StubFontManager;

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
