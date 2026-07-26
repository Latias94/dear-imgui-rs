//! Render-world resources and renderer-owned state.

use super::prepare::{
    create_standard_imgui_sampler, create_texture_sampler_bind_group, write_texture_rows,
};
use super::*;

/// Camera/render-target association for an extracted ImGui overlay frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImguiCameraTarget {
    /// Context whose immutable snapshot is routed to this view.
    pub context_id: imgui::ContextId,
    /// Main-world route epoch captured with this target.
    pub route_epoch: u64,
    /// Main-world camera entity that should receive the ImGui overlay.
    pub camera: Entity,
    /// Stable Bevy render-view identity for this camera epoch.
    pub view: RetainedViewEntity,
    /// Explicit overlay order among Contexts routed to one view.
    pub order: isize,
    /// Bevy camera order captured when the route was resolved.
    pub camera_order: isize,
    /// Bevy render-graph schedule captured when the route was resolved.
    pub camera_schedule: bevy_ecs::schedule::InternedScheduleLabel,
    /// Normalized render target resolved from the camera and current primary window.
    pub target: NormalizedRenderTarget,
    /// Actual main-pass texture format of the extracted Bevy view.
    pub target_format: TextureFormat,
    /// Actual usages of the extracted Bevy main-pass texture.
    pub texture_usages: TextureUsages,
    /// Actual MSAA mode of the extracted Bevy view.
    pub msaa: Msaa,
    /// Physical size of the complete render target.
    pub physical_target_size: [u32; 2],
    /// Dear ImGui viewport whose draw data should be rendered into this target.
    pub viewport_id: Option<imgui::Id>,
    /// Physical camera viewport to use when rendering this overlay target.
    pub camera_viewport: Option<ImguiCameraViewport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImguiRenderRouteSnapshot {
    pub(super) context_id: imgui::ContextId,
    pub(super) route_epoch: u64,
    pub(super) route_entity: Option<Entity>,
    pub(super) camera: Entity,
    pub(super) order: isize,
    pub(super) camera_order: isize,
    pub(super) camera_schedule: bevy_ecs::schedule::InternedScheduleLabel,
    pub(super) target: NormalizedRenderTarget,
    pub(super) physical_target_size: [u32; 2],
    pub(super) viewport_id: Option<imgui::Id>,
    pub(super) camera_viewport: Option<ImguiCameraViewport>,
}

/// Legacy camera marker retained until the public-surface cleanup.
///
/// This marker no longer affects routing. Use [`crate::route::ImguiRenderRoute`] for explicit
/// routing; the primary Context uses deterministic `AutoPrimary` routing when no declaration
/// exists.
#[derive(Component, Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ImguiOverlayCamera;

/// Legacy camera marker retained until the public-surface cleanup.
///
/// This marker no longer affects routing. Secondary windows and offscreen targets receive no
/// automatic route, so they need no opt-out marker.
#[derive(Component, Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ImguiOverlayDisabled;

/// Physical viewport extracted from a Bevy camera for ImGui overlay rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiCameraViewport {
    /// Top-left physical framebuffer position.
    pub physical_position: [u32; 2],
    /// Physical framebuffer size.
    pub physical_size: [u32; 2],
}

impl From<&Viewport> for ImguiCameraViewport {
    fn from(viewport: &Viewport) -> Self {
        Self {
            physical_position: [viewport.physical_position.x, viewport.physical_position.y],
            physical_size: [viewport.physical_size.x, viewport.physical_size.y],
        }
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

/// Sampler state requested by Dear ImGui standard sampler callbacks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImguiSampler {
    /// Linear filtering, matching Dear ImGui's default WGPU backend sampler.
    #[default]
    Linear,
    /// Nearest filtering for pixel-art or explicitly nearest-sampled draw ranges.
    Nearest,
}

/// Renderer-ready draw command prepared from an extracted [`FrameSnapshot`](imgui::render::FrameSnapshot).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImguiPreparedDraw {
    /// Context that produced this draw command.
    pub context_id: imgui::ContextId,
    /// Main-world route epoch captured with this draw command.
    pub route_epoch: u64,
    /// Main-world camera entity associated with this draw.
    pub camera: Entity,
    /// Stable Bevy render-view identity associated with this draw.
    pub view: RetainedViewEntity,
    /// Explicit overlay order among Contexts routed to one view.
    pub order: isize,
    /// Bevy camera order captured with the view.
    pub camera_order: isize,
    /// Bevy render-graph schedule captured with the view.
    pub camera_schedule: bevy_ecs::schedule::InternedScheduleLabel,
    /// Normalized render target associated with the camera.
    pub target: NormalizedRenderTarget,
    /// Actual main-pass texture format associated with the view.
    pub target_format: TextureFormat,
    /// Actual main-pass texture usages associated with the view.
    pub texture_usages: TextureUsages,
    /// Actual camera MSAA mode associated with the view.
    pub msaa: Msaa,
    /// Physical size of the complete render target captured for this route epoch.
    pub physical_target_size: [u32; 2],
    /// Dear ImGui viewport that produced this draw command.
    pub viewport_id: Option<imgui::Id>,
    /// Texture binding requested by the ImGui draw command.
    pub texture: TextureBinding,
    /// Sampler requested by the active ImGui standard sampler callback state.
    pub sampler: ImguiSampler,
    /// Scissor rectangle after applying display position and framebuffer scale.
    pub scissor: ImguiScissorRect,
    /// Source framebuffer size used to derive `scissor`.
    pub framebuffer_size: [u32; 2],
    /// Physical camera viewport to use when rendering this draw.
    pub camera_viewport: Option<ImguiCameraViewport>,
    /// Global index range inside [`ImguiPreparedRenderFrame::indices`].
    pub index_range: std::ops::Range<u32>,
    /// Global vertex offset to use with `draw_indexed`.
    pub vertex_offset: i32,
}

