//! Render resources management for the WGPU renderer
//!
//! This module handles shared render resources like samplers, uniforms, and bind groups,
//! corresponding to the RenderResources struct in imgui_impl_wgpu.cpp

use crate::{RendererError, RendererResult, UniformBuffer};
use dear_imgui_rs::TextureId;
use std::collections::HashMap;
use wgpu::*;

/// Shared render resources
///
/// This corresponds to the RenderResources struct in the C++ implementation.
/// Contains samplers, uniform buffers, and bind group layouts that are shared
/// across all frames.
pub struct RenderResources {
    /// Linear texture sampler
    pub sampler: Option<Sampler>,
    /// Nearest/point texture sampler
    pub sampler_nearest: Option<Sampler>,
    /// Uniform buffer manager (also owns the common bind group layout)
    pub uniform_buffer: Option<UniformBuffer>,
    /// Common bind group using the nearest/point sampler
    pub nearest_common_bind_group: Option<BindGroup>,
    /// Image bind groups cache (texture_id -> bind_group)
    pub image_bind_groups: HashMap<TextureId, BindGroup>,
    /// Image bind group layout (cached for efficiency)
    pub image_bind_group_layout: Option<BindGroupLayout>,
}

impl RenderResources {
    /// Create new empty render resources
    pub fn new() -> Self {
        Self {
            sampler: None,
            sampler_nearest: None,
            uniform_buffer: None,
            nearest_common_bind_group: None,
            image_bind_groups: HashMap::new(),
            image_bind_group_layout: None,
        }
    }

    /// Initialize render resources
    pub fn initialize(&mut self, device: &Device) -> RendererResult<()> {
        #[cfg(feature = "wgpu-27")]
        fn linear_mipmap_filter() -> FilterMode {
            FilterMode::Linear
        }
        #[cfg(feature = "wgpu-27")]
        fn nearest_mipmap_filter() -> FilterMode {
            FilterMode::Nearest
        }
        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29"))]
        fn linear_mipmap_filter() -> MipmapFilterMode {
            MipmapFilterMode::Linear
        }
        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29"))]
        fn nearest_mipmap_filter() -> MipmapFilterMode {
            MipmapFilterMode::Nearest
        }

        // Create linear texture sampler (matches imgui_impl_wgpu.cpp sampler setup)
        // Bilinear sampling is required by default. Set 'io.Fonts->Flags |= ImFontAtlasFlags_NoBakedLines'
        // or 'style.AntiAliasedLinesUseTex = false' to allow point/nearest sampling
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Dear ImGui Texture Sampler"),
            address_mode_u: AddressMode::ClampToEdge, // matches WGPUAddressMode_ClampToEdge
            address_mode_v: AddressMode::ClampToEdge, // matches WGPUAddressMode_ClampToEdge
            address_mode_w: AddressMode::ClampToEdge, // matches WGPUAddressMode_ClampToEdge
            mag_filter: FilterMode::Linear,           // matches WGPUFilterMode_Linear
            min_filter: FilterMode::Linear,           // matches WGPUFilterMode_Linear
            mipmap_filter: linear_mipmap_filter(),    // matches WGPUMipmapFilterMode_Linear
            anisotropy_clamp: 1,                      // matches maxAnisotropy = 1
            ..Default::default()
        });

        let sampler_nearest = device.create_sampler(&SamplerDescriptor {
            label: Some("Dear ImGui Texture Sampler Nearest"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: nearest_mipmap_filter(),
            anisotropy_clamp: 1,
            ..Default::default()
        });

        // Create uniform buffer + common bind group layout
        let uniform_buffer = UniformBuffer::new(device, &sampler);

        let nearest_common_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Dear ImGui Common Bind Group Nearest Sampler"),
            layout: uniform_buffer.bind_group_layout(),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.buffer().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler_nearest),
                },
            ],
        });

        // Create image bind group layout (for texture views)
        let image_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Dear ImGui Image Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    multisampled: false,
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            }],
        });

        self.sampler = Some(sampler);
        self.sampler_nearest = Some(sampler_nearest);
        self.uniform_buffer = Some(uniform_buffer);
        self.nearest_common_bind_group = Some(nearest_common_bind_group);
        self.image_bind_group_layout = Some(image_bind_group_layout);

        Ok(())
    }

    /// Create an image bind group for a texture
    pub fn create_image_bind_group(
        &self,
        device: &Device,
        texture_view: &TextureView,
    ) -> RendererResult<BindGroup> {
        let layout = self.image_bind_group_layout.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState("Image bind group layout not initialized".to_string())
        })?;

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Dear ImGui Image Bind Group"),
            layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(texture_view),
            }],
        });

        Ok(bind_group)
    }

    /// Get or create an image bind group for a texture
    pub fn get_or_create_image_bind_group(
        &mut self,
        device: &Device,
        texture_id: TextureId,
        texture_view: &TextureView,
    ) -> RendererResult<&BindGroup> {
        if !self.image_bind_groups.contains_key(&texture_id) {
            let bind_group = self.create_image_bind_group(device, texture_view)?;
            self.image_bind_groups.insert(texture_id, bind_group);
        }

        self.image_bind_groups.get(&texture_id).ok_or_else(|| {
            RendererError::InvalidRenderState("Image bind group missing after creation".to_string())
        })
    }

    /// Remove an image bind group
    pub fn remove_image_bind_group(&mut self, texture_id: TextureId) {
        self.image_bind_groups.remove(&texture_id);
    }

    /// Clear all image bind groups
    pub fn clear_image_bind_groups(&mut self) {
        self.image_bind_groups.clear();
    }

    /// Get the texture sampler
    pub fn sampler(&self) -> Option<&Sampler> {
        self.sampler.as_ref()
    }

    /// Get the nearest/point texture sampler
    pub fn sampler_nearest(&self) -> Option<&Sampler> {
        self.sampler_nearest.as_ref()
    }

    /// Get the uniform buffer
    pub fn uniform_buffer(&self) -> Option<&UniformBuffer> {
        self.uniform_buffer.as_ref()
    }

    /// Get the common bind group
    pub fn common_bind_group(&self) -> Option<&BindGroup> {
        self.uniform_buffer.as_ref().map(|ub| ub.bind_group())
    }

    /// Get the common bind group using nearest/point sampling
    pub fn nearest_common_bind_group(&self) -> Option<&BindGroup> {
        self.nearest_common_bind_group.as_ref()
    }

    /// Get the image bind group layout
    pub fn image_bind_group_layout(&self) -> Option<&BindGroupLayout> {
        self.image_bind_group_layout.as_ref()
    }

    /// Check if resources are initialized
    pub fn is_initialized(&self) -> bool {
        self.sampler.is_some()
            && self.sampler_nearest.is_some()
            && self.uniform_buffer.is_some()
            && self.nearest_common_bind_group.is_some()
            && self.image_bind_group_layout.is_some()
    }

    /// Get statistics for debugging
    pub fn stats(&self) -> RenderResourcesStats {
        RenderResourcesStats {
            image_bind_groups_count: self.image_bind_groups.len(),
            is_initialized: self.is_initialized(),
        }
    }
}

impl Default for RenderResources {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for render resources
#[derive(Debug, Clone)]
pub struct RenderResourcesStats {
    pub image_bind_groups_count: usize,
    pub is_initialized: bool,
}
