//! Dear ImGui rendering pipeline implementation

use bevy::{
    prelude::*,
    render::{
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            BindGroupLayout, BindGroupLayoutEntries, BlendState, ColorTargetState, ColorWrites,
            FragmentState, MultisampleState, PrimitiveState, RenderPipelineDescriptor,
            SamplerBindingType, ShaderStages, SpecializedRenderPipeline, TextureFormat,
            TextureSampleType, VertexFormat, VertexState, VertexStepMode,
        },
        renderer::RenderDevice,
        view::ViewTarget,
    },
};
use bevy_mesh::VertexBufferLayout;

use crate::render_impl::IMGUI_SHADER_HANDLE;

/// Transform uniform buffer data
#[derive(encase::ShaderType, Default, Debug, Clone, Copy)]
pub struct ImguiTransform {
    /// Scale for rendering ImGui shapes
    pub scale: Vec2,
    /// Translation for rendering ImGui shapes
    pub translation: Vec2,
}

/// Dear ImGui render pipeline
#[derive(Resource)]
pub struct ImguiPipeline {
    /// Transform bind group layout
    pub transform_bind_group_layout: BindGroupLayout,
    /// Texture bind group layout  
    pub texture_bind_group_layout: BindGroupLayout,
}

impl FromWorld for ImguiPipeline {
    fn from_world(render_world: &mut World) -> Self {
        let render_device = render_world.resource::<RenderDevice>();

        let transform_bind_group_layout = render_device.create_bind_group_layout(
            "imgui_transform_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX,
                uniform_buffer::<ImguiTransform>(true),
            ),
        );

        let texture_bind_group_layout = render_device.create_bind_group_layout(
            "imgui_texture_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );

        ImguiPipeline {
            transform_bind_group_layout,
            texture_bind_group_layout,
        }
    }
}

/// Key for specialized pipeline
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct ImguiPipelineKey {
    /// Reflects the value of [`Camera::hdr`]
    pub hdr: bool,
}

impl SpecializedRenderPipeline for ImguiPipeline {
    type Key = ImguiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("imgui_pipeline".into()),
            layout: vec![
                self.transform_bind_group_layout.clone(),
                self.texture_bind_group_layout.clone(),
            ],
            vertex: VertexState {
                shader: IMGUI_SHADER_HANDLE,
                shader_defs: Vec::new(),
                entry_point: Some("vs_main".into()),
                buffers: vec![VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Vertex,
                    [
                        VertexFormat::Float32x2, // position
                        VertexFormat::Float32x2, // UV
                        VertexFormat::Uint32,    // color (packed RGBA)
                    ],
                )],
            },
            fragment: Some(FragmentState {
                shader: IMGUI_SHADER_HANDLE,
                shader_defs: Vec::new(),
                entry_point: Some("fs_main".into()),
                targets: vec![Some(ColorTargetState {
                    format: if key.hdr {
                        ViewTarget::TEXTURE_FORMAT_HDR
                    } else {
                        TextureFormat::bevy_default()
                    },
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        }
    }
}