/// CPU-side renderer preparation result for the last extracted ImGui frame.
#[derive(Resource, Clone, Debug, Default)]
pub struct ImguiPreparedRenderFrame {
    data: PreparedFrameData,
}

impl ImguiPreparedRenderFrame {
    /// Frame index copied from one extracted Context frame.
    #[must_use]
    pub fn frame_index(&self, context_id: imgui::ContextId) -> Option<u64> {
        self.data
            .contexts
            .get(&context_id)
            .map(|metadata| metadata.frame_index)
    }

    /// Contexts represented by the current prepared batch.
    pub fn context_ids(&self) -> impl Iterator<Item = imgui::ContextId> + '_ {
        self.data.contexts.keys().copied()
    }

    /// Uniforms for one Context routed to one Bevy view.
    #[must_use]
    pub fn uniforms_for_view(
        &self,
        context_id: imgui::ContextId,
        view: RetainedViewEntity,
    ) -> Option<ImguiUniforms> {
        self.data
            .uniforms_by_context_view
            .get(&(context_id, view))
            .copied()
    }

    /// Flattened ImGui vertices for the current extracted frame.
    #[must_use]
    pub fn vertices(&self) -> &[ImguiGpuVertex] {
        &self.data.vertices
    }

    /// Flattened ImGui indices for the current extracted frame.
    #[must_use]
    pub fn indices(&self) -> &[DrawIdx] {
        &self.data.indices
    }

    /// Renderer-ready draw commands grouped by extracted camera target.
    #[must_use]
    pub fn draws(&self) -> &[ImguiPreparedDraw] {
        &self.data.draws
    }

    /// Number of texture requests carried by one source snapshot.
    #[must_use]
    pub fn texture_request_count(&self, context_id: imgui::ContextId) -> usize {
        self.data
            .contexts
            .get(&context_id)
            .map(|metadata| metadata.texture_request_count)
            .unwrap_or_default()
    }

    pub(super) fn replace(&mut self, frame: PreparedFrameData) {
        self.data = frame;
    }

    pub(super) fn clear(&mut self) {
        self.data.clear();
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PreparedContextMetadata {
    pub(super) frame_index: u64,
    pub(super) texture_request_count: usize,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PreparedFrameData {
    pub(super) contexts: HashMap<imgui::ContextId, PreparedContextMetadata>,
    pub(super) uniforms_by_context_view:
        HashMap<(imgui::ContextId, RetainedViewEntity), ImguiUniforms>,
    pub(super) vertices: Vec<ImguiGpuVertex>,
    pub(super) indices: Vec<DrawIdx>,
    pub(super) draws: Vec<ImguiPreparedDraw>,
}

impl PreparedFrameData {
    fn clear(&mut self) {
        self.contexts.clear();
        self.uniforms_by_context_view.clear();
        self.vertices.clear();
        self.indices.clear();
        self.draws.clear();
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

    pub(super) fn vertex_buffer(&self) -> Option<&Buffer> {
        self.vertices.buffer()
    }

    pub(super) fn index_buffer(&self) -> Option<&Buffer> {
        self.indices.buffer()
    }

    pub(super) fn upload(
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
        pad_index_buffer_for_copy_alignment(&mut self.indices);
        self.vertices.write_buffer(render_device, render_queue);
        self.indices.write_buffer(render_device, render_queue);
    }
}

pub(super) fn pad_index_buffer_for_copy_alignment(indices: &mut RawBufferVec<DrawIdx>) {
    let byte_len = indices.len() * size_of::<DrawIdx>();
    if byte_len.is_multiple_of(COPY_BUFFER_ALIGNMENT as usize) {
        return;
    }

    debug_assert_eq!(size_of::<DrawIdx>(), 2);
    indices.push(DrawIdx::default());
}

/// GPU resources shared by all ImGui overlay draws.
#[derive(Resource)]
pub struct ImguiPipelineGpuResources {
    uniforms_by_context_view:
        HashMap<(imgui::ContextId, RetainedViewEntity), ImguiCameraUniformResources>,
    _fallback_texture: Texture,
    _fallback_view: TextureView,
    fallback_bind_group: BindGroup,
}

pub(super) struct ImguiCameraUniformResources {
    buffer: Buffer,
    bind_group: BindGroup,
}

impl FromWorld for ImguiPipelineGpuResources {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let render_queue = world.resource::<RenderQueue>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ImguiRenderPipeline>();
        let texture_layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
        let sampler = create_standard_imgui_sampler(render_device, ImguiSampler::Linear);
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
        let fallback_bind_group = create_texture_sampler_bind_group(
            render_device,
            &texture_layout,
            Some("dear_imgui_bevy_fallback_texture_bind_group"),
            &fallback_view,
            &sampler,
        );
        Self {
            uniforms_by_context_view: HashMap::new(),
            _fallback_texture: fallback_texture,
            _fallback_view: fallback_view,
            fallback_bind_group,
        }
    }
}

impl ImguiPipelineGpuResources {
    pub(super) fn prepare_camera_uniforms(
        &mut self,
        prepared: &ImguiPreparedRenderFrame,
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
        pipeline_cache: &PipelineCache,
        pipeline: &ImguiRenderPipeline,
    ) {
        let active_context_views = prepared
            .draws()
            .iter()
            .map(|draw| (draw.context_id, draw.view))
            .collect::<std::collections::HashSet<_>>();
        self.uniforms_by_context_view
            .retain(|key, _| active_context_views.contains(key));

        for (context_id, view) in active_context_views {
            let Some(uniforms) = prepared.uniforms_for_view(context_id, view) else {
                continue;
            };
            let resources = self
                .uniforms_by_context_view
                .entry((context_id, view))
                .or_insert_with(|| {
                    create_camera_uniform_resources(render_device, pipeline_cache, pipeline)
                });
            render_queue.write_buffer(&resources.buffer, 0, bytemuck::bytes_of(&uniforms));
        }
    }

    pub(super) fn update_camera_uniforms(
        &self,
        context_id: imgui::ContextId,
        view: RetainedViewEntity,
        render_queue: &RenderQueue,
        uniforms: ImguiUniforms,
    ) -> Option<&BindGroup> {
        let resources = self.uniforms_by_context_view.get(&(context_id, view))?;
        render_queue.write_buffer(&resources.buffer, 0, bytemuck::bytes_of(&uniforms));
        Some(&resources.bind_group)
    }

    #[must_use]
    pub fn uniform_bind_group_count(&self) -> usize {
        self.uniforms_by_context_view.len()
    }

    pub(super) fn fallback_bind_group(&self) -> &BindGroup {
        &self.fallback_bind_group
    }

    pub(super) fn take_context(
        &mut self,
        context_id: imgui::ContextId,
    ) -> Vec<ImguiCameraUniformResources> {
        self.uniforms_by_context_view
            .extract_if(|(candidate, _), _| *candidate == context_id)
            .map(|(_, resources)| resources)
            .collect()
    }
}

fn create_camera_uniform_resources(
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
) -> ImguiCameraUniformResources {
    let common_layout = pipeline_cache.get_bind_group_layout(pipeline.common_layout());
    let uniform_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("dear_imgui_bevy_uniforms_camera"),
        size: size_of::<ImguiUniforms>() as BufferAddress,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = render_device.create_bind_group(
        Some("dear_imgui_bevy_common_bind_group"),
        &common_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    );
    ImguiCameraUniformResources {
        buffer: uniform_buffer,
        bind_group,
    }
}

pub(super) struct ImguiRenderTexture {
    pub(super) texture: Option<Texture>,
    pub(super) _view: Option<TextureView>,
    pub(super) extent: Option<[u32; 2]>,
    pub(super) linear_bind_group: BindGroup,
    pub(super) nearest_bind_group: BindGroup,
}

pub(super) struct ImguiTextureUpload<'a> {
    pub(super) format: imgui::texture::TextureFormat,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) row_pitch: usize,
    pub(super) pixels: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ImguiTextureViewCompatibility {
    pub(super) texture_usage: TextureUsages,
    pub(super) view_usage: Option<TextureUsages>,
    pub(super) sample_count: u32,
    pub(super) texture_dimension: TextureDimension,
    pub(super) depth_or_array_layers: u32,
    pub(super) view_dimension: Option<TextureViewDimension>,
    pub(super) format: TextureFormat,
    pub(super) aspect: TextureAspect,
}

impl ImguiTextureViewCompatibility {
    pub(super) fn from_gpu_image(gpu_image: &GpuImage) -> Self {
        let texture_descriptor = &gpu_image.texture_descriptor;
        let view_descriptor = gpu_image.texture_view_descriptor.as_ref();
        Self {
            texture_usage: texture_descriptor.usage,
            view_usage: view_descriptor.and_then(|descriptor| descriptor.usage),
            sample_count: texture_descriptor.sample_count,
            texture_dimension: texture_descriptor.dimension,
            depth_or_array_layers: texture_descriptor.size.depth_or_array_layers,
            view_dimension: view_descriptor.and_then(|descriptor| descriptor.dimension),
            format: view_descriptor
                .and_then(|descriptor| descriptor.format)
                .unwrap_or(texture_descriptor.format),
            aspect: view_descriptor.map_or(TextureAspect::All, |descriptor| descriptor.aspect),
        }
    }

    pub(super) fn supports_imgui_sampling(self, device_features: WgpuFeatures) -> bool {
        if !self
            .resolved_view_usage()
            .contains(TextureUsages::TEXTURE_BINDING)
        {
            return false;
        }
        if self.sample_count != 1 || self.resolved_view_dimension() != TextureViewDimension::D2 {
            return false;
        }

        matches!(
            self.format
                .sample_type(Some(self.aspect), Some(device_features)),
            Some(TextureSampleType::Float { filterable: true })
        )
    }

    fn resolved_view_usage(self) -> TextureUsages {
        let usage = self.view_usage.unwrap_or_else(TextureUsages::empty);
        if usage.is_empty() {
            self.texture_usage
        } else {
            usage
        }
    }

    fn resolved_view_dimension(self) -> TextureViewDimension {
        let default_dimension = match self.texture_dimension {
            TextureDimension::D1 => TextureViewDimension::D1,
            TextureDimension::D2 => {
                if self.depth_or_array_layers == 1 {
                    TextureViewDimension::D2
                } else {
                    TextureViewDimension::D2Array
                }
            }
            TextureDimension::D3 => TextureViewDimension::D3,
        };
        self.view_dimension.unwrap_or(default_dimension)
    }
}

/// Error returned when external texture registration conflicts with a live managed alias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiTextureBindGroupError {
    /// The renderer currently uses this legacy ID to identify a Context-managed texture.
    ManagedTextureIdInUse { texture: imgui::TextureId },
}

