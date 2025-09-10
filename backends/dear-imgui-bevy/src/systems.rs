//! Bevy systems for ImGui integration

use crate::{context::ImguiContext, render_impl::ImguiRenderContext};
use bevy::{
    prelude::*,
    render::{render_asset::RenderAssets, renderer::RenderDevice, MainWorld},
    window::PrimaryWindow,
};
use dear_imgui::OwnedDrawData;
use tracing::info;
use std::ptr::NonNull;

/// Component to hold ImGui render output data that can be extracted to render world
#[derive(Component, Default)]
pub struct ImguiRenderOutput {
    pub draw_data: Option<OwnedDrawData>,
}

/// Resource to hold extracted ImGui draw data in the render world
#[derive(Resource)]
pub struct ExtractedImguiDrawData {
    pub draw_data: Option<OwnedDrawData>,
}

/// System to add ImguiRenderOutput component to cameras that don't have it
pub fn setup_imgui_render_output_system(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera>, Without<ImguiRenderOutput>)>,
) {
    for entity in cameras.iter() {
        commands.entity(entity).insert(ImguiRenderOutput::default());
    }
}

/// System to start a new ImGui frame and handle input
pub fn imgui_new_frame_system(
    mut context: NonSendMut<ImguiContext>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_wheel: EventReader<bevy::input::mouse::MouseWheel>,
) {
    info!("Starting new ImGui frame");
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
        io.add_mouse_button_event(
            dear_imgui::MouseButton::Left,
            mouse.pressed(MouseButton::Left),
        );
        io.add_mouse_button_event(
            dear_imgui::MouseButton::Right,
            mouse.pressed(MouseButton::Right),
        );
        io.add_mouse_button_event(
            dear_imgui::MouseButton::Middle,
            mouse.pressed(MouseButton::Middle),
        );

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
pub fn imgui_end_frame_system(
    mut context: NonSendMut<ImguiContext>,
    mut cameras: Query<&mut ImguiRenderOutput, With<Camera>>,
) {
    // End the imgui frame and capture draw data
    let owned_draw_data = {
        let mut ctx = context.context().write().unwrap();
        let draw_data = ctx.render();
        OwnedDrawData::from(draw_data)
    };

    context.set_ui(None);

    // Store the draw data in the camera component for extraction
    if let Ok(mut render_output) = cameras.single_mut() {
        render_output.draw_data = Some(owned_draw_data);
    }
}

/// System to extract ImGui frame data to the render world
pub fn imgui_extract_frame_system(mut commands: Commands, mut main_world: ResMut<MainWorld>) {
    // Query cameras with ImguiRenderOutput in the main world
    let mut query = main_world.query::<(Entity, &mut ImguiRenderOutput, &Camera)>();

    for (_entity, mut render_output, camera) in query.iter_mut(&mut main_world) {
        // Only process active cameras
        if !camera.is_active {
            continue;
        }

        // Take the draw data from the main world component
        let draw_data = std::mem::take(&mut render_output.draw_data);

        if draw_data.is_some() {
            // Update existing render context or create a simple resource to hold draw data
            // The actual ImguiRenderContext is created in the plugin setup
            commands.insert_resource(ExtractedImguiDrawData { draw_data });
            info!("Extracted ImGui frame data for rendering");
            break; // Only handle one camera for now
        }
    }
}

/// System to update render context with extracted draw data
pub fn imgui_update_render_context_system(extracted_data: Option<Res<ExtractedImguiDrawData>>) {
    if let Some(data) = extracted_data {
        // Draw data will be handled by the prepare systems
        // Don't take the data here, let the prepare system use it
        if data.draw_data.is_some() {
            info!("Render context updated with draw data");
        }
    }
}

/// System to update textures in the render world
pub fn imgui_update_textures_system(
    _context: ResMut<ImguiRenderContext>,
    _device: Res<RenderDevice>,
    _gpu_images: Res<RenderAssets<bevy::render::texture::GpuImage>>,
) {
    // TODO: Implement texture management
    // Remove textures flagged for removal
    // let textures_to_remove = std::mem::take(&mut context.textures_to_remove);
    // for texture_id in &textures_to_remove {
    //     // Note: Our WgpuRenderer doesn't expose texture removal yet
    //     // This would need to be implemented in dear-imgui-wgpu
    //     info!("Texture removal not yet implemented in dear-imgui-wgpu: {:?}", texture_id);
    // }

    // // Add new textures
    // let mut added_textures = Vec::<u32>::new();
    // for (texture_id, _handle) in &context.textures_to_add {
    //     // Note: Texture addition would need to be implemented in dear-imgui-wgpu
    //     // For now, we just track that we've processed this texture
    //     info!("Texture addition not yet fully implemented in dear-imgui-wgpu: {:?}", texture_id);
    //     added_textures.push(*texture_id);
    // }

    // for texture_id in &added_textures {
    //     context.textures_to_add.remove(texture_id);
    // }
}
