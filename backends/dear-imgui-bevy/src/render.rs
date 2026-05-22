//! Render-world extraction data for the Bevy backend.
//!
//! BEVY-080 clones the thread-safe [`FrameSnapshot`](dear_imgui_rs::render::snapshot::FrameSnapshot)
//! produced by the main-world lifecycle and associates it with the Bevy cameras that should receive
//! ImGui overlay rendering. BEVY-090 then prepares renderer-facing CPU batches, shader and pipeline
//! descriptors, and a camera-driven overlay pass without borrowing raw ImGui draw data across
//! worlds.

use bevy_app::App;
use bevy_asset::{Assets, Handle, uuid_handle};
use bevy_camera::{Camera, NormalizedRenderTarget, RenderTarget};
use bevy_core_pipeline::{Core2d, Core2dSystems, Core3d, Core3dSystems};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_mesh::VertexBufferLayout;
use bevy_render::{
    Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderSystems,
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, BindingType,
        BlendState, Buffer, BufferAddress, BufferBindingType, BufferDescriptor, BufferSize,
        BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d,
        FragmentState, IndexFormat, LoadOp, MultisampleState, Operations, Origin3d, PipelineCache,
        PrimitiveState, PrimitiveTopology, RawBufferVec, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor,
        ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, StoreOp,
        TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
        TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
        TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexFormat, VertexState,
        VertexStepMode,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
    view::{ExtractedView, Msaa, ViewTarget},
};
use bevy_shader::Shader;
use bevy_window::PrimaryWindow;
use bytemuck::{Pod, Zeroable};
use dear_imgui_rs as imgui;
use imgui::render::{DrawCmdSnapshot, DrawIdx, TextureBinding};
use std::collections::HashMap;
use std::mem::size_of;

/// Stable handle for the embedded Dear ImGui Bevy renderer shader.
pub const IMGUI_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("2c893cad-60d2-4e92-8544-4ab807ed9c5a");

const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

/// Vertex shader entry point used by the Bevy-native ImGui pipeline.
pub const IMGUI_VERTEX_ENTRY_POINT: &str = "vs_main";
/// Fragment shader entry point used by the Bevy-native ImGui pipeline.
pub const IMGUI_FRAGMENT_ENTRY_POINT: &str = "fs_main";

/// WGSL source for the Bevy-native Dear ImGui renderer.
///
/// BEVY-090 keeps this shader local to the Bevy backend instead of reusing
/// `dear-imgui-wgpu`, because Bevy owns render schedules, target formats, and pipeline
/// specialization.
pub const IMGUI_SHADER_SOURCE: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct ImguiUniforms {
    mvp: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: ImguiUniforms;

@group(0) @binding(1)
var imgui_sampler: sampler;

@group(1) @binding(0)
var imgui_texture: texture_2d<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.mvp * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color * textureSample(imgui_texture, imgui_sampler, in.uv);
}
"#;

/// Per-frame uniform data used by the Dear ImGui shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ImguiUniforms {
    /// Orthographic projection matrix that maps ImGui display coordinates to clip space.
    pub mvp: [[f32; 4]; 4],
}

impl ImguiUniforms {
    /// Create uniforms for an ImGui draw data display rectangle.
    #[must_use]
    pub fn from_display_rect(display_pos: [f32; 2], display_size: [f32; 2]) -> Self {
        let left = display_pos[0];
        let right = display_pos[0] + display_size[0];
        let top = display_pos[1];
        let bottom = display_pos[1] + display_size[1];
        Self {
            mvp: [
                [2.0 / (right - left), 0.0, 0.0, 0.0],
                [0.0, 2.0 / (top - bottom), 0.0, 0.0],
                [0.0, 0.0, 0.5, 0.0],
                [
                    (right + left) / (left - right),
                    (top + bottom) / (bottom - top),
                    0.5,
                    1.0,
                ],
            ],
        }
    }
}

/// Marker proving the render feature is compiled in.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct RenderFeature;

/// Camera/render-target association for an extracted ImGui overlay frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImguiCameraTarget {
    /// Main-world camera entity that should receive the ImGui overlay.
    pub camera: Entity,
    /// Camera order, preserved so the renderer can match Bevy's camera ordering.
    pub order: isize,
    /// Normalized render target resolved from the camera and current primary window.
    pub target: NormalizedRenderTarget,
}

/// GPU vertex layout used by the Bevy-native ImGui renderer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ImguiGpuVertex {
    /// Clip-space input position before the ImGui orthographic transform.
    pub position: [f32; 2],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Packed Dear ImGui RGBA color.
    pub color: u32,
}

