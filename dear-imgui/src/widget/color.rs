//! Color widgets
//!
//! Color edit/picker/button widgets and their option flags. Useful for editing
//! RGBA values with different display/input modes.
//!
//! Upstream's normal alpha preview is the zero-bit default, not an alias for `ALPHA_NO_BG`:
//! ```compile_fail
//! let _ = dear_imgui_rs::ColorEditFlags::ALPHA_PREVIEW;
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::ColorPickerFlags::ALPHA_PREVIEW;
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::ColorButtonFlags::ALPHA_PREVIEW;
//! ```
//!
mod button;
mod edit;
mod entry;
mod flags;
mod picker;
mod validation;

pub use button::ColorButton;
pub use edit::{ColorEdit3, ColorEdit4};
pub use flags::{
    ColorButtonFlags, ColorButtonOptions, ColorDataType, ColorDisplayMode, ColorEditFlags,
    ColorEditOptions, ColorInputMode, ColorPickerDisplayFlags, ColorPickerFlags, ColorPickerMode,
    ColorPickerOptions,
};
pub use picker::{ColorPicker3, ColorPicker4};
