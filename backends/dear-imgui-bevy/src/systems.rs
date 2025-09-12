//! Bevy systems for ImGui integration

use crate::context::ImguiContext;
use bevy::{
    prelude::*,
    render::{render_asset::RenderAssets, renderer::RenderDevice, MainWorld},
    window::PrimaryWindow,
};
use dear_imgui::{OwnedDrawData, TextureId};

use tracing::warn;

/// Convert Bevy KeyCode to Dear ImGui Key
fn bevy_key_to_imgui_key(key_code: KeyCode) -> Option<dear_imgui::Key> {
    use dear_imgui::Key;
    use KeyCode::*;

    match key_code {
        Tab => Some(Key::Tab),
        ArrowLeft => Some(Key::LeftArrow),
        ArrowRight => Some(Key::RightArrow),
        ArrowUp => Some(Key::UpArrow),
        ArrowDown => Some(Key::DownArrow),
        PageUp => Some(Key::PageUp),
        PageDown => Some(Key::PageDown),
        Home => Some(Key::Home),
        End => Some(Key::End),
        Insert => Some(Key::Insert),
        Delete => Some(Key::Delete),
        Backspace => Some(Key::Backspace),
        Space => Some(Key::Space),
        Enter => Some(Key::Enter),
        Escape => Some(Key::Escape),
        ControlLeft => Some(Key::LeftCtrl),
        ControlRight => Some(Key::RightCtrl),
        ShiftLeft => Some(Key::LeftShift),
        ShiftRight => Some(Key::RightShift),
        AltLeft => Some(Key::LeftAlt),
        AltRight => Some(Key::RightAlt),
        SuperLeft => Some(Key::LeftSuper),
        SuperRight => Some(Key::RightSuper),
        F1 => Some(Key::F1),
        F2 => Some(Key::F2),
        F3 => Some(Key::F3),
        F4 => Some(Key::F4),
        F5 => Some(Key::F5),
        F6 => Some(Key::F6),
        F7 => Some(Key::F7),
        F8 => Some(Key::F8),
        F9 => Some(Key::F9),
        F10 => Some(Key::F10),
        F11 => Some(Key::F11),
        F12 => Some(Key::F12),
        // Skip numpad keys for now - not available in current Dear ImGui version
        Numpad0 => None,
        Numpad1 => None,
        Numpad2 => None,
        Numpad3 => None,
        Numpad4 => None,
        Numpad5 => None,
        Numpad6 => None,
        Numpad7 => None,
        Numpad8 => None,
        Numpad9 => None,
        NumpadDecimal => None,
        NumpadDivide => None,
        NumpadMultiply => None,
        NumpadSubtract => None,
        NumpadAdd => None,
        NumpadEnter => Some(Key::Enter), // Use regular Enter for now
        NumpadEqual => None,             // Not available in current Dear ImGui version
        KeyA => Some(Key::A),
        KeyB => Some(Key::B),
        KeyC => Some(Key::C),
        KeyD => Some(Key::D),
        KeyE => Some(Key::E),
        KeyF => Some(Key::F),
        KeyG => Some(Key::G),
        KeyH => Some(Key::H),
        KeyI => Some(Key::I),
        KeyJ => Some(Key::J),
        KeyK => Some(Key::K),
        KeyL => Some(Key::L),
        KeyM => Some(Key::M),
        KeyN => Some(Key::N),
        KeyO => Some(Key::O),
        KeyP => Some(Key::P),
        KeyQ => Some(Key::Q),
        KeyR => Some(Key::R),
        KeyS => Some(Key::S),
        KeyT => Some(Key::T),
        KeyU => Some(Key::U),
        KeyV => Some(Key::V),
        KeyW => Some(Key::W),
        KeyX => Some(Key::X),
        KeyY => Some(Key::Y),
        KeyZ => Some(Key::Z),
        // Map digit keys using the correct Dear ImGui key names
        Digit0 => Some(Key::Key0),
        Digit1 => Some(Key::Key1),
        Digit2 => Some(Key::Key2),
        Digit3 => Some(Key::Key3),
        Digit4 => Some(Key::Key4),
        Digit5 => Some(Key::Key5),
        Digit6 => Some(Key::Key6),
        Digit7 => Some(Key::Key7),
        Digit8 => Some(Key::Key8),
        Digit9 => Some(Key::Key9),
        // Map punctuation keys - using Bevy's naming
        Quote => None,        // Bevy calls it Quote, not Apostrophe
        Comma => None,        // Skip for now
        Minus => None,        // Skip for now
        Period => None,       // Skip for now
        Slash => None,        // Skip for now
        Semicolon => None,    // Skip for now
        Equal => None,        // Skip for now
        BracketLeft => None,  // Skip for now
        Backslash => None,    // Skip for now
        BracketRight => None, // Skip for now
        Backquote => None,    // Skip for now
        _ => None,
    }
}

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
    context: NonSendMut<ImguiContext>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut keyboard_input: MessageReader<bevy::input::keyboard::KeyboardInput>,
    // TODO: Add character input support when available in Bevy
    time: Res<Time>,
) {
    context.with_context(|ctx| {
        let io = ctx.io_mut();

        // Set delta time (required by Dear ImGui)
        // Ensure delta time is always positive (Dear ImGui requirement)
        let delta_time = time.delta_secs().max(1.0 / 60.0); // Minimum 60 FPS fallback
        io.set_delta_time(delta_time);

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

        // Handle keyboard input events
        for event in keyboard_input.read() {
            if let Some(imgui_key) = bevy_key_to_imgui_key(event.key_code) {
                io.add_key_event(imgui_key, event.state.is_pressed());
            }
        }

        // TODO: Handle character input when available in Bevy

        // Handle modifier keys using individual key states
        io.add_key_event(
            dear_imgui::Key::LeftCtrl,
            keyboard.pressed(KeyCode::ControlLeft),
        );
        io.add_key_event(
            dear_imgui::Key::RightCtrl,
            keyboard.pressed(KeyCode::ControlRight),
        );
        io.add_key_event(
            dear_imgui::Key::LeftShift,
            keyboard.pressed(KeyCode::ShiftLeft),
        );
        io.add_key_event(
            dear_imgui::Key::RightShift,
            keyboard.pressed(KeyCode::ShiftRight),
        );
        io.add_key_event(dear_imgui::Key::LeftAlt, keyboard.pressed(KeyCode::AltLeft));
        io.add_key_event(
            dear_imgui::Key::RightAlt,
            keyboard.pressed(KeyCode::AltRight),
        );
        io.add_key_event(
            dear_imgui::Key::LeftSuper,
            keyboard.pressed(KeyCode::SuperLeft),
        );
        io.add_key_event(
            dear_imgui::Key::RightSuper,
            keyboard.pressed(KeyCode::SuperRight),
        );

        // Handle mouse wheel
        for e in mouse_wheel.read() {
            io.add_mouse_wheel_event([e.x, e.y]);
        }

        // NOTE: We don't call ctx.frame() here anymore!
        // The frame will be started when user systems call with_context
        // This ensures proper frame lifecycle management
    });
}