impl From<imgui::render::DrawVert> for ImguiGpuVertex {
    fn from(value: imgui::render::DrawVert) -> Self {
        Self {
            position: value.pos,
            uv: value.uv,
            color: value.col,
        }
    }
}

/// Vertex buffer layout consumed by the Bevy-native ImGui render pipeline.
#[must_use]
pub fn imgui_vertex_buffer_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: size_of::<ImguiGpuVertex>() as BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            VertexAttribute {
                format: VertexFormat::Unorm8x4,
                offset: 16,
                shader_location: 2,
            },
        ],
    }
}

/// Scissor rectangle in framebuffer coordinates for one ImGui draw command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Renderer-ready draw command prepared from an extracted [`FrameSnapshot`](imgui::render::FrameSnapshot).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImguiPreparedDraw {
    /// Main-world camera entity associated with this draw.
    pub camera: Entity,
    /// Camera order preserved from Bevy extraction.
    pub order: isize,
    /// Normalized render target associated with the camera.
    pub target: NormalizedRenderTarget,
    /// Texture binding requested by the ImGui draw command.
    pub texture: TextureBinding,
    /// Scissor rectangle after applying display position and framebuffer scale.
    pub scissor: ImguiScissorRect,
    /// Global index range inside [`ImguiPreparedRenderFrame::indices`].
    pub index_range: std::ops::Range<u32>,
    /// Global vertex offset to use with `draw_indexed`.
    pub vertex_offset: i32,
}

/// CPU-side renderer preparation result for the last extracted ImGui frame.
#[derive(Resource, Clone, Debug, Default)]
pub struct ImguiPreparedRenderFrame {
    frame_index: Option<u64>,
    uniforms: Option<ImguiUniforms>,
    vertices: Vec<ImguiGpuVertex>,
    indices: Vec<DrawIdx>,
    draws: Vec<ImguiPreparedDraw>,
    texture_request_count: usize,
}

impl ImguiPreparedRenderFrame {
    /// Frame index copied from the extracted frame.
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        self.frame_index
    }

    /// Uniforms derived from the source snapshot's display rectangle.
    #[must_use]
    pub fn uniforms(&self) -> Option<ImguiUniforms> {
        self.uniforms
    }

    /// Flattened ImGui vertices for the current extracted frame.
    #[must_use]
    pub fn vertices(&self) -> &[ImguiGpuVertex] {
        &self.vertices
    }

    /// Flattened ImGui indices for the current extracted frame.
    #[must_use]
    pub fn indices(&self) -> &[DrawIdx] {
        &self.indices
    }

    /// Renderer-ready draw commands grouped by extracted camera target.
    #[must_use]
    pub fn draws(&self) -> &[ImguiPreparedDraw] {
        &self.draws
    }

    /// Number of texture requests carried by the source snapshot.
    #[must_use]
    pub fn texture_request_count(&self) -> usize {
        self.texture_request_count
    }

    fn replace(
        &mut self,
        frame_index: u64,
        uniforms: ImguiUniforms,
        vertices: Vec<ImguiGpuVertex>,
        indices: Vec<DrawIdx>,
        draws: Vec<ImguiPreparedDraw>,
        texture_request_count: usize,
    ) {
        self.frame_index = Some(frame_index);
        self.uniforms = Some(uniforms);
        self.vertices = vertices;
        self.indices = indices;
        self.draws = draws;
        self.texture_request_count = texture_request_count;
    }

    fn clear(&mut self, frame_index: Option<u64>) {
        self.frame_index = frame_index;
        self.uniforms = None;
        self.vertices.clear();
        self.indices.clear();
        self.draws.clear();
        self.texture_request_count = 0;
    }
}

/// Optional GPU buffers populated when a real Bevy renderer has `RenderDevice` / `RenderQueue`.
#[derive(Resource)]
pub struct ImguiGpuBuffers {
    vertices: RawBufferVec<ImguiGpuVertex>,
    indices: RawBufferVec<DrawIdx>,
}

impl Default for ImguiGpuBuffers {
    fn default() -> Self {
        let mut vertices = RawBufferVec::new(BufferUsages::VERTEX);
        vertices.set_label(Some("dear_imgui_bevy_vertices"));
        let mut indices = RawBufferVec::new(BufferUsages::INDEX);
        indices.set_label(Some("dear_imgui_bevy_indices"));
        Self { vertices, indices }
    }
}

impl ImguiGpuBuffers {
    /// Number of vertices queued for upload.
    #[must_use]
    pub fn vertex_len(&self) -> usize {
        self.vertices.len()
    }

    /// Number of indices queued for upload.
    #[must_use]
    pub fn index_len(&self) -> usize {
        self.indices.len()
    }

