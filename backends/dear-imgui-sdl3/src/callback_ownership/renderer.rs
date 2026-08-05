use std::ffi::c_void;

use dear_imgui_rs::sys;

use crate::core::Sdl3BackendError;
use crate::runtime::{NativeRendererKind, RuntimeControl};

use super::native_callbacks::{
    sdl3_renderer_create_window, sdl3_renderer_destroy_window, sdl3_renderer_render_window,
    sdl3_renderer_set_window_size, sdl3_renderer_swap_buffers,
};
use super::{
    PlatformCallbacks, PlatformClaimBaseline, RendererBackendState, RendererCallbackOwnership,
    RendererSetWindowSizeCallback, RendererSetWindowSizeInvocation, RendererShutdownRestore,
    SDL_RENDERER_RESERVED_FLAGS, callback_eq, for_each_renderer_non_aggregate_callback,
    for_each_renderer_value, restored_owned_pointer,
};

impl RendererCallbackOwnership {
    pub(crate) unsafe fn claim(
        control: &RuntimeControl,
        baseline: &PlatformClaimBaseline,
    ) -> Result<Option<Self>, Sdl3BackendError> {
        if control.native_renderer() == NativeRendererKind::None {
            return Ok(None);
        }
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        let original = unsafe { PlatformCallbacks::capture(platform_io) };
        let original_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let installed_set_window_size = if original_set_window_size.is_some() {
            RendererSetWindowSizeCallback::Pointer(sdl3_renderer_set_window_size)
        } else {
            RendererSetWindowSizeCallback::Native(None)
        };
        unsafe {
            if original.raw.Renderer_CreateWindow.is_some() {
                (*platform_io).Renderer_CreateWindow = Some(sdl3_renderer_create_window);
            }
            if original.raw.Renderer_DestroyWindow.is_some() {
                (*platform_io).Renderer_DestroyWindow = Some(sdl3_renderer_destroy_window);
            }
            if original_set_window_size.is_some() {
                installed_set_window_size.install(platform_io);
            }
            if original.raw.Renderer_RenderWindow.is_some() {
                (*platform_io).Renderer_RenderWindow = Some(sdl3_renderer_render_window);
            }
            if original.raw.Renderer_SwapBuffers.is_some() {
                (*platform_io).Renderer_SwapBuffers = Some(sdl3_renderer_swap_buffers);
            }
        }

        Ok(Some(Self {
            baseline: baseline.callbacks.snapshot(),
            baseline_set_window_size: baseline.renderer_set_window_size,
            original,
            original_set_window_size,
            installed: unsafe { PlatformCallbacks::capture(platform_io) },
            installed_set_window_size,
            baseline_backend: baseline.renderer_backend,
            installed_backend: unsafe { RendererBackendState::capture(io) },
            baseline_main_viewport_renderer_user_data: baseline.main_viewport_renderer_user_data,
            installed_main_viewport_renderer_user_data: unsafe {
                (*main_viewport).RendererUserData
            },
        }))
    }

    pub(crate) unsafe fn detect_replacements(&self, control: &RuntimeControl) -> bool {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            control.record_renderer_state_replaced("renderer callback table");
            return false;
        }

        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let current_backend = unsafe { RendererBackendState::capture(io) };
        let complete_foreign_takeover =
            self.is_complete_foreign_takeover(&current, current_set_window_size, current_backend);
        let capabilities_were_revoked =
            control.capabilities_were_revoked(SDL_RENDERER_RESERVED_FLAGS);
        let mut owned = true;

        macro_rules! detect_replacement {
            ($field:ident) => {
                if !callback_eq!(self.installed.raw.$field, current.raw.$field) {
                    control.record_renderer_callback_replaced(stringify!($field));
                    owned = false;
                }
            };
        }
        for_each_renderer_non_aggregate_callback!(detect_replacement);
        if !self
            .installed_set_window_size
            .same_callback(current_set_window_size)
        {
            control.record_renderer_callback_replaced("Renderer_SetWindowSize");
            owned = false;
        }
        macro_rules! detect_value_replacement {
            ($field:ident) => {
                if self.installed.raw.$field != current.raw.$field {
                    control.record_renderer_state_replaced(stringify!($field));
                    owned = false;
                }
            };
        }
        for_each_renderer_value!(detect_value_replacement);

