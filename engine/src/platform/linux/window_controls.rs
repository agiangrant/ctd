//! Linux window control buttons (Adwaita-style)
//!
//! Renders close/minimize/maximize buttons matching the Adwaita theme
//! for frameless windows on Linux.

use super::portal::is_dark_mode;

/// Button type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Close,
    Minimize,
    Maximize,
}

/// Button state
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    pub hovered: bool,
    pub pressed: bool,
    pub maximized: bool,
}

/// Colors for a theme
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub button_idle: u32,
    pub button_hover: u32,
    pub close_hover: u32,
    pub icon: u32,
    pub icon_inactive: u32,
}

impl ThemeColors {
    /// Light theme - subtle, modern look
    pub fn light() -> Self {
        Self {
            button_idle: 0x00000000,     // Fully transparent when idle
            button_hover: 0x00000033,    // Subtle dark on hover
            close_hover: 0xE5484DFF,     // Red for close hover
            icon: 0x1A1A1AFF,            // Near black
            icon_inactive: 0x6B6B6BFF,   // Medium gray
        }
    }

    /// Dark theme - modern, clean look
    pub fn dark() -> Self {
        Self {
            button_idle: 0x00000000,     // Fully transparent when idle
            button_hover: 0xFFFFFF33,    // Subtle light on hover
            close_hover: 0xE5484DFF,     // Red for close hover
            icon: 0xFFFFFFFF,            // White
            icon_inactive: 0x8B8B8BFF,   // Light gray
        }
    }

    /// Get colors based on system theme
    pub fn from_system() -> Self {
        if is_dark_mode() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Get colors from app dark mode setting
    /// 0 = light, 1 = dark, 2 = auto/system
    pub fn from_app_setting(dark_mode: u8) -> Self {
        match dark_mode {
            0 => Self::light(),
            1 => Self::dark(),
            _ => Self::from_system(), // 2 or any other value = auto
        }
    }
}

/// Window control button configuration
#[derive(Debug, Clone)]
pub struct WindowControls {
    /// Which buttons to show
    pub show_close: bool,
    pub show_minimize: bool,
    pub show_maximize: bool,
    /// Button positions (right-aligned is standard for most Linux DEs)
    pub right_aligned: bool,
    /// Current button states
    pub close_state: ButtonState,
    pub minimize_state: ButtonState,
    pub maximize_state: ButtonState,
    /// Whether window is active
    pub active: bool,
    /// Current theme colors
    pub colors: ThemeColors,
}

impl Default for WindowControls {
    fn default() -> Self {
        Self {
            show_close: true,
            show_minimize: true,
            show_maximize: true,
            right_aligned: true,
            close_state: ButtonState::default(),
            minimize_state: ButtonState::default(),
            maximize_state: ButtonState::default(),
            active: true,
            colors: ThemeColors::from_system(),
        }
    }
}

/// Button dimensions and layout (in logical pixels)
pub const BUTTON_RADIUS: f32 = 12.0;
pub const BUTTON_SPACING: f32 = 4.0;
pub const BUTTON_MARGIN: f32 = 8.0;
pub const HEADER_HEIGHT: f32 = 36.0;

/// Window border and corner radius for frameless windows
pub const WINDOW_CORNER_RADIUS: f32 = 12.0;
pub const WINDOW_BORDER_WIDTH: f32 = 1.0;
/// Border color for light mode (darker gray)
pub const WINDOW_BORDER_COLOR_LIGHT: u32 = 0x00000040; // ~25% black
/// Border color for dark mode (medium gray)
pub const WINDOW_BORDER_COLOR_DARK: u32 = 0x80808080;  // 50% gray, 50% opacity

impl WindowControls {
    /// Create new window controls with options
    pub fn new(show_close: bool, show_minimize: bool, show_maximize: bool) -> Self {
        Self {
            show_close,
            show_minimize,
            show_maximize,
            ..Default::default()
        }
    }

    /// Create window controls with app dark mode setting
    /// dark_mode: 0 = light, 1 = dark, 2 = auto/system
    pub fn with_dark_mode(show_close: bool, show_minimize: bool, show_maximize: bool, dark_mode: u8) -> Self {
        Self {
            show_close,
            show_minimize,
            show_maximize,
            colors: ThemeColors::from_app_setting(dark_mode),
            ..Default::default()
        }
    }

