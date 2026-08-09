use crate::{
    GammaMode, RendererError, RendererResult, ShaderManager, WgpuBackendData, WgpuTextureManager,
};
use dear_imgui_rs::{
    BackendFlags, Context, ContextBinding,
    render::{PendingFrame, ReconciledFrame, SynchronousRendererConsumer},
    sys,
};
use std::{
    cell::Cell,
    ffi::{c_char, c_void},
};
use wgpu::TextureView;

#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use wgpu::Color;

/// Stable, non-zero-sized marker published through `BackendRendererUserData`.
#[derive(Debug)]
struct RendererBackendToken {
    _marker: u8,
}

/// Pointer-identity publication data shared with the Context-owned deferred-drop attachment.
///
/// The boxed token remains owned by [`RendererContextState`], while this copy is sufficient to
/// clear only exact raw publications during Context teardown. It never dereferences `token_ptr`.
#[derive(Clone, Debug)]
pub(super) struct RendererPublication {
    context: ContextBinding,
    token_ptr: *mut c_void,
    renderer_name_ptr: *const c_char,
    renderer_flags_added: BackendFlags,
}

impl RendererPublication {
    pub(super) fn context(&self) -> ContextBinding {
        self.context.clone()
    }

    /// Clears only raw fields that still carry this renderer's exact identity.
    ///
    /// Returns whether the Context-owned renderer name pointer was still ours.
    pub(super) unsafe fn clear_owned_raw_state_bound(&self) -> bool {
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return false;
        }
        // SAFETY: the owner Context is bound and its native state is still alive.
        let io = unsafe { &mut *io };
        let owned_name =
            !self.renderer_name_ptr.is_null() && io.BackendRendererName == self.renderer_name_ptr;
        let owned_token = io.BackendRendererUserData == self.token_ptr;

        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let owns_draw_callback = if platform_io.is_null() {
            false
        } else {
            // SAFETY: PlatformIO belongs to the bound owner Context.
            WgpuRenderer::owns_any_standard_draw_callback(unsafe {
                dear_imgui_rs::platform_io::PlatformIo::from_raw(platform_io)
            })
        };

        if owned_name {
            io.BackendRendererName = std::ptr::null();
        }
        if owned_token {
            io.BackendRendererUserData = std::ptr::null_mut();
        }

        // Capability bits have no independent identity. Revoke them only while at least one
        // core publication still proves this renderer owns the lease; a complete foreign
        // takeover must retain the foreign backend's advertised capabilities.
        if owned_name || owned_token || owns_draw_callback {
            io.BackendFlags &= !self.renderer_flags_added.bits();
        }

        if !platform_io.is_null() {
            WgpuRenderer::clear_owned_draw_callbacks(unsafe {
                dear_imgui_rs::platform_io::PlatformIo::from_raw_mut(platform_io)
            });
        }
        owned_name
    }

    /// Returns whether the bound Context still contains at least one publication owned by this
    /// renderer. Capability bits deliberately do not participate because they may already
    /// describe a complete foreign renderer takeover.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) unsafe fn owns_any_raw_publication_bound(&self) -> bool {
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return false;
        }
        // SAFETY: the caller has bound this state owner's live Context.
        let io = unsafe { &*io };
        if (!self.renderer_name_ptr.is_null() && io.BackendRendererName == self.renderer_name_ptr)
            || io.BackendRendererUserData == self.token_ptr
        {
            return true;
        }

        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        !platform_io.is_null()
            // SAFETY: PlatformIO belongs to the currently bound owner Context.
            && WgpuRenderer::owns_any_standard_draw_callback(unsafe {
                dear_imgui_rs::platform_io::PlatformIo::from_raw(platform_io)
            })
    }
}

