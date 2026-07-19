use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use dear_imgui_rs::{Context, sys};

use crate::core::Sdl3BackendError;
use crate::runtime::{RuntimeControl, with_current_runtime};

macro_rules! for_each_callback {
    ($macro:ident) => {
        $macro!(Platform_GetClipboardTextFn);
        $macro!(Platform_SetClipboardTextFn);
        $macro!(Platform_OpenInShellFn);
        $macro!(Platform_SetImeDataFn);
        $macro!(Platform_CreateWindow);
        $macro!(Platform_DestroyWindow);
        $macro!(Platform_ShowWindow);
        $macro!(Platform_SetWindowPos);
        $macro!(Platform_GetWindowPos);
        $macro!(Platform_SetWindowSize);
        $macro!(Platform_GetWindowSize);
        $macro!(Platform_GetWindowFramebufferScale);
        $macro!(Platform_SetWindowFocus);
        $macro!(Platform_GetWindowFocus);
        $macro!(Platform_GetWindowMinimized);
        $macro!(Platform_SetWindowTitle);
        $macro!(Platform_SetWindowAlpha);
        $macro!(Platform_UpdateWindow);
        $macro!(Platform_RenderWindow);
        $macro!(Platform_SwapBuffers);
        $macro!(Platform_GetWindowDpiScale);
        $macro!(Platform_OnChangedViewport);
        $macro!(Platform_GetWindowWorkAreaInsets);
        $macro!(Platform_CreateVkSurface);
    };
}

macro_rules! for_each_user_data {
    ($macro:ident) => {
        $macro!(Platform_ClipboardUserData);
        $macro!(Platform_OpenInShellUserData);
        $macro!(Platform_ImeUserData);
    };
}

macro_rules! callback_eq {
    ($left:expr, $right:expr) => {
        match ($left, $right) {
            (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    };
}

struct PlatformCallbacks {
    raw: sys::ImGuiPlatformIO,
}

impl PlatformCallbacks {
    unsafe fn capture(raw: *const sys::ImGuiPlatformIO) -> Self {
        Self {
            raw: unsafe { std::ptr::read(raw) },
        }
    }

    fn snapshot(&self) -> Self {
        // The bindgen platform IO value has no Rust destructor. Copying it here
        // snapshots pointer-sized callback state without taking native ownership.
        unsafe { Self::capture(&self.raw) }
    }
}

#[derive(Clone, Copy)]
struct BackendState {
    user_data: *mut c_void,
    name: *const std::ffi::c_char,
}

impl BackendState {
    unsafe fn capture(io: *const sys::ImGuiIO) -> Self {
        let io = unsafe { &*io };
        Self {
            user_data: io.BackendPlatformUserData,
            name: io.BackendPlatformName,
        }
    }

    unsafe fn restore(self, io: *mut sys::ImGuiIO) {
        let io = unsafe { &mut *io };
        io.BackendPlatformUserData = self.user_data;
        io.BackendPlatformName = self.name;
    }
}

#[derive(Clone, Copy)]
pub(super) struct ViewportPlatformState {
    user_data: *mut c_void,
    handle: *mut c_void,
    handle_raw: *mut c_void,
}

impl ViewportPlatformState {
    pub(super) unsafe fn capture(viewport: *const sys::ImGuiViewport) -> Self {
        let viewport = unsafe { &*viewport };
        Self {
            user_data: viewport.PlatformUserData,
            handle: viewport.PlatformHandle,
            handle_raw: viewport.PlatformHandleRaw,
        }
    }

    pub(super) unsafe fn restore(self, viewport: *mut sys::ImGuiViewport) {
        let viewport = unsafe { &mut *viewport };
        viewport.PlatformUserData = self.user_data;
        viewport.PlatformHandle = self.handle;
        viewport.PlatformHandleRaw = self.handle_raw;
    }
}

fn restored_owned_pointer<T: Copy + Eq>(baseline: T, installed: T, current: T) -> T {
    if baseline == installed {
        current
    } else if installed == current {
        baseline
    } else {
        current
    }
}

pub(super) struct PlatformClaimBaseline {
    callbacks: PlatformCallbacks,
    backend: BackendState,
    main_viewport: ViewportPlatformState,
}

impl PlatformClaimBaseline {
    pub(super) fn snapshot(&self) -> Self {
        Self {
            callbacks: self.callbacks.snapshot(),
            backend: self.backend,
            main_viewport: self.main_viewport,
        }
    }
}

pub(super) struct PlatformCallbackOwnership {
    baseline: PlatformClaimBaseline,
    original: PlatformCallbacks,
    installed: PlatformCallbacks,
    installed_backend: BackendState,
    installed_main_viewport: ViewportPlatformState,
}

pub(super) struct PlatformShutdownRestore {
    callbacks: PlatformCallbacks,
    backend: BackendState,
    main_viewport: ViewportPlatformState,
}

pub(super) fn preflight_platform_claim(
    context: &Context,
) -> Result<PlatformClaimBaseline, Sdl3BackendError> {
    context.binding().try_with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        if io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        if !(*io).BackendPlatformUserData.is_null() {
            return Err(Sdl3BackendError::PlatformBackendOccupied);
        }

        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        if platform_io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        let raw = &*platform_io;
        let occupied = [
            (raw.Platform_CreateWindow.is_some(), "Platform_CreateWindow"),
            (
                raw.Platform_DestroyWindow.is_some(),
                "Platform_DestroyWindow",
            ),
            (raw.Platform_ShowWindow.is_some(), "Platform_ShowWindow"),
            (raw.Platform_SetWindowPos.is_some(), "Platform_SetWindowPos"),
            (raw.Platform_GetWindowPos.is_some(), "Platform_GetWindowPos"),
            (
                raw.Platform_SetWindowSize.is_some(),
                "Platform_SetWindowSize",
            ),
            (
                raw.Platform_GetWindowSize.is_some(),
                "Platform_GetWindowSize",
            ),
            (
                raw.Platform_GetWindowFramebufferScale.is_some(),
                "Platform_GetWindowFramebufferScale",
            ),
            (
                raw.Platform_SetWindowFocus.is_some(),
                "Platform_SetWindowFocus",
            ),
            (
                raw.Platform_GetWindowFocus.is_some(),
                "Platform_GetWindowFocus",
            ),
            (
                raw.Platform_GetWindowMinimized.is_some(),
                "Platform_GetWindowMinimized",
            ),
            (
                raw.Platform_SetWindowTitle.is_some(),
                "Platform_SetWindowTitle",
            ),
            (
                raw.Platform_SetWindowAlpha.is_some(),
                "Platform_SetWindowAlpha",
            ),
            (raw.Platform_UpdateWindow.is_some(), "Platform_UpdateWindow"),
            (raw.Platform_RenderWindow.is_some(), "Platform_RenderWindow"),
            (raw.Platform_SwapBuffers.is_some(), "Platform_SwapBuffers"),
            (
                raw.Platform_GetWindowDpiScale.is_some(),
                "Platform_GetWindowDpiScale",
            ),
            (
                raw.Platform_OnChangedViewport.is_some(),
                "Platform_OnChangedViewport",
            ),
            (
                raw.Platform_GetWindowWorkAreaInsets.is_some(),
                "Platform_GetWindowWorkAreaInsets",
            ),
            (
                raw.Platform_CreateVkSurface.is_some(),
                "Platform_CreateVkSurface",
            ),
        ];
        if let Some((_, callback)) = occupied.into_iter().find(|(occupied, _)| *occupied) {
            return Err(Sdl3BackendError::PlatformCallbackOccupied { callback });
        }
        if !(*main_viewport).PlatformUserData.is_null() {
            return Err(Sdl3BackendError::ForeignPlatformUserData);
        }

        Ok(PlatformClaimBaseline {
            callbacks: PlatformCallbacks::capture(platform_io),
            backend: BackendState::capture(io),
            main_viewport: ViewportPlatformState::capture(main_viewport),
        })
    })?
}

