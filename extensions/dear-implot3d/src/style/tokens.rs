use crate::Plot3DUi;
use crate::sys;
use crate::ui::Plot3DContextBinding;
use std::marker::PhantomData;
use std::rc::Rc;

/// Token for managing style variable changes.
#[must_use]
pub struct StyleVarToken<'ui> {
    pub(super) binding: Plot3DContextBinding,
    pub(super) was_popped: bool,
    pub(super) _lifetime: PhantomData<&'ui Plot3DUi<'ui>>,
    pub(super) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleVarToken<'_> {
    /// Pop this style variable from the stack.
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D style var token twice.");
        }
        self.binding.with_bound_context(|| {
            unsafe { sys::ImPlot3D_PopStyleVar(1) };
        });
        self.was_popped = true;
    }
}

impl Drop for StyleVarToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            let _ = self
                .binding
                .try_with_bound_context(|| unsafe { sys::ImPlot3D_PopStyleVar(1) });
            self.was_popped = true;
        }
    }
}

/// Token for managing style color changes.
#[must_use]
pub struct StyleColorToken<'ui> {
    pub(super) binding: Plot3DContextBinding,
    pub(super) was_popped: bool,
    pub(super) _lifetime: PhantomData<&'ui Plot3DUi<'ui>>,
    pub(super) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl StyleColorToken<'_> {
    /// Pop this style color from the stack.
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D style color token twice.");
        }
        self.binding.with_bound_context(|| {
            unsafe { sys::ImPlot3D_PopStyleColor(1) };
        });
        self.was_popped = true;
    }
}

impl Drop for StyleColorToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            let _ = self
                .binding
                .try_with_bound_context(|| unsafe { sys::ImPlot3D_PopStyleColor(1) });
            self.was_popped = true;
        }
    }
}

/// Token for managing colormap changes.
#[must_use]
pub struct ColormapToken<'ui> {
    pub(super) binding: Plot3DContextBinding,
    pub(super) was_popped: bool,
    pub(super) _lifetime: PhantomData<&'ui Plot3DUi<'ui>>,
    pub(super) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ColormapToken<'_> {
    /// Pop this colormap from the stack.
    pub fn pop(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot3D colormap token twice.");
        }
        self.binding.with_bound_context(|| {
            unsafe { sys::ImPlot3D_PopColormap(1) };
        });
        self.was_popped = true;
    }
}

impl Drop for ColormapToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            let _ = self
                .binding
                .try_with_bound_context(|| unsafe { sys::ImPlot3D_PopColormap(1) });
            self.was_popped = true;
        }
    }
}
