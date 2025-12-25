//! Windows font manager implementation using DirectWrite
//!
//! Uses Microsoft's DirectWrite framework for font loading, glyph metrics,
//! and text shaping on Windows.

use super::{Font, FontError, GlyphMetrics, PlatformFontManagerTrait};
use crate::text::FontStyle;
use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::System::Com::*;

/// Global COM initialization flag
static COM_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Ensure COM is initialized (required for DirectWrite)
fn ensure_com_initialized() {
    COM_INITIALIZED.get_or_init(|| {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        true
    });
}

/// Map CSS font weight (100-900) to DirectWrite font weight
fn map_weight_to_dwrite(weight: u16) -> DWRITE_FONT_WEIGHT {
    DWRITE_FONT_WEIGHT(match weight {
        0..=149 => 100,   // Thin
        150..=249 => 200, // Extra Light
        250..=349 => 300, // Light
        350..=449 => 400, // Regular
        450..=549 => 500, // Medium
        550..=649 => 600, // Semi Bold
        650..=749 => 700, // Bold
        750..=849 => 800, // Extra Bold
        _ => 900,         // Black
    })
}

/// Windows font implementation using DirectWrite
pub struct WindowsFont {
    factory: IDWriteFactory,
    text_format: IDWriteTextFormat,
    font_metrics: DWRITE_FONT_METRICS,
    size: f32,
}

// SAFETY: DirectWrite objects are thread-safe
unsafe impl Send for WindowsFont {}
unsafe impl Sync for WindowsFont {}

impl WindowsFont {
    fn new(
        factory: IDWriteFactory,
        text_format: IDWriteTextFormat,
        font_metrics: DWRITE_FONT_METRICS,
        size: f32,
    ) -> Self {
        Self {
            factory,
            text_format,
            font_metrics,
            size,
        }
    }

    /// Get the design units to pixels scale factor
    fn design_units_to_pixels(&self) -> f32 {
        self.size / self.font_metrics.designUnitsPerEm as f32
    }
}

impl Font for WindowsFont {
    fn glyph_metrics(&self, character: char) -> Option<GlyphMetrics> {
        unsafe {
            // Create a text layout for the character
            let text: Vec<u16> = character.to_string().encode_utf16().collect();

            let layout: IDWriteTextLayout = self.factory.CreateTextLayout(
                &text,
                &self.text_format,
                10000.0,
                10000.0,
            ).ok()?;

            // Get text metrics
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;

            // Get cluster metrics for advance
            let mut cluster_count = 0u32;
            let _ = layout.GetClusterMetrics(None, &mut cluster_count);

            let advance = if cluster_count > 0 {
                let mut clusters = vec![DWRITE_CLUSTER_METRICS::default(); cluster_count as usize];
                if layout.GetClusterMetrics(Some(&mut clusters), &mut cluster_count).is_ok() {
                    clusters.first().map(|c| c.width).unwrap_or(metrics.width)
                } else {
                    metrics.width
                }
            } else {
                metrics.width
            };

            Some(GlyphMetrics {
                glyph_id: character as u32,
                advance,
                width: metrics.width,
                height: metrics.height,
                bearing_x: 0.0,
                bearing_y: self.ascent(),
            })
        }
    }

    fn ascent(&self) -> f32 {
        self.font_metrics.ascent as f32 * self.design_units_to_pixels()
    }

    fn descent(&self) -> f32 {
        self.font_metrics.descent as f32 * self.design_units_to_pixels()
    }

    fn line_height(&self) -> f32 {
        let scale = self.design_units_to_pixels();
        (self.font_metrics.ascent as f32
            + self.font_metrics.descent as f32
            + self.font_metrics.lineGap as f32)
            * scale
    }

    fn cap_height(&self) -> f32 {
        self.font_metrics.capHeight as f32 * self.design_units_to_pixels()
    }

    fn x_height(&self) -> f32 {
        self.font_metrics.xHeight as f32 * self.design_units_to_pixels()
    }

    fn size(&self) -> f32 {
        self.size
    }

