//! Uniform buffer management for the WGPU renderer
//!
//! This module handles the uniform data structure and buffer management,
//! corresponding to the Uniforms struct in imgui_impl_wgpu.cpp

use bytemuck::{Pod, Zeroable};
use wgpu::*;

/// Memory alignment function (equivalent to MEMALIGN macro in C++)
/// Aligns size to the specified alignment boundary
fn align_size(size: usize, alignment: usize) -> usize {
    (size + alignment - 1) & !(alignment - 1)
}

/// Uniform data structure
///
/// This corresponds to the Uniforms struct in the C++ implementation.
/// Contains the MVP matrix and gamma correction value.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    /// Model-View-Projection matrix (4x4 f32 matrix)
    pub mvp: [[f32; 4]; 4],
    /// Gamma correction value
    pub gamma: f32,
    /// Padding to ensure proper alignment
    pub _padding: [f32; 3],
}

impl Uniforms {
    /// Create new uniforms with identity matrix and default gamma
    pub fn new() -> Self {
        Self {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            gamma: 1.0,
            _padding: [0.0; 3],
        }
    }

    /// Create orthographic projection matrix for Dear ImGui
    ///
    /// This matches the matrix calculation in ImGui_ImplWGPU_SetupRenderState
    pub fn create_orthographic_matrix(
        display_pos: [f32; 2],
        display_size: [f32; 2],
    ) -> [[f32; 4]; 4] {
        let l = display_pos[0];
        let r = display_pos[0] + display_size[0];
        let t = display_pos[1];
        let b = display_pos[1] + display_size[1];

        [
            [2.0 / (r - l), 0.0, 0.0, 0.0],
            [0.0, 2.0 / (t - b), 0.0, 0.0],
            [0.0, 0.0, 0.5, 0.0],
            [(r + l) / (l - r), (t + b) / (b - t), 0.5, 1.0],
        ]
    }

    /// Determine gamma value based on texture format
    ///
    /// This matches the gamma detection logic in ImGui_ImplWGPU_SetupRenderState
    pub fn gamma_for_format(format: TextureFormat) -> f32 {
        match format {
            // sRGB formats need gamma correction
            TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Bgra8UnormSrgb
            | TextureFormat::Bc1RgbaUnormSrgb
            | TextureFormat::Bc2RgbaUnormSrgb
            | TextureFormat::Bc3RgbaUnormSrgb
            | TextureFormat::Bc7RgbaUnormSrgb
            | TextureFormat::Etc2Rgb8UnormSrgb
            | TextureFormat::Etc2Rgb8A1UnormSrgb
            | TextureFormat::Etc2Rgba8UnormSrgb
            | TextureFormat::Astc {
                block: _,
                channel: AstcChannel::UnormSrgb,
            } => 2.2,
            // Linear formats don't need gamma correction
            _ => 1.0,
        }
    }

    /// Update the MVP matrix
    pub fn set_mvp(&mut self, mvp: [[f32; 4]; 4]) {
        self.mvp = mvp;
    }

    /// Update the gamma value
    pub fn set_gamma(&mut self, gamma: f32) {
        self.gamma = gamma;
    }

    /// Update both MVP and gamma
    pub fn update(&mut self, mvp: [[f32; 4]; 4], gamma: f32) {
        self.mvp = mvp;
        self.gamma = gamma;
    }
}

impl Default for Uniforms {
    fn default() -> Self {
        Self::new()
    }
}

/// Uniform buffer manager
pub struct UniformBuffer {
    buffer: Buffer,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
}

impl UniformBuffer {
    /// Create a new uniform buffer
    pub fn new(device: &Device, sampler: &Sampler) -> Self {
        // Create the uniform buffer with proper alignment (16 bytes for uniforms)
        let buffer_size = align_size(std::mem::size_of::<Uniforms>(), 16);
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dear ImGui Uniform Buffer"),
            size: buffer_size as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout (uniform buffer + sampler)
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Dear ImGui Common Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Dear ImGui Common Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        });

        Self {
            buffer,
            bind_group,
            bind_group_layout,
        }
    }

    /// Update the uniform buffer with new data
    pub fn update(&self, queue: &Queue, uniforms: &Uniforms) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Get the bind group for rendering
    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }

    /// Get the bind group layout
    pub fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the buffer
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }
}
