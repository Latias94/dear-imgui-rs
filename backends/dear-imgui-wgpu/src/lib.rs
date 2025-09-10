//! WGPU backend for Dear ImGui
//!
//! This crate provides a WGPU-based renderer for Dear ImGui, allowing you to
//! render Dear ImGui interfaces using the WGPU graphics API.

use dear_imgui::{
    render::{DrawCmd, DrawData, DrawIdx, DrawList, DrawVert},
    BackendFlags, Context, TextureData, TextureFormat as ImGuiTextureFormat, TextureId,
    TextureStatus,
};

use smallvec::SmallVec;
use std::collections::HashMap;
use std::mem::size_of;
use wgpu::util::{BufferInitDescriptor, DeviceExt, TextureDataOrder};
use wgpu::*;

static VS_ENTRY_POINT: &str = "vs_main";
static FS_ENTRY_POINT_LINEAR: &str = "fs_main_linear";

#[derive(Debug)]
pub enum RendererError {
    /// Generic error
    Generic(String),
    /// Bad texture error
    BadTexture(String),
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RendererError::Generic(msg) => write!(f, "Renderer error: {}", msg),
            RendererError::BadTexture(msg) => write!(f, "Bad texture error: {}", msg),
        }
    }
}

impl std::error::Error for RendererError {}

pub type RendererResult<T> = Result<T, RendererError>;

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
struct DrawVertPod(DrawVert);

unsafe impl bytemuck::Zeroable for DrawVertPod {}
unsafe impl bytemuck::Pod for DrawVertPod {}

/// WGPU texture resource
#[derive(Debug)]
pub struct WgpuTexture {
    pub texture: Texture,
    pub view: TextureView,
    pub bind_group: BindGroup,
}

/// Texture management for WGPU renderer
#[derive(Debug, Default)]
pub struct WgpuTextureMap {
    textures: HashMap<u64, WgpuTexture>,
    next_id: u64,
}

impl WgpuTextureMap {
    /// Create a new texture map
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 1, // Start from 1, 0 is reserved for null texture
        }
    }

    /// Insert a new texture and return its ID
    pub fn insert(&mut self, texture: WgpuTexture) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, texture);
        id
    }

    /// Get a texture by ID
    pub fn get(&self, id: u64) -> Option<&WgpuTexture> {
        self.textures.get(&id)
    }

    /// Remove a texture by ID
    pub fn remove(&mut self, id: u64) -> Option<WgpuTexture> {
        self.textures.remove(&id)
    }

    /// Check if a texture exists
    pub fn contains(&self, id: u64) -> bool {
        self.textures.contains_key(&id)
    }

    /// Insert a texture with a specific ID
    pub fn insert_with_id(&mut self, id: u64, texture: WgpuTexture) {
        self.textures.insert(id, texture);
    }
}

pub struct RenderData {
    fb_size: [f32; 2],
    last_size: [f32; 2],
    last_pos: [f32; 2],
    vertex_buffer: Option<Buffer>,
    vertex_buffer_size: usize,
    index_buffer: Option<Buffer>,
    index_buffer_size: usize,
    draw_list_offsets: SmallVec<[(i32, u32); 4]>,
    render: bool,
}

pub struct WgpuRenderer {
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    uniform_bind_group: BindGroup,
    texture_layout: BindGroupLayout,
    default_texture_bind_group: BindGroup,
    font_texture_bind_group: Option<BindGroup>,
    render_data: Option<RenderData>,
    texture_format: TextureFormat,
    texture_map: WgpuTextureMap,
}

