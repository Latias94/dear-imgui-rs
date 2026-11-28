use std::num::NonZeroU32;

use dear_imgui_backend_support::DirtyRectNormalizer;
use dear_imgui_rs::texture::get_format_bytes_per_pixel;

use super::*;

impl WgpuTextureManager {
    pub(super) fn create_managed_texture_resource(
        device: &Device,
        queue: &Queue,
        format: ImGuiTextureFormat,
        width: u32,
        height: u32,
        row_pitch: usize,
        pixels: &[u8],
    ) -> RendererResult<(OwnedWgpuTexture, Vec<u8>)> {
        let (rgba, pitch) = convert_rows_to_rgba(format, width, height, row_pitch, pixels)?;
        let resource = create_managed_rgba_resource(device, width, height);
        write_rgba_region(
            queue,
            resource.texture(),
            Origin3d::ZERO,
            [width, height],
            pitch,
            &rgba,
        );
        Ok((resource, rgba))
    }

    pub(super) fn update_managed_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        id: SnapshotTextureId,
        format: ImGuiTextureFormat,
        dimensions: [u32; 2],
        rects: &[TextureUploadRect],
    ) -> RendererResult<(TextureId, bool)> {
        let [width, height] = dimensions;
        let entry = self
            .managed_textures
            .get_mut(id)
            .ok_or(RendererError::ManagedTextureMissing(id))?;
        if entry.width != width || entry.height != height {
            return Err(RendererError::ManagedTextureLayoutMismatch {
                texture: id,
                expected: [entry.width, entry.height],
                actual: [width, height],
            });
        }

        // Validate the whole request before changing the CPU mirror or queuing any writes.
        let normalizer = DirtyRectNormalizer::new(dimensions);
        for update in rects {
            normalizer
                .validate(update.rect)
                .map_err(|error| RendererError::BadTexture(error.to_string()))?;
            validate_source_rows(
                format,
                usize::from(update.rect.w),
                usize::from(update.rect.h),
                update.row_pitch,
                &update.data,
            )?;
        }
        if rects.is_empty() {
            return Ok((entry.texture_id, false));
        }

        let pitch = rgba_row_pitch(width)?;
        let texture_pitch = pitch.get() as usize;
        // Draws holding the old version can still be submitted in any order. They retain
        // that version through WGPU's command tracking, even after the cache stops using it.
        let replacement = (!entry.resource.can_write_in_place())
            .then(|| create_managed_rgba_resource(device, width, height));
        for update in rects {
            let TextureRect { x, y, w, h } = update.rect;
            let start = usize::from(y) * texture_pitch + usize::from(x) * 4;
            let end = start + (usize::from(h) - 1) * texture_pitch + usize::from(w) * 4;
            let rgba = &mut entry.rgba_pixels[start..end];
            copy_rgba_rows(
                format,
                usize::from(w),
                usize::from(h),
                (update.row_pitch, &update.data),
                (texture_pitch, rgba),
            );
            if replacement.is_none() {
                write_rgba_region(
                    queue,
                    entry.resource.texture(),
                    Origin3d {
                        x: u32::from(x),
                        y: u32::from(y),
                        z: 0,
                    },
                    [u32::from(w), u32::from(h)],
                    pitch,
                    rgba,
                );
            }
        }

        if let Some(resource) = replacement {
            write_rgba_region(
                queue,
                resource.texture(),
                Origin3d::ZERO,
                [width, height],
                pitch,
                &entry.rgba_pixels,
            );
            entry.resource = resource;
            return Ok((entry.texture_id, true));
        }

        Ok((entry.texture_id, false))
    }
}

fn create_managed_rgba_resource(device: &Device, width: u32, height: u32) -> OwnedWgpuTexture {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Dear ImGui managed texture"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&TextureViewDescriptor::default());
    OwnedWgpuTexture::new(texture, texture_view)
}

