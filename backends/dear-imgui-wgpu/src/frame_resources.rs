//! Frame resources management for the WGPU renderer
//!
//! This module handles per-frame resources like vertex and index buffers,
//! corresponding to the FrameResources struct in imgui_impl_wgpu.cpp

use dear_imgui::render::{DrawIdx, DrawVert};
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
        }
    }

    /// Ensure vertex buffer can hold the required number of vertices
    pub fn ensure_vertex_buffer_capacity(
        &mut self,
        device: &Device,
        required_vertices: usize,
    ) -> Result<(), String> {
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
    ) -> Result<(), String> {
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
    ) -> Result<(), String> {
        let vertex_buffer = self
            .vertex_buffer
            .as_ref()
            .ok_or("Vertex buffer not initialized")?;

        // Convert vertices to bytes
        let vertex_bytes = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                std::mem::size_of_val(vertices),
            )
        };

        // Copy to host buffer first
        if let Some(ref mut host_buffer) = self.vertex_buffer_host {
            if vertex_bytes.len() <= host_buffer.len() {
                host_buffer[..vertex_bytes.len()].copy_from_slice(vertex_bytes);
            }
        }

        // Upload to GPU with proper alignment
        let aligned_size = align_size(vertex_bytes.len(), 4);
        if aligned_size > vertex_bytes.len() {
            // Need to pad the data to meet alignment requirements
            let mut aligned_data = vertex_bytes.to_vec();
            aligned_data.resize(aligned_size, 0);
            queue.write_buffer(vertex_buffer, 0, &aligned_data);
        } else {
            queue.write_buffer(vertex_buffer, 0, vertex_bytes);
        }
        Ok(())
    }

    /// Upload index data to the GPU buffer
    pub fn upload_index_data(&mut self, queue: &Queue, indices: &[DrawIdx]) -> Result<(), String> {
        let index_buffer = self
            .index_buffer
            .as_ref()
            .ok_or("Index buffer not initialized")?;

        // Convert indices to bytes
        let index_bytes = unsafe {
            std::slice::from_raw_parts(
                indices.as_ptr() as *const u8,
                std::mem::size_of_val(indices),
            )
        };

        // Copy to host buffer first
        if let Some(ref mut host_buffer) = self.index_buffer_host {
            if index_bytes.len() <= host_buffer.len() {
                host_buffer[..index_bytes.len()].copy_from_slice(index_bytes);
            }
        }

        // Upload to GPU with proper alignment
        let aligned_size = align_size(index_bytes.len(), 4);
        if aligned_size > index_bytes.len() {
            // Need to pad the data to meet alignment requirements
            let mut aligned_data = index_bytes.to_vec();
            aligned_data.resize(aligned_size, 0);
            queue.write_buffer(index_buffer, 0, &aligned_data);
        } else {
            queue.write_buffer(index_buffer, 0, index_bytes);
        }
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

    /// Check if buffers are ready for rendering
    pub fn is_ready(&self) -> bool {
        self.vertex_buffer.is_some() && self.index_buffer.is_some()
    }

    /// Get buffer statistics for debugging
    pub fn stats(&self) -> FrameResourcesStats {
        FrameResourcesStats {
            vertex_buffer_size: self.vertex_buffer_size,
            index_buffer_size: self.index_buffer_size,
            vertex_buffer_bytes: self.vertex_buffer_size * std::mem::size_of::<DrawVert>(),
            index_buffer_bytes: self.index_buffer_size * std::mem::size_of::<DrawIdx>(),
        }
    }
}

impl Default for FrameResources {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for frame resources
#[derive(Debug, Clone)]
pub struct FrameResourcesStats {
    pub vertex_buffer_size: usize,
    pub index_buffer_size: usize,
    pub vertex_buffer_bytes: usize,
    pub index_buffer_bytes: usize,
}
