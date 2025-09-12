//! Bevy plugin implementation for Dear ImGui integration

use crate::{
    context::ImguiContext,
    render_impl::{
        systems::{
            prepare_imgui_render_data_system, prepare_imgui_transforms_system,
            queue_imgui_bind_groups_system,
        },
        ImguiFontTexture, ImguiPipeline, ImguiPipelines, ImguiRenderContext, ImguiRenderData,
        ImguiRenderNode, ImguiRenderNodeLabel, ImguiTextureBindGroups, ImguiTransforms,
    },
    shaders::ImguiShaderPlugin,
    systems::{
        imgui_end_frame_system, imgui_extract_frame_system,
        imgui_new_frame_system, imgui_prepare_font_texture_system, imgui_update_render_context_system, imgui_update_textures_system,
        setup_imgui_render_output_system,
    },
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
        extract_resource::ExtractResourcePlugin,
        render_graph::RenderGraphExt,
        render_resource::SpecializedRenderPipelines,
        renderer::{RenderDevice, RenderQueue},
        Render, RenderApp, RenderSystems,
    },
    window::PrimaryWindow,
};
use dear_imgui::Context;
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
    fn build(&self, app: &mut App) {
        // Add shader plugin
        app.add_plugins(ImguiShaderPlugin);

        // Add extract resource plugin for font texture
        app.add_plugins(ExtractResourcePlugin::<crate::render_impl::systems::ExtractedImguiFontTexture>::default());

        // Initialize texture manager resource
        app.insert_resource(crate::texture::BevyImguiTextureManager::new());

        // Initialize font texture resource
        app.insert_resource(crate::render_impl::ImguiFontTexture::default());
    }

    fn finish(&self, app: &mut App) {
        // Create Dear ImGui context
        let mut ctx = Context::create_or_panic();
        ctx.set_ini_filename_or_panic(self.ini_filename.clone());

        // Set backend information and flags
        let io = ctx.io_mut();
        let mut flags = io.backend_flags();

        // Set renderer capabilities
        flags.insert(dear_imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.insert(dear_imgui::BackendFlags::RENDERER_HAS_TEXTURES);
        flags.insert(dear_imgui::BackendFlags::HAS_MOUSE_CURSORS);
        flags.insert(dear_imgui::BackendFlags::HAS_SET_MOUSE_POS);

        // TODO: Add when we implement gamepad support
        // flags.insert(dear_imgui::BackendFlags::HAS_GAMEPAD);

        io.set_backend_flags(flags);

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
            let (_device, _queue) = system_state.get_mut(render_app.world_mut());

            // Set up texture format
            let texture_format = wgpu::TextureFormat::Bgra8UnormSrgb; // Default format

            // Initialize texture manager in render world
            render_app.insert_resource(crate::texture::BevyImguiTextureManager::new());

            // Add render graph nodes
            render_app
                .add_render_graph_node::<ImguiRenderNode>(Core2d, ImguiRenderNodeLabel)
                .add_render_graph_edges(Core2d, (Node2d::EndMainPass, ImguiRenderNodeLabel))
                .add_render_graph_node::<ImguiRenderNode>(Core3d, ImguiRenderNodeLabel)
                .add_render_graph_edges(Core3d, (Node3d::EndMainPass, ImguiRenderNodeLabel));

            // Insert render context resources
            render_app.insert_resource(ImguiRenderContext::new(texture_format, display_scale));

            // Initialize render resources
            render_app.init_resource::<ImguiTransforms>();
            render_app.init_resource::<ImguiTextureBindGroups>();
            render_app.init_resource::<ImguiRenderData>();
            render_app.init_resource::<ImguiPipelines>();
            render_app.init_resource::<ImguiPipeline>();
            render_app.init_resource::<ImguiFontTexture>();
            render_app.init_resource::<crate::render_impl::systems::ExtractedImguiFontTexture>();
            render_app.init_resource::<SpecializedRenderPipelines<ImguiPipeline>>();

            // Add render systems
            render_app.add_systems(
                ExtractSchedule,
                imgui_extract_frame_system,
            );
            render_app.add_systems(
                Render,
                (
                    imgui_update_render_context_system,
                    imgui_update_textures_system,
                    prepare_imgui_transforms_system,
                    prepare_imgui_render_data_system,
                )
                    .in_set(RenderSystems::Prepare),
            );
            render_app.add_systems(
                Render,
                queue_imgui_bind_groups_system.in_set(RenderSystems::Queue),
            );
        }

        // Insert main app context and systems
        app.insert_non_send_resource(context)
            .add_systems(PreUpdate, imgui_new_frame_system)
            .add_systems(Update, imgui_prepare_font_texture_system)
            .add_systems(Last, imgui_end_frame_system)
            .add_systems(PostUpdate, setup_imgui_render_output_system);
    }
}

impl ImguiPlugin {
    // TODO: Implement font loading and display scale handling for our custom renderer
    // TODO: Implement Dear ImGui 1.92+ modern texture API:
    //       - Support for ImGuiBackendFlags_RendererHasTextures
    //       - Use ImFontAtlas::Textures[] array instead of GetTexDataAsRGBA32()
    //       - Implement proper texture lifecycle management
    // TODO: Add gamepad support with ImGuiBackendFlags_HasGamepad
    // TODO: Add viewport support for multi-window applications
}
