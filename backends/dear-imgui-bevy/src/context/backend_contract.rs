//! Raw Dear ImGui backend field claims and renderer ownership validation.

use std::ffi::c_char;
#[cfg(feature = "render")]
use std::ffi::c_void;

#[cfg(feature = "render")]
use crate::render;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use crate::viewport;

use super::ImguiContextConfig;

const BACKEND_NAME: &str = "dear-imgui-bevy";

/// Non-send Bevy resource that owns the Dear ImGui context.
///
/// Dear ImGui has process-global current-context state and `dear_imgui_rs::Context` is intentionally
/// not `Send`/`Sync`. Storing it as a Bevy non-send resource keeps UI lifecycle work on the main
/// thread until later tasks add schedule-specific accessors.
#[cfg(feature = "render")]
#[derive(Clone, Copy)]
pub(super) struct ImguiRendererRuntimeContract {
    backend_user_data: *mut c_void,
    backend_name: *const c_char,
    owned_flags: i32,
    render_state: *mut c_void,
    texture_max_width: i32,
    texture_max_height: i32,
    viewport_callbacks: [usize; 5],
    draw_callbacks: [usize; 3],
}

#[cfg(feature = "render")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiRendererOwnershipError {
    /// A renderer field no longer matches the value installed by this backend.
    FieldReplaced { field: &'static str },
}

/// Failure to enter or finish a temporary active-Context scope.
pub type ImguiContextScopeError = dear_imgui_rs::ContextScopeError;

pub(crate) fn separate_scoped_error<E>(
    error: dear_imgui_rs::ScopedActivationError<E>,
) -> Result<ImguiContextScopeError, E> {
    match error.into_closure_error() {
        Ok(error) => Err(error),
        Err(error) => Ok(error),
    }
}

pub(crate) enum ImguiActiveRendererContextError<E> {
    Operation(E),
    ContextScope(ImguiContextScopeError),
    #[cfg(feature = "render")]
    RendererOwnership(ImguiRendererOwnershipError),
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportBridge(viewport::ImguiViewportRuntimeError),
}

impl<E> ImguiActiveRendererContextError<E> {
    pub(crate) fn from_scoped(error: dear_imgui_rs::ScopedActivationError<Self>) -> Self {
        match separate_scoped_error(error) {
            Ok(error) => Self::ContextScope(error),
            Err(error) => error,
        }
    }
}

#[cfg(feature = "render")]
impl std::fmt::Display for ImguiRendererOwnershipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FieldReplaced { field } => {
                write!(
                    formatter,
                    "Dear ImGui renderer field `{field}` was replaced"
                )
            }
        }
    }
}

#[cfg(feature = "render")]
impl std::error::Error for ImguiRendererOwnershipError {}

/// Reason a registered Context cannot finish Context-local teardown yet.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ImguiContextRemovalPendingReason {
    /// The Context could not enter or finish the active scope required for teardown.
    ContextScope(ImguiContextScopeError),
    RenderWorldReleasePending,
    Renderer(dear_imgui_rs::render::RendererConsumerError),
    #[cfg(feature = "render")]
    RendererOwnership(ImguiRendererOwnershipError),
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportCallbackOwnership(viewport::ImguiViewportCallbackOwnershipError),
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportWorldReleasePending,
}

impl std::fmt::Display for ImguiContextRemovalPendingReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextScope(error) => error.fmt(formatter),
            Self::RenderWorldReleasePending => formatter.write_str(
                "Bevy render-world resources are still live; run the render schedule and retry",
            ),
            Self::Renderer(error) => error.fmt(formatter),
            #[cfg(feature = "render")]
            Self::RendererOwnership(error) => error.fmt(formatter),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportCallbackOwnership(error) => error.fmt(formatter),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportWorldReleasePending => formatter.write_str(
                "Bevy secondary viewport entities are still live; run one update and retry",
            ),
        }
    }
}

impl ImguiContextRemovalPendingReason {
    pub(crate) fn from_scoped(error: dear_imgui_rs::ScopedActivationError<Self>) -> Self {
        match separate_scoped_error(error) {
            Ok(error) => Self::ContextScope(error),
            Err(error) => error,
        }
    }
}

