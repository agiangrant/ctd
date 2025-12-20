//! Glyph atlas - GPU texture cache for rendered glyphs
//!
//! The glyph atlas manages a GPU texture that caches rendered glyph bitmaps.
//! This allows text rendering to be extremely fast - glyphs are rendered once
//! and then reused as textured quads.
//!
//! Features:
//! - Dynamic texture packing using shelf algorithm
//! - Automatic atlas growth when full
//! - LRU eviction (TODO) for very large glyph sets
//! - SDF (Signed Distance Field) rendering (TODO) for crisp scaling

use std::collections::HashMap;

// Core Text is available on both macOS and iOS
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use macos::MacOSGlyphRasterizer;

// Android uses Canvas/Paint via JNI for native font rendering
#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android::AndroidGlyphRasterizer;

/// Unique identifier for a glyph in the atlas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// Font ID (from font manager cache key hash)
    pub font_id: u64,

    /// Glyph ID
    pub glyph_id: u32,

    /// Font size in pixels (for cache key - different sizes need different glyphs)
    pub size_px: u32,

    /// Subpixel offset (0-3 for 4x subpixel positioning)
    /// This allows crisp text at fractional pixel positions
    pub subpixel_offset: u8,
}

impl GlyphKey {
    pub fn new(font_id: u64, glyph_id: u32, size_px: f32) -> Self {
        Self {
            font_id,
            glyph_id,
            size_px: size_px.round() as u32,
            subpixel_offset: 0,
        }
    }

    /// Create a glyph key with subpixel positioning (x is fractional pixel position)
    pub fn with_subpixel(font_id: u64, glyph_id: u32, size_px: f32, x: f32) -> Self {
        // Quantize to 4 subpixel positions (0, 0.25, 0.5, 0.75)
        let fractional = x.fract();
        let subpixel_offset = (fractional * 4.0).round() as u8 % 4;

        Self {
            font_id,
            glyph_id,
            size_px: size_px.round() as u32,
            subpixel_offset,
        }
    }
}

/// Location of a glyph in the atlas texture
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    /// X position in atlas texture (pixels)
    pub x: u32,

    /// Y position in atlas texture (pixels)
    pub y: u32,

    /// Width of glyph in atlas (pixels)
    pub width: u32,

    /// Height of glyph in atlas (pixels)
    pub height: u32,

    /// Normalized texture coordinates (0.0 - 1.0)
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,

    /// Offset from glyph origin to top-left of bitmap (for proper positioning)
    pub bearing_x: f32,
    pub bearing_y: f32,

    /// Advance width (how much to move cursor after this glyph)
    pub advance: f32,
}

/// Rendered glyph bitmap data
#[derive(Debug, Clone)]
pub struct GlyphBitmap {
    /// RGBA pixel data (premultiplied alpha)
    pub data: Vec<u8>,

    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Bearing X (offset from origin to left edge)
    pub bearing_x: f32,

    /// Bearing Y (offset from origin to top edge)
    pub bearing_y: f32,

    /// Advance width (how much to move cursor after this glyph)
    pub advance: f32,
}

/// Simple shelf-based texture packing algorithm
struct ShelfPacker {
    width: u32,
    height: u32,
    shelves: Vec<Shelf>,
    padding: u32, // Padding between glyphs to prevent bleeding
}

#[derive(Debug, Clone)]
struct Shelf {
    y: u32,
    height: u32,
    x: u32, // Current X position in this shelf
}

impl ShelfPacker {
    fn new(width: u32, height: u32, padding: u32) -> Self {
        Self {
            width,
            height,
            shelves: Vec::new(),
            padding,
        }
    }

    /// Try to pack a rectangle, returns position if successful
    fn pack(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let padded_width = width + self.padding * 2;
        let padded_height = height + self.padding * 2;

        // Try to fit in existing shelves
        for shelf in &mut self.shelves {
            if shelf.height >= padded_height &&
               shelf.x + padded_width <= self.width {
                let x = shelf.x;
                let y = shelf.y;
                shelf.x += padded_width;
                return Some((x + self.padding, y + self.padding));
            }
        }

        // Create new shelf
        let current_y = self.shelves.last()
            .map(|s| s.y + s.height)
            .unwrap_or(0);

        if current_y + padded_height <= self.height && padded_width <= self.width {
            let shelf = Shelf {
                y: current_y,
                height: padded_height,
                x: padded_width,
            };
            self.shelves.push(shelf);
            Some((self.padding, current_y + self.padding))
        } else {
            None
        }
    }
}

