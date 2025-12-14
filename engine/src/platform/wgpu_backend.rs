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

/// wgpu rendering backend
pub struct WgpuBackend {
    // wgpu core
    instance: wgpu::Instance,
    adapter: Option<wgpu::Adapter>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,

    // Render pipeline for text
    text_pipeline: Option<wgpu::RenderPipeline>,
    text_bind_group: Option<wgpu::BindGroup>,
    atlas_texture: Option<wgpu::Texture>,

    // Render pipeline for colored geometry (triangles, rectangles)
    geometry_pipeline: Option<wgpu::RenderPipeline>,

    // Glyph atlas
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    glyph_atlas: GlyphAtlas,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    rasterizer: MacOSGlyphRasterizer,

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
        // Create wgpu instance with default backends
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
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
            width: 0,
            height: 0,
            scale_factor: 1.0,
            scissor_stack: Vec::new(),
            scroll_offset_stack: Vec::new(),
            image_textures: HashMap::new(),
            image_pipeline: None,
            image_bind_group_layout: None,
            next_texture_id: 1,
        }
    }

    /// Get the current scale factor (for HiDPI displays)
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
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
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Centered Engine Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
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
            alpha_mode: surface_caps.alpha_modes[0],
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
            depth_stencil: None,
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
            depth_stencil: None,
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
            depth_stencil: None,
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

    /// Get texture dimensions for a loaded image
    pub fn get_texture_size(&self, texture_id: u32) -> Option<(u32, u32)> {
        self.image_textures.get(&texture_id).map(|tex| (tex.width, tex.height))
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
        #[cfg(any(target_os = "macos", target_os = "ios"))]
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

        // Begin render pass
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Clear scissor stack and scroll offset stack at start of frame
            self.scissor_stack.clear();
            self.scroll_offset_stack.clear();

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
                        // Pop from stack
                        self.scissor_stack.pop();

                        // Restore parent scissor or full viewport
                        if let Some(parent) = self.scissor_stack.last() {
                            render_pass.set_scissor_rect(parent.x, parent.y, parent.width.max(1), parent.height.max(1));
                        } else {
                            render_pass.set_scissor_rect(0, 0, full_width, full_height);
                        }
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
    #[cfg(any(target_os = "macos", target_os = "ios"))]
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
        let line_height_px = font.size * layout.line_height * scale;

        // Calculate letter and word spacing (em units -> pixels)
        // letter_spacing applies to every character, word_spacing applies additionally to spaces
        let letter_spacing_px = layout.letter_spacing * font_size;
        let word_spacing_px = layout.word_spacing * font_size;

        // Create a scaled font descriptor for rasterization
        let scaled_font = FontDescriptor {
            source: font.source.clone(),
            weight: font.weight,
            style: font.style,
            size: font_size,
        };

        // Pre-compute font ID for glyph cache lookups
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        scaled_font.cache_key().hash(&mut hasher);
        let font_id = hasher.finish();

        // Break text into lines based on layout config
        let all_lines = self.layout_text_lines(
            text, &scaled_font, font_id, font_size, scaled_max_width,
            letter_spacing_px, word_spacing_px, layout
        )?;

        // Calculate max lines based on max_lines setting and max_height
        let max_lines_from_setting = layout.max_lines.unwrap_or(usize::MAX);
        let max_lines_from_height = if let Some(max_h) = layout.max_height {
            let scaled_max_h = max_h * scale;
            // First line takes font_size (for baseline + descender), subsequent lines take line_height_px
            // So: font_size + (n-1) * line_height_px <= max_height
            // Solving for n: n <= 1 + (max_height - font_size) / line_height_px
            let first_line_height = font_size;
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
            // The text box height is fontSize * lineHeight (e.g., 14 * 1.4 = 19.6)
            // We need to position the baseline such that text is vertically centered
            //
            // For typical fonts: ascent ~= 0.8 * fontSize, descent ~= 0.2 * fontSize
            // Total glyph height = fontSize (ascent + descent)
            // Extra space from lineHeight = fontSize * (lineHeight - 1)
            // Half of extra space goes above: fontSize * (lineHeight - 1) / 2
            // Baseline from top = extra_top + ascent = fontSize * (lineHeight - 1) / 2 + fontSize * 0.8
            //                   = fontSize * ((lineHeight - 1) / 2 + 0.8)
            //                   = fontSize * (lineHeight / 2 - 0.5 + 0.8)
            //                   = fontSize * (lineHeight / 2 + 0.3)
            // For lineHeight 1.4: fontSize * (0.7 + 0.3) = fontSize * 1.0
            // For lineHeight 1.5: fontSize * (0.75 + 0.3) = fontSize * 1.05
            let baseline_offset = font_size * (layout.line_height / 2.0 + 0.3);
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

                // Create 6 vertices for 2 triangles (quad)
                // Triangle 1: top-left, bottom-left, top-right
                vertices.push(TextVertex {
                    position: top_left,
                    tex_coords: [entry.u0, entry.v0],
                    color: text_color,
                });
                vertices.push(TextVertex {
                    position: bottom_left,
                    tex_coords: [entry.u0, entry.v1],
                    color: text_color,
                });
                vertices.push(TextVertex {
                    position: top_right,
                    tex_coords: [entry.u1, entry.v0],
                    color: text_color,
                });

                // Triangle 2: top-right, bottom-left, bottom-right
                vertices.push(TextVertex {
                    position: top_right,
                    tex_coords: [entry.u1, entry.v0],
                    color: text_color,
                });
                vertices.push(TextVertex {
                    position: bottom_left,
                    tex_coords: [entry.u0, entry.v1],
                    color: text_color,
                });
                vertices.push(TextVertex {
                    position: bottom_right,
                    tex_coords: [entry.u1, entry.v1],
                    color: text_color,
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
    fn calculate_glyphs_width(glyphs: &[GlyphInfo], letter_spacing: f32, word_spacing: f32) -> f32 {
        glyphs.iter().map(|g| {
            let mut advance = g.entry.advance + letter_spacing;
            if g.character == ' ' {
                advance += word_spacing;
            }
            advance
        }).sum()
    }

    /// Layout text into lines with word wrapping
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn layout_text_lines(
        &mut self,
        text: &str,
        scaled_font: &FontDescriptor,
        font_id: u64,
        font_size: f32,
        max_width: Option<f32>,
        letter_spacing: f32,
        word_spacing: f32,
        layout: &TextLayoutConfig,
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

            // Get words/tokens for this paragraph
            let words = self.tokenize_text(paragraph, preserve_whitespace);

            if !should_wrap || max_width.is_none() {
                // No wrapping - render entire paragraph as one line
                let glyphs = self.rasterize_text_segment(paragraph, scaled_font, font_id, font_size)?;
                let width = Self::calculate_glyphs_width(&glyphs, letter_spacing, word_spacing);
                lines.push(TextLine { glyphs, width });
            } else {
                // Word wrap
                let max_w = max_width.unwrap();
                let mut current_line_glyphs = Vec::new();
                let mut current_line_width = 0.0f32;

                for word in words {
                    // Measure the word
                    let word_glyphs = self.rasterize_text_segment(&word, scaled_font, font_id, font_size)?;
                    let word_width = Self::calculate_glyphs_width(&word_glyphs, letter_spacing, word_spacing);

                    // Check if word fits on current line
                    if current_line_glyphs.is_empty() {
                        // First word on line - always add it (even if too long)
                        current_line_glyphs.extend(word_glyphs);
                        current_line_width = word_width;
                    } else if current_line_width + word_width <= max_w {
                        // Word fits - add it
                        current_line_glyphs.extend(word_glyphs);
                        current_line_width += word_width;
                    } else {
                        // Word doesn't fit - start new line
                        lines.push(TextLine {
                            glyphs: std::mem::take(&mut current_line_glyphs),
                            width: current_line_width,
                        });

                        // Handle WordBreak::BreakWord for very long words
                        if layout.word_break == WordBreak::BreakWord && word_width > max_w {
                            // Break the word character by character
                            let mut char_glyphs = Vec::new();
                            let mut char_width = 0.0f32;

                            for glyph in word_glyphs {
                                let glyph_advance = glyph.entry.advance + letter_spacing
                                    + if glyph.character == ' ' { word_spacing } else { 0.0 };
                                if char_width + glyph_advance > max_w && !char_glyphs.is_empty() {
                                    lines.push(TextLine {
                                        glyphs: std::mem::take(&mut char_glyphs),
                                        width: char_width,
                                    });
                                    char_width = 0.0;
                                }
                                char_width += glyph_advance;
                                char_glyphs.push(glyph);
                            }

                            current_line_glyphs = char_glyphs;
                            current_line_width = char_width;
                        } else {
                            // Start new line with this word (skip leading space if any)
                            let trimmed_glyphs: Vec<_> = word_glyphs.into_iter()
                                .skip_while(|g| g.character == ' ')
                                .collect();
                            current_line_width = Self::calculate_glyphs_width(&trimmed_glyphs, letter_spacing, word_spacing);
                            current_line_glyphs = trimmed_glyphs;
                        }
                    }
                }

                // Don't forget the last line
                if !current_line_glyphs.is_empty() {
                    lines.push(TextLine {
                        glyphs: current_line_glyphs,
                        width: current_line_width,
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
    #[cfg(any(target_os = "macos", target_os = "ios"))]
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

            glyphs.push(GlyphInfo { character: ch, entry });
        }

        Ok(glyphs)
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
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
        // TODO: Implement for other platforms
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
                // Triangle 1
                TextVertex { position: tl, tex_coords: [u0, v0], color },
                TextVertex { position: bl, tex_coords: [u0, v1], color },
                TextVertex { position: tr, tex_coords: [u1, v0], color },
                // Triangle 2
                TextVertex { position: tr, tex_coords: [u1, v0], color },
                TextVertex { position: bl, tex_coords: [u0, v1], color },
                TextVertex { position: br, tex_coords: [u1, v1], color },
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

            // Triangle: center, p1, p2
            vertices.push(TextVertex { position: center_ndc, tex_coords: center_uv, color });
            vertices.push(TextVertex { position: p1_ndc, tex_coords: p1_uv, color });
            vertices.push(TextVertex { position: p2_ndc, tex_coords: p2_uv, color });
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
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GeometryVertex {
    position: [f32; 3],
    texcoord: [f32; 2],
    color: [f32; 4],
}
