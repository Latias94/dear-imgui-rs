use crate::sys;
use std::marker::PhantomData;
use std::rc::Rc;

/// Token for managing style variable changes.
#[must_use]
pub struct StyleVarToken {
    pub(super) was_popped: bool,
    pub(super) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleVarToken {
    /// Pop this style variable from the stack.
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D style var token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot3D_PopStyleVar(1);
        }
    }
}

impl Drop for StyleVarToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot3D_PopStyleVar(1);
            }
        }
    }
}

/// Token for managing style color changes.
#[must_use]
pub struct StyleColorToken {
    pub(super) was_popped: bool,
    pub(super) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleColorToken {
    /// Pop this style color from the stack.
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D style color token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot3D_PopStyleColor(1);
        }
    }
}

impl Drop for StyleColorToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot3D_PopStyleColor(1);
            }
        }
    }
}

/// Token for managing colormap changes.
#[must_use]
pub struct ColormapToken {
    pub(super) was_popped: bool,
    pub(super) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ColormapToken {
    /// Pop this colormap from the stack.
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D colormap token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot3D_PopColormap(1);
        }
    }
}

impl Drop for ColormapToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot3D_PopColormap(1);
            }
        }
    }
}
