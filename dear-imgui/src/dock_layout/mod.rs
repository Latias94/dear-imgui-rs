//! Declarative docking layouts.
//!
//! A [`DockLayout`] describes the complete dock tree. Applying it through
//! [`Ui::dockspace`](crate::Ui::dockspace) owns the full docking-layout lifecycle, including
//! validation, splitting, window assignment, and finalization.
//!
//! Submit a layout before any window named by the layout or already hosted by its dock tree. This
//! ordering is required by Dear ImGui and is enforced before a safe replacement can mutate native
//! nodes. Validation and recoverable staging failures keep an existing root alive without changing
//! its flags, window class, topology, or persisted settings.
//!
//! Advanced integrations that need Dear ImGui's unstable imperative docking API can use the
//! raw `dear_imgui_rs::sys::igDockBuilder*` functions explicitly.
//!
//! The provisional compatibility types are intentionally absent:
//!
//! ```compile_fail
//! use dear_imgui_rs::{DockLayoutError, DockspaceOptions};
//! ```

mod compile;

#[cfg(test)]
mod tests;

pub(crate) use compile::{DockspaceHost, submit_and_apply, submit_without_layout};

use crate::error::context::ContextError;
use crate::{DockNodeFlags, Id, WindowClass, WindowKey};

/// A complete declarative dock tree.
#[derive(Clone, Debug, PartialEq)]
pub enum DockLayout {
    /// One leaf node containing zero or more tabbed windows.
    ///
    /// An empty list intentionally leaves the leaf available as an empty docking target.
    /// Dear ImGui's builder does not preserve the relative order of these windows.
    Tabs(Vec<WindowKey>),
    /// Split a node and recursively populate both resulting children.
    Split {
        /// Side occupied by `first`.
        direction: DockSplit,
        /// Fraction of the parent occupied by `first`.
        ratio: f32,
        /// Layout placed on `direction`'s side.
        first: Box<DockLayout>,
        /// Layout placed in the remaining space.
        second: Box<DockLayout>,
    },
}

impl DockLayout {
    /// Create one tab leaf from stable window keys.
    pub fn tabs(windows: impl IntoIterator<Item = impl Into<WindowKey>>) -> Self {
        Self::Tabs(windows.into_iter().map(Into::into).collect())
    }

    /// Split a node into a directional child and the remaining child.
    pub fn split(direction: DockSplit, ratio: f32, first: DockLayout, second: DockLayout) -> Self {
        Self::Split {
            direction,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Validate the complete layout without touching Dear ImGui state.
    pub fn validate(&self) -> Result<(), ContextError> {
        compile::compile_layout(self).map(|_| ())
    }
}

/// Direction of the first child produced by a [`DockLayout::Split`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DockSplit {
    Left,
    Right,
    Up,
    Down,
}

/// Policy controlling whether an existing persisted dock tree is preserved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DockLayoutApply {
    /// Build only when the root did not exist before this frame's dockspace submission.
    #[default]
    IfMissing,
    /// Replace the existing root with the complete declared layout.
    ///
    /// The topology is staged under a temporary root before the native commit point. Validation
    /// and recoverable staging failures leave an existing layout unchanged and alive for the
    /// current frame. A successful replacement preserves the root ID, but child-node IDs, tab
    /// selection, focus, and relative tab order are not stable across replacement. Replacement
    /// must be submitted before any affected window begins its frame.
    Replace,
}

#[derive(Clone, Debug)]
pub(crate) struct DockspaceConfig {
    root_id: Id,
    flags: DockNodeFlags,
    window_class: Option<WindowClass>,
}

impl DockspaceConfig {
    pub(crate) fn new(
        root_id: Id,
        flags: DockNodeFlags,
        window_class: Option<WindowClass>,
    ) -> Self {
        Self {
            root_id,
            flags,
            window_class,
        }
    }

    pub(crate) fn root_id(&self) -> Id {
        self.root_id
    }

    pub(crate) fn dock_flags(&self) -> DockNodeFlags {
        self.flags
    }

    pub(crate) fn window_class_ref(&self) -> Option<&WindowClass> {
        self.window_class.as_ref()
    }

    pub(crate) fn validate(&self) -> Result<(), ContextError> {
        if self.root_id.raw() == 0 {
            return Err(ContextError::ZeroRootId);
        }

        let unsupported = self.flags.bits() & !DockNodeFlags::all().bits();
        if unsupported != 0 {
            return Err(ContextError::UnsupportedDockNodeFlags { bits: unsupported });
        }

        if let Some(window_class) = &self.window_class {
            window_class.validate()?;
        }

        Ok(())
    }
}
