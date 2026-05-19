use crate::sys;
use bitflags::bitflags;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

macro_rules! impl_input_flag_raw {
    ($ty:ident) => {
        impl $ty {
            #[inline]
            pub(crate) fn raw(self) -> sys::ImGuiInputTextFlags {
                self.bits() as sys::ImGuiInputTextFlags
            }
        }

        #[cfg(feature = "serde")]
        impl Serialize for $ty {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_i32(self.bits())
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let bits = i32::deserialize(deserializer)?;
                Ok(Self::from_bits_retain(bits))
            }
        }
    };
}

bitflags! {
    /// Independent flags accepted by single-line `InputText()` and
    /// `InputTextWithHint()` widgets.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputTextFlags: i32 {
        /// No flags.
        const NONE = sys::ImGuiInputTextFlags_None as i32;
        /// Allow 0123456789.+-*/
        const CHARS_DECIMAL = sys::ImGuiInputTextFlags_CharsDecimal as i32;
        /// Allow 0123456789ABCDEFabcdef
        const CHARS_HEXADECIMAL = sys::ImGuiInputTextFlags_CharsHexadecimal as i32;
        /// Allow 0123456789.+-*/eE (Scientific notation input)
        const CHARS_SCIENTIFIC = sys::ImGuiInputTextFlags_CharsScientific as i32;
        /// Turn a..z into A..Z
        const CHARS_UPPERCASE = sys::ImGuiInputTextFlags_CharsUppercase as i32;
        /// Filter out spaces, tabs
        const CHARS_NO_BLANK = sys::ImGuiInputTextFlags_CharsNoBlank as i32;

        /// Pressing TAB input a '\t' character into the text field.
        const ALLOW_TAB_INPUT = sys::ImGuiInputTextFlags_AllowTabInput as i32;
        /// Return `true` when Enter is pressed.
        const ENTER_RETURNS_TRUE = sys::ImGuiInputTextFlags_EnterReturnsTrue as i32;
        /// Escape key clears content if not empty, and deactivates otherwise.
        const ESCAPE_CLEARS_ALL = sys::ImGuiInputTextFlags_EscapeClearsAll as i32;

        /// Read-only mode.
        const READ_ONLY = sys::ImGuiInputTextFlags_ReadOnly as i32;
        /// Password mode, display all characters as '*'.
        const PASSWORD = sys::ImGuiInputTextFlags_Password as i32;
        /// Overwrite mode.
        const ALWAYS_OVERWRITE = sys::ImGuiInputTextFlags_AlwaysOverwrite as i32;
        /// Select entire text when first taking mouse focus.
        const AUTO_SELECT_ALL = sys::ImGuiInputTextFlags_AutoSelectAll as i32;
        /// Disable following the cursor horizontally.
        const NO_HORIZONTAL_SCROLL = sys::ImGuiInputTextFlags_NoHorizontalScroll as i32;
        /// Disable undo/redo.
        const NO_UNDO_REDO = sys::ImGuiInputTextFlags_NoUndoRedo as i32;
        /// When text doesn't fit, elide the left side.
        const ELIDE_LEFT = sys::ImGuiInputTextFlags_ElideLeft as i32;

        /// Callback on pressing TAB (for completion handling).
        const CALLBACK_COMPLETION = sys::ImGuiInputTextFlags_CallbackCompletion as i32;
        /// Callback on pressing Up/Down arrows (for history handling).
        const CALLBACK_HISTORY = sys::ImGuiInputTextFlags_CallbackHistory as i32;
        /// Callback on each iteration.
        const CALLBACK_ALWAYS = sys::ImGuiInputTextFlags_CallbackAlways as i32;
        /// Callback on character inputs to replace or discard them.
        const CALLBACK_CHAR_FILTER = sys::ImGuiInputTextFlags_CallbackCharFilter as i32;
        /// Callback on any edit.
        const CALLBACK_EDIT = sys::ImGuiInputTextFlags_CallbackEdit as i32;
    }
}

