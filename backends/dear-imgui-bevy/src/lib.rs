//! Experimental Bevy-native backend for `dear-imgui-rs`.
//!
//! This crate is the Bevy-side integration point for the Bevy Native Backend workstream. It is not
//! a wrapper around `dear-imgui-winit` or `dear-imgui-wgpu`: Bevy owns windows, input, WGPU device
//! state, render schedules, and camera targets. The backend owns only the Bevy plugin/resources that
//! adapt those systems to Dear ImGui.
//!
//! # Compatibility and gates
//!
//! The current Bevy target is Bevy `0.19.0`, which declares Rust `1.95.0`. The root
//! `dear-imgui-rs` workspace currently remains on Rust `1.92`, so this crate has a dedicated
//! `rust-version = "1.95.0"` and should be validated with an explicit Bevy gate, for example:
//!
//! ```text
//! cargo +stable check -p dear-imgui-bevy --no-default-features
//! cargo +stable check -p dear-imgui-bevy --features render
//! cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features --features wasm
//! cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features --features render,wasm
//! cargo +stable nextest run -p dear-imgui-bevy
//! ```
//!
//! Core workspace gates should not silently rely on this crate until the repository-wide MSRV is
//! intentionally raised or CI has a dedicated Rust 1.95+ Bevy lane. The crate currently compiles
//! on `wasm32-unknown-unknown` for both the core and `render` feature sets; mobile targets remain a
//! platform-specific follow-on if a future Bevy target train needs a dedicated gate.
//!
//! The crate also exposes `configure_example_context` for the shared example/editor ImGui setup
//! pattern so the backend examples do not repeat the same initialization boilerplate.
//!
//! # Multi-viewport status
//!
//! `ImguiBackendConfig::multi_viewport` records an explicit request for Dear ImGui platform
//! windows. With the `multi-viewport` and `render` features on native targets, the backend installs
//! the PlatformIO lifecycle bridge, all-window input/platform feedback, and per-window render
//! routing before advertising full multi-viewport support.

pub mod context;
pub mod helpers;
pub mod input;
pub mod schedule;
pub mod texture;
pub mod viewport;

use bevy_app::{App, Plugin};
use bevy_ecs::resource::Resource;
use std::ffi::c_char;
#[cfg(feature = "render")]
use std::ffi::c_void;
use std::rc::Rc;

pub use self::context::{ImguiContexts, ImguiFrameOutput, ImguiFrameState};
pub use self::helpers::configure_example_context;
pub use self::schedule::{ImguiBeginFrame, ImguiEndFrame, ImguiPrimaryContextPass};
#[cfg(feature = "render")]
pub use self::texture::ImguiBevyTextures;
pub use self::viewport::{
    ImguiViewportBridge, ImguiViewportCamera, ImguiViewportCommand, ImguiViewportFeedback,
    ImguiViewportId, ImguiViewportSnapshot, ImguiViewportWindow, ImguiViewportWindowConfig,
};

const MULTI_VIEWPORT_FEATURE_ENABLED: bool = cfg!(feature = "multi-viewport");
const NATIVE_PLATFORM_TARGET: bool = !cfg!(target_arch = "wasm32");

/// Bevy plugin that installs the minimal Dear ImGui resources.
///
/// Later workstream tasks add input collection, frame scheduling, render extraction, and renderer
/// systems. For now the plugin establishes ownership boundaries and resource locations only.
#[derive(Debug, Clone, Default)]
pub struct ImguiPlugin {
    config: ImguiBackendConfig,
}

impl ImguiPlugin {
    /// Create a plugin with explicit backend configuration.
    #[must_use]
    pub fn new(config: ImguiBackendConfig) -> Self {
        Self { config }
    }

    /// Borrow the plugin configuration.
    #[must_use]
    pub fn config(&self) -> &ImguiBackendConfig {
        &self.config
    }
}

impl Plugin for ImguiPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<ImguiBackendConfig>() {
            app.insert_resource(self.config.clone());
        }
        if app.world().get_non_send::<ImguiContext>().is_none() {
            app.insert_non_send(ImguiContext::new(dear_imgui_rs::Context::create()));
        }
        schedule::install_imgui_schedules(app);
        input::install_input_mapping(app);
        context::install_context_lifecycle(app);
        #[cfg(feature = "render")]
        let render_integration_available = render::render_integration_available(app);
        #[cfg(not(feature = "render"))]
        let render_integration_available = false;
        preflight_backend_context_claims(
            app.world()
                .get_non_send::<ImguiContext>()
                .expect("ImguiContext must exist before backend initialization"),
            render_integration_available,
        );
        #[cfg(feature = "render")]
        let render_integration_installed = render::install_render_extraction(app);
        #[cfg(not(feature = "render"))]
        let render_integration_installed = false;
        debug_assert_eq!(render_integration_installed, render_integration_available);
        viewport::install_viewport_bridge(app);
        refresh_backend_status(app, render_integration_installed);
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        {
            let render_integration_installed = render::install_render_extraction(_app);
            refresh_backend_status(_app, render_integration_installed);
        }
    }
}

