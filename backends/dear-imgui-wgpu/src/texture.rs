//! Texture management for the WGPU renderer
//!
//! This module handles texture creation, updates, and management,
//! integrating with Dear ImGui's modern texture system.

use crate::{RendererError, RendererResult};
use dear_imgui::{TextureData, TextureFormat as ImGuiTextureFormat, TextureId, TextureStatus};
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
#[derive(Debug, Default)]
pub struct WgpuTextureManager {
    /// Map from texture ID to WGPU texture
    textures: HashMap<u64, WgpuTexture>,
    /// Next available texture ID
    next_id: u64,
}

impl WgpuTextureManager {
    /// Convert a sub-rectangle of ImGui texture pixels into a tightly packed RGBA8 buffer
    fn convert_subrect_to_rgba(
        texture_data: &TextureData,
        rect: dear_imgui::texture::TextureRect,
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
        let mut rects: Vec<dear_imgui::texture::TextureRect> = texture_data.updates().collect();
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
    /// assign the id; otherwise we update in place.
    pub fn handle_texture_updates(
        &mut self,
        draw_data: &dear_imgui::render::DrawData,
        device: &Device,
        queue: &Queue,
    ) {
        for texture_data in draw_data.textures() {
            let status = texture_data.status();
            let current_tex_id = texture_data.tex_id().id();

            match status {
                TextureStatus::WantCreate => {
                    // Create and upload new texture to graphics system
                    // Following the official imgui_impl_wgpu.cpp implementation

                    match self.create_texture_from_data(device, queue, texture_data) {
                        Ok(wgpu_texture_id) => {
                            // CRITICAL: Set the texture ID back to Dear ImGui
                            // In the C++ implementation, they use the TextureView pointer as ImTextureID.
                            // In Rust, we can't get the raw pointer, so we use our internal texture ID.
                            // This works because our renderer will map the texture ID to the WGPU texture.
                            let new_texture_id = dear_imgui::TextureId::from(wgpu_texture_id);

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
                    // so this frame (or the next) can bind the correct texture.
                    if internal_id == 0 || !self.contains_texture(internal_id) {
                        match self.create_texture_from_data(device, queue, texture_data) {
                            Ok(new_id) => {
                                texture_data.set_tex_id(dear_imgui::TextureId::from(new_id));
                                texture_data.set_status(TextureStatus::OK);
                            }
                            Err(_e) => {
                                // Leave it destroyed to avoid retry storm; user can request create again
                                texture_data.set_status(TextureStatus::Destroyed);
                            }
                        }
                    } else {
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
                        // Remove from cache
                        self.remove_texture(internal_id);
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
    /// # let mut texture_data = dear_imgui::TextureData::new();
    /// let result = texture_manager.update_single_texture(&texture_data, &device, &queue)?;
    /// result.apply_to(&mut texture_data);
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_single_texture(
        &mut self,
        texture_data: &dear_imgui::TextureData,
        device: &Device,
        queue: &Queue,
    ) -> Result<TextureUpdateResult, String> {
        match texture_data.status() {
            TextureStatus::WantCreate => {
                match self.create_texture_from_data(device, queue, texture_data) {
                    Ok(texture_id) => Ok(TextureUpdateResult::Created {
                        texture_id: TextureId::from(texture_id),
                    }),
                    Err(e) => Err(format!("Failed to create texture: {}", e)),
                }
            }
            TextureStatus::WantUpdates => {
                let internal_id = texture_data.tex_id().id();
                if internal_id == 0 || !self.contains_texture(internal_id) {
                    // No valid ID yet: create now and return Created so caller can set TexID
                    match self.create_texture_from_data(device, queue, texture_data) {
                        Ok(texture_id) => Ok(TextureUpdateResult::Created {
                            texture_id: TextureId::from(texture_id),
                        }),
                        Err(e) => Err(format!("Failed to create texture: {}", e)),
                    }
                } else {
                    match self.update_texture_from_data_with_id(
                        device,
                        queue,
                        texture_data,
                        internal_id,
                    ) {
                        Ok(_) => Ok(TextureUpdateResult::Updated),
                        Err(_e) => Ok(TextureUpdateResult::Failed),
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
