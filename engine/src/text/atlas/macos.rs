//! macOS glyph rasterization using Core Graphics
//!
//! Renders glyphs to RGBA bitmaps using CGContext and Core Text.
//! Supports font weights and styles via Core Text font creation APIs.

use super::{GlyphBitmap, GlyphRasterizer};
use crate::text::{FontDescriptor, FontSource, FontStyle};
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::{CFRange, TCFType};
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_graphics::base::CGFloat;
use core_graphics::color::CGColor;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_text::font::CTFont;
use core_text::line::CTLine;
use core_text::string_attributes;
use std::collections::HashMap;
use std::path::Path;

// Core Text types and functions not exposed by core-text crate
type CTFontRef = *const std::ffi::c_void;
type CTFontSymbolicTraits = u32;
type CGFontRef = *const std::ffi::c_void;
type CFURLRef = *const std::ffi::c_void;
type CGDataProviderRef = *const std::ffi::c_void;

// Symbolic trait flags
const KCTFONT_TRAIT_BOLD: CTFontSymbolicTraits = 1 << 1;
const KCTFONT_TRAIT_ITALIC: CTFontSymbolicTraits = 1 << 0;

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCreateWithName(
        name: core_foundation::string::CFStringRef,
        size: f64,
        matrix: *const std::ffi::c_void,
    ) -> CTFontRef;

    fn CTFontCreateCopyWithSymbolicTraits(
        font: CTFontRef,
        size: f64,
        matrix: *const std::ffi::c_void,
        symTraitValue: CTFontSymbolicTraits,
        symTraitMask: CTFontSymbolicTraits,
    ) -> CTFontRef;

    fn CTFontCreateWithGraphicsFont(
        graphicsFont: CGFontRef,
        size: f64,
        matrix: *const std::ffi::c_void,
        attributes: *const std::ffi::c_void,
    ) -> CTFontRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGFontCreateWithDataProvider(provider: CGDataProviderRef) -> CGFontRef;
    fn CGDataProviderCreateWithURL(url: CFURLRef) -> CGDataProviderRef;
    fn CGDataProviderRelease(provider: CGDataProviderRef);
    fn CGFontRelease(font: CGFontRef);
}

/// Wrapper for CGFontRef that implements Send + Sync
/// CGFont is thread-safe on macOS (it's immutable once created)
struct SendableCGFont(CGFontRef);

// SAFETY: CGFont objects are immutable and thread-safe on macOS
unsafe impl Send for SendableCGFont {}
unsafe impl Sync for SendableCGFont {}

/// macOS glyph rasterizer using Core Graphics
pub struct MacOSGlyphRasterizer {
    /// Cache of loaded CGFonts from file paths (path -> CGFontRef)
    /// We keep raw pointers since CGFont doesn't have a safe wrapper
    loaded_fonts: HashMap<String, SendableCGFont>,
}

impl MacOSGlyphRasterizer {
    pub fn new() -> Self {
        Self {
            loaded_fonts: HashMap::new(),
        }
    }

