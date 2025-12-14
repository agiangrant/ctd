//! macOS font manager implementation using Core Text
//!
//! Uses Apple's Core Text framework for font loading, glyph metrics,
//! and text shaping on macOS and iOS.

use super::{Font, FontError, GlyphMetrics};
use crate::text::FontStyle;
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_graphics::data_provider::CGDataProvider;
use core_graphics::font::CGFont;
use core_text::font::CTFont;
use core_text::font_descriptor::CTFontDescriptorRef;
use core_text::line::CTLine;
use core_text::string_attributes;
use std::sync::Arc;

// Core Text types and functions not exposed by core-text crate
type CTFontRef = *const std::ffi::c_void;
type CTFontSymbolicTraits = u32;

// Symbolic trait flags
const kCTFontTraitBold: CTFontSymbolicTraits = 1 << 1;
const kCTFontTraitItalic: CTFontSymbolicTraits = 1 << 0;

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCreateWithName(
        name: core_foundation::string::CFStringRef,
        size: f64,
        matrix: *const std::ffi::c_void,  // CGAffineTransform*, null for identity
    ) -> CTFontRef;

    fn CTFontCreateCopyWithSymbolicTraits(
        font: CTFontRef,
        size: f64,
        matrix: *const std::ffi::c_void,  // CGAffineTransform*, null for identity
        symTraitValue: CTFontSymbolicTraits,
        symTraitMask: CTFontSymbolicTraits,
    ) -> CTFontRef;

    fn CTFontCreateUIFontForLanguage(
        uiType: i32,
        size: f64,
        language: core_foundation::string::CFStringRef,
    ) -> CTFontRef;

    fn CTFontCreateCopyWithAttributes(
        font: CTFontRef,
        size: f64,
        matrix: *const std::ffi::c_void,
        attributes: core_text::font_descriptor::CTFontDescriptorRef,
    ) -> CTFontRef;
}

// System UI font types
const kCTFontUIFontSystem: i32 = 2;  // Regular system font
const kCTFontUIFontEmphasizedSystem: i32 = 3;  // Bold system font

/// macOS font implementation using Core Text
pub struct MacOSFont {
    ct_font: CTFont,
    size: f32,
}

impl MacOSFont {
    /// Create from CTFont
    fn new(ct_font: CTFont, size: f32) -> Self {
        Self { ct_font, size }
    }
}

impl Font for MacOSFont {
    fn glyph_metrics(&self, character: char) -> Option<GlyphMetrics> {
        use core_foundation::base::CFRange;

        // Create a single-character string
        let char_string = character.to_string();
        let cf_string = CFString::new(&char_string);

        // Create mutable attributed string
        let mut attr_string = CFMutableAttributedString::new();
        attr_string.replace_str(&cf_string, CFRange::init(0, 0));

        // Set font attribute on the entire string
        let string_range = CFRange::init(0, cf_string.char_len() as isize);
        unsafe {
            // kCTFontAttributeName is already a CFStringRef constant
            attr_string.set_attribute(string_range, string_attributes::kCTFontAttributeName, &self.ct_font);
        }

        // Create CTLine from attributed string
        let line = CTLine::new_with_attributed_string(attr_string.as_concrete_TypeRef());

        // Get typographic bounds
        let bounds = line.get_typographic_bounds();

        // Calculate metrics
        let advance = bounds.width as f32;
        let height = (bounds.ascent + bounds.descent) as f32;

        Some(GlyphMetrics {
            glyph_id: character as u32,
            advance,
            width: advance,  // For now, use advance as width
            height,
            bearing_x: 0.0,  // Would need CGContext for precise image bounds
            bearing_y: bounds.ascent as f32,
        })
    }

    fn ascent(&self) -> f32 {
        self.ct_font.ascent() as f32
    }

    fn descent(&self) -> f32 {
        self.ct_font.descent() as f32
    }

    fn line_height(&self) -> f32 {
        (self.ct_font.ascent() + self.ct_font.descent() + self.ct_font.leading()) as f32
    }

    fn cap_height(&self) -> f32 {
        self.ct_font.cap_height() as f32
    }

    fn x_height(&self) -> f32 {
        self.ct_font.x_height() as f32
    }

    fn size(&self) -> f32 {
        self.size
    }
}

/// macOS font manager using Core Text
pub struct MacOSFontManager;

impl MacOSFontManager {
    pub fn new() -> Self {
        Self
    }

