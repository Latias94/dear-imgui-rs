//! ImGui managed uploads and Bevy image bindings in the render world.

use std::borrow::Cow;

use bevy_render::render_resource::{SamplerId, TextureViewId};
use dear_imgui_backend_support::{BackendError, DirtyRectNormalizer, ManagedTextureTable};

use super::*;
use crate::route::{ImguiDiagnostic, ImguiDiagnosticKind, ImguiDiagnosticOrigin, ImguiDiagnostics};
use imgui::render::snapshot::{TextureFeedback, TextureOp, TextureRequest, TextureUploadRect};
use imgui::texture::TextureFormat as ImguiTextureFormat;

const MANAGED_TEXTURE_NAMESPACE: u64 = 0x4000_0000_0000_0000;

pub(super) struct ImguiRenderTexture {
    texture: Texture,
    _view: TextureView,
    extent: [u32; 2],
    linear_bind_group: BindGroup,
    nearest_bind_group: BindGroup,
}

struct ManagedRenderTexture {
    alias: imgui::TextureId,
    gpu: ImguiRenderTexture,
}

struct ImguiTextureUpload<'a> {
    format: ImguiTextureFormat,
    extent: [u32; 2],
    row_pitch: usize,
    pixels: &'a [u8],
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BevyImageBindingSource {
    texture_view: TextureViewId,
    sampler: SamplerId,
}

impl From<&GpuImage> for BevyImageBindingSource {
    fn from(image: &GpuImage) -> Self {
        Self {
            texture_view: image.texture_view.id(),
            sampler: image.sampler.id(),
        }
    }
}

struct BevyImageBinding {
    source: BevyImageBindingSource,
    bind_group: BindGroup,
}

/// Texture bind groups currently known to the Bevy-native ImGui renderer.
#[derive(Resource, Default)]
pub struct ImguiTextureBindGroups {
    managed: ManagedTextureTable<ManagedRenderTexture>,
    managed_texture_aliases: HashMap<imgui::TextureId, SnapshotTextureId>,
    next_managed_texture_id: u64,
    bevy_images: HashMap<imgui::TextureId, BevyImageBinding>,
}

impl ImguiTextureBindGroups {
    /// Number of registered texture bind groups.
    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.managed_texture_aliases.len() + self.bevy_images.len()
    }

    /// Whether no texture bind groups are currently registered.
    #[cfg(test)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn get(
        &self,
        texture: &TextureBinding,
        sampler: ImguiSampler,
    ) -> Option<&BindGroup> {
        let id = match *texture {
            TextureBinding::Managed(id) => id,
            TextureBinding::Legacy(alias) => match self.managed_texture_aliases.get(&alias) {
                Some(&id) => id,
                None => return self.bevy_images.get(&alias).map(|image| &image.bind_group),
            },
        };
        let texture = &self.managed.get(id)?.gpu;
        Some(match sampler {
            ImguiSampler::Linear => &texture.linear_bind_group,
            ImguiSampler::Nearest => &texture.nearest_bind_group,
        })
    }

    fn insert_managed_texture(
        &mut self,
        id: SnapshotTextureId,
        gpu: ImguiRenderTexture,
    ) -> Result<imgui::TextureId, BackendError> {
        let alias = match self.managed.get(id) {
            Some(texture) => texture.alias,
            None => self.allocate_managed_texture_id(),
        };
        let replaced = self
            .managed
            .insert(id, ManagedRenderTexture { alias, gpu })
            .map_err(|_| {
                texture_upload_error(id, "managed texture is sealed by a destroy request")
            })?;
        self.managed_texture_aliases.insert(alias, id);
        drop(replaced);
        Ok(alias)
    }

    fn allocate_managed_texture_id(&mut self) -> imgui::TextureId {
        loop {
            self.next_managed_texture_id += 1;
            assert!(
                self.next_managed_texture_id < MANAGED_TEXTURE_NAMESPACE,
                "Bevy managed texture ID namespace exhausted"
            );
            let alias =
                imgui::TextureId::new(MANAGED_TEXTURE_NAMESPACE | self.next_managed_texture_id);
            if !self.bevy_images.contains_key(&alias) {
                return alias;
            }
        }
    }

    pub(super) fn destroy_managed_texture(&mut self, id: SnapshotTextureId, destroy_epoch: u64) {
        if let Some(texture) = self.managed.remove(id) {
            self.managed_texture_aliases.remove(&texture.alias);
        }
        self.managed.acknowledge_destroy(id, destroy_epoch);
    }

    pub(super) fn accepts_managed_texture_upload(&self, id: SnapshotTextureId) -> bool {
        !self.managed.is_destroyed(id)
    }

    pub(super) fn prune_destroyed_managed_textures(
        &mut self,
        completion_watermark: impl FnMut(imgui::ContextId) -> Option<u64>,
    ) {
        self.managed.prune_tombstones(completion_watermark);
    }

    pub(super) fn take_managed_renderer_state(
        &mut self,
        context_id: imgui::ContextId,
    ) -> Vec<ImguiRenderTexture> {
        let aliases = &mut self.managed_texture_aliases;
        let released = self
            .managed
            .extract_resources(|owner| owner == context_id)
            .map(|(_, texture)| {
                aliases.remove(&texture.alias);
                texture.gpu
            })
            .collect();
        self.managed.clear_tombstones(|owner| owner == context_id);
        released
    }
}