    /// Load a CGFont from a file path
    fn load_font_from_file(&mut self, path: &str) -> Option<CGFontRef> {
        // Check cache first
        if let Some(font) = self.loaded_fonts.get(path) {
            return Some(font.0);
        }

        // Try to resolve the path - first check if it's absolute or relative
        let resolved_path = if Path::new(path).is_absolute() {
            path.to_string()
        } else {
            // Try relative to current working directory
            if let Ok(cwd) = std::env::current_dir() {
                let full_path = cwd.join(path);
                if full_path.exists() {
                    full_path.to_string_lossy().to_string()
                } else {
                    // Try relative to executable directory
                    if let Ok(exe_path) = std::env::current_exe() {
                        if let Some(exe_dir) = exe_path.parent() {
                            let exe_relative = exe_dir.join(path);
                            if exe_relative.exists() {
                                exe_relative.to_string_lossy().to_string()
                            } else {
                                eprintln!("Font file not found: {} (tried {} and {})",
                                    path,
                                    full_path.display(),
                                    exe_relative.display());
                                return None;
                            }
                        } else {
                            eprintln!("Font file not found: {}", full_path.display());
                            return None;
                        }
                    } else {
                        eprintln!("Font file not found: {}", full_path.display());
                        return None;
                    }
                }
            } else {
                path.to_string()
            }
        };

        // Create URL from file path
        let url = CFURL::from_path(&Path::new(&resolved_path), false)?;

        unsafe {
            // Create data provider from URL
            let provider = CGDataProviderCreateWithURL(url.as_concrete_TypeRef() as CFURLRef);
            if provider.is_null() {
                eprintln!("Failed to create data provider for font: {}", resolved_path);
                return None;
            }

            // Create CGFont from data provider
            let cg_font = CGFontCreateWithDataProvider(provider);
            CGDataProviderRelease(provider);

            if cg_font.is_null() {
                eprintln!("Failed to create CGFont from file: {}", resolved_path);
                return None;
            }

            // Cache the font
            self.loaded_fonts.insert(path.to_string(), SendableCGFont(cg_font));
            Some(cg_font)
        }
    }

