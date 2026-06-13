use crate::fonts::{Font, FontAtlas, FontAtlasRef, SharedFontAtlas};
use crate::sys;
use std::marker::PhantomData;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

/// Tracks a font pushed through [`Context::push_font`].
///
/// The font stack entry is popped when the token is dropped or when
/// [`Self::pop`] is called.
#[must_use]
pub struct ContextFontStackToken<'ctx> {
    ctx: *mut sys::ImGuiContext,
    _phantom: PhantomData<&'ctx mut Context>,
}

impl<'ctx> ContextFontStackToken<'ctx> {
    fn new(ctx: *mut sys::ImGuiContext) -> Self {
        Self {
            ctx,
            _phantom: PhantomData,
        }
    }

    /// Pops the font stack entry immediately.
    #[doc(alias = "PopFont")]
    pub fn pop(self) {}

    /// Pops the font stack entry immediately.
    #[doc(alias = "PopFont")]
    pub fn end(self) {}
}

impl Drop for ContextFontStackToken<'_> {
    fn drop(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.ctx, || {
                sys::igPopFont();
            });
        }
    }
}

impl Context {
    /// Push a font onto the font stack.
    ///
    /// Returns a token that pops the font stack when dropped.
    #[doc(alias = "PushFont")]
    pub fn push_font(&mut self, font: &Font) -> ContextFontStackToken<'_> {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let font_ptr =
                    crate::fonts::validate_font_for_current_context(font, "Context::push_font()");
                sys::igPushFont(font_ptr, 0.0);
            });
        }
        ContextFontStackToken::new(self.raw)
    }

    /// Get the current font
    #[doc(alias = "GetFont")]
    pub fn current_font(&self) -> &Font {
        let _guard = CTX_MUTEX.lock();
        unsafe { with_bound_context(self.raw, || Font::from_raw(sys::igGetFont() as *const _)) }
    }

    /// Get the current font size
    #[doc(alias = "GetFontSize")]
    pub fn current_font_size(&self) -> f32 {
        let _guard = CTX_MUTEX.lock();
        unsafe { with_bound_context(self.raw, || sys::igGetFontSize()) }
    }

    /// Get a read-only view of the font atlas from the IO structure.
    ///
    /// Use [`Context::font_atlas_mut`] or [`Context::fonts`] for loading fonts
    /// or mutating atlas state.
    pub fn font_atlas(&self) -> FontAtlasRef<'_> {
        let _guard = CTX_MUTEX.lock();

        // wasm32 import-style builds keep Dear ImGui state in a separate module
        // and share linear memory. When the experimental font-atlas feature is
        // enabled, we allow direct access to the atlas pointer, assuming the
        // provider has been correctly configured via xtask.
        #[cfg(all(target_arch = "wasm32", feature = "wasm-font-atlas-experimental"))]
        unsafe {
            let io = self.io_ptr("Context::font_atlas()");
            let atlas_ptr = (*io).Fonts;
            assert!(
                !atlas_ptr.is_null(),
                "ImGui IO Fonts pointer is null on wasm; provider not initialized?"
            );
            FontAtlasRef::from_raw(atlas_ptr)
        }

        // Default wasm path: keep this API disabled to avoid accidental UB.
        #[cfg(all(target_arch = "wasm32", not(feature = "wasm-font-atlas-experimental")))]
        {
            panic!(
                "font_atlas() is not supported on wasm32 targets without \
                 `wasm-font-atlas-experimental` feature; \
                 see docs/WASM.md for current limitations."
            );
        }

        #[cfg(not(target_arch = "wasm32"))]
        unsafe {
            let io = self.io_ptr("Context::font_atlas()");
            let atlas_ptr = (*io).Fonts;
            FontAtlasRef::from_raw(atlas_ptr)
        }
    }

    /// Get a mutable reference to the font atlas from the IO structure
    pub fn font_atlas_mut(&mut self) -> FontAtlas {
        let _guard = CTX_MUTEX.lock();

        // wasm32 import-style builds keep Dear ImGui state in a separate module
        // and share linear memory. When the experimental font-atlas feature is
        // enabled, we allow direct access to the atlas pointer, assuming the
        // provider has been correctly configured via xtask.
        #[cfg(all(target_arch = "wasm32", feature = "wasm-font-atlas-experimental"))]
        unsafe {
            let io = self.io_ptr("Context::font_atlas_mut()");
            let atlas_ptr = (*io).Fonts;
            assert!(
                !atlas_ptr.is_null(),
                "ImGui IO Fonts pointer is null on wasm; provider not initialized?"
            );
            return FontAtlas::from_raw(atlas_ptr);
        }

        // Default wasm path: keep this API disabled to avoid accidental UB.
        #[cfg(all(target_arch = "wasm32", not(feature = "wasm-font-atlas-experimental")))]
        {
            panic!(
                "font_atlas_mut()/fonts() are not supported on wasm32 targets yet; \
                 enable `wasm-font-atlas-experimental` to opt-in for experiments."
            );
        }

        #[cfg(not(target_arch = "wasm32"))]
        unsafe {
            let io = self.io_ptr("Context::font_atlas_mut()");
            let atlas_ptr = (*io).Fonts;
            FontAtlas::from_raw(atlas_ptr)
        }
    }

    /// Returns the font atlas (alias for font_atlas_mut)
    ///
    /// This provides compatibility with imgui-rs naming convention
    pub fn fonts(&mut self) -> FontAtlas {
        self.font_atlas_mut()
    }

    /// Attempts to clone the interior shared font atlas **if it exists**.
    pub fn clone_shared_font_atlas(&mut self) -> Option<SharedFontAtlas> {
        self.shared_font_atlas.clone()
    }
}