fn refresh_backend_status(app: &mut App, render_integration_installed: bool) {
    let effective_config = app.world().resource::<ImguiBackendConfig>().clone();
    #[cfg(feature = "render")]
    if render_integration_installed {
        app.world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiContext must exist before renderer validation")
            .assert_active_renderer_ownership();
    }
    preflight_backend_context_claims(
        app.world()
            .get_non_send::<ImguiContext>()
            .expect("ImguiContext must exist before backend initialization"),
        render_integration_installed,
    );
    #[cfg(feature = "render")]
    if render_integration_installed {
        app.world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiContext must exist before renderer initialization")
            .ensure_renderer_consumer()
            .unwrap_or_else(|error| {
                panic!(
                    "ImguiPlugin could not claim the Dear ImGui context for managed rendering: {error}"
                )
            });
    }
    sync_backend_context_config(app, &effective_config, render_integration_installed);
    app.insert_resource(ImguiBackendStatus::from_config(
        &effective_config,
        render_integration_installed,
    ));
}

/// Validate every Dear ImGui backend slot that Bevy may claim before mutating the context.
///
/// Renderer-consumer creation claims the font atlas and snapshot hub, so this preflight must run
/// before that operation. The sync function repeats the check after consumer creation as a local
/// invariant for callers that do not go through [`refresh_backend_status`].
fn preflight_backend_context_claims(
    imgui_context: &ImguiContext,
    render_integration_installed: bool,
) {
    let context = &imgui_context.context;
    let backend_ownership = &imgui_context.backend_ownership;

    match backend_ownership.platform_name.as_deref() {
        Some(expected) => assert!(
            context.io().backend_platform_name().is_some_and(|actual| {
                actual.as_ptr() == backend_ownership.platform_name_ptr
                    && actual.to_bytes() == expected.as_bytes()
            }),
            "dear-imgui-bevy BackendPlatformName ownership changed while the backend was active"
        ),
        None => {
            // An external platform backend is valid when Bevy was not the claimant. This branch
            // only determines whether the later sync may write the platform name; it does not
            // mutate or clear the foreign state.
            let _ = backend_ownership.viewport_contract
                || (context.io().backend_platform_name().is_none()
                    && !has_platform_backend_state(context));
        }
    }

    #[cfg(feature = "render")]
    if render_integration_installed {
        let renderer_was_owned = backend_ownership.standard_draw_callbacks;
        let renderer_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
        let current_flags = context.io().backend_flags();
        if !renderer_was_owned {
            if let Some(field) =
                renderer_backend_claim_conflict(context, backend_ownership.flags_added)
            {
                panic!(
                    "dear-imgui-bevy cannot claim the renderer backend while `{field}` is owned by another integration"
                );
            }
            if let Some(slot) = render::standard_draw_callback_occupied(context) {
                panic!(
                    "dear-imgui-bevy cannot claim draw callback `{slot}` because another renderer owns it"
                );
            }
        } else {
            assert!(
                current_flags.contains(renderer_flags),
                "dear-imgui-bevy renderer capability flags changed while the backend was active"
            );
            let expected_name = backend_ownership
                .renderer_name
                .as_deref()
                .expect("owned draw callbacks require an owned renderer name");
            assert!(
                context.io().backend_renderer_name().is_some_and(|actual| {
                    actual.as_ptr() == backend_ownership.renderer_name_ptr
                        && actual.to_bytes() == expected_name.as_bytes()
                }),
                "dear-imgui-bevy BackendRendererName ownership changed while the backend was active"
            );
        }
        if renderer_was_owned && let Some(slot) = render::standard_draw_callback_conflict(context) {
            panic!(
                "dear-imgui-bevy cannot claim draw callback `{slot}` because another renderer owns it"
            );
        }
    }

    #[cfg(not(feature = "render"))]
    let _ = render_integration_installed;
}

