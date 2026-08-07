use std::borrow::Cow;

use dear_imgui_rs::render::{
    SnapshotTextureId, TextureFeedback, TextureOp, TextureRequest, TextureUploadRect,
};
use dear_imgui_rs::{TextureFormat, TextureId};
use glow::{Context, HasContext};

use super::GlowRenderer;
use crate::texture::{
    GlTextureUpdate, alpha8_to_rgba, create_texture_from_alpha, create_texture_from_rgba,
    update_texture, upload_texture_data,
};
use crate::{
    GlTexture,
    error::{InitError, InitResult, RenderError, RenderResult},
};

#[derive(Copy, Clone, Debug)]
pub(super) struct ManagedTextureBinding {
    pub(super) texture_id: TextureId,
    pub(super) gl_texture: GlTexture,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct ManagedTextureTombstone {
    latest_destroy_epoch: u64,
    acknowledged_epoch: Option<u64>,
}

struct ManagedTextureCreate<'a> {
    format: TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &'a [u8],
}

impl GlowRenderer {
    pub(super) fn process_texture_requests(
        &mut self,
        gl: &Context,
        requests: &[TextureRequest],
        request_epoch: u64,
    ) -> RenderResult<Vec<TextureFeedback>> {
        let mut feedback = Vec::with_capacity(requests.len());
        for request in requests {
            feedback.push(self.process_texture_request(gl, request, request_epoch)?);
        }
        debug_assert_eq!(feedback.len(), requests.len());
        Ok(feedback)
    }

    fn process_texture_request(
        &mut self,
        gl: &Context,
        request: &TextureRequest,
        request_epoch: u64,
    ) -> RenderResult<TextureFeedback> {
        let texture = request.texture();
        if !matches!(request.operation(), TextureOp::Destroy)
            && self.destroyed_managed_textures.contains_key(&texture)
        {
            return Ok(request.superseded());
        }
        match request.operation() {
            TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => {
                let texture_id = self.create_managed_texture(
                    gl,
                    request.texture(),
                    ManagedTextureCreate {
                        format: *format,
                        width: *width,
                        height: *height,
                        row_pitch: *row_pitch,
                        pixels,
                    },
                )?;
                Ok(request.uploaded(texture_id)?)
            }
            TextureOp::Update {
                format,
                width,
                height,
                rects,
            } => {
                let texture_id = self.update_managed_texture(
                    gl,
                    request.texture(),
                    *format,
                    *width,
                    *height,
                    rects,
                )?;
                Ok(request.uploaded(texture_id)?)
            }
            TextureOp::Destroy => {
                self.seal_destroyed_managed_texture(texture, request_epoch);
                self.destroy_managed_texture(gl, texture);
                let feedback = request.destroyed()?;
                self.acknowledge_destroyed_managed_texture(texture, request_epoch);
                Ok(feedback)
            }
        }
    }

    fn seal_destroyed_managed_texture(&mut self, texture: SnapshotTextureId, request_epoch: u64) {
        self.destroyed_managed_textures
            .entry(texture)
            .and_modify(|tombstone| {
                if request_epoch > tombstone.latest_destroy_epoch {
                    tombstone.latest_destroy_epoch = request_epoch;
                    tombstone.acknowledged_epoch = None;
                }
            })
            .or_insert(ManagedTextureTombstone {
                latest_destroy_epoch: request_epoch,
                acknowledged_epoch: None,
            });
    }

    fn acknowledge_destroyed_managed_texture(
        &mut self,
        texture: SnapshotTextureId,
        request_epoch: u64,
    ) {
        let tombstone =
            self.destroyed_managed_textures
                .entry(texture)
                .or_insert(ManagedTextureTombstone {
                    latest_destroy_epoch: request_epoch,
                    acknowledged_epoch: None,
                });
        if request_epoch < tombstone.latest_destroy_epoch {
            return;
        }
        tombstone.latest_destroy_epoch = request_epoch;
        tombstone.acknowledged_epoch = Some(
            tombstone
                .acknowledged_epoch
                .map_or(request_epoch, |epoch| epoch.max(request_epoch)),
        );
    }

    pub(super) fn prune_destroyed_managed_textures(&mut self, completion_watermark: u64) {
        self.destroyed_managed_textures
            .retain(|_, tombstone| match tombstone.acknowledged_epoch {
                Some(epoch) => {
                    epoch < tombstone.latest_destroy_epoch || epoch > completion_watermark
                }
                None => true,
            });
    }

    fn create_managed_texture(
        &mut self,
        gl: &Context,
        key: SnapshotTextureId,
        create: ManagedTextureCreate<'_>,
    ) -> RenderResult<TextureId> {
        let pixels = tightly_pack_rows(
            create.format,
            create.width,
            create.height,
            create.row_pitch,
            create.pixels,
        )
        .map_err(RenderError::DeviceObjectInit)?;

        if let Some(binding) = self.managed_textures.get(&key).copied() {
            upload_texture_data(
                gl,
                binding.gl_texture,
                create.width,
                create.height,
                create.format,
                &pixels,
            )
            .map_err(RenderError::DeviceObjectInit)?;
            self.texture_map_mut().update_texture(
                binding.texture_id,
                binding.gl_texture,
                create.width,
                create.height,
            );
            return Ok(binding.texture_id);
        }

        let gl_texture = match create.format {
            TextureFormat::RGBA32 => {
                create_texture_from_rgba(gl, create.width, create.height, &pixels)
            }
            TextureFormat::Alpha8 => {
                create_texture_from_alpha(gl, create.width, create.height, &pixels)
            }
        }
        .map_err(RenderError::DeviceObjectInit)?;

        // Publish ownership before entering application-provided TextureMap code. If that code
        // unwinds, explicit renderer teardown still clears any partial map state before deleting
        // the GL object.
        self.track_owned_texture(gl_texture);
        let texture_id = match self.texture_map_mut().register_texture(
            gl_texture,
            create.width,
            create.height,
            create.format,
        ) {
            Ok(texture_id) => texture_id,
            Err(error) => {
                unsafe { gl.delete_texture(gl_texture) };
                self.forget_owned_texture(gl_texture);
                return Err(RenderError::DeviceObjectInit(error));
            }
        };
        self.managed_textures.insert(
            key,
            ManagedTextureBinding {
                texture_id,
                gl_texture,
            },
        );
        Ok(texture_id)
    }