    /// Whether both GPU buffers have been allocated at least once.
    #[must_use]
    pub fn has_uploaded_buffers(&self) -> bool {
        self.vertices.buffer().is_some() && self.indices.buffer().is_some()
    }

    fn vertex_buffer(&self) -> Option<&Buffer> {
        self.vertices.buffer()
    }

    fn index_buffer(&self) -> Option<&Buffer> {
        self.indices.buffer()
    }

    fn upload(
        &mut self,
        prepared: &ImguiPreparedRenderFrame,
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
    ) {
        self.vertices.clear();
        self.indices.clear();
        for vertex in prepared.vertices() {
            self.vertices.push(*vertex);
        }
        for index in prepared.indices() {
            self.indices.push(*index);
        }
        self.vertices.write_buffer(render_device, render_queue);
        self.indices.write_buffer(render_device, render_queue);
    }
}

/// Pipeline specialization key for one Bevy view target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ImguiPipelineKey {
    pub target_format: TextureFormat,
    pub sample_count: u32,
}

/// Bevy render pipeline descriptor source for Dear ImGui overlays.
#[derive(Resource, Clone)]
pub struct ImguiRenderPipeline {
    shader: Handle<Shader>,
    common_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
}

impl ImguiRenderPipeline {
    /// Shader handle used by the pipeline.
    #[must_use]
    pub fn shader(&self) -> &Handle<Shader> {
        &self.shader
    }

    /// Bind group layout for uniforms and sampler.
    #[must_use]
    pub fn common_layout(&self) -> &BindGroupLayoutDescriptor {
        &self.common_layout
    }

    /// Bind group layout for a single ImGui texture binding.
    #[must_use]
    pub fn texture_layout(&self) -> &BindGroupLayoutDescriptor {
        &self.texture_layout
    }
}

impl Default for ImguiRenderPipeline {
    fn default() -> Self {
        let common_layout = BindGroupLayoutDescriptor::new(
            "dear_imgui_bevy_common_layout",
            &[
                bevy_render::render_resource::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(size_of::<ImguiUniforms>() as u64),
                    },
                    count: None,
                },
                bevy_render::render_resource::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );
        let texture_layout = BindGroupLayoutDescriptor::new(
            "dear_imgui_bevy_texture_layout",
            &[bevy_render::render_resource::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        );
        Self {
            shader: IMGUI_SHADER_HANDLE,
            common_layout,
            texture_layout,
        }
    }
}

impl SpecializedRenderPipeline for ImguiRenderPipeline {
    type Key = ImguiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("dear_imgui_bevy_pipeline".into()),
            layout: vec![self.common_layout.clone(), self.texture_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some(IMGUI_VERTEX_ENTRY_POINT.into()),
                buffers: vec![imgui_vertex_buffer_layout()],
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some(IMGUI_FRAGMENT_ENTRY_POINT.into()),
                targets: vec![Some(ColorTargetState {
                    format: key.target_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            multisample: MultisampleState {
                count: key.sample_count,
                ..Default::default()
            },
            zero_initialize_workgroup_memory: true,
            ..Default::default()
        }
    }
}

/// GPU resources shared by all ImGui overlay draws.
#[derive(Resource)]
pub struct ImguiPipelineGpuResources {
    uniform_buffer: Buffer,
    common_bind_group: BindGroup,
    _fallback_texture: Texture,
    _fallback_view: TextureView,
    fallback_bind_group: BindGroup,
}

impl FromWorld for ImguiPipelineGpuResources {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let render_queue = world.resource::<RenderQueue>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ImguiRenderPipeline>();
        let common_layout = pipeline_cache.get_bind_group_layout(pipeline.common_layout());
        let texture_layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
        let uniform_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("dear_imgui_bevy_uniforms"),
            size: size_of::<ImguiUniforms>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = render_device.create_sampler(&SamplerDescriptor {
            label: Some("dear_imgui_bevy_sampler"),
            ..Default::default()
        });
        let common_bind_group = render_device.create_bind_group(
            Some("dear_imgui_bevy_common_bind_group"),
            &common_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        );
        let fallback_texture = render_device.create_texture(&TextureDescriptor {
            label: Some("dear_imgui_bevy_fallback_texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_texture_rows(
            render_queue,
            &fallback_texture,
            Origin3d::ZERO,
            1,
            1,
            4,
            &[255, 255, 255, 255],
        );
        let fallback_view = fallback_texture.create_view(&TextureViewDescriptor::default());
        let fallback_bind_group = render_device.create_bind_group(
            Some("dear_imgui_bevy_fallback_texture_bind_group"),
            &texture_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&fallback_view),
            }],
        );
        Self {
            uniform_buffer,
            common_bind_group,
            _fallback_texture: fallback_texture,
            _fallback_view: fallback_view,
            fallback_bind_group,
        }
    }
}

