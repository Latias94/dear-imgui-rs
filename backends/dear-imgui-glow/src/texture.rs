//! Texture management for Dear ImGui

use crate::{GlTexture, GlVersion, InitError, InitResult};
use dear_imgui_rs::{TextureFormat, TextureId};
use glow::{Context, HasContext};
use std::borrow::Cow;
use std::collections::HashMap;

struct TextureUploadStateGuard<'a> {
    gl: &'a Context,
    active_texture: u32,
    texture_2d_binding_unit_0: Option<GlTexture>,
    unpack_alignment: i32,
    unpack_row_state: Option<[i32; 3]>,
}

impl<'a> TextureUploadStateGuard<'a> {
    fn enter(gl: &'a Context) -> Self {
        unsafe {
            let active_texture = u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE))
                .ok()
                .unwrap_or(glow::TEXTURE0);
            gl.active_texture(glow::TEXTURE0);
            let texture_2d_binding_unit_0 =
                u32::try_from(gl.get_parameter_i32(glow::TEXTURE_BINDING_2D))
                    .ok()
                    .and_then(std::num::NonZeroU32::new)
                    .map(glow::NativeTexture);
            let unpack_alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
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
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

            Self {
                gl,
                active_texture,
                texture_2d_binding_unit_0,
                unpack_alignment,
                unpack_row_state,
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
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, self.texture_2d_binding_unit_0);
            self.gl.active_texture(self.active_texture);
        }
    }
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

/// Rust-owned mapping between opaque Dear ImGui texture IDs and OpenGL resources.
///
/// Managed texture request identity is tracked separately by [`crate::GlowRenderer`] using
/// pointer-free [`dear_imgui_rs::render::SnapshotTextureId`] keys. Implementations of this trait
/// never receive native `ImTextureData` pointers or renderer feedback state.
pub trait TextureMap {
    /// Get the OpenGL texture for a Dear ImGui texture ID
    fn get(&self, texture_id: TextureId) -> Option<GlTexture>;

    /// Set the OpenGL texture for a Dear ImGui texture ID
    fn set(&mut self, texture_id: TextureId, gl_texture: GlTexture);

    /// Remove a texture mapping
    fn remove(&mut self, texture_id: TextureId) -> Option<GlTexture>;

    /// Clear all texture mappings
    fn clear(&mut self);

    /// Register a texture with Dear ImGui's texture management system
    fn register_texture(
        &mut self,
        gl_texture: GlTexture,
        _width: u32,
        _height: u32,
        format: TextureFormat,
    ) -> InitResult<TextureId>;

    /// Update a texture in Dear ImGui's texture management system
    fn update_texture(
        &mut self,
        texture_id: TextureId,
        gl_texture: GlTexture,
        width: u32,
        height: u32,
    );

    /// Pixel format recorded for a renderer-owned texture.
    fn texture_format(&self, texture_id: TextureId) -> Option<TextureFormat>;
}

/// Simple texture map implementation using a HashMap with modern texture management
#[derive(Default)]
pub struct SimpleTextureMap {
    textures: HashMap<TextureId, GlTexture>,
    formats: HashMap<TextureId, TextureFormat>,
    next_id: u64,
}

impl TextureMap for SimpleTextureMap {
    fn get(&self, texture_id: TextureId) -> Option<GlTexture> {
        self.textures.get(&texture_id).copied()
    }

    fn set(&mut self, texture_id: TextureId, gl_texture: GlTexture) {
        self.textures.insert(texture_id, gl_texture);
    }

    fn remove(&mut self, texture_id: TextureId) -> Option<GlTexture> {
        let gl_texture = self.textures.remove(&texture_id);
        self.formats.remove(&texture_id);
        gl_texture
    }

    fn clear(&mut self) {
        self.textures.clear();
        self.formats.clear();
    }

    fn register_texture(
        &mut self,
        gl_texture: GlTexture,
        _width: u32,
        _height: u32,
        format: TextureFormat,
    ) -> InitResult<TextureId> {
        let texture_id = loop {
            self.next_id = self
                .next_id
                .checked_add(1)
                .ok_or(InitError::TextureIdExhausted)?;
            let candidate = TextureId::new(self.next_id);
            if !candidate.is_null() && !self.textures.contains_key(&candidate) {
                break candidate;
            }
        };

        self.textures.insert(texture_id, gl_texture);
        self.formats.insert(texture_id, format);

        Ok(texture_id)
    }

    fn update_texture(
        &mut self,
        texture_id: TextureId,
        gl_texture: GlTexture,
        _width: u32,
        _height: u32,
    ) {
        self.textures.insert(texture_id, gl_texture);
    }

    fn texture_format(&self, texture_id: TextureId) -> Option<TextureFormat> {
        self.formats.get(&texture_id).copied()
    }
}

impl SimpleTextureMap {
    /// Create a new empty texture map
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            formats: HashMap::new(),
            next_id: 0,
        }
    }

    /// Get the number of textures in the map
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Check if the texture map is empty
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// Iterate over all texture mappings
    pub fn iter(&self) -> impl Iterator<Item = (&TextureId, &GlTexture)> {
        self.textures.iter()
    }

    /// Iterate over the formats recorded for renderer-owned textures.
    pub fn format_iter(&self) -> impl Iterator<Item = (&TextureId, &TextureFormat)> {
        self.formats.iter()
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
            glow::PixelUnpackData::Slice(Some(data)),
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
}
