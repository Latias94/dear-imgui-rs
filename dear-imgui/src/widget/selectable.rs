//! Selectable items
//!
//! Clickable items that can be selected, typically used in lists. Supports
//! span-full-width behavior and selection flags.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::Ui;
use crate::sys;

fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

fn validate_selectable_flags(caller: &str, flags: SelectableFlags) {
    let unsupported = flags.bits() & !SelectableFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiSelectableFlags bits: 0x{unsupported:X}"
    );
}

bitflags::bitflags! {
    /// Flags for selectables
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SelectableFlags: i32 {
        /// Clicking this don't close parent popup window
        const NO_AUTO_CLOSE_POPUPS = sys::ImGuiSelectableFlags_NoAutoClosePopups as i32;
        /// Selectable frame can span all columns (text will still fit in current column)
        const SPAN_ALL_COLUMNS = sys::ImGuiSelectableFlags_SpanAllColumns as i32;
        /// Generate press events on double clicks too
        const ALLOW_DOUBLE_CLICK = sys::ImGuiSelectableFlags_AllowDoubleClick as i32;
        /// Cannot be selected, display greyed out text
        const DISABLED = sys::ImGuiSelectableFlags_Disabled as i32;
        /// Hit testing to allow subsequent widgets to overlap this one
        const ALLOW_OVERLAP = sys::ImGuiSelectableFlags_AllowOverlap as i32;
        /// Display the selectable as highlighted, as if hovered.
        const HIGHLIGHT = sys::ImGuiSelectableFlags_Highlight as i32;
        /// Auto-select when moved into by navigation, unless Ctrl is held.
        const SELECT_ON_NAV = sys::ImGuiSelectableFlags_SelectOnNav as i32;
    }
}

impl Ui {
    /// Constructs a new simple selectable.
    ///
    /// Use [selectable_config] for a builder with additional options.
    ///
    /// [selectable_config]: Self::selectable_config
    #[doc(alias = "Selectable")]
    pub fn selectable<T: AsRef<str>>(&self, label: T) -> bool {
        self.selectable_config(label).build()
    }

    /// Constructs a new selectable builder.
    #[doc(alias = "Selectable")]
    pub fn selectable_config<T: AsRef<str>>(&self, label: T) -> Selectable<'_, T> {
        Selectable {
            label,
            selected: false,
            flags: SelectableFlags::empty(),
            size: [0.0, 0.0],
            ui: self,
        }
    }
}

/// Builder for a selectable widget.
#[derive(Clone, Debug)]
#[must_use]
pub struct Selectable<'ui, T> {
    label: T,
    selected: bool,
    flags: SelectableFlags,
    size: [f32; 2],
    ui: &'ui Ui,
}

impl<'ui, T: AsRef<str>> Selectable<'ui, T> {
    /// Constructs a new selectable builder.
    #[doc(alias = "Selectable")]
    #[deprecated(
        since = "0.9.0",
        note = "use `ui.selectable` or `ui.selectable_config`"
    )]
    pub fn new(label: T, ui: &'ui Ui) -> Self {
        Selectable {
            label,
            selected: false,
            flags: SelectableFlags::empty(),
            size: [0.0, 0.0],
            ui,
        }
    }
    /// Replaces all current settings with the given flags
    pub fn flags(mut self, flags: SelectableFlags) -> Self {
        self.flags = flags;
        self
    }
    /// Sets the selected state of the selectable
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    /// Enables/disables closing parent popup window on click.
    ///
    /// Default: enabled
    pub fn close_popups(mut self, value: bool) -> Self {
        self.flags
            .set(SelectableFlags::NO_AUTO_CLOSE_POPUPS, !value);
        self
    }
    /// Enables/disables full column span (text will still fit in the current column).
    ///
    /// Default: disabled
    pub fn span_all_columns(mut self, value: bool) -> Self {
        self.flags.set(SelectableFlags::SPAN_ALL_COLUMNS, value);
        self
    }
    /// Enables/disables click event generation on double clicks.
    ///
    /// Default: disabled
    pub fn allow_double_click(mut self, value: bool) -> Self {
        self.flags.set(SelectableFlags::ALLOW_DOUBLE_CLICK, value);
        self
    }
    /// Enables/disables the selectable.
    ///
    /// When disabled, it cannot be selected and the text uses the disabled text color.
    ///
    /// Default: disabled
    pub fn disabled(mut self, value: bool) -> Self {
        self.flags.set(SelectableFlags::DISABLED, value);
        self
    }
    /// Sets the size of the selectable.
    ///
    /// For the X axis:
    ///
    /// - `> 0.0`: use given width
    /// - `= 0.0`: use remaining width
    ///
    /// For the Y axis:
    ///
    /// - `> 0.0`: use given height
    /// - `= 0.0`: use label height
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = size.into();
        self
    }

    /// Builds the selectable.
    ///
    /// Returns true if the selectable was clicked.
    pub fn build(self) -> bool {
        validate_selectable_flags("Selectable::build()", self.flags);
        assert_non_negative_finite_vec2("Selectable::build()", "size", self.size);
        let size_vec = sys::ImVec2 {
            x: self.size[0],
            y: self.size[1],
        };
        self.ui.run_with_bound_context(|| unsafe {
            sys::igSelectable_Bool(
                self.ui.scratch_txt(self.label),
                self.selected,
                self.flags.bits(),
                size_vec,
            )
        })
    }

    /// Builds the selectable using a mutable reference to selected state.
    pub fn build_with_ref(self, selected: &mut bool) -> bool {
        validate_selectable_flags("Selectable::build_with_ref()", self.flags);
        assert_non_negative_finite_vec2("Selectable::build_with_ref()", "size", self.size);
        let size_vec = sys::ImVec2 {
            x: self.size[0],
            y: self.size[1],
        };
        self.ui.run_with_bound_context(|| unsafe {
            sys::igSelectable_BoolPtr(
                self.ui.scratch_txt(self.label),
                selected as *mut bool,
                self.flags.bits(),
                size_vec,
            )
        })
    }
}