bitflags! {
    /// Independent flags accepted by `InputTextMultiline()` widgets.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputTextMultilineFlags: i32 {
        /// No flags.
        const NONE = sys::ImGuiInputTextFlags_None as i32;
        /// Allow 0123456789.+-*/
        const CHARS_DECIMAL = sys::ImGuiInputTextFlags_CharsDecimal as i32;
        /// Allow 0123456789ABCDEFabcdef
        const CHARS_HEXADECIMAL = sys::ImGuiInputTextFlags_CharsHexadecimal as i32;
        /// Allow 0123456789.+-*/eE (Scientific notation input)
        const CHARS_SCIENTIFIC = sys::ImGuiInputTextFlags_CharsScientific as i32;
        /// Turn a..z into A..Z
        const CHARS_UPPERCASE = sys::ImGuiInputTextFlags_CharsUppercase as i32;
        /// Filter out spaces, tabs
        const CHARS_NO_BLANK = sys::ImGuiInputTextFlags_CharsNoBlank as i32;

        /// Pressing TAB input a '\t' character into the text field.
        const ALLOW_TAB_INPUT = sys::ImGuiInputTextFlags_AllowTabInput as i32;
        /// Return `true` when Enter is pressed.
        const ENTER_RETURNS_TRUE = sys::ImGuiInputTextFlags_EnterReturnsTrue as i32;
        /// Escape key clears content if not empty, and deactivates otherwise.
        const ESCAPE_CLEARS_ALL = sys::ImGuiInputTextFlags_EscapeClearsAll as i32;
        /// In multi-line mode, unfocus with Enter, add new line with Ctrl+Enter.
        const CTRL_ENTER_FOR_NEW_LINE = sys::ImGuiInputTextFlags_CtrlEnterForNewLine as i32;

        /// Read-only mode.
        const READ_ONLY = sys::ImGuiInputTextFlags_ReadOnly as i32;
        /// Overwrite mode.
        const ALWAYS_OVERWRITE = sys::ImGuiInputTextFlags_AlwaysOverwrite as i32;
        /// Select entire text when first taking mouse focus.
        const AUTO_SELECT_ALL = sys::ImGuiInputTextFlags_AutoSelectAll as i32;
        /// Disable following the cursor horizontally.
        const NO_HORIZONTAL_SCROLL = sys::ImGuiInputTextFlags_NoHorizontalScroll as i32;
        /// Disable undo/redo.
        const NO_UNDO_REDO = sys::ImGuiInputTextFlags_NoUndoRedo as i32;
        /// Word-wrap lines that are too long.
        const WORD_WRAP = sys::ImGuiInputTextFlags_WordWrap as i32;

        /// Callback on each iteration.
        const CALLBACK_ALWAYS = sys::ImGuiInputTextFlags_CallbackAlways as i32;
        /// Callback on character inputs to replace or discard them.
        const CALLBACK_CHAR_FILTER = sys::ImGuiInputTextFlags_CallbackCharFilter as i32;
        /// Callback on any edit.
        const CALLBACK_EDIT = sys::ImGuiInputTextFlags_CallbackEdit as i32;
    }
}

bitflags! {
    /// Independent flags accepted by numeric `Input*()` and `InputScalar*()`
    /// widgets.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputScalarFlags: i32 {
        /// No flags.
        const NONE = sys::ImGuiInputTextFlags_None as i32;
        /// Allow 0123456789.+-*/
        const CHARS_DECIMAL = sys::ImGuiInputTextFlags_CharsDecimal as i32;
        /// Allow 0123456789ABCDEFabcdef
        const CHARS_HEXADECIMAL = sys::ImGuiInputTextFlags_CharsHexadecimal as i32;
        /// Allow 0123456789.+-*/eE (Scientific notation input)
        const CHARS_SCIENTIFIC = sys::ImGuiInputTextFlags_CharsScientific as i32;
        /// Turn a..z into A..Z
        const CHARS_UPPERCASE = sys::ImGuiInputTextFlags_CharsUppercase as i32;
        /// Filter out spaces, tabs
        const CHARS_NO_BLANK = sys::ImGuiInputTextFlags_CharsNoBlank as i32;

        /// Escape key clears content if not empty, and deactivates otherwise.
        const ESCAPE_CLEARS_ALL = sys::ImGuiInputTextFlags_EscapeClearsAll as i32;

        /// Read-only mode.
        const READ_ONLY = sys::ImGuiInputTextFlags_ReadOnly as i32;
        /// Overwrite mode.
        const ALWAYS_OVERWRITE = sys::ImGuiInputTextFlags_AlwaysOverwrite as i32;
        /// Select entire text when first taking mouse focus.
        const AUTO_SELECT_ALL = sys::ImGuiInputTextFlags_AutoSelectAll as i32;
        /// Parse empty text as the reference value (usually zero).
        const PARSE_EMPTY_REF_VAL = sys::ImGuiInputTextFlags_ParseEmptyRefVal as i32;
        /// Display the reference value (usually zero) as empty text.
        const DISPLAY_EMPTY_REF_VAL = sys::ImGuiInputTextFlags_DisplayEmptyRefVal as i32;
        /// Disable following the cursor horizontally.
        const NO_HORIZONTAL_SCROLL = sys::ImGuiInputTextFlags_NoHorizontalScroll as i32;
        /// Disable undo/redo.
        const NO_UNDO_REDO = sys::ImGuiInputTextFlags_NoUndoRedo as i32;
    }
}

impl_input_flag_raw!(InputTextFlags);
impl_input_flag_raw!(InputTextMultilineFlags);
impl_input_flag_raw!(InputScalarFlags);