/// Exclusive ownership of renderer fields published into one Dear ImGui Context.
///
/// This value is intentionally not cloneable. Its boxed token and Context-owned renderer name
/// retain stable addresses when `WgpuRenderer` moves, while one owner remains responsible for
/// clearing the raw pointers before the token is released.
#[derive(Debug)]
pub(super) struct RendererContextState {
    publication: RendererPublication,
    token: Box<RendererBackendToken>,
    fault: Cell<Option<&'static str>>,
}

impl RendererContextState {
    pub(super) fn publish(
        context: &mut Context,
        renderer_flags_added: BackendFlags,
    ) -> RendererResult<Self> {
        let renderer_name_ptr = context
            .io()
            .backend_renderer_name()
            .ok_or_else(|| {
                RendererError::InvalidRenderState(
                    "WGPU renderer name was not published before binding".to_owned(),
                )
            })?
            .as_ptr();
        if !context.io().backend_renderer_user_data().is_null() {
            return Err(RendererError::ContextAlreadyHasRenderer);
        }

        let token = Box::new(RendererBackendToken { _marker: 0 });
        let publication = RendererPublication {
            context: context.binding(),
            token_ptr: std::ptr::from_ref(token.as_ref()).cast_mut().cast(),
            renderer_name_ptr,
            renderer_flags_added,
        };
        let state = Self {
            publication,
            token,
            fault: Cell::new(None),
        };
        // SAFETY: the boxed token has a stable address and remains alive until this state clears
        // the raw pointer or until the native Context has already been destroyed.
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(state.publication.token_ptr);
        }
        Ok(state)
    }

    fn ensure_alive(&self) -> RendererResult<()> {
        if self.publication.context.is_alive() {
            Ok(())
        } else {
            Err(RendererError::ContextDropped)
        }
    }

    pub(super) fn ensure_matches(&self, context: &Context) -> RendererResult<()> {
        self.ensure_alive()?;
        if self.publication.context.id() == context.id() {
            Ok(())
        } else {
            Err(RendererError::ContextMismatch)
        }
    }

    pub(super) fn ensure_renderer_contract(&self) -> RendererResult<()> {
        self.ensure_alive()?;
        if let Some(field) = self.fault.get() {
            let _ = self.publication.context.try_with_bound_context(|| unsafe {
                self.clear_owned_raw_state_bound();
            });
            return Err(RendererError::RendererStateDrift { field });
        }

        let fault = self
            .publication
            .context
            .try_with_bound_context(|| {
                let fault = self.current_renderer_state_fault_bound();
                if fault.is_some() {
                    // SAFETY: the owner Context is bound for this validation and cleanup.
                    unsafe { self.clear_owned_raw_state_bound() };
                }
                fault
            })
            .map_err(|_| RendererError::ContextDropped)?;
        if let Some(field) = fault {
            self.fault.set(Some(field));
            Err(RendererError::RendererStateDrift { field })
        } else {
            Ok(())
        }
    }

    pub(super) fn context(&self) -> ContextBinding {
        self.publication.context()
    }

    pub(super) fn publication(&self) -> RendererPublication {
        debug_assert_eq!(self.publication.token_ptr, self.token_ptr());
        self.publication.clone()
    }

    pub(super) fn clear_with_context(&self, context: &mut Context) {
        // SAFETY: callers have already verified the matching live Context.
        let owned_name = unsafe { self.publication.clear_owned_raw_state_bound() };
        if owned_name {
            context
                .set_renderer_name::<String>(None)
                .expect("clearing WGPU BackendRendererName must not fail");
        }
    }

    pub(super) unsafe fn clear_owned_raw_state_bound(&self) -> bool {
        unsafe { self.publication.clear_owned_raw_state_bound() }
    }

    /// Returns whether the bound Context still contains at least one publication owned by this
    /// renderer.
    ///
    /// Capability bits deliberately do not participate: they have no per-renderer identity and
    /// may already describe a complete foreign renderer takeover.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) unsafe fn owns_any_raw_publication_bound(&self) -> bool {
        unsafe { self.publication.owns_any_raw_publication_bound() }
    }

    fn current_renderer_state_fault_bound(&self) -> Option<&'static str> {
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return Some("ImGuiIO");
        }
        // SAFETY: the caller has bound the renderer's Context for the whole check.
        let io = unsafe { &*io };
        if io.BackendRendererUserData != self.publication.token_ptr {
            return Some("BackendRendererUserData");
        }
        if io.BackendRendererName != self.publication.renderer_name_ptr {
            return Some("BackendRendererName");
        }
        let flags = BackendFlags::from_bits_retain(io.BackendFlags);
        for (present, field) in [
            (
                flags.contains(BackendFlags::RENDERER_HAS_VTX_OFFSET),
                "RENDERER_HAS_VTX_OFFSET",
            ),
            (
                flags.contains(BackendFlags::RENDERER_HAS_TEXTURES),
                "RENDERER_HAS_TEXTURES",
            ),
        ] {
            if !present {
                return Some(field);
            }
        }

        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return Some("PlatformIO");
        }
        // SAFETY: PlatformIO belongs to the currently bound Context.
        let platform_io = unsafe { dear_imgui_rs::platform_io::PlatformIo::from_raw(platform_io) };
        if !unsafe { platform_io.renderer_render_state() }.is_null() {
            return Some("Renderer_RenderState");
        }
        // WGPU publishes no texture limit. A non-zero value is therefore foreign state.
        let raw = unsafe { &*platform_io.as_raw() };
        if raw.Renderer_TextureMaxWidth != 0 {
            return Some("Renderer_TextureMaxWidth");
        }
        if raw.Renderer_TextureMaxHeight != 0 {
            return Some("Renderer_TextureMaxHeight");
        }
        if !WgpuRenderer::owned_draw_callbacks_match(platform_io) {
            return Some("DrawCallback_*");
        }
        None
    }

    fn token_ptr(&self) -> *mut c_void {
        std::ptr::from_ref(self.token.as_ref()).cast_mut().cast()
    }
}