#[derive(SystemParam)]
pub(super) struct ImguiTextureBindGroupParams<'w> {
    extracted: ResMut<'w, ImguiExtractedRenderFrame>,
    extracted_bevy_textures: Res<'w, ImguiExtractedBevyTextures>,
    diagnostics: Res<'w, ImguiDiagnostics>,
    gpu_images: Option<Res<'w, RenderAssets<GpuImage>>>,
    render_device: Option<Res<'w, RenderDevice>>,
    render_queue: Option<Res<'w, RenderQueue>>,
    pipeline_cache: Option<Res<'w, PipelineCache>>,
    pipeline: Res<'w, ImguiRenderPipeline>,
    renderer_releases: Res<'w, ImguiRendererReleases>,
}

pub(super) fn prepare_imgui_texture_bind_groups(
    mut params: ImguiTextureBindGroupParams,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    let gpu = match (
        params.render_device.as_deref(),
        params.render_queue.as_deref(),
        params.pipeline_cache.as_deref(),
    ) {
        (Some(device), Some(queue), Some(cache)) => Some((device, queue, cache)),
        _ => None,
    };

    for context_id in params.extracted.sorted_context_ids() {
        if params.renderer_releases.release_requested(context_id) {
            params.extracted.remove_context(context_id);
            continue;
        }
        let Some(snapshot) = params.extracted.snapshot(context_id) else {
            continue;
        };
        let feedback = snapshot
            .texture_requests()
            .iter()
            .map(|request| {
                let Some((device, queue, cache)) = gpu else {
                    return request.retry();
                };
                prepare_managed_texture_request(
                    request,
                    snapshot.epoch().sequence(),
                    &mut texture_bind_groups,
                    device,
                    queue,
                    cache,
                    &params.pipeline,
                )
                .unwrap_or_else(|_| request.retry())
            })
            .collect::<Vec<_>>();
        params
            .extracted
            .extend_texture_feedback(context_id, feedback);
    }

    let unavailable_images = if let Some((device, _, cache)) = gpu {
        prepare_bevy_image_texture_bind_groups(
            params.gpu_images.as_deref(),
            &params.extracted_bevy_textures,
            device,
            cache,
            &params.pipeline,
            &mut texture_bind_groups,
        )
    } else {
        retain_extracted_bevy_image_bindings(
            &params.extracted_bevy_textures,
            &mut texture_bind_groups,
        );
        params
            .extracted_bevy_textures
            .textures()
            .iter()
            .map(|(_, asset_id)| *asset_id)
            .collect()
    };
    publish_unavailable_bevy_image_texture_diagnostics(
        &params.diagnostics,
        &params.extracted_bevy_textures,
        unavailable_images,
    );
}