    fn measure_text(&self, text: &str) -> f32 {
        if text.is_empty() {
            return 0.0;
        }

        unsafe {
            let wide_text: Vec<u16> = text.encode_utf16().collect();

            let layout: IDWriteTextLayout = match self.factory.CreateTextLayout(
                &wide_text,
                &self.text_format,
                10000.0,
                10000.0,
            ) {
                Ok(l) => l,
                Err(_) => return 0.0,
            };

            let mut metrics = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut metrics).is_ok() {
                metrics.width
            } else {
                0.0
            }
        }
    }
}

/// Windows font manager using DirectWrite
pub struct WindowsFontManager {
    factory: IDWriteFactory,
}

impl WindowsFontManager {
    /// Get font metrics from a font face
    fn get_font_metrics(
        &self,
        family_name: &str,
        weight: DWRITE_FONT_WEIGHT,
        style: DWRITE_FONT_STYLE,
    ) -> std::result::Result<DWRITE_FONT_METRICS, FontError> {
        unsafe {
            // Get system font collection
            let mut collection: Option<IDWriteFontCollection> = None;
            self.factory
                .GetSystemFontCollection(&mut collection as *mut _, false)
                .map_err(|e| FontError::PlatformError(format!("Failed to get font collection: {:?}", e)))?;
            let collection = collection.ok_or_else(|| FontError::PlatformError("No font collection returned".to_string()))?;

            // Find font family
            let wide_name: Vec<u16> = family_name.encode_utf16().chain(std::iter::once(0)).collect();
            let mut index = 0u32;
            let mut exists = BOOL::default();

            collection.FindFamilyName(
                PCWSTR::from_raw(wide_name.as_ptr()),
                &mut index,
                &mut exists,
            ).map_err(|e| FontError::PlatformError(format!("Failed to find font family: {:?}", e)))?;

            if !exists.as_bool() {
                // Try fallback to Segoe UI
                let fallback: Vec<u16> = "Segoe UI".encode_utf16().chain(std::iter::once(0)).collect();
                collection.FindFamilyName(
                    PCWSTR::from_raw(fallback.as_ptr()),
                    &mut index,
                    &mut exists,
                ).map_err(|e| FontError::PlatformError(format!("Failed to find fallback font: {:?}", e)))?;

                if !exists.as_bool() {
                    return Err(FontError::NotFound(format!("Font '{}' not found", family_name)));
                }
            }

            // Get font family
            let family: IDWriteFontFamily = collection
                .GetFontFamily(index)
                .map_err(|e| FontError::PlatformError(format!("Failed to get font family: {:?}", e)))?;

            // Get matching font
            let font: IDWriteFont = family
                .GetFirstMatchingFont(weight, DWRITE_FONT_STRETCH_NORMAL, style)
                .map_err(|e| FontError::PlatformError(format!("Failed to get matching font: {:?}", e)))?;

            // Get font metrics
            let mut metrics = DWRITE_FONT_METRICS::default();
            font.GetMetrics(&mut metrics);

            Ok(metrics)
        }
    }
}

