use std::error::Error;
use std::ffi::CStr;
use std::fmt;

use dear_imgui_rs::{
    ContextAttachmentDetachError, ContextAttachmentError, ContextBindingError, ContextId,
};
use dear_imgui_test_engine_sys as sys;

use crate::{AttachmentState, RunState};

/// Typed status returned by the Test Engine C ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TestEngineStatus {
    InvalidArgument,
    InvalidState,
    NotFound,
    OutOfRange,
    Exception,
    Unsupported,
    CaptureFailed,
    BindingOccupied,
    Unknown(i32),
}

impl TestEngineStatus {
    fn from_raw(status: sys::ImGuiTestEngineStatus) -> Self {
        match status {
            sys::ImGuiTestEngineStatus_InvalidArgument => Self::InvalidArgument,
            sys::ImGuiTestEngineStatus_InvalidState => Self::InvalidState,
            sys::ImGuiTestEngineStatus_NotFound => Self::NotFound,
            sys::ImGuiTestEngineStatus_OutOfRange => Self::OutOfRange,
            sys::ImGuiTestEngineStatus_Exception => Self::Exception,
            sys::ImGuiTestEngineStatus_Unsupported => Self::Unsupported,
            sys::ImGuiTestEngineStatus_CaptureFailed => Self::CaptureFailed,
            sys::ImGuiTestEngineStatus_BindingOccupied => Self::BindingOccupied,
            other => Self::Unknown(other),
        }
    }
}

/// Error returned by the safe Test Engine API.
#[derive(Debug)]
#[non_exhaustive]
pub enum TestEngineError {
    /// The native ABI rejected an operation. `diagnostic` is copied before another ABI call.
    Ffi {
        operation: &'static str,
        status: TestEngineStatus,
        diagnostic: String,
    },
    /// The operation is not valid for the current attachment/run state.
    InvalidState {
        operation: &'static str,
        attachment: AttachmentState,
        run: RunState,
        detail: &'static str,
    },
    /// A safe input failed validation before reaching FFI.
    InvalidInput {
        operation: &'static str,
        argument: &'static str,
        detail: &'static str,
    },
    /// Native output violated the safe wrapper's data contract.
    InvalidNativeData {
        operation: &'static str,
        detail: &'static str,
    },
    /// A built-in suite did not match the manifest pinned to this upstream revision.
    UnexpectedTestSuiteManifest {
        category: &'static str,
        expected: &'static [&'static str],
        actual: Vec<String>,
    },
    /// A controlled suite gate did not leave every registered test successful.
    UnexpectedTestSuiteResult {
        category: &'static str,
        expected: usize,
        tested: usize,
        succeeded: usize,
        in_queue: usize,
        exact_manifest: bool,
        non_successful: Vec<String>,
    },
    /// Registration failed and native rollback also failed.
    TestSuiteRollback {
        source: Box<TestEngineError>,
        rollback: Box<TestEngineError>,
    },
    /// A `Ui` belongs to a Context other than the attached Context.
    ContextMismatch {
        operation: &'static str,
        expected: ContextId,
        actual: ContextId,
    },
    /// A UI-only operation was attempted outside a native frame.
    FrameNotActive { operation: &'static str },
    /// The attached Context no longer accepts ordinary bindings.
    ContextBinding {
        operation: &'static str,
        source: ContextBindingError,
    },
    /// The Context refused the unique Test Engine attachment.
    Attachment {
        operation: &'static str,
        source: ContextAttachmentError,
    },
    /// The Context rejected an explicit Test Engine attachment detach.
    AttachmentDetach {
        operation: &'static str,
        source: ContextAttachmentDetachError,
    },
    /// A core attachment lease contradicted the synchronized Test Engine state.
    AttachmentInvariant {
        operation: &'static str,
        detail: &'static str,
    },
}

impl TestEngineError {
    pub(crate) fn invalid_input(
        operation: &'static str,
        argument: &'static str,
        detail: &'static str,
    ) -> Self {
        Self::InvalidInput {
            operation,
            argument,
            detail,
        }
    }

    pub(crate) fn invalid_state(
        operation: &'static str,
        attachment: AttachmentState,
        run: RunState,
        detail: &'static str,
    ) -> Self {
        Self::InvalidState {
            operation,
            attachment,
            run,
            detail,
        }
    }

