use crate::Plot3DUi;
use crate::sys;
use crate::ui::Plot3DContextBinding;
use dear_imgui_rs::ContextAliveToken;
use std::marker::PhantomData;
use std::rc::Rc;

/// Token for managing style variable changes.
#[must_use]
pub struct StyleVarToken<'ui> {
    pub(super) binding: Plot3DContextBinding,
    pub(super) imgui_alive: Option<ContextAliveToken>,
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
        assert_imgui_alive(&self.imgui_alive, "dear-implot3d: StyleVarToken");
        let _guard = self.binding.bind();
        unsafe { sys::ImPlot3D_PopStyleVar(1) };
        self.was_popped = true;
    }
}

impl Drop for StyleVarToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            self.pop_inner();
        }
    }
}

/// Token for managing style color changes.
#[must_use]
pub struct StyleColorToken<'ui> {
    pub(super) binding: Plot3DContextBinding,
    pub(super) imgui_alive: Option<ContextAliveToken>,
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
        assert_imgui_alive(&self.imgui_alive, "dear-implot3d: StyleColorToken");
        let _guard = self.binding.bind();
        unsafe { sys::ImPlot3D_PopStyleColor(1) };
        self.was_popped = true;
    }
}

impl Drop for StyleColorToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            self.pop_inner();
        }
    }
}

/// Token for managing colormap changes.
#[must_use]
pub struct ColormapToken<'ui> {
    pub(super) binding: Plot3DContextBinding,
    pub(super) imgui_alive: Option<ContextAliveToken>,
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
        assert_imgui_alive(&self.imgui_alive, "dear-implot3d: ColormapToken");
        let _guard = self.binding.bind();
        unsafe { sys::ImPlot3D_PopColormap(1) };
        self.was_popped = true;
    }
}

impl Drop for ColormapToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            self.pop_inner();
        }
    }
}

fn assert_imgui_alive(alive: &Option<ContextAliveToken>, caller: &str) {
    if let Some(alive) = alive {
        assert!(alive.is_alive(), "{caller}: ImGui context has been dropped");
    }
}