fn sync_backend_context_config(
    app: &mut App,
    config: &ImguiBackendConfig,
    render_integration_installed: bool,
) {
    let Some(mut imgui_context) = app.world_mut().get_non_send_mut::<ImguiContext>() else {
        return;
    };
    #[cfg(feature = "render")]
    if render_integration_installed {
        imgui_context.assert_active_renderer_ownership();
    }
    preflight_backend_context_claims(&imgui_context, render_integration_installed);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    imgui_context.assert_attached_viewport_callback_ownership();
    imgui_context.frame_lifecycle.revoke();
    let ImguiContext {
        context,
        backend_ownership,
        ..
    } = &mut *imgui_context;
    let mut config_flags = context.io().config_flags();
    if config.docking {
        config_flags.insert(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
    } else {
        config_flags.remove(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
    }
    context.io_mut().set_config_flags(config_flags);

    let imgui_name = sanitized_imgui_backend_name(&config.name);
    let claim_platform_name = match backend_ownership.platform_name.as_deref() {
        Some(expected) => {
            assert!(
                context.io().backend_platform_name().is_some_and(|actual| {
                    actual.as_ptr() == backend_ownership.platform_name_ptr
                        && actual.to_bytes() == expected.as_bytes()
                }),
                "dear-imgui-bevy BackendPlatformName ownership changed while the backend was active"
            );
            true
        }
        None => {
            backend_ownership.viewport_contract
                || (context.io().backend_platform_name().is_none()
                    && !has_platform_backend_state(context))
        }
    };
    if claim_platform_name {
        context
            .set_platform_name(Some(imgui_name.clone()))
            .expect("sanitized backend names must be valid C strings");
        backend_ownership.platform_name = Some(imgui_name.clone());
        backend_ownership.platform_name_ptr = context
            .io()
            .backend_platform_name()
            .expect("the installed platform name must remain available")
            .as_ptr();
    }

    if render_integration_installed {
        let renderer_was_owned = backend_ownership.standard_draw_callbacks;
        let renderer_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
        let current_flags = context.io().backend_flags();
        #[cfg(feature = "render")]
        {
            if !renderer_was_owned {
                if let Some(field) =
                    renderer_backend_claim_conflict(context, backend_ownership.flags_added)
                {
                    panic!(
                        "dear-imgui-bevy cannot claim the renderer backend while `{field}` is owned by another integration"
                    );
                }
            } else {
                assert!(
                    current_flags.contains(renderer_flags),
                    "dear-imgui-bevy renderer capability flags changed while the backend was active"
                );
                let expected_name = backend_ownership
                    .renderer_name
                    .as_deref()
                    .expect("owned draw callbacks require an owned renderer name");
                assert!(
                    context.io().backend_renderer_name().is_some_and(|actual| {
                        actual.as_ptr() == backend_ownership.renderer_name_ptr
                            && actual.to_bytes() == expected_name.as_bytes()
                    }),
                    "dear-imgui-bevy BackendRendererName ownership changed while the backend was active"
                );
            }
            render::install_standard_draw_callbacks_for_context(context).unwrap_or_else(|slot| {
                panic!(
                    "dear-imgui-bevy cannot claim draw callback `{slot}` because another renderer owns it"
                )
            });
            backend_ownership.standard_draw_callbacks = true;
        }

        if !renderer_was_owned {
            backend_ownership.flags_added |= renderer_flags & !current_flags;
            context
                .io_mut()
                .set_backend_flags(current_flags | renderer_flags);
        }
        context
            .set_renderer_name(Some(imgui_name.clone()))
            .expect("sanitized backend names must be valid C strings");
        backend_ownership.renderer_name = Some(imgui_name);
        backend_ownership.renderer_name_ptr = context
            .io()
            .backend_renderer_name()
            .expect("the installed renderer name must remain available")
            .as_ptr();
    }
    #[cfg(not(feature = "render"))]
    let _ = render_integration_installed;
    #[cfg(feature = "render")]
    if render_integration_installed {
        imgui_context.record_renderer_runtime_contract();
    }
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    imgui_context.record_attached_viewport_runtime_contract();
}

fn sanitized_imgui_backend_name(name: &str) -> String {
    name.replace('\0', "?")
}

/// Static configuration for the Bevy backend.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ImguiBackendConfig {
    /// User-facing label recorded in the Dear ImGui context and diagnostics.
    pub name: String,
    /// Whether the backend should request docking support when lifecycle code wires IO flags.
    pub docking: bool,
    /// Whether the user requested Dear ImGui docking multi-viewport OS windows.
    ///
    /// This is recorded in [`ImguiBackendStatus::multi_viewport_requested`]. Full support is only
    /// advertised after the native PlatformIO lifecycle bridge, all-window input feedback, and
    /// secondary viewport render routing are all available.
    pub multi_viewport: bool,
    /// Presentation and composition policy copied into every secondary viewport window.
    pub viewport_window: ImguiViewportWindowConfig,
}

impl Default for ImguiBackendConfig {
    fn default() -> Self {
        Self {
            name: "dear-imgui-bevy".to_owned(),
            docking: true,
            multi_viewport: false,
            viewport_window: ImguiViewportWindowConfig::default(),
        }
    }
}

/// Observable backend state installed by [`ImguiPlugin`].
#[derive(Resource, Debug, Clone, Eq, PartialEq)]
pub struct ImguiBackendStatus {
    /// Bevy version currently targeted by this crate.
    pub bevy_target: &'static str,
    /// Rust version required by the Bevy target train.
    pub rust_target: &'static str,
    /// Whether render integration has been compiled in.
    pub render_feature_enabled: bool,
    /// Whether render-world extraction and overlay systems were installed into Bevy's `RenderApp`.
    pub render_integration_installed: bool,
    /// Whether the current backend configuration requested Dear ImGui platform windows.
    pub multi_viewport_requested: bool,
    /// Whether the Cargo feature needed to compile PlatformIO viewport callbacks is enabled.
    pub multi_viewport_feature_enabled: bool,
    /// Whether the current target can use native Bevy OS windows for Dear ImGui platform windows.
    pub native_platform_target: bool,
    /// Whether PlatformIO lifecycle callbacks can be connected to Bevy-owned window entities.
    pub viewport_lifecycle_bridge_enabled: bool,
    /// Whether input, focus, cursor, DPI, and IME feedback covers all Dear ImGui platform windows.
    pub viewport_input_feedback_enabled: bool,
    /// Whether secondary Dear ImGui viewport draw data is routed to matching Bevy window targets.
    pub viewport_render_routing_enabled: bool,
    /// Whether the backend currently wires the required Bevy OS-window platform callbacks.
    ///
    /// This remains `false` until lifecycle, input feedback, and renderer routing are all wired.
    /// The `multi-viewport` feature may install an internal lifecycle bridge before the backend is
    /// ready to advertise full Dear ImGui OS-level viewport support.
    pub multi_viewport_supported: bool,
}

impl ImguiBackendStatus {
    fn from_config(config: &ImguiBackendConfig, render_integration_installed: bool) -> Self {
        let viewport_lifecycle_bridge_enabled =
            config.multi_viewport && MULTI_VIEWPORT_FEATURE_ENABLED && NATIVE_PLATFORM_TARGET;
        let viewport_input_feedback_enabled =
            config.multi_viewport && MULTI_VIEWPORT_FEATURE_ENABLED && NATIVE_PLATFORM_TARGET;
        let viewport_render_routing_enabled = config.multi_viewport
            && MULTI_VIEWPORT_FEATURE_ENABLED
            && NATIVE_PLATFORM_TARGET
            && render_integration_installed;

        Self {
            bevy_target: BEVY_TARGET_VERSION,
            rust_target: RUST_TARGET_VERSION,
            render_feature_enabled: cfg!(feature = "render"),
            render_integration_installed,
            multi_viewport_requested: config.multi_viewport,
            multi_viewport_feature_enabled: MULTI_VIEWPORT_FEATURE_ENABLED,
            native_platform_target: NATIVE_PLATFORM_TARGET,
            viewport_lifecycle_bridge_enabled,
            viewport_input_feedback_enabled,
            viewport_render_routing_enabled,
            multi_viewport_supported: viewport_lifecycle_bridge_enabled
                && viewport_input_feedback_enabled
                && viewport_render_routing_enabled,
        }
    }
}

impl Default for ImguiBackendStatus {
    fn default() -> Self {
        Self::from_config(&ImguiBackendConfig::default(), false)
    }
}

/// Non-send Bevy resource that owns the Dear ImGui context.
///
/// Dear ImGui has process-global current-context state and `dear_imgui_rs::Context` is intentionally
/// not `Send`/`Sync`. Storing it as a Bevy non-send resource keeps UI lifecycle work on the main
/// thread until later tasks add schedule-specific accessors.
#[cfg(feature = "render")]
#[derive(Clone, Copy)]
struct ImguiRendererRuntimeContract {
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

struct ImguiBackendOwnership {
    flags_added: dear_imgui_rs::BackendFlags,
    platform_name: Option<String>,
    platform_name_ptr: *const c_char,
    renderer_name: Option<String>,
    renderer_name_ptr: *const c_char,
    standard_draw_callbacks: bool,
    viewport_contract: bool,
    #[cfg(feature = "render")]
    renderer_contract: Option<ImguiRendererRuntimeContract>,
    #[cfg(feature = "render")]
    renderer_fault: Option<ImguiRendererOwnershipError>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
enum ImguiViewportBridgeLifecycle {
    Detached,
    Attached(viewport::ImguiViewportBridgeKeepalive),
    EcsReleasePending(viewport::ImguiViewportBridgeKeepalive),
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

pub struct ImguiContext {
    context: dear_imgui_rs::Context,
    frame_lifecycle: Rc<context::ImguiFrameLifecycleControl>,
    backend_ownership: ImguiBackendOwnership,
    #[cfg(feature = "render")]
    renderer_consumer: Option<dear_imgui_rs::render::RendererConsumer>,
    #[cfg(feature = "render")]
    renderer_release: Option<render::ImguiRendererRelease>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    viewport_bridge: ImguiViewportBridgeLifecycle,
}

/// Reason a Bevy-owned Context could not be detached yet.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ImguiContextIntoInnerErrorReason {
    /// The Bevy render world still owns managed GPU resources and must run its cleanup schedule.
    RenderWorldReleasePending,
    /// The core renderer-consumer contract rejected the requested detachment.
    Renderer(dear_imgui_rs::render::RendererConsumerError),
    /// A foreign integration replaced only part of the Bevy renderer contract.
    #[cfg(feature = "render")]
    RendererOwnership(ImguiRendererOwnershipError),
    /// A foreign callback replaced part of the Bevy viewport bridge during its lifetime.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportCallbackOwnership(viewport::ImguiViewportCallbackOwnershipError),
    /// Secondary Bevy window or camera entities must be released before extraction can finish.
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportWorldReleasePending,
}

impl std::fmt::Display for ImguiContextIntoInnerErrorReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RenderWorldReleasePending => formatter.write_str(
                "Bevy render-world resources are still live; run the render schedule and retry Context extraction",
            ),
            Self::Renderer(error) => error.fmt(formatter),
            #[cfg(feature = "render")]
            Self::RendererOwnership(error) => error.fmt(formatter),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportCallbackOwnership(error) => error.fmt(formatter),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Self::ViewportWorldReleasePending => formatter.write_str(
                "Bevy secondary viewport entities are still live; return this owner to the World, run one update, and retry Context extraction",
            ),
        }
    }
}

