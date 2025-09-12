//! Modern texture management for Bevy-ImGui integration
//!
//! This module implements Dear ImGui's modern texture system (1.92+) for Bevy,
//! providing automatic texture creation, updates, and destruction.

use bevy::{
    asset::{Handle, StrongHandle},
    image::Image,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Sampler,
            SamplerDescriptor, TextureView, TextureViewDescriptor,
        },
        renderer::RenderDevice,
        texture::GpuImage,
    },
};
use dear_imgui::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::{collections::HashMap, sync::Arc};
use tracing::{info, warn};

/// Result of a texture update operation
#[derive(Debug, Clone)]
pub enum TextureUpdateResult {
    /// Texture was successfully created
    Created { texture_id: TextureId },
    /// Texture was successfully updated
    Updated,
    /// Texture was destroyed
    Destroyed,
    /// Texture update failed
    Failed,
    /// No action was needed
    NoAction,
}

impl TextureUpdateResult {
    /// Apply the result to a texture data object
    pub fn apply_to(self, texture_data: &mut TextureData) {
        match self {
            TextureUpdateResult::Created { texture_id } => {
                texture_data.set_tex_id(texture_id);
                texture_data.set_status(TextureStatus::OK);
            }
            TextureUpdateResult::Updated => {
                texture_data.set_status(TextureStatus::OK);
            }
            TextureUpdateResult::Destroyed => {
                texture_data.set_status(TextureStatus::Destroyed);
            }
            TextureUpdateResult::Failed => {
                texture_data.set_status(TextureStatus::Destroyed);
            }
            TextureUpdateResult::NoAction => {
                // No changes needed
            }
        }
    }
}

/// Bevy texture resource for Dear ImGui
#[derive(Debug)]
pub struct BevyImguiTexture {
    /// Bevy texture handle
    pub handle: Handle<Image>,
    /// WGPU texture view for binding
    pub texture_view: TextureView,
    /// Sampler for the texture
    pub sampler: Sampler,
    /// Bind group for rendering
    pub bind_group: BindGroup,
}

impl BevyImguiTexture {
    /// Create a new Bevy ImGui texture
    pub fn new(
        handle: Handle<Image>,
        texture_view: TextureView,
        sampler: Sampler,
        bind_group: BindGroup,
    ) -> Self {
        Self {
            handle,
            texture_view,
            sampler,
            bind_group,
        }
    }

    /// Get the bind group for rendering
    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}

/// Modern texture manager for Bevy-ImGui integration
#[derive(Debug, Default, bevy::prelude::Resource)]
pub struct BevyImguiTextureManager {
    /// Map from texture ID to Bevy texture
    textures: HashMap<u64, BevyImguiTexture>,
    /// Next available texture ID
    next_id: u64,
}

