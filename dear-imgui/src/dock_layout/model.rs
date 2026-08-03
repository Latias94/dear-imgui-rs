use crate::{DockNodeFlags, Id, WindowClass, WindowClassError};
use thiserror::Error;

/// A complete declarative dock tree.
#[derive(Clone, Debug, PartialEq)]
pub enum DockLayout {
    /// One leaf node containing zero or more tabbed windows.
    ///
    /// An empty list intentionally leaves the leaf available as an empty docking target.
    /// Dear ImGui's builder does not preserve the relative order of these windows.
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
    /// Replace the existing root with the complete declared layout.
    ///
    /// The topology is staged under a temporary root before the native commit point. Validation
    /// and recoverable staging failures leave an existing layout unchanged and alive for the
    /// current frame. A successful replacement preserves the root ID, but child-node IDs, tab
    /// selection, focus, and relative tab order are not stable across replacement. Replacement
    /// must be submitted before any affected window begins its frame.
    Replace,
}

/// Stable identity and policy for a declarative dockspace.
///
/// The ID must either be unused or identify the explicit root previously owned by these options.
/// An ID that resolves to another tree's child or to an implicit node is rejected before mutation.
///
/// Submission flags and per-window node policies are intentionally distinct types:
///
/// ```compile_fail
/// # use dear_imgui_rs::{DockspaceOptions, Id, WindowClassDockNodeFlags};
/// let options = DockspaceOptions::new(Id::from(1u32)).unwrap()
///     .flags(WindowClassDockNodeFlags::NO_RESIZE);
/// # let _ = options;
/// ```
#[derive(Clone, Debug)]
pub struct DockspaceOptions {
    root_id: Id,
    flags: DockNodeFlags,
    window_class: Option<WindowClass>,
}

impl DockspaceOptions {
    /// Create options with no dock flags or window class.
    pub fn new(root_id: Id) -> Result<Self, DockLayoutError> {
        let options = Self {
            root_id,
            flags: DockNodeFlags::NONE,
            window_class: None,
        };
        options.validate()?;
        Ok(options)
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

    /// Return the stable root ID.
    pub fn root_id(&self) -> Id {
        self.root_id
    }

    /// Return the dockspace submission flags.
    pub fn dock_node_flags(&self) -> DockNodeFlags {
        self.flags
    }

    pub(crate) fn dock_flags(&self) -> DockNodeFlags {
        self.dock_node_flags()
    }

    /// Return the configured window class, if any.
    pub fn configured_window_class(&self) -> Option<&WindowClass> {
        self.window_class.as_ref()
    }

    pub(crate) fn window_class_ref(&self) -> Option<&WindowClass> {
        self.configured_window_class()
    }

    /// Validate identity and all typed policies without touching Dear ImGui state.
    pub fn validate(&self) -> Result<(), DockLayoutError> {
        if self.root_id.raw() == 0 {
            return Err(DockLayoutError::ZeroRootId);
        }

        let unsupported = self.flags.bits() & !DockNodeFlags::all().bits();
        if unsupported != 0 {
            return Err(DockLayoutError::UnsupportedDockNodeFlags { bits: unsupported });
        }

        if let Some(window_class) = &self.window_class {
            window_class.validate()?;
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
    #[error("dockspace host position must contain finite values: {position:?}")]
    InvalidHostPosition { position: [f32; 2] },
    #[error(
        "dockspace host size must be positive, finite, and safely truncatable to i32: {size:?}"
    )]
    InvalidHostSize { size: [f32; 2] },
    #[error("dockspace host window name is {bytes} bytes; at most {max_bytes} bytes are supported")]
    HostWindowNameTooLong { bytes: usize, max_bytes: usize },
    #[error("invalid dockspace window class: {0}")]
    InvalidWindowClass(#[from] WindowClassError),
    #[error("dock split ratio must be finite and strictly between 0 and 1: {ratio}")]
    InvalidSplitRatio { ratio: f32 },
    #[error("dock window title must not be empty")]
    EmptyWindowTitle,
    #[error("dock window title contains an interior NUL byte: {title:?}")]
    WindowTitleContainsNul { title: String },
    #[error("dock window title {title:?} has an empty stable ID after `###`")]
    EmptyWindowId { title: String },
    #[error(
        "dock window titles {first_title:?} and {second_title:?} resolve to the same Dear ImGui ID {id:?}"
    )]
    DuplicateWindowId {
        first_title: String,
        second_title: String,
        id: Id,
    },
    #[error("dock layout contains too many nodes")]
    LayoutTooLarge,
    #[error("docking is not enabled in ConfigFlags")]
    DockingDisabled,
    #[error("dockspace {root_id:?} was already submitted during this frame")]
    DuplicateDockspaceSubmission { root_id: Id },
    #[error("existing dock node {id:?} is not an explicit dockspace root")]
    ExistingNodeIsNotDockspaceRoot { id: Id },
    #[error("dockspace {root_id:?} must be submitted before any window hosted by its dock tree")]
    WindowSubmittedBeforeDockspace { root_id: Id },
    #[error("Dear ImGui could not create dock node {id:?}")]
    NodeCreationFailed { id: Id },
    #[error("Dear ImGui failed to split a docking node {direction:?} at ratio {ratio}")]
    SplitFailed { direction: DockSplit, ratio: f32 },
}