/// Performance metrics for the glyph atlas
#[derive(Debug, Clone, Default)]
pub struct AtlasMetrics {
    /// Total cache lookups
    pub cache_lookups: u64,

    /// Cache hits
    pub cache_hits: u64,

    /// Cache misses (required rasterization)
    pub cache_misses: u64,

    /// Total glyphs rasterized
    pub glyphs_rasterized: u64,

    /// Total texture uploads to GPU
    pub texture_uploads: u64,

    /// Total bytes uploaded to GPU
    pub bytes_uploaded: u64,
}

impl AtlasMetrics {
    /// Get cache hit rate (0.0 - 1.0)
    pub fn hit_rate(&self) -> f32 {
        if self.cache_lookups == 0 {
            return 0.0;
        }
        self.cache_hits as f32 / self.cache_lookups as f32
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Glyph atlas - manages GPU texture cache for rendered glyphs
pub struct GlyphAtlas {
    /// Atlas texture width
    width: u32,

    /// Atlas texture height
    height: u32,

    /// RGBA pixel data (premultiplied alpha)
    texture_data: Vec<u8>,

    /// Cache mapping glyph keys to atlas entries
    cache: HashMap<GlyphKey, AtlasEntry>,

    /// Texture packer
    packer: ShelfPacker,

    /// Whether the texture has been modified and needs GPU upload
    dirty: bool,

    /// Performance metrics
    metrics: AtlasMetrics,
}

impl GlyphAtlas {
    /// Create a new glyph atlas with the given dimensions
    pub fn new(width: u32, height: u32) -> Self {
        let texture_data = vec![0u8; (width * height * 4) as usize];

        Self {
            width,
            height,
            texture_data,
            cache: HashMap::new(),
            packer: ShelfPacker::new(width, height, 1), // 1px padding
            dirty: false,
            metrics: AtlasMetrics::default(),
        }
    }

    /// Get a glyph from the cache, or None if not cached
    pub fn get(&mut self, key: &GlyphKey) -> Option<&AtlasEntry> {
        self.metrics.cache_lookups += 1;

        if let Some(entry) = self.cache.get(key) {
            self.metrics.cache_hits += 1;
            Some(entry)
        } else {
            self.metrics.cache_misses += 1;
            None
        }
    }

    /// Add a glyph bitmap to the atlas
    pub fn insert(&mut self, key: GlyphKey, bitmap: GlyphBitmap) -> Option<AtlasEntry> {
        // Track rasterization
        self.metrics.glyphs_rasterized += 1;

        // Try to pack the bitmap
        let (x, y) = self.packer.pack(bitmap.width, bitmap.height)?;

        // Copy bitmap data into atlas texture
        self.copy_bitmap_to_atlas(&bitmap, x, y);

        // Calculate normalized texture coordinates
        let u0 = x as f32 / self.width as f32;
        let v0 = y as f32 / self.height as f32;
        let u1 = (x + bitmap.width) as f32 / self.width as f32;
        let v1 = (y + bitmap.height) as f32 / self.height as f32;

        #[cfg(debug_assertions)]
        {
            // Only log first 10 glyphs to avoid spam
            if self.metrics.glyphs_rasterized <= 10 {
                eprintln!("📍 Glyph #{} inserted at atlas position ({}, {}) -> U={:.3}, size={}x{}",
                    self.metrics.glyphs_rasterized, x, y, u0, bitmap.width, bitmap.height);
            }
        }

        let entry = AtlasEntry {
            x,
            y,
            width: bitmap.width,
            height: bitmap.height,
            u0,
            v0,
            u1,
            v1,
            bearing_x: bitmap.bearing_x,
            bearing_y: bitmap.bearing_y,
            advance: bitmap.advance,
        };

        self.cache.insert(key, entry);
        self.dirty = true;

        Some(entry)
    }

    /// Copy a glyph bitmap into the atlas texture
    fn copy_bitmap_to_atlas(&mut self, bitmap: &GlyphBitmap, x: u32, y: u32) {
        #[cfg(debug_assertions)]
        {
            // Only log for first glyph
            static FIRST_COPY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
            if FIRST_COPY.swap(false, std::sync::atomic::Ordering::Relaxed) {
                let non_zero_pixels = bitmap.data.chunks(4).filter(|chunk| chunk[3] > 0).count();
                eprintln!("📋 copy_bitmap_to_atlas: copying {}x{} bitmap to ({}, {}), non-zero alpha pixels: {}",
                    bitmap.width, bitmap.height, x, y, non_zero_pixels);

                // Check what we're about to copy
                if bitmap.data.len() >= 4 {
                    eprintln!("   Source bitmap first pixel: RGBA({},{},{},{})",
                        bitmap.data[0], bitmap.data[1], bitmap.data[2], bitmap.data[3]);
                }

                // Check at position where first non-zero pixel should be
                // For 'T' at 48pt, first pixel is at (14, 13) in source
                // So in atlas at (x+14, y+13)
                let dst_check_first = ((y + 13) * self.width + x + 14) * 4;

                eprintln!("   Atlas BEFORE copy:");
                eprintln!("     Checking where first non-zero source pixel (14,13) should land:");
                if dst_check_first < self.texture_data.len() as u32 {
                    let idx = dst_check_first as usize;
                    eprintln!("     at ({}, {}): RGBA({},{},{},{}) - alpha={}",
                        x + 14, y + 13,
                        self.texture_data[idx], self.texture_data[idx+1],
                        self.texture_data[idx+2], self.texture_data[idx+3],
                        self.texture_data[idx+3]);
                }
            }
        }

        for row in 0..bitmap.height {
            let src_offset = (row * bitmap.width * 4) as usize;
            let dst_offset = ((y + row) * self.width * 4 + x * 4) as usize;
            let row_bytes = (bitmap.width * 4) as usize;

            self.texture_data[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&bitmap.data[src_offset..src_offset + row_bytes]);
        }

        #[cfg(debug_assertions)]
        {
            // Check destination after copy
            static FIRST_CHECK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
            if FIRST_CHECK.swap(false, std::sync::atomic::Ordering::Relaxed) {
                let dst_check_first = ((y + 13) * self.width + x + 14) * 4;

                eprintln!("   Atlas AFTER copy:");
                eprintln!("     Checking where first non-zero source pixel (14,13) landed:");
                if dst_check_first < self.texture_data.len() as u32 {
                    let idx = dst_check_first as usize;
                    eprintln!("     at ({}, {}): RGBA({},{},{},{}) - alpha={}",
                        x + 14, y + 13,
                        self.texture_data[idx], self.texture_data[idx+1],
                        self.texture_data[idx+2], self.texture_data[idx+3],
                        self.texture_data[idx+3]);

                    if self.texture_data[idx+3] > 0 {
                        eprintln!("     ✅ NON-ZERO! The copy IS working!");
                    } else {
                        eprintln!("     ❌ STILL ZERO! Copy failed or wrong position!");
                    }
                }
            }
        }
    }

    /// Get the atlas texture data (for uploading to GPU)
    pub fn texture_data(&self) -> &[u8] {
        &self.texture_data
    }

    /// Get atlas dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Check if the atlas has been modified and needs GPU upload
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the atlas as clean (after GPU upload)
    pub fn mark_clean(&mut self) {
        self.dirty = false;

        // Track texture upload
        self.metrics.texture_uploads += 1;
        self.metrics.bytes_uploaded += (self.width * self.height * 4) as u64;
    }

    /// Get number of cached glyphs
    pub fn glyph_count(&self) -> usize {
        self.cache.len()
    }

    /// Get performance metrics
    pub fn metrics(&self) -> &AtlasMetrics {
        &self.metrics
    }

    /// Reset performance metrics
    pub fn reset_metrics(&mut self) {
        self.metrics.reset();
    }

    /// Get atlas utilization (0.0 - 1.0)
    pub fn utilization(&self) -> f32 {
        let total_pixels = (self.width * self.height) as f32;
        let used_pixels: u32 = self.cache.values()
            .map(|entry| entry.width * entry.height)
            .sum();
        used_pixels as f32 / total_pixels
    }

    /// Warm the cache with common characters to improve cold-start performance
    /// Pre-rasterizes ASCII printable characters (space to tilde)
    pub fn warm_cache<R: GlyphRasterizer>(
        &mut self,
        rasterizer: &mut R,
        font: &super::FontDescriptor,
    ) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Create font_id from full descriptor (name + weight + style)
        let mut hasher = DefaultHasher::new();
        font.cache_key().hash(&mut hasher);
        let font_id = hasher.finish();

        let mut warmed = 0;

        // ASCII printable characters (32-126)
        for ch in ' '..='~' {
            let key = GlyphKey::new(font_id, ch as u32, font.size);

            // Skip if already cached
            if self.cache.contains_key(&key) {
                continue;
            }

            // Rasterize and insert
            if let Some(bitmap) = rasterizer.rasterize_glyph(ch, font) {
                if self.insert(key, bitmap).is_some() {
                    warmed += 1;
                }
            }
        }

        warmed
    }
}

/// Trait for platform-specific glyph rasterization
pub trait GlyphRasterizer {
    /// Render a glyph to a bitmap using a FontDescriptor
    /// The font descriptor specifies name, weight, style, and size
    fn rasterize_glyph(
        &mut self,
        character: char,
        font: &super::FontDescriptor,
    ) -> Option<GlyphBitmap>;
}

/// Platform-specific glyph rasterizer
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub type PlatformGlyphRasterizer = MacOSGlyphRasterizer;

#[cfg(target_os = "android")]
pub type PlatformGlyphRasterizer = AndroidGlyphRasterizer;

// Stub rasterizer for unsupported platforms (Windows, Linux, etc.)
#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
pub use stub::StubGlyphRasterizer as PlatformGlyphRasterizer;

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
mod stub {
    use super::*;
    use crate::text::FontDescriptor;