    fn update_managed_texture(
        &mut self,
        gl: &Context,
        key: SnapshotTextureId,
        format: TextureFormat,
        width: u32,
        height: u32,
        rects: &[TextureUploadRect],
    ) -> RenderResult<TextureId> {
        let binding = self
            .managed_textures
            .get(&key)
            .copied()
            .ok_or(RenderError::ManagedTextureMissing(key))?;

        for upload in rects {
            validate_update_rect(upload, width, height).map_err(RenderError::DeviceObjectInit)?;
            let rect_width = u32::from(upload.rect.w);
            let rect_height = u32::from(upload.rect.h);
            let packed = tightly_pack_rows(
                format,
                rect_width,
                rect_height,
                upload.row_pitch,
                &upload.data,
            )
            .map_err(RenderError::DeviceObjectInit)?;
            let rgba;
            let pixels = match format {
                TextureFormat::RGBA32 => packed,
                TextureFormat::Alpha8 => {
                    rgba = alpha8_to_rgba(&packed, rect_width, rect_height)
                        .map_err(RenderError::DeviceObjectInit)?;
                    Cow::Owned(rgba)
                }
            };
            update_texture(
                gl,
                binding.gl_texture,
                GlTextureUpdate::new(
                    [u32::from(upload.rect.x), u32::from(upload.rect.y)],
                    [rect_width, rect_height],
                    TextureFormat::RGBA32,
                    &pixels,
                ),
            )
            .map_err(RenderError::DeviceObjectInit)?;
        }

        self.texture_map_mut().update_texture(
            binding.texture_id,
            binding.gl_texture,
            width,
            height,
        );
        Ok(binding.texture_id)
    }

    fn destroy_managed_texture(&mut self, gl: &Context, key: SnapshotTextureId) {
        let Some(binding) = self.managed_textures.remove(&key) else {
            return;
        };
        self.texture_map_mut().remove(binding.texture_id);
        if self.owned_textures.contains(&binding.gl_texture) {
            unsafe { gl.delete_texture(binding.gl_texture) };
            self.forget_owned_texture(binding.gl_texture);
        }
    }