impl ImguiPipelineGpuResources {
    fn update_uniforms(&self, render_queue: &RenderQueue, uniforms: ImguiUniforms) {
        render_queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn fallback_bind_group(&self) -> &BindGroup {
        &self.fallback_bind_group
    }
}

struct ImguiRenderTexture {
    texture: Option<Texture>,
    _view: Option<TextureView>,
    bind_group: BindGroup,
}

struct ImguiTextureUpload<'a> {
    format: imgui::texture::TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &'a [u8],
}

/// Texture bind groups currently known to the Bevy-native ImGui renderer.
#[derive(Resource, Default)]
pub struct ImguiTextureBindGroups {
    textures: HashMap<TextureBinding, ImguiRenderTexture>,
}

impl ImguiTextureBindGroups {
    /// Register or replace a texture bind group for an ImGui texture binding.
    pub fn insert(&mut self, texture: TextureBinding, bind_group: BindGroup) {
        self.textures.insert(
            texture,
            ImguiRenderTexture {
                texture: None,
                _view: None,
                bind_group,
            },
        );
    }

    /// Remove a texture bind group.
    pub fn remove(&mut self, texture: &TextureBinding) {
        self.textures.remove(texture);
    }

    /// Number of registered texture bind groups.
    #[must_use]
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Whether no texture bind groups are currently registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    fn get(&self, texture: &TextureBinding) -> Option<&BindGroup> {
        self.textures
            .get(texture)
            .map(|texture| &texture.bind_group)
    }

    fn insert_render_texture(
        &mut self,
        texture: TextureBinding,
        render_texture: ImguiRenderTexture,
    ) {
        self.textures.insert(texture, render_texture);
    }
}

/// Pipeline ids queued for the current render frame, keyed by main-world camera entity.
#[derive(Resource, Default)]
pub struct ImguiQueuedPipelines {
    by_camera: HashMap<Entity, CachedRenderPipelineId>,
}

impl ImguiQueuedPipelines {
    /// Queued pipeline for a main-world camera entity.
    #[must_use]
    pub fn get(&self, camera: Entity) -> Option<CachedRenderPipelineId> {
        self.by_camera.get(&camera).copied()
    }

    /// Number of queued camera pipelines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_camera.len()
    }

    /// Whether no camera pipelines are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_camera.is_empty()
    }
}

/// Render-side copy of the last completed primary ImGui frame.
#[derive(Resource, Clone, Debug, Default)]
pub struct ImguiExtractedRenderFrame {
    frame_index: Option<u64>,
    snapshot: Option<imgui::render::snapshot::FrameSnapshot>,
    camera_targets: Vec<ImguiCameraTarget>,
}

impl ImguiExtractedRenderFrame {
    /// Frame index copied from [`ImguiFrameOutput`].
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        self.frame_index
    }

    /// Snapshot copied from the main/UI world.
    #[must_use]
    pub fn snapshot(&self) -> Option<&imgui::render::snapshot::FrameSnapshot> {
        self.snapshot.as_ref()
    }

    /// Camera targets associated with the extracted snapshot.
    #[must_use]
    pub fn camera_targets(&self) -> &[ImguiCameraTarget] {
        &self.camera_targets
    }

    fn replace(
        &mut self,
        frame_index: u64,
        snapshot: imgui::render::snapshot::FrameSnapshot,
        camera_targets: Vec<ImguiCameraTarget>,
    ) {
        self.frame_index = Some(frame_index);
        self.snapshot = Some(snapshot);
        self.camera_targets = camera_targets;
    }

    fn clear(&mut self, frame_index: u64) {
        self.frame_index = (frame_index > 0).then_some(frame_index);
        self.snapshot = None;
        self.camera_targets.clear();
    }
}

#[derive(Resource, Default)]
struct ImguiRenderExtractionInstalled;

