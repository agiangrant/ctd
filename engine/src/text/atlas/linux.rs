//! Linux glyph rasterization using FreeType
//!
//! Renders glyphs to RGBA bitmaps using FreeType library.
//! Supports font weights and styles via fontconfig font matching.

use super::{GlyphBitmap, GlyphRasterizer};
use crate::text::{FontDescriptor, FontSource, FontStyle};
use fontconfig::{Fontconfig, Pattern, FC_FAMILY, FC_SLANT, FC_WEIGHT};
use fontconfig::{
    FC_SLANT_ITALIC, FC_SLANT_ROMAN, FC_WEIGHT_BLACK, FC_WEIGHT_BOLD, FC_WEIGHT_EXTRALIGHT,
    FC_WEIGHT_LIGHT, FC_WEIGHT_MEDIUM, FC_WEIGHT_REGULAR, FC_WEIGHT_SEMIBOLD, FC_WEIGHT_THIN,
    FC_WEIGHT_EXTRABOLD,
};
use freetype::face::LoadFlag;
use freetype::Library;
use std::collections::HashMap;
use std::ffi::CString;
use std::path::PathBuf;

/// Check if a character is an emoji (should render with native colors, not white)
fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        // Emoticons
        0x1F600..=0x1F64F |
        // Miscellaneous Symbols and Pictographs
        0x1F300..=0x1F5FF |
        // Transport and Map Symbols
        0x1F680..=0x1F6FF |
        // Supplemental Symbols and Pictographs
        0x1F900..=0x1F9FF |
        // Symbols and Pictographs Extended-A
        0x1FA00..=0x1FA6F |
        // Symbols and Pictographs Extended-B
        0x1FA70..=0x1FAFF |
        // Dingbats
        0x2700..=0x27BF |
        // Miscellaneous Symbols
        0x2600..=0x26FF |
        // Regional Indicator Symbols (flags)
        0x1F1E0..=0x1F1FF
    )
}

/// Cache key for loaded fonts
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct FontCacheKey {
    path: String,
    size_px: u32,
}

/// Cache key for font path lookups (avoids repeated fontconfig queries)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct FontPathCacheKey {
    family: String,
    weight: u16,
    italic: bool,
}

/// Linux glyph rasterizer using FreeType
pub struct LinuxGlyphRasterizer {
    /// FreeType library handle
    library: Library,
    /// Cache of loaded faces (path+size -> Face)
    face_cache: HashMap<FontCacheKey, freetype::Face>,
    /// Cache of font paths (family+weight+italic -> path) to avoid fontconfig lookups
    font_path_cache: HashMap<FontPathCacheKey, Option<String>>,
    /// Cache of resolved bundled font paths
    bundled_path_cache: HashMap<String, Option<String>>,
}

impl LinuxGlyphRasterizer {
    pub fn new() -> Self {
        let library = Library::init().expect("Failed to initialize FreeType library");
        Self {
            library,
            face_cache: HashMap::new(),
            font_path_cache: HashMap::new(),
            bundled_path_cache: HashMap::new(),
        }
    }

    /// Convert CSS font weight (100-900) to fontconfig weight constant
    fn css_weight_to_fc(weight: u16) -> i32 {
        match weight {
            0..=149 => FC_WEIGHT_THIN,
            150..=249 => FC_WEIGHT_EXTRALIGHT,
            250..=349 => FC_WEIGHT_LIGHT,
            350..=449 => FC_WEIGHT_REGULAR,
            450..=549 => FC_WEIGHT_MEDIUM,
            550..=649 => FC_WEIGHT_SEMIBOLD,
            650..=749 => FC_WEIGHT_BOLD,
            750..=849 => FC_WEIGHT_EXTRABOLD,
            _ => FC_WEIGHT_BLACK,
        }
    }

    /// Find font file path using fontconfig
    fn find_font_path(family: &str, weight: u16, italic: bool) -> Option<PathBuf> {
        let fc = Fontconfig::new()?;

        // Build pattern for font matching
        let family_name = if family == "system" || family.is_empty() {
            "sans-serif"
        } else {
            family
        };

        let family_cstr = CString::new(family_name).ok()?;
        let mut pattern = Pattern::new(&fc);
        pattern.add_string(FC_FAMILY, &family_cstr);

        // Set weight
        let fc_weight = Self::css_weight_to_fc(weight);
        pattern.add_integer(FC_WEIGHT, fc_weight);

        // Set slant
        let slant = if italic {
            FC_SLANT_ITALIC
        } else {
            FC_SLANT_ROMAN
        };
        pattern.add_integer(FC_SLANT, slant);

        // Match and get the font path
        pattern.config_substitute();
        pattern.default_substitute();

        let matched = pattern.font_match();
        matched.filename().map(PathBuf::from)
    }

