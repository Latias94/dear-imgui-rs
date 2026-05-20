use super::*;

impl WgpuTextureManager {
    /// Convert a sub-rectangle of ImGui texture pixels into a tightly packed RGBA8 buffer
    pub(super) fn convert_subrect_to_rgba(
        texture_data: &TextureData,
        rect: dear_imgui_rs::texture::TextureRect,
    ) -> Option<Vec<u8>> {
        let pixels = texture_data.pixels()?;
        let tex_w = usize::try_from(texture_data.width()).ok()?;
        let tex_h = usize::try_from(texture_data.height()).ok()?;
        if tex_w == 0 || tex_h == 0 {
            return None;
        }

        let bpp = texture_data.bytes_per_pixel();
        let (rx, ry, rw, rh) = (
            rect.x as usize,
            rect.y as usize,
            rect.w as usize,
            rect.h as usize,
        );
        if rw == 0 || rh == 0 || rx >= tex_w || ry >= tex_h {
            return None;
        }

        // Clamp to texture bounds defensively
        let rw = rw.min(tex_w.saturating_sub(rx));
        let rh = rh.min(tex_h.saturating_sub(ry));

        let mut out = vec![0u8; rw.checked_mul(rh)?.checked_mul(4)?];
        match texture_data.format() {
            ImGuiTextureFormat::RGBA32 => {
                for row in 0..rh {
                    let src_off = ((ry + row) * tex_w + rx) * bpp;
                    let dst_off = row * rw * 4;
                    // Copy only the row slice and convert layout if needed (it is already RGBA)
                    out[dst_off..dst_off + rw * 4]
                        .copy_from_slice(&pixels[src_off..src_off + rw * 4]);
                }
            }
            ImGuiTextureFormat::Alpha8 => {
                for row in 0..rh {
                    let src_off = ((ry + row) * tex_w + rx) * bpp; // bpp = 1
                    let dst_off = row * rw * 4;
                    for i in 0..rw {
                        let a = pixels[src_off + i];
                        let dst = &mut out[dst_off + i * 4..dst_off + i * 4 + 4];
                        dst.copy_from_slice(&[255, 255, 255, a]);
                    }
                }
            }
        }
        Some(out)
    }

