use crate::sys;

use super::list::DrawListMut;

/// A safe builder for registering a Rust callback to be executed during draw.
#[must_use = "call .build() to register the callback"]
pub struct Callback<'ui, F> {
    draw_list: &'ui DrawListMut<'ui>,
    callback: F,
}

impl<'ui, F: FnOnce() + 'static> Callback<'ui, F> {
    /// Construct a new callback builder. Typically created via `DrawListMut::add_callback_safe`.
    pub fn new(draw_list: &'ui DrawListMut<'_>, callback: F) -> Self {
        Self {
            draw_list,
            callback,
        }
    }

    /// Register the callback with the draw list.
    pub fn build(self) {
        use std::os::raw::c_void;
        // Box the closure so we can pass an owning pointer to C.
        //
        // Note: Dear ImGui's `ImDrawList::AddCallback()` optionally copies `userdata` bytes into an
        // internal unaligned byte buffer when `userdata_size != 0`. That mode is suitable only for
        // plain-old-data payloads; it must not be used for Rust closures.
        let ptr: *mut F = Box::into_raw(Box::new(self.callback));
        unsafe {
            sys::ImDrawList_AddCallback(
                self.draw_list.draw_list,
                Some(Self::run_callback),
                ptr as *mut c_void,
                0,
            );
        }
    }

    unsafe extern "C" fn run_callback(
        _parent_list: *const sys::ImDrawList,
        cmd: *const sys::ImDrawCmd,
    ) {
        if cmd.is_null() {
            return;
        }
        let cmd_ptr = cmd as *mut sys::ImDrawCmd;
        if unsafe { (*cmd_ptr).UserCallbackData.is_null() } {
            return;
        }
        if unsafe { (*cmd_ptr).UserCallbackDataOffset } != -1 {
            eprintln!("dear-imgui-rs: unexpected UserCallbackDataOffset (expected -1)");
            std::process::abort();
        }
        if unsafe { (*cmd_ptr).UserCallbackDataSize } != 0 {
            eprintln!("dear-imgui-rs: unexpected UserCallbackDataSize (expected 0)");
            std::process::abort();
        }
        // Compute pointer to our boxed closure (respect offset if ever used)
        let data_ptr = unsafe { (*cmd_ptr).UserCallbackData as *mut F };
        if data_ptr.is_null() {
            return;
        }
        // Take ownership and clear the pointer/size to avoid double-free or re-entry
        unsafe {
            (*cmd_ptr).UserCallbackData = std::ptr::null_mut();
            (*cmd_ptr).UserCallbackDataSize = 0;
            (*cmd_ptr).UserCallbackDataOffset = 0;
        }
        let cb = unsafe { Box::from_raw(data_ptr) };
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            cb();
        }));
        if res.is_err() {
            eprintln!("dear-imgui-rs: panic in DrawList callback");
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;
    use std::marker::PhantomData;

    #[test]
    fn safe_draw_callback_uses_direct_user_data_pointer() {
        fn noop() {}

        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        // Ensure CmdBuffer.Size > 0 (required by AddCallback).
        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
        };
        draw_list.add_callback_safe(noop).build();

        let cmd_buffer = unsafe { &(*draw_list.draw_list).CmdBuffer };
        assert!(cmd_buffer.Size > 0);
        assert!(!cmd_buffer.Data.is_null());

        let (cmd_ptr, cmd_copy) = {
            let cmds = unsafe {
                let len = usize::try_from(cmd_buffer.Size)
                    .expect("expected non-negative CmdBuffer.Size in test");
                std::slice::from_raw_parts(cmd_buffer.Data, len)
            };
            let (i, cmd) = cmds
                .iter()
                .enumerate()
                .find(|(_, cmd)| cmd.UserCallback.is_some() && !cmd.UserCallbackData.is_null())
                .expect("expected callback command to be present");

            let cmd_ptr = unsafe { cmd_buffer.Data.add(i) as *const sys::ImDrawCmd };
            (cmd_ptr, *cmd)
        };

        assert!(cmd_copy.UserCallback.is_some());
        assert_eq!(cmd_copy.UserCallbackDataOffset, -1);
        assert_eq!(cmd_copy.UserCallbackDataSize, 0);
        assert!(!cmd_copy.UserCallbackData.is_null());

        // Run the callback once to reclaim the boxed closure and avoid leaking in the test.
        unsafe { cmd_copy.UserCallback.unwrap()(draw_list.draw_list as *const _, cmd_ptr) }

        let cmd_after = unsafe { *cmd_ptr };
        assert!(cmd_after.UserCallbackData.is_null());

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn clone_output_rejects_user_callbacks() {
        fn noop() {}

        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
        };
        draw_list.add_callback_safe(noop).build();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = draw_list.clone_output();
        }));
        assert!(result.is_err());

        let cmd_buffer = unsafe { &(*draw_list.draw_list).CmdBuffer };
        let cmd_ptr = {
            let cmds = unsafe {
                let len = usize::try_from(cmd_buffer.Size)
                    .expect("expected non-negative CmdBuffer.Size in test");
                std::slice::from_raw_parts(cmd_buffer.Data, len)
            };
            let (i, _) = cmds
                .iter()
                .enumerate()
                .find(|(_, cmd)| cmd.UserCallback.is_some() && !cmd.UserCallbackData.is_null())
                .expect("expected callback command to be present");

            unsafe { cmd_buffer.Data.add(i) as *const sys::ImDrawCmd }
        };
        let cmd_copy = unsafe { *cmd_ptr };
        unsafe { cmd_copy.UserCallback.unwrap()(draw_list.draw_list as *const _, cmd_ptr) }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }
}
