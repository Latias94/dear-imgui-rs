use bitflags::bitflags;
use std::f32;

use crate::sys;
use crate::{Condition, Ui};

pub(crate) mod child_window;
pub(crate) mod content_region;
pub(crate) mod scroll;

bitflags! {
    /// Window hover check option flags
    #[repr(transparent)]
    pub struct WindowHoveredFlags: i32 {
        /// Return true if any child of the window is hovered
        const CHILD_WINDOWS = sys::ImGuiHoveredFlags_ChildWindows;
        /// Test from root window (top-most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiHoveredFlags_RootWindow;
        /// Return true if any window is hovered
        const ANY_WINDOW = sys::ImGuiHoveredFlags_AnyWindow;
        /// Return true even if a popup window is blocking access to this window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup;
        /// Return true even if an active item is blocking access to this window
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem;
        /// Test from root window, and return true if any child is hovered
        const ROOT_AND_CHILD_WINDOWS = Self::ROOT_WINDOW.bits() | Self::CHILD_WINDOWS.bits();
    }
}

bitflags! {
    /// Window focus check option flags
    #[repr(transparent)]
    pub struct WindowFocusedFlags: i32 {
        /// Return true if any child of the window is focused
        const CHILD_WINDOWS = sys::ImGuiFocusedFlags_ChildWindows;
        /// Test from root window (top-most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiFocusedFlags_RootWindow;
        /// Return true if any window is focused
        const ANY_WINDOW = sys::ImGuiFocusedFlags_AnyWindow;
        /// Test from root window, and return true if any child is focused
        const ROOT_AND_CHILD_WINDOWS = Self::ROOT_WINDOW.bits() | Self::CHILD_WINDOWS.bits();
    }
}

bitflags! {
    /// Configuration flags for windows
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowFlags: i32 {
        /// Disable title-bar
        const NO_TITLE_BAR = sys::ImGuiWindowFlags_NoTitleBar;
        /// Disable user resizing with the lower-right grip
        const NO_RESIZE = sys::ImGuiWindowFlags_NoResize;
        /// Disable user moving the window
        const NO_MOVE = sys::ImGuiWindowFlags_NoMove;
        /// Disable scrollbars (window can still scroll with mouse or programmatically)
        const NO_SCROLLBAR = sys::ImGuiWindowFlags_NoScrollbar;
        /// Disable user vertically scrolling with mouse wheel
        const NO_SCROLL_WITH_MOUSE = sys::ImGuiWindowFlags_NoScrollWithMouse;
        /// Disable user collapsing window by double-clicking on it
        const NO_COLLAPSE = sys::ImGuiWindowFlags_NoCollapse;
        /// Resize every window to its content every frame
        const ALWAYS_AUTO_RESIZE = sys::ImGuiWindowFlags_AlwaysAutoResize;
        /// Disable drawing background color (WindowBg, etc.) and outside border
        const NO_BACKGROUND = sys::ImGuiWindowFlags_NoBackground;
        /// Never load/save settings in .ini file
        const NO_SAVED_SETTINGS = sys::ImGuiWindowFlags_NoSavedSettings;
        /// Disable catching mouse, hovering test with pass through
        const NO_MOUSE_INPUTS = sys::ImGuiWindowFlags_NoMouseInputs;
        /// Has a menu-bar
        const MENU_BAR = sys::ImGuiWindowFlags_MenuBar;
        /// Allow horizontal scrollbar to appear (off by default)
        const HORIZONTAL_SCROLLBAR = sys::ImGuiWindowFlags_HorizontalScrollbar;
        /// Disable taking focus when transitioning from hidden to visible state
        const NO_FOCUS_ON_APPEARING = sys::ImGuiWindowFlags_NoFocusOnAppearing;
        /// Disable bringing window to front when taking focus (e.g. clicking on it or programmatically giving it focus)
        const NO_BRING_TO_FRONT_ON_FOCUS = sys::ImGuiWindowFlags_NoBringToFrontOnFocus;
        /// Always show vertical scrollbar (even if ContentSize.y < Size.y)
        const ALWAYS_VERTICAL_SCROLLBAR = sys::ImGuiWindowFlags_AlwaysVerticalScrollbar;
        /// Always show horizontal scrollbar (even if ContentSize.x < Size.x)
        const ALWAYS_HORIZONTAL_SCROLLBAR = sys::ImGuiWindowFlags_AlwaysHorizontalScrollbar;
        /// No gamepad/keyboard navigation within the window
        const NO_NAV_INPUTS = sys::ImGuiWindowFlags_NoNavInputs;
        /// No focusing toward this window with gamepad/keyboard navigation (e.g. skipped by CTRL+TAB)
        const NO_NAV_FOCUS = sys::ImGuiWindowFlags_NoNavFocus;
        /// Display a dot next to the title. When used in a tab/docking context, tab is selected when clicking the X + closure is not assumed (will wait for user to stop submitting the tab). Otherwise closure is assumed when pressing the X, so if you keep submitting the tab may reappear at end of tab bar.
        const UNSAVED_DOCUMENT = sys::ImGuiWindowFlags_UnsavedDocument;
        /// Disable gamepad/keyboard navigation and focusing
        const NO_NAV = Self::NO_NAV_INPUTS.bits() | Self::NO_NAV_FOCUS.bits();
        /// Disable all window decorations
        const NO_DECORATION = Self::NO_TITLE_BAR.bits() | Self::NO_RESIZE.bits() | Self::NO_SCROLLBAR.bits() | Self::NO_COLLAPSE.bits();
        /// Disable all user interactions
        const NO_INPUTS = Self::NO_MOUSE_INPUTS.bits() | Self::NO_NAV_INPUTS.bits();
    }
}

