//! Render-world resources and renderer-owned state.

use super::prepare::{
    create_standard_imgui_sampler, create_texture_sampler_bind_group, write_texture_rows,
};
use super::*;

/// Camera/render-target association for an extracted ImGui overlay frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImguiCameraTarget {
    /// Main-world camera entity that should receive the ImGui overlay.
    pub camera: Entity,
    /// Camera order, preserved so the renderer can match Bevy's camera ordering.
    pub order: isize,
    /// Normalized render target resolved from the camera and current primary window.
    pub target: NormalizedRenderTarget,
    /// Dear ImGui viewport whose draw data should be rendered into this target.
    pub viewport_id: Option<imgui::Id>,
    /// Physical camera viewport to use when rendering this overlay target.
    pub camera_viewport: Option<ImguiCameraViewport>,
    /// Whether this target was selected from an explicitly marked [`ImguiOverlayCamera`].
    pub explicit: bool,
}

/// Marker component for cameras that explicitly receive Dear ImGui overlay rendering.
///
/// If at least one active camera for a render target has this marker, unmarked cameras on that
/// render target are ignored for ImGui overlay extraction. If no camera on a render target is
/// marked, the backend keeps its fallback behavior and uses the highest-order active camera for
/// that target.
#[derive(Component, Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ImguiOverlayCamera;

/// Marker component for cameras that should not receive Dear ImGui overlay rendering.
///
/// This is useful for editor shell scene cameras that render to a `Handle<Image>` later shown
/// inside an ImGui viewport. Without this marker, the global overlay pass would also draw ImGui into
/// that offscreen scene target.
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
    /// Main-world camera entity associated with this draw.
    pub camera: Entity,
    /// Camera order preserved from Bevy extraction.
    pub order: isize,
    /// Normalized render target associated with the camera.
    pub target: NormalizedRenderTarget,
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
    frame_index: Option<u64>,
    uniforms: Option<ImguiUniforms>,
    uniforms_by_camera: HashMap<Entity, ImguiUniforms>,
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

    /// Uniforms for a camera's routed viewport draw data.
    #[must_use]
    pub fn uniforms_for_camera(&self, camera: Entity) -> Option<ImguiUniforms> {
        self.uniforms_by_camera
            .get(&camera)
            .copied()
            .or(self.uniforms)
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

    pub(super) fn replace(&mut self, frame: PreparedFrameData) {
        self.frame_index = Some(frame.frame_index);
        self.uniforms = frame.uniforms;
        self.uniforms_by_camera = frame.uniforms_by_camera;
        self.vertices = frame.vertices;
        self.indices = frame.indices;
        self.draws = frame.draws;
        self.texture_request_count = frame.texture_request_count;
    }

    pub(super) fn clear(&mut self, frame_index: Option<u64>) {
        self.frame_index = frame_index;
        self.uniforms = None;
        self.uniforms_by_camera.clear();
        self.vertices.clear();
        self.indices.clear();
        self.draws.clear();
        self.texture_request_count = 0;
    }
}

pub(super) struct PreparedFrameData {
    pub(super) frame_index: u64,
    pub(super) uniforms: Option<ImguiUniforms>,
    pub(super) uniforms_by_camera: HashMap<Entity, ImguiUniforms>,
    pub(super) vertices: Vec<ImguiGpuVertex>,
    pub(super) indices: Vec<DrawIdx>,
    pub(super) draws: Vec<ImguiPreparedDraw>,
    pub(super) texture_request_count: usize,
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
    uniforms_by_camera: HashMap<Entity, ImguiCameraUniformResources>,
    _fallback_texture: Texture,
    _fallback_view: TextureView,
    fallback_bind_group: BindGroup,
}

struct ImguiCameraUniformResources {
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
            uniforms_by_camera: HashMap::new(),
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
        let active_cameras = prepared
            .draws()
            .iter()
            .map(|draw| draw.camera)
            .collect::<std::collections::HashSet<_>>();
        self.uniforms_by_camera
            .retain(|camera, _| active_cameras.contains(camera));

