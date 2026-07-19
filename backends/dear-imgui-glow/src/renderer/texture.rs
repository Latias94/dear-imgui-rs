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
    ) -> RenderResult<Vec<TextureFeedback>> {
        requests
            .iter()
            .map(|request| self.process_texture_request(gl, request))
            .collect()
    }

    fn process_texture_request(
        &mut self,
        gl: &Context,
        request: &TextureRequest,
    ) -> RenderResult<TextureFeedback> {
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
                self.destroy_managed_texture(gl, request.texture());
                Ok(request.destroyed()?)
            }
        }
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

        let texture_id = match self.texture_map_mut().register_texture(
            gl_texture,
            create.width,
            create.height,
            create.format,
        ) {
            Ok(texture_id) => texture_id,
            Err(error) => {
                unsafe { gl.delete_texture(gl_texture) };
                return Err(RenderError::DeviceObjectInit(error));
            }
        };
        self.track_owned_texture(gl_texture);
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
        let binding = self.managed_textures.get(&key).copied().ok_or_else(|| {
            RenderError::InvalidTexture(format!(
                "managed texture {key:?} received an update before creation"
            ))
        })?;

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
    ) -> InitResult<()> {
        let gl = self.gl_context.clone().ok_or(InitError::MissingGlContext)?;
        self.update_texture_with_context(&gl, texture_id, width, height, data)
    }

    /// Update a renderer-owned legacy texture using an externally managed OpenGL context.
    pub fn update_texture_with_context(
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
    ) -> InitResult<TextureId> {
        let gl = self.gl_context.clone().ok_or(InitError::MissingGlContext)?;
        self.register_texture_with_context(&gl, width, height, format, data)
    }

    /// Register a renderer-owned legacy texture using an external OpenGL context.
    pub fn register_texture_with_context(
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
        let texture_id = match self
            .texture_map_mut()
            .register_texture(gl_texture, width, height, format)
        {
            Ok(texture_id) => texture_id,
            Err(error) => {
                unsafe { gl.delete_texture(gl_texture) };
                return Err(error);
            }
        };
        self.track_owned_texture(gl_texture);
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
        shaders::Shaders, state::GlStateBackup, texture::SimpleTextureMap, versions::GlVersion,
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
            owned_textures: Vec::new(),
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
            managed_textures: std::collections::HashMap::new(),
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
                .create_renderer_consumer()
                .expect("test renderer consumer should attach"),
        );
        renderer
    }

    fn make_fake_gl() -> glow::Context {
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
            let value = match pname {
                glow::ACTIVE_TEXTURE => glow::TEXTURE0 as i32,
                glow::TEXTURE_BINDING_2D => 0,
                glow::UNPACK_ALIGNMENT => 4,
                _ => 0,
            };
            unsafe { *data = value };
        }
        unsafe extern "system" fn fake_gl_active_texture(_texture: u32) {}
        unsafe extern "system" fn fake_gl_gen_textures(count: i32, textures: *mut u32) {
            for index in 0..count.max(0) as usize {
                unsafe { *textures.add(index) = NEXT_TEXTURE.fetch_add(1, Ordering::SeqCst) };
            }
        }
        unsafe extern "system" fn fake_gl_delete_textures(count: i32, _textures: *const u32) {
            DELETED_TEXTURES.fetch_add(count.max(0) as u32, Ordering::SeqCst);
        }
        unsafe extern "system" fn fake_gl_bind_texture(_target: u32, texture: u32) {
            if texture != 0 {
                LAST_BOUND_TEXTURE.store(texture, Ordering::SeqCst);
            }
        }
        unsafe extern "system" fn fake_gl_pixel_store_i(_pname: u32, _param: i32) {}
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
                    "glTexParameteri" => fake_gl_tex_parameter_i as *const (),
                    "glTexImage2D" => fake_gl_tex_image_2d as *const (),
                    "glTexSubImage2D" => fake_gl_tex_sub_image_2d as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }

    fn register_rgba_texture(context: &mut ImGuiContext) -> ManagedTextureId {
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 2, 2);
        texture.set_data(&[255; 16]);
        context.register_texture(texture)
    }

    fn render_managed_frame(
        context: &mut ImGuiContext,
        texture: Option<ManagedTextureId>,
    ) -> dear_imgui_rs::render::RenderedFrame<'_> {
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame();
        frame.ui().text("managed texture protocol");
        if let Some(texture) = texture {
            frame.ui().image(texture, [8.0, 8.0]);
        }
        frame.render()
    }

    fn user_request(requests: &[TextureRequest], texture: ManagedTextureId) -> &TextureRequest {
        requests
            .iter()
            .find(|request| request.texture() == SnapshotTextureId::User(texture))
            .expect("frame should contain the user texture request")
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
    fn managed_requests_create_update_and_destroy_by_snapshot_key() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        assert!(context.font_atlas().build());
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);

        let mut frame = render_managed_frame(&mut context, Some(texture));
        assert_eq!(
            user_request(frame.texture_requests(), texture).kind(),
            TextureRequestKind::Create
        );
        let feedback = renderer
            .process_texture_requests(&gl, frame.texture_requests())
            .expect("create requests should upload");
        frame
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
            .with_texture_mut(texture, |mut texture| texture.set_data(&[7; 16]))
            .unwrap();
        let mut frame = render_managed_frame(&mut context, Some(texture));
        assert_eq!(
            user_request(frame.texture_requests(), texture).kind(),
            TextureRequestKind::Update
        );
        let feedback = renderer
            .process_texture_requests(&gl, frame.texture_requests())
            .expect("update requests should upload");
        frame
            .reconcile_texture_feedback(feedback)
            .expect("update feedback should reconcile");
        drop(frame);
        assert_eq!(renderer.managed_textures[&key].texture_id, created_id);

        context.remove_texture(texture).unwrap();
        let mut frame = render_managed_frame(&mut context, None);
        assert_eq!(
            user_request(frame.texture_requests(), texture).kind(),
            TextureRequestKind::Destroy
        );
        let deleted_before = DELETED_TEXTURES.load(Ordering::SeqCst);
        let feedback = renderer
            .process_texture_requests(&gl, frame.texture_requests())
            .expect("destroy requests should retire GPU resources");
        assert!(!renderer.managed_textures.contains_key(&key));
        assert_eq!(DELETED_TEXTURES.load(Ordering::SeqCst), deleted_before + 1);
        frame
            .reconcile_texture_feedback(feedback)
            .expect("destroy feedback should reconcile");
        drop(frame);

        renderer.destroy(&gl, &mut context).unwrap();
        assert!(renderer.renderer_consumer.is_none());
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES)
        );
    }

    #[test]
    fn abandoned_create_retries_without_allocating_a_second_gl_texture() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        context.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        assert!(context.font_atlas().build());
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();

        let frame = render_managed_frame(&mut context, Some(texture));
        let feedback = renderer
            .process_texture_requests(&gl, frame.texture_requests())
            .expect("first create should upload");
        let key = SnapshotTextureId::User(texture);
        let first_binding = renderer.managed_textures[&key];
        let texture_count = renderer.managed_textures.len();
        drop(feedback);
        drop(frame);

        let mut retry = render_managed_frame(&mut context, Some(texture));
        assert_eq!(
            user_request(retry.texture_requests(), texture).kind(),
            TextureRequestKind::Create
        );
        let feedback = renderer
            .process_texture_requests(&gl, retry.texture_requests())
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
        retry
            .reconcile_texture_feedback(feedback)
            .expect("retry feedback should reconcile");
        drop(retry);

        renderer.destroy(&gl, &mut context).unwrap();
    }

    #[test]
    fn render_rejects_a_frame_without_the_renderer_epoch() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut context = ImGuiContext::create();
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        assert!(context.font_atlas().build());

        context.prepare_frame(FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0));
        let frame = context.begin_frame();
        frame.ui().text("legacy renderer frame");
        let frame = frame.render();
        assert!(frame.epoch().is_none());

        let error = renderer
            .render_with_context(&gl, frame)
            .expect_err("Glow must reject frames that bypass its renderer consumer");
        assert!(matches!(error, RenderError::MissingRendererEpoch));

        renderer.destroy(&gl, &mut context).unwrap();
    }

    #[test]
    fn render_rejects_a_frame_from_another_context() {
        let _guard = GL_TEST_LOCK.lock().unwrap();
        let mut owner = ImGuiContext::create();
        let mut renderer = make_protocol_renderer(&mut owner);
        let gl = make_fake_gl();
        let owner = owner.suspend();

        let mut foreign = ImGuiContext::create();
        assert!(foreign.font_atlas().build());
        foreign.prepare_frame(FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0));
        let frame = foreign.begin_frame();
        frame.ui().text("foreign frame");
        let frame = frame.render();
        let foreign_id = frame.context_id();

        let error = renderer
            .render_with_context(&gl, frame)
            .expect_err("Glow must reject frames from another Context");
        assert!(matches!(
            error,
            RenderError::ContextMismatch { expected, actual }
                if expected == renderer.renderer_consumer.as_ref().unwrap().context_id()
                    && actual == foreign_id
        ));

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
        assert!(context.font_atlas().build());
        let texture = register_rgba_texture(&mut context);
        let mut renderer = make_protocol_renderer(&mut context);
        let gl = make_fake_gl();
        DELETED_TEXTURES.store(0, Ordering::SeqCst);

        let mut frame = render_managed_frame(&mut context, Some(texture));
        let feedback = renderer
            .process_texture_requests(&gl, frame.texture_requests())
            .expect("create requests should upload");
        frame
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
            .create_renderer_consumer()
            .expect("destroy should release the consumer generation");
        drop(replacement);
    }

    #[test]
    fn legacy_texture_ids_do_not_enter_the_managed_request_map() {
        let mut context = ImGuiContext::create();
        assert!(context.font_atlas().build());
        let mut renderer = make_protocol_renderer(&mut context);
        let texture_id = TextureId::new(77);
        let gl_texture = glow::NativeTexture(std::num::NonZeroU32::new(88).unwrap());
        renderer.texture_map_mut().set(texture_id, gl_texture);

        context.prepare_frame(FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0));
        let frame = context.begin_frame();
        frame.ui().get_foreground_draw_list().add_image(
            texture_id,
            [0.0, 0.0],
            [8.0, 8.0],
            [0.0, 0.0],
            [1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );
        let mut frame = frame.render();
        assert!(frame.texture_requests().is_empty());
        frame
            .reconcile_texture_feedback(std::iter::empty())
            .expect("legacy frames reconcile without managed feedback");
        assert_eq!(renderer.texture_map().get(texture_id), Some(gl_texture));
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
