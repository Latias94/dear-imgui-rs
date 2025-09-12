//! ImGui context management for Bevy integration

use bevy::{
    asset::{Handle, StrongHandle},
    image::Image,
    prelude::*,
};
use dear_imgui::{Context, OwnedDrawData};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

// use crate::texture::TextureRegistry; // TODO: Re-enable when texture system is complete

/// The ImGui context resource for Bevy integration.
///
/// This resource manages the Dear ImGui context and provides safe access
/// to the UI building functionality. It should be accessed as a `NonSendMut` resource
/// in Bevy systems, as Dear ImGui contexts are not thread-safe.
///
/// # Safety
/// This type does NOT implement Send/Sync, ensuring it can only be used
/// as a NonSend resource in Bevy, which is the correct and safe approach.
#[derive(Resource)]
pub struct ImguiContext {
    /// The underlying Dear ImGui context
    ctx: RwLock<Context>,
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
    pub(crate) fn new(ctx: Context) -> Self {
        Self {
            ctx: RwLock::new(ctx),
            textures: HashMap::new(),
            texture_modify: RwLock::new(TextureModifyState::default()),
            rendered_draw_data: RwLock::new(OwnedDrawData::default()),
        }
    }

    /// Execute a closure with mutable access to the Dear ImGui context
    ///
    /// This is the safe way to access the context for operations that need
    /// mutable access, such as creating frames or modifying settings.
    pub fn with_context<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Context) -> R,
    {
        let mut ctx = self.ctx.write().expect("Failed to acquire context lock");
        f(&mut ctx)
    }

    /// Execute a closure with a complete Dear ImGui frame
    ///
    /// This method manages the complete frame lifecycle:
    /// 1. Starts a new frame
    /// 2. Executes the user closure with the UI
    /// 3. Does NOT call render - that's handled by the end frame system
    ///
    /// This is the recommended way for user systems to interact with Dear ImGui.
    pub fn with_ui<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut dear_imgui::Ui) -> R,
    {
        let mut ctx = self.ctx.write().expect("Failed to acquire context lock");
        let ui = ctx.frame();
        f(ui)
    }

    /// Execute a closure with read-only access to the Dear ImGui context
    pub fn with_context_read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Context) -> R,
    {
        let ctx = self.ctx.read().expect("Failed to acquire context lock");
        f(&ctx)
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

// Note: We do NOT implement Send/Sync for ImguiContext
// This ensures it can only be used as a NonSend resource in Bevy,
// which is the correct and safe way to handle Dear ImGui contexts.
// Dear ImGui contexts are not thread-safe and must be accessed from
// the main thread only.
