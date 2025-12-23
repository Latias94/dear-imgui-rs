//! Texture management for the WGPU renderer
//!
//! This module handles texture creation, updates, and management,
//! integrating with Dear ImGui's modern texture system.

use crate::{RenderResources, RendererError, RendererResult};
use dear_imgui_rs::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
use std::collections::HashMap;
use wgpu::*;

/// Result of a texture update operation
///
/// This enum represents the outcome of a texture update operation and
/// contains any state changes that need to be applied to the texture data.
/// This follows Rust's principle of explicit state management.
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
    ///
    /// This method updates the texture data's status and ID based on the operation result.
    /// This is the Rust-idiomatic way to handle state updates.
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
#[derive(Debug)]
pub struct WgpuTextureManager {
    /// Map from texture ID to WGPU texture
    textures: HashMap<u64, WgpuTexture>,
    /// Next available texture ID
    next_id: u64,
    /// Custom samplers registered for external textures (sampler_id -> sampler)
    custom_samplers: HashMap<u64, Sampler>,
    /// Mapping from texture_id -> sampler_id for per-texture custom sampling
    custom_sampler_by_texture: HashMap<u64, u64>,
    /// Cached common bind groups (uniform buffer + sampler) per sampler_id
    common_bind_groups: HashMap<u64, BindGroup>,
    /// Next available sampler ID
    next_sampler_id: u64,
}

