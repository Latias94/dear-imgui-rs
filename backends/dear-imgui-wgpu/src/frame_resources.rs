//! Frame resources management for the WGPU renderer
//!
//! This module handles per-frame resources like vertex and index buffers,
//! corresponding to the FrameResources struct in imgui_impl_wgpu.cpp

use crate::{RenderResources, RendererError, RendererResult, UniformBuffer};
use dear_imgui_rs::render::{DrawIdx, DrawVert};
use wgpu::*;

/// Memory alignment function (equivalent to MEMALIGN macro in C++)
/// Aligns size to the specified alignment boundary
fn align_size(size: usize, alignment: usize) -> usize {
    (size + alignment - 1) & !(alignment - 1)
}

/// Per-frame resources
///
/// This corresponds to the FrameResources struct in the C++ implementation.
/// Each frame in flight has its own set of vertex and index buffers.
pub struct FrameResources {
    /// GPU vertex buffer
    pub vertex_buffer: Option<Buffer>,
    /// GPU index buffer  
    pub index_buffer: Option<Buffer>,
    /// Host-side vertex buffer (for staging)
    pub vertex_buffer_host: Option<Vec<u8>>,
    /// Host-side index buffer (for staging)
    pub index_buffer_host: Option<Vec<u8>>,
    /// Current vertex buffer size in vertices
    pub vertex_buffer_size: usize,
    /// Current index buffer size in indices
    pub index_buffer_size: usize,
    uniform_buffer: Option<UniformBuffer>,
    nearest_common_bind_group: Option<BindGroup>,
}

impl FrameResources {
    /// Create new empty frame resources
    pub fn new() -> Self {
        Self {
            vertex_buffer: None,
            index_buffer: None,
            vertex_buffer_host: None,
            index_buffer_host: None,
            vertex_buffer_size: 0,
            index_buffer_size: 0,
            uniform_buffer: None,
            nearest_common_bind_group: None,
        }
    }

    pub(crate) fn ensure_render_bindings(
        &mut self,
        device: &Device,
        render_resources: &RenderResources,
    ) -> RendererResult<()> {
        if self.uniform_buffer.is_none() || self.nearest_common_bind_group.is_none() {
            let (uniform, nearest) = render_resources.create_frame_bindings(device)?;
            self.uniform_buffer = Some(uniform);
            self.nearest_common_bind_group = Some(nearest);
        }
        Ok(())
    }

    /// Ensure vertex buffer can hold the required number of vertices
    pub fn ensure_vertex_buffer_capacity(
        &mut self,
        device: &Device,
        required_vertices: usize,
    ) -> RendererResult<()> {
        if self.vertex_buffer.is_none() || self.vertex_buffer_size < required_vertices {
            // Add some extra capacity to avoid frequent reallocations
            let new_size = (required_vertices + 5000).max(self.vertex_buffer_size * 2);

            // Create new GPU buffer with proper alignment
            let buffer_size = align_size(new_size * std::mem::size_of::<DrawVert>(), 4);
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Dear ImGui Vertex Buffer"),
                size: buffer_size as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Create new host buffer
            let host_buffer = vec![0u8; new_size * std::mem::size_of::<DrawVert>()];

            self.vertex_buffer = Some(buffer);
            self.vertex_buffer_host = Some(host_buffer);
            self.vertex_buffer_size = new_size;
        }

        Ok(())
    }

    /// Ensure index buffer can hold the required number of indices
    pub fn ensure_index_buffer_capacity(
        &mut self,
        device: &Device,
        required_indices: usize,
    ) -> RendererResult<()> {
        if self.index_buffer.is_none() || self.index_buffer_size < required_indices {
            // Add some extra capacity to avoid frequent reallocations
            let new_size = (required_indices + 10000).max(self.index_buffer_size * 2);

            // Create new GPU buffer with proper alignment
            let buffer_size = align_size(new_size * std::mem::size_of::<DrawIdx>(), 4);
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Dear ImGui Index Buffer"),
                size: buffer_size as u64,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Create new host buffer
            let host_buffer = vec![0u8; new_size * std::mem::size_of::<DrawIdx>()];

            self.index_buffer = Some(buffer);
            self.index_buffer_host = Some(host_buffer);
            self.index_buffer_size = new_size;
        }

        Ok(())
    }