    /// Update theme colors based on system setting
    pub fn update_theme(&mut self) {
        self.colors = ThemeColors::from_system();
    }

    /// Update theme colors based on app dark mode setting
    pub fn update_theme_from_app(&mut self, dark_mode: u8) {
        self.colors = ThemeColors::from_app_setting(dark_mode);
    }

    /// Get button positions for rendering
    /// Returns vec of (kind, center_x, center_y, state)
    pub fn get_button_layout(&self, window_width: f32) -> Vec<(ButtonKind, f32, f32, ButtonState)> {
        let mut buttons = Vec::new();
        let center_y = HEADER_HEIGHT / 2.0;

        // Calculate button positions (right-aligned)
        let mut x = if self.right_aligned {
            window_width - BUTTON_MARGIN - BUTTON_RADIUS
        } else {
            BUTTON_MARGIN + BUTTON_RADIUS
        };

        let step = if self.right_aligned {
            -(BUTTON_RADIUS * 2.0 + BUTTON_SPACING)
        } else {
            BUTTON_RADIUS * 2.0 + BUTTON_SPACING
        };

        // Order for right-aligned: Close, Maximize, Minimize (right to left)
        // Order for left-aligned: Close, Minimize, Maximize (left to right)
        if self.show_close {
            buttons.push((ButtonKind::Close, x, center_y, self.close_state));
            x += step;
        }

        if self.right_aligned {
            if self.show_maximize {
                buttons.push((ButtonKind::Maximize, x, center_y, self.maximize_state));
                x += step;
            }
            if self.show_minimize {
                buttons.push((ButtonKind::Minimize, x, center_y, self.minimize_state));
            }
        } else {
            if self.show_minimize {
                buttons.push((ButtonKind::Minimize, x, center_y, self.minimize_state));
                x += step;
            }
            if self.show_maximize {
                buttons.push((ButtonKind::Maximize, x, center_y, self.maximize_state));
            }
        }

        buttons
    }

    /// Hit test - returns which button (if any) is at the given position
    pub fn hit_test(&self, x: f32, y: f32, window_width: f32) -> Option<ButtonKind> {
        // Quick bounds check for header area
        if y < 0.0 || y > HEADER_HEIGHT {
            return None;
        }

        for (kind, cx, cy, _) in self.get_button_layout(window_width) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= BUTTON_RADIUS * BUTTON_RADIUS {
                return Some(kind);
            }
        }

        None
    }

    /// Update hover state based on mouse position
    /// Returns true if state changed
    pub fn update_hover(&mut self, x: f32, y: f32, window_width: f32) -> bool {
        let hovered = self.hit_test(x, y, window_width);

        let close_hovered = hovered == Some(ButtonKind::Close);
        let minimize_hovered = hovered == Some(ButtonKind::Minimize);
        let maximize_hovered = hovered == Some(ButtonKind::Maximize);

        let changed = self.close_state.hovered != close_hovered
            || self.minimize_state.hovered != minimize_hovered
            || self.maximize_state.hovered != maximize_hovered;

        self.close_state.hovered = close_hovered;
        self.minimize_state.hovered = minimize_hovered;
        self.maximize_state.hovered = maximize_hovered;

        changed
    }

    /// Clear all hover states
    pub fn clear_hover(&mut self) {
        self.close_state.hovered = false;
        self.minimize_state.hovered = false;
        self.maximize_state.hovered = false;
    }

    /// Get button background color
    pub fn button_bg_color(&self, kind: ButtonKind, state: ButtonState) -> u32 {
        if state.hovered {
            if kind == ButtonKind::Close {
                self.colors.close_hover
            } else {
                self.colors.button_hover
            }
        } else {
            self.colors.button_idle
        }
    }

    /// Get icon color
    pub fn icon_color(&self, state: ButtonState) -> u32 {
        if self.active {
            self.colors.icon
        } else {
            self.colors.icon_inactive
        }
    }