impl std::fmt::Display for ImguiTextureBindGroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManagedTextureIdInUse { texture } => write!(
                f,
                "texture ID {} is an active Bevy managed-texture alias",
                texture.id()
            ),
        }
    }
}

impl std::error::Error for ImguiTextureBindGroupError {}

/// Texture bind groups currently known to the Bevy-native ImGui renderer.
#[derive(Resource, Default)]
pub struct ImguiTextureBindGroups {
    pub(super) textures: HashMap<TextureBinding, ImguiRenderTexture>,
    pub(super) bevy_image_bindings: HashSet<TextureBinding>,
    pub(super) managed_texture_ids: HashMap<SnapshotTextureId, imgui::TextureId>,
    pub(super) managed_texture_aliases: HashMap<imgui::TextureId, SnapshotTextureId>,
    /// Managed texture identities sealed by a Destroy request until the renderer has completed
    /// the snapshot epoch that carried that request. Keeping the epoch prevents an older delayed
    /// snapshot from reviving a texture while allowing high-churn identities to be reclaimed.
    pub(super) destroyed_managed_textures: HashMap<SnapshotTextureId, u64>,
    pub(super) next_managed_texture_id: u64,
}

impl ImguiTextureBindGroups {
    /// Register or replace a bind group for an external ImGui texture ID.
    ///
    /// Context-managed textures are intentionally excluded: their bind groups may only change
    /// while processing the matching snapshot request and feedback pair.
    ///
    /// # Errors
    ///
    /// Returns [`ImguiTextureBindGroupError::ManagedTextureIdInUse`] when `texture` is the active
    /// legacy alias of a Context-managed texture.
    pub fn insert(
        &mut self,
        texture: imgui::TextureId,
        bind_group: BindGroup,
    ) -> Result<(), ImguiTextureBindGroupError> {
        self.validate_external_texture_id(texture)?;
        self.insert_binding(TextureBinding::Legacy(texture), bind_group);
        Ok(())
    }