impl Drop for RendererContextState {
    fn drop(&mut self) {
        // Drop runs before `token`, so a live Context cannot retain a pointer to freed storage.
        let _ = self.publication.context.try_with_bound_context(|| unsafe {
            self.clear_owned_raw_state_bound();
        });
    }
}

/// Main WGPU renderer for Dear ImGui
///
/// This corresponds to the main renderer functionality in imgui_impl_wgpu.cpp
///
/// An initialized renderer owns the renderer state of exactly one [`Context`]. Create a separate
/// renderer for every Dear ImGui context. Call [`Self::shutdown`](WgpuRenderer::shutdown) with
/// the matching context when GPU resources must be released before Context teardown or before
/// that live Context can accept a replacement renderer. Dropping a bound renderer without that
/// call is safe but defers its renderer resources and consumer to Context teardown, so it does
/// not make the Context available for reuse. The retained context binding makes this renderer
/// UI-thread-bound.
pub struct WgpuRenderer {
    /// Dear ImGui context whose renderer state this instance owns.
    pub(super) context_state: Option<RendererContextState>,
    /// Backend data
    pub(super) backend_data: Option<WgpuBackendData>,
    /// Shader manager
    pub(super) shader_manager: ShaderManager,
    /// Texture manager
    pub(super) texture_manager: WgpuTextureManager,
    /// Default texture for fallback
    pub(super) default_texture: Option<TextureView>,
    /// Gamma mode: automatic (by format), force linear (1.0), or force 2.2
    pub(super) gamma_mode: GammaMode,
    /// Clear color used for secondary viewports (multi-viewport mode)
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) viewport_clear_color: Color,
    /// Sole managed-texture consumer generation owned by this renderer.
    pub(super) renderer_consumer: Option<SynchronousRendererConsumer>,
    /// Context-owned fallback for a renderer wrapper dropped without explicit shutdown.
    pub(super) drop_deferral: Option<super::lifecycle::RendererDropDeferral>,
}

