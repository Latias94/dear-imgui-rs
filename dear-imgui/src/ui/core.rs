use super::*;

impl Ui {
    pub(crate) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
        assert!(value.is_finite(), "{caller} {name} must be finite");
    }

    pub(super) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
        assert!(
            value[0].is_finite() && value[1].is_finite(),
            "{caller} {name} must contain finite values"
        );
    }

    /// Creates a new Ui instance
    ///
    /// This should only be called by Context::create()
    pub(crate) fn new(
        ctx: *mut sys::ImGuiContext,
        ctx_binding: crate::ContextBinding,
        texture_registry: crate::context::SharedTextureRegistry,
        ime: std::rc::Rc<std::cell::RefCell<crate::context::ime::ImeState>>,
    ) -> Self {
        Ui {
            ctx,
            native_current: unsafe { sys::current_context_slot() },
            ctx_binding,
            texture_registry,
            ime,
            native_scopes: std::cell::RefCell::new(crate::scope::NativeScopeTracker::default()),
            scratch_buffers: ScratchBuffers::default(),
        }
    }

    pub(crate) fn context_raw(&self) -> *mut sys::ImGuiContext {
        self.ctx
    }

    /// Returns a persistent capability for the Context that owns this `Ui`.
    pub fn binding(&self) -> crate::ContextBinding {
        self.ctx_binding.clone()
    }

    /// Returns the process-unique identity of the Context that owns this `Ui`.
    pub fn context_id(&self) -> crate::ContextId {
        self.ctx_binding.id()
    }

    #[inline]
    pub(crate) fn run_with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.assert_native_scopes_usable();
        self.with_live_context(f)
    }

    /// Enter the context owned by this live frame borrow, including during stack cleanup.
    #[inline]
    pub(crate) fn with_live_context<R>(&self, f: impl FnOnce() -> R) -> R {
        let current = unsafe {
            if self.native_current.is_null() {
                sys::igGetCurrentContext()
            } else {
                self.native_current.read()
            }
        };
        if current == self.ctx {
            f()
        } else {
            crate::context::binding::with_bound_context(self.ctx, f)
        }
    }

    pub(crate) fn resolve_texture_ref(
        &self,
        texture: crate::texture::TextureRef<'_>,
    ) -> Result<sys::ImTextureRef, crate::texture::ManagedTextureError> {
        match texture.source() {
            crate::texture::TextureSource::Legacy(id) => Ok(sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: sys::ImTextureID::from(id),
            }),
            crate::texture::TextureSource::Managed(id) => {
                self.texture_registry.borrow().resolve(id)
            }
            crate::texture::TextureSource::FontAtlas { atlas, texture } => {
                let io = unsafe { sys::igGetIO_ContextPtr(self.ctx) };
                if io.is_null() || !std::ptr::eq(unsafe { (*io).Fonts }, atlas) {
                    return Err(crate::texture::ManagedTextureError::ForeignFontAtlas);
                }
                Ok(texture)
            }
        }
    }

    /// Resolve a logical texture for submission to this frame's native draw lists.
    ///
    /// # Safety
    ///
    /// Use the reference on the owning context's thread. An extension may queue it until the
    /// end of its drawing scope to emit commands into this same ImGui frame. The context keeps
    /// managed and atlas textures alive for the resulting draw data. Limit the extension's own
    /// copy to this submission and consume the resulting frame through its renderer API.
    /// For a legacy texture ID, the caller supplies the renderer binding and its lifetime.
    #[doc(hidden)]
    pub unsafe fn resolve_texture_ref_raw(
        &self,
        texture: crate::texture::TextureRef<'_>,
    ) -> Result<sys::ImTextureRef, crate::texture::ManagedTextureError> {
        self.run_with_bound_context(|| self.resolve_texture_ref(texture))
    }

    /// Runs a closure while this `Ui`'s owning ImGui context is current.
    ///
    /// The previously current context is restored before this method returns,
    /// including when the closure panics. This is primarily intended for
    /// extension crates that need to call raw Dear ImGui-adjacent FFI while
    /// still honoring the `Ui` that created the safe wrapper.
    pub fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.assert_native_scopes_usable();
        self.ctx_binding.with_bound_context(f)
    }

    /// Returns an immutable reference to the inputs/outputs object
    #[doc(alias = "GetIO")]
    pub fn io(&self) -> &crate::io::Io {
        self.run_with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            if io.is_null() {
                panic!("Ui::io() requires an active ImGui context");
            }
            &*(io as *const crate::io::Io)
        })
    }

    /// Keeps the latest scratch allocation alive throughout `f`. Reentrant calls use another
    /// buffer. On return or unwind this buffer becomes the next one available for reuse.
    pub(crate) fn with_scratch_buffer<R>(&self, f: impl FnOnce(&mut UiBuffer) -> R) -> R {
        self.scratch_buffers.with(f)
    }

    pub(crate) fn with_scratch_c_string<R>(
        &self,
        text: &str,
        f: impl FnOnce(*const std::ffi::c_char) -> R,
    ) -> R {
        self.scratch_buffers.with_c_string(text, f)
    }

    /// Keeps the staged C string alive while its native consumer runs.
    pub(crate) fn with_scratch_txt<R>(
        &self,
        text: impl AsRef<str>,
        f: impl FnOnce(*const std::ffi::c_char) -> R,
    ) -> R {
        let text = text.as_ref();
        self.run_with_bound_context(|| self.with_scratch_c_string(text, f))
    }

    pub(crate) fn with_scratch_txt_two<R>(
        &self,
        first: impl AsRef<str>,
        second: impl AsRef<str>,
        f: impl FnOnce(*const std::ffi::c_char, *const std::ffi::c_char) -> R,
    ) -> R {
        self.with_scratch_txt_with_opt(first, Some(second), f)
    }

    pub(crate) fn with_scratch_txt_with_opt<R>(
        &self,
        first: impl AsRef<str>,
        second: Option<impl AsRef<str>>,
        f: impl FnOnce(*const std::ffi::c_char, *const std::ffi::c_char) -> R,
    ) -> R {
        let first = first.as_ref();
        let second = second.as_ref().map(AsRef::as_ref);
        self.run_with_bound_context(|| {
            self.with_scratch_buffer(|buf| {
                let (first, second) = buf.scratch_txt_with_opt(first, second);
                f(first, second)
            })
        })
    }

    pub(crate) fn with_scratch_txt_opt<R>(
        &self,
        text: Option<&str>,
        f: impl FnOnce(*const std::ffi::c_char) -> R,
    ) -> R {
        match text {
            Some(text) => self.with_scratch_txt(text, f),
            None => self.run_with_bound_context(|| f(std::ptr::null())),
        }
    }

    /// Returns an ID from a string label in the current ID scope.
    ///
    /// This mirrors `ImGui::GetID(label)`. Useful for building stable IDs
    /// for widgets or dockspaces inside the current window/scope.
    #[doc(alias = "GetID")]
    pub fn get_id(&self, label: &str) -> Id {
        let start = if label.is_empty() {
            c"".as_ptr()
        } else {
            label.as_ptr().cast()
        };
        self.run_with_bound_context(|| unsafe {
            Id::from(sys::igGetID_StrStr(start, start.add(label.len())))
        })
    }
}
