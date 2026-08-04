use std::collections::HashMap;

use bevy_ecs::prelude::Resource;
use dear_imgui_rs as imgui;

/// Native desktop-position capability observed for one Bevy-managed Dear ImGui Context.
///
/// Dear ImGui's native multi-viewport contract needs both global client-area positions and
/// native window positioning. On Wayland this remains unavailable by protocol design, so the
/// backend keeps docking in the host window instead of advertising unusable platform viewports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiNativeViewportStatus {
    /// A native host window has not been created yet.
    PendingNativeWindow,
    /// Native platform viewports may be enabled.
    Available,
    /// The active window system does not provide global desktop coordinates.
    GlobalDesktopCoordinatesUnavailable,
}

/// Per-Context native multi-viewport capability observed during the latest frame.
///
/// A Context is absent until it opts into native multi-viewport and enters the private Context
/// driver. Removed, disabled, and not-yet-driven Contexts are not retained.
#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct ImguiNativeViewportSupport {
    contexts: HashMap<imgui::ContextId, ImguiNativeViewportStatus>,
}

impl ImguiNativeViewportSupport {
    /// Return the latest native multi-viewport status for `context_id`.
    #[must_use]
    pub fn get(&self, context_id: imgui::ContextId) -> Option<ImguiNativeViewportStatus> {
        self.contexts.get(&context_id).copied()
    }

    /// Iterate the Contexts which currently request native multi-viewport support.
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (imgui::ContextId, ImguiNativeViewportStatus)> + '_ {
        self.contexts
            .iter()
            .map(|(context_id, status)| (*context_id, *status))
    }

    /// Return whether native platform windows are available for `context_id`.
    #[must_use]
    pub fn is_available(&self, context_id: imgui::ContextId) -> bool {
        self.get(context_id) == Some(ImguiNativeViewportStatus::Available)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn begin_frame(&mut self, contexts: impl IntoIterator<Item = imgui::ContextId>) {
        self.contexts.clear();
        self.contexts.extend(
            contexts
                .into_iter()
                .map(|context_id| (context_id, ImguiNativeViewportStatus::PendingNativeWindow)),
        );
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn set(&mut self, context_id: imgui::ContextId, status: ImguiNativeViewportStatus) {
        self.contexts.insert(context_id, status);
    }
}
