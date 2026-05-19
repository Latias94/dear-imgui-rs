use crate::sys;
use bitflags::bitflags;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Key identifier for keyboard input
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    /// Ctrl modifier (for `io.KeyMods` / `io.KeyCtrl`)
    ModCtrl = sys::ImGuiMod_Ctrl as i32,
    /// Shift modifier (for `io.KeyMods` / `io.KeyShift`)
    ModShift = sys::ImGuiMod_Shift as i32,
    /// Alt modifier (for `io.KeyMods` / `io.KeyAlt`)
    ModAlt = sys::ImGuiMod_Alt as i32,
    /// Super/Cmd modifier (for `io.KeyMods` / `io.KeySuper`)
    ModSuper = sys::ImGuiMod_Super as i32,
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

    // --- Punctuation and extra named keys ---
    /// Apostrophe (') key
    Apostrophe = sys::ImGuiKey_Apostrophe as i32,
    /// Comma (,) key
    Comma = sys::ImGuiKey_Comma as i32,
    /// Minus (-) key
    Minus = sys::ImGuiKey_Minus as i32,
    /// Period (.) key
    Period = sys::ImGuiKey_Period as i32,
    /// Slash (/) key
    Slash = sys::ImGuiKey_Slash as i32,
    /// Semicolon (;) key
    Semicolon = sys::ImGuiKey_Semicolon as i32,
    /// Equal (=) key
    Equal = sys::ImGuiKey_Equal as i32,
    /// Left bracket ([) key
    LeftBracket = sys::ImGuiKey_LeftBracket as i32,
    /// Backslash (\) key
    Backslash = sys::ImGuiKey_Backslash as i32,
    /// Right bracket (]) key
    RightBracket = sys::ImGuiKey_RightBracket as i32,
    /// Grave accent (`) key
    GraveAccent = sys::ImGuiKey_GraveAccent as i32,
    /// CapsLock key
    CapsLock = sys::ImGuiKey_CapsLock as i32,
    /// ScrollLock key
    ScrollLock = sys::ImGuiKey_ScrollLock as i32,
    /// NumLock key
    NumLock = sys::ImGuiKey_NumLock as i32,
    /// PrintScreen key
    PrintScreen = sys::ImGuiKey_PrintScreen as i32,
    /// Pause key
    Pause = sys::ImGuiKey_Pause as i32,

    // --- Keypad ---
    /// Numpad 0
    Keypad0 = sys::ImGuiKey_Keypad0 as i32,
    /// Numpad 1
    Keypad1 = sys::ImGuiKey_Keypad1 as i32,
    /// Numpad 2
    Keypad2 = sys::ImGuiKey_Keypad2 as i32,
    /// Numpad 3
    Keypad3 = sys::ImGuiKey_Keypad3 as i32,
    /// Numpad 4
    Keypad4 = sys::ImGuiKey_Keypad4 as i32,
    /// Numpad 5
    Keypad5 = sys::ImGuiKey_Keypad5 as i32,
    /// Numpad 6
    Keypad6 = sys::ImGuiKey_Keypad6 as i32,
    /// Numpad 7
    Keypad7 = sys::ImGuiKey_Keypad7 as i32,
    /// Numpad 8
    Keypad8 = sys::ImGuiKey_Keypad8 as i32,
    /// Numpad 9
    Keypad9 = sys::ImGuiKey_Keypad9 as i32,
    /// Numpad decimal
    KeypadDecimal = sys::ImGuiKey_KeypadDecimal as i32,
    /// Numpad divide
    KeypadDivide = sys::ImGuiKey_KeypadDivide as i32,
    /// Numpad multiply
    KeypadMultiply = sys::ImGuiKey_KeypadMultiply as i32,
    /// Numpad subtract
    KeypadSubtract = sys::ImGuiKey_KeypadSubtract as i32,
    /// Numpad add
    KeypadAdd = sys::ImGuiKey_KeypadAdd as i32,
    /// Numpad enter
    KeypadEnter = sys::ImGuiKey_KeypadEnter as i32,
    /// Numpad equal
    KeypadEqual = sys::ImGuiKey_KeypadEqual as i32,

    /// OEM 102 key (ISO < > |)
    Oem102 = sys::ImGuiKey_Oem102 as i32,
}

impl From<Key> for sys::ImGuiKey {
    #[inline]
    fn from(value: Key) -> sys::ImGuiKey {
        value as sys::ImGuiKey
    }
}

// Key modifier flags are available via io.KeyCtrl/KeyShift/KeyAlt/KeySuper.
// Backends should submit modifier state via `Key::ModCtrl`/`ModShift`/`ModAlt`/`ModSuper` using `Io::add_key_event`.

bitflags! {
    /// Modifier flags for building an ImGui key chord.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KeyMods: i32 {
        /// Ctrl modifier
        const CTRL = sys::ImGuiMod_Ctrl as i32;
        /// Shift modifier
        const SHIFT = sys::ImGuiMod_Shift as i32;
        /// Alt modifier
        const ALT = sys::ImGuiMod_Alt as i32;
        /// Super/Cmd modifier
        const SUPER = sys::ImGuiMod_Super as i32;
    }
}

impl Default for KeyMods {
    fn default() -> Self {
        KeyMods::empty()
    }
}

impl KeyMods {
    #[inline]
    pub(crate) fn raw(self) -> sys::ImGuiKeyChord {
        self.bits() as sys::ImGuiKeyChord
    }
}

/// A key chord (key + optional modifier flags), used by ImGui shortcut routing APIs.
///
/// This is a thin wrapper over `sys::ImGuiKeyChord` (an `int`).
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct KeyChord(sys::ImGuiKeyChord);

impl KeyChord {
    /// Create a chord from a key (no modifiers).
    pub fn new(key: Key) -> Self {
        Self(key as sys::ImGuiKeyChord)
    }

    /// Add modifier flags to the chord.
    pub fn with_mods(self, mods: KeyMods) -> Self {
        Self(self.0 | mods.raw())
    }

    /// Returns the raw `ImGuiKeyChord` value.
    pub fn raw(self) -> sys::ImGuiKeyChord {
        self.0
    }
}

impl From<Key> for KeyChord {
    fn from(value: Key) -> Self {
        Self::new(value)
    }
}
