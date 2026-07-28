//! Shader and render-pipeline specialization for the Bevy renderer.

use super::*;

/// Stable handle for the embedded Dear ImGui Bevy renderer shader.
pub const IMGUI_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("2c893cad-60d2-4e92-8544-4ab807ed9c5a");

/// Vertex shader entry point used by the Bevy-native ImGui pipeline.
pub const IMGUI_VERTEX_ENTRY_POINT: &str = "vs_main";
/// Fragment shader entry point used by the Bevy-native ImGui pipeline.
pub const IMGUI_FRAGMENT_ENTRY_POINT: &str = "fs_main";

/// WGSL source for the Bevy-native Dear ImGui renderer.
///
/// BEVY-090 keeps this shader local to the Bevy backend instead of reusing
/// `dear-imgui-wgpu`, because Bevy owns render schedules, target formats, and pipeline
/// specialization.
pub const IMGUI_SHADER_SOURCE: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct ImguiUniforms {
    mvp: mat4x4<f32>,
    gamma: f32,
    _padding: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: ImguiUniforms;

@group(1) @binding(0)
var imgui_texture: texture_2d<f32>;

@group(1) @binding(1)
var imgui_sampler: sampler;

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
    let color = in.color * textureSample(imgui_texture, imgui_sampler, in.uv);
    let corrected = pow(color.rgb, vec3<f32>(uniforms.gamma));
    return vec4<f32>(corrected, color.a);
}
"#;

/// Per-frame uniform data used by the Dear ImGui shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ImguiUniforms {
    /// Orthographic projection matrix that maps ImGui display coordinates to clip space.
    pub mvp: [[f32; 4]; 4],
    /// Gamma used to linearize colors before writing into the render target.
    pub gamma: f32,
    /// Padding to satisfy WGSL uniform layout.
    pub _padding: [f32; 7],
}

impl ImguiUniforms {
    /// Create uniforms for an ImGui draw data display rectangle.
    #[must_use]
    pub fn from_display_rect(display_pos: [f32; 2], display_size: [f32; 2]) -> Self {
        let left = display_pos[0];
        let right = display_pos[0] + display_size[0];
        let top = display_pos[1];
        let bottom = display_pos[1] + display_size[1];
        Self {
            mvp: [
                [2.0 / (right - left), 0.0, 0.0, 0.0],
                [0.0, 2.0 / (top - bottom), 0.0, 0.0],
                [0.0, 0.0, 0.5, 0.0],
                [
                    (right + left) / (left - right),
                    (top + bottom) / (bottom - top),
                    0.5,
                    1.0,
                ],
            ],
            gamma: 1.0,
            _padding: [0.0; 7],
        }
    }

    /// Set the gamma value used by the fragment shader.
    #[must_use]
    pub fn with_gamma(mut self, gamma: f32) -> Self {
        self.gamma = gamma;
        self
    }

    /// Gamma correction value for a given render target format and Bevy compositing space.
    #[must_use]
    pub fn gamma_for_target(
        format: TextureFormat,
        compositing_space: Option<CompositingSpace>,
    ) -> f32 {
        if format.is_srgb() || compositing_space == Some(CompositingSpace::Srgb) {
            2.2
        } else {
            1.0
        }
    }
}

/// GPU vertex layout used by the Bevy-native ImGui renderer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ImguiGpuVertex {
    /// Clip-space input position before the ImGui orthographic transform.
    pub position: [f32; 2],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Packed Dear ImGui RGBA color.
    pub color: u32,
}

impl From<imgui::render::DrawVert> for ImguiGpuVertex {
    fn from(value: imgui::render::DrawVert) -> Self {
        Self {
            position: value.pos,
            uv: value.uv,
            color: value.col,
        }
    }
}

/// Vertex buffer layout consumed by the Bevy-native ImGui render pipeline.
#[must_use]
pub fn imgui_vertex_buffer_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: size_of::<ImguiGpuVertex>() as BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            VertexAttribute {
                format: VertexFormat::Unorm8x4,
                offset: 16,
                shader_location: 2,
            },
        ],
    }
}

/// Pipeline specialization key for one Bevy view target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ImguiPipelineKey {
    pub target_format: TextureFormat,
}

/// Bevy render pipeline descriptor source for Dear ImGui overlays.
#[derive(Resource, Clone)]
pub struct ImguiRenderPipeline {
    shader: Handle<Shader>,
    common_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
}

impl ImguiRenderPipeline {
    /// Bind group layout for camera uniforms.
    #[must_use]
    pub fn common_layout(&self) -> &BindGroupLayoutDescriptor {
        &self.common_layout
    }

    /// Bind group layout for a single ImGui texture binding.
    #[must_use]
    pub fn texture_layout(&self) -> &BindGroupLayoutDescriptor {
        &self.texture_layout
    }
}

impl Default for ImguiRenderPipeline {
    fn default() -> Self {
        let common_layout = BindGroupLayoutDescriptor::new(
            "dear_imgui_bevy_common_layout",
            &[bevy_render::render_resource::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(size_of::<ImguiUniforms>() as u64),
                },
                count: None,
            }],
        );
        let texture_layout = BindGroupLayoutDescriptor::new(
            "dear_imgui_bevy_texture_layout",
            &[
                bevy_render::render_resource::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                bevy_render::render_resource::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );
        Self {
            shader: IMGUI_SHADER_HANDLE,
            common_layout,
            texture_layout,
        }
    }
}

impl SpecializedRenderPipeline for ImguiRenderPipeline {
    type Key = ImguiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("dear_imgui_bevy_pipeline".into()),
            layout: vec![self.common_layout.clone(), self.texture_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some(IMGUI_VERTEX_ENTRY_POINT.into()),
                buffers: vec![imgui_vertex_buffer_layout()],
                ..Default::default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some(IMGUI_FRAGMENT_ENTRY_POINT.into()),
                targets: vec![Some(ColorTargetState {
                    format: key.target_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            multisample: MultisampleState {
                count: 1,
                ..Default::default()
            },
            zero_initialize_workgroup_memory: true,
            ..Default::default()
        }
    }
}

pub(super) fn queue_imgui_pipelines(
    prepared: Res<ImguiPreparedRenderFrame>,
    views: Query<&ExtractedView>,
    pipeline_cache: Option<Res<PipelineCache>>,
    pipeline: Res<ImguiRenderPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ImguiRenderPipeline>>,
    mut queued: ResMut<ImguiQueuedPipelines>,
) {
    queued.by_view.clear();

    let Some(pipeline_cache) = pipeline_cache else {
        return;
    };

    let targets = prepared
        .draws()
        .iter()
        .map(|draw| (draw.view, draw.target_format))
        .collect::<HashSet<_>>();

    for view in &views {
        let view_id = view.retained_view_entity;
        if !targets.contains(&(view_id, view.target_format)) {
            continue;
        }
        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &pipeline,
            ImguiPipelineKey {
                target_format: view.target_format,
            },
        );
        queued.by_view.insert(view_id, pipeline_id);
    }
}
