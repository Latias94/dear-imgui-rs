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
    ) -> RendererResult<WgpuTexture> {
        let rgba = convert_rows_to_rgba(format, width, height, row_pitch, pixels)?;
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
        write_rgba_region(queue, &texture, Origin3d::ZERO, width, height, &rgba)?;
        let texture_view = texture.create_view(&TextureViewDescriptor::default());
        Ok(WgpuTexture::new(texture, texture_view))
    }

    pub(super) fn upload_managed_texture_contents(
        &self,
        queue: &Queue,
        id: SnapshotTextureId,
        format: ImGuiTextureFormat,
        dimensions: [u32; 2],
        row_pitch: usize,
        pixels: &[u8],
    ) -> RendererResult<()> {
        let [width, height] = dimensions;
        let entry = self
            .managed_textures
            .get(&id)
            .ok_or(RendererError::ManagedTextureMissing(id))?;
        if entry.width != width || entry.height != height {
            return Err(RendererError::ManagedTextureLayoutMismatch {
                texture: id,
                expected: [entry.width, entry.height],
                actual: [width, height],
            });
        }

        let rgba = convert_rows_to_rgba(format, width, height, row_pitch, pixels)?;
        write_rgba_region(
            queue,
            entry.resource.texture(),
            Origin3d::ZERO,
            width,
            height,
            &rgba,
        )
    }

    pub(super) fn update_managed_texture(
        &self,
        queue: &Queue,
        id: SnapshotTextureId,
        format: ImGuiTextureFormat,
        width: u32,
        height: u32,
        rects: &[TextureUploadRect],
    ) -> RendererResult<()> {
        let entry = self
            .managed_textures
            .get(&id)
            .ok_or(RendererError::ManagedTextureMissing(id))?;
        if entry.width != width || entry.height != height {
            return Err(RendererError::ManagedTextureLayoutMismatch {
                texture: id,
                expected: [entry.width, entry.height],
                actual: [width, height],
            });
        }

        // Validate and convert every upload before mutating the GPU resource. A malformed later
        // rectangle must not leave an otherwise rejected request half-applied.
        let prepared = rects
            .iter()
            .map(|update| {
                let rect = validate_update_rect(width, height, update.rect)?;
                let rgba =
                    convert_rows_to_rgba(format, rect.w, rect.h, update.row_pitch, &update.data)?;
                Ok((rect, rgba))
            })
            .collect::<RendererResult<Vec<_>>>()?;

        for (rect, rgba) in prepared {
            write_rgba_region(
                queue,
                entry.resource.texture(),
                Origin3d {
                    x: rect.x,
                    y: rect.y,
                    z: 0,
                },
                rect.w,
                rect.h,
                &rgba,
            )?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone)]
struct ValidatedRect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

fn validate_update_rect(
    texture_width: u32,
    texture_height: u32,
    rect: TextureRect,
) -> RendererResult<ValidatedRect> {
    let x = u32::from(rect.x);
    let y = u32::from(rect.y);
    let w = u32::from(rect.w);
    let h = u32::from(rect.h);
    let x_end = x.checked_add(w);
    let y_end = y.checked_add(h);
    if w == 0
        || h == 0
        || x_end.is_none_or(|end| end > texture_width)
        || y_end.is_none_or(|end| end > texture_height)
    {
        return Err(RendererError::BadTexture(format!(
            "managed texture update rectangle ({x}, {y}, {w}, {h}) exceeds {texture_width}x{texture_height}"
        )));
    }
    Ok(ValidatedRect { x, y, w, h })
}

