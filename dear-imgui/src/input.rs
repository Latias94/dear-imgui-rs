use crate::sys;
use bitflags::bitflags;

/// Mouse button identifier
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MouseButton {
    /// Left mouse button
    Left = sys::ImGuiMouseButton_Left,
    /// Right mouse button
    Right = sys::ImGuiMouseButton_Right,
    /// Middle mouse button
    Middle = sys::ImGuiMouseButton_Middle,
}

/// Mouse cursor types
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MouseCursor {
    /// No cursor
    None = sys::ImGuiMouseCursor_None,
    /// Arrow cursor
    Arrow = sys::ImGuiMouseCursor_Arrow,
    /// Text input I-beam cursor
    TextInput = sys::ImGuiMouseCursor_TextInput,
    /// Resize all directions cursor
    ResizeAll = sys::ImGuiMouseCursor_ResizeAll,
    /// Resize north-south cursor
    ResizeNS = sys::ImGuiMouseCursor_ResizeNS,
    /// Resize east-west cursor
    ResizeEW = sys::ImGuiMouseCursor_ResizeEW,
    /// Resize northeast-southwest cursor
    ResizeNESW = sys::ImGuiMouseCursor_ResizeNESW,
    /// Resize northwest-southeast cursor
    ResizeNWSE = sys::ImGuiMouseCursor_ResizeNWSE,
    /// Hand cursor
    Hand = sys::ImGuiMouseCursor_Hand,
    /// Not allowed cursor
    NotAllowed = sys::ImGuiMouseCursor_NotAllowed,
}

/// Key identifier for keyboard input
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Key {
    /// No key
    None = sys::ImGuiKey_None,
    /// Tab key
    Tab = sys::ImGuiKey_Tab,
    /// Left arrow key
    LeftArrow = sys::ImGuiKey_LeftArrow,
    /// Right arrow key
    RightArrow = sys::ImGuiKey_RightArrow,
    /// Up arrow key
    UpArrow = sys::ImGuiKey_UpArrow,
    /// Down arrow key
    DownArrow = sys::ImGuiKey_DownArrow,
    /// Page up key
    PageUp = sys::ImGuiKey_PageUp,
    /// Page down key
    PageDown = sys::ImGuiKey_PageDown,
    /// Home key
    Home = sys::ImGuiKey_Home,
    /// End key
    End = sys::ImGuiKey_End,
    /// Insert key
    Insert = sys::ImGuiKey_Insert,
    /// Delete key
    Delete = sys::ImGuiKey_Delete,
    /// Backspace key
    Backspace = sys::ImGuiKey_Backspace,
    /// Space key
    Space = sys::ImGuiKey_Space,
    /// Enter key
    Enter = sys::ImGuiKey_Enter,
    /// Escape key
    Escape = sys::ImGuiKey_Escape,
    /// Left Ctrl key
    LeftCtrl = sys::ImGuiKey_LeftCtrl,
    /// Left Shift key
    LeftShift = sys::ImGuiKey_LeftShift,
    /// Left Alt key
    LeftAlt = sys::ImGuiKey_LeftAlt,
    /// Left Super key
    LeftSuper = sys::ImGuiKey_LeftSuper,
    /// Right Ctrl key
    RightCtrl = sys::ImGuiKey_RightCtrl,
    /// Right Shift key
    RightShift = sys::ImGuiKey_RightShift,
    /// Right Alt key
    RightAlt = sys::ImGuiKey_RightAlt,
    /// Right Super key
    RightSuper = sys::ImGuiKey_RightSuper,
    /// Menu key
    Menu = sys::ImGuiKey_Menu,
    /// 0 key
    Key0 = sys::ImGuiKey_0,
    /// 1 key
    Key1 = sys::ImGuiKey_1,
    /// 2 key
    Key2 = sys::ImGuiKey_2,
    /// 3 key
    Key3 = sys::ImGuiKey_3,
    /// 4 key
    Key4 = sys::ImGuiKey_4,
    /// 5 key
    Key5 = sys::ImGuiKey_5,
    /// 6 key
    Key6 = sys::ImGuiKey_6,
    /// 7 key
    Key7 = sys::ImGuiKey_7,
    /// 8 key
    Key8 = sys::ImGuiKey_8,
    /// 9 key
    Key9 = sys::ImGuiKey_9,
    /// A key
    A = sys::ImGuiKey_A,
    /// B key
    B = sys::ImGuiKey_B,
    /// C key
    C = sys::ImGuiKey_C,
    /// D key
    D = sys::ImGuiKey_D,
    /// E key
    E = sys::ImGuiKey_E,
    /// F key
    F = sys::ImGuiKey_F,
    /// G key
    G = sys::ImGuiKey_G,
    /// H key
    H = sys::ImGuiKey_H,
    /// I key
    I = sys::ImGuiKey_I,
    /// J key
    J = sys::ImGuiKey_J,
    /// K key
    K = sys::ImGuiKey_K,
    /// L key
    L = sys::ImGuiKey_L,
    /// M key
    M = sys::ImGuiKey_M,
    /// N key
    N = sys::ImGuiKey_N,
    /// O key
    O = sys::ImGuiKey_O,
    /// P key
    P = sys::ImGuiKey_P,
    /// Q key
    Q = sys::ImGuiKey_Q,
    /// R key
    R = sys::ImGuiKey_R,
    /// S key
    S = sys::ImGuiKey_S,
    /// T key
    T = sys::ImGuiKey_T,
    /// U key
    U = sys::ImGuiKey_U,
    /// V key
    V = sys::ImGuiKey_V,
    /// W key
    W = sys::ImGuiKey_W,
    /// X key
    X = sys::ImGuiKey_X,
    /// Y key
    Y = sys::ImGuiKey_Y,
    /// Z key
    Z = sys::ImGuiKey_Z,
    /// F1 key
    F1 = sys::ImGuiKey_F1,
    /// F2 key
    F2 = sys::ImGuiKey_F2,
    /// F3 key
    F3 = sys::ImGuiKey_F3,
    /// F4 key
    F4 = sys::ImGuiKey_F4,
    /// F5 key
    F5 = sys::ImGuiKey_F5,
    /// F6 key
    F6 = sys::ImGuiKey_F6,
    /// F7 key
    F7 = sys::ImGuiKey_F7,
    /// F8 key
    F8 = sys::ImGuiKey_F8,
    /// F9 key
    F9 = sys::ImGuiKey_F9,
    /// F10 key
    F10 = sys::ImGuiKey_F10,
    /// F11 key
    F11 = sys::ImGuiKey_F11,
    /// F12 key
    F12 = sys::ImGuiKey_F12,
}