    /// Load a FreeType face from a file path at the given size
    fn load_face(&mut self, path: &str, size_px: f32) -> Option<&freetype::Face> {
        use std::collections::hash_map::Entry;

        let cache_key = FontCacheKey {
            path: path.to_string(),
            size_px: size_px.round() as u32,
        };

        // Use entry API for single lookup
        match self.face_cache.entry(cache_key) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(entry) => {
                // Load the face
                let face = match self.library.new_face(path, 0) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Failed to load font '{}': {:?}", path, e);
                        return None;
                    }
                };

                // Set character size (FreeType uses 26.6 fixed-point, so multiply by 64)
                // Set DPI to 72 so that point size equals pixel size
                if let Err(e) = face.set_char_size(0, (size_px * 64.0) as isize, 72, 72) {
                    eprintln!("Failed to set font size: {:?}", e);
                    return None;
                }

                Some(entry.insert(face))
            }
        }
    }

    /// Get the font file path from a FontDescriptor (with caching)
    fn get_font_path(&mut self, font: &FontDescriptor) -> Option<String> {
        match &font.source {
            FontSource::System(name) => {
                let cache_key = FontPathCacheKey {
                    family: name.clone(),
                    weight: font.weight,
                    italic: font.style == FontStyle::Italic,
                };

                // Check cache first
                if let Some(cached) = self.font_path_cache.get(&cache_key) {
                    return cached.clone();
                }

                // Not cached - do fontconfig lookup
                let result = Self::find_font_path(name, font.weight, font.style == FontStyle::Italic)
                    .map(|p| p.to_string_lossy().to_string());

                // Cache the result (even if None, to avoid repeated failed lookups)
                self.font_path_cache.insert(cache_key, result.clone());
                result
            }
            FontSource::Bundled(path) => {
                // Check cache first
                if let Some(cached) = self.bundled_path_cache.get(path) {
                    return cached.clone();
                }

                // Not cached - resolve the path
                let result = Self::resolve_bundled_path(path);

                // Cache the result
                self.bundled_path_cache.insert(path.clone(), result.clone());
                result
            }
            FontSource::Memory { .. } => {
                // Memory fonts not yet supported in Linux rasterizer
                eprintln!("Memory fonts not yet supported on Linux");
                None
            }
        }
    }

    /// Resolve a bundled font path (checks multiple locations)
    fn resolve_bundled_path(path: &str) -> Option<String> {
        // Check if path exists as-is
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }

        // Try relative to current directory
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_path = cwd.join(path);
            if cwd_path.exists() {
                return Some(cwd_path.to_string_lossy().to_string());
            }
        }

        // Try relative to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let exe_path = exe_dir.join(path);
                if exe_path.exists() {
                    return Some(exe_path.to_string_lossy().to_string());
                }
            }
        }

        eprintln!("Font file not found: {}", path);
        None
    }

    /// Measure the width of a string
    pub fn measure_string(&mut self, text: &str, font: &FontDescriptor) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        let font_path = match self.get_font_path(font) {
            Some(p) => p,
            None => return 0.0,
        };

        let face = match self.load_face(&font_path, font.size) {
            Some(f) => f,
            None => return 0.0,
        };

        let mut width = 0.0;
        for ch in text.chars() {
            if let Some(glyph_index) = face.get_char_index(ch as usize) {
                if face.load_glyph(glyph_index, LoadFlag::DEFAULT).is_ok() {
                    // Advance is in 26.6 fixed-point format
                    width += face.glyph().advance().x as f32 / 64.0;
                }
            }
        }
        width
    }

    /// Get font metrics (ascent, descent) for a font descriptor
    /// Returns (ascent, descent) in pixels, or (0.0, 0.0) on error
    pub fn get_font_metrics(&mut self, font: &FontDescriptor) -> (f32, f32) {
        let font_path = match self.get_font_path(font) {
            Some(p) => p,
            None => return (0.0, 0.0),
        };

        let face = match self.load_face(&font_path, font.size) {
            Some(f) => f,
            None => return (0.0, 0.0),
        };

        // Get metrics from size_metrics (values are in 26.6 fixed-point format)
        if let Some(size_metrics) = face.size_metrics() {
            let ascent = size_metrics.ascender as f32 / 64.0;
            let descent = (size_metrics.descender.abs()) as f32 / 64.0;
            (ascent, descent)
        } else {
            // Fallback for bitmap fonts
            (font.size * 0.8, font.size * 0.2)
        }
    }
}

