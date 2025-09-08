//! Bevy systems for ImGui integration

use crate::{context::ImguiContext, render::ImguiRenderContext};
use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    prelude::*,
    render::{
        render_asset::RenderAssets,
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
        view::ExtractedWindows,
        Extract,
    },
    window::PrimaryWindow,
};
use dear_imgui::OwnedDrawData;
use log::info;
use std::{ops::DerefMut, ptr::NonNull};

/// System to start a new ImGui frame and handle input
pub fn imgui_new_frame_system(
    mut context: NonSendMut<ImguiContext>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: EventReader<bevy::input::mouse::MouseWheel>,
) {
    // Simplified input handling for now
    // TODO: Implement proper key mapping when Dear ImGui API is more complete


    let ui_ptr: NonNull<dear_imgui::Ui>;
    {
        let mut ctx = context.context().write().unwrap();
        let io = ctx.io_mut();

        // Update display size and mouse position
        if let Ok((_, primary)) = primary_window.single() {
            io.set_display_size([primary.width(), primary.height()]);
            io.set_display_framebuffer_scale([primary.scale_factor(), primary.scale_factor()]);

            if let Some(pos) = primary.cursor_position() {
                io.set_mouse_pos([pos.x, pos.y]);
            }
        }

        // Handle mouse buttons
        io.add_mouse_button_event(dear_imgui::MouseButton::Left, mouse.pressed(MouseButton::Left));
        io.add_mouse_button_event(dear_imgui::MouseButton::Right, mouse.pressed(MouseButton::Right));
        io.add_mouse_button_event(dear_imgui::MouseButton::Middle, mouse.pressed(MouseButton::Middle));

        // TODO: Handle character input
        // Character input handling would need to be implemented with proper event readers

        // Handle mouse wheel
        for e in mouse_wheel.read() {
            io.add_mouse_wheel_event([e.x, e.y]);
        }

        // Start new frame
        let ui = ctx.frame();
        ui_ptr = unsafe { NonNull::new_unchecked(ui as *const _ as *mut _) };
    }

    context.set_ui(Some(ui_ptr));
}

/// System to end the ImGui frame and capture draw data
pub fn imgui_end_frame_system(mut context: NonSendMut<ImguiContext>) {
    // End the imgui frame and capture draw data
    let owned_draw_data = {
        let mut ctx = context.context().write().unwrap();
        let draw_data = ctx.render();
        OwnedDrawData::from(draw_data)
    };

    context.set_ui(None);
    *context.rendered_draw_data().write().unwrap() = owned_draw_data;
}

/// System to extract ImGui frame data to the render world
pub fn imgui_extract_frame_system(
    primary_window: Extract<Query<&Window, With<PrimaryWindow>>>,
    imgui_context: Extract<NonSend<ImguiContext>>,
    mut render_context: ResMut<ImguiRenderContext>,
) {
    // Get the rendered draw data
    let owned_draw_data = {
        let mut draw_data_guard = imgui_context.rendered_draw_data().write().unwrap();
        std::mem::take(&mut *draw_data_guard)
    };

    // Store the draw data
    render_context.draw_data = Some(owned_draw_data);
}

/// System to update textures in the render world
pub fn imgui_update_textures_system(
    mut context: ResMut<ImguiRenderContext>,
    _device: Res<RenderDevice>,
    _gpu_images: Res<RenderAssets<bevy::render::texture::GpuImage>>,
) {
    // Remove textures flagged for removal
    let textures_to_remove = std::mem::take(&mut context.textures_to_remove);
    for texture_id in &textures_to_remove {
        // Note: Our WgpuRenderer doesn't expose texture removal yet
        // This would need to be implemented in dear-imgui-wgpu
        info!("Texture removal not yet implemented in dear-imgui-wgpu: {:?}", texture_id);
    }

    // Add new textures
    let mut added_textures = Vec::<u32>::new();
    for (texture_id, _handle) in &context.textures_to_add {
        // Note: Texture addition would need to be implemented in dear-imgui-wgpu
        // For now, we just track that we've processed this texture
        info!("Texture addition not yet fully implemented in dear-imgui-wgpu: {:?}", texture_id);
        added_textures.push(*texture_id);
    }

    for texture_id in &added_textures {
        context.textures_to_add.remove(texture_id);
    }
}
