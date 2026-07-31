use std::cell::Cell;
use std::ffi::{c_char, c_void};

use dear_imgui_rs::platform_io::PlatformIo;
use dear_imgui_rs::{BackendFlags, Context, ContextBinding, ContextLifecycle, sys};

use super::{
    RendererError, RendererResult, draw_callback_reset_render_state,
    draw_callback_set_sampler_linear, draw_callback_set_sampler_nearest,
};

const RENDERER_NAME: &str = concat!("dear-imgui-ash ", env!("CARGO_PKG_VERSION"));

fn core_renderer_flags() -> BackendFlags {
    BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES
}

fn renderer_viewport_flag_bits() -> i32 {
    sys::ImGuiBackendFlags_RendererHasViewports
}

fn first_renderer_window_slot(raw: &sys::ImGuiPlatformIO) -> Option<&'static str> {
    [
        (raw.Renderer_CreateWindow.is_some(), "Renderer_CreateWindow"),
        (
            raw.Renderer_DestroyWindow.is_some(),
            "Renderer_DestroyWindow",
        ),
        (
            raw.Renderer_SetWindowSize.is_some(),
            "Renderer_SetWindowSize",
        ),
        (raw.Renderer_RenderWindow.is_some(), "Renderer_RenderWindow"),
        (raw.Renderer_SwapBuffers.is_some(), "Renderer_SwapBuffers"),
    ]
    .into_iter()
    .find_map(|(occupied, field)| occupied.then_some(field))
}