    pub(super) fn insert_binding(&mut self, texture: TextureBinding, bind_group: BindGroup) {
        self.bevy_image_bindings.remove(&texture);
        self.textures.insert(
            texture,
            ImguiRenderTexture {
                texture: None,
                _view: None,
                extent: None,
                linear_bind_group: bind_group.clone(),
                nearest_bind_group: bind_group,
            },
        );
    }

    /// Remove the bind group for an external ImGui texture ID.
    ///
    /// # Errors
    ///
    /// Returns [`ImguiTextureBindGroupError::ManagedTextureIdInUse`] when `texture` is the active
    /// legacy alias of a Context-managed texture.
    pub fn remove(&mut self, texture: imgui::TextureId) -> Result<(), ImguiTextureBindGroupError> {
        self.validate_external_texture_id(texture)?;
        self.remove_binding(&TextureBinding::Legacy(texture));
        Ok(())
    }

    pub(super) fn remove_binding(&mut self, texture: &TextureBinding) {
        self.textures.remove(texture);
        self.bevy_image_bindings.remove(texture);
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

    pub(super) fn get(
        &self,
        texture: &TextureBinding,
        sampler: ImguiSampler,
    ) -> Option<&BindGroup> {
        let managed_alias = match texture {
            TextureBinding::Legacy(texture_id) => self
                .managed_texture_aliases
                .get(texture_id)
                .copied()
                .map(TextureBinding::Managed),
            TextureBinding::Managed(_) => None,
        };
        self.textures
            .get(managed_alias.as_ref().unwrap_or(texture))
            .map(|texture| match sampler {
                ImguiSampler::Linear => &texture.linear_bind_group,
                ImguiSampler::Nearest => &texture.nearest_bind_group,
            })
    }

    pub(super) fn insert_render_texture(
        &mut self,
        texture: TextureBinding,
        render_texture: ImguiRenderTexture,
    ) {
        self.bevy_image_bindings.remove(&texture);
        self.textures.insert(texture, render_texture);
    }

    pub(super) fn managed_texture_id(&mut self, id: SnapshotTextureId) -> imgui::TextureId {
        if let Some(texture_id) = self.managed_texture_ids.get(&id) {
            return *texture_id;
        }
        loop {
            let sequence = self
                .next_managed_texture_id
                .checked_add(1)
                .expect("Bevy managed texture ID space exhausted");
            assert!(
                sequence < MANAGED_TEXTURE_NAMESPACE,
                "Bevy managed texture ID namespace exhausted"
            );
            self.next_managed_texture_id = sequence;
            let texture_id = imgui::TextureId::new(MANAGED_TEXTURE_NAMESPACE | sequence);
            if self.managed_texture_aliases.contains_key(&texture_id)
                || self
                    .textures
                    .contains_key(&TextureBinding::Legacy(texture_id))
            {
                continue;
            }
            self.managed_texture_ids.insert(id, texture_id);
            self.managed_texture_aliases.insert(texture_id, id);
            return texture_id;
        }
    }

    pub(super) fn validate_external_texture_id(
        &self,
        texture: imgui::TextureId,
    ) -> Result<(), ImguiTextureBindGroupError> {
        if self.managed_texture_aliases.contains_key(&texture) {
            Err(ImguiTextureBindGroupError::ManagedTextureIdInUse { texture })
        } else {
            Ok(())
        }
    }

    pub(super) fn remove_managed_texture(&mut self, id: SnapshotTextureId) {
        self.remove_binding(&TextureBinding::Managed(id));
        if let Some(texture_id) = self.managed_texture_ids.remove(&id) {
            self.managed_texture_aliases.remove(&texture_id);
        }
    }

    pub(super) fn destroy_managed_texture(&mut self, id: SnapshotTextureId, destroy_epoch: u64) {
        self.destroyed_managed_textures
            .entry(id)
            .and_modify(|epoch| *epoch = (*epoch).max(destroy_epoch))
            .or_insert(destroy_epoch);
        self.remove_managed_texture(id);
    }

    pub(super) fn managed_texture_is_destroyed(&self, id: SnapshotTextureId) -> bool {
        self.destroyed_managed_textures.contains_key(&id)
    }

    pub(super) fn accepts_managed_texture_upload(&self, id: SnapshotTextureId) -> bool {
        !self.managed_texture_is_destroyed(id)
    }

    pub(super) fn prune_destroyed_managed_textures(
        &mut self,
        completion_watermarks: &HashMap<imgui::ContextId, u64>,
    ) {
        self.destroyed_managed_textures.retain(|id, destroy_epoch| {
            completion_watermarks
                .get(&snapshot_texture_context_id(*id))
                .is_none_or(|watermark| *destroy_epoch > *watermark)
        });
    }

    pub(super) fn take_managed_renderer_state(
        &mut self,
        context_id: imgui::ContextId,
    ) -> Vec<ImguiRenderTexture> {
        let mut released = Vec::new();
        let managed_ids = self
            .managed_texture_ids
            .keys()
            .filter(|id| snapshot_texture_context_id(**id) == context_id)
            .copied()
            .collect::<Vec<_>>();
        for id in managed_ids {
            let texture_id = self
                .managed_texture_ids
                .remove(&id)
                .expect("selected managed texture identity must still exist");
            self.managed_texture_aliases.remove(&texture_id);
            let binding = TextureBinding::Managed(id);
            self.bevy_image_bindings.remove(&binding);
            if let Some(texture) = self.textures.remove(&binding) {
                released.push(texture);
            }
        }
        let orphaned = self
            .textures
            .keys()
            .copied()
            .filter(|binding| {
                matches!(
                    binding,
                    TextureBinding::Managed(id)
                        if snapshot_texture_context_id(*id) == context_id
                )
            })
            .collect::<Vec<_>>();
        for binding in orphaned {
            if let Some(texture) = self.textures.remove(&binding) {
                released.push(texture);
            }
        }
        self.destroyed_managed_textures
            .retain(|id, _| snapshot_texture_context_id(*id) != context_id);
        released
    }

    pub(super) fn insert_bevy_image(&mut self, texture: TextureBinding, bind_group: BindGroup) {
        self.textures.insert(
            texture,
            ImguiRenderTexture {
                texture: None,
                _view: None,
                extent: None,
                linear_bind_group: bind_group.clone(),
                nearest_bind_group: bind_group,
            },
        );
        self.bevy_image_bindings.insert(texture);
    }

    pub(super) fn retain_bevy_image_bindings(&mut self, active_bindings: &HashSet<TextureBinding>) {
        let stale_bindings = self
            .bevy_image_bindings
            .difference(active_bindings)
            .copied()
            .collect::<Vec<_>>();
        for binding in stale_bindings {
            self.remove_binding(&binding);
        }
    }
}

fn snapshot_texture_context_id(id: SnapshotTextureId) -> imgui::ContextId {
    match id {
        SnapshotTextureId::User(id) => id.context_id(),
        SnapshotTextureId::FontAtlas { context, .. } => context,
    }
}

/// Render-world copy of main-world Bevy image texture registrations.
#[derive(Resource, Clone, Debug, Default)]
pub struct ImguiExtractedBevyTextures {
    textures: Vec<(imgui::TextureId, bevy_asset::AssetId<Image>)>,
}

impl ImguiExtractedBevyTextures {
    /// Registered Dear ImGui texture id to Bevy image asset id mappings.
    #[must_use]
    pub fn textures(&self) -> &[(imgui::TextureId, bevy_asset::AssetId<Image>)] {
        &self.textures
    }