impl std::error::Error for ImguiContextIntoInnerErrorReason {}

/// Retryable failure returned when a Bevy-owned Context cannot be detached yet.
pub struct ImguiContextIntoInnerError {
    error: ImguiContextIntoInnerErrorReason,
    owner: ImguiContext,
}

impl ImguiContextIntoInnerError {
    /// Lifecycle reason that prevented detachment.
    #[must_use]
    pub fn error(&self) -> ImguiContextIntoInnerErrorReason {
        self.error
    }

    /// Recover the still-owned wrapper, complete or abandon pending work, and retry shutdown.
    #[must_use]
    pub fn into_owner(self) -> ImguiContext {
        self.owner
    }
}

impl std::fmt::Debug for ImguiContextIntoInnerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImguiContextIntoInnerError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for ImguiContextIntoInnerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for ImguiContextIntoInnerError {}

impl ImguiContext {
    /// Wrap an existing Dear ImGui context for insertion into a Bevy world.
    #[must_use]
    pub fn new(context: dear_imgui_rs::Context) -> Self {
        Self {
            context,
            frame_lifecycle: Rc::new(context::ImguiFrameLifecycleControl::default()),
            backend_ownership: ImguiBackendOwnership::default(),
            #[cfg(feature = "render")]
            renderer_consumer: None,
            #[cfg(feature = "render")]
            renderer_release: None,
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            viewport_bridge: ImguiViewportBridgeLifecycle::Detached,
        }
    }

