use crate::sys;

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
        const POPUP_ALIGN_LEFT = sys::ImGuiComboFlags_PopupAlignLeft as i32;
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
            Self::Small => sys::ImGuiComboFlags_HeightSmall as i32,
            Self::Regular => sys::ImGuiComboFlags_HeightRegular as i32,
            Self::Large => sys::ImGuiComboFlags_HeightLarge as i32,
            Self::Largest => sys::ImGuiComboFlags_HeightLargest as i32,
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
            Self::PreviewNoArrowButton => sys::ImGuiComboFlags_NoArrowButton as i32,
            Self::PreviewFit => sys::ImGuiComboFlags_WidthFitPreview as i32,
            Self::PreviewFitNoArrowButton => {
                sys::ImGuiComboFlags_WidthFitPreview as i32
                    | sys::ImGuiComboFlags_NoArrowButton as i32
            }
            Self::NoPreview => sys::ImGuiComboFlags_NoPreview as i32,
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
        let height_mask = sys::ImGuiComboFlags_HeightMask_ as i32;
        let no_arrow_button = sys::ImGuiComboFlags_NoArrowButton as i32;
        let no_preview = sys::ImGuiComboFlags_NoPreview as i32;
        let width_fit_preview = sys::ImGuiComboFlags_WidthFitPreview as i32;
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
    (sys::ImGuiComboFlags_NoArrowButton
        | sys::ImGuiComboFlags_NoPreview
        | sys::ImGuiComboFlags_WidthFitPreview) as i32
}

impl From<ComboBoxFlags> for ComboBoxOptions {
    fn from(flags: ComboBoxFlags) -> Self {
        Self::new().flags(flags)
    }
}