pub(crate) fn install_render_extraction(app: &mut App) {
    install_imgui_shader_asset(app);

    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };

    if render_app
        .world()
        .contains_resource::<ImguiRenderExtractionInstalled>()
    {
        return;
    }

    render_app
        .init_resource::<ImguiExtractedRenderFrame>()
        .init_resource::<ImguiPreparedRenderFrame>()
        .init_resource::<ImguiGpuBuffers>()
        .init_resource::<ImguiRenderPipeline>()
        .init_resource::<SpecializedRenderPipelines<ImguiRenderPipeline>>()
        .init_resource::<ImguiTextureBindGroups>()
        .init_resource::<ImguiQueuedPipelines>()
        .init_gpu_resource::<ImguiPipelineGpuResources>()
        .insert_resource(ImguiRenderExtractionInstalled)
        .add_systems(ExtractSchedule, extract_imgui_render_frame)
        .add_systems(
            Render,
            (prepare_imgui_render_frame, queue_imgui_pipelines)
                .chain()
                .in_set(RenderSystems::Queue),
        )
        .add_systems(
            Render,
            upload_imgui_buffers.in_set(RenderSystems::PrepareResources),
        )
        .add_systems(
            Render,
            prepare_imgui_texture_bind_groups.in_set(RenderSystems::PrepareBindGroups),
        )
        .add_systems(
            Core2d,
            render_imgui_overlay.in_set(Core2dSystems::PostProcess),
        )
        .add_systems(
            Core3d,
            render_imgui_overlay.in_set(Core3dSystems::PostProcess),
        );
}

fn install_imgui_shader_asset(app: &mut App) {
    app.init_resource::<Assets<Shader>>();
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            IMGUI_SHADER_HANDLE.id(),
            Shader::from_wgsl(IMGUI_SHADER_SOURCE, "dear_imgui_bevy/imgui.wgsl"),
        )
        .expect("UUID shader handles are always valid asset ids");
}

fn extract_imgui_render_frame(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    output: Extract<Res<crate::ImguiFrameOutput>>,
    primary_window: Extract<Query<Entity, With<PrimaryWindow>>>,
    cameras: Extract<Query<(Entity, &Camera, &RenderTarget)>>,
) {
    let Some(snapshot) = output.snapshot().cloned() else {
        extracted.clear(output.frame_index());
        return;
    };
    let primary_window = primary_window.single().ok();
    let camera_targets = collect_camera_targets(primary_window, cameras.iter());
    extracted.replace(output.frame_index(), snapshot, camera_targets);
}

fn collect_camera_targets<'w>(
    primary_window: Option<Entity>,
    cameras: impl Iterator<Item = (Entity, &'w Camera, &'w RenderTarget)>,
) -> Vec<ImguiCameraTarget> {
    let mut targets = cameras
        .filter(|(_, camera, _)| camera.is_active)
        .filter_map(|(entity, camera, target)| {
            target
                .normalize(primary_window)
                .map(|target| ImguiCameraTarget {
                    camera: entity,
                    order: camera.order,
                    target,
                })
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|target| target.order);
    targets
}

fn prepare_imgui_render_frame(
    extracted: Res<ImguiExtractedRenderFrame>,
    mut prepared: ResMut<ImguiPreparedRenderFrame>,
) {
    let Some(snapshot) = extracted.snapshot() else {
        prepared.clear(extracted.frame_index());
        return;
    };

    let Some(frame_index) = extracted.frame_index() else {
        prepared.clear(None);
        return;
    };

    let uniforms =
        ImguiUniforms::from_display_rect(snapshot.draw.display_pos, snapshot.draw.display_size);
    let (vertices, indices, draws) =
        prepare_snapshot_draw_data(snapshot, extracted.camera_targets());
    prepared.replace(
        frame_index,
        uniforms,
        vertices,
        indices,
        draws,
        snapshot.texture_requests.len(),
    );
}

fn upload_imgui_buffers(
    prepared: Res<ImguiPreparedRenderFrame>,
    mut gpu_buffers: ResMut<ImguiGpuBuffers>,
    render_device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
    gpu_resources: Option<Res<ImguiPipelineGpuResources>>,
) {
    if let (Some(render_device), Some(render_queue)) = (render_device, render_queue) {
        if let (Some(gpu_resources), Some(uniforms)) = (gpu_resources, prepared.uniforms()) {
            gpu_resources.update_uniforms(&render_queue, uniforms);
        }
        gpu_buffers.upload(&prepared, &render_device, &render_queue);
    }
}

fn prepare_imgui_texture_bind_groups(
    extracted: Res<ImguiExtractedRenderFrame>,
    render_device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
    pipeline_cache: Option<Res<PipelineCache>>,
    pipeline: Res<ImguiRenderPipeline>,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    let (Some(render_device), Some(render_queue), Some(pipeline_cache)) =
        (render_device, render_queue, pipeline_cache)
    else {
        return;
    };

    let Some(snapshot) = extracted.snapshot() else {
        return;
    };

    for request in &snapshot.texture_requests {
        match &request.op {
            imgui::render::TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => {
                if let Some(render_texture) = create_imgui_render_texture(
                    &render_device,
                    &render_queue,
                    &pipeline_cache,
                    &pipeline,
                    ImguiTextureUpload {
                        format: *format,
                        width: *width,
                        height: *height,
                        row_pitch: *row_pitch,
                        pixels,
                    },
                ) {
                    texture_bind_groups
                        .insert_render_texture(TextureBinding::Managed(request.id), render_texture);
                }
            }
            imgui::render::TextureOp::Update { format, rects, .. } => {
                if let Some(render_texture) = texture_bind_groups
                    .textures
                    .get(&TextureBinding::Managed(request.id))
                    .and_then(|texture| texture.texture.as_ref())
                {
                    for rect in rects {
                        if let Some((pixels, row_pitch)) = convert_imgui_texture_pixels(
                            *format,
                            u32::from(rect.rect.w),
                            u32::from(rect.rect.h),
                            rect.row_pitch,
                            &rect.data,
                        ) {
                            write_texture_rows(
                                &render_queue,
                                render_texture,
                                Origin3d {
                                    x: u32::from(rect.rect.x),
                                    y: u32::from(rect.rect.y),
                                    z: 0,
                                },
                                u32::from(rect.rect.w),
                                u32::from(rect.rect.h),
                                row_pitch,
                                &pixels,
                            );
                        }
                    }
                }
            }
            imgui::render::TextureOp::Destroy => {
                texture_bind_groups.remove(&TextureBinding::Managed(request.id));
            }
        }
    }
}

fn create_imgui_render_texture(
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    upload: ImguiTextureUpload<'_>,
) -> Option<ImguiRenderTexture> {
    let (pixels, row_pitch) = convert_imgui_texture_pixels(
        upload.format,
        upload.width,
        upload.height,
        upload.row_pitch,
        upload.pixels,
    )?;
    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some("dear_imgui_bevy_texture"),
        size: Extent3d {
            width: upload.width,
            height: upload.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_texture_rows(
        render_queue,
        &texture,
        Origin3d::ZERO,
        upload.width,
        upload.height,
        row_pitch,
        &pixels,
    );
    let view = texture.create_view(&TextureViewDescriptor::default());
    let layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
    let bind_group = render_device.create_bind_group(
        Some("dear_imgui_bevy_texture_bind_group"),
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(&view),
        }],
    );
    Some(ImguiRenderTexture {
        texture: Some(texture),
        _view: Some(view),
        bind_group,
    })
}