        if self.baseline_backend.user_data != self.installed_backend.user_data
            && self.installed_backend.user_data != current_backend.user_data
        {
            control.record_renderer_state_replaced("BackendRendererUserData");
            owned = false;
        }
        if self.baseline_backend.name != self.installed_backend.name
            && self.installed_backend.name != current_backend.name
        {
            control.record_renderer_state_replaced("BackendRendererName");
            owned = false;
        }
        if !capabilities_were_revoked
            && current_backend.flags & SDL_RENDERER_RESERVED_FLAGS
                != self.installed_backend.flags & SDL_RENDERER_RESERVED_FLAGS
        {
            control.record_renderer_state_replaced("BackendFlags(renderer-owned bits)");
            owned = false;
        }
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if main_viewport.is_null() {
            control.record_renderer_state_replaced("MainViewport.RendererUserData");
            owned = false;
        } else {
            let current = unsafe { (*main_viewport).RendererUserData };
            if self.baseline_main_viewport_renderer_user_data
                != self.installed_main_viewport_renderer_user_data
                && current != self.installed_main_viewport_renderer_user_data
            {
                control.record_renderer_state_replaced("MainViewport.RendererUserData");
                owned = false;
            }
        }

        if !owned && complete_foreign_takeover {
            control.preserve_complete_foreign_renderer_capabilities(current_backend.flags);
        }