    /// Number of extracted Bevy image texture mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Whether no Bevy image texture mappings are extracted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    pub(super) fn replace(
        &mut self,
        textures: Vec<(imgui::TextureId, bevy_asset::AssetId<Image>)>,
    ) {
        self.textures = textures;
    }
}

/// Pipeline ids queued for the current render frame, keyed by stable Bevy view identity.
#[derive(Resource, Default)]
pub struct ImguiQueuedPipelines {
    pub(super) by_view: HashMap<RetainedViewEntity, CachedRenderPipelineId>,
}

impl ImguiQueuedPipelines {
    /// Queued pipeline for one stable Bevy view.
    #[must_use]
    pub fn get(&self, view: RetainedViewEntity) -> Option<CachedRenderPipelineId> {
        self.by_view.get(&view).copied()
    }

    /// Number of queued camera pipelines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_view.len()
    }

    /// Whether no camera pipelines are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_view.is_empty()
    }
}

#[derive(Debug)]
struct ImguiExtractedContextFrame {
    frame_index: u64,
    snapshot: Option<imgui::render::snapshot::FrameSnapshot>,
    route_snapshots: Vec<ImguiRenderRouteSnapshot>,
    camera_targets: Vec<ImguiCameraTarget>,
    texture_feedback: Vec<imgui::render::snapshot::TextureFeedback>,
}

/// Render-side owner of the latest extracted frame for every Dear ImGui Context.
#[derive(Resource, Debug, Default)]
pub struct ImguiExtractedRenderFrame {
    frames: HashMap<imgui::ContextId, ImguiExtractedContextFrame>,
    completion_watermarks: HashMap<imgui::ContextId, u64>,
    route_epoch: u64,
}

