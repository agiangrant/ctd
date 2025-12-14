//! Style system - Tailwind-inspired utility classes
//!
//! Performance considerations:
//! - Styles are parsed and compiled at startup into lookup tables
//! - Runtime styling is just integer lookups
//! - Zero-cost abstractions for custom classes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Color representation (RGBA)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 24) & 0xFF) as u8,
            g: ((hex >> 16) & 0xFF) as u8,
            b: ((hex >> 8) & 0xFF) as u8,
            a: (hex & 0xFF) as u8,
        }
    }

    pub fn to_u32(&self) -> u32 {
        ((self.r as u32) << 24)
            | ((self.g as u32) << 16)
            | ((self.b as u32) << 8)
            | (self.a as u32)
    }

    pub fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0, 255)
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }
}

/// Font weight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontWeight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Normal = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

/// Border style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
}

/// Computed style for a widget
#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    // Colors
    pub background_color: Option<Color>,
    pub text_color: Option<Color>,
    pub border_color: Option<Color>,

    // Typography
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,

    // Borders
    pub border_width: Option<f32>,
    pub border_style: Option<BorderStyle>,
    pub border_radius: Option<f32>,

    // Effects
    pub opacity: Option<f32>,
    pub shadow_offset_x: Option<f32>,
    pub shadow_offset_y: Option<f32>,
    pub shadow_blur: Option<f32>,
    pub shadow_color: Option<Color>,
}

/// Style rule that can be applied
#[derive(Debug, Clone)]
pub enum StyleRule {
    // Colors
    BackgroundColor(Color),
    TextColor(Color),
    BorderColor(Color),

    // Typography
    FontSize(f32),
    FontWeight(FontWeight),
    TextAlign(TextAlign),
    LineHeight(f32),

    // Borders
    BorderWidth(f32),
    BorderStyle(BorderStyle),
    BorderRadius(f32),

    // Effects
    Opacity(f32),
    Shadow {
        offset_x: f32,
        offset_y: f32,
        blur: f32,
        color: Color,
    },
}

/// Theme configuration loaded from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct ThemeConfig {
    #[serde(default)]
    pub colors: HashMap<String, String>,
    #[serde(default)]
    pub spacing: HashMap<String, f32>,
    #[serde(default)]
    pub custom_classes: HashMap<String, Vec<String>>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        let mut colors = HashMap::new();

        // Default Tailwind-like colors
        colors.insert("white".to_string(), "#FFFFFF".to_string());
        colors.insert("black".to_string(), "#000000".to_string());
        colors.insert("gray-50".to_string(), "#F9FAFB".to_string());
        colors.insert("gray-100".to_string(), "#F3F4F6".to_string());
        colors.insert("gray-200".to_string(), "#E5E7EB".to_string());
        colors.insert("gray-300".to_string(), "#D1D5DB".to_string());
        colors.insert("gray-400".to_string(), "#9CA3AF".to_string());
        colors.insert("gray-500".to_string(), "#6B7280".to_string());
        colors.insert("gray-600".to_string(), "#4B5563".to_string());
        colors.insert("gray-700".to_string(), "#374151".to_string());
        colors.insert("gray-800".to_string(), "#1F2937".to_string());
        colors.insert("gray-900".to_string(), "#111827".to_string());

        colors.insert("blue-500".to_string(), "#3B82F6".to_string());
        colors.insert("blue-600".to_string(), "#2563EB".to_string());
        colors.insert("red-500".to_string(), "#EF4444".to_string());
        colors.insert("green-500".to_string(), "#10B981".to_string());

        let mut spacing = HashMap::new();
        // Default spacing scale (in pixels)
        for i in 0..=96 {
            spacing.insert(i.to_string(), (i as f32) * 0.25 * 16.0); // 0.25rem = 4px
        }

        Self {
            colors,
            spacing,
            custom_classes: HashMap::new(),
        }
    }
}

/// Main style system
pub struct StyleSystem {
    theme: ThemeConfig,
    /// Cache of parsed utility classes to style rules
    class_cache: HashMap<String, Vec<StyleRule>>,
}

impl StyleSystem {
    pub fn new() -> Self {
        Self {
            theme: ThemeConfig::default(),
            class_cache: HashMap::new(),
        }
    }

    /// Load a theme configuration from TOML
    pub fn load_theme(&mut self, toml_str: &str) -> Result<(), String> {
        let theme: ThemeConfig = toml::from_str(toml_str)
            .map_err(|e| format!("Failed to parse theme TOML: {}", e))?;

        self.theme = theme;
        self.class_cache.clear();
        Ok(())
    }

    /// Parse a class string and return computed styles
    pub fn parse_classes(&mut self, class_str: &str) -> ComputedStyle {
        let mut computed = ComputedStyle::default();

        for class in class_str.split_whitespace() {
            let rules = self.get_or_parse_class(class);
            for rule in rules {
                self.apply_rule(&mut computed, &rule);
            }
        }

        computed
    }

    /// Get style rules for a class, using cache or parsing if needed
    fn get_or_parse_class(&mut self, class: &str) -> Vec<StyleRule> {
        if let Some(rules) = self.class_cache.get(class) {
            return rules.clone();
        }

        // Check if it's a custom class (clone to avoid borrow issues)
        let expanded_classes = self.theme.custom_classes.get(class).cloned();
        if let Some(expanded) = expanded_classes {
            let mut all_rules = Vec::new();
            for expanded_class in &expanded {
                all_rules.extend(self.get_or_parse_class(expanded_class));
            }
            self.class_cache.insert(class.to_string(), all_rules.clone());
            return all_rules;
        }

        // Parse utility class
        let rules = self.parse_utility_class(class);
        self.class_cache.insert(class.to_string(), rules.clone());
        rules
    }

