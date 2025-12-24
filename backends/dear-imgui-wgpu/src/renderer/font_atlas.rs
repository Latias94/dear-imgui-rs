// Renderer font atlas handling (prepare & fallback upload)

use super::*;
use dear_imgui_rs::Context;

impl WgpuRenderer {
    /// Load font texture from Dear ImGui context
    ///
    /// With the new texture management system in Dear ImGui 1.92+, font textures are
    /// automatically managed through ImDrawData->Textures[] during rendering.
    /// Do not manually call `fonts.build()` here: with ImGui 1.92+ this is handled by
    /// `ImFontAtlasUpdateNewFrame()` when `BackendFlags::RENDERER_HAS_TEXTURES` is set, and
    /// calling Build() in the legacy mode can trigger assertions on the next frame.
    pub(super) fn reload_font_texture(
        &mut self,
        imgui_ctx: &mut Context,
        _device: &Device,
        _queue: &Queue,
    ) -> RendererResult<()> {
        let _ = imgui_ctx;

        Ok(())
    }

    /// Legacy/fallback path: upload font atlas texture immediately and assign TexID.
    /// Returns Some(tex_id) on success, None if texdata is unavailable.
    pub(super) fn try_upload_font_atlas_legacy(
        &mut self,
        imgui_ctx: &mut Context,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<Option<u64>> {
        // SAFETY: Access raw TexData/bytes only to copy pixels. Requires fonts.build() called.
        let fonts = imgui_ctx.font_atlas();
        // Try to read raw texture data to determine bytes-per-pixel
        let raw_tex = fonts.get_tex_data();
        if raw_tex.is_null() {
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas TexData is null; skip legacy upload"
                );
            }
            return Ok(None);
        }
        // Read metadata
        let (width, height, bpp, pixels_slice): (u32, u32, i32, Option<&[u8]>) = unsafe {
            let w = (*raw_tex).Width as u32;
            let h = (*raw_tex).Height as u32;
            let bpp = (*raw_tex).BytesPerPixel;
            let px_ptr = (*raw_tex).Pixels as *const u8;
            if px_ptr.is_null() || w == 0 || h == 0 || bpp <= 0 {
                (w, h, bpp, None)
            } else {
                let bpp_usize = match usize::try_from(bpp) {
                    Ok(v) if v > 0 => v,
                    _ => 0,
                };
                let size = (w as usize)
                    .checked_mul(h as usize)
                    .and_then(|v| v.checked_mul(bpp_usize));
                match size {
                    Some(size) => (w, h, bpp, Some(std::slice::from_raw_parts(px_ptr, size))),
                    None => (w, h, bpp, None),
                }
            }
        };

        if let Some(src) = pixels_slice {
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas texdata: {}x{} bpp={} (fallback upload for font atlas)",
                    width, height, bpp
                );
            }
            // Convert to RGBA8 if needed
            let (format, converted): (wgpu::TextureFormat, Vec<u8>) = if bpp == 4 {
                (wgpu::TextureFormat::Rgba8Unorm, src.to_vec())
            } else if bpp == 1 {
                // Alpha8 -> RGBA8 (white RGB + alpha)
                let px_count = match (width as usize).checked_mul(height as usize) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let cap = match px_count.checked_mul(4) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let mut out = Vec::with_capacity(cap);
                for &a in src.iter() {
                    out.extend_from_slice(&[255, 255, 255, a]);
                }
                (wgpu::TextureFormat::Rgba8Unorm, out)
            } else {
                // Unexpected format; don't proceed
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Unexpected font atlas bpp={} -> skip",
                        bpp
                    );
                }
                return Ok(None);
            };

            // Create WGPU texture
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Dear ImGui Font Atlas"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            // Write with 256-byte aligned row pitch
            let bpp = 4u32;
            let unpadded = width * bpp;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded = unpadded.div_ceil(align) * align;
            if padded == unpadded {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &converted,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(unpadded),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            } else {
                let mut padded_buf = vec![0u8; (padded * height) as usize];
                for row in 0..height as usize {
                    let src = row * (unpadded as usize);
                    let dst = row * (padded as usize);
                    padded_buf[dst..dst + (unpadded as usize)]
                        .copy_from_slice(&converted[src..src + (unpadded as usize)]);
                }
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &padded_buf,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Upload font atlas with padded row pitch: unpadded={} padded={}",
                        unpadded, padded
                    );
                }
            }

            // Register texture and set IDs so draw commands can bind it
            let tex_id = self
                .texture_manager
                .register_texture(crate::WgpuTexture::new(texture, view));

            // Set atlas texture id + status OK (updates TexRef and TexData)
            {
                let mut fonts_mut = imgui_ctx.font_atlas_mut();
                fonts_mut.set_texture_id(dear_imgui_rs::TextureId::from(tex_id));
            }
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas fallback upload complete: tex_id={}",
                    tex_id
                );
            }

            return Ok(Some(tex_id));
        }
        if cfg!(debug_assertions) {
            tracing::debug!(
                target: "dear-imgui-wgpu",
                "[dear-imgui-wgpu][debug] Font atlas has no CPU pixel buffer; skipping fallback upload (renderer will use modern texture updates)"
            );
        }
        Ok(None)
    }
}