impl BevyImguiTextureManager {
    /// Create a new texture manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 1, // Start from 1, 0 is reserved for null texture
        }
    }

    /// Register a new texture and return its ID
    pub fn register_texture(&mut self, texture: BevyImguiTexture) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, texture);
        id
    }

    /// Get a texture by ID
    pub fn get_texture(&self, id: u64) -> Option<&BevyImguiTexture> {
        self.textures.get(&id)
    }

    /// Remove a texture by ID
    pub fn remove_texture(&mut self, id: u64) -> Option<BevyImguiTexture> {
        self.textures.remove(&id)
    }

    /// Check if a texture exists
    pub fn contains_texture(&self, id: u64) -> bool {
        self.textures.contains_key(&id)
    }

    /// Insert a texture with a specific ID
    pub fn insert_texture_with_id(&mut self, id: u64, texture: BevyImguiTexture) {
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

    /// Create a texture from a Bevy image handle
    pub fn create_texture_from_handle(
        &mut self,
        tex_id: u64,
        image_handle: Handle<Image>,
        device: &RenderDevice,
        _queue: &bevy::render::renderer::RenderQueue,
        gpu_images: &RenderAssets<GpuImage>,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> Result<(), String> {
        // Get the GPU image from the handle
        if let Some(gpu_image) = gpu_images.get(&image_handle) {
            // Create bind group for the texture
            let bind_group = device.create_bind_group(
                Some("imgui_font_texture_bind_group"),
                texture_bind_group_layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&gpu_image.texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&gpu_image.sampler),
                    },
                ],
            );

            // Create Bevy texture
            let bevy_texture = BevyImguiTexture::new(
                image_handle,
                gpu_image.texture_view.clone(),
                gpu_image.sampler.clone(),
                bind_group,
            );

            // Insert with specific ID
            self.insert_texture_with_id(tex_id, bevy_texture);
            Ok(())
        } else {
            Err("GPU image not found for handle".to_string())
        }
    }

    /// Create a texture from Dear ImGui texture data
    pub fn create_texture_from_data(
        &mut self,
        device: &RenderDevice,
        queue: &bevy::render::renderer::RenderQueue,
        _gpu_images: &RenderAssets<GpuImage>,
        texture_bind_group_layout: &BindGroupLayout,
        texture_data: &TextureData,
    ) -> Result<u64, String> {
        let width = texture_data.width() as u32;
        let height = texture_data.height() as u32;
        let format = texture_data.format();

        let pixels = texture_data
            .pixels()
            .ok_or_else(|| "No pixel data available".to_string())?;

        // Convert ImGui texture format to WGPU format and handle data conversion
        let (wgpu_format, converted_data, _bytes_per_pixel) = match format {
            ImGuiTextureFormat::RGBA32 => {
                // RGBA32 maps directly to RGBA8Unorm
                if pixels.len() != (width * height * 4) as usize {
                    return Err(format!(
                        "RGBA32 texture data size mismatch: expected {} bytes, got {}",
                        width * height * 4,
                        pixels.len()
                    ));
                }
                (wgpu::TextureFormat::Rgba8Unorm, pixels.to_vec(), 4u32)
            }
            ImGuiTextureFormat::Alpha8 => {
                // Convert Alpha8 to RGBA32 for WGPU (white RGB + original alpha)
                if pixels.len() != (width * height) as usize {
                    return Err(format!(
                        "Alpha8 texture data size mismatch: expected {} bytes, got {}",
                        width * height,
                        pixels.len()
                    ));
                }
                let mut rgba_data = Vec::with_capacity(pixels.len() * 4);
                for &alpha in pixels {
                    rgba_data.extend_from_slice(&[255, 255, 255, alpha]); // White RGB + alpha
                }
                (wgpu::TextureFormat::Rgba8Unorm, rgba_data, 4u32)
            }
        };

        // Create WGPU texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dear ImGui Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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
                bytes_per_row: Some(width * 4), // Always 4 bytes per pixel (RGBA)
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // Create texture view
        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        // Create sampler
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("imgui_texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group
        let bind_group = device.create_bind_group(
            Some("imgui_texture_bind_group"),
            texture_bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        );

        // Create Bevy texture (we need a dummy handle for now)
        let dummy_handle = Handle::<Image>::default();
        let bevy_texture = BevyImguiTexture::new(dummy_handle, texture_view, sampler, bind_group);

        // Register and return ID
        let texture_id = self.register_texture(bevy_texture);
        Ok(texture_id)
    }

    /// Handle texture updates from Dear ImGui draw data
    pub fn handle_texture_updates(
        &mut self,
        draw_data: &dear_imgui::render::DrawData,
        device: &RenderDevice,
        queue: &bevy::render::renderer::RenderQueue,
        gpu_images: &RenderAssets<GpuImage>,
        texture_bind_group_layout: &BindGroupLayout,
    ) {
        for texture_data in draw_data.textures() {
            let status = texture_data.status();
            let current_tex_id = texture_data.tex_id().id();

            match status {
                TextureStatus::WantCreate => {
                    match self.create_texture_from_data(
                        device,
                        queue,
                        gpu_images,
                        texture_bind_group_layout,
                        texture_data,
                    ) {
                        Ok(bevy_texture_id) => {
                            let new_texture_id = TextureId::from(bevy_texture_id as usize);
                            texture_data.set_tex_id(new_texture_id);
                            texture_data.set_status(TextureStatus::OK);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to create texture for ID: {}, error: {}",
                                current_tex_id, e
                            );
                        }
                    }
                }
                TextureStatus::WantUpdates => {
                    let imgui_tex_id = texture_data.tex_id();
                    let internal_id = imgui_tex_id.id() as u64;

                    // For now, we recreate the texture instead of updating in place
                    if self.contains_texture(internal_id) {
                        self.remove_texture(internal_id);
                        match self.create_texture_from_data(
                            device,
                            queue,
                            gpu_images,
                            texture_bind_group_layout,
                            texture_data,
                        ) {
                            Ok(new_texture_id) => {
                                if new_texture_id != internal_id {
                                    if let Some(texture) = self.remove_texture(new_texture_id) {
                                        self.insert_texture_with_id(internal_id, texture);
                                    }
                                }
                                texture_data.set_status(TextureStatus::OK);
                            }
                            Err(_) => {
                                texture_data.set_status(TextureStatus::Destroyed);
                            }
                        }
                    } else {
                        texture_data.set_status(TextureStatus::Destroyed);
                    }
                }
                TextureStatus::WantDestroy => {
                    let imgui_tex_id = texture_data.tex_id();
                    let internal_id = imgui_tex_id.id() as u64;
                    self.remove_texture(internal_id);
                    texture_data.set_status(TextureStatus::Destroyed);
                }
                TextureStatus::OK | TextureStatus::Destroyed => {
                    // No action needed
                }
            }
        }
    }

    /// Update a single texture based on its status
    pub fn update_single_texture(
        &mut self,
        texture_data: &TextureData,
        device: &RenderDevice,
        queue: &bevy::render::renderer::RenderQueue,
        gpu_images: &RenderAssets<GpuImage>,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> Result<TextureUpdateResult, String> {
        match texture_data.status() {
            TextureStatus::WantCreate => {
                match self.create_texture_from_data(
                    device,
                    queue,
                    gpu_images,
                    texture_bind_group_layout,
                    texture_data,
                ) {
                    Ok(texture_id) => Ok(TextureUpdateResult::Created {
                        texture_id: TextureId::from(texture_id),
                    }),
                    Err(e) => Err(format!("Failed to create texture: {}", e)),
                }
            }
            TextureStatus::WantUpdates => {
                let imgui_tex_id = texture_data.tex_id();
                let internal_id = imgui_tex_id.id() as u64;

                if self.contains_texture(internal_id) {
                    self.remove_texture(internal_id);
                    match self.create_texture_from_data(
                        device,
                        queue,
                        gpu_images,
                        texture_bind_group_layout,
                        texture_data,
                    ) {
                        Ok(new_texture_id) => {
                            if new_texture_id != internal_id {
                                if let Some(texture) = self.remove_texture(new_texture_id) {
                                    self.insert_texture_with_id(internal_id, texture);
                                }
                            }
                            Ok(TextureUpdateResult::Updated)
                        }
                        Err(_) => Ok(TextureUpdateResult::Failed),
                    }
                } else {
                    Ok(TextureUpdateResult::Failed)
                }
            }
            TextureStatus::WantDestroy => {
                let imgui_tex_id = texture_data.tex_id();
                let internal_id = imgui_tex_id.id() as u64;
                self.remove_texture(internal_id);
                Ok(TextureUpdateResult::Destroyed)
            }
            TextureStatus::OK | TextureStatus::Destroyed => Ok(TextureUpdateResult::NoAction),
        }
    }
}