fn convert_rows_to_rgba(
    format: ImGuiTextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &[u8],
) -> RendererResult<Vec<u8>> {
    if width == 0 || height == 0 {
        return Err(RendererError::BadTexture(
            "managed texture dimensions must be positive".to_owned(),
        ));
    }
    let source_bpp = match format {
        ImGuiTextureFormat::RGBA32 => 4usize,
        ImGuiTextureFormat::Alpha8 => 1usize,
    };
    let width = usize::try_from(width)
        .map_err(|_| RendererError::BadTexture("texture width exceeds usize".to_owned()))?;
    let height = usize::try_from(height)
        .map_err(|_| RendererError::BadTexture("texture height exceeds usize".to_owned()))?;
    let tight_source_pitch = width
        .checked_mul(source_bpp)
        .ok_or_else(|| RendererError::BadTexture("texture row size overflow".to_owned()))?;
    if row_pitch < tight_source_pitch {
        return Err(RendererError::BadTexture(format!(
            "managed texture row pitch {row_pitch} is smaller than {tight_source_pitch}"
        )));
    }
    let required = row_pitch
        .checked_mul(height.saturating_sub(1))
        .and_then(|prefix| prefix.checked_add(tight_source_pitch))
        .ok_or_else(|| RendererError::BadTexture("texture byte size overflow".to_owned()))?;
    if pixels.len() < required {
        return Err(RendererError::BadTexture(format!(
            "managed texture data is truncated: expected at least {required} bytes, got {}",
            pixels.len()
        )));
    }

    let rgba_pitch = width
        .checked_mul(4)
        .ok_or_else(|| RendererError::BadTexture("RGBA row size overflow".to_owned()))?;
    let mut rgba = vec![
        0;
        rgba_pitch.checked_mul(height).ok_or_else(|| {
            RendererError::BadTexture("converted texture byte size overflow".to_owned())
        })?
    ];
    for row in 0..height {
        let source = &pixels[row * row_pitch..row * row_pitch + tight_source_pitch];
        let destination = &mut rgba[row * rgba_pitch..(row + 1) * rgba_pitch];
        match format {
            ImGuiTextureFormat::RGBA32 => destination.copy_from_slice(source),
            ImGuiTextureFormat::Alpha8 => {
                for (rgba, alpha) in destination.chunks_exact_mut(4).zip(source.iter().copied()) {
                    rgba.copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
        }
    }
    Ok(rgba)
}

fn write_rgba_region(
    queue: &Queue,
    texture: &Texture,
    origin: Origin3d,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> RendererResult<()> {
    let tight_pitch = width
        .checked_mul(4)
        .ok_or_else(|| RendererError::BadTexture("RGBA upload row size overflow".to_owned()))?;
    let expected = usize::try_from(tight_pitch)
        .ok()
        .and_then(|pitch| {
            usize::try_from(height)
                .ok()
                .and_then(|h| pitch.checked_mul(h))
        })
        .ok_or_else(|| RendererError::BadTexture("RGBA upload byte size overflow".to_owned()))?;
    if rgba.len() != expected {
        return Err(RendererError::BadTexture(format!(
            "RGBA upload size mismatch: expected {expected} bytes, got {}",
            rgba.len()
        )));
    }

    let alignment = COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_pitch = tight_pitch.div_ceil(alignment) * alignment;
    let padded;
    let upload = if padded_pitch == tight_pitch {
        rgba
    } else {
        let padded_len = usize::try_from(padded_pitch)
            .ok()
            .and_then(|pitch| {
                usize::try_from(height)
                    .ok()
                    .and_then(|h| pitch.checked_mul(h))
            })
            .ok_or_else(|| RendererError::BadTexture("padded upload size overflow".to_owned()))?;
        let mut rows = vec![0; padded_len];
        let tight_pitch = tight_pitch as usize;
        let padded_pitch = padded_pitch as usize;
        for row in 0..height as usize {
            rows[row * padded_pitch..row * padded_pitch + tight_pitch]
                .copy_from_slice(&rgba[row * tight_pitch..(row + 1) * tight_pitch]);
        }
        padded = rows;
        &padded
    };

    queue.write_texture(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin,
            aspect: TextureAspect::All,
        },
        upload,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(padded_pitch),
            rows_per_image: Some(height),
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Ok(())
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    #[test]
    fn alpha_rows_with_padding_expand_to_rgba() {
        let rgba = convert_rows_to_rgba(ImGuiTextureFormat::Alpha8, 2, 2, 3, &[10, 20, 0, 30, 40])
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

    #[test]
    fn out_of_bounds_update_rect_is_rejected() {
        assert!(
            validate_update_rect(
                4,
                4,
                TextureRect {
                    x: 3,
                    y: 0,
                    w: 2,
                    h: 1,
                },
            )
            .is_err()
        );
    }
}