impl std::error::Error for ImguiContextRemovalPendingReason {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ContextScope(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) struct ImguiBackendOwnership {
    pub(super) flags_added: dear_imgui_rs::BackendFlags,
    pub(super) platform_name: Option<String>,
    pub(super) platform_name_ptr: *const c_char,
    pub(super) renderer_name: Option<String>,
    pub(super) renderer_name_ptr: *const c_char,
    pub(super) standard_draw_callbacks: bool,
    pub(super) viewport_contract: bool,
    #[cfg(feature = "render")]
    pub(super) renderer_contract: Option<ImguiRendererRuntimeContract>,
    #[cfg(feature = "render")]
    pub(super) renderer_fault: Option<ImguiRendererOwnershipError>,
}

#[derive(Clone)]
pub(crate) struct BackendAttachment {
    pub(crate) render_integration_installed: bool,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) viewport_bridge_registration: Option<viewport::ImguiViewportBridgeRegistration>,
    #[cfg(feature = "render")]
    pub(crate) renderer_releases: Option<render::ImguiRendererReleases>,
}

impl Default for ImguiBackendOwnership {
    fn default() -> Self {
        Self {
            flags_added: dear_imgui_rs::BackendFlags::empty(),
            platform_name: None,
            platform_name_ptr: std::ptr::null(),
            renderer_name: None,
            renderer_name_ptr: std::ptr::null(),
            standard_draw_callbacks: false,
            viewport_contract: false,
            #[cfg(feature = "render")]
            renderer_contract: None,
            #[cfg(feature = "render")]
            renderer_fault: None,
        }
    }
}

pub(super) fn preflight_backend_context_claims(
    context: &dear_imgui_rs::Context,
    ownership: &ImguiBackendOwnership,
    render_integration_installed: bool,
) -> Result<(), &'static str> {
    if let Some(expected) = ownership.platform_name.as_deref()
        && !context.io().backend_platform_name().is_some_and(|actual| {
            actual.as_ptr() == ownership.platform_name_ptr
                && actual.to_bytes() == expected.as_bytes()
        })
    {
        return Err("BackendPlatformName");
    }

    #[cfg(feature = "render")]
    if render_integration_installed {
        if !ownership.standard_draw_callbacks {
            if let Some(field) = renderer_backend_claim_conflict(context, ownership.flags_added) {
                return Err(field);
            }
            if let Some(slot) = render::standard_draw_callback_occupied(context) {
                return Err(slot);
            }
        } else {
            let renderer_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
            if !context.io().backend_flags().contains(renderer_flags) {
                return Err("BackendFlags");
            }
            let Some(expected) = ownership.renderer_name.as_deref() else {
                return Err("BackendRendererName");
            };
            if !context.io().backend_renderer_name().is_some_and(|actual| {
                actual.as_ptr() == ownership.renderer_name_ptr
                    && actual.to_bytes() == expected.as_bytes()
            }) {
                return Err("BackendRendererName");
            }
            if let Some(slot) = render::standard_draw_callback_conflict(context) {
                return Err(slot);
            }
        }
    }

    #[cfg(not(feature = "render"))]
    let _ = render_integration_installed;
    Ok(())
}

