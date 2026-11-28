//! Render-world extraction data for the Bevy backend.
//!
//! The main world moves one [`FrameSnapshot`](dear_imgui_rs::render::snapshot::FrameSnapshot)
//! through a bounded mailbox and associates it with the Bevy cameras that should receive ImGui
//! overlay rendering. The render world owns the snapshot until request-bound texture feedback is
//! committed, without borrowing raw ImGui draw data across worlds.

use bevy_app::App;
use bevy_asset::{Assets, Handle, uuid_handle};
use bevy_camera::{
    Camera, CameraMainTextureUsages, CameraOutputMode, ClearColor, ClearColorConfig,
    CompositingSpace, NormalizedRenderTarget, RenderTarget, Viewport,
};
use bevy_core_pipeline::{Core2d, Core2dSystems, Core3d, Core3dSystems, upscaling::upscaling};
use bevy_ecs::entity::ContainsEntity;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::SystemParam;
use bevy_image::Image;
use bevy_math::{UVec2, UVec4};
use bevy_mesh::VertexBufferLayout;
use bevy_render::{
    Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
    camera::ExtractedCamera,
    render_asset::RenderAssets,
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindingResource,
        BindingType, BlendState, Buffer, BufferAddress, BufferBindingType, BufferDescriptor,
        BufferSize, BufferUsages, COPY_BUFFER_ALIGNMENT, CachedRenderPipelineId, ColorTargetState,
        ColorWrites, CommandEncoderDescriptor, Extent3d, FilterMode, FragmentState, IndexFormat,
        LoadOp, MipmapFilterMode, MultisampleState, Operations, Origin3d, PipelineCache,
        PrimitiveState, PrimitiveTopology, RawBufferVec, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
        SamplerDescriptor, ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines,
        StoreOp, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
        TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexFormat,
        VertexState, VertexStepMode, WgpuFeatures,
    },
    renderer::{
        RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue, ViewQuery,
    },
    sync_world::MainEntity,
    texture::GpuImage,
    view::{
        ExtractedView, ExtractedWindow, Msaa, ResolvedCompositingSpace, RetainedViewEntity,
        ViewTarget,
    },
};
use bevy_shader::Shader;
use bevy_window::{PrimaryWindow, Window};
use bytemuck::{Pod, Zeroable};
use dear_imgui_rs as imgui;
use imgui::render::{DrawCmdSnapshot, DrawIdx, SnapshotTextureId, TextureBinding};
use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::viewport::ImguiViewportOwner;
use crate::{ImguiViewportCamera, ImguiViewportWindow};

mod extract;
mod lifecycle;
mod pass;
mod pipeline;
mod plugin;
mod prepare;
mod resources;
mod texture;

#[cfg(test)]
pub(crate) use pipeline::{
    IMGUI_FRAGMENT_ENTRY_POINT, IMGUI_VERTEX_ENTRY_POINT, ImguiPipelineKey,
    imgui_vertex_buffer_layout,
};
pub(crate) use pipeline::{
    IMGUI_SHADER_HANDLE, IMGUI_SHADER_SOURCE, ImguiGpuVertex, ImguiRenderPipeline, ImguiUniforms,
};
pub use plugin::ImguiRenderSystems;
#[cfg(feature = "bevy-ui")]
pub use plugin::ImguiUiRenderOrder;
#[cfg(not(feature = "bevy-ui"))]
pub(crate) use plugin::ImguiUiRenderOrder;
pub(crate) use plugin::{
    clear_standard_draw_callbacks_if_owned, install_render_extraction,
    install_standard_draw_callbacks_for_context, render_integration_available,
    standard_draw_callback_conflict, standard_draw_callback_contract,
    standard_draw_callback_occupied,
};
pub(crate) use resources::{
    ImguiCameraTarget, ImguiCameraViewport, ImguiExtractedBevyTextures, ImguiExtractedRenderFrame,
    ImguiGpuBuffers, ImguiPipelineGpuResources, ImguiPreparedDraw, ImguiPreparedRenderFrame,
    ImguiQueuedPipelines, ImguiSampler, ImguiScissorRect,
};
pub(crate) use resources::{ImguiRendererReleaseLease, ImguiRendererReleases};
pub(crate) use texture::ImguiTextureBindGroups;

#[cfg(test)]
#[path = "tests/render_extract.rs"]
mod render_extract_tests;
#[cfg(test)]
#[path = "tests/texture.rs"]
mod texture_tests;

type ViewportCameraQuery<'w> = Query<
    'w,
    'w,
    (
        Entity,
        &'w Camera,
        &'w RenderTarget,
        &'w bevy_render::camera::CameraRenderGraph,
        &'w ImguiViewportCamera,
        &'w ImguiViewportOwner,
    ),
>;

type ViewportWindowQuery<'w> = Query<
    'w,
    'w,
    (&'w Window, &'w ImguiViewportWindow, &'w ImguiViewportOwner),
    With<ImguiViewportWindow>,
>;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
