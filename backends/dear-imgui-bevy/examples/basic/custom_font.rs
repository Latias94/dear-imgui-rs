//! Custom font configuration through the Bevy Context registry.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example custom_font`

use bevy::{
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::prelude::*;
use dear_imgui_rs::{Condition, FontId, FontSource, StbTrueTypeFontData};

const ROBOTO_MEDIUM: &[u8] = include_bytes!("../assets/Roboto-Medium.ttf");

// FontId is Context-bound and deliberately !Send/!Sync.
#[derive(Default)]
struct CustomFonts {
    roboto_medium: Option<FontId>,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "dear-imgui-bevy custom font".to_owned(),
            resolution: (720, 480).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_plugins(ImguiPlugin::default())
    .insert_non_send(CustomFonts::default())
    .add_systems(Startup, (setup_scene, configure_fonts));
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(custom_font_ui))
        .run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn configure_fonts(
    mut contexts: NonSendMut<ImguiContexts>,
    mut fonts: NonSendMut<CustomFonts>,
) -> Result {
    let primary = contexts
        .primary_id()
        .ok_or("ImguiPlugin should install a primary Context before Startup")?;

    let roboto = StbTrueTypeFontData::from_slice(ROBOTO_MEDIUM)?;
    let roboto_medium = contexts.configure(primary, move |context| {
        let atlas = context.font_atlas();
        atlas.add_font(&[FontSource::default_font_with_size(16.0)]);

        let source = FontSource::stb_truetype_with_size(roboto, 20.0);
        atlas.add_font(&[source])
    })?;

    fonts.roboto_medium = Some(roboto_medium);
    Ok(())
}

fn custom_font_ui(frame: ImguiFrame<'_>, fonts: NonSend<CustomFonts>) -> Result {
    let roboto_medium = fonts
        .roboto_medium
        .ok_or("the custom font should be configured before the first UI frame")?;
    let ui = frame.ui();

    ui.window("Custom Font")
        .size([440.0, 190.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Default font with a 16 px reference size.");
            ui.separator();
            {
                let _font = ui.push_font(roboto_medium);
                ui.text("Roboto Medium with a 20 px reference size.");
                ui.text("Sphinx of black quartz, judge my vow.");
            }
        });

    Ok(())
}