    /// Borrow the inner Dear ImGui context.
    #[must_use]
    pub fn context(&self) -> &dear_imgui_rs::Context {
        &self.context
    }

    /// Mutably borrow the inner Dear ImGui context.
    ///
    /// # Panics
    ///
    /// Panics while `ImguiPrimaryContextPass` exposes a live [`dear_imgui_rs::Ui`]. Use
    /// [`ImguiContexts`] for UI work and mutate the Context outside that schedule.
    #[must_use]
    pub fn context_mut(&mut self) -> &mut dear_imgui_rs::Context {
        assert!(
            !self.frame_lifecycle.is_frame_open(),
            "ImguiContext::context_mut() is unavailable while ImguiPrimaryContextPass exposes a live Ui"
        );
        self.frame_lifecycle.revoke();
        &mut self.context
    }

    pub(crate) fn frame_lifecycle_control(&self) -> Rc<context::ImguiFrameLifecycleControl> {
        Rc::clone(&self.frame_lifecycle)
    }

    fn revoke_frame_access(&mut self) {
        self.frame_lifecycle.revoke();
        let _ = self.context.end_frame();
    }

    /// Run teardown work while this wrapper's native Dear ImGui context is current.
    ///
    /// Bevy normally owns the active context for its schedules, but a host can temporarily bind a
    /// different context through [`dear_imgui_rs::ContextBinding`]. Core renderer operations and
    /// frame finalization deliberately require their owner to be current, so teardown must bind
    /// the whole transaction rather than only the initial `EndFrame` call.
    fn with_bound_context<R>(&mut self, operation: impl FnOnce(&mut Self) -> R) -> R {
        let binding = self.context.binding();
        binding.with_bound_context(|| operation(self))
    }

