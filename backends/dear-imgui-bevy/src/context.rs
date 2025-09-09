//! ImGui context management for Bevy integration

use bevy::{
    asset::{Handle, StrongHandle},
    image::Image,
    prelude::*,
};
use dear_imgui::{Context, OwnedDrawData};
use std::{
    collections::HashMap,
    ptr::NonNull,
    sync::{Arc, RwLock},
};

/// The ImGui context resource for Bevy integration.
///
/// This resource manages the Dear ImGui context and provides thread-safe access
/// to the UI building functionality. It should be accessed as a `NonSendMut` resource
/// in Bevy systems.
#[derive(Resource)]
pub struct ImguiContext {
    /// The underlying Dear ImGui context
    ctx: RwLock<Context>,
    /// Current UI pointer (only valid during frame rendering)
    ui: Option<NonNull<dear_imgui::Ui>>,
    /// Registered Bevy textures mapped to ImGui texture IDs
    textures: HashMap<u32, Arc<StrongHandle>>,
    /// Texture modification state for render thread synchronization
    texture_modify: RwLock<TextureModifyState>,
    /// Rendered draw data for the current frame
    rendered_draw_data: RwLock<OwnedDrawData>,
}

/// Internal state for tracking texture modifications
#[derive(Default)]
struct TextureModifyState {
    /// Textures to be added in the next render frame
    pub(crate) to_add: Vec<u32>,
    /// Textures to be removed in the next render frame
    pub(crate) to_remove: Vec<u32>,
    /// Next available texture ID
    next_free_id: usize,
}

impl ImguiContext {
    /// Create a new ImGui context
    pub(crate) fn new(mut ctx: Context) -> Self {
        // Key mappings will be handled in the input system

        Self {
            ctx: RwLock::new(ctx),
            ui: None,
            textures: HashMap::new(),
            texture_modify: RwLock::new(TextureModifyState::default()),
            rendered_draw_data: RwLock::new(OwnedDrawData::default()),
        }
    }

    /// Get mutable access to the current UI frame.
    ///
    /// This method should only be called during the Update phase when a frame is active.
    /// Panics if called outside of an active ImGui frame.
    pub fn ui(&mut self) -> &mut dear_imgui::Ui {
        unsafe {
            self.ui
                .expect("Not currently rendering an imgui frame! Make sure to call this during Update phase.")
                .as_mut()
        }
    }

    /// Register a Bevy texture with ImGui.
    ///
    /// The provided Handle must be strong, and the texture will be kept alive
    /// until `unregister_bevy_texture` is called. Returns an ImGui texture ID
    /// that can be used in ImGui draw calls.
    pub fn register_bevy_texture(&mut self, handle: Handle<Image>) -> u32 {
        if let Handle::Strong(strong) = handle {
            let mut texture_modify = self.texture_modify.write().unwrap();
            let result = texture_modify.next_free_id as u32;
            self.textures.insert(result, strong.clone());
            texture_modify.to_add.push(result);
            texture_modify.next_free_id += 1;
            result
        } else {
            panic!("register_bevy_texture requires a strong Handle<Image>");
        }
    }

    /// Unregister a previously registered Bevy texture.
    ///
    /// The texture_id must have been returned by a previous call to `register_bevy_texture`.
    pub fn unregister_bevy_texture(&mut self, texture_id: u32) {
        self.textures.remove(&texture_id);
        self.texture_modify
            .write()
            .unwrap()
            .to_remove
            .push(texture_id);
    }

    /// Get the underlying Dear ImGui context (internal use)
    pub(crate) fn context(&self) -> &RwLock<Context> {
        &self.ctx
    }

    /// Set the current UI pointer (internal use)
    pub(crate) fn set_ui(&mut self, ui: Option<NonNull<dear_imgui::Ui>>) {
        self.ui = ui;
    }

    /// Get texture modification state (internal use)
    pub(crate) fn texture_modify(&self) -> &RwLock<TextureModifyState> {
        &self.texture_modify
    }

    /// Get registered textures (internal use)
    pub(crate) fn textures(&self) -> &HashMap<u32, Arc<StrongHandle>> {
        &self.textures
    }

    /// Get rendered draw data (internal use)
    pub(crate) fn rendered_draw_data(&self) -> &RwLock<OwnedDrawData> {
        &self.rendered_draw_data
    }
}

// Safety: ImguiContext is designed to be used as NonSend in Bevy
// The RwLock ensures thread safety for the parts that need it
unsafe impl Send for ImguiContext {}
unsafe impl Sync for ImguiContext {}
