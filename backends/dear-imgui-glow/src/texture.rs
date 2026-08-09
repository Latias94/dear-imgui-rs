//! Texture management for Dear ImGui

use crate::{GlBuffer, GlTexture, GlVersion, InitError, InitResult};
use dear_imgui_rs::{TextureFormat, TextureId};
use glow::{Context, HasContext};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_texture_id() -> InitResult<TextureId> {
    let id = NEXT_TEXTURE_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| InitError::TextureIdExhausted)?;
    Ok(TextureId::new(id))
}

/// Opaque handle for a texture allocated and owned by [`crate::GlowRenderer`].
///
/// Convert it to a Dear ImGui [`TextureId`] with [`Self::texture_id`] when building image widgets.
/// The handle cannot be forged from a raw ID, so update and removal operations cannot target a
/// managed or application-owned texture accidentally.
///
/// ```compile_fail
/// use dear_imgui_glow::RendererTextureId;
/// use dear_imgui_rs::TextureId;
///
/// let _: RendererTextureId = TextureId::new(1).into();
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct RendererTextureId(TextureId);

impl RendererTextureId {
    pub(crate) const fn new(texture_id: TextureId) -> Self {
        Self(texture_id)
    }

    /// Returns the Dear ImGui texture identifier used by image widgets and draw-list commands.
    #[must_use]
    pub const fn texture_id(self) -> TextureId {
        self.0
    }
}

impl From<RendererTextureId> for TextureId {
    fn from(texture: RendererTextureId) -> Self {
        texture.texture_id()
    }
}

/// Opaque handle for an application-owned OpenGL texture registered with a renderer.
///
/// Registration does not transfer ownership of the OpenGL object. The handle can be converted to
/// a Dear ImGui [`TextureId`] with [`Self::texture_id`], but cannot be forged from a raw ID or used
/// with renderer-owned texture operations.
///
/// ```compile_fail
/// use dear_imgui_glow::ExternalTextureId;
/// use dear_imgui_rs::TextureId;
///
/// let _: ExternalTextureId = TextureId::new(1).into();
/// ```
///
/// ```compile_fail
/// use dear_imgui_glow::{GlowRenderer, RendererTextureId};
///
/// fn unregister_through_wrong_owner(
///     renderer: &mut GlowRenderer,
///     texture: RendererTextureId,
/// ) {
///     renderer.unregister_external_texture(texture).unwrap();
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct ExternalTextureId(TextureId);

impl ExternalTextureId {
    pub(crate) const fn new(texture_id: TextureId) -> Self {
        Self(texture_id)
    }

    /// Returns the Dear ImGui texture identifier used by image widgets and draw-list commands.
    #[must_use]
    pub const fn texture_id(self) -> TextureId {
        self.0
    }
}

impl From<ExternalTextureId> for TextureId {
    fn from(texture: ExternalTextureId) -> Self {
        texture.texture_id()
    }
}

struct TextureUploadStateGuard<'a> {
    gl: &'a Context,
    active_texture: u32,
    texture_2d_binding_unit_0: Option<GlTexture>,
    unpack_alignment: i32,
    unpack_row_state: Option<[i32; 3]>,
    pixel_unpack_buffer_binding: Option<GlBuffer>,
}

impl<'a> TextureUploadStateGuard<'a> {
    fn enter(gl: &'a Context) -> Self {
        unsafe {
            let active_texture = u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE))
                .ok()
                .unwrap_or(glow::TEXTURE0);
            gl.active_texture(glow::TEXTURE0);
            let texture_2d_binding_unit_0 = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
            let unpack_alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
            let pixel_unpack_buffer_binding =
                gl.get_parameter_buffer(glow::PIXEL_UNPACK_BUFFER_BINDING);
            let gl_version = GlVersion::read(gl);
            let unpack_row_state = (!gl_version.is_es || gl_version.major >= 3).then(|| {
                [
                    gl.get_parameter_i32(glow::UNPACK_ROW_LENGTH),
                    gl.get_parameter_i32(glow::UNPACK_SKIP_PIXELS),
                    gl.get_parameter_i32(glow::UNPACK_SKIP_ROWS),
                ]
            });

            if unpack_row_state.is_some() {
                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
                gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
                gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
            }
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

            Self {
                gl,
                active_texture,
                texture_2d_binding_unit_0,
                unpack_alignment,
                unpack_row_state,
                pixel_unpack_buffer_binding,
            }
        }
    }
}