    #[cfg(feature = "render")]
    pub(crate) fn attach_renderer_release(&mut self, release: render::ImguiRendererRelease) {
        self.renderer_release = Some(release);
    }

    #[cfg(feature = "render")]
    pub(crate) fn assert_active_renderer_ownership(&mut self) {
        let Some(expected) = self.backend_ownership.renderer_contract else {
            return;
        };
        let actual = ImguiRendererRuntimeContract::capture(&self.context);
        let error = self
            .backend_ownership
            .renderer_fault
            .or_else(|| expected.first_drift(actual));
        let Some(error) = error else {
            return;
        };
        self.backend_ownership.renderer_fault.get_or_insert(error);
        if expected.retains_any_identity(actual) {
            let io = self.context.io_mut();
            let owned_flags = dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET;
            io.set_backend_flags(io.backend_flags() & !owned_flags);
        }
        panic!("dear-imgui-bevy renderer ownership changed: {error}");
    }

    #[cfg(feature = "render")]
    fn renderer_capabilities_still_owned(&self) -> bool {
        self.backend_ownership
            .renderer_contract
            .is_some_and(|expected| {
                expected.retains_any_identity(ImguiRendererRuntimeContract::capture(&self.context))
            })
    }

    #[cfg(feature = "render")]
    fn record_renderer_runtime_contract(&mut self) {
        self.backend_ownership.renderer_contract = self
            .backend_ownership
            .standard_draw_callbacks
            .then(|| ImguiRendererRuntimeContract::capture(&self.context));
    }

