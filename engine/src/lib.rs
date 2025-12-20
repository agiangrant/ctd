//! Centered Engine - Platform-agnostic UI rendering engine
//!
//! This library provides the core rendering, layout, and styling infrastructure
//! for the Centered UI framework. It is designed to be platform-agnostic and
//! supports both immediate mode and retained mode rendering.

// Import objc macros for macOS/iOS FFI (audio/video use AVFoundation on both platforms)
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;

// Core modules
pub mod audio;
pub mod event;
#[cfg(not(target_arch = "wasm32"))]
pub mod ffi;
pub mod geometry;
pub mod image;
pub mod layout;
pub mod platform;
pub mod render;
pub mod style;
pub mod text;
pub mod video;
pub mod widget;

// Re-exports for convenience
pub use layout::LayoutEngine;
pub use render::{RenderMode, Renderer};
pub use style::StyleSystem;
pub use widget::WidgetTree;
pub use event::EventDispatcher;

/// Engine configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineConfig {
    /// Initial width of the rendering surface
    pub width: u32,
    /// Initial height of the rendering surface
    pub height: u32,
    /// Rendering mode (immediate or retained)
    pub mode: RenderMode,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            mode: RenderMode::Retained,
        }
    }
}

/// Main engine handle
pub struct Engine {
    config: EngineConfig,
    pub layout_engine: LayoutEngine,
    pub renderer: Renderer,
    pub style_system: StyleSystem,
    pub widget_tree: WidgetTree,
    pub event_dispatcher: EventDispatcher,
}

impl Engine {
    /// Create a new engine instance with the given configuration
    pub fn new(config: EngineConfig) -> Self {
        Self {
            layout_engine: LayoutEngine::new(),
            renderer: Renderer::new(config.mode),
            style_system: StyleSystem::new(),
            widget_tree: WidgetTree::new(),
            event_dispatcher: EventDispatcher::new(),
            config,
        }
    }

    /// Get the current rendering mode
    pub fn mode(&self) -> RenderMode {
        self.config.mode
    }

    /// Resize the rendering surface
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let config = EngineConfig::default();
        let engine = Engine::new(config);
        assert_eq!(engine.mode(), RenderMode::Retained);
    }
}
