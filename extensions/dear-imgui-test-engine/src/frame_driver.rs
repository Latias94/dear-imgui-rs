use std::error::Error;
use std::fmt;

use dear_imgui_rs::render::{ReconciledFrame, RenderedFrame};

use crate::TestEngineError;

/// Presentation mode selected for one bounded Test Engine run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunMode {
    /// An external driver was responsible for presenting a surface.
    ///
    /// This records the selected integration path, not independent proof that an operating-system
    /// swapchain displayed pixels.
    Graphical,
    /// An external driver presented a surface and provided synchronous framebuffer capture.
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
    /// The driver failed while consuming rendered draw data.
    Render,
    /// The native pre-swap hook failed before presentation began.
    PreSwap,
    /// The driver failed to present the rendered surface.
    Present,
    /// The native post-swap hook failed after successful presentation.
    PostSwap,
}

/// Backend integration used by [`crate::TestEngine`] and [`crate::TestRunner`].
///
/// A single mutable driver owns both phases so a surface, command queue, or pending frame can move
/// naturally from rendering to presentation without shared mutability workarounds. Implementations
/// must not present the primary surface from [`render`](Self::render); the runner calls
/// [`present`](Self::present) exactly once after the native pre-swap hook succeeds. A
/// multi-viewport renderer may complete auxiliary platform surfaces during `render` so their
/// acquisition and presentation lifetimes do not overlap the primary swap interval.
pub trait TestFrameDriver {
    /// Error returned while consuming the rendered frame.
    type RenderError;
    /// Error returned while presenting the surface.
    type PresentError;

    /// Reconciles and renders one frame without retaining the render lease.
    ///
    /// A successful implementation returns the proof produced by
    /// [`RenderedFrame::into_reconciled`]. This keeps the one-use lease linear while preventing a
    /// false successful return before managed-texture feedback is reconciled.
    fn render<'frame>(
        &mut self,
        frame: RenderedFrame<'frame>,
        frame_index: u64,
    ) -> Result<ReconciledFrame<'frame>, Self::RenderError>;

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
/// be written before it returns.
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

/// Failure produced while driving one rendered Test Engine frame.
#[derive(Debug)]
#[non_exhaustive]
pub enum FrameDriverError<RenderError, PresentError> {
    /// The frame belongs to a Context other than the attached Context.
    Context(TestEngineError),
    /// The driver failed while consuming draw data. No swap hook ran.
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

impl<RenderError, PresentError> FrameDriverError<RenderError, PresentError> {
    /// Returns the protocol phase that failed, if frame processing had begun.
    #[must_use]
    pub const fn phase(&self) -> Option<FrameDriverPhase> {
        match self {
            Self::Context(_) => None,
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

impl<RenderError, PresentError> fmt::Display for FrameDriverError<RenderError, PresentError>
where
    RenderError: fmt::Display,
    PresentError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context(source) => write!(formatter, "frame Context was rejected: {source}"),
            Self::Render(source) => write!(formatter, "frame rendering failed: {source}"),
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

impl<RenderError, PresentError> Error for FrameDriverError<RenderError, PresentError>
where
    RenderError: Error + 'static,
    PresentError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Context(source) | Self::PreSwap(source) => Some(source),
            Self::Render(source) => Some(source),
            Self::Present { source, .. } => Some(source),
            Self::PostSwap { source, .. } => Some(source),
        }
    }
}
