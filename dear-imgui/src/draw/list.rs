use std::cell::RefCell;

use crate::sys;

mod clip;
mod geometry;
mod image;
mod raw;
mod sampler;
mod split;
#[cfg(test)]
mod tests;
mod text;
mod texture;

pub use clip::DrawListClipRectToken;
pub use raw::RawDrawCallback;
pub use texture::DrawListTextureToken;

thread_local! {
    static BORROWED_DRAW_LISTS: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}

/// Object implementing the custom draw API.
///
/// Called from [`crate::Ui::get_window_draw_list`], [`crate::Ui::get_background_draw_list`] or [`crate::Ui::get_foreground_draw_list`].
/// Only one mutable wrapper can exist for the same raw draw list on the same thread at a time.
/// Reuse the existing wrapper, or drop it before requesting the same draw list again. The program
/// will panic when attempting to wrap the same draw list twice.
pub struct DrawListMut<'ui> {
    pub(super) draw_list: *mut sys::ImDrawList,
    pub(super) ui: Option<&'ui crate::Ui>,
    pub(super) provenance: DrawListProvenance,
}

#[derive(Clone, Copy)]
pub(super) enum DrawListProvenance {
    Frame,
    Window(crate::scope::WindowScope),
}

pub(super) struct ChannelsSplitMergeGuard<'ui> {
    pub(super) draw_list: &'ui DrawListMut<'ui>,
}

impl Drop for ChannelsSplitMergeGuard<'_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_ChannelsMerge(self.draw_list.draw_list) };
    }
}

impl Drop for DrawListMut<'_> {
    fn drop(&mut self) {
        let ptr = self.draw_list as usize;
        BORROWED_DRAW_LISTS.with(|borrowed| {
            let mut borrowed = borrowed.borrow_mut();
            if let Some(index) = borrowed.iter().position(|&value| value == ptr) {
                borrowed.swap_remove(index);
            }
        });
    }
}

impl<'ui> DrawListMut<'ui> {
    fn borrow_draw_list(draw_list: *mut sys::ImDrawList) {
        assert!(
            !draw_list.is_null(),
            "DrawListMut::borrow_draw_list() received a null draw list"
        );
        let ptr = draw_list as usize;
        BORROWED_DRAW_LISTS.with(|borrowed| {
            let mut borrowed = borrowed.borrow_mut();
            if borrowed.contains(&ptr) {
                panic!(
                    "A DrawListMut is already in use for this draw list; reuse the existing wrapper or drop it before acquiring another"
                );
            }
            borrowed.push(ptr);
        });
    }

    fn from_raw(
        ui: &'ui crate::Ui,
        draw_list: *mut sys::ImDrawList,
        provenance: DrawListProvenance,
    ) -> Self {
        Self::borrow_draw_list(draw_list);
        Self {
            draw_list,
            ui: Some(ui),
            provenance,
        }
    }

    /// Wrap a raw ImDrawList pointer for the current Dear ImGui frame.
    ///
    /// # Safety
    ///
    /// `draw_list` must be a valid mutable draw-list pointer owned by the active
    /// Dear ImGui frame and remain valid for `'ui`. The caller must also ensure
    /// the pointer is not independently mutated while the returned wrapper is
    /// alive. Raw wrappers are treated as frame-scoped; the caller must enforce any narrower
    /// window or viewport provenance required by the native draw list.
    pub unsafe fn from_raw_mut(ui: &'ui crate::Ui, draw_list: *mut sys::ImDrawList) -> Self {
        Self::from_raw(ui, draw_list, DrawListProvenance::Frame)
    }

    pub(crate) fn window(ui: &'ui crate::Ui) -> Self {
        ui.run_with_bound_context(|| unsafe {
            let scope = ui
                .current_native_scope()
                .window()
                .expect("Ui::get_window_draw_list() requires a current window");
            Self::from_raw(
                ui,
                sys::igGetWindowDrawList(),
                DrawListProvenance::Window(scope),
            )
        })
    }

    pub(crate) fn background(ui: &'ui crate::Ui) -> Self {
        ui.run_with_bound_context(|| {
            let viewport = unsafe { sys::igGetMainViewport() };
            Self::from_raw(
                ui,
                unsafe { sys::igGetBackgroundDrawList(viewport) },
                DrawListProvenance::Frame,
            )
        })
    }

    pub(crate) fn foreground(ui: &'ui crate::Ui) -> Self {
        ui.run_with_bound_context(|| {
            let viewport = unsafe { sys::igGetMainViewport() };
            Self::from_raw(
                ui,
                unsafe { sys::igGetForegroundDrawList_ViewportPtr(viewport) },
                DrawListProvenance::Frame,
            )
        })
    }

    pub(super) fn ui(&self) -> &crate::Ui {
        self.ui
            .expect("this draw-list operation requires a DrawListMut borrowed from Ui")
    }

    pub(super) fn assert_scope(&self, operation: &'static str) {
        let DrawListProvenance::Window(scope) = self.provenance else {
            return;
        };
        self.ui().run_with_bound_context(|| {
            assert!(
                self.ui().current_native_scope().window() == Some(scope),
                "{operation} requires the window Begin scope that created this DrawListMut"
            );
        });
    }

    pub(super) fn is_window_scoped(&self) -> bool {
        matches!(self.provenance, DrawListProvenance::Window(_))
    }
}