    /// Get the font name for a given weight (for system fonts)
    /// San Francisco (system font) has named variants for different weights
    fn get_system_font_name_for_weight(weight: u16) -> &'static str {
        // San Francisco system font weight variants
        match weight {
            0..=149 => ".AppleSystemUIFontUltraLight",
            150..=249 => ".AppleSystemUIFontThin",
            250..=349 => ".AppleSystemUIFontLight",
            350..=449 => ".AppleSystemUIFont",        // Regular
            450..=549 => ".AppleSystemUIFontMedium",
            550..=649 => ".AppleSystemUIFontDemi",    // Semibold
            650..=749 => ".AppleSystemUIFontBold",
            750..=849 => ".AppleSystemUIFontHeavy",
            _ => ".AppleSystemUIFontBlack",           // 850+
        }
    }

    /// Create a CTFont with the specified weight and style
    fn create_font(&mut self, font: &FontDescriptor) -> Option<CTFont> {
        let size = font.size as f64;

        // Handle bundled fonts specially - load from file
        if let FontSource::Bundled(path) = &font.source {
            let cg_font = self.load_font_from_file(path)?;

            unsafe {
                // Create CTFont from CGFont
                let ct_font_ref = CTFontCreateWithGraphicsFont(
                    cg_font,
                    size,
                    std::ptr::null(),
                    std::ptr::null(),
                );

                if ct_font_ref.is_null() {
                    eprintln!("Failed to create CTFont from CGFont for: {}", path);
                    return None;
                }

                return Some(CTFont::wrap_under_create_rule(
                    ct_font_ref as core_text::font::CTFontRef
                ));
            }
        }

        // Get font name from source (system or memory fonts)
        let font_name = match &font.source {
            FontSource::System(name) => {
                if name == "system" || name.is_empty() {
                    // Use weight-specific system font variant
                    Self::get_system_font_name_for_weight(font.weight).to_string()
                } else {
                    name.clone()
                }
            }
            FontSource::Bundled(_) => unreachable!(), // Handled above
            FontSource::Memory { name, .. } => name.clone(),
        };

        unsafe {
            // Create base font
            let cf_name = CFString::new(&font_name);
            let base_font_ref = CTFontCreateWithName(
                cf_name.as_concrete_TypeRef(),
                size,
                std::ptr::null(),
            );

            if base_font_ref.is_null() {
                // Fall back to system font for the weight
                let fallback_name = Self::get_system_font_name_for_weight(font.weight);
                let cf_fallback = CFString::new(fallback_name);
                let fallback_ref = CTFontCreateWithName(
                    cf_fallback.as_concrete_TypeRef(),
                    size,
                    std::ptr::null(),
                );
                if fallback_ref.is_null() {
                    return None;
                }
                return Some(CTFont::wrap_under_create_rule(
                    fallback_ref as core_text::font::CTFontRef
                ));
            }

            // For non-system fonts with bold weight, apply bold trait
            let font_after_weight = if font_name.starts_with(".AppleSystemUIFont") {
                // System fonts already have weight built in
                base_font_ref
            } else if font.weight >= 600 {
                // Apply bold trait for named fonts
                let bold_ref = CTFontCreateCopyWithSymbolicTraits(
                    base_font_ref,
                    size,
                    std::ptr::null(),
                    KCTFONT_TRAIT_BOLD,
                    KCTFONT_TRAIT_BOLD,
                );
                if !bold_ref.is_null() {
                    core_foundation::base::CFRelease(base_font_ref as *const _);
                    bold_ref
                } else {
                    base_font_ref
                }
            } else {
                base_font_ref
            };

            // Apply italic if requested
            let final_font_ref = if font.style == FontStyle::Italic {
                let italic_ref = CTFontCreateCopyWithSymbolicTraits(
                    font_after_weight,
                    size,
                    std::ptr::null(),
                    KCTFONT_TRAIT_ITALIC,
                    KCTFONT_TRAIT_ITALIC,
                );
                if !italic_ref.is_null() {
                    core_foundation::base::CFRelease(font_after_weight as *const _);
                    italic_ref
                } else {
                    font_after_weight
                }
            } else {
                font_after_weight
            };

            Some(CTFont::wrap_under_create_rule(
                final_font_ref as core_text::font::CTFontRef
            ))
        }
    }

    /// Create an attributed string with the font and white foreground color
    fn create_attributed_string(&self, text: &str, ct_font: &CTFont) -> CFMutableAttributedString {
        let cf_string = CFString::new(text);
        let mut attr_string = CFMutableAttributedString::new();
        attr_string.replace_str(&cf_string, CFRange::init(0, 0));

        let string_range = CFRange::init(0, cf_string.char_len() as isize);

        // Create white color for the text
        let white_color = CGColor::rgb(1.0, 1.0, 1.0, 1.0);

        unsafe {
            // Set font attribute
            attr_string.set_attribute(
                string_range,
                string_attributes::kCTFontAttributeName,
                ct_font,
            );

            // Set foreground color attribute to white
            // This is critical - Core Text ignores CGContext fill color
            attr_string.set_attribute(
                string_range,
                string_attributes::kCTForegroundColorAttributeName,
                &white_color,
            );
        }

        attr_string
    }

    /// Measure the width of a string using CTLine (fast path, no rasterization)
    /// Returns the width in pixels at the given font size
    pub fn measure_string(&mut self, text: &str, font: &FontDescriptor) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        // Create font with proper weight and style
        let ct_font = match self.create_font(font) {
            Some(f) => f,
            None => return 0.0,
        };

        // Create attributed string for the text
        let attr_string = self.create_attributed_string(text, &ct_font);

        // Create CTLine and get typographic bounds
        let line = CTLine::new_with_attributed_string(attr_string.as_concrete_TypeRef());
        let bounds = line.get_typographic_bounds();

        bounds.width as f32
    }
}

