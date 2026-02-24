use super::super::{MiniMapCallbackHolder, NodeEditor};
use crate::sys;
use std::os::raw::c_void;

impl<'ui> NodeEditor<'ui> {
    /// Draw a minimap in the editor
    pub fn minimap(&self, size_fraction: f32, location: crate::MiniMapLocation) {
        self.bind();
        unsafe {
            sys::imnodes_MiniMap(
                size_fraction,
                location as sys::ImNodesMiniMapLocation,
                None,
                std::ptr::null_mut(),
            )
        }
    }

    /// Draw a minimap with a node-hover callback (invoked during this call)
    pub fn minimap_with_callback<F>(
        &mut self,
        size_fraction: f32,
        location: crate::MiniMapLocation,
        callback: F,
    ) where
        F: FnMut(i32) + 'ui,
    {
        unsafe extern "C" fn trampoline(node_id: i32, user: *mut c_void) {
            if user.is_null() {
                return;
            }
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                let holder = &mut *(user as *mut MiniMapCallbackHolder<'_>);
                (holder.callback)(node_id);
            }));
            if res.is_err() {
                eprintln!("dear-imnodes: panic in minimap callback");
                std::process::abort();
            }
        }

        // ImNodes may invoke the callback during EndNodeEditor(). Keep the closure alive for the
        // whole editor frame by storing it inside the NodeEditor token.
        self.bind();
        self.minimap_callbacks.push(Box::new(MiniMapCallbackHolder {
            callback: Box::new(callback),
        }));
        let user_ptr = self
            .minimap_callbacks
            .last_mut()
            .map(|b| b.as_mut() as *mut MiniMapCallbackHolder<'ui> as *mut c_void)
            .unwrap_or(std::ptr::null_mut());
        unsafe {
            sys::imnodes_MiniMap(
                size_fraction,
                location as sys::ImNodesMiniMapLocation,
                Some(trampoline),
                user_ptr,
            )
        }
    }
}