fn convert_imgui_texture_pixels(
    format: imgui::texture::TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &[u8],
) -> Option<(Vec<u8>, u32)> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    match format {
        imgui::texture::TextureFormat::RGBA32 => {
            let dst_row_pitch = width.checked_mul(4)?;
            copy_or_repack_rows(pixels, row_pitch, dst_row_pitch, height)
                .map(|pixels| (pixels, dst_row_pitch as u32))
        }
        imgui::texture::TextureFormat::Alpha8 => {
            let mut rgba = vec![0; width.checked_mul(height)?.checked_mul(4)?];
            for row in 0..height {
                let src_start = row.checked_mul(row_pitch)?;
                let src_end = src_start.checked_add(width)?;
                if src_end > pixels.len() {
                    return None;
                }
                for (col, alpha) in pixels[src_start..src_end].iter().copied().enumerate() {
                    let dst = row.checked_mul(width)?.checked_add(col)?.checked_mul(4)?;
                    rgba[dst..dst + 4].copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
            Some((rgba, width.checked_mul(4)? as u32))
        }
    }
}

fn copy_or_repack_rows(
    pixels: &[u8],
    src_row_pitch: usize,
    dst_row_pitch: usize,
    rows: usize,
) -> Option<Vec<u8>> {
    if src_row_pitch < dst_row_pitch {
        return None;
    }
    let required_src = src_row_pitch.checked_mul(rows)?;
    if pixels.len() < required_src {
        return None;
    }
    if src_row_pitch == dst_row_pitch {
        return Some(pixels[..required_src].to_vec());
    }

    let mut out = vec![0; dst_row_pitch.checked_mul(rows)?];
    for row in 0..rows {
        let src = row.checked_mul(src_row_pitch)?;
        let dst = row.checked_mul(dst_row_pitch)?;
        out[dst..dst + dst_row_pitch].copy_from_slice(&pixels[src..src + dst_row_pitch]);
    }
    Some(out)
}

fn write_texture_rows(
    render_queue: &RenderQueue,
    texture: &Texture,
    origin: Origin3d,
    width: u32,
    height: u32,
    row_pitch: u32,
    pixels: &[u8],
) {
    if width == 0 || height == 0 || row_pitch == 0 {
        return;
    }

    let alignment = COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_row_pitch = row_pitch.div_ceil(alignment) * alignment;
    if padded_row_pitch == row_pitch {
        render_queue.write_texture(
            TexelCopyTextureInfo {
                texture: &**texture,
                mip_level: 0,
                origin,
                aspect: TextureAspect::All,
            },
            pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_pitch),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        return;
    }

    let row_pitch = usize::try_from(row_pitch).ok();
    let padded_row_pitch = usize::try_from(padded_row_pitch).ok();
    let height_usize = usize::try_from(height).ok();
    let (Some(row_pitch), Some(padded_row_pitch), Some(height_usize)) =
        (row_pitch, padded_row_pitch, height_usize)
    else {
        return;
    };
    let Some(required) = row_pitch.checked_mul(height_usize) else {
        return;
    };
    if pixels.len() < required {
        return;
    }
    let Some(padded_len) = padded_row_pitch.checked_mul(height_usize) else {
        return;
    };
    let mut padded = vec![0; padded_len];
    for row in 0..height_usize {
        let src = row * row_pitch;
        let dst = row * padded_row_pitch;
        padded[dst..dst + row_pitch].copy_from_slice(&pixels[src..src + row_pitch]);
    }
    render_queue.write_texture(
        TexelCopyTextureInfo {
            texture: &**texture,
            mip_level: 0,
            origin,
            aspect: TextureAspect::All,
        },
        &padded,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(padded_row_pitch as u32),
            rows_per_image: Some(height),
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn queue_imgui_pipelines(
    prepared: Res<ImguiPreparedRenderFrame>,
    views: Query<(&ExtractedView, Option<&Msaa>)>,
    pipeline_cache: Option<Res<PipelineCache>>,
    pipeline: Res<ImguiRenderPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ImguiRenderPipeline>>,
    mut queued: ResMut<ImguiQueuedPipelines>,
) {
    queued.by_camera.clear();

    let Some(pipeline_cache) = pipeline_cache else {
        return;
    };

    for (view, msaa) in &views {
        let camera = view.retained_view_entity.main_entity.id();
        if !prepared.draws().iter().any(|draw| draw.camera == camera) {
            continue;
        }
        let sample_count = msaa.map_or(1, Msaa::samples);
        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &pipeline,
            ImguiPipelineKey {
                target_format: view.target_format,
                sample_count,
            },
        );
        queued.by_camera.insert(camera, pipeline_id);
    }
}

#[derive(SystemParam)]
struct ImguiRenderPassParams<'w> {
    pipeline_cache: Option<Res<'w, PipelineCache>>,
    queued: Res<'w, ImguiQueuedPipelines>,
    prepared: Res<'w, ImguiPreparedRenderFrame>,
    gpu_buffers: Res<'w, ImguiGpuBuffers>,
    gpu_resources: Option<Res<'w, ImguiPipelineGpuResources>>,
    texture_bind_groups: Res<'w, ImguiTextureBindGroups>,
}

fn render_imgui_overlay(
    view: ViewQuery<(&ViewTarget, &ExtractedView)>,
    params: ImguiRenderPassParams,
    mut render_context: RenderContext,
) {
    let Some(pipeline_cache) = params.pipeline_cache else {
        return;
    };
    let Some(gpu_resources) = params.gpu_resources else {
        return;
    };
    if !params.gpu_buffers.has_uploaded_buffers() {
        return;
    }

    let (view_target, view) = view.into_inner();
    let camera = view.retained_view_entity.main_entity.id();
    let Some(pipeline_id) = params.queued.get(camera) else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
        return;
    };

    let drawable = params
        .prepared
        .draws()
        .iter()
        .filter(|draw| draw.camera == camera)
        .collect::<Vec<_>>();
    if drawable.is_empty() {
        return;
    }

    let color_attachment = view_target.get_color_attachment();
    let mut render_pass =
        render_context
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("dear_imgui_bevy_overlay_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: color_attachment.view,
                    depth_slice: color_attachment.depth_slice,
                    resolve_target: color_attachment.resolve_target,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, &gpu_resources.common_bind_group, &[]);
    if let Some(vertex_buffer) = params.gpu_buffers.vertex_buffer() {
        render_pass.set_vertex_buffer(0, *vertex_buffer.slice(..));
    } else {
        return;
    }
    if let Some(index_buffer) = params.gpu_buffers.index_buffer() {
        render_pass.set_index_buffer(*index_buffer.slice(..), IndexFormat::Uint16);
    } else {
        return;
    }

    for draw in drawable {
        let texture_bind_group = params
            .texture_bind_groups
            .get(&draw.texture)
            .unwrap_or_else(|| gpu_resources.fallback_bind_group());
        render_pass.set_bind_group(1, texture_bind_group, &[]);
        render_pass.set_scissor_rect(
            draw.scissor.x,
            draw.scissor.y,
            draw.scissor.width,
            draw.scissor.height,
        );
        render_pass.draw_indexed(draw.index_range.clone(), draw.vertex_offset, 0..1);
    }
}

