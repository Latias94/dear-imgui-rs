use dear_imgui_rs::{TextureData, TextureFormat, TextureId};
use glow::{Context, HasContext};

use super::GlowRenderer;
use crate::{
    error::{InitError, InitResult, RenderError, RenderResult},
    texture::{checked_gl_texture_size, gl_texture_size_i32},
};

impl GlowRenderer {
    #[inline]
    fn is_font_atlas_texture(&self, texture_data: &dear_imgui_rs::TextureData) -> bool {
        !self.font_atlas_texture_data.is_null()
            && std::ptr::eq(
                texture_data.as_raw(),
                self.font_atlas_texture_data.cast_const(),
            )
    }

    fn convert_subrect_to_rgba(
        texture_data: &dear_imgui_rs::TextureData,
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

        let rw = rw.min(tex_w.saturating_sub(rx));
        let rh = rh.min(tex_h.saturating_sub(ry));

        let mut out = vec![0u8; rw.checked_mul(rh)?.checked_mul(4)?];
        match texture_data.format() {
            dear_imgui_rs::TextureFormat::RGBA32 => {
                for row in 0..rh {
                    let src_off = ((ry + row) * tex_w + rx) * bpp;
                    let dst_off = row * rw * 4;
                    out[dst_off..dst_off + rw * 4]
                        .copy_from_slice(&pixels[src_off..src_off + rw * 4]);
                }
            }
            dear_imgui_rs::TextureFormat::Alpha8 => {
                for row in 0..rh {
                    let src_off = ((ry + row) * tex_w + rx) * bpp;
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

    /// Update texture from Dear ImGui texture data
    /// Following the original Dear ImGui OpenGL3 implementation
    pub(super) fn update_texture_from_data(
        &mut self,
        gl: Option<&Context>,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        use dear_imgui_rs::TextureStatus;

        match texture_data.status() {
            TextureStatus::WantCreate => {
                // Create new texture and assign ID back to Dear ImGui
                let gl = Self::required_gl_context(gl)?;
                self.create_texture_from_data(gl, texture_data)?;
            }
            TextureStatus::WantUpdates => {
                // Update existing texture
                let gl = Self::required_gl_context(gl)?;
                self.update_existing_texture_from_data(gl, texture_data)?;
            }
            TextureStatus::WantDestroy => {
                // Destroy texture
                self.destroy_texture_from_data(gl, texture_data)?;
            }
            TextureStatus::OK | TextureStatus::Destroyed => {
                // Nothing to do
            }
        }

        Ok(())
    }

    fn required_gl_context(gl: Option<&Context>) -> RenderResult<&Context> {
        gl.ok_or(RenderError::MissingGlContext)
    }

    /// Create a new texture from ImTextureData
    fn create_texture_from_data(
        &mut self,
        gl: &Context,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        let is_font_atlas = self.is_font_atlas_texture(texture_data);
        let width = texture_data.width();
        let height = texture_data.height();
        let (width_i32, height_i32) =
            checked_gl_texture_size(width, height).map_err(RenderError::DeviceObjectInit)?;
        let format = texture_data.format();

        if let Some(pixels) = texture_data.pixels() {
            let gl_texture = unsafe {
                // Backup texture binding / active texture / unpack alignment
                let last_active = u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE))
                    .ok()
                    .unwrap_or(glow::TEXTURE0);
                gl.active_texture(glow::TEXTURE0);
                let last_texture = u32::try_from(gl.get_parameter_i32(glow::TEXTURE_BINDING_2D))
                    .ok()
                    .and_then(std::num::NonZeroU32::new)
                    .map(glow::NativeTexture);
                let last_unpack = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);

                let gl_texture = gl
                    .create_texture()
                    .map_err(|e| RenderError::CreateResource {
                        resource: "texture",
                        error: e,
                    })?;

                gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

                // Set texture parameters
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::LINEAR as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_S,
                    glow::CLAMP_TO_EDGE as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_T,
                    glow::CLAMP_TO_EDGE as i32,
                );

                // Upload texture data based on format
                match format {
                    dear_imgui_rs::TextureFormat::RGBA32 => {
                        gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA as i32,
                            width_i32,
                            height_i32,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(pixels)),
                        );
                    }
                    dear_imgui_rs::TextureFormat::Alpha8 => {
                        // NOTE(opt): Could use GL RED + TEXTURE_SWIZZLE to avoid 4x expansion when
                        // GL 3.3+/GLES3.0+/ARB_texture_swizzle is available. See note in prepare_font_atlas().
                        // Convert Alpha8 to RGBA32 for OpenGL
                        let mut rgba_data = Vec::with_capacity(
                            usize::try_from(width)
                                .ok()
                                .and_then(|w| {
                                    usize::try_from(height).ok().and_then(|h| w.checked_mul(h))
                                })
                                .and_then(|px| px.checked_mul(4))
                                .ok_or_else(|| {
                                    RenderError::InvalidTexture(
                                        "Alpha8 texture size overflow".to_string(),
                                    )
                                })?,
                        );
                        for &alpha in pixels {
                            rgba_data.push(255); // R
                            rgba_data.push(255); // G
                            rgba_data.push(255); // B
                            rgba_data.push(alpha); // A
                        }

                        gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA as i32,
                            width_i32,
                            height_i32,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(&rgba_data)),
                        );
                    }
                }

                // Restore pixel store and previous binding
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack);
                gl.bind_texture(glow::TEXTURE_2D, last_texture);
                gl.active_texture(last_active);
                gl_texture
            };
            // Register texture and set ID back to Dear ImGui
            let tex_id = self
                .texture_map_mut()
                .register_texture(gl_texture, width, height, format);
            texture_data.set_tex_id(tex_id);
            texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
            if is_font_atlas {
                self.font_atlas_texture = Some(gl_texture);
            }
        }

        Ok(())
    }

    /// Update an existing texture from ImTextureData
    fn update_existing_texture_from_data(
        &mut self,
        gl: &Context,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        let is_font_atlas = self.is_font_atlas_texture(texture_data);
        let tex_id = texture_data.tex_id();
        let gl_texture = match self.texture_map().get(tex_id) {
            Some(t) => t,
            None => {
                // If texture doesn't exist, create it fully
                return self.create_texture_from_data(gl, texture_data);
            }
        };

        match texture_data.pixels() {
            Some(_) => {}
            None => {
                // No CPU pixels available this frame; nothing to upload.
                // Mark as OK to avoid retry storm and proceed with existing GPU texture.
                texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
                return Ok(());
            }
        };

        // Backup texture binding / active texture / unpack alignment
        let last_active = u32::try_from(unsafe { gl.get_parameter_i32(glow::ACTIVE_TEXTURE) })
            .ok()
            .unwrap_or(glow::TEXTURE0);
        let last_texture = u32::try_from(unsafe { gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) })
            .ok()
            .and_then(std::num::NonZeroU32::new)
            .map(glow::NativeTexture);
        let last_unpack = unsafe { gl.get_parameter_i32(glow::UNPACK_ALIGNMENT) };
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        }

        // Collect update rects; prefer explicit Updates[] then fallback to single UpdateRect
        let mut rects: Vec<dear_imgui_rs::texture::TextureRect> = texture_data.updates().collect();
        if rects.is_empty() {
            let r = texture_data.update_rect();
            if r.w > 0 && r.h > 0 {
                rects.push(r);
            }
        }

        if rects.is_empty() {
            // Nothing to update; mark OK and return
            texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
            // Restore previous binding and pixel store
            unsafe {
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack);
                gl.bind_texture(glow::TEXTURE_2D, last_texture);
                gl.active_texture(last_active);
            }
            return Ok(());
        }

        // Iterate update rects and upload each sub-region
        for rect in rects.into_iter() {
            let rx = rect.x as u32;
            let ry = rect.y as u32;
            let rw = rect.w as u32;
            let rh = rect.h as u32;
            let rx_i32 = gl_texture_size_i32("x", rx).map_err(RenderError::DeviceObjectInit)?;
            let ry_i32 = gl_texture_size_i32("y", ry).map_err(RenderError::DeviceObjectInit)?;
            let (rw_i32, rh_i32) =
                checked_gl_texture_size(rw, rh).map_err(RenderError::DeviceObjectInit)?;

            let Some(sub_rgba) = Self::convert_subrect_to_rgba(texture_data, rect) else {
                continue;
            };

            unsafe {
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    rx_i32,
                    ry_i32,
                    rw_i32,
                    rh_i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&sub_rgba)),
                );
            }
        }

        // Restore previous binding and pixel store
        unsafe {
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack);
            gl.bind_texture(glow::TEXTURE_2D, last_texture);
            gl.active_texture(last_active);
        }

        // Mark status OK after updates
        texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
        if is_font_atlas {
            self.font_atlas_texture = Some(gl_texture);
        }
        Ok(())
    }

    /// Destroy a texture from ImTextureData
    fn destroy_texture_from_data(
        &mut self,
        gl: Option<&Context>,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        let is_font_atlas = self.is_font_atlas_texture(texture_data);
        let texture_id = texture_data.tex_id();

        if let Some(gl_texture) = self.texture_map().get(texture_id) {
            let gl = Self::required_gl_context(gl)?;
            unsafe {
                gl.delete_texture(gl_texture);
            }
            self.texture_map_mut().remove(texture_id);
        }

        unsafe {
            (*texture_data.as_raw_mut()).WantDestroyNextFrame = true;
        }
        texture_data.set_status(dear_imgui_rs::TextureStatus::Destroyed);
        if is_font_atlas {
            self.font_atlas_texture = None;
        }

        Ok(())
    }

    /// Update a texture from ImGui texture data
    pub fn update_texture(
        &mut self,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> InitResult<()> {
        let gl = self.gl_context.clone().ok_or(InitError::MissingGlContext)?;
        self.update_texture_with_context(&gl, texture_id, width, height, data)
    }

    /// Update a texture using an externally managed OpenGL context.
    pub fn update_texture_with_context(
        &mut self,
        gl: &Context,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> InitResult<()> {
        use crate::texture::{update_imgui_texture, upload_texture_data};

        if texture_id.is_null() {
            return Err(InitError::NullTextureId);
        }

        let gl_texture = if let Some(gl_texture) = self.texture_map().get(texture_id) {
            let format = self
                .texture_map()
                .get_texture_data(texture_id)
                .map(TextureData::format)
                .unwrap_or(TextureFormat::RGBA32);
            upload_texture_data(gl, gl_texture, width, height, format, data)?;
            gl_texture
        } else {
            update_imgui_texture(gl, texture_id, width, height, data)?
        };

        // Update the texture mapping with modern texture management
        self.texture_map_mut()
            .update_texture(texture_id, gl_texture, width, height);

        Ok(())
    }

    /// Register a new texture with the modern texture management system
    pub fn register_texture(
        &mut self,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> InitResult<TextureId> {
        let gl = self.gl_context.clone().ok_or(InitError::MissingGlContext)?;
        self.register_texture_with_context(&gl, width, height, format, data)
    }

    /// Register a new texture using an externally managed OpenGL context.
    pub fn register_texture_with_context(
        &mut self,
        gl: &Context,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> InitResult<TextureId> {
        use crate::texture::{create_texture_from_alpha, create_texture_from_rgba};
        let gl_texture = match format {
            TextureFormat::RGBA32 => create_texture_from_rgba(gl, width, height, data)?,
            TextureFormat::Alpha8 => create_texture_from_alpha(gl, width, height, data)?,
        };
        let texture_id = self
            .texture_map_mut()
            .register_texture(gl_texture, width, height, format);

        Ok(texture_id)
    }

    /// Get texture data for a given texture ID
    pub fn get_texture_data(&self, texture_id: TextureId) -> Option<&TextureData> {
        self.texture_map.as_deref()?.get_texture_data(texture_id)
    }

    /// Get mutable texture data for a given texture ID
    pub fn get_texture_data_mut(&mut self, texture_id: TextureId) -> Option<&mut TextureData> {
        self.texture_map
            .as_deref_mut()?
            .get_texture_data_mut(texture_id)
    }
}

#[cfg(test)]
mod tests {
    use super::GlowRenderer;
    use crate::{
        shaders::Shaders, state::GlStateBackup, texture::SimpleTextureMap, versions::GlVersion,
    };
    use dear_imgui_rs::{
        TextureData, TextureFormat, TextureId, TextureStatus, texture::TextureRect,
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    static LAST_BOUND_TEXTURE: AtomicU32 = AtomicU32::new(0);

    fn make_test_renderer() -> GlowRenderer {
        GlowRenderer {
            shaders: Shaders {
                program: None,
                attrib_location_tex: None,
                attrib_location_proj_mtx: None,
                attrib_location_color_gamma: None,
                attrib_location_vtx_pos: 0,
                attrib_location_vtx_uv: 0,
                attrib_location_vtx_color: 0,
            },
            state_backup: GlStateBackup::default(),
            vbo_handle: None,
            ebo_handle: None,
            font_atlas_texture: None,
            font_atlas_texture_data: std::ptr::null_mut(),
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version: GlVersion {
                major: 3,
                minor: 3,
                is_es: false,
            },
            has_clip_origin_support: false,
            is_destroyed: false,
            gl_context: None,
            texture_map: Some(Box::new(SimpleTextureMap::default())),
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn make_fake_gl() -> glow::Context {
        unsafe extern "system" fn fake_gl_get_string(_name: u32) -> *const u8 {
            b"4.6\0".as_ptr()
        }
        unsafe extern "system" fn fake_gl_get_string_i(_name: u32, _index: u32) -> *const u8 {
            b"\0".as_ptr()
        }
        unsafe extern "system" fn fake_gl_get_integer_v(pname: u32, data: *mut i32) {
            if data.is_null() {
                return;
            }
            let value = match pname {
                glow::ACTIVE_TEXTURE => glow::TEXTURE0 as i32,
                glow::TEXTURE_BINDING_2D => 0,
                glow::UNPACK_ALIGNMENT => 4,
                _ => 0,
            };
            unsafe {
                *data = value;
            }
        }

        unsafe extern "system" fn fake_gl_active_texture(_texture: u32) {}
        unsafe extern "system" fn fake_gl_bind_texture(_target: u32, texture: u32) {
            if texture != 0 {
                LAST_BOUND_TEXTURE.store(texture, Ordering::SeqCst);
            }
        }
        unsafe extern "system" fn fake_gl_pixel_store_i(_pname: u32, _param: i32) {}
        unsafe extern "system" fn fake_gl_tex_image_2d(
            _target: u32,
            _level: i32,
            _internalformat: i32,
            _width: i32,
            _height: i32,
            _border: i32,
            _format: u32,
            _type_: u32,
            _pixels: *const std::ffi::c_void,
        ) {
        }

        unsafe {
            glow::Context::from_loader_function(|name| {
                let ptr = match name {
                    "glGetString" => fake_gl_get_string as *const () as *const std::ffi::c_void,
                    "glGetStringi" => fake_gl_get_string_i as *const () as *const std::ffi::c_void,
                    "glGetIntegerv" => {
                        fake_gl_get_integer_v as *const () as *const std::ffi::c_void
                    }
                    "glActiveTexture" => {
                        fake_gl_active_texture as *const () as *const std::ffi::c_void
                    }
                    "glBindTexture" => fake_gl_bind_texture as *const () as *const std::ffi::c_void,
                    "glPixelStorei" => {
                        fake_gl_pixel_store_i as *const () as *const std::ffi::c_void
                    }
                    "glTexImage2D" => fake_gl_tex_image_2d as *const () as *const std::ffi::c_void,
                    _ => std::ptr::null(),
                };
                ptr
            })
        }
    }

    #[test]
    fn convert_subrect_to_rgba_rgba32_full_rect() {
        let mut tex = TextureData::new();
        tex.create(TextureFormat::RGBA32, 2, 2);

        let pixels: [u8; 16] = [
            10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
        ];
        tex.set_data(&pixels);

        let rect = TextureRect {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        };

        let out = GlowRenderer::convert_subrect_to_rgba(&tex, rect).expect("expected data");
        assert_eq!(out, pixels);
    }

    #[test]
    fn convert_subrect_to_rgba_alpha8_partial_rect() {
        let mut tex = TextureData::new();
        tex.create(TextureFormat::Alpha8, 3, 2);

        let alphas: [u8; 6] = [0, 10, 20, 30, 40, 50];
        tex.set_data(&alphas);

        let rect = TextureRect {
            x: 1,
            y: 0,
            w: 2,
            h: 2,
        };

        let out = GlowRenderer::convert_subrect_to_rgba(&tex, rect).expect("expected data");
        assert_eq!(
            out,
            vec![
                255, 255, 255, 10, 255, 255, 255, 20, 255, 255, 255, 40, 255, 255, 255, 50,
            ]
        );
    }

    #[test]
    fn convert_subrect_to_rgba_out_of_bounds_returns_none() {
        let mut tex = TextureData::new();
        tex.create(TextureFormat::RGBA32, 2, 2);
        let rect = TextureRect {
            x: 10,
            y: 10,
            w: 1,
            h: 1,
        };

        assert!(GlowRenderer::convert_subrect_to_rgba(&tex, rect).is_none());
    }

    #[test]
    fn destroy_texture_with_external_context_does_not_require_owned_gl_context() {
        let mut renderer = make_test_renderer();

        let texture_id = TextureId::new(42);
        let mut tex = TextureData::new();
        tex.set_tex_id(texture_id);
        tex.set_status(TextureStatus::WantDestroy);

        renderer
            .update_texture_from_data(None, &mut tex)
            .expect("destroying an unknown texture should not require an owned GL context");

        assert_eq!(tex.status(), TextureStatus::Destroyed);
    }

    #[test]
    fn update_texture_with_context_uses_registered_gl_texture() {
        let mut renderer = make_test_renderer();
        let texture_id = TextureId::from(42u64);
        let gl_texture = glow::NativeTexture(std::num::NonZeroU32::new(99).unwrap());
        renderer.texture_map_mut().set(texture_id, gl_texture);

        LAST_BOUND_TEXTURE.store(0, Ordering::SeqCst);
        let gl = make_fake_gl();
        let data = [1u8, 2, 3, 4];

        renderer
            .update_texture_with_context(&gl, texture_id, 1, 1, &data)
            .expect("update should use the registered GL texture");

        assert_eq!(LAST_BOUND_TEXTURE.load(Ordering::SeqCst), 99);
    }
}
