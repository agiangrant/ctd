//! Example demonstrating colored rectangle and triangle rendering
//!
//! This example showcases:
//! - DrawRect command for high-level rectangle rendering
//! - DrawTriangles command for low-level custom geometry
//! - Gradient fills (linear, radial)
//! - Multiple colors and shapes
//! - Cross-platform wgpu backend

use centered_engine::platform::wgpu_backend::{SurfaceConfig, WgpuBackend};
use centered_engine::render::{Border, Gradient, GradientStop, RenderCommand, Vertex};
use centered_engine::style::Color;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct GeometryApp {
    window: Option<Window>,
    backend: Option<WgpuBackend>,
    frame_count: u64,
}

impl GeometryApp {
    fn new() -> Self {
        Self {
            window: None,
            backend: None,
            frame_count: 0,
        }
    }
}

impl ApplicationHandler for GeometryApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window
        let window_attrs = Window::default_attributes()
            .with_title("Centered Engine - Geometry Rendering")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = event_loop.create_window(window_attrs).unwrap();
        let scale_factor = window.scale_factor();
        let size = window.inner_size();

        println!("Window created: {}x{} (scale: {})", size.width, size.height, scale_factor);

        // Initialize backend
        let mut backend = WgpuBackend::new();
        let config = SurfaceConfig {
            width: size.width,
            height: size.height,
            scale_factor,
            vsync: true,
            low_power_gpu: false,
            allow_software_fallback: false,
        };

        pollster::block_on(backend.init_with_window(&window, config))
            .expect("Failed to initialize backend");

        println!("Backend initialized");

        self.window = Some(window);
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
                println!("Close requested - {} frames rendered", self.frame_count);
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                println!("Window resized to: {}x{}", new_size.width, new_size.height);

                if let Some(backend) = &mut self.backend {
                    if let Some(window) = &self.window {
                        let scale_factor = window.scale_factor();
                        backend.resize(new_size.width, new_size.height, scale_factor)
                            .expect("Failed to resize backend");
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(backend) = &mut self.backend {
                    self.frame_count += 1;

                    // Build command list with rectangles and triangles
                    let mut commands = Vec::new();

                    // Clear to light gray
                    commands.push(RenderCommand::Clear(Color {
                        r: 240,
                        g: 240,
                        b: 245,
                        a: 255,
                    }));

                    // === Row 1: Basic Rectangles ===

                    // Blue rectangle - no rounded corners
                    commands.push(RenderCommand::DrawRect {
                        x: 50.0,
                        y: 50.0,
                        width: 150.0,
                        height: 100.0,
                        color: 0x3B82F6FF, // Blue
                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                        rotation: 0.0,
                        border: None,
                        gradient: None,
                    });

                    // Red rectangle - uniform rounded corners
                    commands.push(RenderCommand::DrawRect {
                        x: 220.0,
                        y: 50.0,
                        width: 150.0,
                        height: 100.0,
                        color: 0xEF4444FF, // Red
                        corner_radii: [16.0, 16.0, 16.0, 16.0],
                        rotation: 0.0,
                        border: None,
                        gradient: None,
                    });

                    // Green rectangle - large uniform radius (pill shape)
                    commands.push(RenderCommand::DrawRect {
                        x: 390.0,
                        y: 50.0,
                        width: 200.0,
                        height: 100.0,
                        color: 0x10B981FF, // Green
                        rotation: 0.0,
                        corner_radii: [50.0, 50.0, 50.0, 50.0], // Will be clamped to 50 (height/2)
                        border: None,
                        gradient: None,
                    });

                    // === Row 2: Per-Corner Radius ===

                    // Only top-left rounded
                    commands.push(RenderCommand::DrawRect {
                        x: 50.0,
                        y: 180.0,
                        width: 120.0,
                        height: 100.0,
                        color: 0xF59E0BFF, // Amber
                        rotation: 0.0,
                        corner_radii: [24.0, 0.0, 0.0, 0.0],
                        border: None,
                        gradient: None,
                    });

                    // Top corners rounded
                    commands.push(RenderCommand::DrawRect {
                        x: 190.0,
                        y: 180.0,
                        width: 120.0,
                        height: 100.0,
                        color: 0x8B5CF6FF, // Purple
                        rotation: 0.0,
                        corner_radii: [20.0, 20.0, 0.0, 0.0],
                        border: None,
                        gradient: None,
                    });

                    // Diagonal corners rounded (top-left, bottom-right)
                    commands.push(RenderCommand::DrawRect {
                        x: 330.0,
                        y: 180.0,
                        width: 120.0,
                        height: 100.0,
                        color: 0xEC4899FF, // Pink
                        rotation: 0.0,
                        corner_radii: [24.0, 0.0, 24.0, 0.0],
                        border: None,
                        gradient: None,
                    });

                    // Different radius per corner
                    commands.push(RenderCommand::DrawRect {
                        x: 470.0,
                        y: 180.0,
                        width: 120.0,
                        height: 100.0,
                        color: 0x06B6D4FF, // Cyan
                        rotation: 0.0,
                        corner_radii: [8.0, 16.0, 24.0, 32.0],
                        border: None,
                        gradient: None,
                    });

                    // === Row 3: Borders + Shadows ===
                    // Note: Shadows must be drawn BEFORE the rect they're shadowing

                    // Shadow for first rectangle (small, subtle)
                    commands.push(RenderCommand::DrawShadow {
                        x: 50.0,
                        y: 310.0,
                        width: 140.0,
                        height: 100.0,
                        blur: 8.0,
                        color: 0x00000040, // Black with 25% alpha
                        offset_x: 2.0,
                        offset_y: 4.0,
                        corner_radii: [12.0, 12.0, 12.0, 12.0],
                    });

                    // Rectangle with border
                    commands.push(RenderCommand::DrawRect {
                        x: 50.0,
                        y: 310.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFEF3C7FF, // Light yellow background
                        rotation: 0.0,
                        corner_radii: [12.0, 12.0, 12.0, 12.0],
                        border: Some(Border::solid(3.0, 0xF59E0BFF)), // Amber border
                        gradient: None,
                    });

                    // Shadow for second rectangle (medium, more visible)
                    commands.push(RenderCommand::DrawShadow {
                        x: 210.0,
                        y: 310.0,
                        width: 140.0,
                        height: 100.0,
                        blur: 16.0,
                        color: 0x6366F180, // Indigo with 50% alpha
                        offset_x: 4.0,
                        offset_y: 8.0,
                        corner_radii: [20.0, 20.0, 20.0, 20.0],
                    });

                    // Rounded rect with thick border
                    commands.push(RenderCommand::DrawRect {
                        x: 210.0,
                        y: 310.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xE0E7FFFF, // Light indigo background
                        rotation: 0.0,
                        corner_radii: [20.0, 20.0, 20.0, 20.0],
                        border: Some(Border::solid(4.0, 0x6366F1FF)), // Indigo border
                        gradient: None,
                    });

                    // Shadow for third rectangle (large, dramatic)
                    commands.push(RenderCommand::DrawShadow {
                        x: 370.0,
                        y: 310.0,
                        width: 140.0,
                        height: 100.0,
                        blur: 24.0,
                        color: 0xDB277780, // Pink with 50% alpha
                        offset_x: 6.0,
                        offset_y: 12.0,
                        corner_radii: [16.0, 16.0, 16.0, 16.0],
                    });

                    // Semi-transparent with border
                    commands.push(RenderCommand::DrawRect {
                        x: 370.0,
                        y: 310.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFCE7F3FF, // Solid pink (changed from semi-transparent to show shadow better)
                        rotation: 0.0,
                        corner_radii: [16.0, 16.0, 16.0, 16.0],
                        border: Some(Border::solid(2.0, 0xDB2777FF)), // Dark pink border
                        gradient: None,
                    });

                    // === Row 4: Gradients ===

                    // Linear gradient - horizontal (left to right)
                    commands.push(RenderCommand::DrawRect {
                        x: 50.0,
                        y: 440.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFFFFFFFF, // Ignored when gradient is present
                        rotation: 0.0,
                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                        border: None,
                        gradient: Some(Gradient::horizontal(0x3B82F6FF, 0x8B5CF6FF)), // Blue to purple
                    });

                    // Linear gradient - vertical (top to bottom)
                    commands.push(RenderCommand::DrawRect {
                        x: 210.0,
                        y: 440.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFFFFFFFF,
                        rotation: 0.0,
                        corner_radii: [0.0, 0.0, 0.0, 0.0],
                        border: None,
                        gradient: Some(Gradient::vertical(0xFBBF24FF, 0xEF4444FF)), // Yellow to red
                    });

                    // Linear gradient with rounded corners
                    commands.push(RenderCommand::DrawRect {
                        x: 370.0,
                        y: 440.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFFFFFFFF,
                        rotation: 0.0,
                        corner_radii: [20.0, 20.0, 20.0, 20.0],
                        border: None,
                        gradient: Some(Gradient::Linear {
                            angle: 45.0, // Diagonal
                            stops: vec![
                                GradientStop { position: 0.0, color: 0x10B981FF }, // Green
                                GradientStop { position: 0.5, color: 0x06B6D4FF }, // Cyan
                                GradientStop { position: 1.0, color: 0x3B82F6FF }, // Blue
                            ],
                        }),
                    });

                    // Radial gradient
                    commands.push(RenderCommand::DrawRect {
                        x: 530.0,
                        y: 440.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFFFFFFFF,
                        rotation: 0.0,
                        corner_radii: [16.0, 16.0, 16.0, 16.0],
                        border: None,
                        gradient: Some(Gradient::Radial {
                            center_x: 0.5,
                            center_y: 0.5,
                            stops: vec![
                                GradientStop { position: 0.0, color: 0xFBBF24FF }, // Yellow center
                                GradientStop { position: 1.0, color: 0xEF4444FF }, // Red edge
                            ],
                        }),
                    });

                    // === Row 5: Gradient + Border combinations ===

                    // Gradient with border
                    commands.push(RenderCommand::DrawRect {
                        x: 50.0,
                        y: 570.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFFFFFFFF,
                        rotation: 0.0,
                        corner_radii: [12.0, 12.0, 12.0, 12.0],
                        border: Some(Border::solid(3.0, 0x1F2937FF)), // Dark gray border
                        gradient: Some(Gradient::vertical(0xF3F4F6FF, 0xD1D5DBFF)), // Light gray gradient
                    });

                    // Rainbow gradient
                    commands.push(RenderCommand::DrawRect {
                        x: 210.0,
                        y: 570.0,
                        width: 200.0,
                        height: 100.0,
                        color: 0xFFFFFFFF,
                        rotation: 0.0,
                        corner_radii: [50.0, 50.0, 50.0, 50.0], // Pill shape
                        border: None,
                        gradient: Some(Gradient::Linear {
                            angle: 0.0, // Horizontal
                            stops: vec![
                                GradientStop { position: 0.0, color: 0xEF4444FF },  // Red
                                GradientStop { position: 0.2, color: 0xF59E0BFF },  // Orange
                                GradientStop { position: 0.4, color: 0xFBBF24FF },  // Yellow
                                GradientStop { position: 0.6, color: 0x10B981FF },  // Green
                                GradientStop { position: 0.8, color: 0x3B82F6FF },  // Blue
                                GradientStop { position: 1.0, color: 0x8B5CF6FF },  // Purple
                            ],
                        }),
                    });

                    // Radial gradient with per-corner radius
                    commands.push(RenderCommand::DrawRect {
                        x: 430.0,
                        y: 570.0,
                        width: 140.0,
                        height: 100.0,
                        color: 0xFFFFFFFF,
                        rotation: 0.0,
                        corner_radii: [30.0, 0.0, 30.0, 0.0], // Diagonal corners
                        border: Some(Border::solid(2.0, 0x6366F1FF)),
                        gradient: Some(Gradient::Radial {
                            center_x: 0.3,
                            center_y: 0.3,
                            stops: vec![
                                GradientStop { position: 0.0, color: 0xFFFFFFFF }, // White center
                                GradientStop { position: 0.5, color: 0xC7D2FEFF }, // Light indigo
                                GradientStop { position: 1.0, color: 0x6366F1FF }, // Indigo
                            ],
                        }),
                    });

                    // Draw a custom triangle using low-level DrawTriangles
                    // Yellow triangle (bottom area)
                    let triangle_vertices = vec![
                        Vertex::new([550.0, 300.0, 0.0], [0.5, 0.0], 0xFBBF24FF), // Top (yellow)
                        Vertex::new([450.0, 500.0, 0.0], [0.0, 1.0], 0xF59E0BFF), // Bottom-left (darker yellow)
                        Vertex::new([650.0, 500.0, 0.0], [1.0, 1.0], 0xF59E0BFF), // Bottom-right (darker yellow)
                    ];
                    let triangle_indices = vec![0, 1, 2];

                    commands.push(RenderCommand::DrawTriangles {
                        vertices: triangle_vertices,
                        indices: triangle_indices,
                        texture_id: None,
                    });

                    // Draw a multi-colored quad using triangles
                    // Each vertex has a different color to show gradient effect
                    let quad_vertices = vec![
                        Vertex::new([700.0, 100.0, 0.0], [0.0, 0.0], 0xFF0000FF), // Top-left: Red
                        Vertex::new([900.0, 100.0, 0.0], [1.0, 0.0], 0x00FF00FF), // Top-right: Green
                        Vertex::new([700.0, 300.0, 0.0], [0.0, 1.0], 0x0000FFFF), // Bottom-left: Blue
                        Vertex::new([900.0, 300.0, 0.0], [1.0, 1.0], 0xFFFF00FF), // Bottom-right: Yellow
                    ];
                    let quad_indices = vec![0, 1, 2, 1, 3, 2]; // Two triangles

                    commands.push(RenderCommand::DrawTriangles {
                        vertices: quad_vertices,
                        indices: quad_indices,
                        texture_id: None,
                    });

                    // Render the frame
                    if let Err(e) = backend.render_frame(&commands) {
                        eprintln!("Render error: {}", e);
                    }

                    // Request next frame for continuous rendering
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = GeometryApp::new();
    event_loop.run_app(&mut app).unwrap();
}