pub(super) fn prepare_managed_texture_request(
    request: &TextureRequest,
    epoch: u64,
    bindings: &mut ImguiTextureBindGroups,
    device: &RenderDevice,
    queue: &RenderQueue,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
) -> Result<TextureFeedback, BackendError> {
    let id = request.texture();
    let alias = match request.operation() {
        TextureOp::Destroy => {
            let feedback = request
                .destroyed()
                .map_err(|error| texture_upload_error(id, error.to_string()))?;
            bindings.destroy_managed_texture(id, epoch);
            return Ok(feedback);
        }
        _ if !bindings.accepts_managed_texture_upload(id) => return Ok(request.superseded()),
        TextureOp::Create {
            format,
            width,
            height,
            row_pitch,
            pixels,
        } => create_imgui_render_texture(
            id,
            device,
            queue,
            pipeline_cache,
            pipeline,
            ImguiTextureUpload {
                format: *format,
                extent: [*width, *height],
                row_pitch: *row_pitch,
                pixels,
            },
        )
        .and_then(|texture| bindings.insert_managed_texture(id, texture)),
        TextureOp::Update {
            format,
            width,
            height,
            rects,
        } => update_managed_texture(bindings, queue, id, *format, [*width, *height], rects),
    }?;
    request
        .uploaded(alias)
        .map_err(|error| texture_upload_error(id, error.to_string()))
}

fn update_managed_texture(
    bindings: &ImguiTextureBindGroups,
    queue: &RenderQueue,
    id: SnapshotTextureId,
    format: ImguiTextureFormat,
    extent: [u32; 2],
    rects: &[TextureUploadRect],
) -> Result<imgui::TextureId, BackendError> {
    let texture = bindings
        .managed
        .get(id)
        .ok_or_else(|| texture_upload_error(id, "managed texture is missing"))?;
    if texture.gpu.extent != extent {
        return Err(texture_upload_error(
            id,
            "update extent differs from the managed texture",
        ));
    }
    // Validate every payload before queuing any writes for this request.
    for update in convert_imgui_texture_update_rects(id, format, extent, rects)? {
        write_texture_rows(
            queue,
            &texture.gpu.texture,
            update.origin,
            update.extent,
            update.row_pitch,
            &update.pixels,
        );
    }
    Ok(texture.alias)
}

fn texture_upload_error(id: SnapshotTextureId, reason: impl Into<String>) -> BackendError {
    BackendError::TextureUpload {
        binding: TextureBinding::Managed(id),
        reason: reason.into(),
    }
}

pub(super) fn acknowledge_retired_bevy_image_textures(
    extracted_bevy_textures: Res<ImguiExtractedBevyTextures>,
    extracted_frames: Res<ImguiExtractedRenderFrame>,
    prepared: Res<ImguiPreparedRenderFrame>,
    texture_bind_groups: Res<ImguiTextureBindGroups>,
) {
    if extracted_frames.has_pending_snapshots() {
        return;
    }

    let acknowledgements = extracted_bevy_textures
        .retirement_candidates()
        .iter()
        .copied()
        .filter(|identity| {
            let binding = TextureBinding::Legacy(identity.texture_id());
            !texture_bind_groups
                .bevy_images
                .contains_key(&identity.texture_id())
                && !prepared.draws().iter().any(|draw| draw.texture == binding)
        })
        .collect::<Vec<_>>();
    extracted_bevy_textures.acknowledge_retirements(acknowledgements);
}

pub(super) fn managed_texture_extent_supported(
    width: u32,
    height: u32,
    max_dimension_2d: u32,
) -> bool {
    width > 0 && height > 0 && width <= max_dimension_2d && height <= max_dimension_2d
}

