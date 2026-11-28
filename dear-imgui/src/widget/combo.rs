//! Combo boxes
//!
//! Single-selection dropdowns with optional height and popup alignment flags.
//! Builders provide both string and custom item sources.
//!

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{Ui, sys};
use std::borrow::Cow;

/// # Combo Box Widgets
impl Ui {
    /// Creates a combo box and starts appending to it.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo(
        &self,
        label: impl AsRef<str>,
        preview_value: impl AsRef<str>,
    ) -> Option<ComboBoxToken<'_>> {
        self.begin_combo_with_flags(label, preview_value, ComboBoxFlags::NONE)
    }

    /// Creates a combo box with flags and starts appending to it.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_with_flags(
        &self,
        label: impl AsRef<str>,
        preview_value: impl AsRef<str>,
        flags: impl Into<ComboBoxOptions>,
    ) -> Option<ComboBoxToken<'_>> {
        let options = flags.into();
        options.validate("Ui::begin_combo_with_flags()");
        let should_render =
            self.with_scratch_txt_two(label, preview_value, |label_ptr, preview_ptr| unsafe {
                sys::igBeginCombo(label_ptr, preview_ptr, options.raw())
            });

        if should_render {
            Some(ComboBoxToken::new(self))
        } else {
            None
        }
    }

    /// Creates a combo box without preview value.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_no_preview(&self, label: impl AsRef<str>) -> Option<ComboBoxToken<'_>> {
        self.begin_combo_no_preview_with_flags(label, ComboBoxFlags::NONE)
    }

    /// Creates a combo box without preview value and with flags.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_no_preview_with_flags(
        &self,
        label: impl AsRef<str>,
        flags: impl Into<ComboBoxOptions>,
    ) -> Option<ComboBoxToken<'_>> {
        let mut options = flags.into();
        options.preview_mode = ComboBoxPreviewMode::NoPreview;
        options.validate("Ui::begin_combo_no_preview_with_flags()");
        let should_render = self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igBeginCombo(label_ptr, std::ptr::null(), options.raw())
        });

        if should_render {
            Some(ComboBoxToken::new(self))
        } else {
            None
        }
    }

    /// Builds a simple combo box for choosing from a slice of values.
    #[doc(alias = "Combo")]
    pub fn combo<V, L>(
        &self,
        label: impl AsRef<str>,
        current_item: &mut usize,
        items: &[V],
        label_fn: L,
    ) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let label_fn = &label_fn;
        let mut result = false;
        let preview_value = items.get(*current_item).map(label_fn);

        if let Some(combo_token) = self.begin_combo(
            label,
            preview_value.as_ref().map(|s| s.as_ref()).unwrap_or(""),
        ) {
            for (idx, item) in items.iter().enumerate() {
                let is_selected = idx == *current_item;
                let clicked = self
                    .selectable_config(label_fn(item).as_ref())
                    .selected(is_selected)
                    .build();
                if is_selected {
                    self.set_item_default_focus();
                }

                if clicked {
                    *current_item = idx;
                    result = true;
                }
            }
            combo_token.end();
        }

        result
    }

    /// Builds a simple combo box using an `i32` index (ImGui-style).
    ///
    /// This is useful when you want to represent \"no selection\" with `-1`, matching Dear ImGui's
    /// `Combo()` API.
    #[doc(alias = "Combo")]
    pub fn combo_i32<V, L>(
        &self,
        label: impl AsRef<str>,
        current_item: &mut i32,
        items: &[V],
        label_fn: L,
    ) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let label_fn = &label_fn;
        let mut result = false;

        let preview_value = if *current_item >= 0 {
            items.get(*current_item as usize).map(label_fn)
        } else {
            None
        };

        if let Some(combo_token) = self.begin_combo(
            label,
            preview_value.as_ref().map(|s| s.as_ref()).unwrap_or(""),
        ) {
            for (idx, item) in items.iter().enumerate() {
                if idx > i32::MAX as usize {
                    break;
                }
                let idx_i32 = idx as i32;
                let is_selected = idx_i32 == *current_item;
                let clicked = self
                    .selectable_config(label_fn(item).as_ref())
                    .selected(is_selected)
                    .build();
                if is_selected {
                    self.set_item_default_focus();
                }
                if clicked {
                    *current_item = idx_i32;
                    result = true;
                }
            }
            combo_token.end();
        }

        result
    }

    /// Builds a simple combo box for choosing from a slice of strings
    #[doc(alias = "Combo")]
    pub fn combo_simple_string(
        &self,
        label: impl AsRef<str>,
        current_item: &mut usize,
        items: &[impl AsRef<str>],
    ) -> bool {
        self.combo(label, current_item, items, |s| Cow::Borrowed(s.as_ref()))
    }

    /// Builds a simple combo box for choosing from a slice of strings using an `i32` index.
    #[doc(alias = "Combo")]
    pub fn combo_simple_string_i32(
        &self,
        label: impl AsRef<str>,
        current_item: &mut i32,
        items: &[impl AsRef<str>],
    ) -> bool {
        self.combo_i32(label, current_item, items, |s| Cow::Borrowed(s.as_ref()))
    }

    /// Makes the last submitted item the default focus of a newly appearing window.
    #[doc(alias = "SetItemDefaultFocus")]
    pub fn set_item_default_focus(&self) {
        self.run_with_bound_context(|| unsafe { sys::igSetItemDefaultFocus() });
    }
}