impl Default for LinuxGlyphRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: LinuxGlyphRasterizer is only accessed from the main render thread via the
// global BACKEND mutex in ffi.rs. The FreeType library and face cache are not shared
// across threads - they're always accessed through a single-threaded path guarded by
// the parent Mutex<Option<WgpuBackend>>.
unsafe impl Send for LinuxGlyphRasterizer {}
unsafe impl Sync for LinuxGlyphRasterizer {}

impl GlyphRasterizer for LinuxGlyphRasterizer {
    fn rasterize_glyph(
        &mut self,
        character: char,
        font: &FontDescriptor,
    ) -> Option<GlyphBitmap> {
        let font_path = self.get_font_path(font)?;
        let face = self.load_face(&font_path, font.size)?;

        // Handle whitespace characters - no visual glyph needed
        if character.is_whitespace() {
            let glyph_index = face.get_char_index(character as usize)?;
            face.load_glyph(glyph_index, LoadFlag::DEFAULT).ok()?;
            let advance = face.glyph().advance().x as f32 / 64.0;

            return Some(GlyphBitmap {
                data: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
            });
        }

        // Get glyph index for the character
        let glyph_index = face.get_char_index(character as usize)?;

        // Load and render the glyph
        if face.load_glyph(glyph_index, LoadFlag::RENDER).is_err() {
            return None;
        }

        let glyph = face.glyph();
        let bitmap = glyph.bitmap();
        let metrics = glyph.metrics();

        let width = bitmap.width() as u32;
        let height = bitmap.rows() as u32;
        let advance = glyph.advance().x as f32 / 64.0;

        // Bearing values are in 26.6 fixed-point
        let bearing_x = (metrics.horiBearingX >> 6) as f32;
        let bearing_y = (metrics.horiBearingY >> 6) as f32;

        if width == 0 || height == 0 {
            // Empty glyph (control character or similar)
            return Some(GlyphBitmap {
                data: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
            });
        }

        // Convert grayscale bitmap to RGBA
        let buffer = bitmap.buffer();
        let pitch = bitmap.pitch().unsigned_abs() as usize;
        let char_is_emoji = is_emoji(character);

        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height as usize {
            for x in 0..width as usize {
                let alpha = buffer[y * pitch + x];
                if char_is_emoji {
                    // For emoji, use white color with alpha
                    rgba_data.push(255); // R
                    rgba_data.push(255); // G
                    rgba_data.push(255); // B
                    rgba_data.push(alpha); // A
                } else {
                    // Regular text: white color with alpha from bitmap
                    rgba_data.push(255); // R
                    rgba_data.push(255); // G
                    rgba_data.push(255); // B
                    rgba_data.push(alpha); // A
                }
            }
        }

        Some(GlyphBitmap {
            data: rgba_data,
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterizer_creation() {
        let rasterizer = LinuxGlyphRasterizer::new();
        let _ = rasterizer;
    }

    #[test]
    fn test_find_system_font() {
        // This test requires fontconfig to be installed with fonts
        let path = LinuxGlyphRasterizer::find_font_path("sans-serif", 400, false);
        if let Some(p) = path {
            assert!(p.exists(), "Font path should exist: {:?}", p);
        }
    }

    #[test]
    fn test_rasterize_glyph() {
        let mut rasterizer = LinuxGlyphRasterizer::new();
        let font = FontDescriptor::system("sans-serif", 400, FontStyle::Normal, 16.0);
        let bitmap = rasterizer.rasterize_glyph('A', &font);

        if let Some(bitmap) = bitmap {
            assert!(bitmap.width > 0, "Glyph should have width");
            assert!(bitmap.height > 0, "Glyph should have height");
        }
    }
}