/// System to end the ImGui frame and capture draw data
pub fn imgui_end_frame_system(
    context: NonSendMut<ImguiContext>,
    mut cameras: Query<&mut ImguiRenderOutput, With<Camera>>,
) {
    // End the imgui frame and capture draw data
    let owned_draw_data = context.with_context(|ctx| {
        let draw_data = ctx.render();
        OwnedDrawData::from(draw_data)
    });

    // Store the draw data in the camera component for extraction
    if let Ok(mut render_output) = cameras.single_mut() {
        render_output.draw_data = Some(owned_draw_data);
    }
}

// Font texture extraction is now handled by ExtractResourcePlugin

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

            break; // Only handle one camera for now
        }
    }
}

/// System to update render context with extracted draw data
pub fn imgui_update_render_context_system(extracted_data: Option<Res<ExtractedImguiDrawData>>) {
    if let Some(data) = extracted_data {
        // Draw data will be handled by the prepare systems
        // Don't take the data here, let the prepare system use it
        if data.draw_data.is_some() {}
    }
}

/// System to update textures in the render world using modern texture API
pub fn imgui_update_textures_system(
    mut texture_manager: ResMut<crate::texture::BevyImguiTextureManager>,
    device: Res<RenderDevice>,
    queue: Res<bevy::render::renderer::RenderQueue>,
    gpu_images: Res<RenderAssets<bevy::render::texture::GpuImage>>,
    imgui_pipeline: Res<crate::render_impl::ImguiPipeline>,
    extracted_data: Option<Res<ExtractedImguiDrawData>>,
) {
    let Some(extracted_data) = extracted_data else {
        return;
    };

    let Some(ref draw_data_owned) = extracted_data.draw_data else {
        return;
    };

    // Convert OwnedDrawData to DrawData for texture processing
    if let Some(draw_data) = draw_data_owned.draw_data() {
        // Handle texture updates using the modern Dear ImGui texture API
        texture_manager.handle_texture_updates(
            draw_data,
            &device,
            &queue,
            &gpu_images,
            &imgui_pipeline.texture_bind_group_layout,
        );
    }
}

/// System to initialize font texture in Dear ImGui context
pub fn imgui_prepare_font_texture_system(
    context: NonSend<ImguiContext>,
    mut font_texture: ResMut<crate::render_impl::ImguiFontTexture>,
) {
    // Access the context safely to prepare font texture
    context.with_context(|ctx| {
        let mut fonts = ctx.fonts();

        // Build the font atlas if not already built
        if !fonts.is_built() {
            fonts.build();

            // Get font texture data from Dear ImGui
            if let Some((pixels_ptr, width, height)) = unsafe { fonts.get_tex_data_ptr() } {
                // Dear ImGui font texture is single channel (Alpha8), but we need to convert to RGBA
                let pixel_count = (width * height) as usize; // Single channel
                let alpha_data = unsafe { std::slice::from_raw_parts(pixels_ptr, pixel_count) };

                // Convert Alpha8 to RGBA8 (white text with alpha)
                let mut rgba_data = Vec::with_capacity(pixel_count * 4);
                for &alpha in alpha_data {
                    rgba_data.push(255); // R
                    rgba_data.push(255); // G
                    rgba_data.push(255); // B
                    rgba_data.push(alpha); // A
                }

                // Update the font texture resource
                font_texture.width = width;
                font_texture.height = height;
                font_texture.data = rgba_data;
                font_texture.needs_update = true;

                // Set the font texture reference in Dear ImGui
                // Use TextureId(0) for font texture to match the rendering system
                let font_texture_id = TextureId::from(0usize);
                let texture_ref =
                    dear_imgui::texture::create_texture_ref(font_texture_id.id() as u64);
                fonts.set_tex_ref(texture_ref);
            } else {
                warn!("Failed to get font texture data from Dear ImGui");
            }
        }
    });
}