impl PlatformCallbackOwnership {
    pub(super) unsafe fn claim(
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
            if original.raw.Platform_CreateWindow.is_some() {
                (*platform_io).Platform_CreateWindow = Some(sdl3_create_window);
            }
            if original.raw.Platform_DestroyWindow.is_some() {
                (*platform_io).Platform_DestroyWindow = Some(sdl3_destroy_window);
            }
        }
        let installed = unsafe { PlatformCallbacks::capture(platform_io) };
        let installed_backend = unsafe { BackendState::capture(io) };
        let installed_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        if !installed_main_viewport.user_data.is_null() {
            control.remember_owned_viewport(main_viewport, installed_main_viewport);
        }

        Ok(Self {
            baseline,
            original,
            installed,
            installed_backend,
            installed_main_viewport,
        })
    }

    pub(super) unsafe fn prepare_shutdown(
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

        macro_rules! detect_replacement {
            ($field:ident) => {
                if !callback_eq!(
                    self.baseline.callbacks.raw.$field,
                    self.installed.raw.$field
                ) && !callback_eq!(self.installed.raw.$field, current.raw.$field)
                {
                    control.record_callback_replaced(stringify!($field));
                }
            };
        }
        for_each_callback!(detect_replacement);

        macro_rules! detect_user_data_replacement {
            ($field:ident) => {
                if self.baseline.callbacks.raw.$field != self.installed.raw.$field
                    && self.installed.raw.$field != current.raw.$field
                {
                    control.record_platform_state_replaced(stringify!($field));
                }
            };
        }
        for_each_user_data!(detect_user_data_replacement);
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

        unsafe {
            macro_rules! restore_owned_callback {
                ($field:ident) => {
                    if !callback_eq!(
                        self.baseline.callbacks.raw.$field,
                        self.installed.raw.$field
                    ) {
                        (*platform_io).$field = self.installed.raw.$field;
                    }
                };
            }
            for_each_callback!(restore_owned_callback);

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
        })
    }

    pub(super) unsafe fn restore_after_shutdown(
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
            macro_rules! restore_callback {
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
            for_each_callback!(restore_callback);

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
            };
            backend.restore(io);
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

    pub(super) fn original_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Platform_CreateWindow
    }

    pub(super) fn original_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Platform_DestroyWindow
    }

    pub(super) unsafe fn detect_replacements(&self, control: &RuntimeControl) {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() {
            return;
        }
        let current = unsafe { &*platform_io };
        macro_rules! detect_replacement {
            ($field:ident) => {
                if !callback_eq!(
                    self.baseline.callbacks.raw.$field,
                    self.installed.raw.$field
                ) && !callback_eq!(self.installed.raw.$field, current.$field)
                {
                    control.record_callback_replaced(stringify!($field));
                }
            };
        }
        for_each_callback!(detect_replacement);

        macro_rules! detect_user_data_replacement {
            ($field:ident) => {
                if self.baseline.callbacks.raw.$field != self.installed.raw.$field
                    && self.installed.raw.$field != current.$field
                {
                    control.record_platform_state_replaced(stringify!($field));
                }
            };
        }
        for_each_user_data!(detect_user_data_replacement);
        if !io.is_null() {
            let current_backend = unsafe { BackendState::capture(io) };
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
        }
    }
}

