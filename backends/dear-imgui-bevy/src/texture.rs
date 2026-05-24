//! Texture interop resources for the Bevy backend.
//!
//! This module owns the main-world texture-facing API. Renderer feedback for ImGui-managed
//! textures is queued here and applied on the next UI-thread frame, while Bevy `Handle<Image>`
//! registrations are converted into stable legacy [`TextureId`](dear_imgui_rs::TextureId)
//! values that render-world code can resolve through Bevy's `RenderAssets<GpuImage>`.

use bevy_ecs::resource::Resource;
use dear_imgui_rs as imgui;
use std::sync::{Arc, Mutex};

/// Main-world queue of managed texture feedback produced by the render world.
#[derive(Resource, Debug, Clone, Default)]
pub struct ImguiTextureFeedbackQueue {
    feedback: Arc<Mutex<Vec<imgui::render::snapshot::TextureFeedback>>>,
    last_applied: usize,
}

impl ImguiTextureFeedbackQueue {
    /// Queue one managed texture feedback item to be applied before the next frame begins.
    pub fn push(&self, feedback: imgui::render::snapshot::TextureFeedback) {
        self.feedback
            .lock()
            .expect("ImguiTextureFeedbackQueue mutex poisoned")
            .push(feedback);
    }

    /// Number of queued feedback items waiting for UI-thread application.
    #[must_use]
    pub fn len(&self) -> usize {
        self.feedback
            .lock()
            .expect("ImguiTextureFeedbackQueue mutex poisoned")
            .len()
    }

    /// Whether no feedback items are waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of texture entries updated by the most recent drain/apply pass.
    #[must_use]
    pub fn last_applied(&self) -> usize {
        self.last_applied
    }

    pub(crate) fn drain(&self) -> Vec<imgui::render::snapshot::TextureFeedback> {
        std::mem::take(
            &mut *self
                .feedback
                .lock()
                .expect("ImguiTextureFeedbackQueue mutex poisoned"),
        )
    }

    pub(crate) fn set_last_applied(&mut self, applied: usize) {
        self.last_applied = applied;
    }
}

#[cfg(feature = "render")]
mod render {
    use super::*;
    use bevy_asset::{AssetId, Handle};
    use bevy_image::Image;
    use std::collections::HashMap;

    const BEVY_IMAGE_TEXTURE_NAMESPACE: u64 = 0x8000_0000_0000_0000;

    /// Main-world registry that maps Bevy `Image` handles to Dear ImGui legacy texture ids.
    #[derive(Resource, Debug)]
    pub struct ImguiBevyTextures {
        by_asset: HashMap<AssetId<Image>, imgui::TextureId>,
        by_texture: HashMap<imgui::TextureId, AssetId<Image>>,
        next_texture_id: u64,
    }

    impl Default for ImguiBevyTextures {
        fn default() -> Self {
            Self {
                by_asset: HashMap::new(),
                by_texture: HashMap::new(),
                next_texture_id: BEVY_IMAGE_TEXTURE_NAMESPACE,
            }
        }
    }

    impl ImguiBevyTextures {
        /// Register a Bevy image handle and return the Dear ImGui texture id used by `ui.image`.
        ///
        /// Registering the same handle repeatedly is idempotent and returns the same texture id.
        pub fn register(&mut self, image: &Handle<Image>) -> imgui::TextureId {
            let asset_id = image.id();
            if let Some(texture_id) = self.by_asset.get(&asset_id) {
                return *texture_id;
            }

            let texture_id = self.allocate_texture_id();
            self.by_asset.insert(asset_id, texture_id);
            self.by_texture.insert(texture_id, asset_id);
            texture_id
        }

        /// Remove a registered image handle.
        pub fn unregister(&mut self, image: &Handle<Image>) -> Option<imgui::TextureId> {
            let texture_id = self.by_asset.remove(&image.id())?;
            self.by_texture.remove(&texture_id);
            Some(texture_id)
        }

        /// Resolve a registered Dear ImGui texture id back to the Bevy image asset id.
        #[must_use]
        pub fn asset_id(&self, texture_id: imgui::TextureId) -> Option<AssetId<Image>> {
            self.by_texture.get(&texture_id).copied()
        }

        /// Iterate registered texture id to Bevy asset id pairs.
        pub fn iter(&self) -> impl Iterator<Item = (imgui::TextureId, AssetId<Image>)> + '_ {
            self.by_texture
                .iter()
                .map(|(texture_id, asset_id)| (*texture_id, *asset_id))
        }

        /// Number of registered Bevy image textures.
        #[must_use]
        pub fn len(&self) -> usize {
            self.by_texture.len()
        }

        /// Whether no Bevy image textures are registered.
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.by_texture.is_empty()
        }

        fn allocate_texture_id(&mut self) -> imgui::TextureId {
            if self.next_texture_id < BEVY_IMAGE_TEXTURE_NAMESPACE {
                self.next_texture_id = BEVY_IMAGE_TEXTURE_NAMESPACE;
            }

            loop {
                let texture_id = imgui::TextureId::new(self.next_texture_id);
                self.next_texture_id = self.next_texture_id.wrapping_add(1);
                if self.next_texture_id < BEVY_IMAGE_TEXTURE_NAMESPACE {
                    self.next_texture_id = BEVY_IMAGE_TEXTURE_NAMESPACE;
                }
                if !texture_id.is_null() && !self.by_texture.contains_key(&texture_id) {
                    return texture_id;
                }
            }
        }
    }
}

#[cfg(feature = "render")]
pub use render::ImguiBevyTextures;
