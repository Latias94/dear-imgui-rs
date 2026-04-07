use super::{Context, EditorContext, ImNodesScope};
use crate::sys;
use dear_imgui_rs::Context as ImGuiContext;
use dear_imgui_rs::sys as imgui_sys;
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::ptr::NonNull;

/// An ImNodes editor context bound to a specific ImNodes context.
///
/// Prefer using this type over calling methods directly on `EditorContext` to avoid
/// accidentally operating on the wrong global ImNodes context.
pub struct BoundEditor<'a> {
    scope: ImNodesScope,
    _ctx: &'a Context,
    _editor: &'a EditorContext,
}

impl<'a> BoundEditor<'a> {
    #[inline]
    fn bind(&self) {
        self.scope.bind();
    }

    #[inline]
    pub fn set_current(&self) {
        self.bind();
    }

    pub fn get_panning(&self) -> [f32; 2] {
        self.bind();
        let out = unsafe { crate::compat_ffi::imnodes_EditorContextGetPanning() };
        [out.x, out.y]
    }

    pub fn reset_panning(&self, pos: [f32; 2]) {
        self.bind();
        unsafe {
            sys::imnodes_EditorContextResetPanning(sys::ImVec2_c {
                x: pos[0],
                y: pos[1],
            })
        };
    }

    pub fn move_to_node(&self, node_id: i32) {
        self.bind();
        unsafe { sys::imnodes_EditorContextMoveToNode(node_id) };
    }

    /// Save this editor's state to an INI string.
    pub fn save_state_to_ini_string(&self) -> String {
        self.bind();
        unsafe {
            let mut size: usize = 0;
            let ptr =
                sys::imnodes_SaveEditorStateToIniString(self._editor.raw, &mut size as *mut usize);
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

    /// Load this editor's state from an INI string.
    pub fn load_state_from_ini_string(&self, data: &str) {
        self.bind();
        unsafe {
            sys::imnodes_LoadEditorStateFromIniString(
                self._editor.raw,
                data.as_ptr() as *const c_char,
                data.len(),
            )
        }
    }

    /// Save this editor's state directly to an INI file.
    pub fn save_state_to_ini_file(&self, file_name: &str) {
        self.bind();
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
            sys::imnodes_SaveEditorStateToIniFile(self._editor.raw, ptr)
        })
    }

    /// Load this editor's state from an INI file.
    pub fn load_state_from_ini_file(&self, file_name: &str) {
        self.bind();
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
            sys::imnodes_LoadEditorStateFromIniFile(self._editor.raw, ptr)
        })
    }
}

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

    /// Bind an `EditorContext` to this ImNodes context.
    pub fn bind_editor<'a>(&'a self, editor: &'a EditorContext) -> BoundEditor<'a> {
        if let Some(bound) = editor.bound_ctx_raw {
            assert_eq!(
                bound, self.raw,
                "dear-imnodes: EditorContext is bound to a different ImNodes context"
            );
        }
        let scope = ImNodesScope {
            imgui_ctx_raw: self.imgui_ctx_raw,
            imgui_alive: self.imgui_alive.clone(),
            ctx_raw: self.raw,
            editor_raw: Some(editor.raw),
        };
        BoundEditor {
            scope,
            _ctx: self,
            _editor: editor,
        }
    }

    pub fn try_create_editor_context(&self) -> dear_imgui_rs::ImGuiResult<EditorContext> {
        assert!(
            self.imgui_alive.is_alive(),
            "dear-imnodes: ImGui context has been dropped"
        );
        unsafe {
            sys::imnodes_SetImGuiContext(self.imgui_ctx_raw);
            sys::imnodes_SetCurrentContext(self.raw);
        }
        let raw = unsafe { sys::imnodes_EditorContextCreate() };
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_EditorContextCreate returned null",
            ));
        }
        Ok(EditorContext {
            raw,
            bound_ctx_raw: Some(self.raw),
            bound_imgui_ctx_raw: Some(self.imgui_ctx_raw),
            bound_imgui_alive: Some(self.imgui_alive.clone()),
            _not_send_sync: PhantomData,
        })
    }

    pub fn create_editor_context(&self) -> EditorContext {
        self.try_create_editor_context()
            .expect("Failed to create ImNodes editor context")
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

impl Drop for EditorContext {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            if let Some(alive) = &self.bound_imgui_alive {
                if !alive.is_alive() {
                    // Avoid calling into ImGui allocators after the context has been dropped.
                    // Best-effort: leak the editor context instead of risking UB.
                    return;
                }
            }
            if let Some(imgui_ctx_raw) = self.bound_imgui_ctx_raw {
                unsafe { sys::imnodes_SetImGuiContext(imgui_ctx_raw) };
            }
            if let Some(ctx_raw) = self.bound_ctx_raw {
                unsafe { sys::imnodes_SetCurrentContext(ctx_raw) };
            }
            unsafe { sys::imnodes_EditorContextFree(self.raw) };
        }
    }
}

// EditorContext is also not thread-safe to move/share across threads.