    /// Apply queued sub-rectangle updates to an existing WGPU texture.
    /// Returns true if any update was applied.
    pub(super) fn apply_subrect_updates(
        &mut self,
        queue: &Queue,
        texture_data: &TextureData,
        texture_id: TextureId,
    ) -> RendererResult<bool> {
        let wgpu_tex = match self.textures.get(&texture_id) {
            Some(t) => t,
            None => return Ok(false),
        };

        // Collect update rectangles; prefer explicit Updates[] if present,
        // otherwise fallback to single UpdateRect.
        let mut rects: Vec<dear_imgui_rs::texture::TextureRect> = texture_data.updates().collect();
        if rects.is_empty() {
            let r = texture_data.update_rect();
            if r.w > 0 && r.h > 0 {
                rects.push(r);
            }
        }
        if rects.is_empty() {
            return Ok(false);
        }

        // Upload each rect
        for rect in rects {
            if let Some(tight_rgba) = Self::convert_subrect_to_rgba(texture_data, rect) {
                let width = rect.w as u32;
                let height = rect.h as u32;
                let bpp = 4u32;
                let unpadded_bytes_per_row = width * bpp;
                let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT; // 256
                let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

                if padded_bytes_per_row == unpadded_bytes_per_row {
                    // Aligned: direct upload
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: wgpu_tex.texture(),
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: rect.x as u32,
                                y: rect.y as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &tight_rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(unpadded_bytes_per_row),
                            rows_per_image: Some(height),
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                } else {
                    // Pad each row to the required alignment
                    let mut padded = vec![0u8; (padded_bytes_per_row * height) as usize];
                    for row in 0..height as usize {
                        let src_off = row * (unpadded_bytes_per_row as usize);
                        let dst_off = row * (padded_bytes_per_row as usize);
                        padded[dst_off..dst_off + (unpadded_bytes_per_row as usize)]
                            .copy_from_slice(
                                &tight_rgba[src_off..src_off + (unpadded_bytes_per_row as usize)],
                            );
                    }
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: wgpu_tex.texture(),
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: rect.x as u32,
                                y: rect.y as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &padded,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_bytes_per_row),
                            rows_per_image: Some(height),
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Updated texture id={} subrect x={} y={} w={} h={}",
                        texture_id.id(), rect.x, rect.y, rect.w, rect.h
                    );
                }
            } else {
                // No pixels available, cannot update this rect
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Skipped subrect update: no pixels available"
                    );
                }
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Update an existing texture from Dear ImGui texture data with specific ID
    pub fn update_texture_from_data_with_id(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
        texture_id: TextureId,
    ) -> RendererResult<()> {
        // For WGPU, we recreate the texture instead of updating in place.
        // Create the replacement first so a failure does not destroy the existing texture.
        if self.contains_texture(texture_id) {
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;

            // Move the texture to the correct ID slot if needed.
            if new_texture_id != texture_id
                && let Some(texture) = self.remove_texture(new_texture_id)
            {
                self.remove_texture(texture_id);
                self.insert_texture_with_id(texture_id, texture);
            }

            Ok(())
        } else {
            Err(RendererError::InvalidTextureId(texture_id))
        }
    }

    /// Texture creation and management functions
    /// Create a texture from Dear ImGui texture data
    pub fn create_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> RendererResult<TextureId> {
        let width = texture_data.width();
        let height = texture_data.height();
        let format = texture_data.format();

        let pixels = texture_data
            .pixels()
            .ok_or_else(|| RendererError::BadTexture("No pixel data available".to_string()))?;

        // Convert ImGui texture format to WGPU format and handle data conversion
        // This matches the texture format handling in imgui_impl_wgpu.cpp
        let (wgpu_format, converted_data, _bytes_per_pixel) = match format {
            ImGuiTextureFormat::RGBA32 => {
                // RGBA32 maps directly to RGBA8Unorm (matches C++ implementation)
                let expected_len = usize::try_from(width)
                    .ok()
                    .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
                    .and_then(|px| px.checked_mul(4))
                    .ok_or_else(|| {
                        RendererError::BadTexture("RGBA32 texture size overflow".to_string())
                    })?;
                if pixels.len() != expected_len {
                    return Err(RendererError::BadTexture(format!(
                        "RGBA32 texture data size mismatch: expected {} bytes, got {}",
                        expected_len,
                        pixels.len()
                    )));
                }
                (TextureFormat::Rgba8Unorm, pixels.to_vec(), 4u32)
            }
            ImGuiTextureFormat::Alpha8 => {
                // Convert Alpha8 to RGBA32 for WGPU (white RGB + original alpha)
                // This ensures compatibility with the standard RGBA8Unorm format
                let expected_len = usize::try_from(width)
                    .ok()
                    .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
                    .ok_or_else(|| {
                        RendererError::BadTexture("Alpha8 texture size overflow".to_string())
                    })?;
                if pixels.len() != expected_len {
                    return Err(RendererError::BadTexture(format!(
                        "Alpha8 texture data size mismatch: expected {} bytes, got {}",
                        expected_len,
                        pixels.len()
                    )));
                }
                let mut rgba_data = Vec::with_capacity(pixels.len() * 4);
                for &alpha in pixels {
                    rgba_data.extend_from_slice(&[255, 255, 255, alpha]); // White RGB + alpha
                }
                (TextureFormat::Rgba8Unorm, rgba_data, 4u32)
            }
        };

        // Create WGPU texture (matches the descriptor setup in imgui_impl_wgpu.cpp)
        if cfg!(debug_assertions) {
            tracing::debug!(
                target: "dear-imgui-wgpu",
                "[dear-imgui-wgpu][debug] Create texture: {}x{} format={:?}",
                width, height, format
            );
        }
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Dear ImGui Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: wgpu_format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Validate texture data size before upload
        let expected_size = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .and_then(|px| px.checked_mul(4))
            .ok_or_else(|| {
                RendererError::BadTexture("Converted texture size overflow".to_string())
            })?; // Always RGBA after conversion
        if converted_data.len() != expected_size {
            return Err(RendererError::BadTexture(format!(
                "Converted texture data size mismatch: expected {} bytes, got {}",
                expected_size,
                converted_data.len()
            )));
        }

        // Upload texture data (matches the upload logic in imgui_impl_wgpu.cpp)
        // WebGPU requires bytes_per_row to be 256-byte aligned. Pad rows if needed.
        let bpp = 4u32;
        let unpadded_bytes_per_row = width * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT; // 256
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        if padded_bytes_per_row == unpadded_bytes_per_row {
            // Aligned: direct upload
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &converted_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            // Pad each row to the required alignment
            let mut padded: Vec<u8> = vec![0; (padded_bytes_per_row * height) as usize];
            for row in 0..height as usize {
                let src_off = row * (unpadded_bytes_per_row as usize);
                let dst_off = row * (padded_bytes_per_row as usize);
                padded[dst_off..dst_off + (unpadded_bytes_per_row as usize)].copy_from_slice(
                    &converted_data[src_off..src_off + (unpadded_bytes_per_row as usize)],
                );
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &padded,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Upload texture with padded row pitch: unpadded={} padded={}",
                    unpadded_bytes_per_row, padded_bytes_per_row
                );
            }
        }

        // Create texture view
        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        // Create WGPU texture wrapper
        let wgpu_texture = WgpuTexture::new(texture, texture_view);

        // Register and return ID
        let texture_id = self.register_texture(wgpu_texture);
        if cfg!(debug_assertions) {
            tracing::debug!(
                target: "dear-imgui-wgpu",
                "[dear-imgui-wgpu][debug] Texture registered: id={}",
                texture_id.id()
            );
        }
        Ok(texture_id)
    }

    /// Update an existing texture from Dear ImGui texture data
    pub fn update_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> RendererResult<()> {
        let texture_id = texture_data.tex_id();

        // If the texture already exists and the TextureData only requests sub-rectangle
        // updates, honor them in-place to match Dear ImGui 1.92 semantics.
        // Fallback to full re-create when there is no pixel data available.
        if self.contains_texture(texture_id) {
            // Attempt sub-rect updates first (preferred path)
            if self.apply_subrect_updates(queue, texture_data, texture_id)? {
                return Ok(());
            }

            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;
            if new_texture_id != texture_id
                && let Some(texture) = self.remove_texture(new_texture_id)
            {
                self.remove_texture(texture_id);
                self.insert_texture_with_id(texture_id, texture);
            }
        } else {
            // Create new texture if it doesn't exist
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;
            if new_texture_id != texture_id
                && let Some(texture) = self.remove_texture(new_texture_id)
            {
                self.insert_texture_with_id(texture_id, texture);
            }
        }

        Ok(())
    }

    /// Update a single texture based on its status
    ///
    /// This corresponds to ImGui_ImplWGPU_UpdateTexture in the C++ implementation.
    ///
    /// # Returns
    ///
    /// Returns a `TextureUpdateResult` that contains the operation result and
    /// any status/ID updates that need to be applied to the texture data.
    /// This follows Rust's principle of explicit state management.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui_wgpu::*;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut texture_manager = WgpuTextureManager::new();
    /// # let device = todo!();
    /// # let queue = todo!();
    /// # let mut texture_data = dear_imgui_rs::TextureData::new();
    /// let result = texture_manager.update_single_texture(&texture_data, &device, &queue)?;
    /// result.apply_to(&mut texture_data);
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_single_texture(
        &mut self,
        texture_data: &dear_imgui_rs::TextureData,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<TextureUpdateResult> {
        match texture_data.status() {
            TextureStatus::WantCreate => {
                let texture_id = self.create_texture_from_data(device, queue, texture_data)?;
                Ok(TextureUpdateResult::Created { texture_id })
            }
            TextureStatus::WantUpdates => {
                let internal_id = texture_data.tex_id();
                if internal_id.is_null() || !self.contains_texture(internal_id) {
                    // No valid ID yet: create now and return Created so caller can set TexID
                    let texture_id = self.create_texture_from_data(device, queue, texture_data)?;
                    Ok(TextureUpdateResult::Created { texture_id })
                } else {
                    match self.update_texture_from_data_with_id(
                        device,
                        queue,
                        texture_data,
                        internal_id,
                    ) {
                        Ok(_) => Ok(TextureUpdateResult::Updated),
                        Err(e) => Err(e),
                    }
                }
            }
            TextureStatus::WantDestroy => {
                self.destroy_texture(texture_data.tex_id());
                Ok(TextureUpdateResult::Destroyed)
            }
            TextureStatus::OK | TextureStatus::Destroyed => Ok(TextureUpdateResult::NoAction),
        }
    }
}
