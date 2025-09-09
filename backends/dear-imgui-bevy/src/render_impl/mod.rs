//! Dear ImGui rendering implementation for Bevy
//!
//! This module implements custom wgpu rendering for Dear ImGui, similar to bevy_egui's approach.
//! We don't use dear-imgui-wgpu to avoid type conversion issues between Bevy and wgpu types.

pub use pipeline::*;
pub use systems::*;

mod pipeline;
mod render_pass;
pub mod systems;

use bevy::{
    asset::{uuid_handle, Handle},
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext, RenderLabel},
        renderer::RenderContext,
    },
};

/// Dear ImGui shader handle
pub const IMGUI_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("05a4d7a0-4f24-4d7f-b606-3f399074261f");

/// Render context resource for ImGui rendering
#[derive(Resource)]
pub struct ImguiRenderContext {
    /// Current texture format
    pub texture_format: wgpu::TextureFormat,
    /// Current display scale
    pub display_scale: f32,
}

impl ImguiRenderContext {
    pub fn new(texture_format: wgpu::TextureFormat, display_scale: f32) -> Self {
        Self {
            texture_format,
            display_scale,
        }
    }
}

/// The label used by the render node responsible for rendering ImGui
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct ImguiRenderNodeLabel;

/// Render node for ImGui rendering
pub struct ImguiRenderNode {
    pass_node: render_pass::ImguiPassNode,
}

impl Default for ImguiRenderNode {
    fn default() -> Self {
        Self {
            pass_node: render_pass::ImguiPassNode::new(&mut World::new()),
        }
    }
}

impl Node for ImguiRenderNode {
    fn update(&mut self, world: &mut World) {
        self.pass_node.update(world);
    }

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        self.pass_node.run(graph, render_context, world)
    }
}