fn create_imgui_render_texture(
    id: SnapshotTextureId,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    upload: ImguiTextureUpload<'_>,
) -> Result<ImguiRenderTexture, BackendError> {
    let [width, height] = upload.extent;
    if !managed_texture_extent_supported(
        width,
        height,
        render_device.limits().max_texture_dimension_2d,
    ) {
        return Err(texture_upload_error(
            id,
            "texture extent exceeds the device limits or is empty",
        ));
    }
    let (pixels, row_pitch) = convert_imgui_texture_pixels(
        upload.format,
        upload.extent,
        upload.row_pitch,
        upload.pixels,
    )
    .ok_or_else(|| texture_upload_error(id, "invalid pixel layout"))?;
    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some("dear_imgui_bevy_texture"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_texture_rows(
        render_queue,
        &texture,
        Origin3d::ZERO,
        upload.extent,
        row_pitch,
        &pixels,
    );
    let view = texture.create_view(&TextureViewDescriptor::default());
    let layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
    let linear_sampler = create_standard_imgui_sampler(render_device, ImguiSampler::Linear);
    let nearest_sampler = create_standard_imgui_sampler(render_device, ImguiSampler::Nearest);
    let linear_bind_group = create_texture_sampler_bind_group(
        render_device,
        &layout,
        Some("dear_imgui_bevy_texture_bind_group"),
        &view,
        &linear_sampler,
    );
    let nearest_bind_group = create_texture_sampler_bind_group(
        render_device,
        &layout,
        Some("dear_imgui_bevy_texture_bind_group_nearest"),
        &view,
        &nearest_sampler,
    );
    Ok(ImguiRenderTexture {
        texture,
        _view: view,
        extent: upload.extent,
        linear_bind_group,
        nearest_bind_group,
    })
}

pub(super) fn create_standard_imgui_sampler(
    render_device: &RenderDevice,
    sampler: ImguiSampler,
) -> Sampler {
    match sampler {
        ImguiSampler::Linear => render_device.create_sampler(&SamplerDescriptor {
            label: Some("dear_imgui_bevy_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        }),
        ImguiSampler::Nearest => render_device.create_sampler(&SamplerDescriptor {
            label: Some("dear_imgui_bevy_nearest_sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        }),
    }
}

pub(super) fn create_texture_sampler_bind_group(
    render_device: &RenderDevice,
    layout: &BindGroupLayout,
    label: Option<&'static str>,
    view: &TextureView,
    sampler: &Sampler,
) -> BindGroup {
    render_device.create_bind_group(
        label,
        layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    )
}

pub(super) struct ConvertedTextureUpdateRect<'a> {
    pub(super) origin: Origin3d,
    pub(super) extent: [u32; 2],
    pub(super) row_pitch: u32,
    pub(super) pixels: Cow<'a, [u8]>,
}

pub(super) fn convert_imgui_texture_update_rects<'a>(
    id: SnapshotTextureId,
    format: ImguiTextureFormat,
    extent: [u32; 2],
    rects: &'a [TextureUploadRect],
) -> Result<Vec<ConvertedTextureUpdateRect<'a>>, BackendError> {
    if rects.is_empty() {
        return Err(texture_upload_error(
            id,
            "texture update has no upload rectangles",
        ));
    }
    let normalizer = DirtyRectNormalizer::new(extent);
    rects
        .iter()
        .map(|update| {
            let rect = update.rect;
            normalizer.validate(rect)?;
            let extent = [u32::from(rect.w), u32::from(rect.h)];
            let (pixels, row_pitch) =
                convert_imgui_texture_pixels(format, extent, update.row_pitch, &update.data)
                    .ok_or_else(|| {
                        texture_upload_error(id, "invalid pixel layout for texture update")
                    })?;
            Ok(ConvertedTextureUpdateRect {
                origin: Origin3d {
                    x: u32::from(rect.x),
                    y: u32::from(rect.y),
                    z: 0,
                },
                extent,
                row_pitch,
                pixels,
            })
        })
        .collect()
}

pub(super) fn prepare_bevy_image_texture_bind_groups(
    gpu_images: Option<&RenderAssets<GpuImage>>,
    extracted_bevy_textures: &ImguiExtractedBevyTextures,
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    texture_bind_groups: &mut ImguiTextureBindGroups,
) -> Vec<bevy_asset::AssetId<Image>> {
    retain_extracted_bevy_image_bindings(extracted_bevy_textures, texture_bind_groups);

    let Some(gpu_images) = gpu_images else {
        return extracted_bevy_textures
            .textures()
            .iter()
            .map(|(_, asset_id)| *asset_id)
            .collect();
    };

    let mut unavailable_images = Vec::new();
    for &(texture_id, asset_id) in extracted_bevy_textures.textures() {
        let Some(gpu_image) = gpu_images.get(asset_id) else {
            texture_bind_groups.bevy_images.remove(&texture_id);
            unavailable_images.push(asset_id);
            continue;
        };
        let source = BevyImageBindingSource::from(gpu_image);
        if texture_bind_groups
            .bevy_images
            .get(&texture_id)
            .is_some_and(|image| image.source == source)
        {
            continue;
        }
        if let Some(bind_group) =
            create_bevy_image_texture_bind_group(render_device, pipeline_cache, pipeline, gpu_image)
        {
            texture_bind_groups
                .bevy_images
                .insert(texture_id, BevyImageBinding { source, bind_group });
        } else {
            texture_bind_groups.bevy_images.remove(&texture_id);
            unavailable_images.push(asset_id);
        }
    }
    unavailable_images
}

fn publish_unavailable_bevy_image_texture_diagnostics(
    diagnostics: &ImguiDiagnostics,
    extracted_bevy_textures: &ImguiExtractedBevyTextures,
    unavailable_images: impl IntoIterator<Item = bevy_asset::AssetId<Image>>,
) {
    diagnostics.replace(
        ImguiDiagnosticOrigin::Texture,
        extracted_bevy_textures.extraction_epoch(),
        unavailable_images.into_iter().map(|image| {
            ImguiDiagnostic::new(ImguiDiagnosticKind::UnavailableBevyImageTexture { image })
        }),
    );
}

fn retain_extracted_bevy_image_bindings(
    extracted_bevy_textures: &ImguiExtractedBevyTextures,
    texture_bind_groups: &mut ImguiTextureBindGroups,
) {
    let active = extracted_bevy_textures
        .textures()
        .iter()
        .map(|(texture_id, _)| *texture_id)
        .collect::<HashSet<_>>();
    texture_bind_groups
        .bevy_images
        .retain(|id, _| active.contains(id));
}

fn create_bevy_image_texture_bind_group(
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    gpu_image: &GpuImage,
) -> Option<BindGroup> {
    if !ImguiTextureViewCompatibility::from_gpu_image(gpu_image)
        .supports_imgui_sampling(render_device.features())
    {
        return None;
    }

    let layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
    Some(create_texture_sampler_bind_group(
        render_device,
        &layout,
        Some("dear_imgui_bevy_image_texture_bind_group"),
        &gpu_image.texture_view,
        &gpu_image.sampler,
    ))
}

pub(super) fn convert_imgui_texture_pixels(
    format: ImguiTextureFormat,
    [width, height]: [u32; 2],
    row_pitch: usize,
    pixels: &[u8],
) -> Option<(Cow<'_, [u8]>, u32)> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let row_bytes = width.checked_mul(imgui::get_format_bytes_per_pixel(format))?;
    if row_bytes == 0 || height == 0 || row_pitch < row_bytes {
        return None;
    }
    // A pitched upload only needs the data through the last row's final pixel.
    let required = row_pitch.checked_mul(height - 1)?.checked_add(row_bytes)?;
    let pixels = pixels.get(..required)?;
    if format == ImguiTextureFormat::RGBA32 {
        return Some((Cow::Borrowed(pixels), u32::try_from(row_pitch).ok()?));
    }

    let row_bytes_rgba = width.checked_mul(4)?;
    let row_pitch_rgba = u32::try_from(row_bytes_rgba).ok()?;
    let mut rgba = vec![0; row_bytes_rgba.checked_mul(height)?];
    for (src, dst) in pixels
        .chunks(row_pitch)
        .zip(rgba.chunks_exact_mut(row_bytes_rgba))
    {
        for (alpha, color) in src[..row_bytes].iter().zip(dst.as_chunks_mut::<4>().0) {
            *color = [255, 255, 255, *alpha];
        }
    }
    Some((Cow::Owned(rgba), row_pitch_rgba))
}

pub(super) fn write_texture_rows(
    render_queue: &RenderQueue,
    texture: &Texture,
    origin: Origin3d,
    [width, height]: [u32; 2],
    row_pitch: u32,
    pixels: &[u8],
) {
    // Queue writes accept the supplied row pitch without encoder-copy alignment padding.
    render_queue.write_texture(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin,
            aspect: TextureAspect::All,
        },
        pixels,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(row_pitch),
            rows_per_image: None,
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}