impl Drop for TextureUploadStateGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            if let Some([row_length, skip_pixels, skip_rows]) = self.unpack_row_state {
                self.gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, row_length);
                self.gl
                    .pixel_store_i32(glow::UNPACK_SKIP_PIXELS, skip_pixels);
                self.gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, skip_rows);
            }
            self.gl
                .pixel_store_i32(glow::UNPACK_ALIGNMENT, self.unpack_alignment);
            self.gl
                .bind_buffer(glow::PIXEL_UNPACK_BUFFER, self.pixel_unpack_buffer_binding);
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, self.texture_2d_binding_unit_0);
            self.gl.active_texture(self.active_texture);
        }
    }
}

#[cfg(test)]
pub(crate) fn texture_upload_state_guard_for_test(gl: &Context) -> impl Drop + '_ {
    TextureUploadStateGuard::enter(gl)
}

struct PendingTexture<'a> {
    gl: &'a Context,
    texture: Option<GlTexture>,
}

impl<'a> PendingTexture<'a> {
    fn create(gl: &'a Context) -> InitResult<Self> {
        let texture = unsafe { gl.create_texture() }.map_err(InitError::CreateTexture)?;
        Ok(Self {
            gl,
            texture: Some(texture),
        })
    }

    fn handle(&self) -> GlTexture {
        self.texture.expect("pending texture must own a handle")
    }

    fn commit(mut self) -> GlTexture {
        self.texture
            .take()
            .expect("pending texture must own a handle")
    }
}

impl Drop for PendingTexture<'_> {
    fn drop(&mut self) {
        if let Some(texture) = self.texture.take() {
            unsafe { self.gl.delete_texture(texture) };
        }
    }
}

pub(crate) fn gl_texture_size_i32(dimension: &'static str, value: u32) -> InitResult<i32> {
    i32::try_from(value).map_err(|_| InitError::TextureDimensionOutOfRange { dimension, value })
}