    /// Parse a single utility class into style rules
    fn parse_utility_class(&self, class: &str) -> Vec<StyleRule> {
        let mut rules = Vec::new();

        // Handle state modifiers (hover:, focus:, etc.)
        if class.contains(':') {
            // TODO: Implement state modifiers
            return rules;
        }

        // Text color (text-{color})
        if let Some(color_name) = class.strip_prefix("text-") {
            if let Some(color) = self.parse_color(color_name) {
                rules.push(StyleRule::TextColor(color));
            }
        }

        // Background color (bg-{color})
        if let Some(color_name) = class.strip_prefix("bg-") {
            if let Some(color) = self.parse_color(color_name) {
                rules.push(StyleRule::BackgroundColor(color));
            }
        }

        // Font size (text-{size})
        match class {
            "text-xs" => rules.push(StyleRule::FontSize(12.0)),
            "text-sm" => rules.push(StyleRule::FontSize(14.0)),
            "text-base" => rules.push(StyleRule::FontSize(16.0)),
            "text-lg" => rules.push(StyleRule::FontSize(18.0)),
            "text-xl" => rules.push(StyleRule::FontSize(20.0)),
            "text-2xl" => rules.push(StyleRule::FontSize(24.0)),
            "text-3xl" => rules.push(StyleRule::FontSize(30.0)),
            "text-4xl" => rules.push(StyleRule::FontSize(36.0)),
            _ => {}
        }

        // Font weight
        match class {
            "font-thin" => rules.push(StyleRule::FontWeight(FontWeight::Thin)),
            "font-light" => rules.push(StyleRule::FontWeight(FontWeight::Light)),
            "font-normal" => rules.push(StyleRule::FontWeight(FontWeight::Normal)),
            "font-medium" => rules.push(StyleRule::FontWeight(FontWeight::Medium)),
            "font-semibold" => rules.push(StyleRule::FontWeight(FontWeight::SemiBold)),
            "font-bold" => rules.push(StyleRule::FontWeight(FontWeight::Bold)),
            _ => {}
        }

        // Border radius
        match class {
            "rounded" => rules.push(StyleRule::BorderRadius(4.0)),
            "rounded-sm" => rules.push(StyleRule::BorderRadius(2.0)),
            "rounded-md" => rules.push(StyleRule::BorderRadius(6.0)),
            "rounded-lg" => rules.push(StyleRule::BorderRadius(8.0)),
            "rounded-xl" => rules.push(StyleRule::BorderRadius(12.0)),
            "rounded-full" => rules.push(StyleRule::BorderRadius(9999.0)),
            _ => {}
        }

        // Opacity
        if let Some(opacity_str) = class.strip_prefix("opacity-") {
            if let Ok(opacity_pct) = opacity_str.parse::<f32>() {
                rules.push(StyleRule::Opacity(opacity_pct / 100.0));
            }
        }

        rules
    }

    /// Parse a color name to Color
    fn parse_color(&self, color_name: &str) -> Option<Color> {
        self.theme.colors.get(color_name).and_then(|hex_str| {
            // Parse hex color string like "#RRGGBB" or "#RRGGBBAA"
            let hex_str = hex_str.trim_start_matches('#');
            if hex_str.len() == 6 {
                let r = u8::from_str_radix(&hex_str[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex_str[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex_str[4..6], 16).ok()?;
                Some(Color::new(r, g, b, 255))
            } else if hex_str.len() == 8 {
                let r = u8::from_str_radix(&hex_str[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex_str[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex_str[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex_str[6..8], 16).ok()?;
                Some(Color::new(r, g, b, a))
            } else {
                None
            }
        })
    }

    /// Apply a style rule to computed styles
    fn apply_rule(&self, computed: &mut ComputedStyle, rule: &StyleRule) {
        match rule {
            StyleRule::BackgroundColor(color) => computed.background_color = Some(*color),
            StyleRule::TextColor(color) => computed.text_color = Some(*color),
            StyleRule::BorderColor(color) => computed.border_color = Some(*color),
            StyleRule::FontSize(size) => computed.font_size = Some(*size),
            StyleRule::FontWeight(weight) => computed.font_weight = Some(*weight),
            StyleRule::TextAlign(align) => computed.text_align = Some(*align),
            StyleRule::LineHeight(height) => computed.line_height = Some(*height),
            StyleRule::BorderWidth(width) => computed.border_width = Some(*width),
            StyleRule::BorderStyle(style) => computed.border_style = Some(*style),
            StyleRule::BorderRadius(radius) => computed.border_radius = Some(*radius),
            StyleRule::Opacity(opacity) => computed.opacity = Some(*opacity),
            StyleRule::Shadow { offset_x, offset_y, blur, color } => {
                computed.shadow_offset_x = Some(*offset_x);
                computed.shadow_offset_y = Some(*offset_y);
                computed.shadow_blur = Some(*blur);
                computed.shadow_color = Some(*color);
            }
        }
    }
}

impl Default for StyleSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_conversion() {
        let color = Color::new(255, 128, 64, 32);
        let u32_val = color.to_u32();
        let color2 = Color::from_hex(u32_val);
        assert_eq!(color, color2);
    }

    #[test]
    fn test_parse_simple_classes() {
        let mut system = StyleSystem::new();
        let computed = system.parse_classes("text-lg font-bold");
        assert_eq!(computed.font_size, Some(18.0));
        assert_eq!(computed.font_weight, Some(FontWeight::Bold));
    }

    #[test]
    fn test_parse_color_classes() {
        let mut system = StyleSystem::new();
        let computed = system.parse_classes("text-blue-500 bg-white");
        assert!(computed.text_color.is_some());
        assert!(computed.background_color.is_some());
    }
}
