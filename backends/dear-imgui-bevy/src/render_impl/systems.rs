//! Dear ImGui rendering systems

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupEntry, BindingResource, Buffer, BufferDescriptor, BufferId,
            BufferUsages, CachedRenderPipelineId, DynamicUniformBuffer, PipelineCache,
            SpecializedRenderPipelines, TexelCopyBufferLayout,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        texture::GpuImage,
        view::ExtractedView,
    },
};
use std::collections::HashMap;

use crate::render_impl::pipeline::{ImguiPipeline, ImguiPipelineKey, ImguiTransform};

/// Texture ID for Dear ImGui textures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImguiTextureId {
    /// Managed texture (font texture, etc.)
    Managed(MainEntity, u32),
    /// User texture
    User(u64),
}

/// Transform buffer for Dear ImGui
#[derive(Resource, Default)]
pub struct ImguiTransforms {
    /// Uniform buffer
    pub buffer: DynamicUniformBuffer<ImguiTransform>,
    /// The Entity is from the main world
    pub offsets: HashMap<MainEntity, u32>,
    /// Bind group
    pub bind_group: Option<(BufferId, BindGroup)>,
}

/// Texture bind groups for Dear ImGui
#[derive(Resource, Default)]
pub struct ImguiTextureBindGroups(pub HashMap<ImguiTextureId, BindGroup>);

