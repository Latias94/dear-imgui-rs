use crate::sys;
use bitflags::bitflags;

/// Mouse button identifier
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MouseButton {
    /// Left mouse button
    Left = sys::ImGuiMouseButton_Left as i32,
    /// Right mouse button
    Right = sys::ImGuiMouseButton_Right as i32,
    /// Middle mouse button
    Middle = sys::ImGuiMouseButton_Middle as i32,
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
    None = sys::ImGuiKey_None as i32,
    /// Tab key
    Tab = sys::ImGuiKey_Tab as i32,
    /// Left arrow key
    LeftArrow = sys::ImGuiKey_LeftArrow as i32,
    /// Right arrow key
    RightArrow = sys::ImGuiKey_RightArrow as i32,
    /// Up arrow key
    UpArrow = sys::ImGuiKey_UpArrow as i32,
    /// Down arrow key
    DownArrow = sys::ImGuiKey_DownArrow as i32,
    /// Page up key
    PageUp = sys::ImGuiKey_PageUp as i32,
    /// Page down key
    PageDown = sys::ImGuiKey_PageDown as i32,
    /// Home key
    Home = sys::ImGuiKey_Home as i32,
    /// End key
    End = sys::ImGuiKey_End as i32,
    /// Insert key
    Insert = sys::ImGuiKey_Insert as i32,
    /// Delete key
    Delete = sys::ImGuiKey_Delete as i32,
    /// Backspace key
    Backspace = sys::ImGuiKey_Backspace as i32,
    /// Space key
    Space = sys::ImGuiKey_Space as i32,
    /// Enter key
    Enter = sys::ImGuiKey_Enter as i32,
    /// Escape key
    Escape = sys::ImGuiKey_Escape as i32,
    /// Left Ctrl key
    LeftCtrl = sys::ImGuiKey_LeftCtrl as i32,
    /// Left Shift key
    LeftShift = sys::ImGuiKey_LeftShift as i32,
    /// Left Alt key
    LeftAlt = sys::ImGuiKey_LeftAlt as i32,
    /// Left Super key
    LeftSuper = sys::ImGuiKey_LeftSuper as i32,
    /// Right Ctrl key
    RightCtrl = sys::ImGuiKey_RightCtrl as i32,
    /// Right Shift key
    RightShift = sys::ImGuiKey_RightShift as i32,
    /// Right Alt key
    RightAlt = sys::ImGuiKey_RightAlt as i32,
    /// Right Super key
    RightSuper = sys::ImGuiKey_RightSuper as i32,
    /// Menu key
    Menu = sys::ImGuiKey_Menu as i32,
    /// 0 key
    Key0 = sys::ImGuiKey_0 as i32,
    /// 1 key
    Key1 = sys::ImGuiKey_1 as i32,
    /// 2 key
    Key2 = sys::ImGuiKey_2 as i32,
    /// 3 key
    Key3 = sys::ImGuiKey_3 as i32,
    /// 4 key
    Key4 = sys::ImGuiKey_4 as i32,
    /// 5 key
    Key5 = sys::ImGuiKey_5 as i32,
    /// 6 key
    Key6 = sys::ImGuiKey_6 as i32,
    /// 7 key
    Key7 = sys::ImGuiKey_7 as i32,
    /// 8 key
    Key8 = sys::ImGuiKey_8 as i32,
    /// 9 key
    Key9 = sys::ImGuiKey_9 as i32,
    /// A key
    A = sys::ImGuiKey_A as i32,
    /// B key
    B = sys::ImGuiKey_B as i32,
    /// C key
    C = sys::ImGuiKey_C as i32,
    /// D key
    D = sys::ImGuiKey_D as i32,
    /// E key
    E = sys::ImGuiKey_E as i32,
    /// F key
    F = sys::ImGuiKey_F as i32,
    /// G key
    G = sys::ImGuiKey_G as i32,
    /// H key
    H = sys::ImGuiKey_H as i32,
    /// I key
    I = sys::ImGuiKey_I as i32,
    /// J key
    J = sys::ImGuiKey_J as i32,
    /// K key
    K = sys::ImGuiKey_K as i32,
    /// L key
    L = sys::ImGuiKey_L as i32,
    /// M key
    M = sys::ImGuiKey_M as i32,
    /// N key
    N = sys::ImGuiKey_N as i32,
    /// O key
    O = sys::ImGuiKey_O as i32,
    /// P key
    P = sys::ImGuiKey_P as i32,
    /// Q key
    Q = sys::ImGuiKey_Q as i32,
    /// R key
    R = sys::ImGuiKey_R as i32,
    /// S key
    S = sys::ImGuiKey_S as i32,
    /// T key
    T = sys::ImGuiKey_T as i32,
    /// U key
    U = sys::ImGuiKey_U as i32,
    /// V key
    V = sys::ImGuiKey_V as i32,
    /// W key
    W = sys::ImGuiKey_W as i32,
    /// X key
    X = sys::ImGuiKey_X as i32,
    /// Y key
    Y = sys::ImGuiKey_Y as i32,
    /// Z key
    Z = sys::ImGuiKey_Z as i32,
    /// F1 key
    F1 = sys::ImGuiKey_F1 as i32,
    /// F2 key
    F2 = sys::ImGuiKey_F2 as i32,
    /// F3 key
    F3 = sys::ImGuiKey_F3 as i32,
    /// F4 key
    F4 = sys::ImGuiKey_F4 as i32,
    /// F5 key
    F5 = sys::ImGuiKey_F5 as i32,
    /// F6 key
    F6 = sys::ImGuiKey_F6 as i32,
    /// F7 key
    F7 = sys::ImGuiKey_F7 as i32,
    /// F8 key
    F8 = sys::ImGuiKey_F8 as i32,
    /// F9 key
    F9 = sys::ImGuiKey_F9 as i32,
    /// F10 key
    F10 = sys::ImGuiKey_F10 as i32,
    /// F11 key
    F11 = sys::ImGuiKey_F11 as i32,
    /// F12 key
    F12 = sys::ImGuiKey_F12 as i32,
}

