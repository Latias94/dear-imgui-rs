//! Bevy integration for Dear ImGui
//!
//! This crate provides a Bevy plugin for integrating Dear ImGui into Bevy applications,
//! using the dear-imgui and dear-imgui-wgpu crates.
//!
//! # Quick Start
//!
//! ```no_run
//! use bevy::prelude::*;
//! use dear_imgui_bevy::prelude::*;
//!
//! #[derive(Resource)]
//! struct UiState {
//!     demo_window_open: bool,
//! }
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(ImguiPlugin::default())
//!         .insert_resource(UiState { demo_window_open: true })
//!         .add_systems(Startup, setup)
//!         .add_systems(Update, ui_system)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn(Camera3d::default());
//! }
//!
//! fn ui_system(context: NonSendMut<ImguiContext>, mut state: ResMut<UiState>) {
//!     context.with_context(|ctx| {
//!         let ui = ctx.frame();
//!         if state.demo_window_open {
//!             ui.show_demo_window(&mut state.demo_window_open);
//!         }
//!     });
//! }
//! ```

mod context;
mod plugin;
mod render_impl;
mod shaders;
mod systems;
mod texture;

pub use context::ImguiContext;
pub use plugin::ImguiPlugin;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{ImguiContext, ImguiPlugin};
    pub use dear_imgui::*;
}