impl Default for WgpuTextureManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WgpuTextureManager {
    /// Convert a sub-rectangle of ImGui texture pixels into a tightly packed RGBA8 buffer
    fn convert_subrect_to_rgba(
        texture_data: &TextureData,
        rect: dear_imgui_rs::texture::TextureRect,
    ) -> Option<Vec<u8>> {
        let pixels = texture_data.pixels()?;
        let tex_w = texture_data.width() as usize;
        let tex_h = texture_data.height() as usize;
        if tex_w == 0 || tex_h == 0 {
            return None;
        }

        let bpp = texture_data.bytes_per_pixel() as usize;
        let (rx, ry, rw, rh) = (
            rect.x as usize,
            rect.y as usize,
            rect.w as usize,
            rect.h as usize,
        );
        if rw == 0 || rh == 0 || rx >= tex_w || ry >= tex_h {
            return None;
        }

        // Clamp to texture bounds defensively
        let rw = rw.min(tex_w.saturating_sub(rx));
        let rh = rh.min(tex_h.saturating_sub(ry));

        let mut out = vec![0u8; rw * rh * 4];
        match texture_data.format() {
            ImGuiTextureFormat::RGBA32 => {
                for row in 0..rh {
                    let src_off = ((ry + row) * tex_w + rx) * bpp;
                    let dst_off = row * rw * 4;
                    // Copy only the row slice and convert layout if needed (it is already RGBA)
                    out[dst_off..dst_off + rw * 4]
                        .copy_from_slice(&pixels[src_off..src_off + rw * 4]);
                }
            }
            ImGuiTextureFormat::Alpha8 => {
                for row in 0..rh {
                    let src_off = ((ry + row) * tex_w + rx) * bpp; // bpp = 1
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

    /// Apply queued sub-rectangle updates to an existing WGPU texture.
    /// Returns true if any update was applied.
    fn apply_subrect_updates(
        &mut self,
        queue: &Queue,
        texture_data: &TextureData,
        texture_id: u64,
    ) -> RendererResult<bool> {
        let wgpu_tex = match self.textures.get(&texture_id) {
            Some(t) => t,
            None => return Ok(false),
        };

        // Collect update rectangles; prefer explicit Updates[] if present,
        // otherwise fallback to single UpdateRect.
        let mut rects: Vec<dear_imgui_rs::texture::TextureRect> = texture_data.updates().collect();
        if rects.is_empty() {
            let r = texture_data.update_rect();
            if r.w > 0 && r.h > 0 {
                rects.push(r);
            }
        }
        if rects.is_empty() {
            return Ok(false);
        }

        // Upload each rect
        for rect in rects {
            if let Some(tight_rgba) = Self::convert_subrect_to_rgba(texture_data, rect) {
                let width = rect.w as u32;
                let height = rect.h as u32;
                let bpp = 4u32;
                let unpadded_bytes_per_row = width * bpp;
                let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT; // 256
                let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

                if padded_bytes_per_row == unpadded_bytes_per_row {
                    // Aligned: direct upload
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: wgpu_tex.texture(),
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: rect.x as u32,
                                y: rect.y as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &tight_rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(unpadded_bytes_per_row),
                            rows_per_image: Some(height),
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                } else {
                    // Pad each row to the required alignment
                    let mut padded = vec![0u8; (padded_bytes_per_row * height) as usize];
                    for row in 0..height as usize {
                        let src_off = row * (unpadded_bytes_per_row as usize);
                        let dst_off = row * (padded_bytes_per_row as usize);
                        padded[dst_off..dst_off + (unpadded_bytes_per_row as usize)]
                            .copy_from_slice(
                                &tight_rgba[src_off..src_off + (unpadded_bytes_per_row as usize)],
                            );
                    }
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: wgpu_tex.texture(),
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: rect.x as u32,
                                y: rect.y as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &padded,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_bytes_per_row),
                            rows_per_image: Some(height),
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Updated texture id={} subrect x={} y={} w={} h={}",
                        texture_id, rect.x, rect.y, rect.w, rect.h
                    );
                }
            } else {
                // No pixels available, cannot update this rect
                if cfg!(debug_assertions) {
                    tracing::debug!(
                        target: "dear-imgui-wgpu",
                        "[dear-imgui-wgpu][debug] Skipped subrect update: no pixels available"
                    );
                }
                return Ok(false);
            }
        }

        Ok(true)
    }
    /// Create a new texture manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            next_id: 1, // Start from 1, 0 is reserved for null texture
            custom_samplers: HashMap::new(),
            custom_sampler_by_texture: HashMap::new(),
            common_bind_groups: HashMap::new(),
            next_sampler_id: 1, // Start from 1, 0 means "default sampler"
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

    /// Associate a custom sampler with a texture id (used by external textures).
    ///
    /// Returns the internal sampler_id assigned to this sampler.
    pub(crate) fn set_custom_sampler_for_texture(
        &mut self,
        texture_id: u64,
        sampler: Sampler,
    ) -> u64 {
        let sampler_id = self.next_sampler_id;
        self.next_sampler_id += 1;
        self.custom_samplers.insert(sampler_id, sampler);
        self.custom_sampler_by_texture
            .insert(texture_id, sampler_id);
        // Invalidate any cached common bind group for this sampler id (defensive).
        self.common_bind_groups.remove(&sampler_id);
        sampler_id
    }

    /// Update or set a custom sampler for an existing texture.
    ///
    /// If the texture already has a custom sampler association, we replace the sampler
    /// in place (keeping the sampler_id stable) and invalidate the cached common bind group.
    /// If there is no association yet, we create one.
    ///
    /// Returns false if the texture_id is not registered.
    pub(crate) fn update_custom_sampler_for_texture(
        &mut self,
        texture_id: u64,
        sampler: Sampler,
    ) -> bool {
        if !self.textures.contains_key(&texture_id) {
            return false;
        }
        if let Some(sampler_id) = self.custom_sampler_by_texture.get(&texture_id).copied() {
            self.custom_samplers.insert(sampler_id, sampler);
            self.common_bind_groups.remove(&sampler_id);
        } else {
            self.set_custom_sampler_for_texture(texture_id, sampler);
        }
        true
    }

    /// Get the custom sampler id for a texture (if any).
    pub(crate) fn custom_sampler_id_for_texture(&self, texture_id: u64) -> Option<u64> {
        self.custom_sampler_by_texture.get(&texture_id).copied()
    }

    /// Remove any custom sampler association for a texture.
    pub(crate) fn clear_custom_sampler_for_texture(&mut self, texture_id: u64) {
        if let Some(sampler_id) = self.custom_sampler_by_texture.remove(&texture_id) {
            // Drop cached bind group so next use rebuilds it.
            self.common_bind_groups.remove(&sampler_id);
        }
    }

    /// Get or create a common bind group (uniform buffer + sampler) for the given sampler id.
    ///
    /// The bind group uses the same uniform buffer but swaps the sampler, allowing
    /// per-texture sampling without changing the pipeline layout.
    pub(crate) fn get_or_create_common_bind_group_for_sampler(
        &mut self,
        device: &Device,
        common_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        sampler_id: u64,
    ) -> Option<BindGroup> {
        if let Some(bg) = self.common_bind_groups.get(&sampler_id) {
            return Some(bg.clone());
        }
        let sampler = self.custom_samplers.get(&sampler_id)?;
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Dear ImGui Common Bind Group (custom sampler)"),
            layout: common_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        });
        self.common_bind_groups.insert(sampler_id, bg.clone());
        Some(bg)
    }

