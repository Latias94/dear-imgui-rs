use std::cell::RefCell;
use std::marker::PhantomData;

use crate::sys;

mod clip;
mod geometry;
mod image;
mod raw;
mod split;
#[cfg(test)]
mod tests;
mod text;
mod texture;

pub use clip::DrawListClipRectToken;
pub use texture::DrawListTextureToken;

thread_local! {
    static BORROWED_DRAW_LISTS: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}

/// Object implementing the custom draw API.
///
/// Called from [`Ui::get_window_draw_list`], [`Ui::get_background_draw_list`] or [`Ui::get_foreground_draw_list`].
/// Only one mutable wrapper can exist for the same raw draw list on the same thread at a time.
/// The program will panic when attempting to wrap the same draw list twice.
pub struct DrawListMut<'ui> {
    pub(super) draw_list: *mut sys::ImDrawList,
    pub(super) _phantom: PhantomData<&'ui crate::Ui>,
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
                panic!("A DrawListMut is already in use for this draw list");
            }
            borrowed.push(ptr);
        });
    }

    fn from_raw(draw_list: *mut sys::ImDrawList) -> Self {
        Self::borrow_draw_list(draw_list);
        Self {
            draw_list,
            _phantom: PhantomData,
        }
    }

    /// Wrap a raw ImDrawList pointer for the current Dear ImGui frame.
    ///
    /// # Safety
    ///
    /// `draw_list` must be a valid mutable draw-list pointer owned by the active
    /// Dear ImGui frame and remain valid for `'ui`. The caller must also ensure
    /// the pointer is not independently mutated while the returned wrapper is
    /// alive.
    pub unsafe fn from_raw_mut(_ui: &'ui crate::Ui, draw_list: *mut sys::ImDrawList) -> Self {
        Self::from_raw(draw_list)
    }

    pub(crate) fn window(_ui: &'ui crate::Ui) -> Self {
        Self::from_raw(unsafe { sys::igGetWindowDrawList() })
    }

    pub(crate) fn background(_ui: &'ui crate::Ui) -> Self {
        let viewport = unsafe { sys::igGetMainViewport() };
        Self::from_raw(unsafe { sys::igGetBackgroundDrawList(viewport) })
    }

    pub(crate) fn foreground(_ui: &'ui crate::Ui) -> Self {
        let viewport = unsafe { sys::igGetMainViewport() };
        Self::from_raw(unsafe { sys::igGetForegroundDrawList_ViewportPtr(viewport) })
    }
}
