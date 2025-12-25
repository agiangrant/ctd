//! Windows window control buttons (Windows 11 style)
//!
//! Renders close/minimize/maximize buttons matching Windows 11 design
//! for frameless windows on Windows.

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
    pub close_hover_icon: u32,
    pub icon: u32,
    pub icon_hover: u32,
    pub icon_inactive: u32,
}

impl ThemeColors {
    /// Light theme - Windows 11 style
    pub fn light() -> Self {
        Self {
            button_idle: 0x00000000,      // Transparent when idle
            button_hover: 0x0000001A,     // Subtle dark on hover (~10%)
            close_hover: 0xC42B1CFF,      // Windows red for close
            close_hover_icon: 0xFFFFFFFF, // White icon on red
            icon: 0x606060FF,             // Light gray icons
            icon_hover: 0x000000FF,       // Black icons on hover
            icon_inactive: 0x00000080,    // 50% black when inactive
        }
    }

    /// Dark theme - Windows 11 style
    pub fn dark() -> Self {
        Self {
            button_idle: 0x00000000,      // Transparent when idle
            button_hover: 0xFFFFFF1A,     // Subtle light on hover (~10%)
            close_hover: 0xC42B1CFF,      // Windows red for close
            close_hover_icon: 0xFFFFFFFF, // White icon on red
            icon: 0x9E9E9EFF,             // Light gray icons (like Windows 11)
            icon_hover: 0xFFFFFFFF,       // White icons on hover
            icon_inactive: 0xFFFFFF80,    // 50% white when inactive
        }
    }

    /// Get colors based on system theme
    pub fn from_system() -> Self {
        // TODO: Check Windows dark mode setting
        // For now, default to dark theme as it's common
        Self::dark()
    }

    /// Get colors from app dark mode setting
    /// 0 = light, 1 = dark, 2 = auto/system
    pub fn from_app_setting(dark_mode: u8) -> Self {
        match dark_mode {
            0 => Self::light(),
            1 => Self::dark(),
            _ => Self::from_system(),
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
            close_state: ButtonState::default(),
            minimize_state: ButtonState::default(),
            maximize_state: ButtonState::default(),
            active: true,
            colors: ThemeColors::from_system(),
        }
    }
}

/// Button dimensions and layout (in logical pixels)
/// Windows 11 uses 46x32 pixel buttons
pub const BUTTON_WIDTH: f32 = 46.0;
pub const BUTTON_HEIGHT: f32 = 32.0;
pub const HEADER_HEIGHT: f32 = 32.0;

