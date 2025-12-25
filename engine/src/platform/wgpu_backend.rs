//! wgpu-based cross-platform rendering backend
//!
//! This backend uses wgpu for cross-platform rendering (Metal, Vulkan, D3D12, WebGPU).
//! It handles text rendering using our glyph atlas system.

use crate::image::LoadedImage;
use crate::render::RenderCommand;
use crate::text::atlas::{GlyphAtlas, GlyphRasterizer};
use crate::text::{FontDescriptor, TextLayoutConfig, TextAlign, WhiteSpace, WordBreak, TextOverflow};
use std::collections::HashMap;
use std::error::Error;
use wgpu::util::DeviceExt;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::text::atlas::MacOSGlyphRasterizer;

#[cfg(target_os = "android")]
use crate::text::atlas::AndroidGlyphRasterizer;

#[cfg(target_os = "linux")]
use crate::text::atlas::LinuxGlyphRasterizer;

#[cfg(target_os = "windows")]
use crate::text::atlas::WindowsGlyphRasterizer;


/// Surface configuration for wgpu
pub struct SurfaceConfig {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub vsync: bool,
    pub low_power_gpu: bool,
    pub allow_software_fallback: bool,
}

/// Scissor rect for clipping
#[derive(Debug, Clone, Copy)]
struct ScissorRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Clamp a scissor rect to fit within the viewport bounds.
/// This prevents wgpu validation errors when scissor rects extend beyond the render target.
fn clamp_scissor_to_viewport(rect: ScissorRect, viewport_width: u32, viewport_height: u32) -> ScissorRect {
    // If the rect starts beyond the viewport, return an empty rect at the edge
    if rect.x >= viewport_width || rect.y >= viewport_height {
        return ScissorRect {
            x: viewport_width.saturating_sub(1),
            y: viewport_height.saturating_sub(1),
            width: 1,
            height: 1,
        };
    }

    // Clamp the right and bottom edges to the viewport
    let clamped_width = rect.width.min(viewport_width.saturating_sub(rect.x));
    let clamped_height = rect.height.min(viewport_height.saturating_sub(rect.y));

    ScissorRect {
        x: rect.x,
        y: rect.y,
        width: clamped_width.max(1),
        height: clamped_height.max(1),
    }
}

/// Scroll offset for scroll views (in logical pixels)
#[derive(Debug, Clone, Copy)]
struct ScrollOffset {
    /// Viewport position X (logical pixels)
    viewport_x: f32,
    /// Viewport position Y (logical pixels)
    viewport_y: f32,
    /// Content offset X (positive = scrolled right, content moves left)
    offset_x: f32,
    /// Content offset Y (positive = scrolled down, content moves up)
    offset_y: f32,
}

/// GPU texture resource for loaded images
struct GpuTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// Stencil clip state for rounded corner clipping
#[derive(Debug, Clone)]
struct StencilClipState {
    /// Whether stencil clipping is active
    active: bool,
    /// The rounded clip region (x, y, width, height, corner_radii)
    region: Option<(f32, f32, f32, f32, [f32; 4])>,
}

impl Default for StencilClipState {
    fn default() -> Self {
        Self {
            active: false,
            region: None,
        }
    }
}

/// wgpu rendering backend
pub struct WgpuBackend {
    // wgpu core
    instance: wgpu::Instance,
    adapter: Option<wgpu::Adapter>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,

    // Stencil buffer for rounded corner clipping
    stencil_texture: Option<wgpu::Texture>,
    stencil_view: Option<wgpu::TextureView>,
    stencil_pipeline: Option<wgpu::RenderPipeline>,
    stencil_clip_state: StencilClipState,

    // Render pipeline for text
    text_pipeline: Option<wgpu::RenderPipeline>,
    text_bind_group: Option<wgpu::BindGroup>,
    atlas_texture: Option<wgpu::Texture>,

    // Render pipeline for colored geometry (triangles, rectangles)
    geometry_pipeline: Option<wgpu::RenderPipeline>,

    // Glyph atlas (platform-specific rasterizers)
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    glyph_atlas: GlyphAtlas,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    rasterizer: MacOSGlyphRasterizer,

    #[cfg(target_os = "android")]
    glyph_atlas: GlyphAtlas,
    #[cfg(target_os = "android")]
    rasterizer: AndroidGlyphRasterizer,

    #[cfg(target_os = "linux")]
    glyph_atlas: GlyphAtlas,
    #[cfg(target_os = "linux")]
    rasterizer: LinuxGlyphRasterizer,

    #[cfg(target_os = "windows")]
    glyph_atlas: GlyphAtlas,
    #[cfg(target_os = "windows")]
    rasterizer: WindowsGlyphRasterizer,

    // Configuration
    width: u32,
    height: u32,
    scale_factor: f64,

    // Clipping state - stack of scissor rects for nested clipping
    scissor_stack: Vec<ScissorRect>,

    // Scroll view state - stack of scroll offsets for nested scroll views
    scroll_offset_stack: Vec<ScrollOffset>,

    // Image textures - map from texture_id to GPU texture
    image_textures: HashMap<u32, GpuTexture>,
    image_pipeline: Option<wgpu::RenderPipeline>,
    image_bind_group_layout: Option<wgpu::BindGroupLayout>,
    next_texture_id: u32,
}

impl WgpuBackend {
    pub fn new() -> Self {
        // Create wgpu instance
        // On Android, prefer Vulkan to avoid EGL surface conflicts with NativeActivity
        #[cfg(target_os = "android")]
        let backends = wgpu::Backends::VULKAN;
        #[cfg(not(target_os = "android"))]
        let backends = wgpu::Backends::all();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        Self {
            instance,
            adapter: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            text_pipeline: None,
            text_bind_group: None,
            atlas_texture: None,
            geometry_pipeline: None,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            glyph_atlas: GlyphAtlas::new(2048, 2048),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            rasterizer: MacOSGlyphRasterizer::new(),
            #[cfg(target_os = "android")]
            glyph_atlas: GlyphAtlas::new(2048, 2048),
            #[cfg(target_os = "android")]
            rasterizer: AndroidGlyphRasterizer::new(),
            #[cfg(target_os = "linux")]
            glyph_atlas: GlyphAtlas::new(2048, 2048),
            #[cfg(target_os = "linux")]
            rasterizer: LinuxGlyphRasterizer::new(),
            #[cfg(target_os = "windows")]
            glyph_atlas: GlyphAtlas::new(2048, 2048),
            #[cfg(target_os = "windows")]
            rasterizer: WindowsGlyphRasterizer::new(),
            width: 0,
            height: 0,
            scale_factor: 1.0,
            scissor_stack: Vec::new(),
            scroll_offset_stack: Vec::new(),
            image_textures: HashMap::new(),
            image_pipeline: None,
            image_bind_group_layout: None,
            next_texture_id: 1,
            stencil_texture: None,
            stencil_view: None,
            stencil_pipeline: None,
            stencil_clip_state: StencilClipState::default(),
        }
    }

    /// Get the current scale factor (for HiDPI displays)
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Measure the width of a string using the rasterizer
    #[cfg(target_os = "windows")]
    pub fn measure_string(&mut self, text: &str, font: &crate::text::FontDescriptor) -> f32 {
        self.rasterizer.measure_string(text, font)
    }

    /// Get font metrics (ascent, descent) for a given font
    #[cfg(target_os = "windows")]
    pub fn get_font_metrics(&mut self, font: &crate::text::FontDescriptor) -> (f32, f32) {
        self.rasterizer.get_font_metrics(font)
    }