        for camera in active_cameras {
            let Some(uniforms) = prepared.uniforms_for_camera(camera) else {
                continue;
            };
            let resources = self.uniforms_by_camera.entry(camera).or_insert_with(|| {
                create_camera_uniform_resources(camera, render_device, pipeline_cache, pipeline)
            });
            render_queue.write_buffer(&resources.buffer, 0, bytemuck::bytes_of(&uniforms));
        }
    }

    pub(super) fn update_camera_uniforms(
        &self,
        camera: Entity,
        render_queue: &RenderQueue,
        uniforms: ImguiUniforms,
    ) -> Option<&BindGroup> {
        let resources = self.uniforms_by_camera.get(&camera)?;
        render_queue.write_buffer(&resources.buffer, 0, bytemuck::bytes_of(&uniforms));
        Some(&resources.bind_group)
    }

    #[must_use]
    pub fn uniform_bind_group_count(&self) -> usize {
        self.uniforms_by_camera.len()
    }

    pub(super) fn fallback_bind_group(&self) -> &BindGroup {
        &self.fallback_bind_group
    }
}

fn create_camera_uniform_resources(
    camera: Entity,
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
) -> ImguiCameraUniformResources {
    let common_layout = pipeline_cache.get_bind_group_layout(pipeline.common_layout());
    let _ = camera;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ImguiViewportTarget {
    pub(super) viewport_id: imgui::Id,
    pub(super) window: Entity,
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

    pub(super) fn prune_destroyed_managed_textures(&mut self, completion_watermark: u64) {
        self.destroyed_managed_textures
            .retain(|_, destroy_epoch| *destroy_epoch > completion_watermark);
    }

    pub(super) fn has_managed_resources(&self) -> bool {
        !self.managed_texture_ids.is_empty()
            || self
                .textures
                .keys()
                .any(|binding| matches!(binding, TextureBinding::Managed(_)))
    }

    pub(super) fn take_managed_renderer_state(&mut self) -> Vec<ImguiRenderTexture> {
        let mut released = Vec::new();
        for (id, texture_id) in self.managed_texture_ids.drain() {
            self.managed_texture_aliases.remove(&texture_id);
            let binding = TextureBinding::Managed(id);
            self.bevy_image_bindings.remove(&binding);
            if let Some(texture) = self.textures.remove(&binding) {
                released.push(texture);
            }
        }
        debug_assert!(self.managed_texture_aliases.is_empty());
        self.managed_texture_aliases.clear();
        let orphaned = self
            .textures
            .keys()
            .copied()
            .filter(|binding| matches!(binding, TextureBinding::Managed(_)))
            .collect::<Vec<_>>();
        for binding in orphaned {
            if let Some(texture) = self.textures.remove(&binding) {
                released.push(texture);
            }
        }
        self.destroyed_managed_textures.clear();
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

/// Pipeline ids queued for the current render frame, keyed by main-world camera entity.
#[derive(Resource, Default)]
pub struct ImguiQueuedPipelines {
    pub(super) by_camera: HashMap<Entity, CachedRenderPipelineId>,
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

/// Render-side owner of the last extracted primary ImGui frame.
#[derive(Resource, Debug, Default)]
pub struct ImguiExtractedRenderFrame {
    frame_index: Option<u64>,
    snapshot: Option<imgui::render::snapshot::FrameSnapshot>,
    camera_targets: Vec<ImguiCameraTarget>,
    texture_feedback: Vec<imgui::render::snapshot::TextureFeedback>,
    completion_watermark: u64,
}

impl ImguiExtractedRenderFrame {
    /// Frame index copied from [`crate::ImguiFrameOutput`].
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        self.frame_index
    }

    /// Snapshot moved from the main/UI world, if it has not been completed yet.
    #[must_use]
    pub fn snapshot(&self) -> Option<&imgui::render::snapshot::FrameSnapshot> {
        self.snapshot.as_ref()
    }

    /// Camera targets associated with the extracted snapshot.
    #[must_use]
    pub fn camera_targets(&self) -> &[ImguiCameraTarget] {
        &self.camera_targets
    }

    /// Highest snapshot epoch that the render world has committed or abandoned.
    #[must_use]
    pub fn completion_watermark(&self) -> u64 {
        self.completion_watermark
    }

    pub(super) fn replace(
        &mut self,
        frame_index: u64,
        snapshot: imgui::render::snapshot::FrameSnapshot,
        camera_targets: Vec<ImguiCameraTarget>,
    ) {
        self.abandon();
        self.frame_index = Some(frame_index);
        self.snapshot = Some(snapshot);
        self.camera_targets = camera_targets;
    }

    pub(super) fn clear(&mut self, frame_index: u64) {
        self.abandon();
        self.frame_index = (frame_index > 0).then_some(frame_index);
        self.camera_targets.clear();
    }

    pub(super) fn extend_texture_feedback(
        &mut self,
        feedback: impl IntoIterator<Item = imgui::render::snapshot::TextureFeedback>,
    ) {
        self.texture_feedback.extend(feedback);
    }

    pub(super) fn commit(&mut self) {
        let feedback = std::mem::take(&mut self.texture_feedback);
        if let Some(snapshot) = self.snapshot.take() {
            // The mailbox is single-slot and this resource owns at most one extracted snapshot;
            // an older snapshot is committed or abandoned before a newer one can be processed.
            // Therefore the highest locally completed sequence is the renderer's safe watermark.
            self.completion_watermark = self.completion_watermark.max(snapshot.epoch().sequence());
            let _ = snapshot.commit(feedback);
        }
    }

    pub(super) fn abandon(&mut self) {
        self.texture_feedback.clear();
        if let Some(snapshot) = self.snapshot.take() {
            self.completion_watermark = self.completion_watermark.max(snapshot.epoch().sequence());
            drop(snapshot);
        }
    }
}

impl Drop for ImguiExtractedRenderFrame {
    fn drop(&mut self) {
        self.abandon();
    }
}

#[derive(Resource, Default)]
pub(super) struct ImguiRenderExtractionInstalled;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
enum ImguiRendererReleasePhase {
    #[default]
    NotInstalled,
    Released {
        generation: u64,
    },
    Acknowledged {
        generation: u64,
    },
    Live {
        generation: u64,
    },
    Requested {
        generation: u64,
        resources_live: bool,
    },
}

#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct ImguiRendererRelease {
    phase: Arc<Mutex<ImguiRendererReleasePhase>>,
}

impl ImguiRendererRelease {
    fn phase(&self) -> MutexGuard<'_, ImguiRendererReleasePhase> {
        self.phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn install(&self) {
        let mut phase = self.phase();
        if matches!(*phase, ImguiRendererReleasePhase::NotInstalled) {
            *phase = ImguiRendererReleasePhase::Released { generation: 1 };
        }
    }

    pub(crate) fn request_release(&self) -> bool {
        let mut phase = self.phase();
        match *phase {
            ImguiRendererReleasePhase::NotInstalled => true,
            ImguiRendererReleasePhase::Released { generation } => {
                *phase = ImguiRendererReleasePhase::Requested {
                    generation,
                    resources_live: false,
                };
                true
            }
            ImguiRendererReleasePhase::Live { generation } => {
                *phase = ImguiRendererReleasePhase::Requested {
                    generation,
                    resources_live: true,
                };
                false
            }
            ImguiRendererReleasePhase::Requested { resources_live, .. } => !resources_live,
            ImguiRendererReleasePhase::Acknowledged { .. } => true,
        }
    }

    pub(crate) fn release_requested(&self) -> bool {
        matches!(
            *self.phase(),
            ImguiRendererReleasePhase::Requested { .. }
                | ImguiRendererReleasePhase::Acknowledged { .. }
        )
    }

    pub(super) fn update_resources_live(&self, resources_live: bool) {
        let mut phase = self.phase();
        match (*phase, resources_live) {
            (ImguiRendererReleasePhase::Released { generation }, true) => {
                let generation = generation
                    .checked_add(1)
                    .expect("Bevy renderer release generation space exhausted");
                *phase = ImguiRendererReleasePhase::Live { generation };
            }
            (ImguiRendererReleasePhase::Live { generation }, false) => {
                *phase = ImguiRendererReleasePhase::Released { generation };
            }
            _ => {}
        }
    }

    pub(super) fn requested_generation(&self) -> Option<u64> {
        match *self.phase() {
            ImguiRendererReleasePhase::Requested { generation, .. } => Some(generation),
            _ => None,
        }
    }

    pub(super) fn acknowledge_release(&self, generation: u64) -> bool {
        let mut phase = self.phase();
        match *phase {
            ImguiRendererReleasePhase::Requested {
                generation: expected,
                ..
            } if expected == generation => {
                *phase = ImguiRendererReleasePhase::Acknowledged { generation };
                true
            }
            _ => false,
        }
    }
}
