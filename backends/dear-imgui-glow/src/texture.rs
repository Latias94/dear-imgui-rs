//! Texture management for Dear ImGui

use crate::{GlTexture, InitError, InitResult};
use dear_imgui_rs::{OwnedTextureData, TextureData, TextureFormat, TextureId, TextureStatus};
use glow::{Context, HasContext};
use std::collections::HashMap;

/// Trait for managing texture mappings with modern Dear ImGui texture system
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
        width: i32,
        height: i32,
        format: TextureFormat,
    ) -> TextureId;

    /// Update a texture in Dear ImGui's texture management system
    fn update_texture(
        &mut self,
        texture_id: TextureId,
        gl_texture: GlTexture,
        width: i32,
        height: i32,
    );

    /// Get texture data from Dear ImGui's texture management system
    fn get_texture_data(&self, texture_id: TextureId) -> Option<&TextureData>;

    /// Get mutable texture data from Dear ImGui's texture management system
    fn get_texture_data_mut(&mut self, texture_id: TextureId) -> Option<&mut TextureData>;
}

/// Simple texture map implementation using a HashMap with modern texture management
#[derive(Default)]
pub struct SimpleTextureMap {
    textures: HashMap<TextureId, GlTexture>,
    texture_data: HashMap<TextureId, OwnedTextureData>,
    next_id: usize,
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
        self.texture_data.remove(&texture_id);
        gl_texture
    }

    fn clear(&mut self) {
        self.textures.clear();
        self.texture_data.clear();
    }

    fn register_texture(
        &mut self,
        gl_texture: GlTexture,
        width: i32,
        height: i32,
        format: TextureFormat,
    ) -> TextureId {
        self.next_id += 1;
        let texture_id = TextureId::new(self.next_id as u64);

        let mut texture_data = TextureData::new();
        texture_data.create(format, width, height);
        texture_data.set_tex_id(texture_id);
        texture_data.set_status(TextureStatus::OK);

        self.textures.insert(texture_id, gl_texture);
        self.texture_data.insert(texture_id, texture_data);

        texture_id
    }

    fn update_texture(
        &mut self,
        texture_id: TextureId,
        gl_texture: GlTexture,
        _width: i32,
        _height: i32,
    ) {
        self.textures.insert(texture_id, gl_texture);

        if let Some(texture_data) = self.texture_data.get_mut(&texture_id) {
            texture_data.set_tex_id(texture_id);
            texture_data.set_status(TextureStatus::OK);
        }
    }

    fn get_texture_data(&self, texture_id: TextureId) -> Option<&TextureData> {
        self.texture_data.get(&texture_id).map(AsRef::as_ref)
    }

    fn get_texture_data_mut(&mut self, texture_id: TextureId) -> Option<&mut TextureData> {
        self.texture_data.get_mut(&texture_id).map(AsMut::as_mut)
    }
}

impl SimpleTextureMap {
    /// Create a new empty texture map
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            texture_data: HashMap::new(),
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

    /// Iterate over all texture data
    pub fn texture_data_iter(&self) -> impl Iterator<Item = (&TextureId, &TextureData)> {
        self.texture_data
            .iter()
            .map(|(id, texture_data)| (id, texture_data.as_ref()))
    }
}

/// Create a texture from raw RGBA data
pub fn create_texture_from_rgba(
    gl: &Context,
    width: u32,
    height: u32,
    data: &[u8],
) -> InitResult<GlTexture> {
    unsafe {
        let texture = gl.create_texture().map_err(InitError::CreateTexture)?;

        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width as i32,
            height as i32,
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

        gl.bind_texture(glow::TEXTURE_2D, None);

        Ok(texture)
    }
}

/// Create a texture from raw alpha data (single channel)
pub fn create_texture_from_alpha(
    gl: &Context,
    width: u32,
    height: u32,
    data: &[u8],
) -> InitResult<GlTexture> {
    unsafe {
        let texture = gl.create_texture().map_err(InitError::CreateTexture)?;

        gl.bind_texture(glow::TEXTURE_2D, Some(texture));

        // Set pixel store parameters
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RED as i32,
            width as i32,
            height as i32,
            0,
            glow::RED,
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

        gl.bind_texture(glow::TEXTURE_2D, None);

        Ok(texture)
    }
}

/// Update a texture with new data
pub fn update_texture(
    gl: &Context,
    texture: GlTexture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    data: &[u8],
    format: u32,
) {
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_sub_image_2d(
            glow::TEXTURE_2D,
            0,
            x as i32,
            y as i32,
            width as i32,
            height as i32,
            format,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(data)),
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
    }
}

/// Update texture from ImGui texture data (similar to ImGui_ImplOpenGL3_UpdateTexture)
pub fn update_imgui_texture(
    gl: &Context,
    texture_id: TextureId,
    width: u32,
    height: u32,
    data: &[u8],
) -> InitResult<GlTexture> {
    unsafe {
        // Backup current texture binding
        let last_texture = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) as u32;

        // Set pixel store parameters
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

        let gl_texture = if texture_id.id() == 0 {
            // Create new texture
            let texture = gl.create_texture().map_err(InitError::CreateTexture)?;

            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
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
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );

            texture
        } else {
            // Update existing texture
            let texture_u32 = u32::try_from(texture_id.id()).map_err(|_| {
                InitError::Generic("TextureId is out of range for OpenGL".to_string())
            })?;
            let texture_nz = std::num::NonZeroU32::new(texture_u32).ok_or_else(|| {
                InitError::Generic("TextureId must be non-zero for OpenGL".to_string())
            })?;
            let texture = glow::NativeTexture(texture_nz);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );

            texture
        };

        // Restore previous texture binding
        if last_texture != 0 {
            let restore_texture =
                glow::NativeTexture(std::num::NonZeroU32::new(last_texture).unwrap());
            gl.bind_texture(glow::TEXTURE_2D, Some(restore_texture));
        } else {
            gl.bind_texture(glow::TEXTURE_2D, None);
        }

        Ok(gl_texture)
    }
}