fn draw_callback_matches(
    actual: sys::ImDrawCallback,
    expected: unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn occupied(field: &'static str) -> RendererError {
    RendererError::RendererStateOccupied { field }
}

fn replaced(field: &'static str) -> RendererError {
    RendererError::RendererStateReplaced { field }
}

pub(super) struct RendererContextState {
    binding: ContextBinding,
    token: Box<u8>,
    name_ptr: Cell<*const c_char>,
    published: Cell<bool>,
    fault: Cell<Option<&'static str>>,
}

impl RendererContextState {
    pub(super) fn binding(&self) -> ContextBinding {
        self.binding.clone()
    }

    pub(super) fn prepare(context: &Context) -> RendererResult<Self> {
        Self::preflight(context)?;
        Ok(Self {
            binding: context.binding(),
            token: Box::new(0),
            name_ptr: Cell::new(std::ptr::null()),
            published: Cell::new(false),
            fault: Cell::new(None),
        })
    }

    pub(super) fn preflight(context: &Context) -> RendererResult<()> {
        let io = context.io();
        if !io.backend_renderer_user_data().is_null() {
            return Err(occupied("BackendRendererUserData"));
        }
        if io.backend_renderer_name().is_some() {
            return Err(occupied("BackendRendererName"));
        }
        let reserved_flags = core_renderer_flags().bits() | renderer_viewport_flag_bits();
        if io.backend_flags().bits() & reserved_flags != 0 {
            return Err(occupied("BackendFlags renderer capabilities"));
        }

        let platform_io = context.platform_io();
        let raw = unsafe { &*platform_io.as_raw() };
        if raw.DrawCallback_ResetRenderState.is_some() {
            return Err(occupied("DrawCallback_ResetRenderState"));
        }
        if raw.DrawCallback_SetSamplerLinear.is_some() {
            return Err(occupied("DrawCallback_SetSamplerLinear"));
        }
        if raw.DrawCallback_SetSamplerNearest.is_some() {
            return Err(occupied("DrawCallback_SetSamplerNearest"));
        }
        if !raw.Renderer_RenderState.is_null() {
            return Err(occupied("Renderer_RenderState"));
        }
        if raw.Renderer_TextureMaxWidth != 0 {
            return Err(occupied("Renderer_TextureMaxWidth"));
        }
        if raw.Renderer_TextureMaxHeight != 0 {
            return Err(occupied("Renderer_TextureMaxHeight"));
        }
        if let Some(field) = first_renderer_window_slot(raw) {
            return Err(occupied(field));
        }
        Ok(())
    }

    fn token_ptr(&self) -> *mut c_void {
        std::ptr::from_ref(self.token.as_ref()).cast_mut().cast()
    }

    pub(super) fn publish(&self, context: &mut Context) -> RendererResult<()> {
        Self::preflight(context)?;
        context
            .set_renderer_name(Some(RENDERER_NAME.to_owned()))
            .map_err(|error| RendererError::InvalidRenderState(error.to_string()))?;
        let name_ptr = context
            .io()
            .backend_renderer_name()
            .expect("Ash just published BackendRendererName")
            .as_ptr();
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(self.token_ptr());
        }
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | core_renderer_flags());
        unsafe {
            let platform_io = context.platform_io_mut();
            platform_io
                .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
            platform_io
                .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
            platform_io
                .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        }
        self.name_ptr.set(name_ptr);
        self.fault.set(None);
        self.published.set(true);
        Ok(())
    }

    fn validate_bound(&self) -> RendererResult<()> {
        if !self.published.get() {
            return Err(RendererError::RendererNotAttached);
        }
        let io = unsafe { sys::igGetIO_Nil() };
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if io.is_null() || platform_io.is_null() {
            return Err(replaced("renderer Context pointers"));
        }
        let io = unsafe { &*io };
        if io.BackendRendererUserData != self.token_ptr() {
            return Err(replaced("BackendRendererUserData"));
        }
        if io.BackendRendererName != self.name_ptr.get() {
            return Err(replaced("BackendRendererName"));
        }
        let flags = BackendFlags::from_bits_retain(io.BackendFlags);
        if !flags.contains(core_renderer_flags()) {
            return Err(replaced("BackendFlags renderer capabilities"));
        }

        let platform_io = unsafe { PlatformIo::from_raw(platform_io) };
        let raw = unsafe { &*platform_io.as_raw() };
        if !draw_callback_matches(
            raw.DrawCallback_ResetRenderState,
            draw_callback_reset_render_state,
        ) {
            return Err(replaced("DrawCallback_ResetRenderState"));
        }
        if !draw_callback_matches(
            raw.DrawCallback_SetSamplerLinear,
            draw_callback_set_sampler_linear,
        ) {
            return Err(replaced("DrawCallback_SetSamplerLinear"));
        }
        if !draw_callback_matches(
            raw.DrawCallback_SetSamplerNearest,
            draw_callback_set_sampler_nearest,
        ) {
            return Err(replaced("DrawCallback_SetSamplerNearest"));
        }
        if !raw.Renderer_RenderState.is_null() {
            return Err(replaced("Renderer_RenderState"));
        }
        if raw.Renderer_TextureMaxWidth != 0 {
            return Err(replaced("Renderer_TextureMaxWidth"));
        }
        if raw.Renderer_TextureMaxHeight != 0 {
            return Err(replaced("Renderer_TextureMaxHeight"));
        }

        let has_viewport_capability = io.BackendFlags & renderer_viewport_flag_bits() != 0;
        if has_viewport_capability {
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            if let Some(field) = super::vulkan_viewport::first_renderer_callback_drift(platform_io)
            {
                return Err(replaced(field));
            }
            #[cfg(not(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3")))]
            return Err(replaced("RENDERER_HAS_VIEWPORTS"));
        } else if let Some(field) = first_renderer_window_slot(raw) {
            return Err(replaced(field));
        }
        Ok(())
    }

    /// Reports whether the current Context still contains one of this renderer's core
    /// publications. Callers use this before changing shared capability bits after a callback
    /// contract fault: a complete foreign takeover must not lose the replacement renderer's
    /// capability declaration.
    pub(super) fn owns_core_publication_bound(&self, platform_io: &PlatformIo) -> bool {
        if !self.published.get() {
            return false;
        }
        let io = unsafe { sys::igGetIO_Nil() };
        if io.is_null() {
            return false;
        }
        let io = unsafe { &*io };
        io.BackendRendererUserData == self.token_ptr()
            || (!self.name_ptr.get().is_null() && io.BackendRendererName == self.name_ptr.get())
            || draw_callback_matches(
                platform_io.draw_callback_reset_render_state_raw(),
                draw_callback_reset_render_state,
            )
            || draw_callback_matches(
                platform_io.draw_callback_set_sampler_linear_raw(),
                draw_callback_set_sampler_linear,
            )
            || draw_callback_matches(
                platform_io.draw_callback_set_sampler_nearest_raw(),
                draw_callback_set_sampler_nearest,
            )
    }

    /// Returns whether the native Context is already gone, without entering or inspecting any
    /// current Context state.
    pub(super) fn native_context_is_destroyed(&self) -> bool {
        matches!(self.binding.lifecycle(), ContextLifecycle::NativeDestroyed)
    }

    /// Discards bookkeeping for a Context whose native state was already destroyed.
    ///
    /// No native pointer is dereferenced here. The renderer may need this terminal path after a
    /// prior device-wait failure is retried from the owner once Context teardown has completed.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn forget_destroyed_context(&self) {
        self.published.set(false);
        self.name_ptr.set(std::ptr::null());
    }

    fn revoke_owned_capabilities_bound(&self) {
        if !self.published.get() {
            return;
        }
        let io = unsafe { sys::igGetIO_Nil() };
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if io.is_null() || platform_io.is_null() {
            return;
        }
        let io = unsafe { &mut *io };
        let platform_io = unsafe { PlatformIo::from_raw_mut(platform_io) };
        let owns_core_publication = self.owns_core_publication_bound(platform_io);
        if owns_core_publication {
            io.BackendFlags &= !core_renderer_flags().bits();
        }

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        if io.BackendFlags & renderer_viewport_flag_bits() != 0
            && super::vulkan_viewport::first_renderer_callback_drift(platform_io).is_none()
        {
            io.BackendFlags &= !renderer_viewport_flag_bits();
        }
    }

    pub(super) fn validate(&self) -> RendererResult<()> {
        if let Some(field) = self.fault.get() {
            let binding = self.binding.clone();
            let _ = binding.try_with_bound_context(|| self.revoke_owned_capabilities_bound());
            return Err(replaced(field));
        }
        let binding = self.binding.clone();
        let result = binding
            .try_with_bound_context(|| {
                let result = self.validate_bound();
                if matches!(result, Err(RendererError::RendererStateReplaced { .. })) {
                    self.revoke_owned_capabilities_bound();
                }
                result
            })
            .map_err(|error| RendererError::InvalidRenderState(error.to_string()))?;
        if let Err(RendererError::RendererStateReplaced { field }) = &result {
            self.fault.set(Some(field));
        }
        result
    }

    pub(super) fn unpublish_bound(&self) -> bool {
        if !self.published.replace(false) {
            return false;
        }
        let io = unsafe { sys::igGetIO_Nil() };
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if io.is_null() || platform_io.is_null() {
            return false;
        }
        let io = unsafe { &mut *io };
        let user_data_is_ours = io.BackendRendererUserData == self.token_ptr();
        if user_data_is_ours {
            io.BackendRendererUserData = std::ptr::null_mut();
        }
        let name_ptr = self.name_ptr.replace(std::ptr::null());
        let name_is_ours = !name_ptr.is_null() && io.BackendRendererName == name_ptr;
        if name_is_ours {
            io.BackendRendererName = std::ptr::null();
        }
        let platform_io = unsafe { PlatformIo::from_raw_mut(platform_io) };
        let still_owns_core_publication = user_data_is_ours
            || name_is_ours
            || draw_callback_matches(
                platform_io.draw_callback_reset_render_state_raw(),
                draw_callback_reset_render_state,
            )
            || draw_callback_matches(
                platform_io.draw_callback_set_sampler_linear_raw(),
                draw_callback_set_sampler_linear,
            )
            || draw_callback_matches(
                platform_io.draw_callback_set_sampler_nearest_raw(),
                draw_callback_set_sampler_nearest,
            );
        if still_owns_core_publication {
            io.BackendFlags &= !core_renderer_flags().bits();
        }
        if draw_callback_matches(
            platform_io.draw_callback_reset_render_state_raw(),
            draw_callback_reset_render_state,
        ) {
            unsafe { platform_io.set_draw_callback_reset_render_state_raw(None) };
        }
        if draw_callback_matches(
            platform_io.draw_callback_set_sampler_linear_raw(),
            draw_callback_set_sampler_linear,
        ) {
            unsafe { platform_io.set_draw_callback_set_sampler_linear_raw(None) };
        }
        if draw_callback_matches(
            platform_io.draw_callback_set_sampler_nearest_raw(),
            draw_callback_set_sampler_nearest,
        ) {
            unsafe { platform_io.set_draw_callback_set_sampler_nearest_raw(None) };
        }
        name_is_ours
    }

    pub(super) fn unpublish(&self, context: &mut Context) {
        let name_was_ours = self.unpublish_bound();
        if name_was_ours {
            let _ = context.set_renderer_name(None::<String>);
        }
    }
}

