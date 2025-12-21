//! Rendering module - supports both immediate and retained mode rendering

use crate::text::{FontDescriptor, TextLayoutConfig};
use serde::{Deserialize, Serialize};

/// Rendering mode for the engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    /// Immediate mode: Go sends complete scene description every frame
    Immediate,
    /// Retained mode: Rust maintains widget tree, Go sends updates only
    Retained,
}

/// Command buffer for immediate mode rendering
/// Designed for efficient serialization and zero-copy where possible
#[derive(Debug, Default)]
pub struct CommandBuffer {
    /// Commands to execute this frame
    commands: Vec<RenderCommand>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: RenderCommand) {
        self.commands.push(command);
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }
}

// ===== Supporting Types (must be defined before RenderCommand) =====

/// Border specification for rectangles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Border {
    /// Border width in pixels
    pub width: f32,
    /// Border color (0xRRGGBBAA)
    pub color: u32,
    /// Border style
    pub style: BorderStyle,
}

impl Border {
    /// Create a solid border with given width and color
    pub fn solid(width: f32, color: u32) -> Self {
        Self {
            width,
            color,
            style: BorderStyle::Solid,
        }
    }
}

/// Border style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Gradient color stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    /// Position along gradient (0.0 to 1.0)
    pub position: f32,
    /// Color at this position (0xRRGGBBAA)
    pub color: u32,
}

/// Gradient specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Gradient {
    /// Linear gradient with angle in degrees (0 = right, 90 = down)
    Linear {
        angle: f32,
        stops: Vec<GradientStop>,
    },
    /// Radial gradient from center
    Radial {
        /// Center X (0.0 to 1.0, relative to rect)
        center_x: f32,
        /// Center Y (0.0 to 1.0, relative to rect)
        center_y: f32,
        stops: Vec<GradientStop>,
    },
}

impl Gradient {
    /// Create a simple horizontal gradient from left to right
    pub fn horizontal(start_color: u32, end_color: u32) -> Self {
        Gradient::Linear {
            angle: 0.0,
            stops: vec![
                GradientStop { position: 0.0, color: start_color },
                GradientStop { position: 1.0, color: end_color },
            ],
        }
    }

    /// Create a simple vertical gradient from top to bottom
    pub fn vertical(start_color: u32, end_color: u32) -> Self {
        Gradient::Linear {
            angle: 90.0,
            stops: vec![
                GradientStop { position: 0.0, color: start_color },
                GradientStop { position: 1.0, color: end_color },
            ],
        }
    }
}

/// Blend mode for compositing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    /// Normal alpha blending
    Normal,
    /// Additive blending (for glows, particles)
    Additive,
    /// Multiply blending
    Multiply,
    /// No blending (opaque)
    Opaque,
}

// ===== Render Commands =====

