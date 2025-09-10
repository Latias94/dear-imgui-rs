//! Shader management for the WGPU renderer
//!
//! This module handles shader creation and management, including the WGSL shaders
//! and pipeline creation logic.

use crate::{RendererError, RendererResult};
use dear_imgui::render::DrawVert;
use std::mem::size_of;
use wgpu::*;

/// Vertex shader entry point
pub const VS_ENTRY_POINT: &str = "vs_main";
/// Fragment shader entry point
pub const FS_ENTRY_POINT: &str = "fs_main";

/// WGSL shader source
///
/// This includes both vertex and fragment shaders with gamma correction support,
/// similar to the embedded shaders in imgui_impl_wgpu.cpp
pub const SHADER_SOURCE: &str = r#"
// Dear ImGui WGSL Shader
// Vertex and fragment shaders for rendering Dear ImGui draw data

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

struct Uniforms {
    mvp: mat4x4<f32>,
    gamma: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var u_sampler: sampler;

@group(1) @binding(0)
var u_texture: texture_2d<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.mvp * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = in.color * textureSample(u_texture, u_sampler, in.uv);
    let corrected_color = pow(color.rgb, vec3<f32>(uniforms.gamma));
    return vec4<f32>(corrected_color, color.a);
}
"#;

/// Shader manager
pub struct ShaderManager {
    shader_module: Option<ShaderModule>,
}

impl ShaderManager {
    /// Create a new shader manager
    pub fn new() -> Self {
        Self {
            shader_module: None,
        }
    }

    /// Initialize shaders
    pub fn initialize(&mut self, device: &Device) -> RendererResult<()> {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Dear ImGui Shader"),
            source: ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        self.shader_module = Some(shader_module);
        Ok(())
    }

    /// Get the shader module
    pub fn shader_module(&self) -> Option<&ShaderModule> {
        self.shader_module.as_ref()
    }

    /// Get the shader module reference (with error handling)
    pub fn get_shader_module(&self) -> RendererResult<&ShaderModule> {
        self.shader_module.as_ref().ok_or_else(|| {
            RendererError::ShaderCompilationFailed("Shader module not initialized".to_string())
        })
    }

    /// Check if shaders are initialized
    pub fn is_initialized(&self) -> bool {
        self.shader_module.is_some()
    }
}

impl Default for ShaderManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create vertex buffer layout for Dear ImGui vertices
pub fn create_vertex_buffer_layout() -> VertexBufferLayout<'static> {
    const VERTEX_ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // uv
        2 => Unorm8x4    // color
    ];

    VertexBufferLayout {
        array_stride: size_of::<DrawVert>() as BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: VERTEX_ATTRIBUTES,
    }
}

/// Create vertex state for render pipeline
pub fn create_vertex_state<'a>(
    shader_module: &'a ShaderModule,
    buffers: &'a [VertexBufferLayout],
) -> VertexState<'a> {
    VertexState {
        module: shader_module,
        entry_point: Some(VS_ENTRY_POINT),
        compilation_options: Default::default(),
        buffers,
    }
}

/// Create fragment state for render pipeline
pub fn create_fragment_state<'a>(
    shader_module: &'a ShaderModule,
    targets: &'a [Option<ColorTargetState>],
    _use_gamma_correction: bool,
) -> FragmentState<'a> {
    let entry_point = FS_ENTRY_POINT;

    FragmentState {
        module: shader_module,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        targets,
    }
}

/// Create bind group layouts for the pipeline
pub fn create_bind_group_layouts(device: &Device) -> (BindGroupLayout, BindGroupLayout) {
    // Common bind group layout (uniforms + sampler)
    let common_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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

    // Image bind group layout (texture)
    let image_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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

    (common_layout, image_layout)
}
