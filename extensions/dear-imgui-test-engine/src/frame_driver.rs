use std::error::Error;
use std::fmt;

use dear_imgui_rs::FrameToken;

use crate::TestEngineError;

/// Presentation mode selected for one bounded Test Engine run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunMode {
    /// An external driver owned the surface route.
    ///
    /// Individual frames may still be skipped. This records the selected integration path, not
    /// independent proof that an operating-system swapchain displayed pixels.
    Graphical,
    /// An external driver owned the surface route and provided synchronous framebuffer capture.
    ///
    /// Skipped frames do not invoke the capture provider.
    GraphicalWithCapture,
    /// Frames advanced through a deliberate no-surface presentation boundary.
    Headless,
}

/// One RGBA8 framebuffer pixel requested by the native capture tool.
#[cfg(feature = "capture")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Borrowed framebuffer region that a capturing driver must fill completely.
#[cfg(feature = "capture")]
pub struct CaptureRequest<'pixels> {
    viewport_id: u32,
    origin: [i32; 2],
    size: [u32; 2],
    pixels: &'pixels mut [Rgba8],
}

#[cfg(feature = "capture")]
impl CaptureRequest<'_> {
    pub(crate) fn new(
        viewport_id: u32,
        origin: [i32; 2],
        size: [u32; 2],
        pixels: &mut [Rgba8],
    ) -> CaptureRequest<'_> {
        CaptureRequest {
            viewport_id,
            origin,
            size,
            pixels,
        }
    }

    /// Opaque Dear ImGui viewport identifier. Zero remains a valid upstream identifier.
    #[must_use]
    pub const fn viewport_id(&self) -> u32 {
        self.viewport_id
    }

    /// Top-left coordinate of the requested region, relative to the named viewport framebuffer.
    #[must_use]
    pub const fn origin(&self) -> [i32; 2] {
        self.origin
    }

    /// Width and height of the requested region.
    #[must_use]
    pub const fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Mutable RGBA8 output buffer with exactly `width * height` pixels.
    ///
    /// Pixels are tightly packed in row-major order from the top-left to the bottom-right. There is
    /// no row padding. Drivers must preserve the framebuffer's rendered color values and channel
    /// order; they must not apply color-space or alpha conversion, or return a bottom-up API
    /// readback without flipping its rows.
    pub fn pixels_mut(&mut self) -> &mut [Rgba8] {
        self.pixels
    }
}

/// Failure observed at the run-scoped framebuffer callback boundary.
#[derive(Debug)]
#[non_exhaustive]
pub enum CaptureProviderError<DriverError> {
    /// The driver rejected the framebuffer readback request.
    Driver(DriverError),
    /// The driver panicked; the panic was caught before crossing the C ABI.
    Panicked,
    /// Native code supplied an invalid region or pixel buffer.
    InvalidRequest { detail: &'static str },
}

impl<DriverError: fmt::Display> fmt::Display for CaptureProviderError<DriverError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(source) => write!(formatter, "framebuffer capture failed: {source}"),
            Self::Panicked => formatter.write_str("framebuffer capture provider panicked"),
            Self::InvalidRequest { detail } => {
                write!(
                    formatter,
                    "native framebuffer capture request was invalid: {detail}"
                )
            }
        }
    }
}

impl<DriverError> Error for CaptureProviderError<DriverError>
where
    DriverError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Driver(source) => Some(source),
            Self::Panicked | Self::InvalidRequest { .. } => None,
        }
    }
}

/// Phase of the frame-driver protocol that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameDriverPhase {
    /// The driver failed while closing or preparing the Dear ImGui frame.
    Prepare,
    /// The driver failed while rendering the prepared main target.
    Render,
    /// The native pre-swap hook failed before presentation began.
    PreSwap,
    /// The driver failed to present the rendered surface.
    Present,
    /// The native post-swap hook failed after successful presentation.
    PostSwap,
}

