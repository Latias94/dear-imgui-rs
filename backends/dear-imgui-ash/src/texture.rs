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
    /// Apply a legacy renderer transition directly to a `TextureData` object.
    ///
    /// Managed textures must use the request-bound rendering API instead.
    ///
    /// # Safety
    ///
    /// `self` must be the result produced for this exact texture by the same renderer, all GPU
    /// synchronization required by the transition must be complete, and `texture_data` must not be
    /// registered with a `Context`.
    pub unsafe fn apply_to(self, texture_data: &mut TextureData) {
        unsafe {
            match self {
                TextureUpdateResult::Created { texture_id } => {
                    texture_data.set_tex_id(texture_id);
                    texture_data.set_status(TextureStatus::OK);
                }
                TextureUpdateResult::Updated => {
                    texture_data.set_status(TextureStatus::OK);
                }
                TextureUpdateResult::Destroyed => {
                    // ImGui's SetStatus(Destroyed) has special semantics: if
                    // WantDestroyNextFrame is false, Destroyed may translate back to WantCreate.
                    // When honoring a requested destroy, set WantDestroyNextFrame first.
                    (*texture_data.as_raw_mut()).WantDestroyNextFrame = true;
                    texture_data.set_status(TextureStatus::Destroyed);
                }
                TextureUpdateResult::Failed => {
                    // Best-effort: mark destroyed. If this was not a requested destroy, ImGui may
                    // translate this back to WantCreate, which is acceptable.
                    texture_data.set_status(TextureStatus::Destroyed);
                }
                TextureUpdateResult::NoAction => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::{OwnedTextureData, TextureFormat};

    #[test]
    fn texture_update_result_apply_to_sets_status_and_id() {
        let mut tex = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();

        unsafe {
            // The test applies transitions produced for its sole unregistered texture.
            TextureUpdateResult::Created {
                texture_id: TextureId::from(42u64),
            }
            .apply_to(&mut tex);
        }
        assert_eq!(tex.status(), TextureStatus::OK);
        assert_eq!(tex.tex_id().id(), 42);

        unsafe {
            TextureUpdateResult::Updated.apply_to(&mut tex);
        }
        assert_eq!(tex.status(), TextureStatus::OK);
        assert_eq!(tex.tex_id().id(), 42);

        unsafe {
            TextureUpdateResult::Destroyed.apply_to(&mut tex);
        }
        assert_eq!(tex.status(), TextureStatus::Destroyed);
        unsafe {
            assert!((*tex.as_raw()).WantDestroyNextFrame);
        }

        unsafe {
            (*tex.as_raw_mut()).WantDestroyNextFrame = false;
        }
        tex = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();
        unsafe {
            TextureUpdateResult::Failed.apply_to(&mut tex);
        }
        assert_eq!(tex.status(), TextureStatus::WantCreate);

        unsafe {
            TextureUpdateResult::NoAction.apply_to(&mut tex);
        }
        assert_eq!(tex.status(), TextureStatus::WantCreate);
    }
}
