//! Texture management for the WGPU renderer
//!
//! This module handles texture creation, updates, and management,
//! integrating with Dear ImGui's modern texture system.

use crate::{RendererError, RendererResult};
use dear_imgui::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::collections::HashMap;
use wgpu::*;

/// WGPU texture resource
///
/// This corresponds to ImGui_ImplWGPU_Texture in the C++ implementation
#[derive(Debug)]
pub struct WgpuTexture {
    /// WGPU texture object
    pub texture: Texture,
    /// Texture view for binding
    pub texture_view: TextureView,
}

impl WgpuTexture {
    /// Create a new WGPU texture
    pub fn new(texture: Texture, texture_view: TextureView) -> Self {
        Self {
            texture,
            texture_view,
        }
    }

    /// Get the texture view for binding
    pub fn view(&self) -> &TextureView {
        &self.texture_view
    }

    /// Get the texture object
    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}

/// Texture manager for WGPU renderer
///
/// This manages the mapping between Dear ImGui texture IDs and WGPU textures,
/// similar to the ImageBindGroups storage in the C++ implementation.
#[derive(Debug, Default)]
pub struct WgpuTextureManager {
    /// Map from texture ID to WGPU texture
    textures: HashMap<u64, WgpuTexture>,
    /// Next available texture ID
    next_id: u64,
}

impl WgpuTextureManager {
    /// Create a new texture manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 1, // Start from 1, 0 is reserved for null texture
        }
    }

    /// Register a new texture and return its ID
    pub fn register_texture(&mut self, texture: WgpuTexture) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, texture);
        id
    }

    /// Get a texture by ID
    pub fn get_texture(&self, id: u64) -> Option<&WgpuTexture> {
        self.textures.get(&id)
    }

    /// Remove a texture by ID
    pub fn remove_texture(&mut self, id: u64) -> Option<WgpuTexture> {
        self.textures.remove(&id)
    }

    /// Check if a texture exists
    pub fn contains_texture(&self, id: u64) -> bool {
        self.textures.contains_key(&id)
    }

    /// Insert a texture with a specific ID
    pub fn insert_texture_with_id(&mut self, id: u64, texture: WgpuTexture) {
        self.textures.insert(id, texture);
        // Update next_id if necessary
        if id >= self.next_id {
            self.next_id = id + 1;
        }
    }

    /// Get the number of registered textures
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Clear all textures
    pub fn clear(&mut self) {
        self.textures.clear();
        self.next_id = 1;
    }
}

/// Texture creation and management functions
impl WgpuTextureManager {
    /// Create a texture from Dear ImGui texture data
    pub fn create_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> RendererResult<u64> {
        let width = texture_data.width() as u32;
        let height = texture_data.height() as u32;
        let format = texture_data.format();

        let pixels = texture_data
            .pixels()
            .ok_or_else(|| RendererError::BadTexture("No pixel data available".to_string()))?;

        // Convert ImGui texture format to WGPU format and handle data conversion
        let (wgpu_format, converted_data) = match format {
            ImGuiTextureFormat::RGBA32 => (TextureFormat::Rgba8Unorm, pixels.to_vec()),
            ImGuiTextureFormat::Alpha8 => {
                // Convert Alpha8 to RGBA32 for WGPU
                let mut rgba_data = Vec::with_capacity(pixels.len() * 4);
                for &alpha in pixels {
                    rgba_data.extend_from_slice(&[255, 255, 255, alpha]); // White + alpha
                }
                (TextureFormat::Rgba8Unorm, rgba_data)
            }
        };

        // Create WGPU texture
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

        // Upload texture data
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
                bytes_per_row: Some(width * 4), // RGBA = 4 bytes per pixel
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // Create texture view
        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        // Create WGPU texture wrapper
        let wgpu_texture = WgpuTexture::new(texture, texture_view);

        // Register and return ID
        let texture_id = self.register_texture(wgpu_texture);
        Ok(texture_id)
    }

    /// Update an existing texture from Dear ImGui texture data
    pub fn update_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> RendererResult<()> {
        let texture_id = texture_data.tex_id().id() as u64;

        // For WGPU, we recreate the texture instead of updating in place
        // This is simpler and more reliable than trying to update existing textures
        if self.contains_texture(texture_id) {
            // Remove old texture
            self.remove_texture(texture_id);

            // Create new texture
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;

            // Move the texture to the correct ID slot if needed
            if new_texture_id != texture_id {
                if let Some(texture) = self.remove_texture(new_texture_id) {
                    self.insert_texture_with_id(texture_id, texture);
                }
            }
        } else {
            // Create new texture if it doesn't exist
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;
            if new_texture_id != texture_id {
                if let Some(texture) = self.remove_texture(new_texture_id) {
                    self.insert_texture_with_id(texture_id, texture);
                }
            }
        }

        Ok(())
    }

    /// Destroy a texture
    pub fn destroy_texture(&mut self, texture_id: TextureId) {
        let texture_id_u64 = texture_id.id() as u64;
        self.remove_texture(texture_id_u64);
        // WGPU textures are automatically cleaned up when dropped
    }

    /// Handle texture updates from Dear ImGui draw data
    pub fn handle_texture_updates(
        &mut self,
        draw_data: &dear_imgui::render::DrawData,
        device: &Device,
        queue: &Queue,
    ) {
        for texture_data in draw_data.textures() {
            match texture_data.status() {
                TextureStatus::WantCreate => {
                    if let Ok(texture_id) =
                        self.create_texture_from_data(device, queue, texture_data)
                    {
                        // Update the texture data with the new ID
                        texture_data.set_tex_id(TextureId::from(texture_id as usize));
                        texture_data.set_status(TextureStatus::OK);
                    }
                }
                TextureStatus::WantUpdates => {
                    if self
                        .update_texture_from_data(device, queue, texture_data)
                        .is_err()
                    {
                        // If update fails, mark as destroyed
                        texture_data.set_status(TextureStatus::Destroyed);
                    } else {
                        texture_data.set_status(TextureStatus::OK);
                    }
                }
                TextureStatus::WantDestroy => {
                    self.destroy_texture(texture_data.tex_id());
                    texture_data.set_status(TextureStatus::Destroyed);
                }
                TextureStatus::OK | TextureStatus::Destroyed => {
                    // No action needed
                }
            }
        }
    }
}
