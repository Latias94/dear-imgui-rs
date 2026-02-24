use super::{Context, EditorContext};
use crate::sys;
use dear_imgui_rs::Context as ImGuiContext;
use dear_imgui_rs::sys as imgui_sys;
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr::NonNull;

impl Context {
    /// Try to create a new ImNodes context bound to the current Dear ImGui context
    pub fn try_create(imgui: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        let imgui_ctx_raw = imgui.as_raw();
        let imgui_alive = imgui.alive_token();
        unsafe { sys::imnodes_SetImGuiContext(imgui_ctx_raw) };
        let raw = unsafe { sys::imnodes_CreateContext() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_CreateContext returned null",
            ));
        }
        unsafe { sys::imnodes_SetCurrentContext(raw) };
        Ok(Self {
            raw,
            imgui_ctx_raw,
            imgui_alive,
            _not_send_sync: PhantomData,
        })
    }

    /// Create a new ImNodes context (panics on error)
    pub fn create(imgui: &ImGuiContext) -> Self {
        Self::try_create(imgui).expect("Failed to create ImNodes context")
    }

    /// Set as current ImNodes context
    pub fn set_as_current(&self) {
        assert!(
            self.imgui_alive.is_alive(),
            "dear-imnodes: ImGui context has been dropped"
        );
        unsafe { sys::imnodes_SetImGuiContext(self.imgui_ctx_raw) };
        unsafe { sys::imnodes_SetCurrentContext(self.raw) };
    }

    /// Return the raw pointer for this context.
    pub fn as_raw(&self) -> *mut sys::ImNodesContext {
        self.raw
    }

    /// Return the raw Dear ImGui context pointer this ImNodes context is bound to.
    pub fn imgui_context_raw(&self) -> *mut imgui_sys::ImGuiContext {
        self.imgui_ctx_raw
    }

    /// Get the currently active ImNodes context.
    pub fn current_raw() -> Option<NonNull<sys::ImNodesContext>> {
        let ptr = unsafe { sys::imnodes_GetCurrentContext() };
        NonNull::new(ptr)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            if self.imgui_alive.is_alive() {
                unsafe {
                    sys::imnodes_SetImGuiContext(self.imgui_ctx_raw);
                    if sys::imnodes_GetCurrentContext() == self.raw {
                        sys::imnodes_SetCurrentContext(std::ptr::null_mut());
                    }
                }
            } else {
                // Avoid calling `SetImGuiContext` with a dangling pointer.
                // Best-effort cleanup: destroy the ImNodes context without rebinding ImGui.
                unsafe {
                    if sys::imnodes_GetCurrentContext() == self.raw {
                        sys::imnodes_SetCurrentContext(std::ptr::null_mut());
                    }
                }
            }
            unsafe { sys::imnodes_DestroyContext(self.raw) };
        }
    }
}

// ImNodes context interacts with Dear ImGui state and is not thread-safe.

impl EditorContext {
    #[inline]
    fn bind_current(&self) {
        let imgui_ctx_raw = unsafe { imgui_sys::igGetCurrentContext() };
        assert!(
            !imgui_ctx_raw.is_null(),
            "dear-imnodes: EditorContext methods require an active ImGui context"
        );
        unsafe { sys::imnodes_SetImGuiContext(imgui_ctx_raw) };
        assert!(
            !unsafe { sys::imnodes_GetCurrentContext() }.is_null(),
            "dear-imnodes: EditorContext methods require an active ImNodes context (call Context::set_as_current or use NodesUi)"
        );
        unsafe { sys::imnodes_EditorContextSet(self.raw) };
    }

    pub fn try_create() -> dear_imgui_rs::ImGuiResult<Self> {
        let raw = unsafe { sys::imnodes_EditorContextCreate() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_EditorContextCreate returned null",
            ));
        }
        Ok(Self {
            raw,
            _not_send_sync: PhantomData,
        })
    }

    pub fn create() -> Self {
        Self::try_create().expect("Failed to create ImNodes editor context")
    }

    pub fn set_current(&self) {
        self.bind_current();
    }

    pub fn get_panning(&self) -> [f32; 2] {
        self.bind_current();
        let out = unsafe { crate::compat_ffi::imnodes_EditorContextGetPanning() };
        [out.x, out.y]
    }

    pub fn reset_panning(&self, pos: [f32; 2]) {
        self.bind_current();
        unsafe {
            sys::imnodes_EditorContextResetPanning(sys::ImVec2_c {
                x: pos[0],
                y: pos[1],
            })
        };
    }

    pub fn move_to_node(&self, node_id: i32) {
        self.bind_current();
        unsafe { sys::imnodes_EditorContextMoveToNode(node_id) };
    }

    /// Save this editor's state to an INI string
    pub fn save_state_to_ini_string(&self) -> String {
        self.bind_current();
        unsafe {
            let mut size: usize = 0;
            let ptr = sys::imnodes_SaveEditorStateToIniString(self.raw, &mut size as *mut usize);
            if ptr.is_null() || size == 0 {
                return String::new();
            }
            let mut slice = std::slice::from_raw_parts(ptr as *const u8, size);
            if slice.last() == Some(&0) {
                slice = &slice[..slice.len().saturating_sub(1)];
            }
            String::from_utf8_lossy(slice).into_owned()
        }
    }

    /// Load this editor's state from an INI string
    pub fn load_state_from_ini_string(&self, data: &str) {
        self.bind_current();
        unsafe {
            sys::imnodes_LoadEditorStateFromIniString(
                self.raw,
                data.as_ptr() as *const c_char,
                data.len(),
            )
        }
    }

    /// Save this editor's state directly to an INI file
    pub fn save_state_to_ini_file(&self, file_name: &str) {
        self.bind_current();
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
            sys::imnodes_SaveEditorStateToIniFile(self.raw, ptr)
        })
    }

    /// Load this editor's state from an INI file
    pub fn load_state_from_ini_file(&self, file_name: &str) {
        self.bind_current();
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
            sys::imnodes_LoadEditorStateFromIniFile(self.raw, ptr)
        })
    }
}

impl Drop for EditorContext {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sys::imnodes_EditorContextFree(self.raw) };
        }
    }
}

// EditorContext is also not thread-safe to move/share across threads.