    /// Destroy a texture by ID
    pub fn destroy_texture_by_id(&mut self, id: u64) {
        self.remove_texture(id);
    }

    /// Update an existing texture from Dear ImGui texture data with specific ID
    pub fn update_texture_from_data_with_id(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
        texture_id: u64,
    ) -> RendererResult<()> {
        // For WGPU, we recreate the texture instead of updating in place
        // This is simpler and more reliable than trying to update existing textures
        if self.contains_texture(texture_id) {
            // Remove old texture
            self.remove_texture(texture_id);

            // Create new texture
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;

            // Move the texture to the correct ID slot if needed
            if new_texture_id != texture_id
                && let Some(texture) = self.remove_texture(new_texture_id)
            {
                self.insert_texture_with_id(texture_id, texture);
            }

            Ok(())
        } else {
            Err(RendererError::InvalidTextureId(texture_id))
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
        self.custom_sampler_by_texture.clear();
        self.common_bind_groups.clear();
        // Keep samplers around? Clear to avoid holding stale handles after device loss.
        self.custom_samplers.clear();
        self.next_sampler_id = 1;
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
        // This matches the texture format handling in imgui_impl_wgpu.cpp
        let (wgpu_format, converted_data, _bytes_per_pixel) = match format {
            ImGuiTextureFormat::RGBA32 => {
                // RGBA32 maps directly to RGBA8Unorm (matches C++ implementation)
                if pixels.len() != (width * height * 4) as usize {
                    return Err(RendererError::BadTexture(format!(
                        "RGBA32 texture data size mismatch: expected {} bytes, got {}",
                        width * height * 4,
                        pixels.len()
                    )));
                }
                (TextureFormat::Rgba8Unorm, pixels.to_vec(), 4u32)
            }
            ImGuiTextureFormat::Alpha8 => {
                // Convert Alpha8 to RGBA32 for WGPU (white RGB + original alpha)
                // This ensures compatibility with the standard RGBA8Unorm format
                if pixels.len() != (width * height) as usize {
                    return Err(RendererError::BadTexture(format!(
                        "Alpha8 texture data size mismatch: expected {} bytes, got {}",
                        width * height,
                        pixels.len()
                    )));
                }
                let mut rgba_data = Vec::with_capacity(pixels.len() * 4);
                for &alpha in pixels {
                    rgba_data.extend_from_slice(&[255, 255, 255, alpha]); // White RGB + alpha
                }
                (TextureFormat::Rgba8Unorm, rgba_data, 4u32)
            }
        };

        // Create WGPU texture (matches the descriptor setup in imgui_impl_wgpu.cpp)
        if cfg!(debug_assertions) {
            tracing::debug!(
                target: "dear-imgui-wgpu",
                "[dear-imgui-wgpu][debug] Create texture: {}x{} format={:?}",
                width, height, format
            );
        }
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

        // Validate texture data size before upload
        let expected_size = (width * height * 4) as usize; // Always RGBA after conversion
        if converted_data.len() != expected_size {
            return Err(RendererError::BadTexture(format!(
                "Converted texture data size mismatch: expected {} bytes, got {}",
                expected_size,
                converted_data.len()
            )));
        }

        // Upload texture data (matches the upload logic in imgui_impl_wgpu.cpp)
        // WebGPU requires bytes_per_row to be 256-byte aligned. Pad rows if needed.
        let bpp = 4u32;
        let unpadded_bytes_per_row = width * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT; // 256
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        if padded_bytes_per_row == unpadded_bytes_per_row {
            // Aligned: direct upload
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
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            // Pad each row to the required alignment
            let mut padded: Vec<u8> = vec![0; (padded_bytes_per_row * height) as usize];
            for row in 0..height as usize {
                let src_off = row * (unpadded_bytes_per_row as usize);
                let dst_off = row * (padded_bytes_per_row as usize);
                padded[dst_off..dst_off + (unpadded_bytes_per_row as usize)].copy_from_slice(
                    &converted_data[src_off..src_off + (unpadded_bytes_per_row as usize)],
                );
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &padded,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            if cfg!(debug_assertions) {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Upload texture with padded row pitch: unpadded={} padded={}",
                    unpadded_bytes_per_row, padded_bytes_per_row
                );
            }
        }

        // Create texture view
        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        // Create WGPU texture wrapper
        let wgpu_texture = WgpuTexture::new(texture, texture_view);

        // Register and return ID
        let texture_id = self.register_texture(wgpu_texture);
        if cfg!(debug_assertions) {
            tracing::debug!(
                target: "dear-imgui-wgpu",
                "[dear-imgui-wgpu][debug] Texture registered: id={}",
                texture_id
            );
        }
        Ok(texture_id)
    }

    /// Update an existing texture from Dear ImGui texture data
    pub fn update_texture_from_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture_data: &TextureData,
    ) -> RendererResult<()> {
        let texture_id = texture_data.tex_id().id();

        // If the texture already exists and the TextureData only requests sub-rectangle
        // updates, honor them in-place to match Dear ImGui 1.92 semantics.
        // Fallback to full re-create when there is no pixel data available.
        if self.contains_texture(texture_id) {
            // Attempt sub-rect updates first (preferred path)
            if self.apply_subrect_updates(queue, texture_data, texture_id)? {
                return Ok(());
            }

            // Otherwise, recreate from full data
            self.remove_texture(texture_id);
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;
            if new_texture_id != texture_id
                && let Some(texture) = self.remove_texture(new_texture_id)
            {
                self.insert_texture_with_id(texture_id, texture);
            }
        } else {
            // Create new texture if it doesn't exist
            let new_texture_id = self.create_texture_from_data(device, queue, texture_data)?;
            if new_texture_id != texture_id
                && let Some(texture) = self.remove_texture(new_texture_id)
            {
                self.insert_texture_with_id(texture_id, texture);
            }
        }

        Ok(())
    }

