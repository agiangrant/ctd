//! Linux font manager implementation using FreeType and fontconfig
//!
//! Uses FreeType for font loading and glyph metrics,
//! fontconfig for system font discovery.

use super::{Font, FontError, GlyphMetrics, PlatformFontManagerTrait};
use crate::text::FontStyle;
use fontconfig::{Fontconfig, Pattern, FC_FAMILY, FC_SLANT, FC_WEIGHT};
use fontconfig::{
    FC_SLANT_ITALIC, FC_SLANT_ROMAN, FC_WEIGHT_BLACK, FC_WEIGHT_BOLD, FC_WEIGHT_EXTRALIGHT,
    FC_WEIGHT_EXTRABOLD, FC_WEIGHT_LIGHT, FC_WEIGHT_MEDIUM, FC_WEIGHT_REGULAR,
    FC_WEIGHT_SEMIBOLD, FC_WEIGHT_THIN,
};
use freetype::face::LoadFlag;
use freetype::Library;
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::Mutex;

/// Thread-safe wrapper for FreeType Face
struct FreetypeFaceWrapper {
    face: freetype::Face,
}

/// Linux font implementation using FreeType
pub struct LinuxFont {
    face: Mutex<FreetypeFaceWrapper>,
    size: f32,
    ascent: f32,
    descent: f32,
    line_height: f32,
    cap_height: f32,
    x_height: f32,
}

// Manually implement Send + Sync since we're using Mutex for thread safety
unsafe impl Send for LinuxFont {}
unsafe impl Sync for LinuxFont {}

impl LinuxFont {
    /// Create from FreeType face
    fn new(face: freetype::Face, size: f32) -> Self {
        // Get metrics (values are in 26.6 fixed-point format for scalable fonts)
        let (ascent, descent, line_height) = if let Some(size_metrics) = face.size_metrics() {
            (
                size_metrics.ascender as f32 / 64.0,
                (size_metrics.descender.abs()) as f32 / 64.0,
                size_metrics.height as f32 / 64.0,
            )
        } else {
            // Fallback for bitmap fonts
            (size * 0.8, size * 0.2, size * 1.2)
        };

        // Cap height and x-height - estimate based on typical ratios
        let cap_height = ascent * 0.7;
        let x_height = ascent * 0.5;

        Self {
            face: Mutex::new(FreetypeFaceWrapper { face }),
            size,
            ascent,
            descent,
            line_height,
            cap_height,
            x_height,
        }
    }
}

impl Font for LinuxFont {
    fn glyph_metrics(&self, character: char) -> Option<GlyphMetrics> {
        let face_guard = self.face.lock().ok()?;
        let face = &face_guard.face;

        // Get glyph index
        let glyph_index = face.get_char_index(character as usize)?;

        // Load glyph (without rendering, just for metrics)
        face.load_glyph(glyph_index, LoadFlag::DEFAULT).ok()?;

        let glyph = face.glyph();
        let metrics = glyph.metrics();

        // Values are in 26.6 fixed-point format
        Some(GlyphMetrics {
            glyph_id: glyph_index,
            advance: (metrics.horiAdvance >> 6) as f32,
            width: (metrics.width >> 6) as f32,
            height: (metrics.height >> 6) as f32,
            bearing_x: (metrics.horiBearingX >> 6) as f32,
            bearing_y: (metrics.horiBearingY >> 6) as f32,
        })
    }

    fn ascent(&self) -> f32 {
        self.ascent
    }

    fn descent(&self) -> f32 {
        self.descent
    }

    fn line_height(&self) -> f32 {
        self.line_height
    }

    fn cap_height(&self) -> f32 {
        self.cap_height
    }

    fn x_height(&self) -> f32 {
        self.x_height
    }

    fn size(&self) -> f32 {
        self.size
    }
}

/// Linux font manager using FreeType and fontconfig
pub struct LinuxFontManager {
    library: Library,
}

// SAFETY: LinuxFontManager is only accessed through a Mutex in the global FONT_MANAGER
// in ffi.rs. The FreeType library is not thread-safe, but we ensure single-threaded access
// via the Mutex.
unsafe impl Send for LinuxFontManager {}
unsafe impl Sync for LinuxFontManager {}

impl LinuxFontManager {
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

        let font = pattern.font_match();
        font.filename().map(PathBuf::from)
    }
}

impl PlatformFontManagerTrait for LinuxFontManager {
    fn new() -> Self {
        let library = Library::init().expect("Failed to initialize FreeType library");
        Self { library }
    }

    fn load_system_font(
        &mut self,
        name: &str,
        weight: u16,
        style: FontStyle,
        size: f32,
    ) -> Result<Box<dyn Font>, FontError> {
        let italic = style == FontStyle::Italic;

        // Find font path using fontconfig
        let font_path = Self::find_font_path(name, weight, italic)
            .ok_or_else(|| FontError::NotFound(format!("Font '{}' not found via fontconfig", name)))?;

        // Load font with FreeType
        let face = self.library.new_face(&font_path, 0)
            .map_err(|e| FontError::LoadFailed(format!("FreeType error: {:?}", e)))?;

        // Set character size (FreeType uses 26.6 fixed-point, so multiply by 64)
        face.set_char_size(0, (size * 64.0) as isize, 72, 72)
            .map_err(|e| FontError::LoadFailed(format!("Failed to set font size: {:?}", e)))?;

        Ok(Box::new(LinuxFont::new(face, size)))
    }

    fn load_font_from_data(
        &mut self,
        data: &[u8],
        _weight: u16,
        _style: FontStyle,
        size: f32,
    ) -> Result<Box<dyn Font>, FontError> {
        let face = self.library.new_memory_face(data.to_vec(), 0)
            .map_err(|e| FontError::InvalidData(format!("FreeType error loading font data: {:?}", e)))?;

        face.set_char_size(0, (size * 64.0) as isize, 72, 72)
            .map_err(|e| FontError::LoadFailed(format!("Failed to set font size: {:?}", e)))?;

        Ok(Box::new(LinuxFont::new(face, size)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_creation() {
        let _manager = LinuxFontManager::new();
    }

    #[test]
    fn test_find_system_font() {
        let path = LinuxFontManager::find_font_path("sans-serif", 400, false);
        if let Some(p) = path {
            assert!(p.exists(), "Font path should exist: {:?}", p);
        }
    }
}