impl ImguiExtractedRenderFrame {
    /// Frame index copied from the main-world Context output.
    #[must_use]
    pub fn frame_index(&self, context_id: imgui::ContextId) -> Option<u64> {
        self.frames.get(&context_id).map(|frame| frame.frame_index)
    }

    /// Snapshot moved from the main world for `context_id`, if it is not terminal yet.
    #[must_use]
    pub fn snapshot(
        &self,
        context_id: imgui::ContextId,
    ) -> Option<&imgui::render::snapshot::FrameSnapshot> {
        self.frames
            .get(&context_id)
            .and_then(|frame| frame.snapshot.as_ref())
    }

    /// Contexts that currently have extracted frame metadata.
    pub fn context_ids(&self) -> impl Iterator<Item = imgui::ContextId> + '_ {
        self.frames.keys().copied()
    }

    /// Latest main-world route epoch observed by extraction.
    #[must_use]
    pub const fn route_epoch(&self) -> u64 {
        self.route_epoch
    }

    /// Camera targets associated with one extracted Context snapshot.
    #[must_use]
    pub fn camera_targets(&self, context_id: imgui::ContextId) -> &[ImguiCameraTarget] {
        self.frames
            .get(&context_id)
            .map_or(&[], |frame| frame.camera_targets.as_slice())
    }

    /// Highest contiguous completion watermark confirmed by the owning core Context.
    #[must_use]
    pub fn completion_watermark(&self, context_id: imgui::ContextId) -> u64 {
        self.completion_watermarks
            .get(&context_id)
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn begin_extraction(
        &mut self,
        route_epoch: u64,
        completion_watermarks: HashMap<imgui::ContextId, u64>,
    ) {
        self.abandon_all();
        self.frames.clear();
        self.route_epoch = route_epoch;
        for (context_id, watermark) in completion_watermarks {
            let current = self.completion_watermarks.entry(context_id).or_default();
            *current = (*current).max(watermark);
        }
    }

    pub(super) fn replace(
        &mut self,
        context_id: imgui::ContextId,
        frame: crate::context::PendingFrame,
        route_snapshots: Vec<ImguiRenderRouteSnapshot>,
    ) {
        debug_assert_eq!(frame.snapshot.epoch().context_id(), context_id);
        let previous = self.frames.insert(
            context_id,
            ImguiExtractedContextFrame {
                frame_index: frame.frame_index,
                snapshot: Some(frame.snapshot),
                route_snapshots,
                camera_targets: Vec::new(),
                texture_feedback: Vec::new(),
            },
        );
        drop(previous);
    }

    pub(super) fn route_snapshots(
        &self,
        context_id: imgui::ContextId,
    ) -> &[ImguiRenderRouteSnapshot] {
        self.frames
            .get(&context_id)
            .map_or(&[], |frame| frame.route_snapshots.as_slice())
    }

    pub(super) fn replace_camera_targets(
        &mut self,
        context_id: imgui::ContextId,
        camera_targets: Vec<ImguiCameraTarget>,
    ) {
        if let Some(frame) = self.frames.get_mut(&context_id) {
            frame.camera_targets = camera_targets;
        }
    }

    pub(super) fn extend_texture_feedback(
        &mut self,
        context_id: imgui::ContextId,
        feedback: impl IntoIterator<Item = imgui::render::snapshot::TextureFeedback>,
    ) {
        if let Some(frame) = self.frames.get_mut(&context_id) {
            frame.texture_feedback.extend(feedback);
        }
    }

    pub(super) fn commit_all(&mut self) {
        for frame in self.frames.values_mut() {
            let feedback = std::mem::take(&mut frame.texture_feedback);
            if let Some(snapshot) = frame.snapshot.take() {
                let _ = snapshot.commit(feedback);
            }
        }
    }

    pub(super) fn remove_context(&mut self, context_id: imgui::ContextId) {
        let removed = self.frames.remove(&context_id);
        drop(removed);
        self.completion_watermarks.remove(&context_id);
    }

    fn abandon_all(&mut self) {
        for frame in self.frames.values_mut() {
            frame.texture_feedback.clear();
            drop(frame.snapshot.take());
        }
    }
}

impl Drop for ImguiExtractedRenderFrame {
    fn drop(&mut self) {
        self.abandon_all();
    }
}

#[derive(Resource, Default)]
pub(super) struct ImguiRenderExtractionInstalled;

#[derive(Resource, Debug, Default)]
pub(super) struct ImguiRenderDeviceState {
    generation: u64,
}

impl ImguiRenderDeviceState {
    pub(super) fn advance(&mut self) -> (u64, bool) {
        let recovering = self.generation != 0;
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Bevy render device generation space exhausted");
        (self.generation, recovering)
    }
}

#[derive(Debug)]
enum ImguiRendererReleasePhase {
    Live {
        generation: u64,
    },
    Requested {
        generation: u64,
    },
    Detached {
        generation: u64,
        packet: Option<ImguiRendererReleasePacket>,
    },
    ResourcesReleased {
        generation: u64,
    },
    RecoveryDetached {
        generation: u64,
        device_generation: u64,
        packet: Option<ImguiRendererReleasePacket>,
    },
    RecoveryResourcesReleased {
        generation: u64,
        device_generation: u64,
    },
}

#[derive(Default)]
pub(super) struct ImguiRendererReleasePacket {
    pub(super) textures: Vec<ImguiRenderTexture>,
    pub(super) uniforms: Vec<ImguiCameraUniformResources>,
}

impl std::fmt::Debug for ImguiRendererReleasePacket {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImguiRendererReleasePacket")
            .field("texture_count", &self.textures.len())
            .field("uniform_count", &self.uniforms.len())
            .finish()
    }
}