impl Drop for RendererContextState {
    fn drop(&mut self) {
        if !self.published.get() {
            return;
        }
        let binding = self.binding.clone();
        let _ = binding.try_with_bound_context(|| {
            self.unpublish_bound();
        });
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn published_identity_survives_state_moves() {
        let mut context = Context::create();
        let state = RendererContextState::prepare(&context).unwrap();
        state.publish(&mut context).unwrap();
        let token = state.token_ptr();
        let name = state.name_ptr.get();

        let moved = state;

        assert_eq!(moved.token_ptr(), token);
        assert_eq!(moved.name_ptr.get(), name);
        assert_eq!(context.io().backend_renderer_user_data(), token);
        assert_eq!(context.io().backend_renderer_name().unwrap().as_ptr(), name);
    }

    #[test]
    fn same_bytes_foreign_name_is_detected_and_preserved() {
        let mut context = Context::create();
        let state = RendererContextState::prepare(&context).unwrap();
        state.publish(&mut context).unwrap();
        let foreign_name = CString::new(RENDERER_NAME).unwrap();
        assert_ne!(foreign_name.as_ptr(), state.name_ptr.get());
        unsafe {
            (*sys::igGetIO_Nil()).BackendRendererName = foreign_name.as_ptr();
        }

        assert!(matches!(
            state.validate(),
            Err(RendererError::RendererStateReplaced {
                field: "BackendRendererName"
            })
        ));
        state.unpublish(&mut context);

        assert_eq!(
            unsafe { (*sys::igGetIO_Nil()).BackendRendererName },
            foreign_name.as_ptr()
        );
        assert!(context.io().backend_renderer_user_data().is_null());
        context.set_renderer_name(None::<String>).unwrap();
    }

    #[test]
    fn foreign_user_data_is_detected_and_preserved() {
        let mut context = Context::create();
        let state = RendererContextState::prepare(&context).unwrap();
        state.publish(&mut context).unwrap();
        let mut foreign_token = Box::new(1_u8);
        let foreign_ptr = std::ptr::from_mut(foreign_token.as_mut()).cast();
        unsafe {
            (*sys::igGetIO_Nil()).BackendRendererUserData = foreign_ptr;
        }

        assert!(matches!(
            state.validate(),
            Err(RendererError::RendererStateReplaced {
                field: "BackendRendererUserData"
            })
        ));
        state.unpublish(&mut context);

        assert_eq!(context.io().backend_renderer_user_data(), foreign_ptr);
        unsafe {
            context
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::null_mut());
        }
    }