pub(crate) fn checked_gl_texture_size(width: u32, height: u32) -> InitResult<(i32, i32)> {
    Ok((
        gl_texture_size_i32("width", width)?,
        gl_texture_size_i32("height", height)?,
    ))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RendererTexture {
    pub(crate) gl_texture: GlTexture,
    pub(crate) format: TextureFormat,
}

#[derive(Clone, Copy, Debug)]
enum RegisteredTexture {
    Managed(GlTexture),
    Renderer(RendererTexture),
    External(GlTexture),
}

impl RegisteredTexture {
    fn gl_texture(self) -> GlTexture {
        match self {
            Self::Managed(texture) | Self::External(texture) => texture,
            Self::Renderer(texture) => texture.gl_texture,
        }
    }
}

/// Renderer-private registry for globally unique Dear ImGui texture IDs.
#[derive(Default)]
pub(crate) struct TextureRegistry {
    textures: HashMap<TextureId, RegisteredTexture>,
}

impl TextureRegistry {
    pub(crate) fn get(&self, texture_id: TextureId) -> Option<GlTexture> {
        self.textures
            .get(&texture_id)
            .copied()
            .map(RegisteredTexture::gl_texture)
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, texture_id: TextureId) -> bool {
        self.textures.contains_key(&texture_id)
    }

    pub(crate) fn register_managed(&mut self, gl_texture: GlTexture) -> InitResult<TextureId> {
        let texture_id = allocate_texture_id()?;
        self.insert(texture_id, RegisteredTexture::Managed(gl_texture));
        Ok(texture_id)
    }

    pub(crate) fn register_renderer(
        &mut self,
        gl_texture: GlTexture,
        format: TextureFormat,
    ) -> InitResult<RendererTextureId> {
        let texture_id = allocate_texture_id()?;
        self.insert(
            texture_id,
            RegisteredTexture::Renderer(RendererTexture { gl_texture, format }),
        );
        Ok(RendererTextureId::new(texture_id))
    }

    pub(crate) fn register_external(
        &mut self,
        gl_texture: GlTexture,
    ) -> InitResult<ExternalTextureId> {
        let texture_id = allocate_texture_id()?;
        self.insert(texture_id, RegisteredTexture::External(gl_texture));
        Ok(ExternalTextureId::new(texture_id))
    }

    fn insert(&mut self, texture_id: TextureId, texture: RegisteredTexture) {
        let previous = self.textures.insert(texture_id, texture);
        debug_assert!(previous.is_none(), "process-unique TextureId was reused");
    }

    pub(crate) fn renderer(&self, texture: RendererTextureId) -> Option<RendererTexture> {
        match self.textures.get(&texture.texture_id())? {
            RegisteredTexture::Renderer(texture) => Some(*texture),
            RegisteredTexture::Managed(_) | RegisteredTexture::External(_) => None,
        }
    }

    pub(crate) fn external(&self, texture: ExternalTextureId) -> Option<GlTexture> {
        match self.textures.get(&texture.texture_id())? {
            RegisteredTexture::External(texture) => Some(*texture),
            RegisteredTexture::Managed(_) | RegisteredTexture::Renderer(_) => None,
        }
    }

    pub(crate) fn update_external(
        &mut self,
        texture: ExternalTextureId,
        gl_texture: GlTexture,
    ) -> bool {
        let Some(registered) = self.textures.get_mut(&texture.texture_id()) else {
            return false;
        };
        match registered {
            RegisteredTexture::External(registered) => {
                *registered = gl_texture;
                true
            }
            RegisteredTexture::Managed(_) | RegisteredTexture::Renderer(_) => false,
        }
    }

    pub(crate) fn remove_renderer(&mut self, texture: RendererTextureId) -> Option<GlTexture> {
        if !matches!(
            self.textures.get(&texture.texture_id()),
            Some(RegisteredTexture::Renderer(_))
        ) {
            return None;
        }
        match self.textures.remove(&texture.texture_id()) {
            Some(RegisteredTexture::Renderer(texture)) => Some(texture.gl_texture),
            _ => unreachable!("validated renderer texture changed before removal"),
        }
    }

    pub(crate) fn remove_external(&mut self, texture: ExternalTextureId) -> Option<GlTexture> {
        if !matches!(
            self.textures.get(&texture.texture_id()),
            Some(RegisteredTexture::External(_))
        ) {
            return None;
        }
        match self.textures.remove(&texture.texture_id()) {
            Some(RegisteredTexture::External(texture)) => Some(texture),
            _ => unreachable!("validated external texture changed before removal"),
        }
    }

    pub(crate) fn remove_managed(&mut self, texture_id: TextureId) -> Option<GlTexture> {
        if !matches!(
            self.textures.get(&texture_id),
            Some(RegisteredTexture::Managed(_))
        ) {
            return None;
        }
        match self.textures.remove(&texture_id) {
            Some(RegisteredTexture::Managed(texture)) => Some(texture),
            _ => unreachable!("validated managed texture changed before removal"),
        }
    }

    pub(crate) fn aliases_renderer_owned(&self, gl_texture: GlTexture) -> bool {
        self.textures.values().any(|texture| {
            !matches!(texture, RegisteredTexture::External(_)) && texture.gl_texture() == gl_texture
        })
    }

    pub(crate) fn take_renderer_owned(&mut self) -> Vec<GlTexture> {
        let mut owned = Vec::new();
        self.textures.retain(|_, texture| match texture {
            RegisteredTexture::External(_) => true,
            RegisteredTexture::Managed(_) | RegisteredTexture::Renderer(_) => {
                owned.push(texture.gl_texture());
                false
            }
        });
        owned
    }

    pub(crate) fn clear(&mut self) {
        self.textures.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.textures.len()
    }
}

/// Create a texture from raw RGBA data
pub fn create_texture_from_rgba(
    gl: &Context,
    width: u32,
    height: u32,
    data: &[u8],
) -> InitResult<GlTexture> {
    let (width_i32, height_i32) = checked_gl_texture_size(width, height)?;
    let data = validated_rgba_upload_data(TextureFormat::RGBA32, width, height, data)?;
    let _state = TextureUploadStateGuard::enter(gl);
    let texture = PendingTexture::create(gl)?;
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture.handle()));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width_i32,
            height_i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(data.as_ref())),
        );

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
    }
    Ok(texture.commit())
}

/// Create a texture from raw alpha data (single channel)
pub fn create_texture_from_alpha(
    gl: &Context,
    width: u32,
    height: u32,
    data: &[u8],
) -> InitResult<GlTexture> {
    let (width_i32, height_i32) = checked_gl_texture_size(width, height)?;
    let rgba_data = alpha8_to_rgba(data, width, height)?;
    let _state = TextureUploadStateGuard::enter(gl);
    let texture = PendingTexture::create(gl)?;

    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture.handle()));

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
    }
    Ok(texture.commit())
}

/// A validated sub-image upload for an existing OpenGL texture.
#[derive(Clone, Copy, Debug)]
pub struct GlTextureUpdate<'a> {
    /// Pixel offset within the destination texture.
    pub offset: [u32; 2],
    /// Width and height of the uploaded region.
    pub size: [u32; 2],
    /// Source pixel format.
    pub format: TextureFormat,
    /// Tightly packed source pixels.
    pub data: &'a [u8],
}

impl<'a> GlTextureUpdate<'a> {
    /// Describe one tightly packed texture sub-image upload.
    pub fn new(offset: [u32; 2], size: [u32; 2], format: TextureFormat, data: &'a [u8]) -> Self {
        Self {
            offset,
            size,
            format,
            data,
        }
    }
}

