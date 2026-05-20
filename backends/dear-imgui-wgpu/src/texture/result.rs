use super::*;

/// Result of a texture update operation
///
/// This enum represents the outcome of a texture update operation and
/// contains any state changes that need to be applied to the texture data.
/// This follows Rust's principle of explicit state management.
#[derive(Debug, Clone)]
pub enum TextureUpdateResult {
    /// Texture was successfully created
    Created { texture_id: TextureId },
    /// Texture was successfully updated
    Updated,
    /// Texture was destroyed
    Destroyed,
    /// Texture update failed
    Failed,
    /// No action was needed
    NoAction,
}

impl TextureUpdateResult {
    /// Apply the result to a texture data object
    ///
    /// This method updates the texture data's status and ID based on the operation result.
    /// This is the Rust-idiomatic way to handle state updates.
    pub fn apply_to(self, texture_data: &mut TextureData) {
        match self {
            TextureUpdateResult::Created { texture_id } => {
                texture_data.set_tex_id(texture_id);
                texture_data.set_status(TextureStatus::OK);
            }
            TextureUpdateResult::Updated => {
                texture_data.set_status(TextureStatus::OK);
            }
            TextureUpdateResult::Destroyed => mark_texture_destroyed(texture_data),
            TextureUpdateResult::Failed => {
                texture_data.set_status(TextureStatus::Destroyed);
            }
            TextureUpdateResult::NoAction => {
                // No changes needed
            }
        }
    }
}

pub(super) fn mark_texture_destroyed(texture_data: &mut TextureData) {
    unsafe {
        // ImGui's SetStatus(Destroyed) has special semantics: if WantDestroyNextFrame is false,
        // Destroyed may translate back to WantCreate. When honoring a requested destroy, set it
        // before writing the final status.
        (*texture_data.as_raw_mut()).WantDestroyNextFrame = true;
    }
    texture_data.set_status(TextureStatus::Destroyed);
}
