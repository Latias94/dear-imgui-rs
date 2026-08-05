use bevy_ecs::prelude::Component;
use dear_imgui_rs as imgui;

use super::protocol::{ImguiViewportId, ImguiViewportInstanceId};

/// Backend-owned identity marker on Bevy `Window` entities created for secondary viewports.
///
/// Applications should query this component by reference and use its accessors. Constructing or
/// moving the marker does not transfer backend ownership.
#[derive(Component, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportWindow {
    instance_id: ImguiViewportInstanceId,
    viewport_id: ImguiViewportId,
}

impl ImguiViewportWindow {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) const fn new(
        instance_id: ImguiViewportInstanceId,
        viewport_id: ImguiViewportId,
    ) -> Self {
        Self {
            instance_id,
            viewport_id,
        }
    }

    /// Returns the Context which owns this native viewport.
    #[must_use]
    pub const fn context_id(&self) -> imgui::ContextId {
        self.instance_id.context_id()
    }

    /// Returns the stable viewport instance identity.
    #[must_use]
    pub const fn instance_id(&self) -> ImguiViewportInstanceId {
        self.instance_id
    }

    /// Returns the current Dear ImGui routing identifier within [`Self::context_id`].
    ///
    /// Docking may change this value while [`Self::instance_id`] remains stable.
    #[must_use]
    pub const fn viewport_id(&self) -> ImguiViewportId {
        self.viewport_id
    }
}

/// Backend-owned identity marker on Bevy camera entities created to render secondary viewports.
///
/// Applications should query this component by reference and use its accessors. Constructing or
/// moving the marker does not transfer backend ownership.
#[derive(Component, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportCamera {
    instance_id: ImguiViewportInstanceId,
    viewport_id: ImguiViewportId,
}

impl ImguiViewportCamera {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) const fn new(
        instance_id: ImguiViewportInstanceId,
        viewport_id: ImguiViewportId,
    ) -> Self {
        Self {
            instance_id,
            viewport_id,
        }
    }

    /// Returns the Context which owns this native viewport.
    #[must_use]
    pub const fn context_id(&self) -> imgui::ContextId {
        self.instance_id.context_id()
    }

    /// Returns the stable viewport instance identity.
    #[must_use]
    pub const fn instance_id(&self) -> ImguiViewportInstanceId {
        self.instance_id
    }

    /// Returns the current Dear ImGui routing identifier within [`Self::context_id`].
    ///
    /// Docking may change this value while [`Self::instance_id`] remains stable.
    #[must_use]
    pub const fn viewport_id(&self) -> ImguiViewportId {
        self.viewport_id
    }
}

/// Private capability paired with each public native viewport identity marker.
///
/// Public markers are observable ECS data and can be moved by applications. Backend systems
/// therefore require this unforgeable companion before treating an entity as backend-owned.
#[derive(Component, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ImguiViewportOwner {
    kind: ImguiViewportOwnerKind,
    instance_id: ImguiViewportInstanceId,
}

#[derive(Debug, Eq, PartialEq, Hash)]
enum ImguiViewportOwnerKind {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    Window,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    Camera,
}

impl ImguiViewportOwner {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(super) const fn window(instance_id: ImguiViewportInstanceId) -> Self {
        Self {
            kind: ImguiViewportOwnerKind::Window,
            instance_id,
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(super) const fn camera(instance_id: ImguiViewportInstanceId) -> Self {
        Self {
            kind: ImguiViewportOwnerKind::Camera,
            instance_id,
        }
    }

    #[must_use]
    pub(crate) fn matches_window(&self, marker: &ImguiViewportWindow) -> bool {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            matches!(&self.kind, ImguiViewportOwnerKind::Window)
                && self.instance_id == marker.instance_id
        }
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        {
            let _ = (self, marker);
            false
        }
    }

    #[cfg(feature = "render")]
    #[must_use]
    pub(crate) fn matches_camera(&self, marker: &ImguiViewportCamera) -> bool {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            matches!(&self.kind, ImguiViewportOwnerKind::Camera)
                && self.instance_id == marker.instance_id
        }
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        {
            let _ = (self, marker);
            false
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(crate) const fn window_identity(
        &self,
    ) -> Option<(imgui::ContextId, ImguiViewportInstanceId)> {
        match &self.kind {
            ImguiViewportOwnerKind::Window => {
                Some((self.instance_id.context_id(), self.instance_id))
            }
            ImguiViewportOwnerKind::Camera => None,
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[must_use]
    pub(super) const fn camera_identity(
        &self,
    ) -> Option<(imgui::ContextId, ImguiViewportInstanceId)> {
        match &self.kind {
            ImguiViewportOwnerKind::Window => None,
            ImguiViewportOwnerKind::Camera => {
                Some((self.instance_id.context_id(), self.instance_id))
            }
        }
    }
}
