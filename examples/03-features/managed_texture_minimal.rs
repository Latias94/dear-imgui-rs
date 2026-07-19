//! Minimal Context-owned managed texture lifecycle.

use dear_app::{
    AppConfig, Application, FrameContext, InitContext, PrepareFrameContext, RunError, run,
};
use dear_imgui_rs::{Condition, ManagedTextureId, OwnedTextureData, TextureFormat};

const TEXTURE_WIDTH: u32 = 256;
const TEXTURE_HEIGHT: u32 = 192;

#[derive(Clone, Copy)]
enum TextureAction {
    Update,
    Remove,
    Recreate,
}

#[derive(Default)]
struct ManagedTextureApp {
    texture: Option<ManagedTextureId>,
    pending_action: Option<TextureAction>,
    revision: u32,
}

impl ManagedTextureApp {
    fn texture_data(revision: u32) -> OwnedTextureData {
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, TEXTURE_WIDTH, TEXTURE_HEIGHT);
        texture.set_data(&Self::pixels(revision));
        texture
    }

    fn pixels(revision: u32) -> Vec<u8> {
        let mut pixels = Vec::with_capacity((TEXTURE_WIDTH * TEXTURE_HEIGHT * 4) as usize);
        let shift = revision.wrapping_mul(23) % 256;
        let checker_offset = revision & 1;

        for y in 0..TEXTURE_HEIGHT {
            for x in 0..TEXTURE_WIDTH {
                let checker = (x / 24 + y / 24 + checker_offset).is_multiple_of(2);
                let red = ((x * 255 / (TEXTURE_WIDTH - 1) + shift) % 256) as u8;
                let green = (y * 255 / (TEXTURE_HEIGHT - 1)) as u8;
                let blue = if checker { 224 } else { 48 };
                pixels.extend_from_slice(&[red, green, blue, 255]);
            }
        }

        pixels
    }

    fn register(&mut self, context: &mut dear_imgui_rs::Context, revision: u32) {
        self.texture = Some(context.register_texture(Self::texture_data(revision)));
        self.revision = revision;
    }
}

impl Application for ManagedTextureApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        self.register(context.imgui(), self.revision);
        Ok(())
    }

    fn prepare_frame(&mut self, context: &mut PrepareFrameContext<'_>) -> Result<(), RunError> {
        let Some(action) = self.pending_action.take() else {
            return Ok(());
        };

        match action {
            TextureAction::Update => {
                let Some(texture) = self.texture else {
                    return Ok(());
                };
                let revision = self.revision.wrapping_add(1);
                let pixels = Self::pixels(revision);
                context
                    .imgui()
                    .with_texture_mut(texture, |mut texture| texture.set_data(&pixels))
                    .map_err(|error| {
                        RunError::application(
                            "prepare_frame",
                            format!("failed to update managed texture: {error}"),
                        )
                    })?;
                self.revision = revision;
            }
            TextureAction::Remove => {
                let Some(texture) = self.texture else {
                    return Ok(());
                };
                context.imgui().remove_texture(texture).map_err(|error| {
                    RunError::application(
                        "prepare_frame",
                        format!("failed to remove managed texture: {error}"),
                    )
                })?;
                self.texture = None;
            }
            TextureAction::Recreate => {
                if self.texture.is_none() {
                    self.register(context.imgui(), self.revision.wrapping_add(1));
                }
            }
        }

        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let ui = context.ui();

        ui.window("Managed Texture")
            .size([520.0, 430.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Revision: {}", self.revision));
                ui.separator();

                if let Some(texture) = self.texture {
                    ui.image(texture, [384.0, 288.0]);
                    ui.separator();
                    if ui.button("Update pixels") {
                        self.pending_action = Some(TextureAction::Update);
                    }
                    ui.same_line();
                    if ui.button("Remove") {
                        self.pending_action = Some(TextureAction::Remove);
                    }
                } else {
                    ui.text("Texture removed");
                    if ui.button("Recreate") {
                        self.pending_action = Some(TextureAction::Recreate);
                    }
                }
            });

        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Managed Texture".to_owned(),
        window_size: (760.0, 560.0),
        ..Default::default()
    };

    run(config, ManagedTextureApp::default())
}