bitflags::bitflags! {
    /// Independent flags for combo box widgets.
    ///
    /// Mutually exclusive preview and height choices are represented by
    /// [`ComboBoxPreviewMode`] and [`ComboBoxHeight`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ComboBoxFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Align the popup toward the left by default
        const POPUP_ALIGN_LEFT = sys::ImGuiComboFlags_PopupAlignLeft;
    }
}

/// Height policy for combo box popups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboBoxHeight {
    /// Max roughly 4 items visible.
    Small,
    /// Max roughly 8 items visible.
    Regular,
    /// Max roughly 20 items visible.
    Large,
    /// As many fitting items as possible.
    Largest,
}

impl ComboBoxHeight {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Small => sys::ImGuiComboFlags_HeightSmall,
            Self::Regular => sys::ImGuiComboFlags_HeightRegular,
            Self::Large => sys::ImGuiComboFlags_HeightLarge,
            Self::Largest => sys::ImGuiComboFlags_HeightLargest,
        }
    }
}

/// Preview/arrow layout for a combo box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ComboBoxPreviewMode {
    /// Standard preview box with arrow button.
    #[default]
    Preview,
    /// Standard preview box without the square arrow button.
    PreviewNoArrowButton,
    /// Width dynamically calculated from preview contents.
    PreviewFit,
    /// Fit preview width without the square arrow button.
    PreviewFitNoArrowButton,
    /// Display only a square arrow button.
    NoPreview,
}

impl ComboBoxPreviewMode {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Preview => 0,
            Self::PreviewNoArrowButton => sys::ImGuiComboFlags_NoArrowButton,
            Self::PreviewFit => sys::ImGuiComboFlags_WidthFitPreview,
            Self::PreviewFitNoArrowButton => {
                sys::ImGuiComboFlags_WidthFitPreview | sys::ImGuiComboFlags_NoArrowButton
            }
            Self::NoPreview => sys::ImGuiComboFlags_NoPreview,
        }
    }
}

/// Complete combo box options assembled from independent flags and exclusive
/// mode selections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ComboBoxOptions {
    pub flags: ComboBoxFlags,
    pub height: Option<ComboBoxHeight>,
    pub preview_mode: ComboBoxPreviewMode,
}

impl Default for ComboBoxOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl ComboBoxOptions {
    pub const fn new() -> Self {
        Self {
            flags: ComboBoxFlags::NONE,
            height: None,
            preview_mode: ComboBoxPreviewMode::Preview,
        }
    }