    /// Destroy a texture
    pub fn destroy_texture(&mut self, texture_id: TextureId) {
        let texture_id_u64 = texture_id.id();
        self.remove_texture(texture_id_u64);
        // WGPU textures are automatically cleaned up when dropped
    }

    /// Handle texture updates from Dear ImGui draw data
    ///
    /// This iterates `DrawData::textures()` and applies create/update/destroy requests.
    /// For `WantCreate`, we create the GPU texture, then write the generated id back into
    /// the `ImTextureData` via `set_tex_id()` and mark status `OK` (matching C++ backend).
    /// For `WantUpdates`, if a valid id is not yet assigned (first use), we create now and
    /// assign the id; otherwise we update in place. When textures are recreated or destroyed,
    /// the corresponding cached bind groups in `RenderResources` are invalidated so that
    /// subsequent draws will see the updated views.
    pub fn handle_texture_updates(
        &mut self,
        draw_data: &dear_imgui_rs::render::DrawData,
        device: &Device,
        queue: &Queue,
        render_resources: &mut RenderResources,
    ) {
        for texture_data in draw_data.textures() {
            let status = texture_data.status();
            let current_tex_id = texture_data.tex_id().id();

            match status {
                TextureStatus::WantCreate => {
                    // Create and upload new texture to graphics system
                    // Following the official imgui_impl_wgpu.cpp implementation

                    // If ImGui already had a TexID associated, drop any stale bind group
                    // so that a new one is created the first time we render with it.
                    if current_tex_id != 0 {
                        render_resources.remove_image_bind_group(current_tex_id);
                    }

                    match self.create_texture_from_data(device, queue, texture_data) {
                        Ok(wgpu_texture_id) => {
                            // CRITICAL: Set the texture ID back to Dear ImGui
                            // In the C++ implementation, they use the TextureView pointer as ImTextureID.
                            // In Rust, we can't get the raw pointer, so we use our internal texture ID.
                            // This works because our renderer will map the texture ID to the WGPU texture.
                            let new_texture_id = dear_imgui_rs::TextureId::from(wgpu_texture_id);

                            texture_data.set_tex_id(new_texture_id);

                            // Mark texture as ready
                            texture_data.set_status(TextureStatus::OK);
                        }
                        Err(e) => {
                            println!(
                                "Failed to create texture for ID: {}, error: {}",
                                current_tex_id, e
                            );
                        }
                    }
                }
                TextureStatus::WantUpdates => {
                    let imgui_tex_id = texture_data.tex_id();
                    let internal_id = imgui_tex_id.id();

                    // If we don't have a valid texture id yet (first update) or the
                    // id isn't registered, create it now and write back the TexID,
                    // so this frame (or the next one) can bind the correct texture.
                    if internal_id == 0 || !self.contains_texture(internal_id) {
                        match self.create_texture_from_data(device, queue, texture_data) {
                            Ok(new_id) => {
                                texture_data.set_tex_id(dear_imgui_rs::TextureId::from(new_id));
                                texture_data.set_status(TextureStatus::OK);
                            }
                            Err(_e) => {
                                // Leave it destroyed to avoid retry storm; user can request create again
                                texture_data.set_status(TextureStatus::Destroyed);
                            }
                        }
                    } else {
                        // We are about to update/recreate an existing texture. Invalidate
                        // any cached bind group so it will be rebuilt with the new view.
                        render_resources.remove_image_bind_group(internal_id);

                        // Try in-place sub-rect updates first
                        if self
                            .apply_subrect_updates(queue, texture_data, internal_id)
                            .unwrap_or(false)
                        {
                            texture_data.set_status(TextureStatus::OK);
                        } else if self
                            .update_texture_from_data_with_id(
                                device,
                                queue,
                                texture_data,
                                internal_id,
                            )
                            .is_err()
                        {
                            // If update fails, mark as destroyed
                            texture_data.set_status(TextureStatus::Destroyed);
                        } else {
                            texture_data.set_status(TextureStatus::OK);
                        }
                    }
                }
                TextureStatus::WantDestroy => {
                    // Only destroy when unused frames > 0 (align with official backend behavior)
                    let mut can_destroy = true;
                    unsafe {
                        let raw = texture_data.as_raw();
                        if !raw.is_null() {
                            // If field not present in bindings on some versions, default true
                            #[allow(unused_unsafe)]
                            {
                                // Access UnusedFrames if available
                                // SAFETY: reading a plain field from raw C struct
                                can_destroy = (*raw).UnusedFrames > 0;
                            }
                        }
                    }
                    if can_destroy {
                        let imgui_tex_id = texture_data.tex_id();
                        let internal_id = imgui_tex_id.id();
                        // Remove from texture cache and any associated bind groups
                        self.remove_texture(internal_id);
                        self.clear_custom_sampler_for_texture(internal_id);
                        render_resources.remove_image_bind_group(internal_id);
                        texture_data.set_status(TextureStatus::Destroyed);
                    }
                }
                TextureStatus::OK | TextureStatus::Destroyed => {
                    // No action needed
                }
            }
        }
    }