    /// Upload vertex data to the GPU buffer
    pub fn upload_vertex_data(
        &mut self,
        queue: &Queue,
        vertices: &[DrawVert],
    ) -> RendererResult<()> {
        let vertex_buffer = self.vertex_buffer.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState("Vertex buffer not initialized".to_string())
        })?;

        let required_bytes = std::mem::size_of_val(vertices);
        let aligned_size = align_size(required_bytes, 4);

        let host_buffer = self.vertex_buffer_host.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Vertex host buffer not initialized".to_string())
        })?;
        if aligned_size > host_buffer.len() {
            return Err(RendererError::InvalidRenderState(
                "Vertex host buffer capacity is too small".to_string(),
            ));
        }

        // Avoid reinterpreting `DrawVert` as bytes: that can read uninitialized padding bytes.
        // Pack vertices explicitly in the same layout used by Dear ImGui (pos, uv, col).
        host_buffer[..aligned_size].fill(0);
        const VERT_STRIDE: usize = std::mem::size_of::<DrawVert>();
        for (i, v) in vertices.iter().enumerate() {
            let base = i * VERT_STRIDE;
            host_buffer[base..base + 4].copy_from_slice(&v.pos[0].to_ne_bytes());
            host_buffer[base + 4..base + 8].copy_from_slice(&v.pos[1].to_ne_bytes());
            host_buffer[base + 8..base + 12].copy_from_slice(&v.uv[0].to_ne_bytes());
            host_buffer[base + 12..base + 16].copy_from_slice(&v.uv[1].to_ne_bytes());
            host_buffer[base + 16..base + 20].copy_from_slice(&v.col.to_ne_bytes());
        }

        // Upload to GPU with proper alignment
        queue.write_buffer(vertex_buffer, 0, &host_buffer[..aligned_size]);
        Ok(())
    }

    /// Upload index data to the GPU buffer
    pub fn upload_index_data(&mut self, queue: &Queue, indices: &[DrawIdx]) -> RendererResult<()> {
        let index_buffer = self.index_buffer.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState("Index buffer not initialized".to_string())
        })?;

        let required_bytes = std::mem::size_of_val(indices);
        let aligned_size = align_size(required_bytes, 4);

        let host_buffer = self.index_buffer_host.as_mut().ok_or_else(|| {
            RendererError::InvalidRenderState("Index host buffer not initialized".to_string())
        })?;
        if aligned_size > host_buffer.len() {
            return Err(RendererError::InvalidRenderState(
                "Index host buffer capacity is too small".to_string(),
            ));
        }

        host_buffer[..aligned_size].fill(0);
        for (i, &idx) in indices.iter().enumerate() {
            let bytes = idx.to_ne_bytes();
            let base = i * std::mem::size_of::<DrawIdx>();
            host_buffer[base..base + 2].copy_from_slice(&bytes);
        }

        // Upload to GPU with proper alignment
        queue.write_buffer(index_buffer, 0, &host_buffer[..aligned_size]);
        Ok(())
    }

    /// Get the vertex buffer for rendering
    pub fn vertex_buffer(&self) -> Option<&Buffer> {
        self.vertex_buffer.as_ref()
    }

    /// Get the index buffer for rendering
    pub fn index_buffer(&self) -> Option<&Buffer> {
        self.index_buffer.as_ref()
    }

    pub(crate) fn uniform_buffer(&self) -> RendererResult<&UniformBuffer> {
        self.uniform_buffer.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState("Frame uniform buffer not initialized".to_owned())
        })
    }

    pub(crate) fn nearest_common_bind_group(&self) -> RendererResult<&BindGroup> {
        self.nearest_common_bind_group.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState(
                "Frame nearest sampler bind group not initialized".to_owned(),
            )
        })
    }
}

impl Default for FrameResources {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct FrameResourceArena {
    passes: Vec<FrameResources>,
    next_pass: usize,
}

impl FrameResourceArena {
    pub(crate) const fn new() -> Self {
        Self {
            passes: Vec::new(),
            next_pass: 0,
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.next_pass = 0;
    }

    pub(crate) fn acquire(&mut self) -> &mut FrameResources {
        let index = self.next_pass;
        self.next_pass += 1;
        if index == self.passes.len() {
            self.passes.push(FrameResources::new());
        }
        &mut self.passes[index]
    }
}

#[cfg(test)]
mod arena_tests {
    use super::FrameResourceArena;

    #[test]
    fn frame_arena_gives_each_pass_a_slot_and_reuses_it_only_after_reset() {
        let mut arena = FrameResourceArena::new();
        arena.begin_frame();
        let _ = arena.acquire();
        let _ = arena.acquire();
        assert_eq!(arena.passes.len(), 2);
        assert_eq!(arena.next_pass, 2);

        arena.begin_frame();
        let _ = arena.acquire();
        assert_eq!(arena.passes.len(), 2);
        assert_eq!(arena.next_pass, 1);
    }
}