/// Window border and corner radius for frameless windows
pub const WINDOW_CORNER_RADIUS: f32 = 8.0;
pub const WINDOW_BORDER_WIDTH: f32 = 1.0;
/// Border color for light mode
pub const WINDOW_BORDER_COLOR_LIGHT: u32 = 0x00000033;
/// Border color for dark mode
pub const WINDOW_BORDER_COLOR_DARK: u32 = 0xFFFFFF33;

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
    /// Returns vec of (kind, x, y, width, height, state)
    /// Windows order (right to left): Close, Maximize, Minimize
    pub fn get_button_layout(&self, window_width: f32) -> Vec<(ButtonKind, f32, f32, f32, f32, ButtonState)> {
        let mut buttons = Vec::new();
        let mut x = window_width;

        // Close button (rightmost)
        if self.show_close {
            x -= BUTTON_WIDTH;
            buttons.push((ButtonKind::Close, x, 0.0, BUTTON_WIDTH, BUTTON_HEIGHT, self.close_state));
        }

        // Maximize button
        if self.show_maximize {
            x -= BUTTON_WIDTH;
            buttons.push((ButtonKind::Maximize, x, 0.0, BUTTON_WIDTH, BUTTON_HEIGHT, self.maximize_state));
        }

        // Minimize button
        if self.show_minimize {
            x -= BUTTON_WIDTH;
            buttons.push((ButtonKind::Minimize, x, 0.0, BUTTON_WIDTH, BUTTON_HEIGHT, self.minimize_state));
        }

        buttons
    }

    /// Hit test - returns which button (if any) is at the given position
    pub fn hit_test(&self, x: f32, y: f32, window_width: f32) -> Option<ButtonKind> {
        // Quick bounds check for header area
        if y < 0.0 || y > HEADER_HEIGHT {
            return None;
        }

        for (kind, bx, by, bw, bh, _) in self.get_button_layout(window_width) {
            if x >= bx && x < bx + bw && y >= by && y < by + bh {
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

    /// Get icon color - light gray normally, white on hover (like Windows 11)
    pub fn icon_color(&self, kind: ButtonKind, state: ButtonState) -> u32 {
        if kind == ButtonKind::Close && state.hovered {
            self.colors.close_hover_icon  // White on red background
        } else if state.hovered {
            self.colors.icon_hover  // White on hover
        } else if self.active {
            self.colors.icon  // Light gray normally
        } else {
            self.colors.icon_inactive
        }
    }

    /// Generate render commands for the window control buttons
    /// Uses shape-based icons for consistency and precise control
    pub fn to_render_commands(&self, window_width: f32) -> Vec<crate::render::RenderCommand> {
        use crate::render::RenderCommand;

        let mut commands = Vec::new();
        let buttons = self.get_button_layout(window_width);

        for (kind, x, y, w, h, state) in buttons {
            let bg_color = self.button_bg_color(kind, state);

            // Draw background when hovered (transparent when not hovered)
            if bg_color & 0xFF != 0 {
                commands.push(RenderCommand::DrawRect {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: bg_color,
                    corner_radii: [0.0, 0.0, 0.0, 0.0],
                    rotation: 0.0,
                    border: None,
                    gradient: None,
                });
            }

            let icon_color = self.icon_color(kind, state);
            let cx = x + w / 2.0;
            let cy = y + h / 2.0;

            // Draw icon based on button type - using shapes for consistency
            match kind {
                ButtonKind::Close => {
                    // X icon - like a plus rotated 45 degrees, square proportions
                    let arm_length = 13.0;  // Length of each arm
                    let arm_thickness = 0.8;  // Thin like maximize border
                    let rotation_45 = std::f32::consts::FRAC_PI_4;  // 45 degrees in radians

                    // First diagonal (rotated 45 degrees = π/4 radians)
                    commands.push(RenderCommand::DrawRect {
                        x: cx - arm_length / 2.0,
                        y: (cy - arm_thickness / 2.0) + 1.0,
                        width: arm_length,
                        height: arm_thickness,
                        color: icon_color,
                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                        rotation: rotation_45,
                        border: None,
                        gradient: None,
                    });

                    // Second diagonal (rotated -45 degrees = -π/4 radians)
                    commands.push(RenderCommand::DrawRect {
                        x: cx - arm_length / 2.0,
                        y: (cy - arm_thickness / 2.0) + 1.0,
                        width: arm_length,
                        height: arm_thickness,
                        color: icon_color,
                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                        rotation: -rotation_45,
                        border: None,
                        gradient: None,
                    });
                }
                ButtonKind::Minimize => {
                    // Horizontal line - thin rectangle matching maximize border weight
                    let line_width = 10.0;
                    let line_height = 0.8;  // Match maximize border thickness

                    commands.push(RenderCommand::DrawRect {
                        x: cx - line_width / 2.0,
                        y: cy - (line_height / 2.0) + 1.5,
                        width: line_width,
                        height: line_height,
                        color: icon_color,
                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                        rotation: 0.0,
                        border: None,
                        gradient: None,
                    });
                }
                ButtonKind::Maximize => {
                    // Draw maximize/restore as bordered rectangles with subtle radius
                    let icon_size = if state.maximized { 7.0 } else { 10.0 };
                    let icon_x = cx - icon_size / 2.0;
                    let icon_y = cy - icon_size / 2.0;

                    if state.maximized {
                        // Restore icon - two overlapping rectangles
                        let offset = 1.5;

                        // Back rectangle
                        commands.push(RenderCommand::DrawRect {
                            x: icon_x + offset,
                            y: icon_y - offset,
                            width: icon_size,
                            height: icon_size,
                            color: 0x00000000,
                            corner_radii: [1.5, 1.5, 1.5, 1.5],
                            rotation: 0.0,
                            border: Some(crate::render::Border {
                                width: 1.0,
                                color: icon_color,
                                style: crate::render::BorderStyle::Solid,
                            }),
                            gradient: None,
                        });

                        // Front rectangle
                        commands.push(RenderCommand::DrawRect {
                            x: icon_x,
                            y: icon_y + offset,
                            width: icon_size,
                            height: icon_size,
                            color: 0x00000000,
                            corner_radii: [1.5, 1.5, 1.5, 1.5],
                            rotation: 0.0,
                            border: Some(crate::render::Border {
                                width: 1.0,
                                color: icon_color,
                                style: crate::render::BorderStyle::Solid,
                            }),
                            gradient: None,
                        });
                    } else {
                        // Maximize icon - single rectangle
                        commands.push(RenderCommand::DrawRect {
                            x: icon_x,
                            y: icon_y + 1.0,
                            width: icon_size,
                            height: icon_size,
                            color: 0x00000000,
                            corner_radii: [1.5, 1.5, 1.5, 1.5],
                            rotation: 0.0,
                            border: Some(crate::render::Border {
                                width: 1.0,
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
        if self.colors.icon == 0xFFFFFFFF {
            WINDOW_BORDER_COLOR_DARK
        } else {
            WINDOW_BORDER_COLOR_LIGHT
        }
    }
}

/// Generate render command for window border
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
        color: 0x00000000,
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