impl ImguiRendererReleasePacket {
    pub(super) fn new(
        textures: Vec<ImguiRenderTexture>,
        uniforms: Vec<ImguiCameraUniformResources>,
    ) -> Self {
        Self { textures, uniforms }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.textures.is_empty() && self.uniforms.is_empty()
    }

    fn append(&mut self, mut other: Self) {
        self.textures.append(&mut other.textures);
        self.uniforms.append(&mut other.uniforms);
    }
}

#[derive(Debug, Default)]
struct ImguiRendererReleaseState {
    next_generation: u64,
    device_generation: u64,
    phases: HashMap<imgui::ContextId, ImguiRendererReleasePhase>,
}

#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct ImguiRendererReleases {
    state: Arc<Mutex<ImguiRendererReleaseState>>,
}

impl ImguiRendererReleases {
    fn state(&self) -> MutexGuard<'_, ImguiRendererReleaseState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn admit(&self, context_id: imgui::ContextId) -> ImguiRendererReleaseLease {
        let mut state = self.state();
        assert!(
            !state.phases.contains_key(&context_id),
            "a Bevy renderer release lease already exists for Context {context_id:?}"
        );
        let generation = state
            .next_generation
            .checked_add(1)
            .expect("Bevy renderer release generation space exhausted");
        state.next_generation = generation;
        state
            .phases
            .insert(context_id, ImguiRendererReleasePhase::Live { generation });
        ImguiRendererReleaseLease {
            context_id,
            generation,
            releases: self.clone(),
        }
    }

    pub(crate) fn release_requested(&self, context_id: imgui::ContextId) -> bool {
        matches!(
            self.state().phases.get(&context_id),
            Some(
                ImguiRendererReleasePhase::Requested { .. }
                    | ImguiRendererReleasePhase::Detached { .. }
                    | ImguiRendererReleasePhase::ResourcesReleased { .. }
                    | ImguiRendererReleasePhase::RecoveryDetached { .. }
                    | ImguiRendererReleasePhase::RecoveryResourcesReleased { .. }
            )
        )
    }

    pub(crate) fn release_requested_contexts(&self) -> HashSet<imgui::ContextId> {
        self.state()
            .phases
            .iter()
            .filter_map(|(context_id, phase)| {
                (!matches!(phase, ImguiRendererReleasePhase::Live { .. })).then_some(*context_id)
            })
            .collect()
    }

    pub(crate) fn recovery_requested(&self, context_id: imgui::ContextId) -> bool {
        matches!(
            self.state().phases.get(&context_id),
            Some(
                ImguiRendererReleasePhase::RecoveryDetached { .. }
                    | ImguiRendererReleasePhase::RecoveryResourcesReleased { .. }
            )
        )
    }

    pub(super) fn requested_releases(&self) -> Vec<(imgui::ContextId, u64)> {
        let mut requested = self
            .state()
            .phases
            .iter()
            .filter_map(|(context_id, phase)| match phase {
                ImguiRendererReleasePhase::Requested { generation } => {
                    Some((*context_id, *generation))
                }
                ImguiRendererReleasePhase::Live { .. }
                | ImguiRendererReleasePhase::Detached { .. }
                | ImguiRendererReleasePhase::ResourcesReleased { .. }
                | ImguiRendererReleasePhase::RecoveryDetached { .. }
                | ImguiRendererReleasePhase::RecoveryResourcesReleased { .. } => None,
            })
            .collect::<Vec<_>>();
        requested.sort_by_key(|(context_id, _)| context_id.get().get());
        requested
    }

    pub(super) fn registered_contexts(&self) -> Vec<imgui::ContextId> {
        let mut contexts = self.state().phases.keys().copied().collect::<Vec<_>>();
        contexts.sort_by_key(|context_id| context_id.get().get());
        contexts
    }

    pub(super) fn begin_device_recovery(
        &self,
        device_generation: u64,
        mut packets: HashMap<imgui::ContextId, ImguiRendererReleasePacket>,
    ) -> bool {
        let mut state = self.state();
        if device_generation <= state.device_generation {
            return false;
        }
        state.device_generation = device_generation;
        for (context_id, phase) in &mut state.phases {
            let packet = packets.remove(context_id).unwrap_or_default();
            match phase {
                ImguiRendererReleasePhase::Live { generation } => {
                    *phase = ImguiRendererReleasePhase::RecoveryDetached {
                        generation: *generation,
                        device_generation,
                        packet: Some(packet),
                    };
                }
                ImguiRendererReleasePhase::Requested { generation } => {
                    *phase = ImguiRendererReleasePhase::Detached {
                        generation: *generation,
                        packet: Some(packet),
                    };
                }
                ImguiRendererReleasePhase::Detached {
                    packet: detached_packet,
                    ..
                } => {
                    detached_packet
                        .as_mut()
                        .expect("detached renderer release must retain its resource packet")
                        .append(packet);
                }
                ImguiRendererReleasePhase::ResourcesReleased { .. } => {
                    assert!(
                        packet.is_empty(),
                        "renderer resources reappeared after Context teardown released them"
                    );
                }
                ImguiRendererReleasePhase::RecoveryDetached {
                    device_generation: pending_generation,
                    packet: detached_packet,
                    ..
                } => {
                    *pending_generation = device_generation;
                    detached_packet
                        .as_mut()
                        .expect("detached renderer recovery must retain its resource packet")
                        .append(packet);
                }
                ImguiRendererReleasePhase::RecoveryResourcesReleased {
                    device_generation: pending_generation,
                    ..
                } => {
                    assert!(
                        packet.is_empty(),
                        "renderer resources reappeared after device recovery released them"
                    );
                    *pending_generation = device_generation;
                }
            }
        }
        drop(packets);
        true
    }

    pub(super) fn acknowledge_release(
        &self,
        context_id: imgui::ContextId,
        generation: u64,
        packet: ImguiRendererReleasePacket,
    ) -> bool {
        let mut state = self.state();
        let Some(phase) = state.phases.get_mut(&context_id) else {
            return false;
        };
        let expected = match phase {
            ImguiRendererReleasePhase::Requested { generation } => *generation,
            ImguiRendererReleasePhase::Live { .. }
            | ImguiRendererReleasePhase::Detached { .. }
            | ImguiRendererReleasePhase::ResourcesReleased { .. }
            | ImguiRendererReleasePhase::RecoveryDetached { .. }
            | ImguiRendererReleasePhase::RecoveryResourcesReleased { .. } => return false,
        };
        if expected != generation {
            return false;
        }
        *phase = ImguiRendererReleasePhase::Detached {
            generation,
            packet: Some(packet),
        };
        true
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.state().phases.len()
    }
}

