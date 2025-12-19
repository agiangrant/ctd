//! Platform abstraction layer
//!
//! This module provides cross-platform rendering and window management.
//! Each platform has its own native backend implementation:
//! - macOS: AppKit via objc2
//! - iOS: UIKit via objc2
//! - Windows: Win32 (planned)
//! - Linux: X11/Wayland (planned)
//!
//! The wgpu backend handles actual GPU rendering on all platforms.

pub mod backend;
pub mod wgpu_backend;
pub mod window_styling;

// Native platform backends (bypassing winit)
// Currently iOS and Android use native backends - desktop uses winit
#[cfg(target_os = "ios")]
pub mod ios;

#[cfg(target_os = "android")]
pub mod android;

// macOS native backend - kept for reference but not used (winit works fine on macOS)
// Uncomment to use direct AppKit instead of winit
// #[cfg(target_os = "macos")]
// pub mod macos;

// Re-exports
pub use backend::{AppConfig, EventCallback, EventResponse, PlatformBackend, PlatformEvent, SafeAreaInsets};
pub use wgpu_backend::{SurfaceConfig, WgpuBackend};
pub use window_styling::{apply_window_style, WindowStyleOptions};

// Platform-specific backend alias (iOS and Android use native backends, others use winit)
#[cfg(target_os = "ios")]
pub use ios::IosBackend as NativeBackend;

#[cfg(target_os = "android")]
pub use android::AndroidBackend as NativeBackend;