pub(super) fn sync_backend_context_config(
    context: &mut dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
    backend: &BackendAttachment,
    config: &ImguiContextConfig,
) {
    let mut config_flags = context.io().config_flags();
    if config.docking() {
        config_flags.insert(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
    } else {
        config_flags.remove(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
    }
    context.io_mut().set_config_flags(config_flags);

    let imgui_name = BACKEND_NAME.to_owned();
    let claim_platform_name = match ownership.platform_name.as_deref() {
        Some(expected) => context.io().backend_platform_name().is_some_and(|actual| {
            actual.as_ptr() == ownership.platform_name_ptr
                && actual.to_bytes() == expected.as_bytes()
        }),
        None => {
            ownership.viewport_contract
                || (context.io().backend_platform_name().is_none()
                    && !has_platform_backend_state(context))
        }
    };
    if claim_platform_name {
        context
            .set_platform_name(Some(imgui_name.clone()))
            .expect("sanitized backend names must be valid C strings");
        ownership.platform_name = Some(imgui_name.clone());
        ownership.platform_name_ptr = context
            .io()
            .backend_platform_name()
            .expect("installed platform name must remain available")
            .as_ptr();
    }

    #[cfg(feature = "render")]
    if backend.render_integration_installed {
        let renderer_was_owned = ownership.standard_draw_callbacks;
        let renderer_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
        let current_flags = context.io().backend_flags();
        render::install_standard_draw_callbacks_for_context(context)
            .expect("renderer callback ownership was preflighted");
        ownership.standard_draw_callbacks = true;
        if !renderer_was_owned {
            ownership.flags_added |= renderer_flags & !current_flags;
            context
                .io_mut()
                .set_backend_flags(current_flags | renderer_flags);
        }
        context
            .set_renderer_name(Some(imgui_name.clone()))
            .expect("sanitized backend names must be valid C strings");
        ownership.renderer_name = Some(imgui_name);
        ownership.renderer_name_ptr = context
            .io()
            .backend_renderer_name()
            .expect("installed renderer name must remain available")
            .as_ptr();
        ownership.renderer_contract = Some(ImguiRendererRuntimeContract::capture(context));
    }

    #[cfg(not(feature = "render"))]
    let _ = backend.render_integration_installed;
}

#[cfg(feature = "render")]
pub(super) fn preflight_renderer_teardown_ownership(
    context: &dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
) -> Result<(), ImguiRendererOwnershipError> {
    let result = validate_renderer_teardown_ownership(context, ownership);
    if result.is_ok() {
        ownership.renderer_fault = None;
    }
    result
}

#[cfg(feature = "render")]
pub(super) fn validate_renderer_teardown_ownership(
    context: &dear_imgui_rs::Context,
    ownership: &ImguiBackendOwnership,
) -> Result<(), ImguiRendererOwnershipError> {
    let Some(expected) = ownership.renderer_contract else {
        return Ok(());
    };
    let actual = ImguiRendererRuntimeContract::capture(context);
    let Some(error) = expected.first_drift(actual) else {
        return Ok(());
    };
    if expected.retains_any_identity(actual) {
        return Err(ownership.renderer_fault.unwrap_or(error));
    }
    Ok(())
}

#[cfg(feature = "render")]
pub(super) fn validate_active_renderer_ownership(
    context: &mut dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
) -> Result<(), ImguiRendererOwnershipError> {
    let Some(expected) = ownership.renderer_contract else {
        return Ok(());
    };
    let actual = ImguiRendererRuntimeContract::capture(context);
    let error = ownership
        .renderer_fault
        .or_else(|| expected.first_drift(actual));
    let Some(error) = error else {
        return Ok(());
    };
    ownership.renderer_fault.get_or_insert(error);
    if expected.retains_any_identity(actual) {
        let io = context.io_mut();
        let owned_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
        io.set_backend_flags(io.backend_flags() & !owned_flags);
    }
    Err(error)
}

pub(super) fn clear_backend_data(
    context: &mut dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
) {
    #[cfg(feature = "render")]
    let renderer_capabilities_still_owned = ownership.renderer_contract.is_some_and(|expected| {
        expected.retains_any_identity(ImguiRendererRuntimeContract::capture(context))
    });
    #[cfg(feature = "render")]
    if ownership.standard_draw_callbacks {
        render::clear_standard_draw_callbacks_if_owned(context);
        ownership.standard_draw_callbacks = false;
        ownership.renderer_contract = None;
    }

    let flags_added = std::mem::replace(
        &mut ownership.flags_added,
        dear_imgui_rs::BackendFlags::empty(),
    );
    #[cfg(feature = "render")]
    let mut flags_to_clear = flags_added;
    #[cfg(not(feature = "render"))]
    let flags_to_clear = flags_added;
    #[cfg(feature = "render")]
    if !renderer_capabilities_still_owned {
        flags_to_clear.remove(
            dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET,
        );
    }
    let current_flags = context.io().backend_flags();
    context
        .io_mut()
        .set_backend_flags(current_flags & !flags_to_clear);

    clear_backend_name_if_owned(
        context,
        &mut ownership.platform_name,
        &mut ownership.platform_name_ptr,
        BackendNameKind::Platform,
    );
    clear_backend_name_if_owned(
        context,
        &mut ownership.renderer_name,
        &mut ownership.renderer_name_ptr,
        BackendNameKind::Renderer,
    );
}

#[derive(Clone, Copy)]
enum BackendNameKind {
    Platform,
    Renderer,
}

fn clear_backend_name_if_owned(
    context: &mut dear_imgui_rs::Context,
    owned_name: &mut Option<String>,
    owned_name_ptr: &mut *const c_char,
    kind: BackendNameKind,
) {
    let Some(expected) = owned_name.take() else {
        *owned_name_ptr = std::ptr::null();
        return;
    };
    let expected_ptr = std::mem::replace(owned_name_ptr, std::ptr::null());
    let still_owned = match kind {
        BackendNameKind::Platform => context.io().backend_platform_name(),
        BackendNameKind::Renderer => context.io().backend_renderer_name(),
    }
    .is_some_and(|actual| {
        actual.as_ptr() == expected_ptr && actual.to_bytes() == expected.as_bytes()
    });
    if !still_owned {
        return;
    }
    match kind {
        BackendNameKind::Platform => context
            .set_platform_name::<String>(None)
            .expect("clearing BackendPlatformName must not fail"),
        BackendNameKind::Renderer => context
            .set_renderer_name::<String>(None)
            .expect("clearing BackendRendererName must not fail"),
    }
}

fn has_platform_backend_state(context: &dear_imgui_rs::Context) -> bool {
    let raw = unsafe { &*context.platform_io().as_raw() };
    !context.io().backend_platform_user_data().is_null()
        || !raw.Monitors.Data.is_null()
        || raw.Monitors.Size != 0
        || raw.Monitors.Capacity != 0
        || raw.Platform_CreateWindow.is_some()
        || raw.Platform_DestroyWindow.is_some()
        || raw.Platform_ShowWindow.is_some()
        || raw.Platform_SetWindowPos.is_some()
        || raw.Platform_GetWindowPos.is_some()
        || raw.Platform_SetWindowSize.is_some()
        || raw.Platform_GetWindowSize.is_some()
        || raw.Platform_GetWindowFramebufferScale.is_some()
        || raw.Platform_SetWindowFocus.is_some()
        || raw.Platform_GetWindowFocus.is_some()
        || raw.Platform_GetWindowMinimized.is_some()
        || raw.Platform_SetWindowTitle.is_some()
        || raw.Platform_SetWindowAlpha.is_some()
        || raw.Platform_UpdateWindow.is_some()
        || raw.Platform_RenderWindow.is_some()
        || raw.Platform_SwapBuffers.is_some()
        || raw.Platform_GetWindowDpiScale.is_some()
        || raw.Platform_OnChangedViewport.is_some()
        || raw.Platform_GetWindowWorkAreaInsets.is_some()
        || raw.Platform_CreateVkSurface.is_some()
}

#[cfg(feature = "render")]
fn renderer_backend_claim_conflict(
    context: &dear_imgui_rs::Context,
    owned_flags: dear_imgui_rs::BackendFlags,
) -> Option<&'static str> {
    if !context.io().backend_renderer_user_data().is_null() {
        return Some("BackendRendererUserData");
    }
    if context.io().backend_renderer_name().is_some() {
        return Some("BackendRendererName");
    }
    let reserved_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
    #[cfg(feature = "multi-viewport")]
    let reserved_flags = reserved_flags | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS;
    if !(context.io().backend_flags() & reserved_flags & !owned_flags).is_empty() {
        return Some("BackendFlags");
    }

    let platform_io = context.platform_io();
    let raw = unsafe { &*platform_io.as_raw() };
    if unsafe { !platform_io.renderer_render_state().is_null() } {
        return Some("Renderer_RenderState");
    }
    for (occupied, field) in [
        (
            raw.Renderer_TextureMaxWidth != 0,
            "Renderer_TextureMaxWidth",
        ),
        (
            raw.Renderer_TextureMaxHeight != 0,
            "Renderer_TextureMaxHeight",
        ),
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
    ] {
        if occupied {
            return Some(field);
        }
    }
    None
}

#[cfg(feature = "render")]
fn renderer_owned_flag_mask() -> i32 {
    (dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET)
        .bits()
}

#[cfg(feature = "render")]
impl ImguiRendererRuntimeContract {
    fn capture(context: &dear_imgui_rs::Context) -> Self {
        let io = context.io();
        let platform_io = context.platform_io();
        let raw = unsafe { &*platform_io.as_raw() };
        Self {
            backend_user_data: io.backend_renderer_user_data(),
            backend_name: io
                .backend_renderer_name()
                .map_or(std::ptr::null(), std::ffi::CStr::as_ptr),
            owned_flags: io.backend_flags().bits() & renderer_owned_flag_mask(),
            render_state: unsafe { platform_io.renderer_render_state() },
            texture_max_width: raw.Renderer_TextureMaxWidth,
            texture_max_height: raw.Renderer_TextureMaxHeight,
            viewport_callbacks: [
                raw.Renderer_CreateWindow
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_DestroyWindow
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_SetWindowSize
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_RenderWindow
                    .map_or(0, |callback| callback as usize),
                raw.Renderer_SwapBuffers
                    .map_or(0, |callback| callback as usize),
            ],
            draw_callbacks: render::standard_draw_callback_contract(context),
        }
    }

    fn first_drift(self, actual: Self) -> Option<ImguiRendererOwnershipError> {
        for (changed, field) in [
            (
                actual.backend_user_data != self.backend_user_data,
                "BackendRendererUserData",
            ),
            (
                actual.backend_name != self.backend_name,
                "BackendRendererName",
            ),
            (actual.owned_flags != self.owned_flags, "BackendFlags"),
            (
                actual.render_state != self.render_state,
                "Renderer_RenderState",
            ),
            (
                actual.texture_max_width != self.texture_max_width,
                "Renderer_TextureMaxWidth",
            ),
            (
                actual.texture_max_height != self.texture_max_height,
                "Renderer_TextureMaxHeight",
            ),
        ] {
            if changed {
                return Some(ImguiRendererOwnershipError::FieldReplaced { field });
            }
        }
        for ((actual, expected), field) in actual
            .viewport_callbacks
            .into_iter()
            .zip(self.viewport_callbacks)
            .zip([
                "Renderer_CreateWindow",
                "Renderer_DestroyWindow",
                "Renderer_SetWindowSize",
                "Renderer_RenderWindow",
                "Renderer_SwapBuffers",
            ])
        {
            if actual != expected {
                return Some(ImguiRendererOwnershipError::FieldReplaced { field });
            }
        }
        actual
            .draw_callbacks
            .into_iter()
            .zip(self.draw_callbacks)
            .zip([
                "DrawCallback_ResetRenderState",
                "DrawCallback_SetSamplerLinear",
                "DrawCallback_SetSamplerNearest",
            ])
            .find_map(|((actual, expected), field)| {
                (actual != expected).then_some(ImguiRendererOwnershipError::FieldReplaced { field })
            })
    }

    fn retains_any_identity(self, actual: Self) -> bool {
        (!self.backend_user_data.is_null() && actual.backend_user_data == self.backend_user_data)
            || (!self.backend_name.is_null() && actual.backend_name == self.backend_name)
            || (!self.render_state.is_null() && actual.render_state == self.render_state)
            || self
                .viewport_callbacks
                .into_iter()
                .zip(actual.viewport_callbacks)
                .any(|(expected, actual)| expected != 0 && expected == actual)
            || self
                .draw_callbacks
                .into_iter()
                .zip(actual.draw_callbacks)
                .any(|(expected, actual)| expected != 0 && expected == actual)
    }
}

#[cfg(test)]
mod context_scope_error_tests {
    use super::{ImguiActiveRendererContextError, ImguiContextScopeError, separate_scoped_error};

    #[test]
    fn scoped_activation_translation_preserves_every_core_scope_failure() {
        assert_eq!(
            separate_scoped_error::<()>(dear_imgui_rs::ScopedActivationError::Scope(
                dear_imgui_rs::ContextScopeError::Activation(
                    dear_imgui_rs::ContextActivationReason::ContextAlreadyActive,
                ),
            )),
            Ok(ImguiContextScopeError::Activation(
                dear_imgui_rs::ContextActivationReason::ContextAlreadyActive,
            ))
        );
        assert_eq!(
            separate_scoped_error::<()>(dear_imgui_rs::ScopedActivationError::Scope(
                dear_imgui_rs::ContextScopeError::FrameLeftOpen,
            )),
            Ok(ImguiContextScopeError::FrameLeftOpen)
        );
        assert_eq!(
            separate_scoped_error::<()>(dear_imgui_rs::ScopedActivationError::Scope(
                dear_imgui_rs::ContextScopeError::ContextUnavailable(
                    dear_imgui_rs::ContextBindingError::NativeDestroyed,
                ),
            )),
            Ok(ImguiContextScopeError::ContextUnavailable(
                dear_imgui_rs::ContextBindingError::NativeDestroyed,
            ))
        );
        assert_eq!(
            separate_scoped_error(dear_imgui_rs::ScopedActivationError::Closure(
                "operation failed",
            )),
            Err("operation failed")
        );
    }

    #[test]
    fn renderer_scope_translation_keeps_closure_errors_on_the_operation_channel() {
        let operation = ImguiActiveRendererContextError::from_scoped(
            dear_imgui_rs::ScopedActivationError::Closure(
                ImguiActiveRendererContextError::Operation("operation failed"),
            ),
        );
        assert!(matches!(
            operation,
            ImguiActiveRendererContextError::Operation("operation failed")
        ));

        let scope = ImguiActiveRendererContextError::<()>::from_scoped(
            dear_imgui_rs::ScopedActivationError::Scope(
                dear_imgui_rs::ContextScopeError::FrameLeftOpen,
            ),
        );
        assert!(matches!(
            scope,
            ImguiActiveRendererContextError::ContextScope(ImguiContextScopeError::FrameLeftOpen)
        ));
    }
}

#[cfg(all(test, feature = "render"))]
mod renderer_contract_tests {
    use std::ffi::c_char;

    use super::{ImguiRendererOwnershipError, ImguiRendererRuntimeContract};

    fn empty_contract() -> ImguiRendererRuntimeContract {
        ImguiRendererRuntimeContract {
            backend_user_data: std::ptr::null_mut(),
            backend_name: std::ptr::null(),
            owned_flags: 0,
            render_state: std::ptr::null_mut(),
            texture_max_width: 0,
            texture_max_height: 0,
            viewport_callbacks: [0; 5],
            draw_callbacks: [0; 3],
        }
    }

    fn changed_viewport_callback(index: usize) -> ImguiRendererRuntimeContract {
        let mut contract = empty_contract();
        contract.viewport_callbacks[index] = 1;
        contract
    }

    fn changed_draw_callback(index: usize) -> ImguiRendererRuntimeContract {
        let mut contract = empty_contract();
        contract.draw_callbacks[index] = 1;
        contract
    }

    #[test]
    fn renderer_contract_reports_every_owned_field() {
        let changed_contracts = [
            (
                "BackendRendererUserData",
                ImguiRendererRuntimeContract {
                    backend_user_data: std::ptr::dangling_mut::<u8>().cast(),
                    ..empty_contract()
                },
            ),
            (
                "BackendRendererName",
                ImguiRendererRuntimeContract {
                    backend_name: std::ptr::dangling::<c_char>(),
                    ..empty_contract()
                },
            ),
            (
                "BackendFlags",
                ImguiRendererRuntimeContract {
                    owned_flags: 1,
                    ..empty_contract()
                },
            ),
            (
                "Renderer_RenderState",
                ImguiRendererRuntimeContract {
                    render_state: std::ptr::dangling_mut::<u8>().cast(),
                    ..empty_contract()
                },
            ),
            (
                "Renderer_TextureMaxWidth",
                ImguiRendererRuntimeContract {
                    texture_max_width: 1,
                    ..empty_contract()
                },
            ),
            (
                "Renderer_TextureMaxHeight",
                ImguiRendererRuntimeContract {
                    texture_max_height: 1,
                    ..empty_contract()
                },
            ),
            ("Renderer_CreateWindow", changed_viewport_callback(0)),
            ("Renderer_DestroyWindow", changed_viewport_callback(1)),
            ("Renderer_SetWindowSize", changed_viewport_callback(2)),
            ("Renderer_RenderWindow", changed_viewport_callback(3)),
            ("Renderer_SwapBuffers", changed_viewport_callback(4)),
            ("DrawCallback_ResetRenderState", changed_draw_callback(0)),
            ("DrawCallback_SetSamplerLinear", changed_draw_callback(1)),
            ("DrawCallback_SetSamplerNearest", changed_draw_callback(2)),
        ];

        assert_eq!(empty_contract().first_drift(empty_contract()), None);
        for (expected_field, actual) in changed_contracts {
            assert_eq!(
                empty_contract().first_drift(actual),
                Some(ImguiRendererOwnershipError::FieldReplaced {
                    field: expected_field
                })
            );
        }
    }
}
