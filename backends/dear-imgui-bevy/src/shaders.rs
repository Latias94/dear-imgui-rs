//! Shader loading for Dear ImGui

use bevy::{asset::embedded_asset, prelude::*};

use crate::render_impl::IMGUI_SHADER_HANDLE;

/// Plugin to load Dear ImGui shaders
pub struct ImguiShaderPlugin;

impl Plugin for ImguiShaderPlugin {
    fn build(&self, app: &mut App) {
        // Load the embedded shader
        embedded_asset!(app, "render_impl/imgui.wgsl");

        // Register the shader handle
        let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(
            &IMGUI_SHADER_HANDLE,
            Shader::from_wgsl(
                include_str!("render_impl/imgui.wgsl"),
                "render_impl/imgui.wgsl",
            ),
        );
    }
}