pub(super) unsafe fn restore_baseline_after_failed_initialization(baseline: PlatformClaimBaseline) {
    let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
    let main_viewport = unsafe { sys::igGetMainViewport() };
    if !platform_io.is_null() {
        unsafe {
            macro_rules! restore_callback {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_callback!(restore_callback);
            macro_rules! restore_user_data {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_user_data!(restore_user_data);
        }
    }
    if !main_viewport.is_null() {
        unsafe { baseline.main_viewport.restore(main_viewport) };
    }
}

fn register_runtime(control: &Rc<RuntimeControl>) {
    crate::runtime::register_runtime(control);
}

unsafe extern "C" fn sdl3_create_window(viewport: *mut sys::ImGuiViewport) {
    run_callback("Platform_CreateWindow", |control| unsafe {
        if viewport.is_null() {
            return;
        }
        if !(*viewport).PlatformUserData.is_null() {
            control.record_foreign_platform_user_data();
            (*viewport).PlatformRequestClose = true;
            return;
        }
        let Some(callback) = control.original_create_window() else {
            return;
        };
        callback(viewport);
        let state = ViewportPlatformState::capture(viewport);
        if !state.user_data.is_null() {
            control.remember_owned_viewport(viewport, state);
        }
        if state.user_data.is_null() || state.handle.is_null() {
            control.record_viewport_creation_failed();
            (*viewport).PlatformRequestClose = true;
        }
    });
}

unsafe extern "C" fn sdl3_destroy_window(viewport: *mut sys::ImGuiViewport) {
    run_callback("Platform_DestroyWindow", |control| unsafe {
        if viewport.is_null() {
            return;
        }
        let Some(callback) = control.original_destroy_window() else {
            return;
        };
        let actual = ViewportPlatformState::capture(viewport);
        let Some(expected) = control.take_owned_viewport(viewport) else {
            record_viewport_replacements(control, None, actual);
            return;
        };

        if viewport_platform_state_eq(actual, expected) {
            callback(viewport);
            return;
        }

        record_viewport_replacements(control, Some(expected), actual);
        expected.restore(viewport);
        callback(viewport);
        actual.restore(viewport);
    });
}

fn viewport_platform_state_eq(left: ViewportPlatformState, right: ViewportPlatformState) -> bool {
    left.user_data == right.user_data
        && left.handle == right.handle
        && left.handle_raw == right.handle_raw
}

fn record_viewport_replacements(
    control: &RuntimeControl,
    expected: Option<ViewportPlatformState>,
    actual: ViewportPlatformState,
) {
    if expected.is_none_or(|expected| expected.user_data != actual.user_data)
        && !actual.user_data.is_null()
    {
        control.record_foreign_platform_user_data();
    }
    if expected.is_none_or(|expected| expected.handle != actual.handle) && !actual.handle.is_null()
    {
        control.record_platform_state_replaced("Viewport.PlatformHandle");
    }
    if expected.is_none_or(|expected| expected.handle_raw != actual.handle_raw)
        && !actual.handle_raw.is_null()
    {
        control.record_platform_state_replaced("Viewport.PlatformHandleRaw");
    }
}

fn run_callback(name: &'static str, callback: impl FnOnce(&RuntimeControl)) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = with_current_runtime(callback);
    }));
    if result.is_err() {
        let _ = with_current_runtime(|control| control.record_callback_panicked(name));
    }
}

#[cfg(test)]
pub(super) unsafe fn create_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_create_window(viewport) }
}

#[cfg(test)]
pub(super) unsafe fn destroy_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_destroy_window(viewport) }
}
