use crate::sys;
use crate::{Ui, WindowFlags};
use std::ffi::CString;

/// Represents a child window that can be built
pub struct ChildWindow<'ui> {
    name: String,
    size: [f32; 2],
    border: bool,
    flags: WindowFlags,
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> ChildWindow<'ui> {
    /// Creates a new child window builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            size: [0.0, 0.0],
            border: false,
            flags: WindowFlags::empty(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Sets the size of the child window
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the child window has a border
    pub fn border(mut self, border: bool) -> Self {
        self.border = border;
        self
    }

    /// Sets window flags for the child window
    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the child window and calls the provided closure
    pub fn build<F, R>(self, ui: &'ui Ui, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        let token = self.begin(ui)?;
        let result = f();
        drop(token); // Explicitly drop the token to call EndChild
        Some(result)
    }

    /// Begins the child window and returns a token
    fn begin(self, _ui: &'ui Ui) -> Option<ChildWindowToken<'ui>> {
        let name_cstr = CString::new(self.name).ok()?;

        let result = unsafe {
            let size_vec = sys::ImVec2 {
                x: self.size[0],
                y: self.size[1],
            };
            sys::ImGui_BeginChild(
                name_cstr.as_ptr(),
                &size_vec,
                self.border as i32,
                self.flags.bits() as i32,
            )
        };

        if result {
            Some(ChildWindowToken {
                _phantom: std::marker::PhantomData,
            })
        } else {
            None
        }
    }
}

/// Token representing an active child window
pub struct ChildWindowToken<'ui> {
    _phantom: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> Drop for ChildWindowToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_EndChild();
        }
    }
}

impl<'ui> Ui {
    /// Creates a child window builder
    pub fn child_window(&self, name: impl Into<String>) -> ChildWindow<'_> {
        ChildWindow::new(name)
    }
}