/// Update a texture with validated, tightly packed pixel data.
pub fn update_texture(
    gl: &Context,
    texture: GlTexture,
    update: GlTextureUpdate<'_>,
) -> InitResult<()> {
    let [x, y] = update.offset;
    let [width, height] = update.size;
    let x = gl_texture_size_i32("x", x)?;
    let y = gl_texture_size_i32("y", y)?;
    let (width, height) = checked_gl_texture_size(width, height)?;
    let data =
        validated_rgba_upload_data(update.format, update.size[0], update.size[1], update.data)?;
    let _state = TextureUploadStateGuard::enter(gl);
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_sub_image_2d(
            glow::TEXTURE_2D,
            0,
            x,
            y,
            width,
            height,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(data.as_ref())),
        );
    }
    Ok(())
}

fn validated_rgba_upload_data<'a>(
    format: TextureFormat,
    width: u32,
    height: u32,
    data: &'a [u8],
) -> InitResult<Cow<'a, [u8]>> {
    match format {
        TextureFormat::RGBA32 => {
            let expected_len = (width as usize)
                .checked_mul(height as usize)
                .and_then(|len| len.checked_mul(4))
                .ok_or(InitError::TextureSizeOverflow {
                    format: TextureFormat::RGBA32,
                })?;
            if data.len() != expected_len {
                return Err(InitError::TextureDataSizeMismatch {
                    format: TextureFormat::RGBA32,
                    expected: expected_len,
                    actual: data.len(),
                });
            }
            Ok(Cow::Borrowed(data))
        }
        TextureFormat::Alpha8 => Ok(Cow::Owned(alpha8_to_rgba(data, width, height)?)),
    }
}

pub(crate) fn alpha8_to_rgba(data: &[u8], width: u32, height: u32) -> InitResult<Vec<u8>> {
    let expected_len =
        (width as usize)
            .checked_mul(height as usize)
            .ok_or(InitError::TextureSizeOverflow {
                format: TextureFormat::Alpha8,
            })?;
    if data.len() != expected_len {
        return Err(InitError::TextureDataSizeMismatch {
            format: TextureFormat::Alpha8,
            expected: expected_len,
            actual: data.len(),
        });
    }

    let mut rgba = Vec::with_capacity(expected_len * 4);
    for &alpha in data {
        rgba.extend_from_slice(&[255, 255, 255, alpha]);
    }

    Ok(rgba)
}

pub(crate) fn upload_texture_data(
    gl: &Context,
    texture: GlTexture,
    width: u32,
    height: u32,
    format: TextureFormat,
    data: &[u8],
) -> InitResult<()> {
    let (width_i32, height_i32) = checked_gl_texture_size(width, height)?;
    let data = validated_rgba_upload_data(format, width, height, data)?;
    let _state = TextureUploadStateGuard::enter(gl);

    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width_i32,
            height_i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(data.as_ref())),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha8_to_rgba_expands_white_rgb_and_alpha() {
        let rgba = alpha8_to_rgba(&[0, 64, 255], 3, 1).expect("valid alpha data");

        assert_eq!(
            rgba,
            vec![
                255, 255, 255, 0, //
                255, 255, 255, 64, //
                255, 255, 255, 255,
            ]
        );
    }

    #[test]
    fn alpha8_to_rgba_rejects_size_mismatch() {
        assert!(alpha8_to_rgba(&[0, 1], 3, 1).is_err());
    }

    #[test]
    fn rgba_upload_validation_rejects_short_source_data() {
        assert!(matches!(
            validated_rgba_upload_data(TextureFormat::RGBA32, 2, 2, &[0; 15]),
            Err(InitError::TextureDataSizeMismatch {
                format: TextureFormat::RGBA32,
                expected: 16,
                actual: 15,
            })
        ));
    }

    #[test]
    fn rgba_upload_validation_rejects_long_source_data() {
        assert!(matches!(
            validated_rgba_upload_data(TextureFormat::RGBA32, 2, 2, &[0; 17]),
            Err(InitError::TextureDataSizeMismatch {
                format: TextureFormat::RGBA32,
                expected: 16,
                actual: 17,
            })
        ));
    }

    #[test]
    fn rgba_upload_validation_rejects_size_overflow() {
        assert!(matches!(
            validated_rgba_upload_data(TextureFormat::RGBA32, u32::MAX, u32::MAX, &[]),
            Err(InitError::TextureSizeOverflow {
                format: TextureFormat::RGBA32,
            })
        ));
    }
}
