use super::{
    Context, EditorContext, EditorContextInner, ImNodesContextAliveToken, ImNodesContextGuard,
    ImNodesScope, SavedBinding,
};
use crate::sys;
use dear_imgui_rs::Context as ImGuiContext;
use dear_imgui_rs::sys as imgui_sys;
use std::cell::Cell;
use std::os::raw::c_char;
use std::rc::Rc;

/// Access to an explicit editor's panning and saved state between submissions.
/// This handle retains the editor and restores the caller's bindings after each operation.
pub struct BoundEditor<'a> {
    scope: ImNodesScope<'a>,
}

impl<'a> BoundEditor<'a> {
    #[inline]
    fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.scope
            .ctx
            .assert_no_active_frame("BoundEditor operation");
        self.scope.with_bound_context(f)
    }

    fn editor_raw(&self) -> *mut sys::ImNodesEditorContext {
        self.scope
            .editor_lease
            .as_ref()
            .map(|editor| editor.raw)
            .expect("BoundEditor always owns an explicit EditorContext lease")
    }

    pub fn get_panning(&self) -> [f32; 2] {
        let out = self.with_bound_context(|| unsafe { sys::imnodes_EditorContextGetPanning() });
        [out.x, out.y]
    }

    pub fn reset_panning(&self, pos: [f32; 2]) {
        self.with_bound_context(|| unsafe {
            sys::imnodes_EditorContextResetPanning(sys::ImVec2_c {
                x: pos[0],
                y: pos[1],
            })
        });
    }

    /// Save this editor's state to an INI string.
    pub fn save_state_to_ini_string(&self) -> String {
        self.with_bound_context(|| unsafe {
            let mut size: usize = 0;
            let ptr =
                sys::imnodes_SaveEditorStateToIniString(self.editor_raw(), &mut size as *mut usize);
            if ptr.is_null() || size == 0 {
                return String::new();
            }
            let mut slice = std::slice::from_raw_parts(ptr as *const u8, size);
            if slice.last() == Some(&0) {
                slice = &slice[..slice.len().saturating_sub(1)];
            }
            String::from_utf8_lossy(slice).into_owned()
        })
    }

    /// Load this editor's state from an INI string.
    pub fn load_state_from_ini_string(&self, data: &str) {
        self.with_bound_context(|| unsafe {
            sys::imnodes_LoadEditorStateFromIniString(
                self.editor_raw(),
                data.as_ptr() as *const c_char,
                data.len(),
            )
        });
    }

    /// Save this editor's state directly to an INI file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_state_to_ini_file(&self, file_name: &str) {
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        self.with_bound_context(|| {
            dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
                sys::imnodes_SaveEditorStateToIniFile(self.editor_raw(), ptr)
            })
        })
    }

    /// Load this editor's state from an INI file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_state_from_ini_file(&self, file_name: &str) {
        let file_name = if file_name.contains('\0') {
            ""
        } else {
            file_name
        };
        self.with_bound_context(|| {
            dear_imgui_rs::with_scratch_txt(file_name, |ptr| unsafe {
                sys::imnodes_LoadEditorStateFromIniFile(self.editor_raw(), ptr)
            })
        })
    }
}