/// Result of rendering the main target before any presentation hook runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MainRenderOutcome {
    /// The main target is ready for the native pre-swap hook and presentation.
    ReadyToPresent,
    /// The main target was deliberately skipped for this frame.
    ///
    /// This is appropriate for recoverable surface conditions such as timeout, occlusion, loss,
    /// or an outdated swapchain. The driver remains responsible for any backend-specific recovery.
    Skipped,
}

/// Observable result of driving one complete Test Engine frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameDriveOutcome {
    /// The main target was rendered, bracketed by Test Engine swap hooks, and presented.
    Presented,
    /// Main-target rendering was skipped; no swap hook or presentation method ran.
    Skipped,
}

/// Backend integration used by [`crate::TestEngine`] and [`crate::TestRunner`].
///
/// A single mutable driver owns all three phases so auxiliary viewport work, a main surface, a
/// command queue, or a pending frame can move naturally without shared-mutability workarounds.
/// Implementations must not acquire the main presentation surface during
/// [`prepare`](Self::prepare). WSI backends that require non-overlapping acquisitions complete
/// auxiliary platform surfaces there. Backends such as OpenGL may instead draw the main viewport
/// and then switch through auxiliary contexts inside [`render_main`](Self::render_main), provided
/// every draw needed for the presentation is complete before that method returns. The runner calls
/// [`present`](Self::present) exactly once after `render_main` returns
/// [`MainRenderOutcome::ReadyToPresent`] and the native pre-swap hook succeeds.
pub trait TestFrameDriver {
    /// Linear frame state produced by [`prepare`](Self::prepare).
    ///
    /// Single-viewport drivers normally use [`dear_imgui_rs::render::ReconciledFrame`]. A
    /// multi-viewport backend may instead retain route-specific proof that auxiliary surfaces,
    /// deferred faults, and resource retirement were handled before main-target rendering.
    type PreparedFrame<'frame>;
    /// Error returned while closing and preparing the Dear ImGui frame.
    type PrepareError;
    /// Error returned while rendering the prepared main target.
    type RenderError;
    /// Error returned while presenting the surface.
    type PresentError;

    /// Closes and prepares one Dear ImGui frame without retaining its frame token.
    ///
    /// The driver chooses the renderer capability appropriate for its backend. Managed-texture
    /// renderers consume the token with [`FrameToken::render`] and reconcile every texture request;
    /// legacy renderers consume it with [`FrameToken::render_legacy`]. The returned associated type
    /// keeps both paths linear and prevents route-specific proof from being erased back into a
    /// plain reconciled frame. Multi-viewport drivers must also finish auxiliary viewport rendering
    /// and presentation in this phase when their WSI contract requires it to precede main-surface
    /// acquisition.
    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError>;

    /// Returns the Context identity carried by one prepared frame.
    ///
    /// The Test Engine validates this identity before allowing main-target rendering. Composite
    /// prepared transactions must report the identity of their embedded
    /// [`dear_imgui_rs::render::ReconciledFrame`].
    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId;

    /// Renders the main target without presenting it.
    ///
    /// The prepared frame is consumed by value, so draw data cannot escape this call. This method
    /// must complete the main draw and any renderer-specific auxiliary work that still has to
    /// precede main presentation. Recoverable surface conditions return
    /// [`MainRenderOutcome::Skipped`] rather than pretending that a presentation occurred or
    /// converting an unavailable surface into an infrastructure failure.
    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError>;

    /// Presents the surface rendered for `frame_index`.
    fn present(&mut self, frame_index: u64) -> Result<(), Self::PresentError>;
}

/// Frame driver that can synchronously read the framebuffer for Test Engine capture requests.
///
/// The runner installs this capability only for [`crate::TestRunner::run_graphical_with_capture`]
/// and removes it before the borrow of the driver ends. Native capture requests are made from the
/// `post-swap` call, after [`TestFrameDriver::present`] has returned, and refer to that same
/// presented frame. A backend whose presentation consumes the render target must copy or stage
/// readable pixels before presentation and retain them through `post-swap`; reading an unrelated
/// later swapchain image is not valid. The callback is synchronous, so every requested pixel must
/// be written before it returns. A skipped main render never invokes this callback.
#[cfg(feature = "capture")]
pub trait CapturingTestFrameDriver: TestFrameDriver {
    /// Error returned by framebuffer readback.
    type CaptureError;