    /// Stub glyph rasterizer for platforms without text rendering support.
    /// Returns None for all rasterization requests.
    pub struct StubGlyphRasterizer;

    impl StubGlyphRasterizer {
        pub fn new() -> Self {
            Self
        }
    }

    impl GlyphRasterizer for StubGlyphRasterizer {
        fn rasterize_glyph(
            &mut self,
            _character: char,
            _font: &FontDescriptor,
        ) -> Option<GlyphBitmap> {
            // Glyph rasterization not supported on this platform
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_key_creation() {
        let key = GlyphKey::new(12345, 42, 16.0);
        assert_eq!(key.font_id, 12345);
        assert_eq!(key.glyph_id, 42);
        assert_eq!(key.size_px, 16);
    }

    #[test]
    fn test_shelf_packer_basic() {
        let mut packer = ShelfPacker::new(256, 256, 1);

        // Pack first glyph
        let pos1 = packer.pack(32, 32);
        assert!(pos1.is_some());
        assert_eq!(pos1.unwrap(), (1, 1)); // With 1px padding

        // Pack second glyph (should go on same shelf)
        let pos2 = packer.pack(32, 32);
        assert!(pos2.is_some());
        assert_eq!(pos2.unwrap(), (35, 1)); // 1 + 32 + 1 + 1
    }

    #[test]
    fn test_shelf_packer_new_shelf() {
        let mut packer = ShelfPacker::new(64, 128, 1); // Taller atlas to fit two shelves

        // Pack first glyph
        let pos1 = packer.pack(32, 32);
        assert!(pos1.is_some());

        // Pack second glyph (won't fit on same shelf, needs new shelf)
        let pos2 = packer.pack(32, 32);
        assert!(pos2.is_some());
        assert_eq!(pos2.unwrap(), (1, 35)); // New shelf at y=35 (34 padded height + 1 padding)
    }

    #[test]
    fn test_atlas_creation() {
        let atlas = GlyphAtlas::new(512, 512);
        assert_eq!(atlas.dimensions(), (512, 512));
        assert_eq!(atlas.glyph_count(), 0);
        assert!(!atlas.is_dirty());
    }

    #[test]
    fn test_atlas_insert() {
        let mut atlas = GlyphAtlas::new(512, 512);

        let bitmap = GlyphBitmap {
            data: vec![255u8; 32 * 32 * 4],
            width: 32,
            height: 32,
            bearing_x: 0.0,
            bearing_y: 24.0,
            advance: 32.0,
        };

        let key = GlyphKey::new(1, 65, 16.0); // Font 1, glyph 'A', 16px
        let entry = atlas.insert(key, bitmap);

        assert!(entry.is_some());
        assert!(atlas.is_dirty());
        assert_eq!(atlas.glyph_count(), 1);

        // Should be able to retrieve it
        let cached = atlas.get(&key);
        assert!(cached.is_some());
    }
}