impl WgpuRenderer {
    /// Create a new WGPU renderer
    pub fn new(device: &Device, queue: &Queue, texture_format: TextureFormat) -> Self {
        // Load shaders
        let shader_module = device.create_shader_module(include_wgsl!("shader.wgsl"));

        // Create the uniform matrix buffer
        let size = 64;
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("imgui-wgpu uniform buffer"),
            size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create the uniform matrix buffer bind group layout
        let uniform_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create the uniform matrix buffer bind group
        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("imgui-wgpu bind group"),
            layout: &uniform_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create the texture layout for further usage
        let texture_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("imgui-wgpu bind group layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create the render pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("imgui-wgpu pipeline layout"),
            bind_group_layouts: &[&uniform_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        // Create the render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("imgui-wgpu pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some(VS_ENTRY_POINT),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: size_of::<DrawVert>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some(FS_ENTRY_POINT_LINEAR),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: texture_format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::OneMinusDstAlpha,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        // Create a default white texture for rendering using DeviceExt
        let default_texture = device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some("imgui-wgpu default texture"),
                size: Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::default(),
            &[255u8, 255u8, 255u8, 255u8], // RGBA white
        );

        let default_texture_view = default_texture.create_view(&TextureViewDescriptor::default());
        let default_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("imgui-wgpu default sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        let default_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("imgui-wgpu default texture bind group"),
            layout: &texture_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&default_texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&default_sampler),
                },
            ],
        });

        Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            texture_layout,
            default_texture_bind_group,
            font_texture_bind_group: None,
            render_data: None,
            texture_format,
            texture_map: WgpuTextureMap::new(),
        }
    }

    /// Configure Dear ImGui context with WGPU backend capabilities
    pub fn configure_imgui_context(&self, imgui_context: &mut Context) {
        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();

        // Set WGPU renderer capabilities
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);

        io.set_backend_flags(flags);
    }

    /// Prepare font atlas for rendering (should be called after configuring context)
    pub fn prepare_font_atlas(&mut self, imgui_ctx: &mut Context, device: &Device, queue: &Queue) {
        // This method ensures the font atlas is ready for the new texture management system
        self.reload_font_texture(imgui_ctx, device, queue);
    }

    /// Create a texture from texture data
    fn create_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> Result<u64, RendererError> {
        let width = texture_data.width();
        let height = texture_data.height();
        let format = texture_data.format();
        let data = texture_data.pixels().ok_or(RendererError::BadTexture(
            "No pixel data available".to_string(),
        ))?;

        // Convert ImGui texture format to WGPU format
        let (wgpu_format, _bytes_per_pixel) = match format {
            ImGuiTextureFormat::RGBA32 => (TextureFormat::Rgba8Unorm, 4),
            ImGuiTextureFormat::Alpha8 => {
                // Convert Alpha8 to RGBA32 for WGPU
                let mut rgba_data = Vec::with_capacity(data.len() * 4);
                for &alpha in data {
                    rgba_data.extend_from_slice(&[255, 255, 255, alpha]); // White + alpha
                }

                let texture = device.create_texture_with_data(
                    queue,
                    &TextureDescriptor {
                        label: Some("imgui-wgpu texture"),
                        size: Extent3d {
                            width: width.try_into().unwrap(),
                            height: height.try_into().unwrap(),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::Rgba8Unorm,
                        usage: TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    TextureDataOrder::default(),
                    &rgba_data,
                );

                return self.create_texture_bind_group(texture, device);
            }
        };

        // Create RGBA32 texture directly
        let texture = device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some("imgui-wgpu texture"),
                size: Extent3d {
                    width: width.try_into().unwrap(),
                    height: height.try_into().unwrap(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: wgpu_format,
                usage: TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::default(),
            data,
        );

        self.create_texture_bind_group(texture, device)
    }

    /// Create a bind group for a texture and add it to the texture map
    fn create_texture_bind_group(
        &mut self,
        texture: Texture,
        device: &Device,
    ) -> Result<u64, RendererError> {
        let texture_view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("imgui-wgpu texture sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("imgui-wgpu texture bind group"),
            layout: &self.texture_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let wgpu_texture = WgpuTexture {
            texture,
            view: texture_view,
            bind_group,
        };

        let texture_id = self.texture_map.insert(wgpu_texture);
        Ok(texture_id)
    }

    /// Update an existing texture from texture data
    fn update_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> Result<(), RendererError> {
        let texture_id = texture_data.tex_id();

        // For WGPU, we recreate the texture instead of updating in place
        // This is simpler and more reliable than trying to update existing textures
        let texture_id_u64 = texture_id.id() as u64;
        if self.texture_map.contains(texture_id_u64) {
            // Remove old texture
            self.texture_map.remove(texture_id_u64);

            // Create new texture with same ID
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;

            // Move the texture to the correct ID slot
            if let Some(texture) = self.texture_map.remove(new_texture_id) {
                self.texture_map.insert_with_id(texture_id_u64, texture);
            }
        }

        Ok(())
    }

    /// Destroy a texture
    fn destroy_texture(&mut self, texture_id: TextureId) {
        let texture_id_u64 = texture_id.id() as u64;
        self.texture_map.remove(texture_id_u64);
        // WGPU textures are automatically cleaned up when dropped
    }

    /// Register a new texture with the renderer
    pub fn register_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        width: u32,
        height: u32,
        format: ImGuiTextureFormat,
        data: &[u8],
    ) -> Result<TextureId, RendererError> {
        // Create texture data using the proper constructor
        let mut texture_data = TextureData::new();

        // Create the texture with the specified format and dimensions
        texture_data.create(format, width as i32, height as i32);

        // Set the pixel data
        texture_data.set_data(data);

        // Create the texture
        let texture_id = self.create_texture_from_data(device, queue, &texture_data)?;

        // Set the texture ID and mark as OK
        texture_data.set_tex_id(TextureId::from(texture_id as usize));
        texture_data.set_status(TextureStatus::OK);

        Ok(TextureId::from(texture_id as usize))
    }

    /// Update an existing texture
    pub fn update_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), RendererError> {
        // Create temporary texture data for update
        let mut texture_data = TextureData::new();
        texture_data.set_tex_id(texture_id);
        texture_data.set_width(width);
        texture_data.set_height(height);
        texture_data.set_format(ImGuiTextureFormat::RGBA32); // Assume RGBA32 for updates
        texture_data.set_data(data);
        texture_data.set_status(TextureStatus::WantUpdates);

        self.update_texture_from_data(device, queue, &texture_data)
    }

    /// Get texture data for inspection
    pub fn get_texture_data(&self, _texture_id: TextureId) -> Option<&TextureData> {
        // This is a simplified implementation - in a real scenario,
        // you might want to store texture metadata separately
        None // For now, return None as we don't store the original TextureData
    }

    /// Handle texture updates from Dear ImGui
    fn handle_texture_updates(&mut self, draw_data: &DrawData, device: &Device, queue: &Queue) {
        for texture_data in draw_data.textures() {
            match texture_data.status() {
                TextureStatus::WantCreate => {
                    if let Ok(texture_id) =
                        self.create_texture_from_data(device, queue, texture_data)
                    {
                        // Update the texture data with the new ID
                        texture_data.set_tex_id(TextureId::from(texture_id as usize));
                        texture_data.set_status(TextureStatus::OK);
                    }
                }
                TextureStatus::WantUpdates => {
                    if self
                        .update_texture_from_data(device, queue, texture_data)
                        .is_err()
                    {
                        // If update fails, mark as destroyed
                        texture_data.set_status(TextureStatus::Destroyed);
                    } else {
                        texture_data.set_status(TextureStatus::OK);
                    }
                }
                TextureStatus::WantDestroy => {
                    self.destroy_texture(texture_data.tex_id());
                    texture_data.set_status(TextureStatus::Destroyed);
                }
                TextureStatus::OK | TextureStatus::Destroyed => {
                    // No action needed
                }
            }
        }
    }

    /// Enable multi-viewport support for WGPU renderer
    ///
    /// **Note**: Multi-viewport support for WGPU is currently disabled while we focus on
    /// improving the platform layer (Winit) implementation. This will be re-enabled in a future version.
    #[cfg(feature = "multi-viewport")]
    pub fn enable_multi_viewport(&self, _context: &mut dear_imgui::Context) {
        // TODO: Implement WGPU multi-viewport support
        // For now, we focus on the platform layer (Winit) implementation
        eprintln!("Warning: WGPU multi-viewport support is temporarily disabled. Use a simpler renderer like OpenGL for multi-viewport features.");
    }

    /// Prepare buffers for the current imgui frame
    pub fn prepare(
        &mut self,
        draw_data: &DrawData,
        render_data: Option<RenderData>,
        queue: &Queue,
        device: &Device,
    ) -> RenderData {
        // Handle texture management
        self.handle_texture_updates(draw_data, device, queue);

        // Continue with existing buffer preparation logic
        let [display_width, display_height] = draw_data.display_size();
        let [scale_x, scale_y] = draw_data.framebuffer_scale();
        let fb_width = display_width * scale_x;
        let fb_height = display_height * scale_y;

        let mut render_data = render_data.unwrap_or_else(|| RenderData {
            fb_size: [fb_width, fb_height],
            last_size: [0.0, 0.0],
            last_pos: [0.0, 0.0],
            vertex_buffer: None,
            vertex_buffer_size: 0,
            index_buffer: None,
            index_buffer_size: 0,
            draw_list_offsets: SmallVec::<[_; 4]>::new(),
            render: false,
        });

        // If the render area is <= 0, exit here and now
        if fb_width <= 0.0 || fb_height <= 0.0 {
            render_data.render = false;
            return render_data;
        } else {
            render_data.render = true;
        }

        // If there are no draw lists, exit here
        if !draw_data.valid() {
            render_data.render = false;
            return render_data;
        }

        // Only update matrices if the size or position changes
        let [display_pos_x, display_pos_y] = draw_data.display_pos();
        if (render_data.last_size[0] - display_width).abs() > f32::EPSILON
            || (render_data.last_size[1] - display_height).abs() > f32::EPSILON
            || (render_data.last_pos[0] - display_pos_x).abs() > f32::EPSILON
            || (render_data.last_pos[1] - display_pos_y).abs() > f32::EPSILON
        {
            render_data.fb_size = [fb_width, fb_height];
            render_data.last_size = [display_width, display_height];
            render_data.last_pos = [display_pos_x, display_pos_y];

            // Create orthographic projection matrix (matching imgui-wgpu-rs implementation)
            let offset_x = display_pos_x / display_width;
            let offset_y = display_pos_y / display_height;

            let matrix = [
                [2.0 / display_width, 0.0, 0.0, 0.0],
                [0.0, 2.0 / -display_height, 0.0, 0.0], // Note: negative height for correct Y orientation
                [0.0, 0.0, 1.0, 0.0],
                [-1.0 - offset_x * 2.0, 1.0 + offset_y * 2.0, 0.0, 1.0],
            ];

            self.update_uniform_buffer(queue, &matrix);
        }

        render_data.draw_list_offsets.clear();

        let mut vertex_count = 0;
        let mut index_count = 0;
        for draw_list in draw_data.draw_lists() {
            render_data
                .draw_list_offsets
                .push((vertex_count as i32, index_count as u32));
            vertex_count += draw_list.vtx_buffer().len();
            index_count += draw_list.idx_buffer().len();
        }

        let mut vertices = Vec::with_capacity(vertex_count * std::mem::size_of::<DrawVertPod>());
        let mut indices = Vec::with_capacity(index_count * std::mem::size_of::<DrawIdx>());

        for draw_list in draw_data.draw_lists() {
            // Safety: DrawVertPod is #[repr(transparent)] over ImDrawVert
            let vertices_pod: &[DrawVertPod] = unsafe {
                std::slice::from_raw_parts(
                    draw_list.vtx_buffer().as_ptr() as *const DrawVertPod,
                    draw_list.vtx_buffer().len(),
                )
            };
            vertices.extend_from_slice(bytemuck::cast_slice(vertices_pod));
            indices.extend_from_slice(bytemuck::cast_slice(draw_list.idx_buffer()));
        }

        // Copies in wgpu must be padded to 4 byte alignment
        indices.resize(
            indices.len() + COPY_BUFFER_ALIGNMENT as usize
                - indices.len() % COPY_BUFFER_ALIGNMENT as usize,
            0,
        );

        // Handle index buffer
        if render_data.index_buffer.is_none() || render_data.index_buffer_size < indices.len() {
            let buffer = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("imgui-wgpu index buffer"),
                contents: &indices,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });
            render_data.index_buffer = Some(buffer);
            render_data.index_buffer_size = indices.len();
        } else if let Some(buffer) = render_data.index_buffer.as_ref() {
            queue.write_buffer(buffer, 0, &indices);
        }

        // Handle vertex buffer
        if render_data.vertex_buffer.is_none() || render_data.vertex_buffer_size < vertices.len() {
            let buffer = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("imgui-wgpu vertex buffer"),
                contents: &vertices,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            render_data.vertex_buffer = Some(buffer);
            render_data.vertex_buffer_size = vertices.len();
        } else if let Some(buffer) = render_data.vertex_buffer.as_ref() {
            queue.write_buffer(buffer, 0, &vertices);
        }

        render_data
    }

    /// Updates the current uniform buffer containing the transform matrix
    fn update_uniform_buffer(&self, queue: &Queue, matrix: &[[f32; 4]; 4]) {
        let data = bytemuck::bytes_of(matrix);
        queue.write_buffer(&self.uniform_buffer, 0, data);
    }

    /// Load font texture from Dear ImGui context
    pub fn reload_font_texture(&mut self, imgui_ctx: &mut Context, device: &Device, queue: &Queue) {
        let mut fonts = imgui_ctx.font_atlas_mut();

        // Build the font atlas if not already built
        if !fonts.is_built() {
            fonts.build();
        }

        // Get actual texture data from the font atlas
        unsafe {
            if let Some((pixels_ptr, width, height)) = fonts.get_tex_data_ptr() {
                // Our Dear ImGui version uses RGBA format
                let bytes_per_pixel = 4;
                let texture_size = (width * height * bytes_per_pixel) as usize;

                // Copy texture data from Dear ImGui
                let texture_data = std::slice::from_raw_parts(pixels_ptr, texture_size);

                // Create font texture
                let font_texture = device.create_texture_with_data(
                    queue,
                    &TextureDescriptor {
                        label: Some("imgui-wgpu font texture"),
                        size: Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::Rgba8Unorm,
                        usage: TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    TextureDataOrder::default(),
                    texture_data,
                );

                let font_texture_view = font_texture.create_view(&TextureViewDescriptor::default());
                let font_sampler = device.create_sampler(&SamplerDescriptor {
                    label: Some("imgui-wgpu font sampler"),
                    address_mode_u: AddressMode::ClampToEdge,
                    address_mode_v: AddressMode::ClampToEdge,
                    address_mode_w: AddressMode::ClampToEdge,
                    mag_filter: FilterMode::Linear,
                    min_filter: FilterMode::Linear,
                    mipmap_filter: FilterMode::Linear,
                    ..Default::default()
                });

                let font_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("imgui-wgpu font texture bind group"),
                    layout: &self.texture_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&font_texture_view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&font_sampler),
                        },
                    ],
                });

                // Keep the old bind group for backward compatibility
                self.font_texture_bind_group = Some(font_texture_bind_group.clone());

                // Create WGPU texture and add to texture map
                let wgpu_texture = WgpuTexture {
                    texture: font_texture,
                    view: font_texture_view,
                    bind_group: font_texture_bind_group,
                };

                // Insert into texture map and get ID
                let font_texture_id = self.texture_map.insert(wgpu_texture);

                // Set the texture reference in Dear ImGui using the new texture management system
                let tex_ref = dear_imgui::create_texture_ref(font_texture_id);
                fonts.set_tex_ref(tex_ref);

                println!("Font texture loaded: {}x{} pixels", width, height);
            } else {
                println!("Failed to get font texture data");
            }
        }
    }

    /// Render the current imgui frame (compatibility method)
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        draw_data: &DrawData,
    ) -> RendererResult<()> {
        let render_data = self.render_data.take();
        self.render_data = Some(self.prepare(draw_data, render_data, queue, device));

        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("imgui render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.split_render(draw_data, self.render_data.as_ref().unwrap(), &mut rpass)
    }

    /// Render the current imgui frame with render pass
    pub fn render_with_renderpass<'r>(
        &'r mut self,
        draw_data: &DrawData,
        queue: &Queue,
        device: &Device,
        rpass: &mut RenderPass<'r>,
    ) -> RendererResult<()> {
        let render_data = self.render_data.take();
        self.render_data = Some(self.prepare(draw_data, render_data, queue, device));
        self.split_render(draw_data, self.render_data.as_ref().unwrap(), rpass)
    }

    /// Render the current imgui frame with prepared data
    pub fn split_render<'r>(
        &'r self,
        draw_data: &DrawData,
        render_data: &'r RenderData,
        rpass: &mut RenderPass<'r>,
    ) -> RendererResult<()> {
        if !render_data.render {
            return Ok(());
        }

        let vertex_buffer = render_data.vertex_buffer.as_ref().unwrap();
        if vertex_buffer.size() == 0 {
            return Ok(());
        }

        let index_buffer = render_data.index_buffer.as_ref().unwrap();
        if index_buffer.size() == 0 {
            return Ok(());
        }

        // Setup viewport (following official implementation)
        let [display_width, display_height] = draw_data.display_size();
        let [scale_x, scale_y] = draw_data.framebuffer_scale();
        let viewport_width = scale_x * display_width;
        let viewport_height = scale_y * display_height;

        rpass.set_viewport(0.0, 0.0, viewport_width, viewport_height, 0.0, 1.0);
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
        rpass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);

        // Execute all the imgui render work
        let fb_size = [viewport_width, viewport_height]; // Use actual framebuffer size for clipping
        for (draw_list, bases) in draw_data
            .draw_lists()
            .zip(render_data.draw_list_offsets.iter())
        {
            self.render_draw_list(
                rpass,
                draw_list,
                fb_size, // Use framebuffer size for proper clipping
                draw_data.display_pos(),
                draw_data.framebuffer_scale(),
                *bases,
            )?;
        }

        Ok(())
    }

    /// Render a given DrawList from imgui onto a wgpu frame
    fn render_draw_list<'render>(
        &'render self,
        rpass: &mut RenderPass<'render>,
        draw_list: &DrawList,
        fb_size: [f32; 2],
        clip_off: [f32; 2],
        clip_scale: [f32; 2],
        (vertex_base, index_base): (i32, u32),
    ) -> RendererResult<()> {
        let mut start = index_base;

        for cmd in draw_list.commands() {
            match cmd {
                crate::DrawCmd::Elements { count, cmd_params } => {
                    // Project scissor/clipping rectangles into framebuffer space
                    let clip_min_x = (cmd_params.clip_rect[0] - clip_off[0]) * clip_scale[0];
                    let clip_min_y = (cmd_params.clip_rect[1] - clip_off[1]) * clip_scale[1];
                    let clip_max_x = (cmd_params.clip_rect[2] - clip_off[0]) * clip_scale[0];
                    let clip_max_y = (cmd_params.clip_rect[3] - clip_off[1]) * clip_scale[1];

                    // Clamp to viewport as set_scissor_rect() won't accept values that are off bounds
                    let clip_min_x = clip_min_x.max(0.0);
                    let clip_min_y = clip_min_y.max(0.0);
                    let clip_max_x = clip_max_x.min(fb_size[0]);
                    let clip_max_y = clip_max_y.min(fb_size[1]);

                    // Set scissors on the renderpass
                    let end = start + count as u32;
                    if clip_max_x > clip_min_x && clip_max_y > clip_min_y {
                        let x = clip_min_x as u32;
                        let y = clip_min_y as u32;
                        let w = (clip_max_x - clip_min_x) as u32;
                        let h = (clip_max_y - clip_min_y) as u32;

                        if w > 0 && h > 0 {
                            rpass.set_scissor_rect(x, y, w, h);

                            // Choose the appropriate texture bind group based on texture ID
                            let texture_bind_group = {
                                let tex_id = cmd_params.texture_id.id();
                                if tex_id == 0 {
                                    // Default texture for null/invalid texture ID
                                    &self.default_texture_bind_group
                                } else if tex_id == 1 && self.font_texture_bind_group.is_some() {
                                    // Legacy font texture (for backward compatibility)
                                    self.font_texture_bind_group.as_ref().unwrap()
                                } else if let Some(wgpu_texture) =
                                    self.texture_map.get(tex_id as u64)
                                {
                                    // Modern texture management
                                    &wgpu_texture.bind_group
                                } else {
                                    // Fallback to default texture if texture not found
                                    &self.default_texture_bind_group
                                }
                            };

                            rpass.set_bind_group(1, texture_bind_group, &[]);

                            // Draw the current batch of vertices with the renderpass
                            rpass.draw_indexed(start..end, vertex_base, 0..1);
                        }
                    }

                    // Increment the index regardless of whether or not this batch was drawn
                    start = end;
                }
                crate::DrawCmd::ResetRenderState => {
                    // Handle reset render state if needed
                }
                crate::DrawCmd::RawCallback { .. } => {
                    // Skip raw callbacks for now
                }
            }
        }
        Ok(())
    }
}

// Multi-viewport support will be added in a future version