impl From<MouseButton> for sys::ImGuiMouseButton {
    #[inline]
    fn from(value: MouseButton) -> sys::ImGuiMouseButton {
        value as sys::ImGuiMouseButton
    }
}

impl From<Key> for sys::ImGuiKey {
    #[inline]
    fn from(value: Key) -> sys::ImGuiKey {
        value as sys::ImGuiKey
    }
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

bitflags! {
    /// Input text flags for text input widgets
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputTextFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiInputTextFlags_None as i32;
        /// Allow 0123456789.+-*/
        const CHARS_DECIMAL = sys::ImGuiInputTextFlags_CharsDecimal as i32;
        /// Allow 0123456789ABCDEFabcdef
        const CHARS_HEXADECIMAL = sys::ImGuiInputTextFlags_CharsHexadecimal as i32;
        /// Turn a..z into A..Z
        const CHARS_UPPERCASE = sys::ImGuiInputTextFlags_CharsUppercase as i32;
        /// Filter out spaces, tabs
        const CHARS_NO_BLANK = sys::ImGuiInputTextFlags_CharsNoBlank as i32;
        /// Select entire text when first taking mouse focus
        const AUTO_SELECT_ALL = sys::ImGuiInputTextFlags_AutoSelectAll as i32;
        /// Return 'true' when Enter is pressed (as opposed to every time the value was modified)
        const ENTER_RETURNS_TRUE = sys::ImGuiInputTextFlags_EnterReturnsTrue as i32;
        /// Callback on pressing TAB (for completion handling)
        const CALLBACK_COMPLETION = sys::ImGuiInputTextFlags_CallbackCompletion as i32;
        /// Callback on pressing Up/Down arrows (for history handling)
        const CALLBACK_HISTORY = sys::ImGuiInputTextFlags_CallbackHistory as i32;
        /// Callback on each iteration (user can query cursor and modify text)
        const CALLBACK_ALWAYS = sys::ImGuiInputTextFlags_CallbackAlways as i32;
        /// Callback on character inputs to replace or discard them
        const CALLBACK_CHAR_FILTER = sys::ImGuiInputTextFlags_CallbackCharFilter as i32;
        /// Pressing TAB input a '\t' character into the text field
        const ALLOW_TAB_INPUT = sys::ImGuiInputTextFlags_AllowTabInput as i32;
        /// In multi-line mode, unfocus with Enter, add new line with Ctrl+Enter
        const CTRL_ENTER_FOR_NEW_LINE = sys::ImGuiInputTextFlags_CtrlEnterForNewLine as i32;
        /// Disable following the cursor horizontally
        const NO_HORIZONTAL_SCROLL = sys::ImGuiInputTextFlags_NoHorizontalScroll as i32;
        /// Overwrite mode
        const ALWAYS_OVERWRITE = sys::ImGuiInputTextFlags_AlwaysOverwrite as i32;
        /// Read-only mode
        const READ_ONLY = sys::ImGuiInputTextFlags_ReadOnly as i32;
        /// Password mode, display all characters as '*'
        const PASSWORD = sys::ImGuiInputTextFlags_Password as i32;
        /// Disable undo/redo
        const NO_UNDO_REDO = sys::ImGuiInputTextFlags_NoUndoRedo as i32;
        /// Allow 0123456789.+-*/eE (Scientific notation input)
        const CHARS_SCIENTIFIC = sys::ImGuiInputTextFlags_CharsScientific as i32;
        /// Callback on buffer capacity changes request
        const CALLBACK_RESIZE = sys::ImGuiInputTextFlags_CallbackResize as i32;
        /// Callback on any edit (note that InputText() already returns true on edit)
        const CALLBACK_EDIT = sys::ImGuiInputTextFlags_CallbackEdit as i32;
    }
}