/// Individual render command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderCommand {
    // ===== High-Level Commands (Web UI) =====

    /// Draw a filled rectangle with full styling support
    DrawRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Fill color (0xRRGGBBAA)
        color: u32,
        /// Corner radii [top-left, top-right, bottom-right, bottom-left]
        /// Use [r, r, r, r] for uniform radius
        corner_radii: [f32; 4],
        /// Rotation angle in radians (around center), defaults to 0
        #[serde(default)]
        rotation: f32,
        /// Optional border
        border: Option<Border>,
        /// Optional gradient (overrides solid color if present)
        gradient: Option<Gradient>,
    },

    /// Draw text with full font and layout control
    DrawText {
        x: f32,
        y: f32,
        text: String,
        font: FontDescriptor,
        color: u32,
        layout: TextLayoutConfig,
    },

    /// Draw an image from a loaded texture asset
    DrawImage {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Asset ID from asset bundle
        texture_id: u32,
        /// Optional source rect for sprite sheets (x, y, w, h in texture coords 0-1)
        source_rect: Option<(f32, f32, f32, f32)>,
        /// Corner radii [top-left, top-right, bottom-right, bottom-left]
        #[serde(default)]
        corner_radii: [f32; 4],
    },

    /// Draw a sprite from a sprite sheet
    DrawSprite {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Sprite sheet asset ID
        sprite_sheet_id: u32,
        /// Sprite index in the sheet
        sprite_index: u32,
    },

    /// Draw a shadow (typically rendered before the element)
    /// Shadows should be drawn BEFORE the element they're shadowing
    DrawShadow {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Blur radius in pixels (larger = softer shadow)
        blur: f32,
        /// Shadow color (0xRRGGBBAA) - typically black with low alpha
        color: u32,
        /// Horizontal offset from the element
        offset_x: f32,
        /// Vertical offset from the element
        offset_y: f32,
        /// Corner radii to match the element shape [top-left, top-right, bottom-right, bottom-left]
        corner_radii: [f32; 4],
    },

    // ===== Low-Level Commands (Games/Performance) =====

    /// Draw raw triangles with custom vertices
    DrawTriangles {
        /// Vertex data (position + texcoord + color)
        vertices: Vec<Vertex>,
        /// Index buffer for triangle indices
        indices: Vec<u16>,
        /// Optional texture to apply
        texture_id: Option<u32>,
    },

    /// Draw instanced geometry (for particle systems, etc.)
    DrawInstanced {
        /// Base mesh/geometry ID
        mesh_id: u32,
        /// Per-instance transform matrices
        transforms: Vec<[f32; 16]>,
        /// Optional per-instance colors
        colors: Option<Vec<u32>>,
    },

    // ===== State Commands =====

    /// Begin a rectangular clip region (scissor-based, fast)
    PushClip {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },

    /// Begin a rounded clip region (stencil-based, for rounded corners)
    /// All subsequent drawing will be masked to this rounded rectangle
    PushRoundedClip {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Corner radii: [top-left, top-right, bottom-right, bottom-left]
        corner_radii: [f32; 4],
    },

    /// End the current clip region (works for both PushClip and PushRoundedClip)
    PopClip {},

    /// Begin a scroll view region
    /// This sets up clipping and applies a content offset to all subsequent draws
    /// until EndScrollView is called. Scroll views can be nested.
    BeginScrollView {
        /// Viewport X position (where the scroll view appears on screen)
        x: f32,
        /// Viewport Y position
        y: f32,
        /// Viewport width (visible area)
        width: f32,
        /// Viewport height (visible area)
        height: f32,
        /// Content offset X (how far the content is scrolled horizontally, positive = scrolled right)
        scroll_x: f32,
        /// Content offset Y (how far the content is scrolled vertically, positive = scrolled down)
        scroll_y: f32,
        /// Total content width (for scroll indicator calculations, optional)
        content_width: Option<f32>,
        /// Total content height (for scroll indicator calculations, optional)
        content_height: Option<f32>,
    },

    /// End the current scroll view region
    /// Restores the previous clip and offset state
    EndScrollView {},

    /// Set opacity for subsequent draws
    SetOpacity(f32),

    /// Set blend mode for subsequent draws
    SetBlendMode(BlendMode),

    /// Clear the screen with a color
    Clear(crate::style::Color),
}

/// Vertex structure for low-level rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Texture coordinates (u, v)
    pub texcoord: [f32; 2],
    /// Color (RGBA as packed floats for shader)
    pub color: [f32; 4],
}

impl Vertex {
    /// Create vertex from position and u32 color
    pub fn new(position: [f32; 3], texcoord: [f32; 2], color: u32) -> Self {
        // Unpack RGBA u32 (0xRRGGBBAA) to normalized floats
        let r = ((color >> 24) & 0xFF) as f32 / 255.0;
        let g = ((color >> 16) & 0xFF) as f32 / 255.0;
        let b = ((color >> 8) & 0xFF) as f32 / 255.0;
        let a = (color & 0xFF) as f32 / 255.0;

        Self {
            position,
            texcoord,
            color: [r, g, b, a],
        }
    }
}

/// Main renderer structure
pub struct Renderer {
    mode: RenderMode,
    command_buffer: CommandBuffer,
}

impl Renderer {
    pub fn new(mode: RenderMode) -> Self {
        Self {
            mode,
            command_buffer: CommandBuffer::new(),
        }
    }

    pub fn mode(&self) -> RenderMode {
        self.mode
    }

    /// Submit a frame for immediate mode rendering
    pub fn submit_frame(&mut self, commands: Vec<RenderCommand>) {
        debug_assert_eq!(self.mode, RenderMode::Immediate);
        self.command_buffer.clear();
        self.command_buffer.commands = commands;
    }

    /// Get the current command buffer
    pub fn command_buffer(&self) -> &CommandBuffer {
        &self.command_buffer
    }

    /// Execute rendering (platform implementation will override this)
    pub fn render(&mut self) {
        // Platform-specific implementation will be provided by Go layer
        // This is just the command preparation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_immediate_mode_submission() {
        let mut renderer = Renderer::new(RenderMode::Immediate);
        let commands = vec![
            RenderCommand::DrawRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                color: 0xFF0000FF,
                corner_radii: [0.0, 0.0, 0.0, 0.0],
                rotation: 0.0,
                border: None,
                gradient: None,
            },
        ];
        renderer.submit_frame(commands);
        assert_eq!(renderer.command_buffer().commands().len(), 1);
    }
}
