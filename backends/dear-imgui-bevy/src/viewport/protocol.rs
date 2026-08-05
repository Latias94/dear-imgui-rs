use dear_imgui_rs as imgui;

/// Dear ImGui viewport's current numeric routing identifier.
///
/// Docking may change this value in place while preserving the native viewport and its Bevy
/// window. Use [`ImguiViewportInstanceId`] when identity must survive across frames.
pub type ImguiViewportId = imgui::Id;

/// Stable opaque identity for one Bevy-managed Dear ImGui viewport instance.
///
/// Unlike [`ImguiViewportId`], this value does not change when docking transfers a live native
/// viewport to another Dear ImGui window. Values are scoped by their owning Context and are never
/// reused during that Context's bridge lifetime.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImguiViewportInstanceId {
    pub(super) context_id: imgui::ContextId,
    pub(super) generation: std::num::NonZeroU64,
}

impl ImguiViewportInstanceId {
    /// Returns the Context which owns this viewport instance.
    #[must_use]
    pub const fn context_id(self) -> imgui::ContextId {
        self.context_id
    }
}

/// Snapshot of Dear ImGui viewport state copied while a PlatformIO callback is running.
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImguiViewportSnapshot {
    pub id: ImguiViewportId,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub dpi_scale: f32,
    pub flags: imgui::ViewportFlags,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportSnapshot {
    #[must_use]
    pub fn from_viewport(viewport: &imgui::Viewport) -> Self {
        Self {
            id: viewport.id(),
            pos: viewport.pos(),
            size: viewport.size(),
            // SAFETY: `viewport` is live for this callback. Only scalar state is copied.
            dpi_scale: unsafe { (*viewport.as_raw()).DpiScale },
            flags: viewport.flags(),
        }
    }
}

/// Internal intent captured from Dear ImGui PlatformIO callbacks.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ImguiViewportCommand {
    Create(ImguiViewportSnapshot),
    Destroy {
        id: ImguiViewportId,
    },
    Show {
        id: ImguiViewportId,
    },
    Update {
        id: ImguiViewportId,
        previous_flags: Option<imgui::ViewportFlags>,
        flags: imgui::ViewportFlags,
    },
    SetPos {
        id: ImguiViewportId,
        pos: [f32; 2],
        dpi_scale: f32,
    },
    SetSize {
        id: ImguiViewportId,
        size: [f32; 2],
        dpi_scale: f32,
    },
    SetFocus {
        id: ImguiViewportId,
    },
    SetTitle {
        id: ImguiViewportId,
        title: String,
    },
}

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportCommand {
    pub(super) fn current_id(&self) -> ImguiViewportId {
        match self {
            Self::Create(snapshot) => snapshot.id,
            Self::Destroy { id }
            | Self::Show { id }
            | Self::Update { id, .. }
            | Self::SetPos { id, .. }
            | Self::SetSize { id, .. }
            | Self::SetFocus { id }
            | Self::SetTitle { id, .. } => *id,
        }
    }
}

/// Last Bevy-observed platform state for a Dear ImGui viewport window.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImguiViewportFeedback {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
    pub dpi_scale: f32,
    pub focused: bool,
    pub minimized: bool,
}
