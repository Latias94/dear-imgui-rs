//! Progress bars
//!
//! Simple progress indicators with size and overlay text customization.
//!
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

/// # Progress Bar Widgets
impl Ui {
    /// Creates a progress bar widget.
    ///
    /// The fraction should be between 0.0 (0%) and 1.0 (100%).
    #[doc(alias = "ProgressBar")]
    pub fn progress_bar(&self, fraction: f32) -> ProgressBar<'_> {
        ProgressBar::new(self, fraction)
    }

    /// Creates a progress bar with overlay text.
    #[doc(alias = "ProgressBar")]
    pub fn progress_bar_with_overlay<'ui>(
        &'ui self,
        fraction: f32,
        overlay: impl Into<Cow<'ui, str>>,
    ) -> ProgressBar<'ui> {
        ProgressBar::new(self, fraction).overlay_text(overlay)
    }
}

/// Builder for a progress bar widget.
///
/// # Examples
///
/// ```no_run
/// # use dear_imgui_rs::*;
/// # let mut ctx = Context::create();
/// # let ui = ctx.frame();
/// ui.progress_bar(0.6)
///     .size([100.0, 12.0])
///     .overlay_text("Progress!")
///     .build();
/// ```
#[derive(Clone, Debug)]
#[must_use]
pub struct ProgressBar<'ui> {
    fraction: f32,
    size: [f32; 2],
    overlay_text: Option<Cow<'ui, str>>,
    ui: &'ui Ui,
}

impl<'ui> ProgressBar<'ui> {
    /// Creates a progress bar with a given fraction showing
    /// the progress (0.0 = 0%, 1.0 = 100%).
    ///
    /// The progress bar will be automatically sized to fill the entire width of the window if no
    /// custom size is specified.
    #[inline]
    #[doc(alias = "ProgressBar")]
    pub fn new(ui: &'ui Ui, fraction: f32) -> Self {
        ProgressBar {
            fraction,
            size: [-1.0, 0.0], // -1.0 means auto-size to fill width
            overlay_text: None,
            ui,
        }
    }

    /// Sets an optional text that will be drawn over the progress bar.
    pub fn overlay_text(mut self, overlay_text: impl Into<Cow<'ui, str>>) -> Self {
        self.overlay_text = Some(overlay_text.into());
        self
    }

    /// Sets the size of the progress bar.
    ///
    /// Negative values will automatically align to the end of the axis, zero will let the progress
    /// bar choose a size, and positive values will use the given size.
    #[inline]
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = size.into();
        self
    }

    /// Sets the progress fraction (0.0 to 1.0)
    pub fn fraction(mut self, fraction: f32) -> Self {
        self.fraction = fraction;
        self
    }

    /// Builds the progress bar
    pub fn build(self) {
        let size_vec: sys::ImVec2 = self.size.into();
        let overlay_ptr = self
            .overlay_text
            .as_deref()
            .map_or(std::ptr::null(), |txt| self.ui.scratch_txt(txt));

        unsafe {
            sys::igProgressBar(self.fraction, size_vec, overlay_ptr);
        }
    }
}