    /// Get the font name for a given weight
    /// San Francisco (system font) has named variants for different weights
    fn get_system_font_name_for_weight(weight: u16) -> &'static str {
        // San Francisco system font weight variants
        match weight {
            0..=149 => ".AppleSystemUIFontUltraLight",
            150..=249 => ".AppleSystemUIFontThin",
            250..=349 => ".AppleSystemUIFontLight",
            350..=449 => ".AppleSystemUIFont",  // Regular
            450..=549 => ".AppleSystemUIFontMedium",
            550..=649 => ".AppleSystemUIFontDemi",  // Semibold
            650..=749 => ".AppleSystemUIFontBold",
            750..=849 => ".AppleSystemUIFontHeavy",
            _ => ".AppleSystemUIFontBlack",  // 850+
        }
    }

    /// Load a system font by name with weight and style
    pub fn load_system_font(
        &mut self,
        name: &str,
        weight: u16,
        style: FontStyle,
        size: f32,
    ) -> Result<Box<dyn Font>, FontError> {
        let ct_font = unsafe {
            let base_font_ref: CTFontRef = if name == "system" || name.is_empty() {
                // For system font, use the weight-specific variant names
                let weight_name = Self::get_system_font_name_for_weight(weight);
                let cf_name = CFString::new(weight_name);
                CTFontCreateWithName(
                    cf_name.as_concrete_TypeRef(),
                    size as f64,
                    std::ptr::null(),
                )
            } else {
                // For named fonts, create base font then apply bold trait if needed
                let cf_name = CFString::new(name);
                let base_ref = CTFontCreateWithName(
                    cf_name.as_concrete_TypeRef(),
                    size as f64,
                    std::ptr::null(),
                );

                // Apply bold trait if weight >= 600
                if weight >= 600 && !base_ref.is_null() {
                    let bold_ref = CTFontCreateCopyWithSymbolicTraits(
                        base_ref,
                        size as f64,
                        std::ptr::null(),
                        kCTFontTraitBold,
                        kCTFontTraitBold,
                    );
                    if !bold_ref.is_null() {
                        // Release the base font since we have a new one
                        core_foundation::base::CFRelease(base_ref as *const _);
                        bold_ref
                    } else {
                        base_ref  // Bold variant not available, use base
                    }
                } else {
                    base_ref
                }
            };

            if base_font_ref.is_null() {
                return Err(FontError::NotFound(format!("Font '{}' not found", name)));
            }

            // Apply italic trait if requested
            let final_font_ref = if style == FontStyle::Italic {
                let italic_ref = CTFontCreateCopyWithSymbolicTraits(
                    base_font_ref,
                    size as f64,
                    std::ptr::null(),
                    kCTFontTraitItalic,
                    kCTFontTraitItalic,
                );
                if !italic_ref.is_null() {
                    core_foundation::base::CFRelease(base_font_ref as *const _);
                    italic_ref
                } else {
                    base_font_ref  // Italic variant not available, use base
                }
            } else {
                base_font_ref
            };

            // Wrap in CTFont - we need to use wrap_under_create_rule since we own the reference
            CTFont::wrap_under_create_rule(final_font_ref as core_text::font::CTFontRef)
        };

        Ok(Box::new(MacOSFont::new(ct_font, size)))
    }

    /// Load a font from data (TTF/OTF file)
    pub fn load_font_from_data(
        &mut self,
        data: &[u8],
        _weight: u16,
        _style: FontStyle,
        size: f32,
    ) -> Result<Box<dyn Font>, FontError> {
        // Create CGDataProvider directly from data
        // CGDataProvider::from_buffer requires Arc<T: AsRef<[u8]>>
        let data_vec = data.to_vec();
        let data_provider = CGDataProvider::from_buffer(Arc::new(data_vec));

        // Create CGFont from data provider
        let cg_font = CGFont::from_data_provider(data_provider)
            .map_err(|_| FontError::InvalidData("Failed to create CGFont from data".to_string()))?;

        // Create CTFont from CGFont
        let ct_font = core_text::font::new_from_CGFont(&cg_font, size as f64);

        Ok(Box::new(MacOSFont::new(ct_font, size)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_system_font() {
        let mut manager = MacOSFontManager::new();

        // Load Helvetica
        let result = manager.load_system_font("Helvetica", 400, FontStyle::Normal, 16.0);
        assert!(result.is_ok(), "Failed to load Helvetica: {:?}", result.err());

        let font = result.unwrap();
        assert_eq!(font.size(), 16.0);
        assert!(font.ascent() > 0.0);
        assert!(font.descent() > 0.0);
    }

    #[test]
    fn test_glyph_metrics() {
        let mut manager = MacOSFontManager::new();
        let font = manager
            .load_system_font("Helvetica", 400, FontStyle::Normal, 16.0)
            .unwrap();

        // Get metrics for 'A'
        let metrics = font.glyph_metrics('A');
        assert!(metrics.is_some());

        let m = metrics.unwrap();
        assert!(m.advance > 0.0);
        assert!(m.width > 0.0);
        assert!(m.height > 0.0);

        println!("Glyph 'A' metrics: {:?}", m);
    }

    #[test]
    fn test_measure_text() {
        let mut manager = MacOSFontManager::new();
        let font = manager
            .load_system_font("Helvetica", 400, FontStyle::Normal, 16.0)
            .unwrap();

        let width = font.measure_text("Hello");
        assert!(width > 0.0);
        println!("'Hello' width: {}", width);

        // "Hello" should be wider than "Hi"
        let width2 = font.measure_text("Hi");
        assert!(width > width2);
    }
}