bitflags! {
    /// Key modifier flags
    #[repr(transparent)]
    pub struct KeyModFlags: i32 {
        /// No modifiers
        const NONE = 0;
        /// Ctrl key modifier
        const CTRL = 1 << 0;
        /// Shift key modifier
        const SHIFT = 1 << 1;
        /// Alt key modifier
        const ALT = 1 << 2;
        /// Super key modifier
        const SUPER = 1 << 3;
    }
}

/// Input text flags for text input widgets
bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputTextFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiInputTextFlags_None;
        /// Allow 0123456789.+-*/
        const CHARS_DECIMAL = sys::ImGuiInputTextFlags_CharsDecimal;
        /// Allow 0123456789ABCDEFabcdef
        const CHARS_HEXADECIMAL = sys::ImGuiInputTextFlags_CharsHexadecimal;
        /// Turn a..z into A..Z
        const CHARS_UPPERCASE = sys::ImGuiInputTextFlags_CharsUppercase;
        /// Filter out spaces, tabs
        const CHARS_NO_BLANK = sys::ImGuiInputTextFlags_CharsNoBlank;
        /// Select entire text when first taking mouse focus
        const AUTO_SELECT_ALL = sys::ImGuiInputTextFlags_AutoSelectAll;
        /// Return 'true' when Enter is pressed (as opposed to every time the value was modified)
        const ENTER_RETURNS_TRUE = sys::ImGuiInputTextFlags_EnterReturnsTrue;
        /// Callback on pressing TAB (for completion handling)
        const CALLBACK_COMPLETION = sys::ImGuiInputTextFlags_CallbackCompletion;
        /// Callback on pressing Up/Down arrows (for history handling)
        const CALLBACK_HISTORY = sys::ImGuiInputTextFlags_CallbackHistory;
        /// Callback on each iteration (user can query cursor and modify text)
        const CALLBACK_ALWAYS = sys::ImGuiInputTextFlags_CallbackAlways;
        /// Callback on character inputs to replace or discard them
        const CALLBACK_CHAR_FILTER = sys::ImGuiInputTextFlags_CallbackCharFilter;
        /// Pressing TAB input a '\t' character into the text field
        const ALLOW_TAB_INPUT = sys::ImGuiInputTextFlags_AllowTabInput;
        /// In multi-line mode, unfocus with Enter, add new line with Ctrl+Enter
        const CTRL_ENTER_FOR_NEW_LINE = sys::ImGuiInputTextFlags_CtrlEnterForNewLine;
        /// Disable following the cursor horizontally
        const NO_HORIZONTAL_SCROLL = sys::ImGuiInputTextFlags_NoHorizontalScroll;
        /// Overwrite mode
        const ALWAYS_OVERWRITE = sys::ImGuiInputTextFlags_AlwaysOverwrite;
        /// Read-only mode
        const READ_ONLY = sys::ImGuiInputTextFlags_ReadOnly;
        /// Password mode, display all characters as '*'
        const PASSWORD = sys::ImGuiInputTextFlags_Password;
        /// Disable undo/redo
        const NO_UNDO_REDO = sys::ImGuiInputTextFlags_NoUndoRedo;
        /// Allow 0123456789.+-*/eE (Scientific notation input)
        const CHARS_SCIENTIFIC = sys::ImGuiInputTextFlags_CharsScientific;
        /// Callback on buffer capacity changes request
        const CALLBACK_RESIZE = sys::ImGuiInputTextFlags_CallbackResize;
        /// Callback on any edit (note that InputText() already returns true on edit)
        const CALLBACK_EDIT = sys::ImGuiInputTextFlags_CallbackEdit;
    }
}

// TODO: Add NavInput enum once we have proper constants in sys crate
