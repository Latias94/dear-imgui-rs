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
    ) -> Self {
        Ui {
            ctx,
            ctx_binding,
            texture_registry,
            buffer: UnsafeCell::new(UiBuffer::new(1024)),
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

    pub(crate) fn run_with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.ctx_binding.with_bound_context(f)
    }

    pub(crate) fn resolve_texture_ref(
        &self,
        texture: crate::texture::TextureRef<'_>,
    ) -> Result<sys::ImTextureRef, crate::texture::ManagedTextureError> {
        match texture.source() {
            crate::texture::TextureSource::Legacy(id) => Ok(sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: id.id() as sys::ImTextureID,
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

    /// Resolve a logical texture for an adjacent extension FFI call.
    ///
    /// # Safety
    ///
    /// A returned managed pointer is valid only for an immediate native call while this `Ui` and
    /// its frame remain borrowed. It must not be stored, sent, or used after the call returns.
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
        self.run_with_bound_context(f)
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

    /// Internal method to push a single text to our scratch buffer.
    pub(crate) fn scratch_txt(&self, txt: impl AsRef<str>) -> *const std::os::raw::c_char {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt(txt)
        }
    }

    /// Stages an explicit text range with a readable NUL sentinel at its end.
    pub(crate) fn scratch_txt_range(
        &self,
        txt: impl AsRef<str>,
    ) -> std::ops::Range<*const std::os::raw::c_char> {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt_range(txt)
        }
    }

    /// Helper method for two strings
    pub(crate) fn scratch_txt_two(
        &self,
        txt_0: impl AsRef<str>,
        txt_1: impl AsRef<str>,
    ) -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt_two(txt_0, txt_1)
        }
    }

    /// Helper method with one optional value
    pub(crate) fn scratch_txt_with_opt(
        &self,
        txt_0: impl AsRef<str>,
        txt_1: Option<impl AsRef<str>>,
    ) -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
        unsafe {
            let handle = &mut *self.buffer.get();
            handle.scratch_txt_with_opt(txt_0, txt_1)
        }
    }

    /// Get access to the scratch buffer for complex string operations
    pub(crate) fn scratch_buffer(&self) -> &UnsafeCell<UiBuffer> {
        &self.buffer
    }

    /// Returns an ID from a string label in the current ID scope.
    ///
    /// This mirrors `ImGui::GetID(label)`. Useful for building stable IDs
    /// for widgets or dockspaces inside the current window/scope.
    #[doc(alias = "GetID")]
    pub fn get_id(&self, label: &str) -> Id {
        let label = self.scratch_txt(label);
        self.run_with_bound_context(|| unsafe { Id::from(sys::igGetID_Str(label)) })
    }
}
