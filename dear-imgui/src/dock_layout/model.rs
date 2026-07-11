use crate::{DockNodeFlags, Id, WindowClass};
use thiserror::Error;

/// A complete declarative dock tree.
#[derive(Clone, Debug, PartialEq)]
pub enum DockLayout {
    /// One leaf node containing zero or more tabbed windows.
    ///
    /// An empty list intentionally leaves the leaf available as an empty docking target.
    Tabs(Vec<String>),
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
    /// Create one tab leaf from window titles.
    pub fn tabs(windows: impl IntoIterator<Item = impl Into<String>>) -> Self {
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
    pub fn validate(&self) -> Result<(), DockLayoutError> {
        super::compile::compile_layout(self).map(|_| ())
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
    /// Clear the existing root's docked windows and child tree, then build the declared layout.
    Replace,
}

/// Stable target and initial geometry for a declarative dockspace.
///
/// Main-viewport submission uses `initial_position`. Current-window submission derives its
/// position from the actual cursor so the host window and dock node cannot diverge.
#[derive(Clone, Debug)]
pub struct DockspaceTarget {
    root_id: Id,
    flags: DockNodeFlags,
    window_class: Option<WindowClass>,
    initial_position: [f32; 2],
    initial_size: [f32; 2],
}

impl DockspaceTarget {
    /// Create a target with no dock flags or window class.
    pub fn new(
        root_id: Id,
        initial_position: [f32; 2],
        initial_size: [f32; 2],
    ) -> Result<Self, DockLayoutError> {
        let target = Self {
            root_id,
            flags: DockNodeFlags::NONE,
            window_class: None,
            initial_position,
            initial_size,
        };
        target.validate()?;
        Ok(target)
    }

    /// Set the public dock node flags used when submitting the dockspace.
    #[must_use]
    pub fn flags(mut self, flags: DockNodeFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the optional window class used when submitting the dockspace.
    #[must_use]
    pub fn window_class(mut self, window_class: WindowClass) -> Self {
        self.window_class = Some(window_class);
        self
    }

    pub fn root_id(&self) -> Id {
        self.root_id
    }

    pub(crate) fn dock_flags(&self) -> DockNodeFlags {
        self.flags
    }

    pub(crate) fn window_class_ref(&self) -> Option<&WindowClass> {
        self.window_class.as_ref()
    }

    /// Return the initial position used by main-viewport submission.
    pub fn initial_position(&self) -> [f32; 2] {
        self.initial_position
    }

    pub fn initial_size(&self) -> [f32; 2] {
        self.initial_size
    }

    /// Validate target identity, flags, and initial geometry.
    pub fn validate(&self) -> Result<(), DockLayoutError> {
        if self.root_id.raw() == 0 {
            return Err(DockLayoutError::ZeroRootId);
        }

        let unsupported = self.flags.bits() & !DockNodeFlags::all().bits();
        if unsupported != 0 {
            return Err(DockLayoutError::UnsupportedDockNodeFlags { bits: unsupported });
        }

        if !self.initial_position.iter().all(|value| value.is_finite()) {
            return Err(DockLayoutError::NonFiniteInitialPosition {
                position: self.initial_position,
            });
        }

        if !self
            .initial_size
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        {
            return Err(DockLayoutError::InvalidInitialSize {
                size: self.initial_size,
            });
        }

        Ok(())
    }
}

/// Validation or application failure for a declarative docking layout.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum DockLayoutError {
    #[error("dockspace root ID must be non-zero")]
    ZeroRootId,
    #[error("dockspace flags contain unsupported ImGuiDockNodeFlags bits: 0x{bits:X}")]
    UnsupportedDockNodeFlags { bits: i32 },
    #[error("dockspace initial position must contain finite values: {position:?}")]
    NonFiniteInitialPosition { position: [f32; 2] },
    #[error("dockspace initial size must contain positive finite values: {size:?}")]
    InvalidInitialSize { size: [f32; 2] },
    #[error("dock split ratio must be finite and strictly between 0 and 1: {ratio}")]
    InvalidSplitRatio { ratio: f32 },
    #[error("dock window title must not be empty")]
    EmptyWindowTitle,
    #[error("dock window title contains an interior NUL byte: {title:?}")]
    WindowTitleContainsNul { title: String },
    #[error("dock window title {title:?} has an empty stable ID after `###`")]
    EmptyWindowId { title: String },
    #[error(
        "dock window titles {first_title:?} and {second_title:?} resolve to the same Dear ImGui stable ID"
    )]
    DuplicateWindowId {
        first_title: String,
        second_title: String,
    },
    #[error("dock layout contains too many nodes")]
    LayoutTooLarge,
    #[error("docking is not enabled in ConfigFlags")]
    DockingDisabled,
    #[error("dockspace submission did not create root {root_id:?}")]
    DockspaceSubmissionFailed { root_id: Id },
    #[error("dockspace root disappeared while resetting layout {root_id:?}")]
    RootResetFailed { root_id: Id },
    #[error("compiled dock node {index} was unavailable during application")]
    CompiledNodeUnavailable { index: usize },
    #[error("Dear ImGui failed to split a docking node {direction:?} at ratio {ratio}")]
    SplitFailed { direction: DockSplit, ratio: f32 },
}
