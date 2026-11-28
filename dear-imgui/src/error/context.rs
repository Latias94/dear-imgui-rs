//! Errors from Context ownership, platform integration and configuration.

use std::error::Error as StdError;
use std::ffi::NulError;
use std::num::{NonZeroU16, TryFromIntError};

use thiserror::Error;

use crate::Id;
use crate::context::{ContextAttachmentPhase, ContextAttachmentRole};
use crate::dock_layout::DockSplit;
use crate::error::FrameError;
use crate::ini_settings::IniSessionDate;
use crate::numeric_format::MAX_DIRECTIVE_BYTES;

/// A failure to create, configure, bind or shut down a Dear ImGui Context or its platform state.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContextError {
    /// Dear ImGui could not construct a Context.
    #[error("Failed to create Dear ImGui context: {reason}")]
    Creation { reason: String },
    /// Another OS thread holds a lease for Dear ImGui's process-global state.
    #[error(
        "another OS thread owns Dear ImGui native state. Drop all contexts, native owners, and remaining thread leases there before acquiring ownership here"
    )]
    ThreadConflict,
    /// The process-wide native owner lease count was exhausted.
    #[error("Resource allocation failed: Dear ImGui native owner thread lease count exhausted")]
    ThreadLeaseExhausted,
    /// A managed renderer requires exclusive use of the shared atlas.
    #[error(
        "Cannot register another Dear ImGui context: the shared font atlas is exclusively owned by a managed renderer"
    )]
    SharedFontAtlasManaged,
    /// A previous renderer still owns native bindings in the shared atlas.
    #[error(
        "Cannot register another Dear ImGui context: the shared font atlas still belongs to a renderer whose texture release was not committed; release the renderer resources, then drop and recreate the atlas"
    )]
    SharedFontAtlasRendererReleasePending,
    /// A persistent binding cannot enter its Context.
    #[error("{0}")]
    Binding(#[from] BindingKind),
    /// A scoped operation could not enter or leave the Context.
    #[error("{0}")]
    Scope(#[from] ScopeKind),

    /// Context-owned teardown has begun.
    #[error("Dear ImGui context teardown is in progress")]
    ContextDropping,
    /// An attachment with the same marker type is already registered.
    #[error("an attachment with this marker type is already registered")]
    DuplicateAttachment,
    /// The exclusive platform or renderer role is already occupied.
    #[error("the {0:?} attachment role is already occupied")]
    AttachmentRoleOccupied(ContextAttachmentRole),
    /// The renderer needs an active platform attachment.
    #[error("a renderer attachment requires an active platform attachment")]
    MissingPlatformAttachment,
    /// A release transaction has reserved this attachment generation.
    #[error("platform attachment release is already in progress")]
    PlatformReleaseInProgress,
    /// Renderer resources still depend on platform-owned native handles.
    #[error("the platform attachment cannot be released while a renderer attachment is active")]
    RendererAttachmentActive,
    /// The supplied attachment generation is no longer active.
    #[error("the platform attachment generation is no longer active")]
    PlatformAttachmentInactive,
    /// The supplied attachment does not own the platform role.
    #[error("the supplied attachment does not own the platform role")]
    NotPlatformAttachment,
    /// The attachment is not this Context's active platform generation.
    #[error("the supplied attachment is not the active platform generation for this Context")]
    PlatformGenerationMismatch,
    /// An attachment hook reported a failure during teardown.
    #[error("{message}")]
    AttachmentTeardown {
        /// Diagnostic from the attachment.
        message: String,
        /// Original error reported by the backend, when available.
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
    /// A platform-window teardown transaction attempted to enter itself.
    #[error("platform-window teardown cannot be reentered")]
    ReentrantPlatformWindowTeardown,
    /// The platform attachment rejected teardown before any native window was destroyed.
    #[error("platform attachment rejected platform-window teardown: {0}")]
    PlatformWindowTeardownPreflight(#[source] Box<dyn StdError + Send + Sync>),
    /// The platform attachment could not finish cleanup after native teardown completed.
    #[error("platform attachment could not complete platform-window teardown: {0}")]
    PlatformWindowTeardownPostflight(#[source] Box<dyn StdError + Send + Sync>),
    /// The attachment panicked before native platform-window teardown began.
    #[error("platform attachment panicked before platform-window teardown")]
    PlatformWindowTeardownBeginPanicked,
    /// The attachment panicked after native platform-window teardown completed.
    #[error("platform attachment panicked after platform-window teardown")]
    PlatformWindowTeardownEndPanicked,
    /// The attachment attempted to reset renderer bindings in a different teardown phase.
    #[error("renderer texture reset requires the RendererResources phase, not {phase:?}")]
    RendererTextureResetWrongPhase { phase: ContextAttachmentPhase },
    /// Renderer bindings can be reset through the teardown capability only during teardown.
    #[error("renderer texture reset requires active Context teardown")]
    RendererTextureResetOutsideTeardown,
    /// The renderer texture reset transaction attempted to enter itself.
    #[error("renderer texture reset cannot be reentered")]
    ReentrantRendererTextureReset,
    /// The renderer could not reset its texture bindings safely during teardown.
    #[error("renderer texture reset preflight failed: {0}")]
    RendererTextureResetPreflight(#[source] FrameError),
    /// Native window registration identities have been exhausted.
    #[error("native window registration identity space is exhausted")]
    NativeWindowIdentityExhausted,

    /// IME cursor offsets must be UTF-8 boundaries within the preedit text.
    #[error("IME cursor offsets must be UTF-8 boundaries within the preedit text")]
    InvalidImeCursorRange,
    /// IME preedit text contained an interior NUL byte.
    #[error("IME preedit text must not contain NUL")]
    ImePreeditContainsNul,
    /// The optional system clipboard cannot run on this target.
    #[error("system clipboard integration is unavailable on this target")]
    ClipboardUnsupportedPlatform,
    /// The system clipboard could not be initialized.
    #[error("failed to initialize the system clipboard: {0}")]
    ClipboardInitialization(#[source] Box<dyn StdError + Send + Sync>),
    /// Text could not be read from the system clipboard.
    #[error("failed to read the system clipboard: {0}")]
    ClipboardRead(#[source] Box<dyn StdError + Send + Sync>),
    /// Text could not be written to the system clipboard.
    #[error("failed to write the system clipboard: {0}")]
    ClipboardWrite(#[source] Box<dyn StdError + Send + Sync>),

    /// A Context setting could not be passed as a C string.
    #[error("configuration string contains an interior NUL byte: {0}")]
    InvalidCString(#[from] NulError),
    /// The date is outside the supported Dear ImGui packed calendar.
    #[error("{0}")]
    IniSessionDate(#[from] IniDateKind),
    /// The requested period would underflow Dear ImGui's packed date year.
    #[error(
        "automatic discard period of {months} months exceeds the {max_months}-month limit for session date {session_date}"
    )]
    RetentionUnderflowsSessionDate {
        /// Date used as the retention upper bound.
        session_date: IniSessionDate,
        /// Requested automatic-discard period.
        months: NonZeroU16,
        /// Largest supported period for `session_date`.
        max_months: u16,
    },
    /// The session date is immutable after Dear ImGui has started its first frame.
    #[error("`.ini` retention must be configured before the first Dear ImGui frame")]
    LockedAfterFirstFrame,
    /// Automatic cleanup cannot be changed after settings have already been loaded.
    #[error("`.ini` retention must be configured before loading Dear ImGui settings")]
    LockedAfterSettingsLoad,
    /// Native platform state contains a date outside the safe packed range.
    #[error("native Platform_SessionDate {raw} is not a valid Dear ImGui packed date")]
    InvalidNativeSessionDate {
        /// Raw packed date reported by Dear ImGui.
        raw: i32,
        /// Calendar validation failure for a positive packed date.
        #[source]
        source: Option<IniDateKind>,
    },
    /// Native IO state contains an unsupported automatic-discard period.
    #[error("native ConfigIniSettingsAutoDiscardMonths {raw} is invalid")]
    InvalidNativeAutoDiscardMonths {
        /// Raw month count reported by Dear ImGui.
        raw: i32,
        /// Failed integer conversion for an out-of-range positive count.
        #[source]
        source: Option<TryFromIntError>,
    },
    /// Native settings fields encode a combination that cannot be represented safely.
    #[error("native `.ini` retention state is invalid: {reason}")]
    InvalidNativeState {
        /// Explanation of the inconsistent native fields.
        reason: &'static str,
    },

    #[error("a dockspace in the current window requires an explicit non-zero root ID")]
    MissingRootId,
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
    /// A docking window class contains an invalid override.
    #[error("invalid window class: {0}")]
    InvalidWindowClass(#[from] WindowClassKind),
    #[error("dock split ratio must be finite and strictly between 0 and 1: {ratio}")]
    InvalidSplitRatio { ratio: f32 },
    #[error(
        "dock window keys {first_key:?} and {second_key:?} resolve to the same Dear ImGui ID {id:?}"
    )]
    DuplicateWindowKey {
        first_key: String,
        second_key: String,
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

    /// The stable window ID was empty.
    #[error("a stable window ID cannot be empty")]
    EmptyWindowId,
    /// The stable window ID contained a NUL byte.
    #[error("a stable window ID cannot contain an interior NUL byte")]
    WindowIdContainsNul,
    /// The stable window ID contained an identity separator.
    #[error("a stable window ID cannot contain Dear ImGui's `###` identity separator")]
    WindowIdContainsSeparator,
    /// The stable window ID hashed to a reserved native ID.
    #[error("the stable window ID hashes to Dear ImGui's reserved zero ID")]
    WindowIdIsZero,
    /// A C-style numeric format failed validation.
    #[error("{0}")]
    NumericFormat(#[from] NumericFormatKind),
}

impl ContextError {
    /// Report a teardown hook failure from a backend diagnostic.
    pub fn attachment_teardown(message: impl Into<String>) -> Self {
        Self::AttachmentTeardown {
            message: message.into(),
            source: None,
        }
    }
}

/// Failure to enter a Context through a persistent binding capability.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum BindingKind {
    /// Context teardown has started, so ordinary safe access is no longer permitted.
    #[error("Dear ImGui context teardown is in progress")]
    Dropping,
    /// The originating native Context no longer exists.
    #[error("Dear ImGui context has been destroyed")]
    NativeDestroyed,
}

/// Failure at the boundary of a scoped Context operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[non_exhaustive]
pub enum ScopeKind {
    /// A scope cannot take over a frame that was already open.
    #[error("the Context already has an open frame")]
    FrameAlreadyOpen,
    /// The operation returned success with a frame still open.
    #[error("the Context operation returned success with an open frame")]
    FrameLeftOpen,
    /// The native Context could not be bound.
    #[error("the Context is unavailable: {0}")]
    ContextUnavailable(#[source] BindingKind),
}

/// Either the scoped Context operation or its caller's closure failed.
///
/// A closure can return any error payload. When `E` implements `std::error::Error`,
/// this type exposes the scope failure or the closure's error through `source()`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Operation<E> {
    /// The scope could not be entered or finished.
    #[error("{0}")]
    Scope(#[from] ScopeKind),
    /// The caller's closure returned an error.
    #[error("the Context operation failed: {0}")]
    Closure(#[source] E),
}

impl<E> Operation<E> {
    /// Maps the caller's error, preserving Context-scope errors.
    pub fn map_closure<F>(self, f: impl FnOnce(E) -> F) -> Operation<F> {
        match self {
            Self::Scope(error) => Operation::Scope(error),
            Self::Closure(error) => Operation::Closure(f(error)),
        }
    }

    /// Returns the Context-scope failure, if any.
    pub fn scope_error(&self) -> Option<ScopeKind> {
        match self {
            Self::Scope(error) => Some(*error),
            Self::Closure(_) => None,
        }
    }

    /// Borrows the caller's error, if any.
    pub fn closure_error(&self) -> Option<&E> {
        match self {
            Self::Scope(_) => None,
            Self::Closure(error) => Some(error),
        }
    }

    /// Extracts the caller's error or returns the Context-scope failure.
    pub fn into_closure_error(self) -> Result<E, ScopeKind> {
        match self {
            Self::Scope(error) => Err(error),
            Self::Closure(error) => Ok(error),
        }
    }
}

/// Validation failure for a [`crate::WindowClass`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowClassKind {
    #[error("parent viewport ID must be non-zero")]
    ZeroParentViewportId,
    #[error("viewport overrides contain unsupported ImGuiViewportFlags bits: 0x{bits:X}")]
    UnsupportedViewportFlags { bits: i32 },
    #[error("viewport overrides set and clear the same ImGuiViewportFlags bits: 0x{bits:X}")]
    OverlappingViewportFlags { bits: i32 },
    #[error("tab overrides contain unsupported ImGuiTabItemFlags bits: 0x{bits:X}")]
    UnsupportedTabItemFlags { bits: i32 },
    #[error("dock-node overrides contain unsupported ImGuiDockNodeFlags bits: 0x{bits:X}")]
    UnsupportedDockNodeFlags { bits: i32 },
}

/// Errors returned when constructing an [`IniSessionDate`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum IniDateKind {
    /// The year cannot round-trip through Dear ImGui's packed representation.
    #[error("year {year} is outside Dear ImGui's supported packed-date range 2001 through 2127")]
    YearOutOfRange { year: u32 },
    /// The month is not a Gregorian calendar month.
    #[error("month {month} is outside the Gregorian range 1 through 12")]
    MonthOutOfRange { month: u32 },
    /// The day is invalid for the supplied Gregorian year and month.
    #[error("day {day} is invalid for {year:04}-{month:02}")]
    DayOutOfRange { year: u32, month: u32, day: u32 },
}

/// Describes why a C-style numeric format was rejected.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum NumericFormatKind {
    /// The complete UTF-8 format exceeds the bounded native parsing contract.
    #[error("numeric format is {length} UTF-8 bytes; at most {maximum} bytes are supported")]
    FormatTooLong {
        byte_offset: usize,
        length: usize,
        maximum: usize,
    },
    /// The string contains a NUL byte and cannot be passed as one C string.
    #[error("numeric format contains a NUL byte at byte {byte_offset}")]
    InteriorNul { byte_offset: usize },
    /// A `%` directive is incomplete.
    #[error("numeric format has an incomplete directive at byte {byte_offset}")]
    UnterminatedDirective { byte_offset: usize },
    /// More than one directive would consume a value argument.
    #[error(
        "numeric format requests more than Dear ImGui's one value argument at byte {byte_offset}"
    )]
    MultipleConversions { byte_offset: usize },
    /// `*` requests another variadic width or precision argument.
    #[error("numeric format requests a dynamic variadic argument at byte {byte_offset}")]
    DynamicArgument { byte_offset: usize },
    /// A positional argument such as `%1$d` was requested.
    #[error("numeric format uses an unsupported positional argument at byte {byte_offset}")]
    PositionalArgument { byte_offset: usize },
    /// The directive uses a non-portable or unsupported flag.
    #[error("numeric format uses unsupported flag `{flag}` at byte {byte_offset}")]
    UnsupportedFlag { byte_offset: usize, flag: char },
    /// The flag is defined for printf but not for this numeric conversion.
    #[error(
        "numeric format flag `{flag}` is incompatible with `%{conversion}` at byte {byte_offset}"
    )]
    IncompatibleFlag {
        byte_offset: usize,
        flag: char,
        conversion: char,
    },
    /// The requested field width exceeds the bounded native parsing contract.
    #[error(
        "numeric format width {width} at byte {byte_offset} exceeds the supported maximum {maximum}"
    )]
    WidthTooLarge {
        byte_offset: usize,
        width: u32,
        maximum: u32,
    },
    /// The requested precision exceeds the bounded native parsing contract.
    #[error(
        "numeric format precision {precision} at byte {byte_offset} exceeds the supported maximum {maximum}"
    )]
    PrecisionTooLarge {
        byte_offset: usize,
        precision: u32,
        maximum: u32,
    },
    /// The directive uses a length modifier that does not match the C carrier type.
    #[error(
        "numeric format uses a length modifier that does not match its value type at byte {byte_offset}"
    )]
    UnsupportedLength { byte_offset: usize },
    /// The directive is not a supported numeric conversion.
    #[error("numeric format uses unsupported conversion `%{conversion}` at byte {byte_offset}")]
    UnsupportedConversion {
        byte_offset: usize,
        conversion: char,
    },
    /// The conversion is numeric but does not match the signedness or category of `T`.
    #[error("numeric format conversion does not match its value type at byte {byte_offset}")]
    TypeMismatch { byte_offset: usize },
    /// Dear ImGui copies directives into a fixed 32-byte stack buffer.
    #[error(
        "numeric format directive at byte {byte_offset} requires {length} bytes on a supported target; Dear ImGui supports at most {maximum}",
        maximum = MAX_DIRECTIVE_BYTES
    )]
    DirectiveTooLong { byte_offset: usize, length: usize },
}

impl NumericFormatKind {
    /// Returns the byte offset where validation failed.
    pub fn byte_offset(&self) -> usize {
        match *self {
            Self::FormatTooLong { byte_offset, .. }
            | Self::InteriorNul { byte_offset }
            | Self::UnterminatedDirective { byte_offset }
            | Self::MultipleConversions { byte_offset }
            | Self::DynamicArgument { byte_offset }
            | Self::PositionalArgument { byte_offset }
            | Self::UnsupportedFlag { byte_offset, .. }
            | Self::IncompatibleFlag { byte_offset, .. }
            | Self::WidthTooLarge { byte_offset, .. }
            | Self::PrecisionTooLarge { byte_offset, .. }
            | Self::UnsupportedLength { byte_offset }
            | Self::UnsupportedConversion { byte_offset, .. }
            | Self::TypeMismatch { byte_offset }
            | Self::DirectiveTooLong { byte_offset, .. } => byte_offset,
        }
    }
}