    /// Update a renderer-owned legacy texture.
    pub fn update_texture(
        &mut self,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> RenderResult<()> {
        self.ensure_operational()?;
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.update_legacy_texture(&gl, texture_id, width, height, data)
            .map_err(RenderError::DeviceObjectInit)
    }

    /// Update a renderer-owned legacy texture using an externally managed OpenGL context.
    pub fn update_texture_with_context(
        &mut self,
        gl: &Context,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> RenderResult<()> {
        self.ensure_operational()?;
        self.update_legacy_texture(gl, texture_id, width, height, data)
            .map_err(RenderError::DeviceObjectInit)
    }

    fn update_legacy_texture(
        &mut self,
        gl: &Context,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> InitResult<()> {
        if texture_id.is_null() {
            return Err(InitError::NullTextureId);
        }
        let gl_texture = self
            .texture_map()
            .get(texture_id)
            .ok_or(InitError::UnknownTextureId(texture_id))?;
        let format = self
            .texture_map()
            .texture_format(texture_id)
            .unwrap_or(TextureFormat::RGBA32);
        upload_texture_data(gl, gl_texture, width, height, format, data)?;
        self.texture_map_mut()
            .update_texture(texture_id, gl_texture, width, height);
        Ok(())
    }

    /// Register a renderer-owned legacy texture.
    pub fn register_texture(
        &mut self,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> RenderResult<TextureId> {
        self.ensure_operational()?;
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.register_legacy_texture(&gl, width, height, format, data)
            .map_err(RenderError::DeviceObjectInit)
    }

    /// Register a renderer-owned legacy texture using an external OpenGL context.
    pub fn register_texture_with_context(
        &mut self,
        gl: &Context,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> RenderResult<TextureId> {
        self.ensure_operational()?;
        self.register_legacy_texture(gl, width, height, format, data)
            .map_err(RenderError::DeviceObjectInit)
    }

    fn register_legacy_texture(
        &mut self,
        gl: &Context,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> InitResult<TextureId> {
        let gl_texture = match format {
            TextureFormat::RGBA32 => create_texture_from_rgba(gl, width, height, data)?,
            TextureFormat::Alpha8 => create_texture_from_alpha(gl, width, height, data)?,
        };
        self.track_owned_texture(gl_texture);
        let texture_id = match self
            .texture_map_mut()
            .register_texture(gl_texture, width, height, format)
        {
            Ok(texture_id) => texture_id,
            Err(error) => {
                unsafe { gl.delete_texture(gl_texture) };
                self.forget_owned_texture(gl_texture);
                return Err(error);
            }
        };
        Ok(texture_id)
    }

    /// Pixel format recorded for a renderer-owned legacy texture.
    #[must_use]
    pub fn texture_format(&self, texture_id: TextureId) -> Option<TextureFormat> {
        self.texture_map.as_deref()?.texture_format(texture_id)
    }
}

fn bytes_per_pixel(format: TextureFormat) -> usize {
    match format {
        TextureFormat::RGBA32 => 4,
        TextureFormat::Alpha8 => 1,
    }
}

fn tightly_pack_rows<'data>(
    format: TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    data: &'data [u8],
) -> InitResult<Cow<'data, [u8]>> {
    let minimum_pitch = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(bytes_per_pixel(format)))
        .ok_or(InitError::TextureSizeOverflow { format })?;
    if row_pitch < minimum_pitch {
        return Err(InitError::TextureRowPitchTooSmall {
            format,
            minimum: minimum_pitch,
            actual: row_pitch,
        });
    }
    let height = usize::try_from(height).map_err(|_| InitError::TextureSizeOverflow { format })?;
    let packed_len = minimum_pitch
        .checked_mul(height)
        .ok_or(InitError::TextureSizeOverflow { format })?;
    let required_len = if height == 0 {
        0
    } else {
        row_pitch
            .checked_mul(height - 1)
            .and_then(|prefix| prefix.checked_add(minimum_pitch))
            .ok_or(InitError::TextureSizeOverflow { format })?
    };
    if data.len() < required_len {
        return Err(InitError::TextureDataSizeMismatch {
            format,
            expected: required_len,
            actual: data.len(),
        });
    }
    if row_pitch == minimum_pitch {
        return Ok(Cow::Borrowed(&data[..packed_len]));
    }

    let mut packed = Vec::with_capacity(packed_len);
    for row in 0..height {
        let start = row * row_pitch;
        packed.extend_from_slice(&data[start..start + minimum_pitch]);
    }
    Ok(Cow::Owned(packed))
}

fn validate_update_rect(
    upload: &TextureUploadRect,
    texture_width: u32,
    texture_height: u32,
) -> InitResult<()> {
    let x = u32::from(upload.rect.x);
    let y = u32::from(upload.rect.y);
    let width = u32::from(upload.rect.w);
    let height = u32::from(upload.rect.h);
    let in_bounds = x
        .checked_add(width)
        .is_some_and(|right| right <= texture_width)
        && y.checked_add(height)
            .is_some_and(|bottom| bottom <= texture_height);
    if in_bounds {
        Ok(())
    } else {
        Err(InitError::TextureUpdateOutOfBounds {
            x,
            y,
            width,
            height,
            texture_width,
            texture_height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        shaders::Shaders,
        texture::{SimpleTextureMap, TextureMap},
        versions::GlVersion,
    };
    use dear_imgui_rs::render::{SnapshotTextureId, TextureRequestKind};
    use dear_imgui_rs::{
        Context as ImGuiContext, FramePrepareOptions, ManagedTextureId, OwnedTextureData,
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    };

    static LAST_BOUND_TEXTURE: AtomicU32 = AtomicU32::new(0);
    static NEXT_TEXTURE: AtomicU32 = AtomicU32::new(100);
    static DELETED_TEXTURES: AtomicU32 = AtomicU32::new(0);
    static GL_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Default)]
    struct PanicOnceTextureMap {
        inner: SimpleTextureMap,
        panic_on_register: bool,
    }

    impl TextureMap for PanicOnceTextureMap {
        fn get(&self, texture_id: TextureId) -> Option<GlTexture> {
            self.inner.get(texture_id)
        }

        fn set(&mut self, texture_id: TextureId, gl_texture: GlTexture) {
            self.inner.set(texture_id, gl_texture);
        }

        fn remove(&mut self, texture_id: TextureId) -> Option<GlTexture> {
            self.inner.remove(texture_id)
        }

        fn clear(&mut self) {
            self.inner.clear();
        }

        fn register_texture(
            &mut self,
            gl_texture: GlTexture,
            width: u32,
            height: u32,
            format: TextureFormat,
        ) -> InitResult<TextureId> {
            if self.panic_on_register {
                self.panic_on_register = false;
                panic!("injected TextureMap::register_texture panic");
            }
            self.inner
                .register_texture(gl_texture, width, height, format)
        }

        fn update_texture(
            &mut self,
            texture_id: TextureId,
            gl_texture: GlTexture,
            width: u32,
            height: u32,
        ) {
            self.inner
                .update_texture(texture_id, gl_texture, width, height);
        }

        fn texture_format(&self, texture_id: TextureId) -> Option<TextureFormat> {
            self.inner.texture_format(texture_id)
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FakeUploadState {
        active_texture: u32,
        texture_bindings: [u32; 4],
        unpack_alignment: i32,
        unpack_row_length: i32,
        unpack_skip_pixels: i32,
        unpack_skip_rows: i32,
        pixel_unpack_buffer_binding: u32,
    }

    impl FakeUploadState {
        const DEFAULT: Self = Self {
            active_texture: glow::TEXTURE0,
            texture_bindings: [0; 4],
            unpack_alignment: 4,
            unpack_row_length: 0,
            unpack_skip_pixels: 0,
            unpack_skip_rows: 0,
            pixel_unpack_buffer_binding: 0,
        };

        fn active_unit_index(self) -> usize {
            usize::try_from(self.active_texture - glow::TEXTURE0).unwrap()
        }
    }

    static FAKE_UPLOAD_STATE: Mutex<FakeUploadState> = Mutex::new(FakeUploadState::DEFAULT);

    struct PanicRegisterTextureMap;

    impl TextureMap for PanicRegisterTextureMap {
        fn get(&self, _texture_id: TextureId) -> Option<GlTexture> {
            None
        }

        fn set(&mut self, _texture_id: TextureId, _gl_texture: GlTexture) {}

        fn remove(&mut self, _texture_id: TextureId) -> Option<GlTexture> {
            None
        }

        fn clear(&mut self) {}

        fn register_texture(
            &mut self,
            _gl_texture: GlTexture,
            _width: u32,
            _height: u32,
            _format: TextureFormat,
        ) -> InitResult<TextureId> {
            panic!("injected TextureMap::register_texture panic");
        }

        fn update_texture(
            &mut self,
            _texture_id: TextureId,
            _gl_texture: GlTexture,
            _width: u32,
            _height: u32,
        ) {
        }

        fn texture_format(&self, _texture_id: TextureId) -> Option<TextureFormat> {
            None
        }
    }

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
            vbo_handle: None,
            ebo_handle: None,
            owned_textures: Vec::new(),
            samplers: None,
            gl_version: GlVersion {
                major: 3,
                minor: 3,
                is_es: false,
            },
            has_clip_origin_support: false,
            has_separate_polygon_modes: false,
            has_sampler_object_support: true,
            is_destroyed: false,
            gl_context: None,
            context_binding: None,
            backend_user_data: Box::default(),
            renderer_name_ptr: std::ptr::null(),
            renderer_texture_max: [0, 0],
            renderer_state_fault: None,
            synthetic_test_renderer: true,
            texture_map: Some(Box::new(SimpleTextureMap::default())),
            managed_textures: std::collections::HashMap::new(),
            destroyed_managed_textures: std::collections::HashMap::new(),
            renderer_consumer: None,
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn make_protocol_renderer(context: &mut ImGuiContext) -> GlowRenderer {
        let mut renderer = make_test_renderer();
        renderer.renderer_consumer = Some(
            context
                .create_synchronous_renderer_consumer()
                .expect("test renderer consumer should attach"),
        );
        renderer
    }

    fn make_fake_gl() -> glow::Context {
        set_fake_upload_state(FakeUploadState::DEFAULT);

        unsafe extern "system" fn fake_gl_get_string(_name: u32) -> *const u8 {
            c"4.6".as_ptr().cast()
        }
        unsafe extern "system" fn fake_gl_get_string_i(_name: u32, _index: u32) -> *const u8 {
            c"".as_ptr().cast()
        }
        unsafe extern "system" fn fake_gl_get_integer_v(pname: u32, data: *mut i32) {
            if data.is_null() {
                return;
            }
            let state = *FAKE_UPLOAD_STATE.lock().unwrap();
            let value = match pname {
                glow::ACTIVE_TEXTURE => state.active_texture as i32,
                glow::TEXTURE_BINDING_2D => {
                    state.texture_bindings[state.active_unit_index()] as i32
                }
                glow::UNPACK_ALIGNMENT => state.unpack_alignment,
                glow::UNPACK_ROW_LENGTH => state.unpack_row_length,
                glow::UNPACK_SKIP_PIXELS => state.unpack_skip_pixels,
                glow::UNPACK_SKIP_ROWS => state.unpack_skip_rows,
                glow::PIXEL_UNPACK_BUFFER_BINDING => state.pixel_unpack_buffer_binding as i32,
                _ => 0,
            };
            unsafe { *data = value };
        }
        unsafe extern "system" fn fake_gl_active_texture(texture: u32) {
            FAKE_UPLOAD_STATE.lock().unwrap().active_texture = texture;
        }
        unsafe extern "system" fn fake_gl_gen_textures(count: i32, textures: *mut u32) {
            for index in 0..count.max(0) as usize {
                unsafe { *textures.add(index) = NEXT_TEXTURE.fetch_add(1, Ordering::SeqCst) };
            }
        }
        unsafe extern "system" fn fake_gl_delete_textures(count: i32, _textures: *const u32) {
            DELETED_TEXTURES.fetch_add(count.max(0) as u32, Ordering::SeqCst);
        }
        unsafe extern "system" fn fake_gl_bind_texture(_target: u32, texture: u32) {
            let mut state = FAKE_UPLOAD_STATE.lock().unwrap();
            let unit = state.active_unit_index();
            state.texture_bindings[unit] = texture;
            if texture != 0 {
                LAST_BOUND_TEXTURE.store(texture, Ordering::SeqCst);
            }
        }
        unsafe extern "system" fn fake_gl_pixel_store_i(pname: u32, param: i32) {
            let mut state = FAKE_UPLOAD_STATE.lock().unwrap();
            match pname {
                glow::UNPACK_ALIGNMENT => state.unpack_alignment = param,
                glow::UNPACK_ROW_LENGTH => state.unpack_row_length = param,
                glow::UNPACK_SKIP_PIXELS => state.unpack_skip_pixels = param,
                glow::UNPACK_SKIP_ROWS => state.unpack_skip_rows = param,
                _ => {}
            }
        }
        unsafe extern "system" fn fake_gl_bind_buffer(target: u32, buffer: u32) {
            assert_eq!(target, glow::PIXEL_UNPACK_BUFFER);
            FAKE_UPLOAD_STATE
                .lock()
                .unwrap()
                .pixel_unpack_buffer_binding = buffer;
        }
        unsafe extern "system" fn fake_gl_tex_parameter_i(_target: u32, _pname: u32, _param: i32) {}
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
            assert_normalized_upload_state();
        }
        unsafe extern "system" fn fake_gl_tex_sub_image_2d(
            _target: u32,
            _level: i32,
            _x: i32,
            _y: i32,
            _width: i32,
            _height: i32,
            _format: u32,
            _type_: u32,
            _pixels: *const std::ffi::c_void,
        ) {
            assert_normalized_upload_state();
        }

        unsafe {
            glow::Context::from_loader_function(|name| {
                match name {
                    "glGetString" => fake_gl_get_string as *const (),
                    "glGetStringi" => fake_gl_get_string_i as *const (),
                    "glGetIntegerv" => fake_gl_get_integer_v as *const (),
                    "glActiveTexture" => fake_gl_active_texture as *const (),
                    "glGenTextures" => fake_gl_gen_textures as *const (),
                    "glDeleteTextures" => fake_gl_delete_textures as *const (),
                    "glBindTexture" => fake_gl_bind_texture as *const (),
                    "glPixelStorei" => fake_gl_pixel_store_i as *const (),
                    "glBindBuffer" => fake_gl_bind_buffer as *const (),
                    "glTexParameteri" => fake_gl_tex_parameter_i as *const (),
                    "glTexImage2D" => fake_gl_tex_image_2d as *const (),
                    "glTexSubImage2D" => fake_gl_tex_sub_image_2d as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }

    fn assert_normalized_upload_state() {
        let state = *FAKE_UPLOAD_STATE.lock().unwrap();
        assert_eq!(state.active_texture, glow::TEXTURE0);
        assert_ne!(state.texture_bindings[0], 11);
        assert_eq!(state.unpack_alignment, 1);
        assert_eq!(state.unpack_row_length, 0);
        assert_eq!(state.unpack_skip_pixels, 0);
        assert_eq!(state.unpack_skip_rows, 0);
        assert_eq!(state.pixel_unpack_buffer_binding, 0);
    }

    fn set_fake_upload_state(state: FakeUploadState) {
        *FAKE_UPLOAD_STATE.lock().unwrap() = state;
    }

    fn fake_upload_state() -> FakeUploadState {
        *FAKE_UPLOAD_STATE.lock().unwrap()
    }

    fn register_rgba_texture(context: &mut ImGuiContext) -> ManagedTextureId {
        let texture =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 2, 2, &[255; 16]).unwrap();
        context.register_texture(texture)
    }

    fn render_managed_frame<'context>(
        context: &'context mut ImGuiContext,
        renderer: &GlowRenderer,
        texture: Option<ManagedTextureId>,
    ) -> dear_imgui_rs::render::PendingFrame<'context> {
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame();
        frame.ui().text("managed texture protocol");
        if let Some(texture) = texture {
            frame.ui().image(texture, [8.0, 8.0]);
        }
        frame.render(
            renderer
                .renderer_consumer
                .as_ref()
                .expect("test renderer consumer should remain attached"),
        )
    }

    fn user_request(requests: &[TextureRequest], texture: ManagedTextureId) -> &TextureRequest {
        requests
            .iter()
            .find(|request| request.texture() == SnapshotTextureId::User(texture))
            .expect("frame should contain the user texture request")
    }

    fn process_frame_requests(
        renderer: &mut GlowRenderer,
        gl: &glow::Context,
        frame: &dear_imgui_rs::render::PendingFrame<'_>,
    ) -> RenderResult<Vec<TextureFeedback>> {
        let request_epoch = frame.epoch().sequence();
        renderer.process_texture_requests(gl, frame.texture_requests(), request_epoch)
    }

    fn reconcile_with_retries(
        frame: dear_imgui_rs::render::PendingFrame<'_>,
    ) -> dear_imgui_rs::render::ReconciledFrame<'_> {
        let feedback = frame
            .texture_requests()
            .iter()
            .map(TextureRequest::retry)
            .collect::<Vec<_>>();
        frame
            .reconcile_texture_feedback(feedback)
            .expect("retry feedback should reconcile the foreign frame")
    }

    #[test]
    fn row_packing_removes_padding_without_touching_native_texture_state() {
        let packed = tightly_pack_rows(
            TextureFormat::RGBA32,
            1,
            2,
            8,
            &[1, 2, 3, 4, 99, 99, 99, 99, 5, 6, 7, 8],
        )
        .expect("padded rows should be accepted");
        assert_eq!(&*packed, &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn texture_uploads_restore_unit_zero_active_unit_and_unpack_state() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let gl = make_fake_gl();
        let original = FakeUploadState {
            active_texture: glow::TEXTURE0 + 3,
            texture_bindings: [11, 0, 0, 33],
            unpack_alignment: 8,
            unpack_row_length: 7,
            unpack_skip_pixels: 2,
            unpack_skip_rows: 3,
            pixel_unpack_buffer_binding: 91,
        };

        set_fake_upload_state(original);
        let rgba = create_texture_from_rgba(&gl, 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(fake_upload_state(), original);

        set_fake_upload_state(original);
        let alpha = create_texture_from_alpha(&gl, 1, 1, &[255]).unwrap();
        assert_eq!(fake_upload_state(), original);

        set_fake_upload_state(original);
        update_texture(
            &gl,
            rgba,
            GlTextureUpdate::new([0, 0], [1, 1], TextureFormat::RGBA32, &[5, 6, 7, 8]),
        )
        .unwrap();
        assert_eq!(fake_upload_state(), original);

        set_fake_upload_state(original);
        upload_texture_data(&gl, alpha, 1, 1, TextureFormat::RGBA32, &[9, 10, 11, 12]).unwrap();
        assert_eq!(fake_upload_state(), original);

        unsafe {
            gl.delete_texture(rgba);
            gl.delete_texture(alpha);
        }
    }

    #[test]
    fn texture_upload_state_restores_pixel_unpack_buffer_during_unwind() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let gl = make_fake_gl();
        let original = FakeUploadState {
            active_texture: glow::TEXTURE0 + 3,
            texture_bindings: [11, 0, 0, 33],
            unpack_alignment: 8,
            unpack_row_length: 7,
            unpack_skip_pixels: 2,
            unpack_skip_rows: 3,
            pixel_unpack_buffer_binding: 91,
        };
        set_fake_upload_state(original);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = crate::texture::texture_upload_state_guard_for_test(&gl);
            assert_normalized_upload_state();
            panic!("injected upload panic");
        }));

        assert!(panic.is_err());
        assert_eq!(fake_upload_state(), original);
    }

    #[test]
    fn legacy_texture_register_panic_keeps_the_gl_texture_owned_until_teardown() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);
        let gl = make_fake_gl();
        let mut renderer = make_test_renderer();
        renderer.texture_map = Some(Box::new(PanicRegisterTextureMap));

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = renderer.register_texture_with_context(
                &gl,
                1,
                1,
                TextureFormat::RGBA32,
                &[255, 255, 255, 255],
            );
        }));

        assert!(panic.is_err());
        assert_eq!(renderer.owned_textures.len(), 1);
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 0);

        renderer.destroy_gpu_resources_only(&gl).unwrap();
        assert!(renderer.owned_textures.is_empty());
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn managed_requests_create_update_and_destroy_by_snapshot_key() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);

        let frame = render_managed_frame(&mut context, &renderer, Some(texture));
        assert_eq!(
            user_request(frame.texture_requests(), texture).kind(),
            TextureRequestKind::Create
        );
        let feedback = process_frame_requests(&mut renderer, &gl, &frame)
            .expect("create requests should upload");
        let frame = frame
            .reconcile_texture_feedback(feedback)
            .expect("create feedback should reconcile");
        drop(frame);
        let key = SnapshotTextureId::User(texture);
        let created_id = renderer.managed_textures[&key].texture_id;
        context
            .with_texture(texture, |texture| {
                assert_eq!(texture.texture_id(), created_id)
            })
            .unwrap();

        context
            .try_with_texture_mut(texture, |mut texture| texture.replace_pixels(&[7; 16]))
            .unwrap();
        let frame = render_managed_frame(&mut context, &renderer, Some(texture));
        assert_eq!(
            user_request(frame.texture_requests(), texture).kind(),
            TextureRequestKind::Update
        );
        let feedback = process_frame_requests(&mut renderer, &gl, &frame)
            .expect("update requests should upload");
        let frame = frame
            .reconcile_texture_feedback(feedback)
            .expect("update feedback should reconcile");
        drop(frame);
        assert_eq!(renderer.managed_textures[&key].texture_id, created_id);

        context.remove_texture(texture).unwrap();
        let frame = render_managed_frame(&mut context, &renderer, None);
        assert_eq!(
            user_request(frame.texture_requests(), texture).kind(),
            TextureRequestKind::Destroy
        );
        let deleted_before = DELETED_TEXTURES.load(Ordering::SeqCst);
        let feedback = process_frame_requests(&mut renderer, &gl, &frame)
            .expect("destroy requests should retire GPU resources");
        assert!(!renderer.managed_textures.contains_key(&key));
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), deleted_before + 1);
        let destroy_epoch = frame.epoch().sequence();
        let duplicate_feedback = renderer
            .process_texture_requests(
                &gl,
                frame.texture_requests(),
                destroy_epoch.saturating_sub(1),
            )
            .expect("repeated destroy requests should be idempotent");
        assert_eq!(duplicate_feedback.len(), 1);
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), deleted_before + 1);
        assert_eq!(
            renderer.destroyed_managed_textures[&key],
            ManagedTextureTombstone {
                latest_destroy_epoch: destroy_epoch,
                acknowledged_epoch: Some(destroy_epoch),
            }
        );
        let frame = frame
            .reconcile_texture_feedback(feedback)
            .expect("destroy feedback should reconcile");
        renderer.prune_destroyed_managed_textures(frame.completion_progress().watermark());
        assert!(renderer.destroyed_managed_textures.is_empty());
        drop(frame);

        renderer.destroy(&gl, &mut context).unwrap();
        assert!(renderer.destroyed_managed_textures.is_empty());
        assert!(renderer.renderer_consumer.is_none());
    }

    #[test]
    fn texture_map_register_panic_keeps_the_gl_texture_owned_until_teardown() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        renderer.texture_map = Some(Box::new(PanicOnceTextureMap {
            inner: SimpleTextureMap::default(),
            panic_on_register: true,
        }));
        let gl = make_fake_gl();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);

        let frame = render_managed_frame(&mut context, &renderer, Some(texture));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = process_frame_requests(&mut renderer, &gl, &frame);
        }));

        assert!(result.is_err());
        assert_eq!(renderer.owned_textures.len(), 1);
        assert!(renderer.managed_textures.is_empty());
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 0);

        drop(frame);
        renderer.destroy(&gl, &mut context).unwrap();
        assert!(renderer.owned_textures.is_empty());
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unacknowledged_destroy_tombstone_survives_watermarks_and_supersedes_retries() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();

        let atlas_frame = render_managed_frame(&mut context, &renderer, None);
        let atlas_feedback = process_frame_requests(&mut renderer, &gl, &atlas_frame)
            .expect("font atlas should upload before the tombstone scenario");
        let atlas_frame = atlas_frame
            .reconcile_texture_feedback(atlas_feedback)
            .expect("font atlas feedback should reconcile");
        drop(atlas_frame);

        let texture = register_rgba_texture(&mut context);
        let key = SnapshotTextureId::User(texture);
        renderer.seal_destroyed_managed_texture(key, 3);
        renderer.acknowledge_destroyed_managed_texture(key, 3);
        renderer.seal_destroyed_managed_texture(key, 5);
        renderer.seal_destroyed_managed_texture(key, 3);
        assert_eq!(
            renderer.destroyed_managed_textures[&key],
            ManagedTextureTombstone {
                latest_destroy_epoch: 5,
                acknowledged_epoch: None,
            }
        );
        renderer.prune_destroyed_managed_textures(u64::MAX);
        assert!(renderer.destroyed_managed_textures.contains_key(&key));
        let next_texture = NEXT_TEXTURE.load(Ordering::SeqCst);
        let managed_texture_count = renderer.managed_textures.len();

        let frame = render_managed_frame(&mut context, &renderer, Some(texture));
        let feedback = renderer
            .process_texture_requests(&gl, frame.texture_requests(), frame.epoch().sequence())
            .expect("late upload should be superseded");
        assert_eq!(feedback.len(), frame.texture_requests().len());
        assert!(!renderer.managed_textures.contains_key(&key));
        assert_eq!(renderer.managed_textures.len(), managed_texture_count);
        assert_eq!(NEXT_TEXTURE.load(Ordering::SeqCst), next_texture);
        let frame = frame
            .reconcile_texture_feedback(feedback)
            .expect("every request should receive an explicit outcome");
        drop(frame);

        renderer.prune_destroyed_managed_textures(u64::MAX);
        assert!(renderer.destroyed_managed_textures.contains_key(&key));

        let retry = render_managed_frame(&mut context, &renderer, Some(texture));
        let feedback = renderer
            .process_texture_requests(&gl, retry.texture_requests(), retry.epoch().sequence())
            .expect("retry should remain superseded before destroy acknowledgement");
        assert!(!renderer.managed_textures.contains_key(&key));
        assert_eq!(renderer.managed_textures.len(), managed_texture_count);
        assert_eq!(NEXT_TEXTURE.load(Ordering::SeqCst), next_texture);
        let retry = retry
            .reconcile_texture_feedback(feedback)
            .expect("retry feedback should reconcile");
        drop(retry);

        renderer.acknowledge_destroyed_managed_texture(key, 3);
        renderer.prune_destroyed_managed_textures(u64::MAX);
        assert_eq!(
            renderer.destroyed_managed_textures[&key],
            ManagedTextureTombstone {
                latest_destroy_epoch: 5,
                acknowledged_epoch: None,
            }
        );

        renderer.acknowledge_destroyed_managed_texture(key, 5);
        renderer.prune_destroyed_managed_textures(4);
        assert!(renderer.destroyed_managed_textures.contains_key(&key));
        renderer.prune_destroyed_managed_textures(5);
        assert!(renderer.destroyed_managed_textures.is_empty());

        renderer.destroy(&gl, &mut context).unwrap();
        assert!(renderer.destroyed_managed_textures.is_empty());
    }

    #[test]
    fn destroy_tombstones_are_bounded_by_the_completion_watermark() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let context = ImGuiContext::create();
        let mut renderer = make_test_renderer();

        for epoch in 1..=1_024 {
            let key = SnapshotTextureId::FontAtlas {
                context: context.id(),
                stamp: 1,
                generation: epoch,
            };
            renderer.seal_destroyed_managed_texture(key, epoch);
            renderer.acknowledge_destroyed_managed_texture(key, epoch);
        }
        assert_eq!(renderer.destroyed_managed_textures.len(), 1_024);

        renderer.prune_destroyed_managed_textures(512);
        assert_eq!(renderer.destroyed_managed_textures.len(), 512);
        assert!(
            renderer
                .destroyed_managed_textures
                .values()
                .all(|tombstone| tombstone
                    .acknowledged_epoch
                    .is_some_and(|epoch| epoch > 512))
        );

        renderer.prune_destroyed_managed_textures(1_024);
        assert!(renderer.destroyed_managed_textures.is_empty());
    }

    #[test]
    fn abandoned_create_retries_without_allocating_a_second_gl_texture() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();

        let frame = render_managed_frame(&mut context, &renderer, Some(texture));
        let feedback =
            process_frame_requests(&mut renderer, &gl, &frame).expect("first create should upload");
        let key = SnapshotTextureId::User(texture);
        let first_binding = renderer.managed_textures[&key];
        let texture_count = renderer.managed_textures.len();
        drop(feedback);
        drop(frame);

        let retry = render_managed_frame(&mut context, &renderer, Some(texture));
        assert_eq!(
            user_request(retry.texture_requests(), texture).kind(),
            TextureRequestKind::Create
        );
        let feedback = process_frame_requests(&mut renderer, &gl, &retry)
            .expect("abandoned create should retry idempotently");
        assert_eq!(renderer.managed_textures.len(), texture_count);
        assert_eq!(
            renderer.managed_textures[&key].texture_id,
            first_binding.texture_id
        );
        assert_eq!(
            renderer.managed_textures[&key].gl_texture,
            first_binding.gl_texture
        );
        let retry = retry
            .reconcile_texture_feedback(feedback)
            .expect("retry feedback should reconcile");
        drop(retry);

        renderer.destroy(&gl, &mut context).unwrap();
    }

    #[test]
    fn render_paths_reject_frames_and_contexts_from_another_context() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut owner = ImGuiContext::create();
        let mut renderer = make_protocol_renderer(&mut owner);
        let gl = make_fake_gl();
        let owner = owner.suspend_or_panic();

        let mut foreign = ImGuiContext::create();
        foreign.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let foreign_consumer = foreign.create_synchronous_renderer_consumer().unwrap();

        foreign.frame().text("foreign render_context frame");
        let foreign_id = foreign.id();
        let error = renderer
            .render_context(&mut foreign)
            .expect_err("Glow must reject a foreign Context before finalizing its frame");
        assert!(matches!(
            error,
            RenderError::ContextMismatch { expected, actual }
                if expected == renderer.renderer_consumer.as_ref().unwrap().context_id()
                    && actual == foreign_id
        ));
        assert_eq!(
            foreign.frame_lifecycle_state(),
            dear_imgui_rs::FrameLifecycleState::InFrame
        );
        assert!(foreign.end_frame());

        let frame = foreign.begin_frame();
        frame.ui().text("foreign frame");
        let frame = frame.render(&foreign_consumer);

        let error = renderer
            .render_with_context(&gl, frame)
            .expect_err("Glow must reject frames from another Context");
        assert!(matches!(
            error,
            RenderError::ContextMismatch { expected, actual }
                if expected == renderer.renderer_consumer.as_ref().unwrap().context_id()
                    && actual == foreign_id
        ));

        let frame = foreign.begin_frame();
        frame.ui().text("foreign reconciled frame");
        let frame = reconcile_with_retries(frame.render(&foreign_consumer));
        let error = renderer
            .render_reconciled(frame)
            .expect_err("Glow must reject a reconciled frame from another Context");
        assert!(matches!(
            error,
            RenderError::ContextMismatch { expected, actual }
                if expected == renderer.renderer_consumer.as_ref().unwrap().context_id()
                    && actual == foreign_id
        ));

        let frame = foreign.begin_frame();
        frame.ui().text("foreign external-context reconciled frame");
        let frame = reconcile_with_retries(frame.render(&foreign_consumer));
        let error = renderer
            .render_with_context_reconciled(&gl, frame)
            .expect_err("Glow external-context drawing must reject a foreign reconciled frame");
        assert!(matches!(
            error,
            RenderError::ContextMismatch { expected, actual }
                if expected == renderer.renderer_consumer.as_ref().unwrap().context_id()
                    && actual == foreign_id
        ));

        drop(foreign_consumer);
        drop(foreign);
        let mut owner = owner.activate().expect("owner Context should reactivate");
        renderer.destroy(&gl, &mut owner).unwrap();
    }

    #[test]
    fn destroy_releases_active_gpu_bindings_before_the_consumer() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);

        let frame = render_managed_frame(&mut context, &renderer, Some(texture));
        let feedback = process_frame_requests(&mut renderer, &gl, &frame)
            .expect("create requests should upload");
        let frame = frame
            .reconcile_texture_feedback(feedback)
            .expect("create feedback should reconcile");
        drop(frame);

        let owned_texture_count = u32::try_from(renderer.owned_textures.len()).unwrap();
        let renderer_texture_ids = renderer
            .managed_textures
            .values()
            .map(|binding| binding.texture_id)
            .collect::<Vec<_>>();
        assert!(owned_texture_count > 0);
        renderer.destroy(&gl, &mut context).unwrap();

        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), owned_texture_count);
        assert!(renderer.managed_textures.is_empty());
        assert!(
            renderer_texture_ids
                .into_iter()
                .all(|texture_id| renderer.texture_map().get(texture_id).is_none())
        );
        assert!(renderer.renderer_consumer.is_none());
        context
            .with_texture(texture, |texture| assert!(texture.texture_id().is_null()))
            .unwrap();

        let replacement = context
            .create_synchronous_renderer_consumer()
            .expect("destroy should release the consumer generation");
        drop(replacement);
    }

    #[test]
    fn abandoned_pending_frame_does_not_block_teardown() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);

        let uploaded = render_managed_frame(&mut context, &renderer, Some(texture));
        let feedback = process_frame_requests(&mut renderer, &gl, &uploaded).unwrap();
        let uploaded = uploaded.reconcile_texture_feedback(feedback).unwrap();
        drop(uploaded);

        let owned_texture_count = u32::try_from(renderer.owned_textures.len()).unwrap();
        let renderer_texture_ids = renderer
            .managed_textures
            .values()
            .map(|binding| binding.texture_id)
            .collect::<Vec<_>>();
        assert!(owned_texture_count > 0);
        let bound_texture = context
            .with_texture(texture, |texture| texture.texture_id())
            .unwrap();
        assert!(!bound_texture.is_null());

        let pending = render_managed_frame(&mut context, &renderer, Some(texture));
        drop(pending);
        renderer.destroy_device_objects(&gl, &mut context).unwrap();
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), owned_texture_count);
        assert!(renderer.managed_textures.is_empty());
        assert!(
            renderer_texture_ids
                .into_iter()
                .all(|texture_id| renderer.texture_map().get(texture_id).is_none())
        );
        assert!(renderer.renderer_consumer.is_some());
        context
            .with_texture(texture, |texture| assert!(texture.texture_id().is_null()))
            .unwrap();

        renderer.destroy(&gl, &mut context).unwrap();
        assert!(renderer.renderer_consumer.is_none());
    }

    #[test]
    fn legacy_texture_ids_do_not_enter_the_managed_request_map() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        let texture_id = TextureId::new(77);
        let gl_texture = glow::NativeTexture(std::num::NonZeroU32::new(88).unwrap());
        renderer.texture_map_mut().set(texture_id, gl_texture);

        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame();
        frame.ui().get_foreground_draw_list().add_image(
            texture_id,
            [0.0, 0.0],
            [8.0, 8.0],
            [0.0, 0.0],
            [1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );
        let frame = frame.render(
            renderer
                .renderer_consumer
                .as_ref()
                .expect("test renderer consumer should remain attached"),
        );
        assert!(
            frame
                .texture_requests()
                .iter()
                .all(|request| matches!(request.texture(), SnapshotTextureId::FontAtlas { .. }))
        );
        let feedback = process_frame_requests(&mut renderer, &gl, &frame)
            .expect("font atlas requests should upload");
        let frame = frame
            .reconcile_texture_feedback(feedback)
            .expect("font atlas feedback should reconcile");
        drop(frame);
        assert!(
            renderer
                .managed_textures
                .keys()
                .all(|texture| matches!(texture, SnapshotTextureId::FontAtlas { .. }))
        );
        assert_eq!(renderer.texture_map().get(texture_id), Some(gl_texture));
        renderer.destroy(&gl, &mut context).unwrap();
    }

    #[test]
    fn update_texture_with_context_uses_registered_gl_texture() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut renderer = make_test_renderer();
        let texture_id = TextureId::from(42u64);
        let gl_texture = glow::NativeTexture(std::num::NonZeroU32::new(99).unwrap());
        renderer.texture_map_mut().set(texture_id, gl_texture);

        LAST_BOUND_TEXTURE.store(0, Ordering::SeqCst);
        let gl = make_fake_gl();
        renderer
            .update_texture_with_context(&gl, texture_id, 1, 1, &[1, 2, 3, 4])
            .expect("update should use the registered GL texture");

        assert_eq!(LAST_BOUND_TEXTURE.load(Ordering::SeqCst), 99);
    }
}