impl std::ops::Deref for ImguiTextureBindGroups {
    type Target = HashMap<ImguiTextureId, BindGroup>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ImguiTextureBindGroups {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Render data for a single Dear ImGui view
#[derive(Default)]
pub struct ImguiRenderViewData {
    /// Vertex data
    pub vertex_data: Vec<u8>,
    /// Index data  
    pub index_data: Vec<u32>,
    /// Vertex buffer
    pub vertex_buffer: Option<Buffer>,
    /// Index buffer
    pub index_buffer: Option<Buffer>,
    /// Vertex buffer capacity
    pub vertex_buffer_capacity: usize,
    /// Index buffer capacity
    pub index_buffer_capacity: usize,
    /// Draw commands
    pub draw_commands: Vec<DrawCommand>,
    /// Pipeline key
    pub key: Option<ImguiPipelineKey>,
    /// Pixels per point
    pub pixels_per_point: f32,
    /// Target size
    pub target_size: UVec2,
    /// Keep flag for cleanup
    pub keep: bool,
}

/// Draw command for Dear ImGui
#[derive(Debug)]
pub struct DrawCommand {
    /// Clip rectangle
    pub clip_rect: [f32; 4], // [x, y, width, height]
    /// Number of indices
    pub indices_count: usize,
    /// Texture ID
    pub texture_id: ImguiTextureId,
}

/// Render data for all Dear ImGui views
#[derive(Resource, Default)]
pub struct ImguiRenderData(pub HashMap<MainEntity, ImguiRenderViewData>);

/// Pipelines for Dear ImGui
#[derive(Resource, Default)]
pub struct ImguiPipelines(pub HashMap<MainEntity, CachedRenderPipelineId>);

/// System parameter for Dear ImGui rendering
#[derive(SystemParam)]
pub struct ImguiRenderParams<'w> {
    pub render_data: ResMut<'w, ImguiRenderData>,
    pub pipelines: ResMut<'w, ImguiPipelines>,
    pub pipeline_cache: Res<'w, PipelineCache>,
    pub specialized_pipelines: ResMut<'w, SpecializedRenderPipelines<ImguiPipeline>>,
    pub imgui_pipeline: Res<'w, ImguiPipeline>,
    pub render_device: Res<'w, RenderDevice>,
    pub render_queue: Res<'w, RenderQueue>,
}

/// Prepare Dear ImGui transforms system
pub fn prepare_imgui_transforms_system(
    mut imgui_transforms: ResMut<ImguiTransforms>,
    views: Query<&ExtractedView>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    imgui_pipeline: Res<ImguiPipeline>,
) {
    imgui_transforms.buffer.clear();
    imgui_transforms.offsets.clear();

    for view in &views {
        let transform = ImguiTransform {
            scale: Vec2::new(2.0 / view.viewport.z as f32, -2.0 / view.viewport.w as f32),
            translation: Vec2::new(-1.0, 1.0),
        };

        let offset = imgui_transforms.buffer.push(&transform);
        imgui_transforms
            .offsets
            .insert(view.retained_view_entity.main_entity, offset);
    }

    imgui_transforms
        .buffer
        .write_buffer(&render_device, &render_queue);

    if let Some(buffer) = imgui_transforms.buffer.buffer() {
        match imgui_transforms.bind_group {
            Some((id, _)) if buffer.id() == id => {}
            _ => {
                let bind_group = render_device.create_bind_group(
                    "imgui_transform_bind_group",
                    &imgui_pipeline.transform_bind_group_layout,
                    &[BindGroupEntry {
                        binding: 0,
                        resource: imgui_transforms.buffer.binding().unwrap(),
                    }],
                );
                imgui_transforms.bind_group = Some((buffer.id(), bind_group));
            }
        }
    }
}

/// Prepare Dear ImGui render data system
pub fn prepare_imgui_render_data_system(
    mut params: ImguiRenderParams,
    views: Query<&ExtractedView>,
    extracted_data: Option<Res<crate::systems::ExtractedImguiDrawData>>,
) {
    let Some(extracted_data) = extracted_data else {
        return;
    };

    // Clear old data
    for (_, view_data) in params.render_data.0.iter_mut() {
        view_data.keep = false;
    }

    // Process each view
    for view in &views {
        let main_entity = view.retained_view_entity.main_entity;

        // Get extracted draw data for this view
        if let Some(ref draw_data) = extracted_data.draw_data {
            // Get or create view data
            let view_data = params.render_data.0.entry(main_entity).or_default();
            view_data.keep = true;

            // Update view data
            view_data.pixels_per_point = 1.0; // TODO: Get from actual display scale
            view_data.target_size = UVec2::new(view.viewport.z as u32, view.viewport.w as u32);

            // Create or update pipeline
            let pipeline_key = ImguiPipelineKey {
                hdr: false, // TODO: Get HDR from camera
            };
            view_data.key = Some(pipeline_key);

            // Get or create pipeline
            let pipeline_id = params.specialized_pipelines.specialize(
                &params.pipeline_cache,
                &params.imgui_pipeline,
                pipeline_key,
            );
            info!(
                "Created pipeline {:?} for entity {:?}",
                pipeline_id, main_entity
            );
            params.pipelines.0.insert(main_entity, pipeline_id);

            // Convert draw data to vertex/index data (after pipeline setup)
            convert_draw_data_to_buffers(
                draw_data,
                view_data,
                main_entity,
                &params.render_device,
                &params.render_queue,
            );
        } else {
            // Still mark as kept even without draw data
            let view_data = params.render_data.0.entry(main_entity).or_default();
            view_data.keep = true;
        }
    }

    // Remove old view data
    params.render_data.0.retain(|_, view_data| view_data.keep);
}

/// Convert Dear ImGui draw data to vertex/index buffers
fn convert_draw_data_to_buffers(
    draw_data: &dear_imgui::OwnedDrawData,
    view_data: &mut ImguiRenderViewData,
    main_entity: MainEntity,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    // Clear previous data
    view_data.vertex_data.clear();
    view_data.index_data.clear();
    view_data.draw_commands.clear();

    let mut vertex_offset = 0u32;
    let mut index_offset = 0u32;

    // Process each draw list
    let draw_data_ref = draw_data.draw_data().unwrap();
    for draw_list in draw_data_ref.draw_lists() {
        let vtx_buffer = draw_list.vtx_buffer();
        let idx_buffer = draw_list.idx_buffer();

        // Convert vertices
        for vertex in vtx_buffer {
            // ImGui vertex: pos (f32, f32), uv (f32, f32), col (u32)
            view_data
                .vertex_data
                .extend_from_slice(&vertex.pos[0].to_le_bytes());
            view_data
                .vertex_data
                .extend_from_slice(&vertex.pos[1].to_le_bytes());
            view_data
                .vertex_data
                .extend_from_slice(&vertex.uv[0].to_le_bytes());
            view_data
                .vertex_data
                .extend_from_slice(&vertex.uv[1].to_le_bytes());
            view_data
                .vertex_data
                .extend_from_slice(&vertex.col.to_le_bytes());
        }

        // Convert indices
        for &index in idx_buffer {
            view_data.index_data.push(vertex_offset + index as u32);
        }

        // Process draw commands
        for cmd in draw_list.commands() {
            match cmd {
                dear_imgui::draw_data::DrawCmd::Elements { count, cmd_params } => {
                    let texture_id = ImguiTextureId::Managed(
                        main_entity, // Use the main entity
                        cmd_params.texture_id.id() as u32,
                    );

                    view_data.draw_commands.push(DrawCommand {
                        clip_rect: [
                            cmd_params.clip_rect[0],
                            cmd_params.clip_rect[1],
                            cmd_params.clip_rect[2],
                            cmd_params.clip_rect[3],
                        ],
                        indices_count: count,
                        texture_id,
                    });

                    index_offset += count as u32;
                }
                _ => {
                    // Handle other command types if needed
                }
            }
        }

        vertex_offset += vtx_buffer.len() as u32;
    }

    // Create or update vertex buffer
    let vertex_data_size = view_data.vertex_data.len();
    if vertex_data_size > 0 {
        if view_data.vertex_buffer_capacity < vertex_data_size {
            view_data.vertex_buffer = Some(render_device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("imgui_vertex_buffer"),
                size: (vertex_data_size * 2) as u64, // Allocate extra space
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            view_data.vertex_buffer_capacity = vertex_data_size * 2;
        }

        if let Some(ref buffer) = view_data.vertex_buffer {
            render_queue.write_buffer(buffer, 0, &view_data.vertex_data);
        }
    }

    // Create or update index buffer
    let index_data_size = view_data.index_data.len() * std::mem::size_of::<u32>();
    if index_data_size > 0 {
        if view_data.index_buffer_capacity < index_data_size {
            view_data.index_buffer = Some(render_device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("imgui_index_buffer"),
                size: (index_data_size * 2) as u64, // Allocate extra space
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            view_data.index_buffer_capacity = index_data_size * 2;
        }

        if let Some(ref buffer) = view_data.index_buffer {
            let index_bytes: Vec<u8> = view_data
                .index_data
                .iter()
                .flat_map(|&i| i.to_le_bytes())
                .collect();

            render_queue.write_buffer(buffer, 0, &index_bytes);
        }
    }
}

/// System to create texture bind groups for Dear ImGui textures
pub fn queue_imgui_bind_groups_system(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    imgui_pipeline: Res<ImguiPipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    imgui_render_data: Res<ImguiRenderData>,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    // Clear existing bind groups
    texture_bind_groups.clear();

    // Create bind groups for all textures used in the current frame
    for (entity, render_data) in imgui_render_data.0.iter() {
        for draw_command in &render_data.draw_commands {
            let texture_id = draw_command.texture_id;

            // Skip if we already have a bind group for this texture
            if texture_bind_groups.contains_key(&texture_id) {
                continue;
            }

            match texture_id {
                ImguiTextureId::Managed(main_entity, tex_id) => {
                    // For the font texture (tex_id == 0), create a basic white texture
                    // TODO: In the future, we should extract the actual font texture from ImGui
                    if tex_id == 0 {
                        // Create a basic white texture for now
                        let width = 256u32;
                        let height = 256u32;
                        let bytes_per_pixel = 4u32;

                        // Create white texture data
                        let texture_data = vec![255u8; (width * height * bytes_per_pixel) as usize];

                        // Create the font texture
                        let texture = render_device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("imgui_font_texture"),
                            size: wgpu::Extent3d {
                                width,
                                height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING
                                | wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        // Upload the texture data
                        render_queue.write_texture(
                            texture.as_image_copy(),
                            &texture_data,
                            TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(width * bytes_per_pixel),
                                rows_per_image: Some(height),
                            },
                            texture.size(),
                        );

                        let texture_view =
                            texture.create_view(&wgpu::TextureViewDescriptor::default());

                        let sampler = render_device.create_sampler(&wgpu::SamplerDescriptor {
                            label: Some("imgui_font_sampler"),
                            address_mode_u: wgpu::AddressMode::ClampToEdge,
                            address_mode_v: wgpu::AddressMode::ClampToEdge,
                            address_mode_w: wgpu::AddressMode::ClampToEdge,
                            mag_filter: wgpu::FilterMode::Linear,
                            min_filter: wgpu::FilterMode::Linear,
                            mipmap_filter: wgpu::FilterMode::Nearest,
                            ..Default::default()
                        });

                        let bind_group = render_device.create_bind_group(
                            "imgui_font_texture_bind_group",
                            &imgui_pipeline.texture_bind_group_layout,
                            &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::TextureView(&texture_view),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::Sampler(&sampler),
                                },
                            ],
                        );

                        texture_bind_groups.insert(texture_id, bind_group);
                        info!("Created white font texture bind group for texture_id: {:?}, size: {}x{}", texture_id, width, height);
                    }
                }
                ImguiTextureId::User(_) => {
                    // Handle user textures if needed
                }
            }
        }
    }
}