fn prepare_snapshot_draw_data(
    snapshot: &imgui::render::FrameSnapshot,
    camera_targets: &[ImguiCameraTarget],
) -> (Vec<ImguiGpuVertex>, Vec<DrawIdx>, Vec<ImguiPreparedDraw>) {
    let vertex_count = snapshot
        .draw
        .draw_lists
        .iter()
        .map(|list| list.vtx.len())
        .sum();
    let index_count = snapshot
        .draw
        .draw_lists
        .iter()
        .map(|list| list.idx.len())
        .sum();
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);
    let mut draws = Vec::new();

    let mut list_vertex_base = 0usize;
    let mut list_index_base = 0usize;

    for list in &snapshot.draw.draw_lists {
        vertices.extend(list.vtx.iter().copied().map(ImguiGpuVertex::from));
        indices.extend(list.idx.iter().copied());

        for command in &list.commands {
            let DrawCmdSnapshot::Elements {
                count,
                clip_rect,
                texture,
                vtx_offset,
                idx_offset,
            } = command
            else {
                continue;
            };

            let Some(scissor) = scissor_from_clip_rect(&snapshot.draw, *clip_rect) else {
                continue;
            };
            let Some(index_start) = list_index_base.checked_add(*idx_offset) else {
                continue;
            };
            let Some(index_end) = index_start.checked_add(*count) else {
                continue;
            };
            let Some(vertex_offset) = list_vertex_base.checked_add(*vtx_offset) else {
                continue;
            };
            let Ok(index_start) = u32::try_from(index_start) else {
                continue;
            };
            let Ok(index_end) = u32::try_from(index_end) else {
                continue;
            };
            let Ok(vertex_offset) = i32::try_from(vertex_offset) else {
                continue;
            };

            for target in camera_targets {
                draws.push(ImguiPreparedDraw {
                    camera: target.camera,
                    order: target.order,
                    target: target.target.clone(),
                    texture: *texture,
                    scissor,
                    index_range: index_start..index_end,
                    vertex_offset,
                });
            }
        }

        list_vertex_base += list.vtx.len();
        list_index_base += list.idx.len();
    }

    (vertices, indices, draws)
}

