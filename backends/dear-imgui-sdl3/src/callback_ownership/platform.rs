use std::cell::Cell;
use std::rc::Rc;

use dear_imgui_rs::sys;

use crate::core::Sdl3BackendError;
use crate::core::ffi;
use crate::runtime::RuntimeControl;

use super::native_callbacks::{
    register_runtime, sdl3_create_vk_surface, sdl3_create_window, sdl3_destroy_window,
    sdl3_get_window_focus, sdl3_get_window_framebuffer_scale, sdl3_get_window_minimized,
    sdl3_get_window_pos, sdl3_get_window_size, sdl3_render_window, sdl3_set_window_alpha,
    sdl3_set_window_focus, sdl3_set_window_pos, sdl3_set_window_size, sdl3_set_window_title,
    sdl3_show_window, sdl3_swap_buffers, sdl3_update_window,
};
use super::{
    BackendState, MonitorState, PlatformCallbackOwnership, PlatformCallbackSlot, PlatformCallbacks,
    PlatformClaimBaseline, PlatformShutdownRestore, SDL_PLATFORM_RESERVED_FLAGS,
    SDL_PLATFORM_STABLE_FLAGS, ViewportPlatformState, callback_eq,
    for_each_platform_service_callback, for_each_user_data, restored_owned_pointer,
};