    /// Generate render commands for the window control buttons
    /// Uses simple text-based icons for maximum compatibility
    pub fn to_render_commands(&self, window_width: f32) -> Vec<crate::render::RenderCommand> {
        use crate::render::RenderCommand;
        use crate::text::{FontDescriptor, FontSource, FontStyle, TextLayoutConfig, TextAlign, VerticalAlign};

        let mut commands = Vec::new();
        let buttons = self.get_button_layout(window_width);

        for (kind, cx, cy, state) in buttons {
            let bg_color = self.button_bg_color(kind, state);
            let size = BUTTON_RADIUS * 2.0;
            let x = cx - BUTTON_RADIUS;
            let y = cy - BUTTON_RADIUS;

            // Only draw background if it's not fully transparent
            if bg_color & 0xFF != 0 {
                commands.push(RenderCommand::DrawRect {
                    x,
                    y,
                    width: size,
                    height: size,
                    color: bg_color,
                    corner_radii: [BUTTON_RADIUS, BUTTON_RADIUS, BUTTON_RADIUS, BUTTON_RADIUS],
                    rotation: 0.0,
                    border: None,
                    gradient: None,
                });
            }

            // Draw icon using text
            let icon_color = if kind == ButtonKind::Close && state.hovered {
                0xFFFFFFFF // White icon on red hover
            } else {
                self.icon_color(state)
            };

            // Draw icon based on button type
            match kind {
                ButtonKind::Close => {
                    // X icon using text
                    commands.push(RenderCommand::DrawText {
                        x,
                        y,
                        text: "×".to_string(),
                        font: FontDescriptor {
                            source: FontSource::System("sans-serif".to_string()),
                            size: 16.0,
                            weight: 300,
                            style: FontStyle::Normal,
                        },
                        color: icon_color,
                        layout: TextLayoutConfig {
                            max_width: Some(size),
                            max_height: Some(size),
                            alignment: TextAlign::Center,
                            vertical_align: VerticalAlign::Middle,
                            ..Default::default()
                        },
                    });
                }
                ButtonKind::Minimize => {
                    // Minus icon using text
                    commands.push(RenderCommand::DrawText {
                        x,
                        y,
                        text: "−".to_string(),
                        font: FontDescriptor {
                            source: FontSource::System("sans-serif".to_string()),
                            size: 16.0,
                            weight: 300,
                            style: FontStyle::Normal,
                        },
                        color: icon_color,
                        layout: TextLayoutConfig {
                            max_width: Some(size),
                            max_height: Some(size),
                            alignment: TextAlign::Center,
                            vertical_align: VerticalAlign::Middle,
                            ..Default::default()
                        },
                    });
                }
                ButtonKind::Maximize => {
                    // Draw maximize/restore as a bordered rectangle (more reliable than Unicode)
                    let icon_size = if state.maximized { 7.0 } else { 8.0 };
                    let icon_x = cx - icon_size / 2.0;
                    let icon_y = cy - icon_size / 2.0;

                    commands.push(RenderCommand::DrawRect {
                        x: icon_x,
                        y: icon_y,
                        width: icon_size,
                        height: icon_size,
                        color: 0x00000000, // Transparent fill
                        corner_radii: [1.0, 1.0, 1.0, 1.0],
                        rotation: 0.0,
                        border: Some(crate::render::Border {
                            width: 1.5,
                            color: icon_color,
                            style: crate::render::BorderStyle::Solid,
                        }),
                        gradient: None,
                    });

                    // If maximized, draw a second offset rectangle for restore icon
                    if state.maximized {
                        commands.push(RenderCommand::DrawRect {
                            x: icon_x + 2.0,
                            y: icon_y - 2.0,
                            width: icon_size,
                            height: icon_size,
                            color: 0x00000000,
                            corner_radii: [1.0, 1.0, 1.0, 1.0],
                            rotation: 0.0,
                            border: Some(crate::render::Border {
                                width: 1.5,
                                color: icon_color,
                                style: crate::render::BorderStyle::Solid,
                            }),
                            gradient: None,
                        });
                    }
                }
            }
        }

        commands
    }

    /// Get the window border color based on current theme
    pub fn border_color(&self) -> u32 {
        // Check if we're in dark mode by looking at the icon color
        // Dark mode has white icons (0xFFFFFFFF)
        if self.colors.icon == 0xFFFFFFFF {
            WINDOW_BORDER_COLOR_DARK
        } else {
            WINDOW_BORDER_COLOR_LIGHT
        }
    }
}