fn scissor_from_clip_rect(
    draw: &imgui::render::DrawDataSnapshot,
    clip_rect: [f32; 4],
) -> Option<ImguiScissorRect> {
    let scale = draw.framebuffer_scale;
    let min_x = ((clip_rect[0] - draw.display_pos[0]) * scale[0]).floor();
    let min_y = ((clip_rect[1] - draw.display_pos[1]) * scale[1]).floor();
    let max_x = ((clip_rect[2] - draw.display_pos[0]) * scale[0]).ceil();
    let max_y = ((clip_rect[3] - draw.display_pos[1]) * scale[1]).ceil();

    let framebuffer_width = (draw.display_size[0] * scale[0]).ceil().max(0.0);
    let framebuffer_height = (draw.display_size[1] * scale[1]).ceil().max(0.0);

    let min_x = min_x.clamp(0.0, framebuffer_width);
    let min_y = min_y.clamp(0.0, framebuffer_height);
    let max_x = max_x.clamp(min_x, framebuffer_width);
    let max_y = max_y.clamp(min_y, framebuffer_height);

    let width = max_x - min_x;
    let height = max_y - min_y;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(ImguiScissorRect {
        x: min_x as u32,
        y: min_y as u32,
        width: width as u32,
        height: height as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_conversion_repackages_padded_rgba_rows() {
        let pixels = [
            1, 2, 3, 4, 9, 9, 9, 9, //
            5, 6, 7, 8, 8, 8, 8, 8,
        ];

        let (converted, row_pitch) =
            convert_imgui_texture_pixels(imgui::texture::TextureFormat::RGBA32, 1, 2, 8, &pixels)
                .expect("valid padded RGBA32 upload should convert");

        assert_eq!(row_pitch, 4);
        assert_eq!(converted, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn texture_conversion_expands_alpha8_to_white_rgba() {
        let pixels = [0, 128, 255, 64];

        let (converted, row_pitch) =
            convert_imgui_texture_pixels(imgui::texture::TextureFormat::Alpha8, 2, 2, 2, &pixels)
                .expect("valid Alpha8 upload should convert");

        assert_eq!(row_pitch, 8);
        assert_eq!(
            converted,
            [
                255, 255, 255, 0, 255, 255, 255, 128, //
                255, 255, 255, 255, 255, 255, 255, 64,
            ]
        );
    }
}