impl PlatformCallbackOwnership {
    pub(crate) unsafe fn claim(
        control: &Rc<RuntimeControl>,
        baseline: PlatformClaimBaseline,
    ) -> Result<Self, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        let original = unsafe { PlatformCallbacks::capture(platform_io) };
        register_runtime(control);
        unsafe {
            if original.window_callback_is_some(PlatformCallbackSlot::CreateWindow) {
                (*platform_io).Platform_CreateWindow = Some(sdl3_create_window);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::DestroyWindow) {
                (*platform_io).Platform_DestroyWindow = Some(sdl3_destroy_window);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::ShowWindow) {
                (*platform_io).Platform_ShowWindow = Some(sdl3_show_window);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::UpdateWindow) {
                (*platform_io).Platform_UpdateWindow = Some(sdl3_update_window);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::SetWindowPos) {
                sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(
                    platform_io,
                    Some(sdl3_set_window_pos),
                );
            }
            if original.window_callback_is_some(PlatformCallbackSlot::GetWindowPos) {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(
                    platform_io,
                    Some(sdl3_get_window_pos),
                );
            }
            if original.window_callback_is_some(PlatformCallbackSlot::SetWindowSize) {
                sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(
                    platform_io,
                    Some(sdl3_set_window_size),
                );
            }
            if original.window_callback_is_some(PlatformCallbackSlot::GetWindowSize) {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                    platform_io,
                    Some(sdl3_get_window_size),
                );
            }
            if original.window_callback_is_some(PlatformCallbackSlot::GetWindowFramebufferScale) {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    platform_io,
                    Some(sdl3_get_window_framebuffer_scale),
                );
            }
            if original.window_callback_is_some(PlatformCallbackSlot::SetWindowFocus) {
                (*platform_io).Platform_SetWindowFocus = Some(sdl3_set_window_focus);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::GetWindowFocus) {
                (*platform_io).Platform_GetWindowFocus = Some(sdl3_get_window_focus);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::GetWindowMinimized) {
                (*platform_io).Platform_GetWindowMinimized = Some(sdl3_get_window_minimized);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::SetWindowTitle) {
                (*platform_io).Platform_SetWindowTitle = Some(sdl3_set_window_title);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::RenderWindow) {
                (*platform_io).Platform_RenderWindow = Some(sdl3_render_window);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::SwapBuffers) {
                (*platform_io).Platform_SwapBuffers = Some(sdl3_swap_buffers);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::SetWindowAlpha) {
                (*platform_io).Platform_SetWindowAlpha = Some(sdl3_set_window_alpha);
            }
            if original.window_callback_is_some(PlatformCallbackSlot::CreateVkSurface) {
                (*platform_io).Platform_CreateVkSurface = Some(sdl3_create_vk_surface);
            }
        }
        let installed = unsafe { PlatformCallbacks::capture(platform_io) };
        let installed_backend = unsafe { BackendState::capture(io) };
        let installed_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        let owned_monitors = Cell::new(unsafe { MonitorState::capture(platform_io) });
        if !installed_main_viewport.user_data.is_null() {
            control.remember_owned_viewport(main_viewport, installed_main_viewport);
        }

        Ok(Self {
            baseline,
            original,
            installed,
            installed_backend,
            installed_main_viewport,
            owned_monitors,
        })
    }

    pub(crate) unsafe fn prepare_shutdown(
        &self,
        control: &RuntimeControl,
    ) -> Result<PlatformShutdownRestore, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_backend = unsafe { BackendState::capture(io) };
        let current_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        let current_monitors = unsafe { MonitorState::capture(platform_io) };
        let complete_foreign_takeover = self.is_complete_foreign_takeover(
            &current,
            current_backend,
            current_main_viewport,
            current_monitors,
        );
        let capabilities_were_revoked =
            control.capabilities_were_revoked(SDL_PLATFORM_RESERVED_FLAGS);
        let monitors_owned = current_monitors == self.owned_monitors.get();

        for slot in PlatformCallbackSlot::ALL {
            if !self.installed.same_window_callback(&current, slot) {
                control.record_callback_replaced(slot.name());
            }
        }

        if self.baseline.backend.user_data != self.installed_backend.user_data
            && self.installed_backend.user_data != current_backend.user_data
        {
            control.record_platform_state_replaced("BackendPlatformUserData");
        }
        if self.baseline.backend.name != self.installed_backend.name
            && self.installed_backend.name != current_backend.name
        {
            control.record_platform_state_replaced("BackendPlatformName");
        }
        if !capabilities_were_revoked
            && current_backend.flags & SDL_PLATFORM_STABLE_FLAGS
                != self.installed_backend.flags & SDL_PLATFORM_STABLE_FLAGS
        {
            control.record_platform_state_replaced("BackendFlags(platform-owned bits)");
        }
        if !monitors_owned {
            control.record_platform_state_replaced("PlatformIO.Monitors");
        }
        if self.baseline.main_viewport.user_data != self.installed_main_viewport.user_data
            && self.installed_main_viewport.user_data != current_main_viewport.user_data
        {
            control.record_foreign_platform_user_data();
        }
        if self.baseline.main_viewport.handle != self.installed_main_viewport.handle
            && self.installed_main_viewport.handle != current_main_viewport.handle
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandle");
        }
        if self.baseline.main_viewport.handle_raw != self.installed_main_viewport.handle_raw
            && self.installed_main_viewport.handle_raw != current_main_viewport.handle_raw
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandleRaw");
        }
        if complete_foreign_takeover {
            control.preserve_complete_foreign_platform_capabilities(current_backend.flags);
        }

        unsafe {
            macro_rules! restore_owned_service_callback {
                ($field:ident) => {
                    if !callback_eq!(
                        self.baseline.callbacks.raw.$field,
                        self.installed.raw.$field
                    ) {
                        (*platform_io).$field = self.installed.raw.$field;
                    }
                };
            }
            for_each_platform_service_callback!(restore_owned_service_callback);
            self.installed.install_window_callbacks(platform_io);

            macro_rules! restore_owned_user_data {
                ($field:ident) => {
                    if self.baseline.callbacks.raw.$field != self.installed.raw.$field {
                        (*platform_io).$field = self.installed.raw.$field;
                    }
                };
            }
            for_each_user_data!(restore_owned_user_data);

            if self.baseline.backend.user_data != self.installed_backend.user_data {
                (*io).BackendPlatformUserData = self.installed_backend.user_data;
            }
            if self.baseline.backend.name != self.installed_backend.name {
                (*io).BackendPlatformName = self.installed_backend.name;
            }
            (*io).BackendFlags = (current_backend.flags & !SDL_PLATFORM_STABLE_FLAGS)
                | (self.installed_backend.flags & SDL_PLATFORM_STABLE_FLAGS);
            if self.baseline.main_viewport.user_data != self.installed_main_viewport.user_data {
                (*main_viewport).PlatformUserData = self.installed_main_viewport.user_data;
            }
            if self.baseline.main_viewport.handle != self.installed_main_viewport.handle {
                (*main_viewport).PlatformHandle = self.installed_main_viewport.handle;
            }
            if self.baseline.main_viewport.handle_raw != self.installed_main_viewport.handle_raw {
                (*main_viewport).PlatformHandleRaw = self.installed_main_viewport.handle_raw;
            }
        }

        Ok(PlatformShutdownRestore {
            callbacks: current,
            backend: current_backend,
            main_viewport: current_main_viewport,
            monitors_owned,
            foreign_capabilities: control.capabilities_are_foreign(SDL_PLATFORM_RESERVED_FLAGS),
        })
    }

    pub(crate) unsafe fn restore_after_shutdown(
        &self,
        restore: PlatformShutdownRestore,
    ) -> Result<(), Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        unsafe {
            macro_rules! restore_service_callback {
                ($field:ident) => {
                    if callback_eq!(
                        self.baseline.callbacks.raw.$field,
                        self.installed.raw.$field
                    ) {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    } else if callback_eq!(self.installed.raw.$field, restore.callbacks.raw.$field)
                    {
                        (*platform_io).$field = self.baseline.callbacks.raw.$field;
                    } else {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    }
                };
            }
            for_each_platform_service_callback!(restore_service_callback);

            if PlatformCallbackSlot::ALL.into_iter().all(|slot| {
                self.installed
                    .same_window_callback(&restore.callbacks, slot)
            }) {
                self.baseline
                    .callbacks
                    .install_window_callbacks(platform_io);
            } else {
                for slot in PlatformCallbackSlot::ALL {
                    if self
                        .installed
                        .same_window_callback(&restore.callbacks, slot)
                    {
                        self.baseline
                            .callbacks
                            .install_window_callback(platform_io, slot);
                    } else {
                        restore.callbacks.install_window_callback(platform_io, slot);
                    }
                }
            }

            macro_rules! restore_user_data {
                ($field:ident) => {
                    if self.baseline.callbacks.raw.$field == self.installed.raw.$field {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    } else if self.installed.raw.$field == restore.callbacks.raw.$field {
                        (*platform_io).$field = self.baseline.callbacks.raw.$field;
                    } else {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    }
                };
            }
            for_each_user_data!(restore_user_data);

            let current_flags = (*io).BackendFlags;
            let backend = BackendState {
                user_data: restored_owned_pointer(
                    self.baseline.backend.user_data,
                    self.installed_backend.user_data,
                    restore.backend.user_data,
                ),
                name: restored_owned_pointer(
                    self.baseline.backend.name,
                    self.installed_backend.name,
                    restore.backend.name,
                ),
                flags: (current_flags & !SDL_PLATFORM_RESERVED_FLAGS)
                    | if restore.foreign_capabilities {
                        restore.backend.flags & SDL_PLATFORM_RESERVED_FLAGS
                    } else {
                        self.baseline.backend.flags & SDL_PLATFORM_RESERVED_FLAGS
                    },
            };
            backend.restore(io);

            if restore.monitors_owned {
                ffi::dear_imgui_sdl3_backend_clear_platform_monitors();
            } else {
                (*platform_io).Monitors = restore.callbacks.raw.Monitors;
            }
        }

        let main_viewport_restore = ViewportPlatformState {
            user_data: restored_owned_pointer(
                self.baseline.main_viewport.user_data,
                self.installed_main_viewport.user_data,
                restore.main_viewport.user_data,
            ),
            handle: restored_owned_pointer(
                self.baseline.main_viewport.handle,
                self.installed_main_viewport.handle,
                restore.main_viewport.handle,
            ),
            handle_raw: restored_owned_pointer(
                self.baseline.main_viewport.handle_raw,
                self.installed_main_viewport.handle_raw,
                restore.main_viewport.handle_raw,
            ),
        };
        unsafe { main_viewport_restore.restore(main_viewport) };
        Ok(())
    }

    pub(crate) unsafe fn detect_replacements(&self, control: &RuntimeControl) -> bool {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            control.record_platform_state_replaced("platform callback table");
            return false;
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        // Snapshot every compared value before recording a fault. Recording revokes this
        // runtime's capability bits, which must not be mistaken for a second external drift in
        // the same validation pass.
        let current_backend = unsafe { BackendState::capture(io) };
        let current_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        let current_monitors = unsafe { MonitorState::capture(platform_io) };
        let complete_foreign_takeover = self.is_complete_foreign_takeover(
            &current,
            current_backend,
            current_main_viewport,
            current_monitors,
        );
        let capabilities_were_revoked =
            control.capabilities_were_revoked(SDL_PLATFORM_RESERVED_FLAGS);
        let mut owned = true;
        for slot in PlatformCallbackSlot::ALL {
            if !self.installed.same_window_callback(&current, slot) {
                control.record_callback_replaced(slot.name());
                owned = false;
            }
        }

        if self.baseline.backend.user_data != self.installed_backend.user_data
            && self.installed_backend.user_data != current_backend.user_data
        {
            control.record_platform_state_replaced("BackendPlatformUserData");
            owned = false;
        }
        if self.baseline.backend.name != self.installed_backend.name
            && self.installed_backend.name != current_backend.name
        {
            control.record_platform_state_replaced("BackendPlatformName");
            owned = false;
        }
        if !capabilities_were_revoked
            && current_backend.flags & SDL_PLATFORM_STABLE_FLAGS
                != self.installed_backend.flags & SDL_PLATFORM_STABLE_FLAGS
        {
            control.record_platform_state_replaced("BackendFlags(platform-owned bits)");
            owned = false;
        }
        if self.baseline.main_viewport.user_data != self.installed_main_viewport.user_data
            && self.installed_main_viewport.user_data != current_main_viewport.user_data
        {
            if current_main_viewport.user_data.is_null() {
                control.record_platform_state_replaced("MainViewport.PlatformUserData");
            } else {
                control.record_foreign_platform_user_data();
            }
            owned = false;
        }
        if self.baseline.main_viewport.handle != self.installed_main_viewport.handle
            && self.installed_main_viewport.handle != current_main_viewport.handle
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandle");
            owned = false;
        }
        if self.baseline.main_viewport.handle_raw != self.installed_main_viewport.handle_raw
            && self.installed_main_viewport.handle_raw != current_main_viewport.handle_raw
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandleRaw");
            owned = false;
        }
        if current_monitors != self.owned_monitors.get() {
            control.record_platform_state_replaced("PlatformIO.Monitors");
            owned = false;
        }
        if !owned && complete_foreign_takeover {
            control.preserve_complete_foreign_platform_capabilities(current_backend.flags);
        }
        owned
    }

    fn is_complete_foreign_takeover(
        &self,
        current: &PlatformCallbacks,
        current_backend: BackendState,
        current_main_viewport: ViewportPlatformState,
        current_monitors: MonitorState,
    ) -> bool {
        let foreign_core_identity = !current_backend.user_data.is_null()
            && !current_backend.name.is_null()
            && current_backend.user_data != self.installed_backend.user_data
            && current_backend.name != self.installed_backend.name;
        let foreign_main_viewport_identity = !current_main_viewport.user_data.is_null()
            && !current_main_viewport.handle.is_null()
            && !current_main_viewport.handle_raw.is_null()
            && current_main_viewport.user_data != self.installed_main_viewport.user_data
            && current_main_viewport.handle != self.installed_main_viewport.handle
            && current_main_viewport.handle_raw != self.installed_main_viewport.handle_raw;
        if !foreign_core_identity || !foreign_main_viewport_identity {
            return false;
        }

        let all_owned_callbacks_replaced = PlatformCallbackSlot::ALL.into_iter().all(|slot| {
            if self.original.window_callback_is_some(slot) {
                // Every callback claimed by this registration must now point at a foreign
                // implementation. A baseline callback that was already foreign is not evidence
                // of a takeover, but it also must not make the predicate fail.
                self.baseline
                    .callbacks
                    .same_window_callback(&self.installed, slot)
                    || (current.window_callback_is_some(slot)
                        && !self.installed.same_window_callback(current, slot))
            } else {
                // Filling a slot that was empty before the claim is a foreign partial write, not
                // evidence that another backend completed a takeover.
                !current.window_callback_is_some(slot)
            }
        });

        let owned_monitors = self.owned_monitors.get();
        let monitors_transferred = owned_monitors.is_empty()
            || (!current_monitors.is_empty() && current_monitors != owned_monitors);
        all_owned_callbacks_replaced && monitors_transferred
    }

    pub(crate) unsafe fn refresh_owned_monitors(&self) {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if !platform_io.is_null() {
            self.owned_monitors
                .set(unsafe { MonitorState::capture(platform_io) });
        }
    }
}