    /// Initialize the backend with a window (creates surface internally)
    pub async fn init_with_window<W: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Sync>(
        &mut self,
        window: &W,
        config: SurfaceConfig,
    ) -> Result<(), Box<dyn Error>> {
        // Create surface from window using our instance
        // SAFETY: The surface lifetime is tied to the window's lifetime.
        // The caller must ensure the window outlives the surface.
        // This is standard practice in wgpu+winit applications.
        let surface = unsafe {
            let surface = self.instance.create_surface(window)?;
            std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface)
        };
        self.init_with_surface(surface, config).await
    }

    /// Initialize the backend with a surface
    ///
    /// This is a simplified version that requires the caller to provide a wgpu::Surface
    pub async fn init_with_surface(
        &mut self,
        surface: wgpu::Surface<'static>,
        config: SurfaceConfig,
    ) -> Result<(), Box<dyn Error>> {
        self.width = config.width;
        self.height = config.height;
        self.scale_factor = config.scale_factor;

        // Request adapter with configured power preference
        let power_preference = if config.low_power_gpu {
            wgpu::PowerPreference::LowPower
        } else {
            wgpu::PowerPreference::HighPerformance
        };

        let adapter = self.instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                compatible_surface: Some(&surface),
                force_fallback_adapter: config.allow_software_fallback,
            })
            .await
            .ok_or("Failed to find suitable GPU adapter. Set allow_software_fallback=true to enable software rendering.")?;

        println!("wgpu adapter: {:?}", adapter.get_info());

        // Request device and queue
        // Use downlevel limits for emulators (GLES), full limits for real devices (Vulkan)
        let adapter_info = adapter.get_info();
        let is_emulator = adapter_info.name.contains("Emulator")
            || adapter_info.name.contains("SwiftShader")
            || adapter_info.backend == wgpu::Backend::Gl;

        let required_limits = if is_emulator {
            // Emulators often use GLES which doesn't support compute shaders
            wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits())
        } else {
            // Real devices with Vulkan support full limits
            wgpu::Limits::default()
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Centered Engine Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await?;

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Prefer alpha modes that support transparency (PreMultiplied > PostMultiplied > Auto > Opaque)
        let alpha_mode = surface_caps.alpha_modes.iter()
            .find(|m| **m == wgpu::CompositeAlphaMode::PreMultiplied)
            .or_else(|| surface_caps.alpha_modes.iter().find(|m| **m == wgpu::CompositeAlphaMode::PostMultiplied))
            .or_else(|| surface_caps.alpha_modes.iter().find(|m| **m == wgpu::CompositeAlphaMode::Auto))
            .copied()
            .unwrap_or(surface_caps.alpha_modes[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: config.width,
            height: config.height,
            present_mode: if config.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        // Create atlas texture
        let atlas_texture = self.create_atlas_texture(&device)?;

        // Create text rendering pipeline
        let (text_pipeline, text_bind_group) = self.create_text_pipeline(
            &device,
            &surface_config,
            &atlas_texture,
        )?;

        // Create geometry rendering pipeline
        let geometry_pipeline = self.create_geometry_pipeline(&device, &surface_config)?;

        // Create image rendering pipeline
        let (image_pipeline, image_bind_group_layout) = self.create_image_pipeline(&device, &surface_config)?;

        // Create stencil texture and pipeline for rounded corner clipping
        let (stencil_texture, stencil_view) = self.create_stencil_texture(&device, config.width, config.height);
        let stencil_pipeline = self.create_stencil_pipeline(&device, &surface_config)?;

        self.adapter = Some(adapter);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);
        self.atlas_texture = Some(atlas_texture);
        self.text_pipeline = Some(text_pipeline);
        self.text_bind_group = Some(text_bind_group);
        self.geometry_pipeline = Some(geometry_pipeline);
        self.image_pipeline = Some(image_pipeline);
        self.image_bind_group_layout = Some(image_bind_group_layout);
        self.stencil_texture = Some(stencil_texture);
        self.stencil_view = Some(stencil_view);
        self.stencil_pipeline = Some(stencil_pipeline);

        Ok(())
    }

    fn create_atlas_texture(&self, device: &wgpu::Device) -> Result<wgpu::Texture, Box<dyn Error>> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas Texture"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        Ok(texture)
    }

    /// Create or recreate the stencil texture for rounded corner clipping
    fn create_stencil_texture(&self, device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Stencil Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Stencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Create the stencil-write pipeline for drawing rounded rect masks
    fn create_stencil_pipeline(
        &self,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Result<wgpu::RenderPipeline, Box<dyn Error>> {
        // Shader that outputs a dummy color (write_mask prevents actual writes)
        let shader_source = r#"
            struct VertexInput {
                @location(0) position: vec2<f32>,
            }

            struct VertexOutput {
                @builtin(position) clip_position: vec4<f32>,
            }

            @vertex
            fn vs_main(in: VertexInput) -> VertexOutput {
                var out: VertexOutput;
                // Position is already in clip space (-1 to 1)
                out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
                return out;
            }

            @fragment
            fn fs_main() -> @location(0) vec4<f32> {
                // Dummy color output (write_mask prevents actual writes)
                return vec4<f32>(0.0, 0.0, 0.0, 0.0);
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Stencil Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Stencil Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stencil Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 8, // 2 floats * 4 bytes
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                // Match surface format but don't write any color (stencil-only)
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(), // Don't write to color buffer
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace, // Write reference value
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    read_mask: 0xFF,
                    write_mask: 0xFF,
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }

    fn create_text_pipeline(
        &self,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        atlas_texture: &wgpu::Texture,
    ) -> Result<(wgpu::RenderPipeline, wgpu::BindGroup), Box<dyn Error>> {
        // Create texture view and sampler
        let texture_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Shader source
        let shader_source = include_str!("shaders/text.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,  // position
                        1 => Float32x2,  // tex_coords
                        2 => Float32x4,  // color
                        3 => Float32,    // use_texture_color (1.0 for emoji, 0.0 for text)
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            // Stencil testing for rounded corner clipping
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal, // Only draw where stencil == reference
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0x00, // Don't write to stencil
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok((pipeline, bind_group))
    }

    /// Create the geometry rendering pipeline for colored triangles and rectangles
    fn create_geometry_pipeline(
        &self,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Result<wgpu::RenderPipeline, Box<dyn Error>> {
        // Shader source
        let shader_source = include_str!("shaders/geometry.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geometry Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Pipeline layout (no bind groups needed for colored geometry)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GeometryVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3,  // position
                        1 => Float32x2,  // texcoord (for future texture support)
                        2 => Float32x4,  // color
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            // Stencil testing for rounded corner clipping
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal, // Only draw where stencil == reference
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0x00, // Don't write to stencil
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create the image rendering pipeline
    fn create_image_pipeline(
        &self,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> Result<(wgpu::RenderPipeline, wgpu::BindGroupLayout), Box<dyn Error>> {
        // Create bind group layout for image texture
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Shader source
        let shader_source = include_str!("shaders/image.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline (uses same vertex layout as text)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,  // position
                        1 => Float32x2,  // tex_coords
                        2 => Float32x4,  // color
                        3 => Float32,    // use_texture_color (unused for images, but needed for struct alignment)
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            // Stencil testing for rounded corner clipping
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Stencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal, // Only draw where stencil == reference
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Equal,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0x00, // Don't write to stencil
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok((pipeline, bind_group_layout))
    }

    /// Load an image from bytes and return its texture ID
    pub fn load_image(&mut self, image: &LoadedImage) -> Result<u32, Box<dyn Error>> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let queue = self.queue.as_ref().ok_or("Queue not initialized")?;
        let bind_group_layout = self.image_bind_group_layout.as_ref().ok_or("Image bind group layout not initialized")?;

        // Create GPU texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Image Texture"),
            size: wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload pixel data
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );

        // Create texture view and sampler
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group for this texture
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Image Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Assign texture ID and store
        let texture_id = self.next_texture_id;
        self.next_texture_id += 1;

        self.image_textures.insert(texture_id, GpuTexture {
            texture,
            bind_group,
            width: image.width,
            height: image.height,
        });

        Ok(texture_id)
    }

    /// Unload an image texture
    pub fn unload_image(&mut self, texture_id: u32) {
        self.image_textures.remove(&texture_id);
    }

    /// Update an existing texture with new image data (for video/camera frames)
    /// This avoids the overhead of creating new textures for each frame.
    /// If the dimensions don't match, creates a new texture.
    pub fn update_texture(&mut self, texture_id: u32, image: &LoadedImage) -> Result<u32, Box<dyn Error>> {
        let queue = self.queue.as_ref().ok_or("Queue not initialized")?;

        // Check if we can update in-place (same dimensions)
        if let Some(existing) = self.image_textures.get(&texture_id) {
            if existing.width == image.width && existing.height == image.height {
                // Update existing texture in-place
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &existing.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &image.data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(image.width * 4),
                        rows_per_image: Some(image.height),
                    },
                    wgpu::Extent3d {
                        width: image.width,
                        height: image.height,
                        depth_or_array_layers: 1,
                    },
                );
                return Ok(texture_id);
            }
        }

        // Dimensions changed or texture doesn't exist - create new one
        // (remove old one first if it exists)
        self.image_textures.remove(&texture_id);
        self.load_image(image)
    }

    /// Get texture dimensions for a loaded image
    pub fn get_texture_size(&self, texture_id: u32) -> Option<(u32, u32)> {
        self.image_textures.get(&texture_id).map(|tex| (tex.width, tex.height))
    }

    /// Get the current window width in pixels
    pub fn get_width(&self) -> u32 {
        self.width
    }

    /// Get the current window height in pixels
    pub fn get_height(&self) -> u32 {
        self.height
    }

    /// Create a video texture that can be updated each frame
    ///
    /// Returns a texture ID that can be used with update_video_texture and DrawImage commands.
    /// Unlike regular image textures, video textures are optimized for frequent updates.
    pub fn create_video_texture(&mut self, width: u32, height: u32) -> Result<u32, Box<dyn Error>> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let bind_group_layout = self.image_bind_group_layout.as_ref().ok_or("Image bind group layout not initialized")?;

        // Create GPU texture with COPY_DST for frequent updates
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create texture view and sampler
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Video Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group for this texture
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Video Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Assign texture ID and store
        let texture_id = self.next_texture_id;
        self.next_texture_id += 1;

        self.image_textures.insert(texture_id, GpuTexture {
            texture,
            bind_group,
            width,
            height,
        });

        Ok(texture_id)
    }

    /// Update a video texture with new frame data
    ///
    /// This is optimized for frequent updates during video playback.
    /// The frame data must be RGBA format with width * height * 4 bytes.
    pub fn update_video_texture(
        &mut self,
        texture_id: u32,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let queue = self.queue.as_ref().ok_or("Queue not initialized")?;
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let bind_group_layout = self.image_bind_group_layout.as_ref().ok_or("Image bind group layout not initialized")?;

        let gpu_texture = self.image_textures.get_mut(&texture_id)
            .ok_or("Texture not found")?;

        // Check if we need to recreate the texture (size changed)
        if gpu_texture.width != width || gpu_texture.height != height {
            // Create new texture with new size
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Video Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // Create new bind group
            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Video Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Video Bind Group"),
                layout: bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            gpu_texture.texture = texture;
            gpu_texture.bind_group = bind_group;
            gpu_texture.width = width;
            gpu_texture.height = height;
        }

        // Upload new frame data
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &gpu_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Upload atlas texture to GPU if dirty
    fn upload_atlas_if_needed(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux", target_os = "windows"))]
        {
            if self.glyph_atlas.is_dirty() {
                let queue = self.queue.as_ref().ok_or("Queue not initialized")?;
                let atlas_texture = self.atlas_texture.as_ref().ok_or("Atlas texture not initialized")?;

                let (width, height) = self.glyph_atlas.dimensions();
                let texture_data = self.glyph_atlas.texture_data();

                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: atlas_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    texture_data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );

                self.glyph_atlas.mark_clean();
            }
        }
        Ok(())
    }

    /// Convert screen coordinates to NDC (Normalized Device Coordinates)
    fn screen_to_ndc(&self, x: f32, y: f32) -> [f32; 2] {
        let ndc_x = (x / self.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (y / self.height as f32) * 2.0; // Y is flipped in NDC
        [ndc_x, ndc_y]
    }

    /// Render a frame with the given commands
    pub fn render_frame(&mut self, commands: &[RenderCommand]) -> Result<(), Box<dyn Error>> {
        let surface = self.surface.as_ref().ok_or("Surface not initialized")?;
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        // Get the current surface texture
        let frame = surface.get_current_texture()?;

        // Use the ACTUAL frame texture size for rendering, not our cached size.
        // This is critical for iOS rotation where the texture size may differ from
        // our cached dimensions during the transition.
        let actual_width = frame.texture.width();
        let actual_height = frame.texture.height();

        // If the frame size differs from our cached size, update our stored dimensions.
        // This ensures coordinate transformations use the correct values.
        if actual_width != self.width || actual_height != self.height {
            println!("[wgpu] Frame texture size mismatch: cached {}x{}, actual {}x{} - updating",
                self.width, self.height, actual_width, actual_height);
            self.width = actual_width;
            self.height = actual_height;

            // Also reconfigure the surface with the correct size
            if let Some(config) = &mut self.surface_config {
                config.width = actual_width.max(1);
                config.height = actual_height.max(1);
                surface.configure(device, config);
            }

            // Recreate stencil texture with new dimensions
            let (stencil_texture, stencil_view) = self.create_stencil_texture(device, actual_width.max(1), actual_height.max(1));
            self.stencil_texture = Some(stencil_texture);
            self.stencil_view = Some(stencil_view);
        }

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Determine clear color from commands
        let clear_color = commands.iter()
            .find_map(|cmd| {
                if let RenderCommand::Clear(color) = cmd {
                    Some(wgpu::Color {
                        r: (color.r as f64) / 255.0,
                        g: (color.g as f64) / 255.0,
                        b: (color.b as f64) / 255.0,
                        a: (color.a as f64) / 255.0,
                    })
                } else {
                    None
                }
            })
            .unwrap_or(wgpu::Color::BLACK);

        // Get stencil view reference for render pass
        let stencil_view = self.stencil_view.as_ref().ok_or("Stencil view not initialized")?;

        // Begin render pass with stencil attachment
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: stencil_view,
                    depth_ops: None, // No depth buffer
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0), // Clear stencil to 0
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set initial stencil reference to 0 (stencil buffer is 0, so all pixels pass)
            render_pass.set_stencil_reference(0);

            // Clear scissor stack and scroll offset stack at start of frame
            self.scissor_stack.clear();
            self.scroll_offset_stack.clear();
            self.stencil_clip_state = StencilClipState::default();

            // Set initial scissor to full viewport using actual frame dimensions
            let scale = self.scale_factor as f32;
            let full_width = actual_width;
            let full_height = actual_height;
            render_pass.set_scissor_rect(0, 0, full_width, full_height);

            // Process render commands
            for cmd in commands {
                match cmd {
                    RenderCommand::PushClip { x, y, width, height } => {
                        // Convert to physical pixels and apply scale
                        let clip_x = (*x * scale) as u32;
                        let clip_y = (*y * scale) as u32;
                        let clip_w = (*width * scale) as u32;
                        let clip_h = (*height * scale) as u32;

                        // If we have a parent clip, intersect with it
                        let new_rect = if let Some(parent) = self.scissor_stack.last() {
                            // Calculate intersection
                            let int_x = clip_x.max(parent.x);
                            let int_y = clip_y.max(parent.y);
                            let parent_right = parent.x.saturating_add(parent.width);
                            let parent_bottom = parent.y.saturating_add(parent.height);
                            let clip_right = clip_x.saturating_add(clip_w);
                            let clip_bottom = clip_y.saturating_add(clip_h);
                            let int_right = clip_right.min(parent_right);
                            let int_bottom = clip_bottom.min(parent_bottom);

                            ScissorRect {
                                x: int_x,
                                y: int_y,
                                width: int_right.saturating_sub(int_x),
                                height: int_bottom.saturating_sub(int_y),
                            }
                        } else {
                            ScissorRect {
                                x: clip_x,
                                y: clip_y,
                                width: clip_w,
                                height: clip_h,
                            }
                        };

                        // Clamp to render target bounds to avoid wgpu validation errors
                        let clamped_rect = clamp_scissor_to_viewport(new_rect, full_width, full_height);

                        // Push to stack and apply
                        self.scissor_stack.push(clamped_rect);
                        render_pass.set_scissor_rect(clamped_rect.x, clamped_rect.y, clamped_rect.width.max(1), clamped_rect.height.max(1));
                    }
                    RenderCommand::PopClip {} => {
                        // Check if this is a stencil clip pop
                        if self.stencil_clip_state.active {
                            // Restore stencil reference to 0 (no clipping)
                            // Note: The stencil buffer still has 1s from the mask, but with
                            // reference = 0, nothing will pass. For proper nesting, we'd need
                            // to clear the stencil by drawing a fullscreen quad.
                            // For MVP window-level clipping, we don't pop until frame end.
                            render_pass.set_stencil_reference(0);
                            self.stencil_clip_state.active = false;
                            self.stencil_clip_state.region = None;
                        } else {
                            // Regular scissor pop
                            self.scissor_stack.pop();

                            // Restore parent scissor or full viewport
                            if let Some(parent) = self.scissor_stack.last() {
                                render_pass.set_scissor_rect(parent.x, parent.y, parent.width.max(1), parent.height.max(1));
                            } else {
                                render_pass.set_scissor_rect(0, 0, full_width, full_height);
                            }
                        }
                    }
                    RenderCommand::PushRoundedClip { x, y, width, height, corner_radii } => {
                        // Draw rounded rectangle mask to stencil buffer
                        self.render_stencil_mask(&mut render_pass, *x, *y, *width, *height, *corner_radii)?;

                        // After drawing the mask, set stencil reference to 1
                        // Content pipelines test: stencil == reference
                        // Now only pixels inside the rounded rect (stencil = 1) will pass
                        render_pass.set_stencil_reference(1);

                        // Track stencil clip state
                        self.stencil_clip_state.active = true;
                        self.stencil_clip_state.region = Some((*x, *y, *width, *height, *corner_radii));
                    }
                    RenderCommand::BeginScrollView { x, y, width, height, scroll_x, scroll_y, .. } => {
                        // Calculate scroll offset from EXISTING parent scroll views
                        // (before pushing this new one). This is needed to position the
                        // clip rect correctly when nested inside other scroll views.
                        let (parent_scroll_dx, parent_scroll_dy) = self.scroll_offset_stack.iter().fold((0.0f32, 0.0f32), |(dx, dy), s| {
                            (dx - s.offset_x, dy - s.offset_y)
                        });

                        // Push scroll offset to the stack
                        self.scroll_offset_stack.push(ScrollOffset {
                            viewport_x: *x,
                            viewport_y: *y,
                            offset_x: *scroll_x,
                            offset_y: *scroll_y,
                        });

                        // Set up clipping for the scroll view viewport
                        // Apply parent scroll offset to position, then convert to physical pixels
                        let adjusted_x = *x + parent_scroll_dx;
                        let adjusted_y = *y + parent_scroll_dy;

                        // Calculate the clip rect, accounting for partial visibility
                        // When the container is partially scrolled out of view, we need to
                        // reduce the clip size accordingly.
                        let clip_x: u32;
                        let clip_w: u32;
                        if adjusted_x < 0.0 {
                            // Container starts above/left of viewport - reduce width
                            clip_x = 0;
                            let visible_w = (*width + adjusted_x) * scale;
                            clip_w = if visible_w > 0.0 { visible_w as u32 } else { 0 };
                        } else {
                            clip_x = (adjusted_x * scale) as u32;
                            clip_w = (*width * scale) as u32;
                        }

                        let clip_y: u32;
                        let clip_h: u32;
                        if adjusted_y < 0.0 {
                            // Container starts above viewport - reduce height
                            clip_y = 0;
                            let visible_h = (*height + adjusted_y) * scale;
                            clip_h = if visible_h > 0.0 { visible_h as u32 } else { 0 };
                        } else {
                            clip_y = (adjusted_y * scale) as u32;
                            clip_h = (*height * scale) as u32;
                        }

                        // If we have a parent clip, intersect with it
                        let new_rect = if let Some(parent) = self.scissor_stack.last() {
                            let int_x = clip_x.max(parent.x);
                            let int_y = clip_y.max(parent.y);
                            let int_right = (clip_x + clip_w).min(parent.x + parent.width);
                            let int_bottom = (clip_y + clip_h).min(parent.y + parent.height);
                            let int_w = if int_right > int_x { int_right - int_x } else { 0 };
                            let int_h = if int_bottom > int_y { int_bottom - int_y } else { 0 };
                            ScissorRect { x: int_x, y: int_y, width: int_w, height: int_h }
                        } else {
                            ScissorRect { x: clip_x, y: clip_y, width: clip_w, height: clip_h }
                        };

                        // Clamp to render target bounds to avoid wgpu validation errors
                        let clamped_rect = clamp_scissor_to_viewport(new_rect, full_width, full_height);

                        self.scissor_stack.push(clamped_rect);
                        render_pass.set_scissor_rect(clamped_rect.x, clamped_rect.y, clamped_rect.width.max(1), clamped_rect.height.max(1));
                    }
                    RenderCommand::EndScrollView {} => {
                        // Pop scroll offset
                        self.scroll_offset_stack.pop();

                        // Pop scissor (same as PopClip)
                        self.scissor_stack.pop();
                        if let Some(parent) = self.scissor_stack.last() {
                            render_pass.set_scissor_rect(parent.x, parent.y, parent.width.max(1), parent.height.max(1));
                        } else {
                            render_pass.set_scissor_rect(0, 0, full_width, full_height);
                        }
                    }
                    RenderCommand::DrawShadow { x, y, width, height, blur, color, offset_x, offset_y, corner_radii } => {
                        // Apply scroll offset: subtract scroll position so content moves up/left when scrolling down/right
                        let (scroll_dx, scroll_dy) = self.scroll_offset_stack.iter().fold((0.0f32, 0.0f32), |(dx, dy), s| {
                            (dx - s.offset_x, dy - s.offset_y)
                        });
                        self.render_shadow(&mut render_pass, *x + scroll_dx, *y + scroll_dy, *width, *height, *blur, *color, *offset_x, *offset_y, *corner_radii)?;
                    }
                    RenderCommand::DrawRect { x, y, width, height, color, corner_radii, rotation, border, gradient } => {
                        // Apply scroll offset
                        let (scroll_dx, scroll_dy) = self.scroll_offset_stack.iter().fold((0.0f32, 0.0f32), |(dx, dy), s| {
                            (dx - s.offset_x, dy - s.offset_y)
                        });
                        self.render_rect(&mut render_pass, *x + scroll_dx, *y + scroll_dy, *width, *height, *color, *corner_radii, *rotation, border.as_ref(), gradient.as_ref())?;
                    }
                    RenderCommand::DrawTriangles { vertices, indices, .. } => {
                        // Note: DrawTriangles would need vertex transformation for scroll, skipping for now
                        self.render_triangles(&mut render_pass, vertices, indices)?;
                    }
                    RenderCommand::DrawText { x, y, text, font, color, layout } => {
                        // Apply scroll offset
                        let (scroll_dx, scroll_dy) = self.scroll_offset_stack.iter().fold((0.0f32, 0.0f32), |(dx, dy), s| {
                            (dx - s.offset_x, dy - s.offset_y)
                        });
                        self.render_text(&mut render_pass, *x + scroll_dx, *y + scroll_dy, text, font, *color, layout)?;
                    }
                    RenderCommand::DrawImage { x, y, width, height, texture_id, source_rect, corner_radii } => {
                        // Apply scroll offset
                        let (scroll_dx, scroll_dy) = self.scroll_offset_stack.iter().fold((0.0f32, 0.0f32), |(dx, dy), s| {
                            (dx - s.offset_x, dy - s.offset_y)
                        });
                        self.render_image(&mut render_pass, *x + scroll_dx, *y + scroll_dy, *width, *height, *texture_id, source_rect.clone(), *corner_radii)?;
                    }
                    _ => {
                        // Ignore other commands for now
                    }
                }
            }
        }

        // Submit commands (get queue here to avoid borrow conflict)
        let queue = self.queue.as_ref().ok_or("Queue not initialized")?;
        queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    /// Handle window resize - reconfigure surface with new dimensions
    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) -> Result<(), Box<dyn Error>> {
        // Update stored dimensions
        self.width = width;
        self.height = height;
        self.scale_factor = scale_factor;

        // Reconfigure the surface with new size
        if let (Some(surface), Some(device), Some(config)) =
            (&self.surface, &self.device, &mut self.surface_config) {
            config.width = width.max(1);  // Ensure at least 1x1
            config.height = height.max(1);
            surface.configure(device, config);
        }

        // Recreate stencil texture with new dimensions (separate block to avoid borrow conflict)
        if let Some(device) = &self.device {
            let stencil_width = width.max(1);
            let stencil_height = height.max(1);
            let (stencil_texture, stencil_view) = self.create_stencil_texture(device, stencil_width, stencil_height);
            self.stencil_texture = Some(stencil_texture);
            self.stencil_view = Some(stencil_view);
        }

        Ok(())
    }

    /// Render raw triangles with custom vertices
    fn render_triangles(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        vertices: &[crate::render::Vertex],
        indices: &[u16],
    ) -> Result<(), Box<dyn Error>> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let pipeline = self.geometry_pipeline.as_ref().ok_or("Geometry pipeline not initialized")?;

        // Convert render::Vertex to GeometryVertex (they have the same layout)
        let geometry_vertices: Vec<GeometryVertex> = vertices.iter().map(|v| {
            GeometryVertex {
                position: v.position,
                texcoord: v.texcoord,
                color: v.color,
            }
        }).collect();

        // Create vertex buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Triangle Vertex Buffer"),
            contents: bytemuck::cast_slice(&geometry_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create index buffer
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Triangle Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Set pipeline and buffers
        render_pass.set_pipeline(pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // Draw indexed triangles
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

        Ok(())
    }

    /// Render a rounded rectangle to the stencil buffer for clipping
    #[allow(clippy::too_many_arguments)]
    fn render_stencil_mask(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radii: [f32; 4],
    ) -> Result<(), Box<dyn Error>> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let pipeline = self.stencil_pipeline.as_ref().ok_or("Stencil pipeline not initialized")?;

        // Scale coordinates for HiDPI
        let scale = self.scale_factor as f32;
        let scaled_x = x * scale;
        let scaled_y = y * scale;
        let scaled_width = width * scale;
        let scaled_height = height * scale;
        let scaled_radii = [
            corner_radii[0] * scale,
            corner_radii[1] * scale,
            corner_radii[2] * scale,
            corner_radii[3] * scale,
        ];

        // Generate rounded rect geometry (we only need positions, color is ignored)
        let (vertices, indices) = crate::geometry::rounded_rect(
            scaled_x,
            scaled_y,
            scaled_width,
            scaled_height,
            0xFFFFFFFF, // Color doesn't matter for stencil
            scaled_radii,
        );

        // Convert to NDC coordinates (stencil pipeline only uses position.xy)
        let ndc_positions: Vec<[f32; 2]> = vertices.iter().map(|v| {
            let ndc = self.screen_to_ndc(v.position[0], v.position[1]);
            [ndc[0], ndc[1]]
        }).collect();

        // Create vertex buffer with just positions
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Stencil Vertex Buffer"),
            contents: bytemuck::cast_slice(&ndc_positions),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create index buffer
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Stencil Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Set stencil reference to 1 - this is what gets written to the stencil buffer
        render_pass.set_stencil_reference(1);

        // Use stencil pipeline and draw
        render_pass.set_pipeline(pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

        Ok(())
    }

    /// Render a rectangle with optional rounded corners, border, gradient, and rotation
    #[allow(clippy::too_many_arguments)]
    fn render_rect(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: u32,
        corner_radii: [f32; 4],
        rotation: f32,
        border: Option<&crate::render::Border>,
        gradient: Option<&crate::render::Gradient>,
    ) -> Result<(), Box<dyn Error>> {
        // Scale coordinates for HiDPI
        // Floor positions to align with pixel boundaries (matches scissor rect truncation)
        // This prevents sub-pixel gaps at scissor edges, especially for sticky headers
        let scale = self.scale_factor as f32;
        let scaled_x = (x * scale).floor();
        let scaled_y = (y * scale).floor();
        // Ceil width/height to ensure full pixel coverage after flooring position
        let scaled_width = (width * scale).ceil();
        let scaled_height = (height * scale).ceil();
        let scaled_radii = [
            corner_radii[0] * scale,
            corner_radii[1] * scale,
            corner_radii[2] * scale,
            corner_radii[3] * scale,
        ];

        // Generate geometry - use gradient if present, otherwise solid color
        // Both functions support rounded corners via the radii parameter
        let (vertices, indices) = if let Some(gradient) = gradient {
            crate::geometry::gradient_rect(
                scaled_x,
                scaled_y,
                scaled_width,
                scaled_height,
                gradient,
                scaled_radii,
            )
        } else {
            crate::geometry::rounded_rect(
                scaled_x,
                scaled_y,
                scaled_width,
                scaled_height,
                color,
                scaled_radii,
            )
        };

        // Compute center for rotation
        let center_x = scaled_x + scaled_width / 2.0;
        let center_y = scaled_y + scaled_height / 2.0;
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();

        // Convert screen-space vertices to NDC, applying rotation around center
        let ndc_vertices: Vec<crate::render::Vertex> = vertices.iter().map(|v| {
            // Rotate around center if rotation is non-zero
            let (rx, ry) = if rotation.abs() > 0.0001 {
                let dx = v.position[0] - center_x;
                let dy = v.position[1] - center_y;
                let rotated_x = center_x + dx * cos_r - dy * sin_r;
                let rotated_y = center_y + dx * sin_r + dy * cos_r;
                (rotated_x, rotated_y)
            } else {
                (v.position[0], v.position[1])
            };
            let ndc = self.screen_to_ndc(rx, ry);
            crate::render::Vertex {
                position: [ndc[0], ndc[1], 0.0],
                texcoord: v.texcoord,
                color: v.color,
            }
        }).collect();

        // Render the fill
        self.render_triangles(render_pass, &ndc_vertices, &indices)?;

        // Render border if present
        if let Some(border) = border {
            let scaled_border_width = border.width * scale;
            let (border_vertices, border_indices) = crate::geometry::border_rect(
                scaled_x,
                scaled_y,
                scaled_width,
                scaled_height,
                scaled_border_width,
                border.color,
                scaled_radii,
            );

            let ndc_border_vertices: Vec<crate::render::Vertex> = border_vertices.iter().map(|v| {
                // Rotate around center if rotation is non-zero
                let (rx, ry) = if rotation.abs() > 0.0001 {
                    let dx = v.position[0] - center_x;
                    let dy = v.position[1] - center_y;
                    let rotated_x = center_x + dx * cos_r - dy * sin_r;
                    let rotated_y = center_y + dx * sin_r + dy * cos_r;
                    (rotated_x, rotated_y)
                } else {
                    (v.position[0], v.position[1])
                };
                let ndc = self.screen_to_ndc(rx, ry);
                crate::render::Vertex {
                    position: [ndc[0], ndc[1], 0.0],
                    texcoord: v.texcoord,
                    color: v.color,
                }
            }).collect();

            self.render_triangles(render_pass, &ndc_border_vertices, &border_indices)?;
        }

        Ok(())
    }

    /// Render a soft shadow
    #[allow(clippy::too_many_arguments)]
    fn render_shadow(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        blur: f32,
        color: u32,
        offset_x: f32,
        offset_y: f32,
        corner_radii: [f32; 4],
    ) -> Result<(), Box<dyn Error>> {
        // Scale coordinates for HiDPI
        let scale = self.scale_factor as f32;
        let scaled_x = x * scale;
        let scaled_y = y * scale;
        let scaled_width = width * scale;
        let scaled_height = height * scale;
        let scaled_blur = blur * scale;
        let scaled_offset_x = offset_x * scale;
        let scaled_offset_y = offset_y * scale;
        let scaled_radii = [
            corner_radii[0] * scale,
            corner_radii[1] * scale,
            corner_radii[2] * scale,
            corner_radii[3] * scale,
        ];

        // Generate shadow geometry using the geometry module
        let (vertices, indices) = crate::geometry::shadow_rect(
            scaled_x,
            scaled_y,
            scaled_width,
            scaled_height,
            scaled_blur,
            color,
            scaled_offset_x,
            scaled_offset_y,
            scaled_radii,
        );

        // Convert screen-space vertices to NDC
        let ndc_vertices: Vec<crate::render::Vertex> = vertices.iter().map(|v| {
            let ndc = self.screen_to_ndc(v.position[0], v.position[1]);
            crate::render::Vertex {
                position: [ndc[0], ndc[1], 0.0],
                texcoord: v.texcoord,
                color: v.color,
            }
        }).collect();

        // Render the shadow
        self.render_triangles(render_pass, &ndc_vertices, &indices)?;

        Ok(())
    }

    /// Render text at the given position with multi-line support
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux", target_os = "windows"))]
    fn render_text(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        x: f32,
        y: f32,
        text: &str,
        font: &FontDescriptor,
        color: u32,
        layout: &TextLayoutConfig,
    ) -> Result<(), Box<dyn Error>> {
        // Extract RGBA from u32 color (assuming RGBA8 format: 0xRRGGBBAA)
        let r = ((color >> 24) & 0xFF) as f32 / 255.0;
        let g = ((color >> 16) & 0xFF) as f32 / 255.0;
        let b = ((color >> 8) & 0xFF) as f32 / 255.0;
        let a = (color & 0xFF) as f32 / 255.0;
        let text_color = [r, g, b, a];

        // Apply scale factor to font size and positions for proper HiDPI rendering
        let scale = self.scale_factor as f32;
        let font_size = font.size * scale;
        let scaled_x = x * scale;
        let scaled_y = y * scale;

        // Scale layout parameters
        let scaled_max_width = layout.max_width.map(|w| w * scale);

        // Create a scaled font descriptor for rasterization
        let scaled_font = FontDescriptor {
            source: font.source.clone(),
            weight: font.weight,
            style: font.style,
            size: font_size,
        };

        // Get actual font metrics for accurate line height calculations
        // This ensures fonts with non-standard metrics render correctly
        let (ascent, descent) = self.rasterizer.get_font_metrics(&scaled_font);
        let actual_font_height = ascent + descent;

        // Calculate line height based on actual font metrics, not font_size
        let line_height_px = actual_font_height * layout.line_height;

        // Calculate letter and word spacing (em units -> pixels)
        // letter_spacing applies to every character, word_spacing applies additionally to spaces
        let letter_spacing_px = layout.letter_spacing * font_size;
        let word_spacing_px = layout.word_spacing * font_size;

        // Pre-compute font ID for glyph cache lookups
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        scaled_font.cache_key().hash(&mut hasher);
        let font_id = hasher.finish();

        // Break text into lines based on layout config
        let all_lines = self.layout_text_lines(
            text, &scaled_font, font_id, font_size, scaled_max_width,
            letter_spacing_px, word_spacing_px, layout, scale
        )?;

        // Calculate max lines based on max_lines setting and max_height
        let max_lines_from_setting = layout.max_lines.unwrap_or(usize::MAX);
        let max_lines_from_height = if let Some(max_h) = layout.max_height {
            let scaled_max_h = max_h * scale;
            // First line takes actual_font_height, subsequent lines take line_height_px
            // So: actual_font_height + (n-1) * line_height_px <= max_height
            // Solving for n: n <= 1 + (max_height - actual_font_height) / line_height_px
            let first_line_height = actual_font_height;
            let result = if scaled_max_h < first_line_height {
                1 // At minimum show 1 line
            } else {
                let remaining = scaled_max_h - first_line_height;
                (1 + (remaining / line_height_px).floor() as usize).max(1)
            };
            result
        } else {
            usize::MAX
        };
        let max_lines = max_lines_from_setting.min(max_lines_from_height);

        // Check if we need ellipsis due to line count
        let needs_line_ellipsis = layout.overflow == TextOverflow::Ellipsis && all_lines.len() > max_lines;

        // Check if we need ellipsis due to width overflow on single line
        let needs_width_ellipsis = layout.overflow == TextOverflow::Ellipsis
            && scaled_max_width.is_some()
            && all_lines.len() == 1
            && all_lines.get(0).map(|l| l.width > scaled_max_width.unwrap()).unwrap_or(false);

        let needs_ellipsis = needs_line_ellipsis || needs_width_ellipsis;

        // Apply ellipsis if needed
        let lines: Vec<TextLine> = if needs_ellipsis && max_lines > 0 {
            let mut truncated_lines: Vec<_> = all_lines.into_iter().take(max_lines).collect();

            // Truncate the last line and add ellipsis
            if let Some(last_line) = truncated_lines.last_mut() {
                // Rasterize ellipsis
                let ellipsis_glyphs = self.rasterize_text_segment("…", &scaled_font, font_id, font_size)?;
                let ellipsis_width: f32 = ellipsis_glyphs.iter().map(|g| g.entry.advance).sum();

                // If we have a max_width, we need to truncate the line to fit ellipsis
                if let Some(max_w) = scaled_max_width {
                    let target_width = max_w - ellipsis_width;
                    if target_width > 0.0 {
                        // Truncate glyphs until we fit
                        let mut current_width = 0.0f32;
                        let mut truncate_at = 0;

                        for (i, glyph) in last_line.glyphs.iter().enumerate() {
                            if current_width + glyph.entry.advance > target_width {
                                break;
                            }
                            current_width += glyph.entry.advance;
                            truncate_at = i + 1;
                        }

                        last_line.glyphs.truncate(truncate_at);
                        // Trim trailing spaces before ellipsis
                        while last_line.glyphs.last().map(|g| g.character == ' ').unwrap_or(false) {
                            last_line.glyphs.pop();
                        }
                        last_line.width = last_line.glyphs.iter().map(|g| g.entry.advance).sum();
                    } else {
                        // Not enough room even for ellipsis - just use ellipsis
                        last_line.glyphs.clear();
                        last_line.width = 0.0;
                    }
                }

                // Add ellipsis glyphs
                last_line.glyphs.extend(ellipsis_glyphs);
                last_line.width += ellipsis_width;
            }

            truncated_lines
        } else {
            // No ellipsis needed - just apply max_lines limit
            all_lines.into_iter().take(max_lines).collect()
        };

        let mut vertices = Vec::new();

        // Render each line
        for (line_index, line) in lines.iter().enumerate() {
            // Calculate baseline Y for this line
            // The Y coordinate from layout is the TOP of the text box
            // The text box height is actualFontHeight * lineHeight
            // We need to position the baseline such that text is vertically centered
            //
            // Extra space from lineHeight = actualFontHeight * (lineHeight - 1)
            // Half of extra space goes above: actualFontHeight * (lineHeight - 1) / 2
            // Baseline from top = extra_top + ascent
            let extra_space = line_height_px - actual_font_height;
            let baseline_offset = (extra_space / 2.0) + ascent;
            let line_baseline_y = scaled_y + baseline_offset + (line_index as f32 * line_height_px);

            // Calculate X offset for alignment and justify spacing
            let is_last_line = line_index == lines.len() - 1;
            let (line_x, justify_extra_space) = match layout.alignment {
                TextAlign::Left => (scaled_x, 0.0),
                TextAlign::Center => {
                    let x = if let Some(max_w) = scaled_max_width {
                        scaled_x + (max_w - line.width) / 2.0
                    } else {
                        scaled_x
                    };
                    (x, 0.0)
                }
                TextAlign::Right => {
                    let x = if let Some(max_w) = scaled_max_width {
                        scaled_x + max_w - line.width
                    } else {
                        scaled_x
                    };
                    (x, 0.0)
                }
                TextAlign::Justify => {
                    // Justify: distribute extra space among word gaps (spaces)
                    // Don't justify the last line - it stays left-aligned
                    if is_last_line {
                        (scaled_x, 0.0)
                    } else if let Some(max_w) = scaled_max_width {
                        // Count spaces in the line (word gaps)
                        let space_count = line.glyphs.iter()
                            .filter(|g| g.character == ' ')
                            .count();

                        if space_count > 0 {
                            let extra_space = max_w - line.width;
                            let extra_per_space = extra_space / space_count as f32;
                            (scaled_x, extra_per_space)
                        } else {
                            // No spaces - can't justify, fall back to left align
                            (scaled_x, 0.0)
                        }
                    } else {
                        // No max_width - can't justify
                        (scaled_x, 0.0)
                    }
                }
            };

            // Render each glyph in the line
            let mut current_x = line_x;
            for glyph_info in &line.glyphs {
                let entry = glyph_info.entry;

                // For emojis, use white color (no tint) so they render with native colors
                // For regular text, use the specified text_color
                let glyph_color = if glyph_info.is_emoji {
                    [1.0, 1.0, 1.0, a] // White with same alpha as text
                } else {
                    text_color
                };

                // Calculate quad positions
                let glyph_x = current_x + entry.bearing_x;
                let glyph_y = line_baseline_y - entry.bearing_y;
                let glyph_width = entry.width as f32;
                let glyph_height = entry.height as f32;

                // Convert to NDC
                let top_left = self.screen_to_ndc(glyph_x, glyph_y);
                let top_right = self.screen_to_ndc(glyph_x + glyph_width, glyph_y);
                let bottom_left = self.screen_to_ndc(glyph_x, glyph_y + glyph_height);
                let bottom_right = self.screen_to_ndc(glyph_x + glyph_width, glyph_y + glyph_height);

                // For emojis, use texture color directly; for text, use vertex color for tinting
                let use_texture_color = if glyph_info.is_emoji { 1.0 } else { 0.0 };

                // Create 6 vertices for 2 triangles (quad)
                // Triangle 1: top-left, bottom-left, top-right
                vertices.push(TextVertex {
                    position: top_left,
                    tex_coords: [entry.u0, entry.v0],
                    color: glyph_color,
                    use_texture_color,
                });
                vertices.push(TextVertex {
                    position: bottom_left,
                    tex_coords: [entry.u0, entry.v1],
                    color: glyph_color,
                    use_texture_color,
                });
                vertices.push(TextVertex {
                    position: top_right,
                    tex_coords: [entry.u1, entry.v0],
                    color: glyph_color,
                    use_texture_color,
                });

                // Triangle 2: top-right, bottom-left, bottom-right
                vertices.push(TextVertex {
                    position: top_right,
                    tex_coords: [entry.u1, entry.v0],
                    color: glyph_color,
                    use_texture_color,
                });
                vertices.push(TextVertex {
                    position: bottom_left,
                    tex_coords: [entry.u0, entry.v1],
                    color: glyph_color,
                    use_texture_color,
                });
                vertices.push(TextVertex {
                    position: bottom_right,
                    tex_coords: [entry.u1, entry.v1],
                    color: glyph_color,
                    use_texture_color,
                });

                // Advance cursor with letter spacing (and word spacing + justify for spaces)
                let mut advance = entry.advance + letter_spacing_px;
                if glyph_info.character == ' ' {
                    advance += word_spacing_px + justify_extra_space;
                }
                current_x += advance;
            }
        }

        // Upload atlas if it was modified
        self.upload_atlas_if_needed()?;

        // Only render if we have vertices
        if vertices.is_empty() {
            return Ok(());
        }

        // Create vertex buffer
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Set pipeline and bind group
        let pipeline = self.text_pipeline.as_ref().ok_or("Text pipeline not initialized")?;
        let bind_group = self.text_bind_group.as_ref().ok_or("Text bind group not initialized")?;

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);

        Ok(())
    }

    /// Calculate the width of glyphs including letter and word spacing
    /// Note: letter_spacing is added BETWEEN letters, not after the last one
    fn calculate_glyphs_width(glyphs: &[GlyphInfo], letter_spacing: f32, word_spacing: f32) -> f32 {
        let len = glyphs.len();
        glyphs.iter().enumerate().map(|(i, g)| {
            let mut advance = g.entry.advance;
            // Add letter_spacing between letters (not after the last one)
            if i < len - 1 {
                advance += letter_spacing;
            }
            if g.character == ' ' {
                advance += word_spacing;
            }
            advance
        }).sum()
    }

    /// Layout text into lines with word wrapping
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux", target_os = "windows"))]
    fn layout_text_lines(
        &mut self,
        text: &str,
        scaled_font: &FontDescriptor,
        font_id: u64,
        font_size: f32,
        max_width: Option<f32>,
        _letter_spacing: f32,
        _word_spacing: f32,
        layout: &TextLayoutConfig,
        scale: f32,
    ) -> Result<Vec<TextLine>, Box<dyn Error>> {
        let mut lines = Vec::new();

        // Handle whitespace mode
        let should_wrap = layout.white_space != WhiteSpace::NoWrap && layout.white_space != WhiteSpace::Pre;
        let preserve_whitespace = layout.white_space == WhiteSpace::Pre || layout.white_space == WhiteSpace::PreWrap;

        // Split by explicit newlines first
        let paragraphs: Vec<&str> = if preserve_whitespace {
            text.split('\n').collect()
        } else {
            text.split('\n').collect()
        };

        for paragraph in paragraphs {
            if paragraph.is_empty() {
                // Empty line (from double newline or trailing newline)
                lines.push(TextLine { glyphs: Vec::new(), width: 0.0 });
                continue;
            }

            if !should_wrap || max_width.is_none() {
                // No wrapping - render entire paragraph as one line
                let glyphs = self.rasterize_text_segment(paragraph, scaled_font, font_id, font_size)?;
                let width = self.rasterizer.measure_string(paragraph, scaled_font);
                lines.push(TextLine { glyphs, width });
            } else {
                // Character-by-character wrapping to match Go's algorithm exactly
                // This ensures wrap decisions are identical between Go layout and Rust rendering
                let max_w = max_width.unwrap();
                // Go uses 1.0 logical pixel tolerance, so we need scale * 1.0 physical pixels
                let overflow_tolerance = scale;

                let chars: Vec<char> = paragraph.chars().collect();
                let mut line_start = 0;
                let mut last_word_end = 0; // Position after last space (word boundary)
                let mut i = 0;

                while i < chars.len() {
                    let ch = chars[i];

                    // Track word boundaries
                    if ch.is_whitespace() {
                        last_word_end = i + 1;
                    }

                    // Measure text from line_start to i+1 (inclusive of current char)
                    let line_text: String = chars[line_start..=i].iter().collect();
                    let line_width = self.rasterizer.measure_string(&line_text, scaled_font);

                    if line_width > max_w + overflow_tolerance && i > line_start {
                        // Need to wrap - find break point
                        let break_point = if last_word_end > line_start {
                            // Break at last word boundary
                            last_word_end
                        } else {
                            // No word boundary found, break at current character
                            i
                        };

                        // Create line from line_start to break_point
                        let final_line_text: String = chars[line_start..break_point].iter().collect();
                        let final_line_width = self.rasterizer.measure_string(&final_line_text, scaled_font);
                        let line_glyphs = self.rasterize_text_segment(&final_line_text, scaled_font, font_id, font_size)?;
                        lines.push(TextLine {
                            glyphs: line_glyphs,
                            width: final_line_width,
                        });

                        // Skip whitespace at start of next line (matching Go behavior)
                        line_start = break_point;
                        while line_start < chars.len() && chars[line_start] == ' ' {
                            line_start += 1;
                        }
                        i = line_start;
                        last_word_end = line_start;
                        continue;
                    }

                    i += 1;
                }

                // Add remaining text as final line
                if line_start < chars.len() {
                    let final_line_text: String = chars[line_start..].iter().collect();
                    let final_line_width = self.rasterizer.measure_string(&final_line_text, scaled_font);
                    let line_glyphs = self.rasterize_text_segment(&final_line_text, scaled_font, font_id, font_size)?;
                    lines.push(TextLine {
                        glyphs: line_glyphs,
                        width: final_line_width,
                    });
                }
            }
        }

        Ok(lines)
    }

    /// Tokenize text into words (including trailing spaces)
    fn tokenize_text(&self, text: &str, preserve_whitespace: bool) -> Vec<String> {
        if preserve_whitespace {
            // Keep all whitespace as-is, split into individual characters
            text.chars().map(|c| c.to_string()).collect()
        } else {
            // Split by whitespace but keep space attached to previous word for proper measuring
            let mut words = Vec::new();
            let mut current_word = String::new();
            let mut in_whitespace = false;

            for ch in text.chars() {
                if ch.is_whitespace() {
                    if !current_word.is_empty() && !in_whitespace {
                        // Add space to current word
                        current_word.push(ch);
                        in_whitespace = true;
                    } else if in_whitespace {
                        // Multiple spaces - add another space
                        current_word.push(ch);
                    } else {
                        // Leading whitespace - start new word with space
                        current_word.push(ch);
                        in_whitespace = true;
                    }
                } else {
                    if in_whitespace {
                        // End of whitespace - push current word and start new one
                        if !current_word.is_empty() {
                            words.push(std::mem::take(&mut current_word));
                        }
                        in_whitespace = false;
                    }
                    current_word.push(ch);
                }
            }

            if !current_word.is_empty() {
                words.push(current_word);
            }

            words
        }
    }

    /// Rasterize a text segment and return glyph info
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux", target_os = "windows"))]
    fn rasterize_text_segment(
        &mut self,
        text: &str,
        scaled_font: &FontDescriptor,
        font_id: u64,
        font_size: f32,
    ) -> Result<Vec<GlyphInfo>, Box<dyn Error>> {
        let mut glyphs = Vec::new();

        for ch in text.chars() {
            let glyph_key = crate::text::GlyphKey::new(font_id, ch as u32, font_size);

            // Get or rasterize glyph
            let entry = if let Some(e) = self.glyph_atlas.get(&glyph_key) {
                *e
            } else {
                // Rasterize the glyph with full font descriptor
                if let Some(bitmap) = self.rasterizer.rasterize_glyph(ch, scaled_font) {
                    self.glyph_atlas.insert(glyph_key, bitmap)
                        .ok_or_else(|| "Failed to insert glyph into atlas")?
                } else {
                    // Skip this character if rasterization failed
                    continue;
                }
            };

            glyphs.push(GlyphInfo { character: ch, entry, is_emoji: is_emoji(ch) });
        }

        Ok(glyphs)
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android", target_os = "linux", target_os = "windows")))]
    fn render_text(
        &mut self,
        _render_pass: &mut wgpu::RenderPass,
        _x: f32,
        _y: f32,
        _text: &str,
        _font: &FontDescriptor,
        _color: u32,
        _layout: &TextLayoutConfig,
    ) -> Result<(), Box<dyn Error>> {
        // TODO: Implement for other platforms (e.g., web)
        Ok(())
    }

    /// Render an image at the given position
    fn render_image(
        &self,
        render_pass: &mut wgpu::RenderPass,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        texture_id: u32,
        source_rect: Option<(f32, f32, f32, f32)>,
        corner_radii: [f32; 4],
    ) -> Result<(), Box<dyn Error>> {
        let gpu_texture = self.image_textures.get(&texture_id)
            .ok_or_else(|| format!("Texture {} not found", texture_id))?;
        let pipeline = self.image_pipeline.as_ref()
            .ok_or("Image pipeline not initialized")?;
        let device = self.device.as_ref()
            .ok_or("Device not initialized")?;

        // Apply scale factor
        let scale = self.scale_factor as f32;
        let scaled_x = x * scale;
        let scaled_y = y * scale;
        let scaled_w = width * scale;
        let scaled_h = height * scale;

        // Texture coordinates (source rect or full texture)
        let (u0, v0, u1, v1) = source_rect.unwrap_or((0.0, 0.0, 1.0, 1.0));

        // White color = no tint, full opacity
        let color = [1.0f32, 1.0, 1.0, 1.0];

        // Check if we have rounded corners
        let has_rounded = corner_radii.iter().any(|&r| r > 0.5);

        let vertices: Vec<TextVertex> = if has_rounded {
            // Generate rounded rectangle geometry with proper UV mapping
            self.generate_rounded_image_vertices(
                scaled_x, scaled_y, scaled_w, scaled_h,
                corner_radii.map(|r| r * scale),
                u0, v0, u1, v1,
                color,
            )
        } else {
            // Simple quad - 2 triangles, 6 vertices
            let left = scaled_x;
            let right = scaled_x + scaled_w;
            let top = scaled_y;
            let bottom = scaled_y + scaled_h;

            // Convert to NDC
            let tl = self.screen_to_ndc(left, top);
            let tr = self.screen_to_ndc(right, top);
            let bl = self.screen_to_ndc(left, bottom);
            let br = self.screen_to_ndc(right, bottom);

            vec![
                // Triangle 1 - images always use texture color directly
                TextVertex { position: tl, tex_coords: [u0, v0], color, use_texture_color: 1.0 },
                TextVertex { position: bl, tex_coords: [u0, v1], color, use_texture_color: 1.0 },
                TextVertex { position: tr, tex_coords: [u1, v0], color, use_texture_color: 1.0 },
                // Triangle 2
                TextVertex { position: tr, tex_coords: [u1, v0], color, use_texture_color: 1.0 },
                TextVertex { position: bl, tex_coords: [u0, v1], color, use_texture_color: 1.0 },
                TextVertex { position: br, tex_coords: [u1, v1], color, use_texture_color: 1.0 },
            ]
        };

        // Create vertex buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Image Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Set pipeline and bind group for this texture
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &gpu_texture.bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);

        Ok(())
    }

    /// Generate vertices for a rounded rectangle with proper UV mapping for images
    fn generate_rounded_image_vertices(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radii: [f32; 4],
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
    ) -> Vec<TextVertex> {
        use std::f32::consts::PI;
        const CORNER_SEGMENTS: usize = 8;

        // Clamp radii to half the smallest dimension
        let max_radius = (width.min(height)) / 2.0;
        let radii = [
            radii[0].min(max_radius),
            radii[1].min(max_radius),
            radii[2].min(max_radius),
            radii[3].min(max_radius),
        ];

        let mut vertices = Vec::new();

        // Helper to convert screen position to UV
        let pos_to_uv = |px: f32, py: f32| -> [f32; 2] {
            let u = u0 + (px - x) / width * (u1 - u0);
            let v = v0 + (py - y) / height * (v1 - v0);
            [u, v]
        };

        // Center point for fan triangulation
        let center_x = x + width / 2.0;
        let center_y = y + height / 2.0;
        let center_ndc = self.screen_to_ndc(center_x, center_y);
        let center_uv = pos_to_uv(center_x, center_y);

        // Generate vertices along the perimeter and create triangles to center
        // Going clockwise: top-left corner, top edge, top-right corner, right edge, etc.

        let corners = [
            // (corner_center_x, corner_center_y, start_angle, end_angle, radius)
            (x + radii[0], y + radii[0], PI, PI / 2.0, radii[0]),                    // top-left
            (x + width - radii[1], y + radii[1], PI / 2.0, 0.0, radii[1]),           // top-right
            (x + width - radii[2], y + height - radii[2], 0.0, -PI / 2.0, radii[2]), // bottom-right
            (x + radii[3], y + height - radii[3], -PI / 2.0, -PI, radii[3]),         // bottom-left
        ];

        // Sharp corner positions for when radius is 0
        let sharp_corners = [
            (x, y),                         // top-left
            (x + width, y),                 // top-right
            (x + width, y + height),        // bottom-right
            (x, y + height),                // bottom-left
        ];

        // Collect all perimeter points
        let mut perimeter_points: Vec<(f32, f32)> = Vec::new();

        for (corner_idx, &(cx, cy, start_angle, end_angle, radius)) in corners.iter().enumerate() {
            if radius > 0.5 {
                // Rounded corner - generate arc points
                for i in 0..=CORNER_SEGMENTS {
                    let t = i as f32 / CORNER_SEGMENTS as f32;
                    let angle = start_angle + (end_angle - start_angle) * t;
                    let px = cx + angle.cos() * radius;
                    let py = cy - angle.sin() * radius; // Flip Y for screen coordinates
                    perimeter_points.push((px, py));
                }
            } else {
                // Sharp corner - single point
                perimeter_points.push(sharp_corners[corner_idx]);
            }
        }

        // Create triangles from center to each pair of consecutive perimeter points
        let num_points = perimeter_points.len();
        for i in 0..num_points {
            let p1 = perimeter_points[i];
            let p2 = perimeter_points[(i + 1) % num_points];

            let p1_ndc = self.screen_to_ndc(p1.0, p1.1);
            let p2_ndc = self.screen_to_ndc(p2.0, p2.1);
            let p1_uv = pos_to_uv(p1.0, p1.1);
            let p2_uv = pos_to_uv(p2.0, p2.1);

            // Triangle: center, p1, p2 - images always use texture color directly
            vertices.push(TextVertex { position: center_ndc, tex_coords: center_uv, color, use_texture_color: 1.0 });
            vertices.push(TextVertex { position: p1_ndc, tex_coords: p1_uv, color, use_texture_color: 1.0 });
            vertices.push(TextVertex { position: p2_ndc, tex_coords: p2_uv, color, use_texture_color: 1.0 });
        }

        vertices
    }
}

/// Information about a laid-out line of text
struct TextLine {
    glyphs: Vec<GlyphInfo>,
    width: f32,
}

/// Information about a single glyph for layout
#[derive(Clone, Copy)]
struct GlyphInfo {
    character: char,
    entry: crate::text::AtlasEntry,
    is_emoji: bool,
}

/// Check if a character is an emoji (should render with native colors, not text color)
fn is_emoji(c: char) -> bool {
    let cp = c as u32;

    // Emoji ranges (simplified but covers most common cases)
    matches!(cp,
        // Emoticons
        0x1F600..=0x1F64F |
        // Miscellaneous Symbols and Pictographs
        0x1F300..=0x1F5FF |
        // Transport and Map Symbols
        0x1F680..=0x1F6FF |
        // Supplemental Symbols and Pictographs
        0x1F900..=0x1F9FF |
        // Symbols and Pictographs Extended-A
        0x1FA00..=0x1FA6F |
        // Symbols and Pictographs Extended-B
        0x1FA70..=0x1FAFF |
        // Dingbats
        0x2700..=0x27BF |
        // Miscellaneous Symbols
        0x2600..=0x26FF |
        // Regional Indicator Symbols (flags)
        0x1F1E0..=0x1F1FF |
        // Skin tone modifiers
        0x1F3FB..=0x1F3FF |
        // Food and Drink
        0x1F32D..=0x1F37F |
        // Additional common emoji ranges
        0x1F400..=0x1F4FF |
        // Musical symbols that are often emoji
        0x1F3A0..=0x1F3FF
    )
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    color: [f32; 4],
    /// 1.0 = use texture RGB directly (for emojis)
    /// 0.0 = use vertex color RGB (for regular text)
    use_texture_color: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GeometryVertex {
    position: [f32; 3],
    texcoord: [f32; 2],
    color: [f32; 4],
}
