//! Bevy plugin implementation for Dear ImGui integration

use crate::{
    context::ImguiContext,
    render::{ImguiRenderContext, ImguiRenderNode, ImguiRenderNodeLabel},
    systems::{imgui_end_frame_system, imgui_extract_frame_system, imgui_new_frame_system, imgui_update_textures_system},
};
use bevy::{
    app::{App, Plugin},
    core_pipeline::{
        core_2d::graph::{Core2d, Node2d},
        core_3d::graph::{Core3d, Node3d},
    },
    ecs::system::SystemState,
    prelude::*,
    render::{
        render_graph::RenderGraphExt,
        renderer::{RenderDevice, RenderQueue},
        Render, RenderApp, RenderSet,
    },
    window::PrimaryWindow,
};
use dear_imgui::{Context, FontSource};
use dear_imgui_wgpu::WgpuRenderer;
use std::path::PathBuf;

/// Configuration settings for the ImGui plugin
#[derive(Clone)]
pub struct ImguiPlugin {
    /// Sets the path to the ini file (default is "imgui.ini").
    /// Pass None to disable automatic .ini saving
    pub ini_filename: Option<PathBuf>,

    /// The unscaled font size to use (default is 13.0).
    pub font_size: f32,

    /// The number of horizontal font samples to perform. Must be >= 1 (default is 1).
    pub font_oversample_h: i32,

    /// The number of vertical font samples to perform. Must be >= 1 (default is 1).
    pub font_oversample_v: i32,

    /// Whether to apply the window display scale to the font size (default is true).
    pub apply_display_scale_to_font_size: bool,

    /// Whether to apply the window display scale to the number of font samples (default is true).
    pub apply_display_scale_to_font_oversample: bool,
}

impl Default for ImguiPlugin {
    fn default() -> Self {
        Self {
            ini_filename: Some("imgui.ini".into()),
            font_size: 13.0,
            font_oversample_h: 1,
            font_oversample_v: 1,
            apply_display_scale_to_font_size: true,
            apply_display_scale_to_font_oversample: true,
        }
    }
}

impl Plugin for ImguiPlugin {
    fn build(&self, _app: &mut App) {
        // Plugin configuration is handled in finish()
    }

    fn finish(&self, app: &mut App) {
        // Create Dear ImGui context
        let mut ctx = Context::create();
        ctx.set_ini_filename(self.ini_filename.clone());

        // Get display scale from primary window
        let display_scale = {
            let mut system_state: SystemState<Query<&Window, With<PrimaryWindow>>> =
                SystemState::new(app.world_mut());
            let primary_window = system_state.get(app.world());
            primary_window.single().unwrap().scale_factor()
        };

        // Create ImGui context resource
        let context = ImguiContext::new(ctx);

        // Set up render app if available
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            let mut system_state: SystemState<(Res<RenderDevice>, Res<RenderQueue>)> =
                SystemState::new(render_app.world_mut());
            let (device, queue) = system_state.get_mut(render_app.world_mut());

            // Create WGPU renderer with default format
            // The format will be updated during extraction if needed
            let texture_format = wgpu::TextureFormat::Bgra8UnormSrgb; // Default format
            let mut renderer = WgpuRenderer::new(device.wgpu_device(), &queue, texture_format);

            // Initialize font texture
            {
                let mut ctx_guard = context.context().write().unwrap();
                self.update_display_scale(
                    1.0,
                    display_scale,
                    &mut ctx_guard,
                    &mut renderer,
                    device.wgpu_device(),
                    &queue,
                );
            }

            // Add render graph nodes
            render_app
                .add_render_graph_node::<ImguiRenderNode>(Core2d, ImguiRenderNodeLabel)
                .add_render_graph_edges(Core2d, (Node2d::EndMainPass, ImguiRenderNodeLabel))
                .add_render_graph_node::<ImguiRenderNode>(Core3d, ImguiRenderNodeLabel)
                .add_render_graph_edges(Core3d, (Node3d::EndMainPass, ImguiRenderNodeLabel));

            // Insert render context resource
            render_app.insert_resource(ImguiRenderContext::new(
                renderer,
                texture_format,
                self.clone(),
                display_scale,
            ));

            // Add render systems
            render_app.add_systems(ExtractSchedule, imgui_extract_frame_system);
            render_app.add_systems(
                Render,
                imgui_update_textures_system.in_set(RenderSet::Prepare),
            );
        }

        // Insert main app context and systems
        app.insert_non_send_resource(context)
            .add_systems(PreUpdate, imgui_new_frame_system)
            .add_systems(Last, imgui_end_frame_system);
    }
}

impl ImguiPlugin {
    /// Update display scale and reload font accordingly
    pub(crate) fn update_display_scale(
        &self,
        previous_display_scale: f32,
        display_scale: f32,
        ctx: &mut Context,
        renderer: &mut WgpuRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let font_scale = if self.apply_display_scale_to_font_size {
            display_scale
        } else {
            1.0
        };

        let font_oversample_scale = if self.apply_display_scale_to_font_oversample {
            display_scale.ceil() as i32
        } else {
            1
        };

        let io = ctx.io_mut();
        io.set_display_framebuffer_scale([display_scale, display_scale]);
        io.set_font_global_scale(1.0 / font_scale);

        // Reload font
        ctx.font_atlas_mut().clear();
        let font_config = dear_imgui::FontConfig::new()
            .size_pixels(f32::floor(self.font_size * font_scale));
        ctx.font_atlas_mut().add_font(&[FontSource::DefaultFontData {
            config: Some(font_config),
        }]);

        // Reload font texture
        renderer.reload_font_texture(ctx, device, queue);

        // Update style for DPI change - simplified for now
        // TODO: Implement style scaling when API is available
    }
}
