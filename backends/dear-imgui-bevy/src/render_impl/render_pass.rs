//! Dear ImGui render pass implementation

use bevy::{
    ecs::world::World,
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{IndexFormat, PipelineCache, RenderPassDescriptor},
        renderer::RenderContext,
        view::{ExtractedView, ViewTarget},
    },
};

use crate::render_impl::systems::{
    ImguiPipelines, ImguiRenderData, ImguiTextureBindGroups, ImguiTransforms,
};

/// Dear ImGui render pass node
pub struct ImguiPassNode;

impl ImguiPassNode {
    /// Creates a new Dear ImGui pass node
    pub fn new(_world: &mut World) -> Self {
        Self
    }
}

impl Node for ImguiPassNode {
    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let imgui_pipelines = &world.resource::<ImguiPipelines>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let render_data = world.resource::<ImguiRenderData>();

        // Extract the view
        let input_view_entity = graph.view_entity();

        // Query the view components
        let Some(view) = world.get::<ExtractedView>(input_view_entity) else {
            return Ok(());
        };
        let Some(target) = world.get::<ViewTarget>(input_view_entity) else {
            return Ok(());
        };

        let main_entity = view.retained_view_entity.main_entity;

        let Some(data) = render_data.0.get(&main_entity) else {
            return Ok(());
        };

        let Some(pipeline_id) = imgui_pipelines.get(&main_entity) else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(*pipeline_id) else {
            return Ok(());
        };

        // Create render pass
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("imgui_pass"),
            color_attachments: &[Some(target.get_unsampled_color_attachment())],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set up rendering state
        render_pass.set_render_pipeline(pipeline);

        // Get bind groups and buffers
        let bind_groups = world.resource::<ImguiTextureBindGroups>();
        let imgui_transforms = world.resource::<ImguiTransforms>();

        let transform_buffer_offset = imgui_transforms
            .offsets
            .get(&view.retained_view_entity.main_entity)
            .copied()
            .unwrap_or(0);

        let transform_buffer_bind_group = &imgui_transforms
            .bind_group
            .as_ref()
            .expect("Expected a prepared bind group")
            .1;

        // Set transform bind group
        render_pass.set_bind_group(0, transform_buffer_bind_group, &[transform_buffer_offset]);

        let (vertex_buffer, index_buffer) = match (&data.vertex_buffer, &data.index_buffer) {
            (Some(vertex), Some(index)) => (vertex, index),
            _ => {
                return Ok(());
            }
        };

        // Set vertex and index buffers
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

        // We always use Uint32 for indices internally for simplicity and safety
        // This handles both 16-bit and 32-bit Dear ImGui configurations
        render_pass.set_index_buffer(index_buffer.slice(..), 0, IndexFormat::Uint32);

        // Render draw commands
        let mut index_offset = 0u32;
        for draw_command in data.draw_commands.iter() {
            // Set texture bind group
            if let Some(texture_bind_group) = bind_groups.get(&draw_command.texture_id) {
                render_pass.set_bind_group(1, texture_bind_group, &[]);

                // Set scissor rect (simplified for now)
                let clip_rect = &draw_command.clip_rect;
                let min_x = (clip_rect[0] * data.pixels_per_point).max(0.0) as u32;
                let min_y = (clip_rect[1] * data.pixels_per_point).max(0.0) as u32;
                let max_x =
                    (clip_rect[2] * data.pixels_per_point).min(data.target_size.x as f32) as u32;
                let max_y =
                    (clip_rect[3] * data.pixels_per_point).min(data.target_size.y as f32) as u32;

                if max_x > min_x && max_y > min_y {
                    render_pass.set_scissor_rect(min_x, min_y, max_x - min_x, max_y - min_y);

                    // Draw indexed
                    render_pass.draw_indexed(
                        index_offset..(index_offset + draw_command.indices_count as u32),
                        0,
                        0..1,
                    );
                }
            }

            index_offset += draw_command.indices_count as u32;
        }

        Ok(())
    }
}