impl WgpuRenderer {
    pub(super) fn bind_context(
        &mut self,
        context: &mut Context,
        renderer_flags_added: BackendFlags,
    ) -> RendererResult<()> {
        if self.context_state.is_some() {
            return Err(RendererError::InvalidRenderState(
                "renderer is already bound to a Dear ImGui context".to_owned(),
            ));
        }
        let drop_deferral = super::lifecycle::RendererDropDeferral::register(context)?;
        let state = match RendererContextState::publish(context, renderer_flags_added) {
            Ok(state) => state,
            Err(error) => {
                drop(drop_deferral);
                return Err(error);
            }
        };
        drop_deferral.set_publication(state.publication());
        self.context_state = Some(state);
        self.drop_deferral = Some(drop_deferral);
        Ok(())
    }

    pub(super) fn ensure_context_alive(&self) -> RendererResult<()> {
        self.context_state
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .ensure_alive()
    }

    pub(super) fn ensure_renderer_contract(&self) -> RendererResult<()> {
        self.context_state
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .ensure_renderer_contract()
    }

    /// Reports whether the currently bound Context still exposes an exact core WGPU
    /// publication. Callers use this before revoking untagged renderer capability bits.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn owns_context_publication_bound(&self) -> bool {
        self.context_state.as_ref().is_some_and(|state| {
            // SAFETY: callers invoke this only while the renderer's Context is bound.
            unsafe { state.owns_any_raw_publication_bound() }
        })
    }

    pub(super) fn ensure_context_matches(&self, context: &Context) -> RendererResult<()> {
        self.context_state
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .ensure_matches(context)
    }

    pub(super) fn bound_context(&self) -> RendererResult<ContextBinding> {
        Ok(self
            .context_state
            .as_ref()
            .ok_or(RendererError::ContextNotBound)?
            .context())
    }

    pub(super) fn clear_context_state(&mut self) {
        self.context_state = None;
        self.drop_deferral = None;
    }

    /// Returns the synchronous consumer capability owned by this renderer.
    ///
    /// Use it with [`Context::render`] when application code needs to reconcile a frame before
    /// platform-window callbacks or other renderer-managed work.
    pub fn renderer_consumer(&self) -> RendererResult<&SynchronousRendererConsumer> {
        self.renderer_consumer
            .as_ref()
            .ok_or(RendererError::ContextNotBound)
    }

    pub(super) fn ensure_pending_frame_matches(
        &self,
        frame: &PendingFrame<'_>,
    ) -> RendererResult<()> {
        let consumer = self.renderer_consumer()?;
        if frame.context_id() != consumer.context_id() {
            return Err(RendererError::ContextMismatch);
        }
        let epoch = frame.epoch();
        if epoch.consumer_generation() != consumer.generation() {
            return Err(RendererError::InvalidRenderState(format!(
                "pending frame uses consumer generation {}, WGPU owns generation {}",
                epoch.consumer_generation(),
                consumer.generation()
            )));
        }
        Ok(())
    }

    pub(super) fn ensure_reconciled_frame_matches(
        &self,
        frame: &ReconciledFrame<'_>,
    ) -> RendererResult<()> {
        let consumer = self.renderer_consumer()?;
        if frame.context_id() != consumer.context_id() {
            return Err(RendererError::ContextMismatch);
        }
        let epoch = frame.epoch().ok_or_else(|| {
            RendererError::InvalidRenderState(
                "WGPU requires a managed-texture renderer epoch".to_owned(),
            )
        })?;
        if epoch.consumer_generation() != consumer.generation() {
            return Err(RendererError::InvalidRenderState(format!(
                "reconciled frame uses consumer generation {}, WGPU owns generation {}",
                epoch.consumer_generation(),
                consumer.generation()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_owner_is_not_confused_with_a_reused_context_address() {
        let owner = Context::create();
        let owner_id = owner.id();
        let binding = owner.binding();
        drop(owner);

        let replacement = Context::create();
        assert!(!binding.is_alive());
        assert_ne!(owner_id, replacement.id());
    }
}