// TODO: Add NavInput enum once we have proper constants in sys crate

impl crate::Ui {
    /// Check if a key is being held down
    #[doc(alias = "IsKeyDown")]
    pub fn is_key_down(&self, key: Key) -> bool {
        unsafe { sys::igIsKeyDown_Nil(key as sys::ImGuiKey) }
    }

    /// Check if a key was pressed (went from !Down to Down)
    #[doc(alias = "IsKeyPressed")]
    pub fn is_key_pressed(&self, key: Key) -> bool {
        unsafe { sys::igIsKeyPressed_Bool(key as sys::ImGuiKey, true) }
    }

    /// Check if a key was pressed (went from !Down to Down), with repeat
    #[doc(alias = "IsKeyPressed")]
    pub fn is_key_pressed_with_repeat(&self, key: Key, repeat: bool) -> bool {
        unsafe { sys::igIsKeyPressed_Bool(key as sys::ImGuiKey, repeat) }
    }

    /// Check if a key was released (went from Down to !Down)
    #[doc(alias = "IsKeyReleased")]
    pub fn is_key_released(&self, key: Key) -> bool {
        unsafe { sys::igIsKeyReleased_Nil(key as sys::ImGuiKey) }
    }

    /// Check if a mouse button is being held down
    #[doc(alias = "IsMouseDown")]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseDown_Nil(button.into()) }
    }

    /// Check if a mouse button was clicked (went from !Down to Down)
    #[doc(alias = "IsMouseClicked")]
    pub fn is_mouse_clicked(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseClicked_Bool(button.into(), false) }
    }

    /// Check if a mouse button was clicked, with repeat
    #[doc(alias = "IsMouseClicked")]
    pub fn is_mouse_clicked_with_repeat(&self, button: MouseButton, repeat: bool) -> bool {
        unsafe { sys::igIsMouseClicked_Bool(button.into(), repeat) }
    }

    /// Check if a mouse button was released (went from Down to !Down)
    #[doc(alias = "IsMouseReleased")]
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseReleased_Nil(button.into()) }
    }

    /// Check if a mouse button was double-clicked
    #[doc(alias = "IsMouseDoubleClicked")]
    pub fn is_mouse_double_clicked(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseDoubleClicked_Nil(button.into()) }
    }

    /// Get mouse position in screen coordinates
    #[doc(alias = "GetMousePos")]
    pub fn mouse_pos(&self) -> [f32; 2] {
        let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
        unsafe { sys::igGetMousePos(&mut pos) };
        [pos.x, pos.y]
    }

    /// Get mouse position when a specific button was clicked
    #[doc(alias = "GetMousePosOnOpeningCurrentPopup")]
    pub fn mouse_pos_on_opening_current_popup(&self) -> [f32; 2] {
        let mut pos = sys::ImVec2 { x: 0.0, y: 0.0 };
        unsafe { sys::igGetMousePosOnOpeningCurrentPopup(&mut pos) };
        [pos.x, pos.y]
    }

    /// Check if mouse is hovering given rectangle
    #[doc(alias = "IsMouseHoveringRect")]
    pub fn is_mouse_hovering_rect(&self, r_min: [f32; 2], r_max: [f32; 2]) -> bool {
        unsafe {
            sys::igIsMouseHoveringRect(
                sys::ImVec2::new(r_min[0], r_min[1]),
                sys::ImVec2::new(r_max[0], r_max[1]),
                true,
            )
        }
    }

    /// Check if mouse is hovering given rectangle (with clipping test)
    #[doc(alias = "IsMouseHoveringRect")]
    pub fn is_mouse_hovering_rect_with_clip(
        &self,
        r_min: [f32; 2],
        r_max: [f32; 2],
        clip: bool,
    ) -> bool {
        unsafe {
            sys::igIsMouseHoveringRect(
                sys::ImVec2::new(r_min[0], r_min[1]),
                sys::ImVec2::new(r_max[0], r_max[1]),
                clip,
            )
        }
    }

    /// Check if mouse is dragging
    #[doc(alias = "IsMouseDragging")]
    pub fn is_mouse_dragging(&self, button: MouseButton) -> bool {
        unsafe { sys::igIsMouseDragging(button as i32, -1.0) }
    }

    /// Check if mouse is dragging with threshold
    #[doc(alias = "IsMouseDragging")]
    pub fn is_mouse_dragging_with_threshold(
        &self,
        button: MouseButton,
        lock_threshold: f32,
    ) -> bool {
        unsafe { sys::igIsMouseDragging(button as i32, lock_threshold) }
    }

    /// Get mouse drag delta
    #[doc(alias = "GetMouseDragDelta")]
    pub fn mouse_drag_delta(&self, button: MouseButton) -> [f32; 2] {
        let mut delta = sys::ImVec2 { x: 0.0, y: 0.0 };
        unsafe { sys::igGetMouseDragDelta(&mut delta, button as i32, -1.0) };
        [delta.x, delta.y]
    }

    /// Get mouse drag delta with threshold
    #[doc(alias = "GetMouseDragDelta")]
    pub fn mouse_drag_delta_with_threshold(
        &self,
        button: MouseButton,
        lock_threshold: f32,
    ) -> [f32; 2] {
        let mut delta = sys::ImVec2 { x: 0.0, y: 0.0 };
        unsafe { sys::igGetMouseDragDelta(&mut delta, button as i32, lock_threshold) };
        [delta.x, delta.y]
    }

    /// Reset mouse drag delta for a specific button
    #[doc(alias = "ResetMouseDragDelta")]
    pub fn reset_mouse_drag_delta(&self, button: MouseButton) {
        unsafe { sys::igResetMouseDragDelta(button as i32) }
    }
}