        owned
    }

    fn is_complete_foreign_takeover(
        &self,
        current: &PlatformCallbacks,
        current_set_window_size: RendererSetWindowSizeCallback,
        current_backend: RendererBackendState,
    ) -> bool {
        let foreign_core_identity = !current_backend.user_data.is_null()
            && !current_backend.name.is_null()
            && current_backend.user_data != self.installed_backend.user_data
            && current_backend.name != self.installed_backend.name;
        if !foreign_core_identity {
            return false;
        }

        let mut all_owned_callbacks_replaced = true;
        macro_rules! require_owned_callback_replacement {
            ($field:ident) => {
                if self.original.raw.$field.is_some()
                    && callback_eq!(self.installed.raw.$field, current.raw.$field)
                {
                    all_owned_callbacks_replaced = false;
                }
            };
        }
        for_each_renderer_non_aggregate_callback!(require_owned_callback_replacement);
        if self.original_set_window_size.is_some()
            && self
                .installed_set_window_size
                .same_callback(current_set_window_size)
        {
            all_owned_callbacks_replaced = false;
        }
        all_owned_callbacks_replaced
    }

    pub(crate) unsafe fn prepare_platform_shutdown(
        &self,
        control: &RuntimeControl,
    ) -> Result<RendererShutdownRestore, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let current_backend = unsafe { RendererBackendState::capture(io) };
        let current_main_viewport_renderer_user_data = unsafe { (*main_viewport).RendererUserData };
        let _ = unsafe { self.detect_replacements(control) };

        unsafe {
            macro_rules! restore_owned_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.installed.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_owned_callback);
            self.installed_set_window_size.install(platform_io);
            macro_rules! restore_owned_value {
                ($field:ident) => {
                    (*platform_io).$field = self.installed.raw.$field;
                };
            }
            for_each_renderer_value!(restore_owned_value);
            self.restore_owned_backend_fields(io, current_backend.flags);
            if self.baseline_main_viewport_renderer_user_data
                != self.installed_main_viewport_renderer_user_data
            {
                (*main_viewport).RendererUserData = self.installed_main_viewport_renderer_user_data;
            }
        }

        Ok(RendererShutdownRestore {
            callbacks: current,
            set_window_size: current_set_window_size,
            backend: current_backend,
            main_viewport_renderer_user_data: current_main_viewport_renderer_user_data,
            foreign_capabilities: control.capabilities_are_foreign(SDL_RENDERER_RESERVED_FLAGS),
        })
    }

    pub(crate) unsafe fn prepare_native_shutdown(
        &self,
        control: &RuntimeControl,
    ) -> Result<RendererShutdownRestore, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let current_backend = unsafe { RendererBackendState::capture(io) };
        let current_main_viewport_renderer_user_data = unsafe { (*main_viewport).RendererUserData };
        let _ = unsafe { self.detect_replacements(control) };

        unsafe {
            macro_rules! restore_original_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_original_callback);
            self.original_set_window_size.install(platform_io);
            macro_rules! restore_original_value {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_value!(restore_original_value);
            self.restore_owned_backend_fields(io, current_backend.flags);
            if self.baseline_main_viewport_renderer_user_data
                != self.installed_main_viewport_renderer_user_data
            {
                (*main_viewport).RendererUserData = self.installed_main_viewport_renderer_user_data;
            }
        }

        Ok(RendererShutdownRestore {
            callbacks: current,
            set_window_size: current_set_window_size,
            backend: current_backend,
            main_viewport_renderer_user_data: current_main_viewport_renderer_user_data,
            foreign_capabilities: control.capabilities_are_foreign(SDL_RENDERER_RESERVED_FLAGS),
        })
    }

    pub(crate) unsafe fn switch_from_platform_to_native_shutdown(
        &self,
    ) -> Result<(), Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        unsafe {
            macro_rules! restore_original_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_original_callback);
            self.original_set_window_size.install(platform_io);
            macro_rules! restore_original_value {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_value!(restore_original_value);
            self.restore_owned_backend_fields(io, (*io).BackendFlags);
            if self.baseline_main_viewport_renderer_user_data
                != self.installed_main_viewport_renderer_user_data
            {
                (*main_viewport).RendererUserData = self.installed_main_viewport_renderer_user_data;
            }
        }
        Ok(())
    }

    unsafe fn restore_owned_backend_fields(&self, io: *mut sys::ImGuiIO, current_flags: i32) {
        let io = unsafe { &mut *io };
        if self.baseline_backend.user_data != self.installed_backend.user_data {
            io.BackendRendererUserData = self.installed_backend.user_data;
        }
        if self.baseline_backend.name != self.installed_backend.name {
            io.BackendRendererName = self.installed_backend.name;
        }
        io.BackendFlags = (current_flags & !SDL_RENDERER_RESERVED_FLAGS)
            | (self.installed_backend.flags & SDL_RENDERER_RESERVED_FLAGS);
    }

    pub(crate) unsafe fn restore_after_shutdown(
        &self,
        restore: RendererShutdownRestore,
    ) -> Result<(), Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        unsafe {
            macro_rules! restore_callback {
                ($field:ident) => {
                    if callback_eq!(self.baseline.raw.$field, self.installed.raw.$field) {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    } else if callback_eq!(self.installed.raw.$field, restore.callbacks.raw.$field)
                    {
                        (*platform_io).$field = self.baseline.raw.$field;
                    } else {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    }
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_callback);
            let restored_set_window_size = if self
                .baseline_set_window_size
                .same_callback(self.installed_set_window_size)
            {
                restore.set_window_size
            } else if self
                .installed_set_window_size
                .same_callback(restore.set_window_size)
            {
                self.baseline_set_window_size
            } else {
                restore.set_window_size
            };
            restored_set_window_size.install(platform_io);
            macro_rules! restore_value {
                ($field:ident) => {
                    (*platform_io).$field = restored_owned_pointer(
                        self.baseline.raw.$field,
                        self.installed.raw.$field,
                        restore.callbacks.raw.$field,
                    );
                };
            }
            for_each_renderer_value!(restore_value);

            (*main_viewport).RendererUserData = restored_owned_pointer(
                self.baseline_main_viewport_renderer_user_data,
                self.installed_main_viewport_renderer_user_data,
                restore.main_viewport_renderer_user_data,
            );

            let current_flags = (*io).BackendFlags;
            let backend = RendererBackendState {
                user_data: restored_owned_pointer(
                    self.baseline_backend.user_data,
                    self.installed_backend.user_data,
                    restore.backend.user_data,
                ),
                name: restored_owned_pointer(
                    self.baseline_backend.name,
                    self.installed_backend.name,
                    restore.backend.name,
                ),
                flags: (current_flags & !SDL_RENDERER_RESERVED_FLAGS)
                    | if restore.foreign_capabilities {
                        restore.backend.flags & SDL_RENDERER_RESERVED_FLAGS
                    } else {
                        self.baseline_backend.flags & SDL_RENDERER_RESERVED_FLAGS
                    },
            };
            backend.restore(io);
        }
        Ok(())
    }

    pub(crate) fn original_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Renderer_CreateWindow
    }

    pub(crate) fn original_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Renderer_DestroyWindow
    }

    pub(crate) fn original_render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.original.raw.Renderer_RenderWindow
    }

    pub(crate) fn original_set_window_size_invocation(&self) -> RendererSetWindowSizeInvocation {
        RendererSetWindowSizeInvocation {
            callback: self.original_set_window_size,
            native_callbacks: self.original.snapshot(),
        }
    }

    pub(crate) fn original_swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.original.raw.Renderer_SwapBuffers
    }
}