impl PlatformFontManagerTrait for WindowsFontManager {
    fn new() -> Self {
        ensure_com_initialized();

        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .expect("Failed to create DirectWrite factory");

            Self { factory }
        }
    }

    fn load_system_font(
        &mut self,
        name: &str,
        weight: u16,
        style: FontStyle,
        size: f32,
    ) -> std::result::Result<Box<dyn Font>, FontError> {
        let family_name = if name == "system" || name.is_empty() {
            "Segoe UI" // Windows system UI font
        } else {
            name
        };

        let dwrite_weight = map_weight_to_dwrite(weight);
        let dwrite_style = if style == FontStyle::Italic {
            DWRITE_FONT_STYLE_ITALIC
        } else {
            DWRITE_FONT_STYLE_NORMAL
        };

        // Get font metrics
        let font_metrics = self.get_font_metrics(family_name, dwrite_weight, dwrite_style)?;

        unsafe {
            let wide_name: Vec<u16> = family_name.encode_utf16().chain(std::iter::once(0)).collect();
            let locale: Vec<u16> = "en-us".encode_utf16().chain(std::iter::once(0)).collect();

            // Create text format
            let text_format: IDWriteTextFormat = self.factory.CreateTextFormat(
                PCWSTR::from_raw(wide_name.as_ptr()),
                None,
                dwrite_weight,
                dwrite_style,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR::from_raw(locale.as_ptr()),
            ).map_err(|e| {
                // Try fallback
                FontError::PlatformError(format!("Failed to create text format for '{}': {:?}", family_name, e))
            })?;

            Ok(Box::new(WindowsFont::new(
                self.factory.clone(),
                text_format,
                font_metrics,
                size,
            )))
        }
    }

    fn load_font_from_data(
        &mut self,
        data: &[u8],
        weight: u16,
        style: FontStyle,
        size: f32,
    ) -> std::result::Result<Box<dyn Font>, FontError> {
        // For now, load font from data is more complex and requires
        // custom font collection loader. Fall back to system font.
        // TODO: Implement custom font loader using IDWriteFontCollectionLoader

        eprintln!("Warning: load_font_from_data not fully implemented on Windows, using fallback");

        let dwrite_weight = map_weight_to_dwrite(weight);
        let dwrite_style = if style == FontStyle::Italic {
            DWRITE_FONT_STYLE_ITALIC
        } else {
            DWRITE_FONT_STYLE_NORMAL
        };

        // Get metrics for Segoe UI as fallback
        let font_metrics = self.get_font_metrics("Segoe UI", dwrite_weight, dwrite_style)?;

        unsafe {
            let name: Vec<u16> = "Segoe UI".encode_utf16().chain(std::iter::once(0)).collect();
            let locale: Vec<u16> = "en-us".encode_utf16().chain(std::iter::once(0)).collect();

            let text_format: IDWriteTextFormat = self.factory.CreateTextFormat(
                PCWSTR::from_raw(name.as_ptr()),
                None,
                dwrite_weight,
                dwrite_style,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                PCWSTR::from_raw(locale.as_ptr()),
            ).map_err(|e| FontError::PlatformError(format!("Failed to create fallback text format: {:?}", e)))?;

            Ok(Box::new(WindowsFont::new(
                self.factory.clone(),
                text_format,
                font_metrics,
                size,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_system_font() {
        let mut manager = WindowsFontManager::new();

        // Load Segoe UI
        let result = manager.load_system_font("Segoe UI", 400, FontStyle::Normal, 16.0);
        assert!(result.is_ok(), "Failed to load Segoe UI: {:?}", result.err());

        let font = result.unwrap();
        assert_eq!(font.size(), 16.0);
        assert!(font.ascent() > 0.0);
        assert!(font.descent() > 0.0);
    }

    #[test]
    fn test_load_system_font_default() {
        let mut manager = WindowsFontManager::new();

        // Load system default
        let result = manager.load_system_font("system", 400, FontStyle::Normal, 16.0);
        assert!(result.is_ok(), "Failed to load system font: {:?}", result.err());
    }

    #[test]
    fn test_glyph_metrics() {
        let mut manager = WindowsFontManager::new();
        let font = manager
            .load_system_font("Segoe UI", 400, FontStyle::Normal, 16.0)
            .unwrap();

        // Get metrics for 'A'
        let metrics = font.glyph_metrics('A');
        assert!(metrics.is_some());

        let m = metrics.unwrap();
        assert!(m.advance > 0.0);
        assert!(m.width > 0.0);
        assert!(m.height > 0.0);
    }

    #[test]
    fn test_measure_text() {
        let mut manager = WindowsFontManager::new();
        let font = manager
            .load_system_font("Segoe UI", 400, FontStyle::Normal, 16.0)
            .unwrap();

        let width = font.measure_text("Hello");
        assert!(width > 0.0);

        // "Hello" should be wider than "Hi"
        let width2 = font.measure_text("Hi");
        assert!(width > width2);
    }

    #[test]
    fn test_font_weights() {
        let mut manager = WindowsFontManager::new();

        // Regular weight
        let regular = manager.load_system_font("Segoe UI", 400, FontStyle::Normal, 16.0);
        assert!(regular.is_ok());

        // Bold weight
        let bold = manager.load_system_font("Segoe UI", 700, FontStyle::Normal, 16.0);
        assert!(bold.is_ok());
    }
}
