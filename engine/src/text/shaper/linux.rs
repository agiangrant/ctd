//! Linux text shaper
//!
//! Simple text shaping without HarfBuzz (can be added later for complex scripts).
//! Handles line breaking, alignment, and basic glyph positioning.

use super::{ShapedGlyph, ShapedLine, ShapedText, ShaperError, TextShaper};
use crate::text::font_manager::Font;
use crate::text::{TextAlign, TextLayoutConfig, WordBreak};

/// Linux text shaper
///
/// This is a simple implementation that:
/// - Breaks text into lines based on width constraints
/// - Positions glyphs based on font metrics
/// - Handles text alignment
///
/// For complex scripts (Arabic, Hindi, Thai, etc.), HarfBuzz integration
/// would be needed for proper shaping.
pub struct LinuxTextShaper;

impl LinuxTextShaper {
    pub fn new() -> Self {
        Self
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
        // Calculate line width
        let line_width = font.measure_text(text);

        // Calculate X offset based on alignment
        let x_offset = match alignment {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((max_width - line_width) / 2.0).max(0.0),
            TextAlign::Right => (max_width - line_width).max(0.0),
            TextAlign::Justify => 0.0, // TODO: Implement justify
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
            } else {
                // Fallback for missing glyphs - use space advance
                if let Some(space_metrics) = font.glyph_metrics(' ') {
                    current_x += space_metrics.advance;
                }
            }
        }

        ShapedLine {
            glyphs,
            width: line_width,
            height: font.ascent() + font.descent(),
            ascent: font.ascent(),
            descent: font.descent(),
            baseline_y,
        }
    }

    /// Break text into lines based on max_width and word break rules
    fn break_lines(&self, text: &str, font: &dyn Font, max_width: f32, word_break: WordBreak) -> Vec<String> {
        if max_width <= 0.0 || max_width == f32::MAX {
            // No wrapping, but split on explicit newlines
            return text.lines().map(String::from).collect();
        }

        let mut lines = Vec::new();

        // Process each explicit line separately
        for paragraph in text.lines() {
            let mut current_line = String::new();
            let mut current_width = 0.0;

            match word_break {
                WordBreak::Normal | WordBreak::BreakWord => {
                    // Break at word boundaries, and break long words if needed (BreakWord)
                    for word in paragraph.split_whitespace() {
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
                    for ch in paragraph.chars() {
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
                    // Don't break within words
                    lines.push(paragraph.to_string());
                    continue;
                }
            }

            // Push the last line of the paragraph
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }
}

impl Default for LinuxTextShaper {
    fn default() -> Self {
        Self::new()
    }
}

impl TextShaper for LinuxTextShaper {
    fn shape_text(
        &self,
        text: &str,
        font: &dyn Font,
        config: &TextLayoutConfig,
    ) -> Result<ShapedText, ShaperError> {
        if text.is_empty() {
            return Ok(ShapedText::empty());
        }

        // Break text into lines
        let max_width = config.max_width.unwrap_or(f32::MAX);
        let line_strings = self.break_lines(text, font, max_width, config.word_break);

        // Shape each line
        let mut shaped_lines = Vec::new();
        let mut current_y = font.ascent(); // Start at first baseline
        let line_height = font.line_height() * config.line_height;

        for (i, line_text) in line_strings.iter().enumerate() {
            let shaped_line = self.shape_line(
                line_text,
                font,
                current_y,
                config.alignment,
                max_width,
            );

            shaped_lines.push(shaped_line);

            // Move to next line (except for last line)
            if i < line_strings.len() - 1 {
                current_y += line_height;
            }
        }

        // Calculate total dimensions
        let width = shaped_lines
            .iter()
            .map(|line| line.width)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let height = if shaped_lines.is_empty() {
            0.0
        } else {
            current_y + font.descent()
        };

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

    // Mock font for testing
    struct MockFont {
        char_width: f32,
    }

    impl MockFont {
        fn new(char_width: f32) -> Self {
            Self { char_width }
        }
    }

    impl Font for MockFont {
        fn glyph_metrics(&self, _character: char) -> Option<crate::text::font_manager::GlyphMetrics> {
            Some(crate::text::font_manager::GlyphMetrics {
                glyph_id: 0,
                advance: self.char_width,
                width: self.char_width,
                height: 12.0,
                bearing_x: 0.0,
                bearing_y: 10.0,
            })
        }

        fn ascent(&self) -> f32 { 10.0 }
        fn descent(&self) -> f32 { 3.0 }
        fn line_height(&self) -> f32 { 14.0 }
        fn cap_height(&self) -> f32 { 8.0 }
        fn x_height(&self) -> f32 { 6.0 }
        fn size(&self) -> f32 { 12.0 }
    }

    #[test]
    fn test_shaper_creation() {
        let shaper = LinuxTextShaper::new();
        let _ = shaper;
    }

    #[test]
    fn test_shape_empty_text() {
        let shaper = LinuxTextShaper::new();
        let font = MockFont::new(8.0);
        let config = TextLayoutConfig::default();

        let shaped = shaper.shape_text("", &font, &config).unwrap();
        assert_eq!(shaped.lines.len(), 0);
        assert_eq!(shaped.width, 0.0);
    }

    #[test]
    fn test_shape_single_line() {
        let shaper = LinuxTextShaper::new();
        let font = MockFont::new(8.0);
        let config = TextLayoutConfig::default();

        let shaped = shaper.shape_text("Hello", &font, &config).unwrap();
        assert_eq!(shaped.lines.len(), 1);
        assert!(shaped.width > 0.0);
        assert!(shaped.height > 0.0);
        assert_eq!(shaped.lines[0].glyphs.len(), 5);
    }

    #[test]
    fn test_line_breaking() {
        let shaper = LinuxTextShaper::new();
        let font = MockFont::new(10.0); // 10px per char
        let mut config = TextLayoutConfig::default();
        config.max_width = Some(50.0); // 5 chars max per line

        let shaped = shaper.shape_text("Hello World Test", &font, &config).unwrap();
        // Should break into multiple lines
        assert!(shaped.lines.len() > 1);
    }

    #[test]
    fn test_explicit_newlines() {
        let shaper = LinuxTextShaper::new();
        let font = MockFont::new(8.0);
        let config = TextLayoutConfig::default();

        let shaped = shaper.shape_text("Line1\nLine2\nLine3", &font, &config).unwrap();
        assert_eq!(shaped.lines.len(), 3);
    }

    #[test]
    fn test_alignment_center() {
        let shaper = LinuxTextShaper::new();
        let font = MockFont::new(10.0);
        let mut config = TextLayoutConfig::default();
        config.max_width = Some(100.0);
        config.alignment = TextAlign::Center;

        let shaped = shaper.shape_text("Hi", &font, &config).unwrap();
        // "Hi" is 20px wide, centered in 100px should start at 40px
        let first_glyph_x = shaped.lines[0].glyphs[0].x;
        assert!((first_glyph_x - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_alignment_right() {
        let shaper = LinuxTextShaper::new();
        let font = MockFont::new(10.0);
        let mut config = TextLayoutConfig::default();
        config.max_width = Some(100.0);
        config.alignment = TextAlign::Right;

        let shaped = shaper.shape_text("Hi", &font, &config).unwrap();
        // "Hi" is 20px wide, right-aligned in 100px should start at 80px
        let first_glyph_x = shaped.lines[0].glyphs[0].x;
        assert!((first_glyph_x - 80.0).abs() < 0.01);
    }
}