    /// Fills one framebuffer capture request for the just-presented frame before returning success.
    fn capture_framebuffer(
        &mut self,
        request: CaptureRequest<'_>,
    ) -> Result<(), Self::CaptureError>;
}

/// Failure produced while driving one Test Engine frame.
#[derive(Debug)]
#[non_exhaustive]
pub enum FrameDriverError<PrepareError, RenderError, PresentError> {
    /// The frame belongs to a Context other than the attached Context.
    Context(TestEngineError),
    /// The driver failed while closing or preparing the frame. No swap hook ran.
    Prepare(PrepareError),
    /// The driver failed while rendering the prepared main target. No swap hook ran.
    Render(RenderError),
    /// The native pre-swap hook failed. Presentation was not attempted.
    PreSwap(TestEngineError),
    /// Surface presentation failed. The post-swap hook was not called.
    Present {
        source: PresentError,
        abort_error: Option<TestEngineError>,
    },
    /// The native post-swap hook failed after successful presentation.
    PostSwap {
        source: TestEngineError,
        abort_error: Option<TestEngineError>,
    },
}

impl<PrepareError, RenderError, PresentError>
    FrameDriverError<PrepareError, RenderError, PresentError>
{
    /// Returns the protocol phase that failed, if frame processing had begun.
    #[must_use]
    pub const fn phase(&self) -> Option<FrameDriverPhase> {
        match self {
            Self::Context(_) => None,
            Self::Prepare(_) => Some(FrameDriverPhase::Prepare),
            Self::Render(_) => Some(FrameDriverPhase::Render),
            Self::PreSwap(_) => Some(FrameDriverPhase::PreSwap),
            Self::Present { .. } => Some(FrameDriverPhase::Present),
            Self::PostSwap { .. } => Some(FrameDriverPhase::PostSwap),
        }
    }

    /// Returns a secondary failure from aborting an incomplete presentation cycle.
    #[must_use]
    pub const fn abort_error(&self) -> Option<&TestEngineError> {
        match self {
            Self::Present { abort_error, .. } | Self::PostSwap { abort_error, .. } => {
                abort_error.as_ref()
            }
            _ => None,
        }
    }
}

impl<PrepareError, RenderError, PresentError> fmt::Display
    for FrameDriverError<PrepareError, RenderError, PresentError>
where
    PrepareError: fmt::Display,
    RenderError: fmt::Display,
    PresentError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context(source) => write!(formatter, "frame Context was rejected: {source}"),
            Self::Prepare(source) => write!(formatter, "frame preparation failed: {source}"),
            Self::Render(source) => write!(formatter, "main-target rendering failed: {source}"),
            Self::PreSwap(source) => write!(formatter, "pre-swap hook failed: {source}"),
            Self::Present {
                source,
                abort_error: None,
            } => write!(formatter, "surface presentation failed: {source}"),
            Self::Present {
                source,
                abort_error: Some(abort_error),
            } => write!(
                formatter,
                "surface presentation failed: {source}; aborting the presentation cycle also failed: {abort_error}"
            ),
            Self::PostSwap {
                source,
                abort_error: None,
            } => write!(formatter, "post-swap hook failed: {source}"),
            Self::PostSwap {
                source,
                abort_error: Some(abort_error),
            } => write!(
                formatter,
                "post-swap hook failed: {source}; aborting the presentation cycle also failed: {abort_error}"
            ),
        }
    }
}

impl<PrepareError, RenderError, PresentError> Error
    for FrameDriverError<PrepareError, RenderError, PresentError>
where
    PrepareError: Error + 'static,
    RenderError: Error + 'static,
    PresentError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Context(source) | Self::PreSwap(source) => Some(source),
            Self::Prepare(source) => Some(source),
            Self::Render(source) => Some(source),
            Self::Present { source, .. } => Some(source),
            Self::PostSwap { source, .. } => Some(source),
        }
    }
}
