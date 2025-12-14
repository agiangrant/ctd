//! macOS text shaper using Core Text
//!
//! Uses CTLine for text layout and shaping.

use super::{ShapedGlyph, ShapedLine, ShapedText, ShaperError, TextShaper};
use crate::text::font_manager::Font;
use crate::text::{TextAlign, TextLayoutConfig, WordBreak};
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::{CFRange, TCFType};
use core_foundation::string::CFString;
use core_text::font::CTFont;
use core_text::line::CTLine;
use core_text::string_attributes;

/// macOS text shaper using Core Text
pub struct MacOSTextShaper;

impl MacOSTextShaper {
    pub fn new() -> Self {
        Self
    }

    /// Create an attributed string with the font
    fn create_attributed_string(&self, text: &str, ct_font: &CTFont) -> CFMutableAttributedString {
        let cf_string = CFString::new(text);
        let mut attr_string = CFMutableAttributedString::new();
        attr_string.replace_str(&cf_string, CFRange::init(0, 0));

        let string_range = CFRange::init(0, cf_string.char_len() as isize);
        unsafe {
            attr_string.set_attribute(
                string_range,
                string_attributes::kCTFontAttributeName,
                ct_font,
            );
        }

        attr_string
    }

    /// Shape a single line of text
    fn shape_line(
        &self,
        text: &str,
        font: &dyn Font,
        ct_font: &CTFont,
        baseline_y: f32,
        alignment: TextAlign,
        max_width: f32,
    ) -> ShapedLine {
        let attr_string = self.create_attributed_string(text, ct_font);
        let line = CTLine::new_with_attributed_string(attr_string.as_concrete_TypeRef());

        // Get typographic bounds
        let bounds = line.get_typographic_bounds();
        let line_width = bounds.width as f32;

        // Calculate X offset based on alignment
        let x_offset = match alignment {
            TextAlign::Left => 0.0,
            TextAlign::Center => (max_width - line_width) / 2.0,
            TextAlign::Right => max_width - line_width,
            TextAlign::Justify => 0.0, // TODO: Use CTLine justification
        };

        // Shape glyphs
        let mut glyphs = Vec::new();
        let mut current_x = x_offset;

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
            }
        }

        ShapedLine {
            glyphs,
            width: line_width,
            height: (bounds.ascent + bounds.descent) as f32,
            ascent: bounds.ascent as f32,
            descent: bounds.descent as f32,
            baseline_y,
        }
    }

    /// Break text into lines based on max_width and word break rules
    fn break_lines(&self, text: &str, font: &dyn Font, max_width: f32, word_break: WordBreak) -> Vec<String> {
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

impl TextShaper for MacOSTextShaper {
    fn shape_text(
        &self,
        text: &str,
        font: &dyn Font,
        config: &TextLayoutConfig,
    ) -> Result<ShapedText, ShaperError> {
        if text.is_empty() {
            return Ok(ShapedText::empty());
        }

        // Get font metrics
        let font_size = font.line_height(); // Use line height as approximation for font size

        // Get CTFont from the Font trait
        // For now, we'll create a new CTFont - in the future, we could cache this
        let ct_font = core_text::font::new_from_name("System", font_size as f64)
            .map_err(|_| ShaperError::LayoutError("Failed to create CTFont".to_string()))?;

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
                &ct_font,
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
    use crate::text::{FontDescriptor, FontSource};
    use crate::text::font_manager::FontManager;

    #[test]
    fn test_shaper_creation() {
        let shaper = MacOSTextShaper::new();
        // Just verify it can be created
        let _ = shaper;
    }

    #[test]
    fn test_shape_empty_text() {
        let shaper = MacOSTextShaper::new();
        let mut font_manager = FontManager::new();

        let font_desc = FontDescriptor {
            source: FontSource::System("San Francisco".to_string()),
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
        let shaper = MacOSTextShaper::new();
        let mut font_manager = FontManager::new();

        let font_desc = FontDescriptor {
            source: FontSource::System("San Francisco".to_string()),
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
        let shaper = MacOSTextShaper::new();
        let mut font_manager = FontManager::new();

        let font_desc = FontDescriptor {
            source: FontSource::System("San Francisco".to_string()),
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