    pub fn flags(mut self, flags: ComboBoxFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn height(mut self, height: ComboBoxHeight) -> Self {
        self.height = Some(height);
        self
    }

    pub fn preview_mode(mut self, mode: ComboBoxPreviewMode) -> Self {
        self.preview_mode = mode;
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.height.map_or(0, ComboBoxHeight::raw) | self.preview_mode.raw()
    }

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        let unsupported_flags = self.flags.bits() & !ComboBoxFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiComboFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.raw();
        let height_mask = sys::ImGuiComboFlags_HeightMask_;
        let no_arrow_button = sys::ImGuiComboFlags_NoArrowButton;
        let no_preview = sys::ImGuiComboFlags_NoPreview;
        let width_fit_preview = sys::ImGuiComboFlags_WidthFitPreview;
        let supported = ComboBoxFlags::all().bits() | height_mask | combo_preview_mask();
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiComboFlags bits: 0x{unsupported:X}"
        );
        assert!(
            bits & (no_arrow_button | no_preview) != (no_arrow_button | no_preview),
            "{caller} cannot combine NO_ARROW_BUTTON with NO_PREVIEW"
        );
        assert!(
            bits & width_fit_preview == 0 || bits & no_preview == 0,
            "{caller} cannot combine WIDTH_FIT_PREVIEW with NO_PREVIEW"
        );
        assert!(
            (bits & height_mask).count_ones() <= 1,
            "{caller} accepts at most one combo height policy"
        );
    }
}

#[inline]
const fn combo_preview_mask() -> i32 {
    sys::ImGuiComboFlags_NoArrowButton
        | sys::ImGuiComboFlags_NoPreview
        | sys::ImGuiComboFlags_WidthFitPreview
}

impl From<ComboBoxFlags> for ComboBoxOptions {
    fn from(flags: ComboBoxFlags) -> Self {
        Self::new().flags(flags)
    }
}

/// Builder for a combo box widget
#[derive(Clone, Debug)]
#[must_use]
pub struct ComboBox<'ui, Label, Preview = &'static str> {
    pub label: Label,
    pub preview_value: Option<Preview>,
    pub options: ComboBoxOptions,
    pub ui: &'ui Ui,
}

impl<'ui, Label: AsRef<str>> ComboBox<'ui, Label> {
    /// Sets the preview value
    pub fn preview_value<P: AsRef<str>>(self, preview: P) -> ComboBox<'ui, Label, P> {
        ComboBox {
            label: self.label,
            preview_value: Some(preview),
            options: self.options,
            ui: self.ui,
        }
    }

    /// Sets the flags
    pub fn flags(mut self, flags: ComboBoxFlags) -> Self {
        self.options.flags = flags;
        self
    }

    /// Sets the popup height policy.
    pub fn height(mut self, height: ComboBoxHeight) -> Self {
        self.options.height = Some(height);
        self
    }

    /// Sets the preview/arrow layout.
    pub fn preview_mode(mut self, mode: ComboBoxPreviewMode) -> Self {
        self.options.preview_mode = mode;
        self
    }

    /// Creates a combo box and starts appending to it.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    pub fn begin(self) -> Option<ComboBoxToken<'ui>> {
        self.options.validate("ComboBox::begin()");
        let should_render = self.ui.with_scratch_txt_with_opt(
            self.label.as_ref(),
            self.preview_value,
            |label_ptr, preview_ptr| unsafe {
                sys::igBeginCombo(label_ptr, preview_ptr, self.options.raw())
            },
        );

        if should_render {
            Some(ComboBoxToken::new(self.ui))
        } else {
            None
        }
    }
}

/// Tracks a combo box that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in the exact window `Begin`
/// scope that created it. Prefer the combo closure builder for ordinary use.
#[must_use]
#[doc(alias = "EndCombo")]
pub struct ComboBoxToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> ComboBoxToken<'ui> {
    /// Creates a new combo box token
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndCombo, "ComboBoxToken"),
        }
    }

    /// Ends the combo box
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is no longer in its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl Drop for ComboBoxToken<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