#[derive(Debug)]
pub(crate) struct ImguiRendererReleaseLease {
    context_id: imgui::ContextId,
    generation: u64,
    releases: ImguiRendererReleases,
}

impl ImguiRendererReleaseLease {
    pub(crate) fn request_release(&self) -> bool {
        let mut state = self.releases.state();
        let Some(phase) = state.phases.get_mut(&self.context_id) else {
            return true;
        };
        let previous = std::mem::replace(
            phase,
            ImguiRendererReleasePhase::Requested {
                generation: self.generation,
            },
        );
        match previous {
            ImguiRendererReleasePhase::Live { generation } if generation == self.generation => {
                false
            }
            ImguiRendererReleasePhase::Requested { generation }
                if generation == self.generation =>
            {
                false
            }
            previous @ (ImguiRendererReleasePhase::Detached { generation, .. }
            | ImguiRendererReleasePhase::ResourcesReleased { generation })
                if generation == self.generation =>
            {
                *phase = previous;
                true
            }
            ImguiRendererReleasePhase::RecoveryDetached {
                generation, packet, ..
            } if generation == self.generation => {
                *phase = ImguiRendererReleasePhase::Detached { generation, packet };
                true
            }
            ImguiRendererReleasePhase::RecoveryResourcesReleased { generation, .. }
                if generation == self.generation =>
            {
                *phase = ImguiRendererReleasePhase::ResourcesReleased { generation };
                true
            }
            previous => {
                *phase = previous;
                panic!(
                    "Bevy renderer release lease generation changed for Context {:?}",
                    self.context_id
                )
            }
        }
    }

    pub(crate) fn release_renderer_resources(&self) {
        let packet = {
            let mut state = self.releases.state();
            let phase = state.phases.get_mut(&self.context_id).unwrap_or_else(|| {
                panic!(
                    "Bevy renderer release lease disappeared for Context {:?}",
                    self.context_id
                )
            });
            match phase {
                ImguiRendererReleasePhase::Detached { generation, packet }
                    if *generation == self.generation =>
                {
                    let packet = packet.take().expect(
                        "Bevy renderer release resources were consumed without advancing state",
                    );
                    *phase = ImguiRendererReleasePhase::ResourcesReleased {
                        generation: self.generation,
                    };
                    Some(packet)
                }
                ImguiRendererReleasePhase::ResourcesReleased { generation }
                    if *generation == self.generation =>
                {
                    None
                }
                ImguiRendererReleasePhase::RecoveryDetached {
                    generation,
                    device_generation,
                    packet,
                } if *generation == self.generation => {
                    let device_generation = *device_generation;
                    let packet = packet.take().expect(
                        "Bevy renderer recovery resources were consumed without advancing state",
                    );
                    *phase = ImguiRendererReleasePhase::RecoveryResourcesReleased {
                        generation: self.generation,
                        device_generation,
                    };
                    Some(packet)
                }
                ImguiRendererReleasePhase::RecoveryResourcesReleased { generation, .. }
                    if *generation == self.generation =>
                {
                    None
                }
                phase => {
                    panic!(
                        "Bevy renderer resources for Context {:?} released in invalid phase: {phase:?}",
                        self.context_id
                    )
                }
            }
        };
        drop(packet);
    }

    pub(crate) fn finish_device_recovery(&self) {
        let mut state = self.releases.state();
        let expected_device_generation = state.device_generation;
        let phase = state.phases.get_mut(&self.context_id).unwrap_or_else(|| {
            panic!(
                "Bevy renderer recovery lease disappeared for Context {:?}",
                self.context_id
            )
        });
        match phase {
            ImguiRendererReleasePhase::RecoveryResourcesReleased {
                generation,
                device_generation,
            } if *generation == self.generation
                && *device_generation == expected_device_generation =>
            {
                *phase = ImguiRendererReleasePhase::Live {
                    generation: self.generation,
                };
            }
            phase => {
                panic!(
                    "Bevy renderer recovery for Context {:?} completed in invalid phase: {phase:?}",
                    self.context_id
                )
            }
        }
    }

    pub(crate) fn retire(self) {
        let mut state = self.releases.state();
        match state.phases.get(&self.context_id) {
            Some(ImguiRendererReleasePhase::ResourcesReleased { generation })
                if *generation == self.generation =>
            {
                state.phases.remove(&self.context_id);
            }
            phase => {
                panic!(
                    "Bevy renderer release lease for Context {:?} retired before acknowledgement: {phase:?}",
                    self.context_id
                )
            }
        }
    }
}