    /// Returns the copied native diagnostic when this error originated at the C ABI.
    pub fn diagnostic(&self) -> Option<&str> {
        match self {
            Self::Ffi { diagnostic, .. } => Some(diagnostic),
            _ => None,
        }
    }

    /// Returns the native status when this error originated at the C ABI.
    pub fn status(&self) -> Option<TestEngineStatus> {
        match self {
            Self::Ffi { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl fmt::Display for TestEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ffi {
                operation,
                status,
                diagnostic,
            } => write!(f, "{operation} failed with {status:?}: {diagnostic}"),
            Self::InvalidState {
                operation,
                attachment,
                run,
                detail,
            } => write!(
                f,
                "{operation} is invalid in attachment state {attachment:?} and run state {run:?}: {detail}"
            ),
            Self::InvalidInput {
                operation,
                argument,
                detail,
            } => write!(f, "{operation} rejected {argument}: {detail}"),
            Self::InvalidNativeData { operation, detail } => {
                write!(f, "{operation} received invalid native data: {detail}")
            }
            Self::UnexpectedTestSuiteManifest {
                category,
                expected,
                actual,
            } => write!(
                f,
                "built-in test category {category:?} did not match its pinned manifest: expected {expected:?}, found {actual:?}"
            ),
            Self::UnexpectedTestSuiteResult {
                category,
                expected,
                tested,
                succeeded,
                in_queue,
                exact_manifest,
                non_successful,
            } => write!(
                f,
                "built-in test category {category:?} expected {expected} exact successful terminal tests, found exact_manifest={exact_manifest}, tested={tested}, success={succeeded}, in_queue={in_queue}, non_successful={non_successful:?}"
            ),
            Self::TestSuiteRollback { source, rollback } => write!(
                f,
                "built-in test suite registration failed ({source}) and rollback also failed ({rollback})"
            ),
            Self::ContextMismatch {
                operation,
                expected,
                actual,
            } => write!(
                f,
                "{operation} received Ui from Context {actual:?}, expected {expected:?}"
            ),
            Self::FrameNotActive { operation } => {
                write!(f, "{operation} requires an active Dear ImGui frame")
            }
            Self::ContextBinding { operation, source } => {
                write!(
                    f,
                    "{operation} could not bind the attached Context: {source}"
                )
            }
            Self::Attachment { operation, source } => {
                write!(
                    f,
                    "{operation} could not reserve the Context attachment: {source}"
                )
            }
            Self::AttachmentDetach { operation, source } => {
                write!(f, "{operation} could not detach from the Context: {source}")
            }
            Self::AttachmentInvariant { operation, detail } => {
                write!(
                    f,
                    "{operation} violated the Context attachment invariant: {detail}"
                )
            }
        }
    }
}

impl Error for TestEngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ContextBinding { source, .. } => Some(source),
            Self::Attachment { source, .. } => Some(source),
            Self::AttachmentDetach { source, .. } => Some(source),
            Self::TestSuiteRollback { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Result type used by every fallible safe Test Engine operation.
pub type TestEngineResult<T> = Result<T, TestEngineError>;

pub(crate) fn ffi_status(
    operation: &'static str,
    status: sys::ImGuiTestEngineStatus,
) -> TestEngineResult<()> {
    if status == sys::ImGuiTestEngineStatus_Success {
        return Ok(());
    }
    Err(TestEngineError::Ffi {
        operation,
        status: TestEngineStatus::from_raw(status),
        diagnostic: copied_last_error(),
    })
}

fn copied_last_error() -> String {
    let mut required = 0usize;
    let query_status =
        unsafe { sys::imgui_test_engine_get_last_error(std::ptr::null_mut(), 0, &mut required) };
    if query_status != sys::ImGuiTestEngineStatus_Success || required == 0 {
        return "native diagnostic unavailable".to_owned();
    }

    let mut buffer = vec![0u8; required];
    let copy_status = unsafe {
        sys::imgui_test_engine_get_last_error(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut required,
        )
    };
    if copy_status != sys::ImGuiTestEngineStatus_Success {
        return "native diagnostic unavailable".to_owned();
    }

    unsafe { CStr::from_ptr(buffer.as_ptr().cast()) }
        .to_string_lossy()
        .into_owned()
}
