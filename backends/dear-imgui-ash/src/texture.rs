//! Texture helpers for the Ash renderer backend.
//!
//! This mirrors the pattern used by the WGPU backend: expose a small result type that can be
//! applied to an `ImTextureData` (`TextureData`) without requiring the backend to take a mutable
//! reference during upload scheduling.

use dear_imgui_rs::{TextureData, TextureId, TextureStatus};

/// Result of a texture update operation.
#[derive(Debug, Clone)]
pub enum TextureUpdateResult {
    /// Texture was successfully created.
    Created { texture_id: TextureId },
    /// Texture was successfully updated.
    Updated,
    /// Texture was destroyed.
    Destroyed,
    /// Texture update failed.
    Failed,
    /// No action was needed.
    NoAction,
}

impl TextureUpdateResult {
    /// Apply the result to the `TextureData` object.
    pub fn apply_to(self, texture_data: &mut TextureData) {
        match self {
            TextureUpdateResult::Created { texture_id } => {
                texture_data.set_tex_id(texture_id);
                texture_data.set_status(TextureStatus::OK);
            }
            TextureUpdateResult::Updated => {
                texture_data.set_status(TextureStatus::OK);
            }
            TextureUpdateResult::Destroyed => unsafe {
                // ImGui's SetStatus(Destroyed) has special semantics: if WantDestroyNextFrame is
                // false, Destroyed may translate back to WantCreate. When honoring a requested
                // destroy, we must set WantDestroyNextFrame first.
                (*texture_data.as_raw_mut()).WantDestroyNextFrame = true;
                texture_data.set_status(TextureStatus::Destroyed);
            },
            TextureUpdateResult::Failed => {
                // Best-effort: mark destroyed. If this was not a requested destroy, ImGui may
                // translate this back to WantCreate, which is acceptable.
                texture_data.set_status(TextureStatus::Destroyed);
            }
            TextureUpdateResult::NoAction => {}
        }
    }
}