/// Represents a window that can be built
pub struct Window<'ui> {
    ui: &'ui Ui,
    name: String,
    flags: WindowFlags,
    size: Option<[f32; 2]>,
    size_condition: Condition,
    pos: Option<[f32; 2]>,
    pos_condition: Condition,
    content_size: Option<[f32; 2]>,
    content_size_condition: Condition,
    collapsed: Option<bool>,
    collapsed_condition: Condition,
    focused: Option<bool>,
    bg_alpha: Option<f32>,
}

impl<'ui> Window<'ui> {
    /// Creates a new window builder
    pub fn new(ui: &'ui Ui, name: impl Into<String>) -> Self {
        Self {
            ui,
            name: name.into(),
            flags: WindowFlags::empty(),
            size: None,
            size_condition: Condition::Always,
            pos: None,
            pos_condition: Condition::Always,
            content_size: None,
            content_size_condition: Condition::Always,
            collapsed: None,
            collapsed_condition: Condition::Always,
            focused: None,
            bg_alpha: None,
        }
    }

    /// Sets window flags
    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets window size
    pub fn size(mut self, size: [f32; 2], condition: Condition) -> Self {
        self.size = Some(size);
        self.size_condition = condition;
        self
    }

    /// Sets window position
    pub fn position(mut self, pos: [f32; 2], condition: Condition) -> Self {
        self.pos = Some(pos);
        self.pos_condition = condition;
        self
    }

    /// Sets window content size
    pub fn content_size(mut self, size: [f32; 2], condition: Condition) -> Self {
        self.content_size = Some(size);
        self.content_size_condition = condition;
        self
    }

    /// Sets window collapsed state
    pub fn collapsed(mut self, collapsed: bool, condition: Condition) -> Self {
        self.collapsed = Some(collapsed);
        self.collapsed_condition = condition;
        self
    }

    /// Sets window focused state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = Some(focused);
        self
    }

    /// Sets window background alpha
    pub fn bg_alpha(mut self, alpha: f32) -> Self {
        self.bg_alpha = Some(alpha);
        self
    }

    /// Builds the window and calls the provided closure
    pub fn build<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        let _token = self.begin()?;
        Some(f())
    }

    /// Begins the window and returns a token
    fn begin(self) -> Option<WindowToken<'ui>> {
        use std::ffi::CString;

        let name_cstr = CString::new(self.name).ok()?;

        // Set window properties before beginning
        if let Some(size) = self.size {
            unsafe {
                let size_vec = crate::sys::ImVec2 {
                    x: size[0],
                    y: size[1],
                };
                crate::sys::ImGui_SetNextWindowSize(&size_vec, self.size_condition as i32);
            }
        }

        if let Some(pos) = self.pos {
            unsafe {
                let pos_vec = crate::sys::ImVec2 {
                    x: pos[0],
                    y: pos[1],
                };
                let pivot_vec = crate::sys::ImVec2 { x: 0.0, y: 0.0 };
                crate::sys::ImGui_SetNextWindowPos(&pos_vec, self.pos_condition as i32, &pivot_vec);
            }
        }

        if let Some(content_size) = self.content_size {
            unsafe {
                let content_size_vec = crate::sys::ImVec2 {
                    x: content_size[0],
                    y: content_size[1],
                };
                crate::sys::ImGui_SetNextWindowContentSize(&content_size_vec);
            }
        }

        if let Some(collapsed) = self.collapsed {
            unsafe {
                crate::sys::ImGui_SetNextWindowCollapsed(
                    collapsed,
                    self.collapsed_condition as i32,
                );
            }
        }

        if let Some(focused) = self.focused {
            if focused {
                unsafe {
                    crate::sys::ImGui_SetNextWindowFocus();
                }
            }
        }

        if let Some(alpha) = self.bg_alpha {
            unsafe {
                crate::sys::ImGui_SetNextWindowBgAlpha(alpha);
            }
        }

        // Begin the window
        let mut open = true;
        let result =
            unsafe { crate::sys::ImGui_Begin(name_cstr.as_ptr(), &mut open, self.flags.bits()) };

        // IMPORTANT: According to ImGui documentation, Begin/End calls must be balanced.
        // If Begin returns false, we need to call End immediately and return None.
        if result && open {
            Some(WindowToken {
                _phantom: std::marker::PhantomData,
            })
        } else {
            // If Begin returns false, call End immediately and return None
            unsafe {
                crate::sys::ImGui_End();
            }
            None
        }
    }
}

/// Token representing an active window
pub struct WindowToken<'ui> {
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> Drop for WindowToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            crate::sys::ImGui_End();
        }
    }
}