impl Context {
    /// Try to create a new ImNodes context bound to the current Dear ImGui context
    pub fn try_create(imgui: &ImGuiContext) -> dear_imgui_rs::ImGuiResult<Self> {
        let thread_lease = Rc::new(dear_imgui_rs::ContextThreadLease::acquire()?);
        let imgui_binding = imgui.binding();
        let raw = imgui_binding.with_bound_context(|| {
            let _nodes_guard = ImNodesContextGuard::bind(std::ptr::null_mut());
            unsafe { sys::imnodes_CreateContext() }
        });
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_CreateContext returned null",
            ));
        }
        Ok(Self {
            raw,
            imgui_binding,
            alive: Rc::new(()),
            frame_active: Cell::new(false),
            default_editor_submission: Default::default(),
            thread_lease,
        })
    }

    /// Create a new ImNodes context (panics on error)
    pub fn create(imgui: &ImGuiContext) -> Self {
        Self::try_create(imgui).expect("Failed to create ImNodes context")
    }

    /// Return the raw pointer for this context.
    pub fn as_raw(&self) -> *mut sys::ImNodesContext {
        self.raw
    }

    /// Return the raw Dear ImGui context pointer this ImNodes context is bound to.
    pub fn imgui_context_raw(&self) -> *mut imgui_sys::ImGuiContext {
        self.imgui_binding
            .with_bound_context(|| unsafe { imgui_sys::igGetCurrentContext() })
    }

    /// Returns the stable identity of the owning Dear ImGui context.
    pub fn imgui_context_id(&self) -> dear_imgui_rs::ContextId {
        self.imgui_binding.id()
    }

    /// Bind an `EditorContext` to this ImNodes context.
    pub fn bind_editor<'a>(&'a self, editor: &EditorContext) -> BoundEditor<'a> {
        let scope = self.scope(Some(editor));
        BoundEditor { scope }
    }

    pub fn try_create_editor_context(&self) -> dear_imgui_rs::ImGuiResult<EditorContext> {
        let raw = self.imgui_binding.with_bound_context(|| {
            let _nodes_guard = ImNodesContextGuard::bind(self.raw);
            unsafe { sys::imnodes_EditorContextCreate() }
        });
        if raw.is_null() {
            return Err(dear_imgui_rs::ImGuiError::context_creation(
                "imnodes_EditorContextCreate returned null",
            ));
        }
        Ok(EditorContext {
            inner: Rc::new(EditorContextInner {
                raw,
                bound_ctx_raw: self.raw,
                bound_ctx_alive: self.alive_token(),
                imgui_binding: self.imgui_binding.clone(),
                last_submission: Default::default(),
                _thread_lease: self.thread_lease.clone(),
            }),
        })
    }

    pub fn create_editor_context(&self) -> EditorContext {
        self.try_create_editor_context()
            .expect("Failed to create ImNodes editor context")
    }

    pub(crate) fn alive_token(&self) -> ImNodesContextAliveToken {
        ImNodesContextAliveToken(Rc::downgrade(&self.alive))
    }

    pub(crate) fn scope(&self, editor: Option<&EditorContext>) -> ImNodesScope<'_> {
        let editor_lease = editor.map(|editor| {
            let owner = self.alive_token();
            assert!(
                editor.inner.bound_ctx_alive.is_alive()
                    && editor.inner.bound_ctx_alive.same_context(&owner),
                "dear-imnodes: EditorContext is bound to a different or destroyed ImNodes context"
            );
            assert_eq!(
                editor.inner.bound_ctx_raw, self.raw,
                "dear-imnodes: EditorContext is bound to a different ImNodes context"
            );
            assert_eq!(
                editor.inner.imgui_binding.id(),
                self.imgui_binding.id(),
                "dear-imnodes: EditorContext is bound to a different ImGui context"
            );
            editor.inner.clone()
        });
        ImNodesScope {
            ctx: self,
            editor_lease,
        }
    }

    pub(crate) fn assert_no_active_frame(&self, caller: &str) {
        assert!(
            !self.frame_active.get(),
            "dear-imnodes: {caller} cannot run while an editor frame is active"
        );
    }
}

impl EditorContext {
    /// Returns the stable identity of the owning Dear ImGui context.
    pub fn imgui_context_id(&self) -> dear_imgui_rs::ContextId {
        self.inner.imgui_binding.id()
    }

    #[cfg(test)]
    pub(crate) fn raw(&self) -> *mut sys::ImNodesEditorContext {
        self.inner.raw
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            let raw = std::mem::replace(&mut self.raw, std::ptr::null_mut());
            SavedBinding::Context(raw).invalidate(SavedBinding::Context(std::ptr::null_mut()));
            if self
                .imgui_binding
                .try_with_bound_context(|| {
                    let _nodes_guard = ImNodesContextGuard::bind(raw);
                    unsafe { sys::imnodes_DestroyContext(raw) };
                })
                .is_err()
            {
                // ImNodes owns this allocation and can release it without switching the
                // no-longer-enterable Dear ImGui context.
                unsafe { sys::imnodes_DestroyContext(raw) };
            }
        }
    }
}

// ImNodes context interacts with Dear ImGui state and is not thread-safe.

impl Drop for EditorContextInner {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            let raw = std::mem::replace(&mut self.raw, std::ptr::null_mut());
            SavedBinding::Editor(raw).invalidate(SavedBinding::DefaultEditor);
            let free = || {
                if self.bound_ctx_alive.is_alive() {
                    let _nodes_guard = ImNodesContextGuard::bind(self.bound_ctx_raw);
                    unsafe { sys::imnodes_EditorContextResetToDefaultIfCurrent(raw) };
                }
                unsafe { sys::imnodes_EditorContextFree(raw) };
            };
            if self.imgui_binding.try_with_bound_context(free).is_err() {
                // The ImNodes editor slot and allocation outlive Dear ImGui teardown.
                free();
            }
        }
    }
}

// EditorContext is also not thread-safe to move/share across threads.