impl GlyphRasterizer for MacOSGlyphRasterizer {
    fn rasterize_glyph(
        &mut self,
        character: char,
        font: &FontDescriptor,
    ) -> Option<GlyphBitmap> {
        // Create font with proper weight and style
        let ct_font = self.create_font(font)?;

        // Handle whitespace characters - no visual glyph needed
        if character.is_whitespace() {
            let char_string = character.to_string();
            let attr_string = self.create_attributed_string(&char_string, &ct_font);
            let line = CTLine::new_with_attributed_string(attr_string.as_concrete_TypeRef());
            let bounds = line.get_typographic_bounds();

            return Some(GlyphBitmap {
                data: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance: bounds.width as f32,
            });
        }

        // Create attributed string for this character
        let char_string = character.to_string();
        let attr_string = self.create_attributed_string(&char_string, &ct_font);

        // Create CTLine
        let line = CTLine::new_with_attributed_string(attr_string.as_concrete_TypeRef());

        // Get typographic bounds
        let bounds = line.get_typographic_bounds();
        let width = bounds.width.ceil() as u32;
        let height = (bounds.ascent + bounds.descent).ceil() as u32;

        // Add padding for proper rendering
        let padding = 2;
        let bitmap_width = width + padding * 2;
        let bitmap_height = height + padding * 2;

        if bitmap_width == 0 || bitmap_height == 0 {
            // Empty glyph (shouldn't happen after whitespace check, but handle it)
            return Some(GlyphBitmap {
                data: Vec::new(),
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance: bounds.width as f32,
            });
        }

        // Create bitmap context (RGBA, premultiplied alpha)
        let color_space = CGColorSpace::create_device_rgb();
        let mut data = vec![0u8; (bitmap_width * bitmap_height * 4) as usize];

        let context = CGContext::create_bitmap_context(
            Some(data.as_mut_ptr() as *mut _),
            bitmap_width as usize,
            bitmap_height as usize,
            8,                                  // bits per component
            (bitmap_width * 4) as usize,       // bytes per row
            &color_space,
            core_graphics::base::kCGImageAlphaPremultipliedLast,
        );

        // Set text drawing mode
        context.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0); // White text
        context.set_text_drawing_mode(core_graphics::context::CGTextDrawingMode::CGTextFill);

        // Position and draw the line
        // Origin is bottom-left in Core Graphics, so we need to flip
        let x = padding as CGFloat;
        let y = (padding as CGFloat) + bounds.descent;

        context.set_text_position(x, y);
        line.draw(&context);

        // Get the rendered bitmap data
        // Note: Core Graphics uses bottom-left origin, which matches wgpu's texture coordinate system
        // So we DON'T flip - use the data as-is
        let flipped_data = data;  // Not actually flipped anymore!

        // DEBUG: Check if the rasterized glyph has any non-zero alpha
        #[cfg(debug_assertions)]
        {
            let non_zero_pixels = flipped_data.chunks(4).filter(|chunk| chunk[3] > 0).count();
            if character == 'T' {  // Only log for first glyph
                eprintln!("🎨 Rasterized '{}': {}x{}, non-zero pixels: {}, first_pixel: RGBA({},{},{},{})",
                    character, bitmap_width, bitmap_height, non_zero_pixels,
                    flipped_data[0], flipped_data[1], flipped_data[2], flipped_data[3]);

                // Sample specific pixels to match atlas check positions
                // Position (10, 20) and (17, 5)
                let pos1 = ((20 * bitmap_width + 10) * 4) as usize;
                let pos2 = ((5 * bitmap_width + 17) * 4) as usize;

                if pos1 + 3 < flipped_data.len() {
                    eprintln!("   Source at (10, 20): RGBA({},{},{},{}) - alpha={}",
                        flipped_data[pos1], flipped_data[pos1+1],
                        flipped_data[pos1+2], flipped_data[pos1+3], flipped_data[pos1+3]);
                }
                if pos2 + 3 < flipped_data.len() {
                    eprintln!("   Source at (17, 5): RGBA({},{},{},{}) - alpha={}",
                        flipped_data[pos2], flipped_data[pos2+1],
                        flipped_data[pos2+2], flipped_data[pos2+3], flipped_data[pos2+3]);
                }

                // Also find the first non-zero pixel to see where it is
                for (i, chunk) in flipped_data.chunks(4).enumerate() {
                    if chunk[3] > 0 {
                        let x = (i as u32) % bitmap_width;
                        let y = (i as u32) / bitmap_width;
                        eprintln!("   FIRST non-zero pixel at ({}, {}): RGBA({},{},{},{}) - alpha={}",
                            x, y, chunk[0], chunk[1], chunk[2], chunk[3], chunk[3]);
                        break;
                    }
                }
            }
        }

