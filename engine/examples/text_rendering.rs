//! Cross-platform Text Rendering Example
//!
//! Demonstrates text rendering using wgpu + winit for cross-platform windowing.
//! Works on macOS, Linux, Windows, Android, and iOS.

use centered_engine::platform::wgpu_backend::{SurfaceConfig, WgpuBackend};
use centered_engine::render::RenderCommand;
use centered_engine::style::Color;
use centered_engine::text::{FontDescriptor, FontSource, FontStyle, TextLayoutConfig};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Window>,
    backend: Option<WgpuBackend>,
    frame_count: u32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window
        let window_attributes = Window::default_attributes()
            .with_title("Centered - Text Rendering Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = event_loop
            .create_window(window_attributes)
            .expect("Failed to create window");

        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        println!("Window size: {}x{}, scale: {}", size.width, size.height, scale_factor);

        // Store window first so it outlives the surface
        self.window = Some(window);

        let mut backend = WgpuBackend::new();

        // Initialize backend with window (backend creates surface from its own instance)
        let config = SurfaceConfig {
            width: size.width,
            height: size.height,
            scale_factor,
            vsync: true,
            low_power_gpu: false,
            allow_software_fallback: false,
        };

        pollster::block_on(backend.init_with_window(self.window.as_ref().unwrap(), config))
            .expect("Failed to initialize wgpu backend");

        println!("✅ wgpu backend initialized!");
        println!("🎨 Ready to render text\n");

        self.backend = Some(backend);

        // Request initial redraw to start rendering
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("\n✅ Demo complete! {} frames rendered", self.frame_count);
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(backend) = &mut self.backend {
                    // Build render commands
                    let commands = vec![
                        // Clear background
                        RenderCommand::Clear(Color {
                            r: 240,
                            g: 240,
                            b: 245,
                            a: 255,
                        }),
                        // Title
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 50.0,
                            text: "Text Rendering System".to_string(),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 700,
                                style: FontStyle::Normal,
                                size: 48.0,
                            },
                            color: 0x1A1A1AFF,
                            layout: TextLayoutConfig::default(),
                        },
                        // Subtitle
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 110.0,
                            text: "wgpu + winit • Cross-platform • GPU-accelerated".to_string(),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 400,
                                style: FontStyle::Normal,
                                size: 18.0,
                            },
                            color: 0x666666FF,
                            layout: TextLayoutConfig::default(),
                        },
                        // Feature demonstration
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 170.0,
                            text: "Font Sizes:".to_string(),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 600,
                                style: FontStyle::Normal,
                                size: 24.0,
                            },
                            color: 0x000000FF,
                            layout: TextLayoutConfig::default(),
                        },
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 210.0,
                            text: "12pt - Small text for UI labels".to_string(),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 400,
                                style: FontStyle::Normal,
                                size: 12.0,
                            },
                            color: 0x333333FF,
                            layout: TextLayoutConfig::default(),
                        },
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 235.0,
                            text: "16pt - Regular body text".to_string(),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 400,
                                style: FontStyle::Normal,
                                size: 16.0,
                            },
                            color: 0x333333FF,
                            layout: TextLayoutConfig::default(),
                        },
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 265.0,
                            text: "24pt - Section headers".to_string(),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 400,
                                style: FontStyle::Normal,
                                size: 24.0,
                            },
                            color: 0x333333FF,
                            layout: TextLayoutConfig::default(),
                        },
                        // Platform info
                        RenderCommand::DrawText {
                            x: 50.0,
                            y: 350.0,
                            text: format!("Platform: {} • Frame: {}", std::env::consts::OS, self.frame_count),
                            font: FontDescriptor {
                                source: FontSource::System("Helvetica Neue".to_string()),
                                weight: 400,
                                style: FontStyle::Normal,
                                size: 16.0,
                            },
                            color: 0x00AA00FF,
                            layout: TextLayoutConfig::default(),
                        },
                    ];

                    // Render frame
                    if let Err(e) = backend.render_frame(&commands) {
                        eprintln!("Render error: {}", e);
                    }

                    self.frame_count += 1;

                    // Print stats every 100 frames
                    if self.frame_count % 100 == 0 {
                        println!("📊 Frame {}: Rendering...", self.frame_count);
                    }
                }

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(new_size) => {
                println!("Window resized to: {}x{}", new_size.width, new_size.height);

                // Update backend with new dimensions
                if let Some(backend) = &mut self.backend {
                    if let Some(window) = &self.window {
                        let scale_factor = window.scale_factor();
                        backend.resize(new_size.width, new_size.height, scale_factor)
                            .expect("Failed to resize backend");

                        // Request redraw to show content at new size
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Centered Engine - Text Rendering Demo");
    println!("─────────────────────────────────────────");
    println!("Platform: {}", std::env::consts::OS);
    println!("\n");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        window: None,
        backend: None,
        frame_count: 0,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}