/// Generate render command for window border
/// This creates a rounded rectangle outline that visually defines the window bounds
pub fn window_border_command(width: f32, height: f32, is_dark: bool) -> crate::render::RenderCommand {
    use crate::render::{RenderCommand, Border, BorderStyle};

    let border_color = if is_dark {
        WINDOW_BORDER_COLOR_DARK
    } else {
        WINDOW_BORDER_COLOR_LIGHT
    };

    RenderCommand::DrawRect {
        x: 0.0,
        y: 0.0,
        width,
        height,
        color: 0x00000000, // Transparent fill
        corner_radii: [WINDOW_CORNER_RADIUS, WINDOW_CORNER_RADIUS, WINDOW_CORNER_RADIUS, WINDOW_CORNER_RADIUS],
        rotation: 0.0,
        border: Some(Border {
            width: WINDOW_BORDER_WIDTH,
            color: border_color,
            style: BorderStyle::Solid,
        }),
        gradient: None,
    }
}

/// Resize border hit detection for frameless windows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    /// Convert to winit ResizeDirection
    pub fn to_resize_direction(self) -> winit::window::ResizeDirection {
        use winit::window::ResizeDirection;
        match self {
            ResizeEdge::Top => ResizeDirection::North,
            ResizeEdge::Bottom => ResizeDirection::South,
            ResizeEdge::Left => ResizeDirection::West,
            ResizeEdge::Right => ResizeDirection::East,
            ResizeEdge::TopLeft => ResizeDirection::NorthWest,
            ResizeEdge::TopRight => ResizeDirection::NorthEast,
            ResizeEdge::BottomLeft => ResizeDirection::SouthWest,
            ResizeEdge::BottomRight => ResizeDirection::SouthEast,
        }
    }
}

/// Border width for resize detection
pub const RESIZE_BORDER: f32 = 5.0;
/// Corner size for resize detection
pub const RESIZE_CORNER: f32 = 10.0;

/// Detect if position is on a resize edge
pub fn detect_resize_edge(x: f32, y: f32, width: f32, height: f32) -> Option<ResizeEdge> {
    let on_left = x < RESIZE_BORDER;
    let on_right = x >= width - RESIZE_BORDER;
    let on_top = y < RESIZE_BORDER;
    let on_bottom = y >= height - RESIZE_BORDER;

    let in_left_corner = x < RESIZE_CORNER;
    let in_right_corner = x >= width - RESIZE_CORNER;
    let in_top_corner = y < RESIZE_CORNER;
    let in_bottom_corner = y >= height - RESIZE_CORNER;

    // Corners take priority
    if on_top && in_left_corner || on_left && in_top_corner {
        return Some(ResizeEdge::TopLeft);
    }
    if on_top && in_right_corner || on_right && in_top_corner {
        return Some(ResizeEdge::TopRight);
    }
    if on_bottom && in_left_corner || on_left && in_bottom_corner {
        return Some(ResizeEdge::BottomLeft);
    }
    if on_bottom && in_right_corner || on_right && in_bottom_corner {
        return Some(ResizeEdge::BottomRight);
    }

    // Then edges
    if on_top {
        return Some(ResizeEdge::Top);
    }
    if on_bottom {
        return Some(ResizeEdge::Bottom);
    }
    if on_left {
        return Some(ResizeEdge::Left);
    }
    if on_right {
        return Some(ResizeEdge::Right);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_layout() {
        let controls = WindowControls::default();
        let buttons = controls.get_button_layout(800.0);
        assert_eq!(buttons.len(), 3);
    }

    #[test]
    fn test_hit_test() {
        let controls = WindowControls::default();
        // Test hit on close button (rightmost)
        let hit = controls.hit_test(800.0 - BUTTON_MARGIN - BUTTON_RADIUS, HEADER_HEIGHT / 2.0, 800.0);
        assert_eq!(hit, Some(ButtonKind::Close));
    }

    #[test]
    fn test_resize_edge() {
        // Test corner detection
        assert_eq!(detect_resize_edge(2.0, 2.0, 800.0, 600.0), Some(ResizeEdge::TopLeft));
        assert_eq!(detect_resize_edge(798.0, 2.0, 800.0, 600.0), Some(ResizeEdge::TopRight));

        // Test edge detection
        assert_eq!(detect_resize_edge(400.0, 2.0, 800.0, 600.0), Some(ResizeEdge::Top));
        assert_eq!(detect_resize_edge(2.0, 300.0, 800.0, 600.0), Some(ResizeEdge::Left));

        // Test no edge
        assert_eq!(detect_resize_edge(400.0, 300.0, 800.0, 600.0), None);
    }
}