    /// Update a single texture based on its status
    ///
    /// This corresponds to ImGui_ImplWGPU_UpdateTexture in the C++ implementation.
    ///
    /// # Returns
    ///
    /// Returns a `TextureUpdateResult` that contains the operation result and
    /// any status/ID updates that need to be applied to the texture data.
    /// This follows Rust's principle of explicit state management.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui_wgpu::*;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut texture_manager = WgpuTextureManager::new();
    /// # let device = todo!();
    /// # let queue = todo!();
    /// # let mut texture_data = dear_imgui_rs::TextureData::new();
    /// let result = texture_manager.update_single_texture(&texture_data, &device, &queue)?;
    /// result.apply_to(&mut texture_data);
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_single_texture(
        &mut self,
        texture_data: &dear_imgui_rs::TextureData,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<TextureUpdateResult> {
        match texture_data.status() {
            TextureStatus::WantCreate => {
                let texture_id = self.create_texture_from_data(device, queue, texture_data)?;
                Ok(TextureUpdateResult::Created {
                    texture_id: TextureId::from(texture_id),
                })
            }
            TextureStatus::WantUpdates => {
                let internal_id = texture_data.tex_id().id();
                if internal_id == 0 || !self.contains_texture(internal_id) {
                    // No valid ID yet: create now and return Created so caller can set TexID
                    let texture_id = self.create_texture_from_data(device, queue, texture_data)?;
                    Ok(TextureUpdateResult::Created {
                        texture_id: TextureId::from(texture_id),
                    })
                } else {
                    match self.update_texture_from_data_with_id(
                        device,
                        queue,
                        texture_data,
                        internal_id,
                    ) {
                        Ok(_) => Ok(TextureUpdateResult::Updated),
                        Err(e) => Err(e),
                    }
                }
            }
            TextureStatus::WantDestroy => {
                self.destroy_texture(texture_data.tex_id());
                Ok(TextureUpdateResult::Destroyed)
            }
            TextureStatus::OK | TextureStatus::Destroyed => Ok(TextureUpdateResult::NoAction),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::texture::{TextureData, TextureFormat as ImFormat, TextureRect};

    #[test]
    fn texture_update_result_apply_to_sets_status_and_id() {
        let mut tex = TextureData::new();

        // Created -> sets TexID and OK
        TextureUpdateResult::Created {
            texture_id: TextureId::from(42u64),
        }
        .apply_to(&mut tex);
        assert_eq!(tex.status(), TextureStatus::OK);
        assert_eq!(tex.tex_id().id(), 42);

        // Updated -> only status OK
        TextureUpdateResult::Updated.apply_to(&mut tex);
        assert_eq!(tex.status(), TextureStatus::OK);
        assert_eq!(tex.tex_id().id(), 42);

        // Destroyed -> status Destroyed
        // ImGui's ImTextureData::SetStatus has special semantics:
        // setting Destroyed while WantDestroyNextFrame is false will immediately flip back to WantCreate.
        // When honoring a requested destroy, WantDestroyNextFrame is expected to be true.
        unsafe {
            (*tex.as_raw_mut()).WantDestroyNextFrame = true;
        }
        TextureUpdateResult::Destroyed.apply_to(&mut tex);
        assert_eq!(tex.status(), TextureStatus::Destroyed);

        // Failed -> also marks Destroyed
        // In the general case (not a requested destroy), SetStatus(Destroyed) translates to WantCreate.
        unsafe {
            (*tex.as_raw_mut()).WantDestroyNextFrame = false;
        }
        TextureUpdateResult::Failed.apply_to(&mut tex);
        assert_eq!(tex.status(), TextureStatus::WantCreate);

        // NoAction -> leaves state unchanged
        TextureUpdateResult::NoAction.apply_to(&mut tex);
        assert_eq!(tex.status(), TextureStatus::WantCreate);
    }

    #[test]
    fn convert_subrect_to_rgba_rgba32_full_rect() {
        let mut tex = TextureData::new();
        let width = 2;
        let height = 2;
        tex.create(ImFormat::RGBA32, width, height);

        // 2x2 RGBA pixels: row-major
        let pixels: [u8; 16] = [
            10, 20, 30, 40, // (0,0)
            50, 60, 70, 80, // (1,0)
            90, 100, 110, 120, // (0,1)
            130, 140, 150, 160, // (1,1)
        ];
        tex.set_data(&pixels);

        let rect = TextureRect {
            x: 0,
            y: 0,
            w: width as u16,
            h: height as u16,
        };

        let out = WgpuTextureManager::convert_subrect_to_rgba(&tex, rect).expect("expected data");
        assert_eq!(out, pixels);
    }

    #[test]
    fn convert_subrect_to_rgba_alpha8_full_rect() {
        let mut tex = TextureData::new();
        let width = 2;
        let height = 2;
        tex.create(ImFormat::Alpha8, width, height);

        // 2x2 alpha-only pixels
        let alphas: [u8; 4] = [0, 64, 128, 255];
        tex.set_data(&alphas);

        let rect = TextureRect {
            x: 0,
            y: 0,
            w: width as u16,
            h: height as u16,
        };

        let out = WgpuTextureManager::convert_subrect_to_rgba(&tex, rect).expect("expected data");
        // Each alpha should expand to [255,255,255,a]
        assert_eq!(
            out,
            vec![
                255, 255, 255, 0, // a=0
                255, 255, 255, 64, // a=64
                255, 255, 255, 128, // a=128
                255, 255, 255, 255, // a=255
            ]
        );
    }

    #[test]
    fn convert_subrect_to_rgba_out_of_bounds_returns_none() {
        let mut tex = TextureData::new();
        tex.create(ImFormat::RGBA32, 2, 2);
        let rect = TextureRect {
            x: 10,
            y: 10,
            w: 1,
            h: 1,
        };
        assert!(WgpuTextureManager::convert_subrect_to_rgba(&tex, rect).is_none());
    }
}