        // After flipping the bitmap vertically, the bearing_y remains the same
        // because it represents the distance from baseline to the top of the glyph,
        // and flipping doesn't change that relationship in our coordinate system

        Some(GlyphBitmap {
            data: flipped_data,
            width: bitmap_width,
            height: bitmap_height,
            bearing_x: -(padding as f32),
            bearing_y: bounds.ascent as f32 + padding as f32,
            advance: bounds.width as f32,
        })
    }
}

/// Flip a bitmap vertically (Core Graphics uses bottom-left origin)
fn flip_bitmap_vertically(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let row_bytes = (width * 4) as usize;
    let mut flipped = vec![0u8; data.len()];

    for y in 0..height {
        let src_row = ((height - 1 - y) * width * 4) as usize;
        let dst_row = (y * width * 4) as usize;
        flipped[dst_row..dst_row + row_bytes].copy_from_slice(&data[src_row..src_row + row_bytes]);
    }

    flipped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterizer_creation() {
        let rasterizer = MacOSGlyphRasterizer::new();
        // Just verify it can be created
        let _ = rasterizer;
    }

    #[test]
    fn test_rasterize_glyph() {
        let mut rasterizer = MacOSGlyphRasterizer::new();
        let font = FontDescriptor::system("San Francisco", 400, FontStyle::Normal, 16.0);
        let bitmap = rasterizer.rasterize_glyph('A', &font);

        assert!(bitmap.is_some());
        let bitmap = bitmap.unwrap();

        // Should have non-zero dimensions
        assert!(bitmap.width > 0);
        assert!(bitmap.height > 0);

        // Data should be RGBA (4 bytes per pixel)
        assert_eq!(bitmap.data.len(), (bitmap.width * bitmap.height * 4) as usize);
    }

    #[test]
    fn test_rasterize_whitespace() {
        let mut rasterizer = MacOSGlyphRasterizer::new();
        let font = FontDescriptor::system("San Francisco", 400, FontStyle::Normal, 16.0);
        let bitmap = rasterizer.rasterize_glyph(' ', &font);

        assert!(bitmap.is_some());
        let bitmap = bitmap.unwrap();

        // Whitespace should have zero dimensions
        assert_eq!(bitmap.width, 0);
        assert_eq!(bitmap.height, 0);
    }

    #[test]
    fn test_rasterize_different_weights() {
        let mut rasterizer = MacOSGlyphRasterizer::new();

        // Regular weight
        let regular = FontDescriptor::system("system", 400, FontStyle::Normal, 16.0);
        let regular_bitmap = rasterizer.rasterize_glyph('A', &regular);
        assert!(regular_bitmap.is_some());

        // Bold weight
        let bold = FontDescriptor::system("system", 700, FontStyle::Normal, 16.0);
        let bold_bitmap = rasterizer.rasterize_glyph('A', &bold);
        assert!(bold_bitmap.is_some());

        // Bold should produce a different (usually wider) glyph
        let regular_b = regular_bitmap.unwrap();
        let bold_b = bold_bitmap.unwrap();

        // At minimum, the glyphs should render successfully
        assert!(regular_b.width > 0);
        assert!(bold_b.width > 0);
    }

    #[test]
    fn test_flip_bitmap() {
        let data = vec![
            1, 2, 3, 4,  5, 6, 7, 8,    // Row 0
            9, 10, 11, 12,  13, 14, 15, 16,  // Row 1
        ];

        let flipped = flip_bitmap_vertically(&data, 2, 2);

        // Row 0 should become row 1
        assert_eq!(&flipped[0..8], &[9, 10, 11, 12, 13, 14, 15, 16]);
        // Row 1 should become row 0
        assert_eq!(&flipped[8..16], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
