//! Platform abstraction layer
//!
//! This module provides cross-platform rendering using wgpu, which handles
//! Metal (macOS/iOS), Vulkan (Linux/Android), Direct3D 12 (Windows), and WebGPU.

pub mod wgpu_backend;
pub mod window_styling;

pub use wgpu_backend::{SurfaceConfig, WgpuBackend};
pub use window_styling::{apply_window_style, WindowStyleOptions};