    #[cfg(feature = "render")]
    fn preflight_renderer_teardown_ownership(&mut self) -> Result<(), ImguiRendererOwnershipError> {
        if let Some(error) = self.backend_ownership.renderer_fault.take() {
            return Err(error);
        }
        let Some(expected) = self.backend_ownership.renderer_contract else {
            return Ok(());
        };
        let actual = ImguiRendererRuntimeContract::capture(&self.context);
        let Some(error) = expected.first_drift(actual) else {
            return Ok(());
        };
        if expected.retains_any_identity(actual) {
            return Err(error);
        }
        Ok(())
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn mark_viewport_backend_owned(&mut self) {
        let flags = dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        debug_assert!((self.context.io().backend_flags() & flags).is_empty());
        self.backend_ownership.flags_added |= flags;
        self.backend_ownership.viewport_contract = true;
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn attach_viewport_bridge(
        &mut self,
        keepalive: viewport::ImguiViewportBridgeKeepalive,
    ) {
        assert!(
            matches!(self.viewport_bridge, ImguiViewportBridgeLifecycle::Detached),
            "dear-imgui-bevy viewport bridge was attached more than once"
        );
        self.mark_viewport_backend_owned();
        self.viewport_bridge = ImguiViewportBridgeLifecycle::Attached(keepalive);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn assert_attached_viewport_callback_ownership(&mut self) {
        if let ImguiViewportBridgeLifecycle::Attached(keepalive) = &self.viewport_bridge {
            viewport::platform_callback_ownership(&mut self.context, keepalive).unwrap_or_else(
                |error| panic!("dear-imgui-bevy viewport callback ownership changed: {error}"),
            );
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn record_attached_viewport_runtime_contract(&mut self) {
        if let ImguiViewportBridgeLifecycle::Attached(keepalive) = &self.viewport_bridge {
            viewport::record_platform_runtime_contract(&mut self.context, keepalive);
        }
    }

    #[cfg(all(
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    fn assert_viewport_callback_ownership(&mut self) {
        let ImguiViewportBridgeLifecycle::Attached(keepalive) = &self.viewport_bridge else {
            panic!("dear-imgui-bevy viewport bridge is not attached");
        };
        viewport::platform_callback_ownership(&mut self.context, keepalive).unwrap_or_else(
            |error| panic!("dear-imgui-bevy viewport callback ownership changed: {error}"),
        );
    }

    /// Consume the wrapper and return the Dear ImGui context.
    ///
    /// This resets every Bevy-owned texture binding before releasing the Bevy renderer consumer.
    /// A complete foreign renderer takeover preserves its raw renderer fields but receives fresh
    /// managed-texture requests; a partial takeover returns a typed error before frame or renderer
    /// state changes. The operation also fails while the render world still owns a detached frame;
    /// in either case the returned error retains this wrapper so the caller can resolve pending
    /// work and retry without reconstructing ownership.
    pub fn into_inner(mut self) -> Result<dear_imgui_rs::Context, ImguiContextIntoInnerError> {
        let teardown = self.with_bound_context(|this| {
            #[cfg(feature = "render")]
            this.preflight_renderer_teardown_ownership()
                .map_err(ImguiContextIntoInnerErrorReason::RendererOwnership)?;
            #[cfg(feature = "render")]
            if this
                .renderer_release
                .as_ref()
                .is_some_and(|release| !release.request_release())
            {
                return Err(ImguiContextIntoInnerErrorReason::RenderWorldReleasePending);
            }
            // Keep a retryable owner completely intact until the render world has accepted the
            // release request. Once preflight succeeds, revoke Rust UI access before ending the
            // native frame and entering the irreversible texture-reset transaction.
            this.revoke_frame_access();
            this.commit_renderer_texture_reset_after_release()
                .map_err(ImguiContextIntoInnerErrorReason::Renderer)?;
            this.detach_renderer_consumer()
                .map_err(ImguiContextIntoInnerErrorReason::Renderer)?;
            this.clear_backend_data()
        });
        if let Err(error) = teardown {
            return Err(ImguiContextIntoInnerError { error, owner: self });
        }
        let mut this = std::mem::ManuallyDrop::new(self);
        // SAFETY: `this` will not run `Drop`, and we return ownership of the inner context to the
        // caller exactly once. Every remaining field is dropped explicitly below.
        let context = unsafe { std::ptr::read(&this.context) };
        unsafe {
            std::ptr::drop_in_place(&mut this.frame_lifecycle);
            std::ptr::drop_in_place(&mut this.backend_ownership);
            #[cfg(feature = "render")]
            std::ptr::drop_in_place(&mut this.renderer_consumer);
            #[cfg(feature = "render")]
            std::ptr::drop_in_place(&mut this.renderer_release);
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            std::ptr::drop_in_place(&mut this.viewport_bridge);
        }
        Ok(context)
    }

    fn clear_backend_data(&mut self) -> Result<(), ImguiContextIntoInnerErrorReason> {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let (viewport_capabilities_still_owned, viewport_result) = self.advance_viewport_release();
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            if self.backend_ownership.viewport_contract && viewport_capabilities_still_owned {
                let mut config_flags = self.context.io().config_flags();
                config_flags.remove(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE);
                self.context.io_mut().set_config_flags(config_flags);
            }
            self.backend_ownership.viewport_contract = false;
        }

        #[cfg(feature = "render")]
        let renderer_capabilities_still_owned = self.renderer_capabilities_still_owned();
        #[cfg(feature = "render")]
        if self.backend_ownership.standard_draw_callbacks {
            render::clear_standard_draw_callbacks_if_owned(&mut self.context);
            self.backend_ownership.standard_draw_callbacks = false;
        }
        #[cfg(feature = "render")]
        {
            self.backend_ownership.renderer_contract = None;
        }

        let flags_added = std::mem::replace(
            &mut self.backend_ownership.flags_added,
            dear_imgui_rs::BackendFlags::empty(),
        );
        let mut flags_to_clear = flags_added;
        #[cfg(feature = "render")]
        if !renderer_capabilities_still_owned {
            flags_to_clear.remove(
                dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES
                    | dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET,
            );
        }
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if !viewport_capabilities_still_owned {
            flags_to_clear.remove(
                dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
                    | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
                    | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
            );
        }
        let current_flags = self.context.io().backend_flags();
        self.context
            .io_mut()
            .set_backend_flags(current_flags & !flags_to_clear);

        clear_backend_name_if_owned(
            &mut self.context,
            &mut self.backend_ownership.platform_name,
            &mut self.backend_ownership.platform_name_ptr,
            BackendNameKind::Platform,
        );
        clear_backend_name_if_owned(
            &mut self.context,
            &mut self.backend_ownership.renderer_name,
            &mut self.backend_ownership.renderer_name_ptr,
            BackendNameKind::Renderer,
        );

        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        viewport_result?;
        Ok(())
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn advance_viewport_release(&mut self) -> (bool, Result<(), ImguiContextIntoInnerErrorReason>) {
        let lifecycle = std::mem::replace(
            &mut self.viewport_bridge,
            ImguiViewportBridgeLifecycle::Detached,
        );
        match lifecycle {
            ImguiViewportBridgeLifecycle::Detached => (false, Ok(())),
            ImguiViewportBridgeLifecycle::Attached(keepalive) => {
                let capabilities_still_owned =
                    viewport::platform_capabilities_still_owned(&mut self.context, &keepalive);
                let ownership_error =
                    viewport::detach_owned_bridge(&mut self.context, &keepalive).err();
                let ecs_release_pending = viewport::viewport_ecs_release_pending(&keepalive);
                if ownership_error.is_some() || ecs_release_pending {
                    self.viewport_bridge =
                        ImguiViewportBridgeLifecycle::EcsReleasePending(keepalive);
                } else {
                    viewport::finish_viewport_ecs_release(&keepalive);
                }

                if let Some(error) = ownership_error {
                    return (
                        capabilities_still_owned,
                        Err(ImguiContextIntoInnerErrorReason::ViewportCallbackOwnership(
                            error,
                        )),
                    );
                }
                if ecs_release_pending {
                    return (
                        capabilities_still_owned,
                        Err(ImguiContextIntoInnerErrorReason::ViewportWorldReleasePending),
                    );
                }
                (capabilities_still_owned, Ok(()))
            }
            ImguiViewportBridgeLifecycle::EcsReleasePending(keepalive) => {
                let capabilities_still_owned =
                    viewport::platform_capabilities_still_owned(&mut self.context, &keepalive);
                if viewport::viewport_ecs_release_pending(&keepalive) {
                    self.viewport_bridge =
                        ImguiViewportBridgeLifecycle::EcsReleasePending(keepalive);
                    return (
                        capabilities_still_owned,
                        Err(ImguiContextIntoInnerErrorReason::ViewportWorldReleasePending),
                    );
                }
                viewport::finish_viewport_ecs_release(&keepalive);
                (capabilities_still_owned, Ok(()))
            }
        }
    }

    #[cfg(feature = "render")]
    fn ensure_renderer_consumer(
        &mut self,
    ) -> Result<(), dear_imgui_rs::render::RendererConsumerError> {
        if self.renderer_consumer.is_none() {
            let consumer = self.context.create_renderer_consumer()?;
            // A newly-created Bevy consumer has not published any Bevy GPU resources. Preparing
            // and committing the empty reset transaction still clears stale native bindings from
            // a prior renderer generation before this consumer starts producing snapshots.
            let reset = match self.context.prepare_renderer_texture_reset(&consumer) {
                Ok(reset) => reset,
                Err(error) => {
                    drop(consumer);
                    let _ = self.context.poll_snapshot_completions();
                    return Err(error);
                }
            };
            let _ = reset.commit();
            self.renderer_consumer = Some(consumer);
        }
        Ok(())
    }

    #[cfg(feature = "render")]
    pub(crate) fn render_frame_snapshot(
        &mut self,
        multi_viewport_supported: bool,
    ) -> Result<dear_imgui_rs::render::FrameSnapshot, dear_imgui_rs::render::SnapshotError> {
        self.ensure_renderer_consumer()?;
        #[cfg(feature = "multi-viewport")]
        if multi_viewport_supported {
            #[cfg(not(target_arch = "wasm32"))]
            self.assert_viewport_callback_ownership();
            let snapshot = {
                let consumer = self
                    .renderer_consumer
                    .as_ref()
                    .expect("renderer consumer was initialized");
                self.context.render_platform_viewport_snapshot(consumer)?
            };
            #[cfg(not(target_arch = "wasm32"))]
            self.assert_viewport_callback_ownership();
            #[cfg(not(target_arch = "wasm32"))]
            self.context.update_platform_windows();
            return Ok(snapshot);
        }
        #[cfg(not(feature = "multi-viewport"))]
        let _ = multi_viewport_supported;
        let consumer = self
            .renderer_consumer
            .as_ref()
            .expect("renderer consumer was initialized");
        self.context.render_snapshot(consumer)
    }

    fn commit_renderer_texture_reset_after_release(
        &mut self,
    ) -> Result<(), dear_imgui_rs::render::RendererConsumerError> {
        #[cfg(feature = "render")]
        if let Some(consumer) = self.renderer_consumer.as_ref() {
            // `into_inner` reaches this point only after the render world's release generation
            // acknowledged destruction of every Bevy-managed GPU resource.
            let reset = self.context.prepare_renderer_texture_reset(consumer)?;
            let _ = reset.commit();
        }
        Ok(())
    }

    fn detach_renderer_consumer(
        &mut self,
    ) -> Result<(), dear_imgui_rs::render::RendererConsumerError> {
        #[cfg(feature = "render")]
        {
            drop(self.renderer_consumer.take());
            let _ = self.context.poll_snapshot_completions()?;
        }
        Ok(())
    }
}

impl Drop for ImguiContext {
    fn drop(&mut self) {
        self.with_bound_context(|this| {
            this.revoke_frame_access();
            #[cfg(feature = "render")]
            if let Some(release) = this.renderer_release.as_ref() {
                let _ = release.request_release();
            }
            let _ = this.clear_backend_data();
        });
    }
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

/// Current Bevy version targeted by this crate.
pub const BEVY_TARGET_VERSION: &str = "0.19.0";
/// Bevy reference commit used by the workstream.
pub const BEVY_TARGET_COMMIT: &str = "c6f634ca9f406d68ba5109d921247b654cb42c10";
/// Rust version required by the first Bevy target train.
pub const RUST_TARGET_VERSION: &str = "1.95.0";
/// WGPU version used by Bevy `0.19.0`.
pub const WGPU_TARGET_VERSION: &str = "29.0.3";

#[cfg(feature = "render")]
pub mod render;
