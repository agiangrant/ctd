//! Windows text shaper using DirectWrite
//!
//! Uses IDWriteTextLayout for text layout and shaping.

use super::{ShapedGlyph, ShapedLine, ShapedText, ShaperError, TextShaper};
use crate::text::font_manager::Font;
use crate::text::{TextAlign, TextLayoutConfig, WordBreak};
use std::sync::OnceLock;

use windows::core::PCWSTR;
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

/// Windows text shaper using DirectWrite
pub struct WindowsTextShaper {
    factory: IDWriteFactory,
}

impl WindowsTextShaper {
    pub fn new() -> Self {
        ensure_com_initialized();

        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .expect("Failed to create DirectWrite factory");

            Self { factory }
        }
    }

    /// Create a text format for shaping
    fn create_text_format(&self, font: &dyn Font) -> Option<IDWriteTextFormat> {
        unsafe {
            let name: Vec<u16> = "Segoe UI".encode_utf16().chain(std::iter::once(0)).collect();
            let locale: Vec<u16> = "en-us".encode_utf16().chain(std::iter::once(0)).collect();

            self.factory.CreateTextFormat(
                PCWSTR::from_raw(name.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT(400), // Regular
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                font.size(),
                PCWSTR::from_raw(locale.as_ptr()),
            ).ok()
        }
    }

    /// Create a text layout for the given text
    fn create_text_layout(
        &self,
        text: &str,
        text_format: &IDWriteTextFormat,
        max_width: f32,
        max_height: f32,
    ) -> Option<IDWriteTextLayout> {
        unsafe {
            let wide_text: Vec<u16> = text.encode_utf16().collect();

            self.factory.CreateTextLayout(
                &wide_text,
                text_format,
                max_width,
                max_height,
            ).ok()
        }
    }

    /// Shape a single line of text
    fn shape_line(
        &self,
        text: &str,
        font: &dyn Font,
        baseline_y: f32,
        alignment: TextAlign,
        max_width: f32,
    ) -> ShapedLine {
        // Shape glyphs using font metrics
        let mut glyphs = Vec::new();
        let mut current_x = 0.0;
        let mut line_width = 0.0;

        for ch in text.chars() {
            if let Some(metrics) = font.glyph_metrics(ch) {
                glyphs.push(ShapedGlyph {
                    glyph_id: metrics.glyph_id,
                    character: ch,
                    x: current_x,
                    y: baseline_y,
                    advance: metrics.advance,
                    width: metrics.width,
                    height: metrics.height,
                });
                current_x += metrics.advance;
                line_width += metrics.advance;
            }
        }

        // Apply alignment offset
        let x_offset = match alignment {
            TextAlign::Left => 0.0,
            TextAlign::Center => (max_width - line_width).max(0.0) / 2.0,
            TextAlign::Right => (max_width - line_width).max(0.0),
            TextAlign::Justify => 0.0,
        };

        // Offset all glyphs by alignment
        if x_offset > 0.0 {
            for glyph in &mut glyphs {
                glyph.x += x_offset;
            }
        }

        ShapedLine {
            glyphs,
            width: line_width,
            height: font.line_height(),
            ascent: font.ascent(),
            descent: font.descent(),
            baseline_y,
        }
    }

    /// Break text into lines based on max_width and word break rules
    fn break_lines(
        &self,
        text: &str,
        font: &dyn Font,
        max_width: f32,
        word_break: WordBreak,
    ) -> Vec<String> {
        if max_width <= 0.0 {
            // No wrapping, return entire text as single line
            return vec![text.to_string()];
        }

        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0.0;

        match word_break {
            WordBreak::Normal | WordBreak::BreakWord => {
                // Break at word boundaries, and break long words if needed (BreakWord)
                for word in text.split_whitespace() {
                    let word_width = font.measure_text(word);
                    let space_width = font.measure_text(" ");

                    if current_width + word_width <= max_width {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                            current_width += space_width;
                        }
                        current_line.push_str(word);
                        current_width += word_width;
                    } else if word_break == WordBreak::BreakWord && word_width > max_width {
                        // Word is too long, break it character by character
                        for ch in word.chars() {
                            let char_str = ch.to_string();
                            let char_width = font.measure_text(&char_str);

                            if current_width + char_width <= max_width {
                                current_line.push(ch);
                                current_width += char_width;
                            } else {
                                if !current_line.is_empty() {
                                    lines.push(current_line);
                                }
                                current_line = char_str;
                                current_width = char_width;
                            }
                        }
                    } else {
                        // Start new line
                        if !current_line.is_empty() {
                            lines.push(current_line);
                        }
                        current_line = word.to_string();
                        current_width = word_width;
                    }
                }
            }
            WordBreak::BreakAll => {
                // Break at any character
                for ch in text.chars() {
                    let char_str = ch.to_string();
                    let char_width = font.measure_text(&char_str);

                    if current_width + char_width <= max_width {
                        current_line.push(ch);
                        current_width += char_width;
                    } else {
                        if !current_line.is_empty() {
                            lines.push(current_line);
                        }
                        current_line = char_str;
                        current_width = char_width;
                    }
                }
            }
            WordBreak::KeepAll => {
                // Don't break, return single line
                return vec![text.to_string()];
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }
}

impl TextShaper for WindowsTextShaper {
    fn shape_text(
        &self,
        text: &str,
        font: &dyn Font,
        config: &TextLayoutConfig,
    ) -> std::result::Result<ShapedText, ShaperError> {
        if text.is_empty() {
            return Ok(ShapedText::empty());
        }

        // Get font metrics
        let font_size = font.line_height();

        // Break text into lines
        let max_width = config.max_width.unwrap_or(f32::MAX);
        let line_strings = self.break_lines(text, font, max_width, config.word_break);

        // Shape each line
        let mut shaped_lines = Vec::new();
        let mut current_y = 0.0;
        let line_height_multiplier = config.line_height;

        for line_text in line_strings {
            let shaped_line = self.shape_line(
                &line_text,
                font,
                current_y,
                config.alignment,
                max_width,
            );

            let effective_line_height = font_size * line_height_multiplier;
            current_y += effective_line_height.max(shaped_line.height);
            shaped_lines.push(shaped_line);
        }

        // Calculate total dimensions
        let width = shaped_lines
            .iter()
            .map(|line| line.width)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        let height = current_y;

        Ok(ShapedText {
            lines: shaped_lines,
            width,
            height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::font_manager::FontManager;
    use crate::text::{FontDescriptor, FontSource};

    #[test]
    fn test_shaper_creation() {
        let shaper = WindowsTextShaper::new();
        let _ = shaper;
    }

    #[test]
    fn test_shape_empty_text() {
        let shaper = WindowsTextShaper::new();
        let mut font_manager = FontManager::new();

        let font_desc = FontDescriptor {
            source: FontSource::System("Segoe UI".to_string()),
            weight: 400,
            style: crate::text::FontStyle::Normal,
            size: 16.0,
        };

        let font = font_manager.load_font(&font_desc).unwrap();
        let config = TextLayoutConfig::default();

        let shaped = shaper.shape_text("", font, &config).unwrap();
        assert_eq!(shaped.lines.len(), 0);
        assert_eq!(shaped.width, 0.0);
    }

    #[test]
    fn test_shape_single_line() {
        let shaper = WindowsTextShaper::new();
        let mut font_manager = FontManager::new();

        let font_desc = FontDescriptor {
            source: FontSource::System("Segoe UI".to_string()),
            weight: 400,
            style: crate::text::FontStyle::Normal,
            size: 16.0,
        };

        let font = font_manager.load_font(&font_desc).unwrap();
        let config = TextLayoutConfig::default();

        let shaped = shaper.shape_text("Hello", font, &config).unwrap();
        assert_eq!(shaped.lines.len(), 1);
        assert!(shaped.width > 0.0);
        assert!(shaped.height > 0.0);
        assert_eq!(shaped.lines[0].glyphs.len(), 5);
    }

    #[test]
    fn test_line_breaking() {
        let shaper = WindowsTextShaper::new();
        let mut font_manager = FontManager::new();

        let font_desc = FontDescriptor {
            source: FontSource::System("Segoe UI".to_string()),
            weight: 400,
            style: crate::text::FontStyle::Normal,
            size: 16.0,
        };

        let font = font_manager.load_font(&font_desc).unwrap();
        let mut config = TextLayoutConfig::default();
        config.max_width = Some(50.0); // Very narrow, should force wrapping

        let shaped = shaper.shape_text("Hello World Test", font, &config).unwrap();
        // Should break into multiple lines
        assert!(shaped.lines.len() > 1);
    }
}