    #[test]
    fn first_contract_drift_is_sticky_and_keeps_owned_capabilities_revoked() {
        let mut context = Context::create();
        let state = RendererContextState::prepare(&context).unwrap();
        state.publish(&mut context).unwrap();
        let owned_name = state.name_ptr.get();
        let foreign_name = CString::new("foreign renderer").unwrap();
        unsafe {
            (*sys::igGetIO_Nil()).BackendRendererName = foreign_name.as_ptr();
        }

        assert!(matches!(
            state.validate(),
            Err(RendererError::RendererStateReplaced {
                field: "BackendRendererName"
            })
        ));
        assert!(
            !context
                .io()
                .backend_flags()
                .intersects(core_renderer_flags())
        );

        unsafe {
            (*sys::igGetIO_Nil()).BackendRendererName = owned_name;
        }
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | core_renderer_flags());
        assert!(matches!(
            state.validate(),
            Err(RendererError::RendererStateReplaced {
                field: "BackendRendererName"
            })
        ));
        assert!(
            !context
                .io()
                .backend_flags()
                .intersects(core_renderer_flags())
        );
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    #[test]
    fn forgetting_a_destroyed_context_never_mutates_a_later_context() {
        let mut first = Context::create();
        let state = RendererContextState::prepare(&first).unwrap();
        state.publish(&mut first).unwrap();

        // Simulate Context attachment teardown clearing the native publication while an external
        // renderer state has not yet observed that its Context is gone. Leaving the publication
        // behind would make Dear ImGui abort during `Context` destruction before this test could
        // exercise the destroyed-context path.
        unsafe {
            first
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::null_mut());
            first
                .platform_io_mut()
                .set_draw_callback_reset_render_state_raw(None);
            first
                .platform_io_mut()
                .set_draw_callback_set_sampler_linear_raw(None);
            first
                .platform_io_mut()
                .set_draw_callback_set_sampler_nearest_raw(None);
        }
        first
            .set_renderer_name(None::<String>)
            .expect("test Context renderer name should clear");
        let flags = first.io().backend_flags();
        first
            .io_mut()
            .set_backend_flags(flags & !core_renderer_flags());
        let suspended = first.suspend();
        drop(suspended);

        let mut later = Context::create();
        let token = state.token_ptr();
        unsafe {
            later.io_mut().set_backend_renderer_user_data(token);
            let io = later.io_mut();
            io.set_backend_flags(io.backend_flags() | core_renderer_flags());
            later
                .platform_io_mut()
                .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
            later
                .platform_io_mut()
                .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
            later
                .platform_io_mut()
                .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        }

        state.forget_destroyed_context();

        assert_eq!(later.io().backend_renderer_user_data(), token);
        assert!(later.io().backend_flags().contains(core_renderer_flags()));
        assert!(draw_callback_matches(
            later.platform_io().draw_callback_reset_render_state_raw(),
            draw_callback_reset_render_state,
        ));
        unsafe {
            later
                .io_mut()
                .set_backend_renderer_user_data(std::ptr::null_mut());
            later
                .platform_io_mut()
                .set_draw_callback_reset_render_state_raw(None);
            later
                .platform_io_mut()
                .set_draw_callback_set_sampler_linear_raw(None);
            later
                .platform_io_mut()
                .set_draw_callback_set_sampler_nearest_raw(None);
        }
    }
}