fn convert_rows_to_rgba(
    format: ImGuiTextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &[u8],
) -> RendererResult<(Vec<u8>, NonZeroU32)> {
    let pitch = rgba_row_pitch(width)?;
    let width = usize::try_from(width)
        .map_err(|_| RendererError::BadTexture("texture width exceeds usize".to_owned()))?;
    let height = usize::try_from(height)
        .map_err(|_| RendererError::BadTexture("texture height exceeds usize".to_owned()))?;
    validate_source_rows(format, width, height, row_pitch, pixels)?;
    let rgba_pitch = pitch.get() as usize;
    let rgba_len = rgba_pitch.checked_mul(height).ok_or_else(|| {
        RendererError::BadTexture("converted texture byte size overflow".to_owned())
    })?;
    let mut rgba = vec![0; rgba_len];
    copy_rgba_rows(
        format,
        width,
        height,
        (row_pitch, pixels),
        (rgba_pitch, &mut rgba),
    );
    Ok((rgba, pitch))
}

fn validate_source_rows(
    format: ImGuiTextureFormat,
    width: usize,
    height: usize,
    row_pitch: usize,
    pixels: &[u8],
) -> RendererResult<()> {
    if width == 0 || height == 0 {
        return Err(RendererError::BadTexture(
            "managed texture dimensions must be positive".to_owned(),
        ));
    }
    let tight_source_pitch = width
        .checked_mul(get_format_bytes_per_pixel(format))
        .ok_or_else(|| RendererError::BadTexture("texture row size overflow".to_owned()))?;
    if row_pitch < tight_source_pitch {
        return Err(RendererError::BadTexture(format!(
            "managed texture row pitch {row_pitch} is smaller than {tight_source_pitch}"
        )));
    }
    let required = row_pitch
        .checked_mul(height - 1)
        .and_then(|prefix| prefix.checked_add(tight_source_pitch))
        .ok_or_else(|| RendererError::BadTexture("texture byte size overflow".to_owned()))?;
    if pixels.len() < required {
        return Err(RendererError::BadTexture(format!(
            "managed texture data is truncated: expected at least {required} bytes, got {}",
            pixels.len()
        )));
    }
    Ok(())
}

fn copy_rgba_rows(
    format: ImGuiTextureFormat,
    width: usize,
    height: usize,
    (source_pitch, pixels): (usize, &[u8]),
    (rgba_pitch, rgba): (usize, &mut [u8]),
) {
    let source_width = width * get_format_bytes_per_pixel(format);
    let rgba_width = width * 4;
    for row in 0..height {
        let source_start = row * source_pitch;
        let rgba_start = row * rgba_pitch;
        let source = &pixels[source_start..source_start + source_width];
        let destination = &mut rgba[rgba_start..rgba_start + rgba_width];
        match format {
            ImGuiTextureFormat::RGBA32 => destination.copy_from_slice(source),
            ImGuiTextureFormat::Alpha8 => {
                for (rgba, alpha) in destination
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(source.iter().copied())
                {
                    rgba.copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
        }
    }
}

fn rgba_row_pitch(width: u32) -> RendererResult<NonZeroU32> {
    width
        .checked_mul(4)
        .and_then(NonZeroU32::new)
        .ok_or_else(|| RendererError::BadTexture("invalid RGBA upload row size".to_owned()))
}

fn write_rgba_region(
    queue: &Queue,
    texture: &Texture,
    origin: Origin3d,
    [width, height]: [u32; 2],
    row_pitch: NonZeroU32,
    rgba: &[u8],
) {
    // Queue writes accept the mirror's row pitch without encoder-copy alignment padding.
    queue.write_texture(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin,
            aspect: TextureAspect::All,
        },
        rgba,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(row_pitch.get()),
            rows_per_image: None,
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    #[test]
    fn alpha_rows_with_padding_expand_to_rgba() {
        let (rgba, _) =
            convert_rows_to_rgba(ImGuiTextureFormat::Alpha8, 2, 2, 3, &[10, 20, 0, 30, 40])
                .expect("valid padded Alpha8 data");
        assert_eq!(
            rgba,
            [
                255, 255, 255, 10, 255, 255, 255, 20, 255, 255, 255, 30, 255, 255, 255, 40,
            ]
        );
    }

    #[test]
    fn truncated_rows_are_rejected() {
        assert!(convert_rows_to_rgba(ImGuiTextureFormat::RGBA32, 2, 2, 8, &[0; 15]).is_err());
    }
}