/// Create a bind group for a Bevy image texture
pub fn create_texture_bind_group(
    texture_id: u32,
    strong: &Arc<StrongHandle>,
    gpu_images: &RenderAssets<GpuImage>,
    device: &RenderDevice,
    texture_bind_group_layout: &BindGroupLayout,
) -> Result<BindGroup, String> {
    let handle = Handle::<Image>::Strong(strong.clone());

    if let Some(gpu_image) = gpu_images.get(&handle) {
        info!(
            "Creating bind group for texture {:?} with size {}x{}",
            texture_id,
            gpu_image.texture.width(),
            gpu_image.texture.height()
        );

        // Create bind group for the texture
        let label = format!("imgui_texture_bind_group_{}", texture_id);
        let bind_group = device.create_bind_group(
            Some(label.as_str()),
            texture_bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&gpu_image.texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&gpu_image.sampler),
                },
            ],
        );

        Ok(bind_group)
    } else {
        Err(format!(
            "Could not obtain GPU image for texture {:?}. Please ensure textures are loaded prior to registering them with imgui",
            texture_id
        ))
    }
}

/// Texture registry for managing user textures
#[derive(Default)]
pub struct TextureRegistry {
    /// Map from texture ID to bind group
    pub bind_groups: HashMap<u32, BindGroup>,
    /// Map from texture ID to strong handle (to keep textures alive)
    pub handles: HashMap<u32, Arc<StrongHandle>>,
    /// Next available texture ID
    pub next_id: u32,
}

impl TextureRegistry {
    /// Register a new texture and return its ID
    pub fn register_texture(&mut self, handle: Arc<StrongHandle>) -> u32 {
        let id = self.next_id;
        self.handles.insert(id, handle);
        self.next_id += 1;
        id
    }

    /// Unregister a texture by ID
    pub fn unregister_texture(&mut self, texture_id: u32) {
        self.handles.remove(&texture_id);
        self.bind_groups.remove(&texture_id);
        info!("Unregistered texture {:?}", texture_id);
    }

    /// Get bind group for a texture ID
    pub fn get_bind_group(&self, texture_id: u32) -> Option<&BindGroup> {
        self.bind_groups.get(&texture_id)
    }

    /// Update bind groups for all registered textures
    pub fn update_bind_groups(
        &mut self,
        gpu_images: &RenderAssets<GpuImage>,
        device: &RenderDevice,
        texture_bind_group_layout: &BindGroupLayout,
    ) {
        // Clear existing bind groups
        self.bind_groups.clear();

        // Create bind groups for all registered textures
        for (&texture_id, handle) in &self.handles {
            match create_texture_bind_group(
                texture_id,
                handle,
                gpu_images,
                device,
                texture_bind_group_layout,
            ) {
                Ok(bind_group) => {
                    self.bind_groups.insert(texture_id, bind_group);
                }
                Err(e) => {
                    warn!(
                        "Failed to create bind group for texture {}: {}",
                        texture_id, e
                    );
                }
            }
        }
    }
}

/// Configuration for texture sampling in ImGui
#[derive(Debug, Clone)]
pub struct TextureConfig {
    /// Texture label for debugging
    pub label: Option<String>,
    /// Sampler descriptor for the texture
    pub sampler_desc: wgpu::SamplerDescriptor<'static>,
}

impl Default for TextureConfig {
    fn default() -> Self {
        Self {
            label: Some("Bevy Texture for ImGui".to_string()),
            sampler_desc: wgpu::SamplerDescriptor {
                label: Some("Bevy Texture Sampler for ImGui"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                compare: None,
                anisotropy_clamp: 1,
                border_color: None,
            },
        }
    }
}
