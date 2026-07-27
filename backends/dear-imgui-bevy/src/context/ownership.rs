//! Dear ImGui Context ownership and backend attachment contracts.
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

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::World;
use bevy_ecs::resource::Resource;
#[cfg(feature = "render")]
use std::ffi::c_void;
use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::c_char,
    mem::ManuallyDrop,
    rc::{Rc, Weak},
};

use crate::context::{ImguiContextConfig, ImguiContextError, ImguiContexts};
#[cfg(feature = "render")]
use crate::render;
#[cfg(feature = "render")]
use crate::route;
use crate::viewport::ImguiViewportWindowConfig;
use crate::{BEVY_TARGET_VERSION, RUST_TARGET_VERSION};
use crate::{input, schedule, viewport};

const MULTI_VIEWPORT_FEATURE_ENABLED: bool = cfg!(feature = "multi-viewport");
const NATIVE_PLATFORM_TARGET: bool = !cfg!(target_arch = "wasm32");

/// Bevy plugin that installs the minimal Dear ImGui resources.
///
/// Later workstream tasks add input collection, frame scheduling, render extraction, and renderer
/// systems. For now the plugin establishes ownership boundaries and resource locations only.
#[derive(Debug, Clone, Default)]
pub struct ImguiPlugin {
    config: ImguiBackendConfig,
    #[cfg(feature = "render")]
    ui_render_order: render::ImguiUiRenderOrder,
}

impl ImguiPlugin {
    /// Create a plugin with explicit backend configuration.
    #[must_use]
    pub fn new(config: ImguiBackendConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Borrow the plugin configuration.
    #[must_use]
    pub fn config(&self) -> &ImguiBackendConfig {
        &self.config
    }

    /// Configure whether Dear ImGui or Bevy UI is drawn on top for the same camera.
    ///
    /// This setting takes effect when the `bevy-ui` Cargo feature is enabled.
    #[cfg(feature = "render")]
    #[must_use]
    pub fn with_ui_render_order(mut self, order: render::ImguiUiRenderOrder) -> Self {
        self.ui_render_order = order;
        self
    }

    /// Return the configured Dear ImGui/Bevy UI draw order.
    #[cfg(feature = "render")]
    #[must_use]
    pub fn ui_render_order(&self) -> render::ImguiUiRenderOrder {
        self.ui_render_order
    }
}

impl Plugin for ImguiPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<ImguiBackendConfig>() {
            app.insert_resource(self.config.clone());
        }
        if app.world().get_non_send::<ImguiContexts>().is_none() {
            app.insert_non_send(ImguiContexts::with_primary(
                dear_imgui_rs::SuspendedContext::create(),
            ));
        }
        schedule::install_imgui_schedules(app);
        #[cfg(feature = "render")]
        route::install_route_resolution(app);
        input::install_input_mapping(app);
        crate::context::install_context_lifecycle(app);
        #[cfg(feature = "render")]
        crate::texture::install_texture_leases(app);
        #[cfg(feature = "render")]
        let render_integration_available = render::render_integration_available(app);
        #[cfg(not(feature = "render"))]
        let render_integration_available = false;
        #[cfg(feature = "render")]
        let render_integration_installed =
            render::install_render_extraction(app, self.ui_render_order);
        #[cfg(not(feature = "render"))]
        let render_integration_installed = false;
        debug_assert_eq!(render_integration_installed, render_integration_available);
        viewport::install_viewport_bridge(app);
        refresh_backend_status(app, render_integration_installed);
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        {
            let render_integration_installed =
                render::install_render_extraction(_app, self.ui_render_order);
            refresh_backend_status(_app, render_integration_installed);
        }
    }
}

fn refresh_backend_status(app: &mut App, render_integration_installed: bool) {
    let effective_config = app.world().resource::<ImguiBackendConfig>().clone();
    #[cfg(feature = "render")]
    let renderer_releases = render_integration_installed.then(|| {
        app.world()
            .resource::<render::ImguiRendererReleases>()
            .clone()
    });
    let attachment = BackendAttachment {
        config: effective_config.clone(),
        render_integration_installed,
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        viewport_bridge_registration: app
            .world()
            .get_non_send::<viewport::ImguiViewportBridge>()
            .map(viewport::ImguiViewportBridge::registration),
        #[cfg(feature = "render")]
        renderer_releases,
    };
    let mut contexts = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin must retain its Context registry");
    contexts.set_primary_contract(effective_config.docking, effective_config.multi_viewport);
    contexts.attach_backend(attachment).unwrap_or_else(|error| {
        panic!("ImguiPlugin could not attach the Dear ImGui Context registry: {error}")
    });
    app.insert_resource(ImguiBackendStatus::from_config(
        &effective_config,
        render_integration_installed,
    ));
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

pub(crate) enum ImguiActiveRendererContextError<E> {
    Operation(E),
    #[cfg(feature = "render")]
    RendererOwnership(ImguiRendererOwnershipError),
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportBridge(viewport::ImguiViewportBridgeError),
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImguiViewportBridgePhase {
    Detached,
    Attached,
    EcsReleasePending,
    ViewportDrained,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct ImguiViewportBridgeOwner {
    keepalive: viewport::ImguiViewportBridgeKeepalive,
    attachment: dear_imgui_rs::ContextAttachmentLease,
    registration: Option<viewport::ImguiViewportBridgeRegistration>,
    context_id: dear_imgui_rs::ContextId,
    capabilities_still_owned: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct ImguiViewportBridgeLifecycle {
    phase: ImguiViewportBridgePhase,
    owner: Option<ImguiViewportBridgeOwner>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl Default for ImguiViewportBridgeLifecycle {
    fn default() -> Self {
        Self {
            phase: ImguiViewportBridgePhase::Detached,
            owner: None,
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeLifecycle {
    fn attached_keepalive(&self) -> Option<&viewport::ImguiViewportBridgeKeepalive> {
        (self.phase == ImguiViewportBridgePhase::Attached).then(|| {
            &self
                .owner
                .as_ref()
                .expect("an attached bridge must retain its owner")
                .keepalive
        })
    }

    fn is_detached(&self) -> bool {
        self.phase == ImguiViewportBridgePhase::Detached
    }
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

#[derive(Clone)]
pub(crate) struct BackendAttachment {
    pub(crate) config: ImguiBackendConfig,
    pub(crate) render_integration_installed: bool,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) viewport_bridge_registration: Option<viewport::ImguiViewportBridgeRegistration>,
    #[cfg(feature = "render")]
    pub(crate) renderer_releases: Option<render::ImguiRendererReleases>,
}

/// Reason a registered Context cannot finish Context-local teardown yet.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ImguiContextIntoInnerErrorReason {
    RenderWorldReleasePending,
    Renderer(dear_imgui_rs::render::RendererConsumerError),
    #[cfg(feature = "render")]
    RendererOwnership(ImguiRendererOwnershipError),
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportCallbackOwnership(viewport::ImguiViewportCallbackOwnershipError),
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    ViewportWorldReleasePending,
}

impl std::fmt::Display for ImguiContextIntoInnerErrorReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

impl std::error::Error for ImguiContextIntoInnerErrorReason {}

struct ImguiContextRetirementQueue {
    pending: RefCell<VecDeque<ContextRetirement>>,
    #[cfg(feature = "render")]
    snapshot_mailbox: RefCell<Option<super::ImguiFrameMailbox>>,
}

impl Default for ImguiContextRetirementQueue {
    fn default() -> Self {
        Self {
            pending: RefCell::new(VecDeque::new()),
            #[cfg(feature = "render")]
            snapshot_mailbox: RefCell::new(None),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ImguiContextRetirementSink {
    queue: Weak<ImguiContextRetirementQueue>,
}

impl Default for ImguiContextRetirementSink {
    fn default() -> Self {
        Self { queue: Weak::new() }
    }
}

impl ImguiContextRetirementSink {
    fn try_enqueue(
        &self,
        owner: ManuallyDrop<ContextOwner>,
    ) -> Result<(), ManuallyDrop<ContextOwner>> {
        let Some(queue) = self.queue.upgrade() else {
            return Err(owner);
        };
        let Ok(mut pending) = queue.pending.try_borrow_mut() else {
            return Err(owner);
        };
        pending.push_back(ContextRetirement {
            owner: Some(owner),
            sink: self.clone(),
        });
        Ok(())
    }

    fn try_pop_front(&self) -> Option<ContextRetirement> {
        let queue = self.queue.upgrade()?;
        let mut pending = queue.pending.try_borrow_mut().ok()?;
        pending.pop_front()
    }

    fn pending_len(&self) -> usize {
        let Some(queue) = self.queue.upgrade() else {
            return 0;
        };
        let pending = queue
            .pending
            .try_borrow()
            .map_or(0, |pending| pending.len());
        pending
    }

    #[cfg(feature = "render")]
    fn set_snapshot_mailbox(&self, mailbox: super::ImguiFrameMailbox) {
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        if let Ok(mut installed) = queue.snapshot_mailbox.try_borrow_mut() {
            *installed = Some(mailbox);
        }
    }

    #[cfg(feature = "render")]
    fn snapshot_mailbox(&self) -> Option<super::ImguiFrameMailbox> {
        let queue = self.queue.upgrade()?;
        let mailbox = queue.snapshot_mailbox.try_borrow().ok()?.clone();
        mailbox
    }
}

pub(crate) struct ImguiContextRetirements {
    queue: Rc<ImguiContextRetirementQueue>,
}

impl Default for ImguiContextRetirements {
    fn default() -> Self {
        Self {
            queue: Rc::new(ImguiContextRetirementQueue::default()),
        }
    }
}

impl ImguiContextRetirements {
    pub(crate) fn sink(&self) -> ImguiContextRetirementSink {
        ImguiContextRetirementSink {
            queue: Rc::downgrade(&self.queue),
        }
    }
}

struct ContextRetirement {
    owner: Option<ManuallyDrop<ContextOwner>>,
    sink: ImguiContextRetirementSink,
}

pub(crate) fn install_context_retirements(app: &mut App) {
    if app
        .world()
        .get_non_send::<ImguiContextRetirements>()
        .is_none()
    {
        app.insert_non_send(ImguiContextRetirements::default());
    }
    let sink = app
        .world()
        .get_non_send::<ImguiContextRetirements>()
        .expect("Context retirement storage must be installed")
        .sink();
    #[cfg(feature = "render")]
    sink.set_snapshot_mailbox(app.world().resource::<super::ImguiFrameMailbox>().clone());
    if let Some(mut contexts) = app.world_mut().get_non_send_mut::<ImguiContexts>() {
        contexts.set_retirement_sink(sink);
    }
}

fn maintain_context_retirements(world: &mut World) {
    let Some(sink) = world
        .get_non_send::<ImguiContextRetirements>()
        .map(ImguiContextRetirements::sink)
    else {
        return;
    };
    let pending_at_start = sink.pending_len();
    for _ in 0..pending_at_start {
        let Some(mut retirement) = sink.try_pop_front() else {
            break;
        };
        if retirement.advance().is_ok() {
            retirement.finish();
        }
    }
}

pub(crate) fn begin_context_retirements(world: &mut World) {
    maintain_context_retirements(world);
}

pub(crate) fn finish_context_retirements(world: &mut World) {
    maintain_context_retirements(world);
}

pub(crate) struct ContextOwner {
    context: Option<dear_imgui_rs::SuspendedContext>,
    backend_ownership: ImguiBackendOwnership,
    #[cfg(feature = "render")]
    snapshot_mailbox: Option<super::ImguiFrameMailbox>,
    #[cfg(feature = "render")]
    renderer_consumer: Option<dear_imgui_rs::render::RendererConsumer>,
    #[cfg(feature = "render")]
    renderer_release: Option<render::ImguiRendererReleaseLease>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    viewport_bridge: ImguiViewportBridgeLifecycle,
    retirement_sink: Option<ImguiContextRetirementSink>,
}

impl ContextOwner {
    pub(crate) fn new(context: dear_imgui_rs::SuspendedContext) -> Self {
        Self {
            context: Some(context),
            backend_ownership: ImguiBackendOwnership::default(),
            #[cfg(feature = "render")]
            snapshot_mailbox: None,
            #[cfg(feature = "render")]
            renderer_consumer: None,
            #[cfg(feature = "render")]
            renderer_release: None,
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            viewport_bridge: ImguiViewportBridgeLifecycle::default(),
            retirement_sink: None,
        }
    }

    pub(crate) fn set_retirement_sink(&mut self, sink: ImguiContextRetirementSink) {
        #[cfg(feature = "render")]
        {
            self.snapshot_mailbox = sink.snapshot_mailbox();
        }
        self.retirement_sink = Some(sink);
    }

    fn is_unattached(&self) -> bool {
        self.backend_ownership.flags_added.is_empty()
            && self.backend_ownership.platform_name.is_none()
            && self.backend_ownership.renderer_name.is_none()
            && !self.backend_ownership.standard_draw_callbacks
            && !self.backend_ownership.viewport_contract
            && {
                #[cfg(feature = "render")]
                {
                    self.renderer_consumer.is_none() && self.renderer_release.is_none()
                }
                #[cfg(not(feature = "render"))]
                {
                    true
                }
            }
            && {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                {
                    self.viewport_bridge.is_detached()
                }
                #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
                {
                    true
                }
            }
    }

    fn take_for_retirement(&mut self) -> ContextOwner {
        let sink = self.retirement_sink.clone().unwrap_or_default();
        ContextOwner {
            context: self.context.take(),
            backend_ownership: std::mem::take(&mut self.backend_ownership),
            #[cfg(feature = "render")]
            snapshot_mailbox: self.snapshot_mailbox.take(),
            #[cfg(feature = "render")]
            renderer_consumer: self.renderer_consumer.take(),
            #[cfg(feature = "render")]
            renderer_release: self.renderer_release.take(),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            viewport_bridge: std::mem::take(&mut self.viewport_bridge),
            retirement_sink: Some(sink),
        }
    }

    pub(crate) fn try_with_active_context<T, E>(
        &mut self,
        operation: impl FnOnce(&mut dear_imgui_rs::Context) -> Result<T, E>,
    ) -> Result<T, E> {
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(operation)
    }

    #[cfg(all(feature = "render", test))]
    pub(crate) fn try_with_active_renderer_context<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(
            &mut dear_imgui_rs::Context,
            Option<&dear_imgui_rs::render::RendererConsumer>,
        ) -> Result<T, E>,
    ) -> Result<T, E> {
        match self.try_with_active_renderer_context_checked(multi_viewport, operation) {
            Ok(value) => Ok(value),
            Err(ImguiActiveRendererContextError::Operation(error)) => Err(error),
            Err(ImguiActiveRendererContextError::RendererOwnership(error)) => {
                panic!("dear-imgui-bevy renderer ownership changed: {error}")
            }
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Err(ImguiActiveRendererContextError::ViewportBridge(error)) => {
                panic!("dear-imgui-bevy viewport bridge failed: {error}")
            }
        }
    }

    #[cfg(all(not(feature = "render"), test))]
    pub(crate) fn try_with_active_renderer_context<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(&mut dear_imgui_rs::Context, Option<&()>) -> Result<T, E>,
    ) -> Result<T, E> {
        match self.try_with_active_renderer_context_checked(multi_viewport, operation) {
            Ok(value) => Ok(value),
            Err(ImguiActiveRendererContextError::Operation(error)) => Err(error),
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Err(ImguiActiveRendererContextError::ViewportBridge(error)) => {
                panic!("dear-imgui-bevy viewport bridge failed: {error}")
            }
        }
    }

    #[cfg(feature = "render")]
    pub(crate) fn try_with_active_renderer_context_checked<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(
            &mut dear_imgui_rs::Context,
            Option<&dear_imgui_rs::render::RendererConsumer>,
        ) -> Result<T, E>,
    ) -> Result<T, ImguiActiveRendererContextError<E>> {
        let consumer = self.renderer_consumer.as_ref();
        let renderer_ownership = &mut self.backend_ownership;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_keepalive = if multi_viewport {
            Some(
                self.viewport_bridge
                    .attached_keepalive()
                    .expect("dear-imgui-bevy viewport bridge is not attached"),
            )
        } else {
            None
        };
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let _ = multi_viewport;
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                validate_active_renderer_ownership(context, renderer_ownership)
                    .map_err(ImguiActiveRendererContextError::RendererOwnership)?;
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                if let Some(keepalive) = viewport_keepalive {
                    validate_viewport_bridge(context, keepalive)
                        .map_err(ImguiActiveRendererContextError::ViewportBridge)?;
                }
                let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(context, consumer)
                }));
                let platform_completion =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                        if let Some(keepalive) = viewport_keepalive {
                            return complete_platform_frame_if_needed(context, keepalive);
                        }
                        Ok::<(), viewport::ImguiViewportBridgeError>(())
                    }));
                match operation {
                    Ok(result) => {
                        match platform_completion {
                            Ok(Ok(())) => {}
                            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                            Ok(Err(error)) => {
                                return Err(ImguiActiveRendererContextError::ViewportBridge(error));
                            }
                            #[cfg(not(all(
                                feature = "multi-viewport",
                                not(target_arch = "wasm32")
                            )))]
                            Ok(Err(_)) => unreachable!("platform completion is disabled"),
                            Err(payload) => std::panic::resume_unwind(payload),
                        }
                        result.map_err(ImguiActiveRendererContextError::Operation)
                    }
                    Err(payload) => {
                        drop(platform_completion);
                        std::panic::resume_unwind(payload);
                    }
                }
            })
    }

    #[cfg(not(feature = "render"))]
    pub(crate) fn try_with_active_renderer_context_checked<T, E>(
        &mut self,
        multi_viewport: bool,
        operation: impl FnOnce(&mut dear_imgui_rs::Context, Option<&()>) -> Result<T, E>,
    ) -> Result<T, ImguiActiveRendererContextError<E>> {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_keepalive = if multi_viewport {
            Some(
                self.viewport_bridge
                    .attached_keepalive()
                    .expect("dear-imgui-bevy viewport bridge is not attached"),
            )
        } else {
            None
        };
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let _ = multi_viewport;
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                if let Some(keepalive) = viewport_keepalive {
                    validate_viewport_bridge(context, keepalive)
                        .map_err(ImguiActiveRendererContextError::ViewportBridge)?;
                }
                let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(context, None)
                }));
                let platform_completion =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                        if let Some(keepalive) = viewport_keepalive {
                            return complete_platform_frame_if_needed(context, keepalive);
                        }
                        Ok::<(), viewport::ImguiViewportBridgeError>(())
                    }));
                match operation {
                    Ok(result) => {
                        match platform_completion {
                            Ok(Ok(())) => {}
                            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                            Ok(Err(error)) => {
                                return Err(ImguiActiveRendererContextError::ViewportBridge(error));
                            }
                            #[cfg(not(all(
                                feature = "multi-viewport",
                                not(target_arch = "wasm32")
                            )))]
                            Ok(Err(_)) => unreachable!("platform completion is disabled"),
                            Err(payload) => std::panic::resume_unwind(payload),
                        }
                        result.map_err(ImguiActiveRendererContextError::Operation)
                    }
                    Err(payload) => {
                        drop(platform_completion);
                        std::panic::resume_unwind(payload);
                    }
                }
            })
    }

    pub(crate) fn preflight_backend_attachment(
        &mut self,
        backend: &BackendAttachment,
        config: &ImguiContextConfig,
    ) -> Result<(), ImguiContextError> {
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let _ = config;
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();

        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if config.multi_viewport() {
            if backend.viewport_bridge_registration.is_none() {
                return Err(ImguiContextError::BackendOwnershipConflict {
                    context_id,
                    field: "ViewportBridge",
                });
            }
            match self.viewport_bridge.phase {
                ImguiViewportBridgePhase::Attached => {}
                ImguiViewportBridgePhase::EcsReleasePending
                | ImguiViewportBridgePhase::ViewportDrained => {
                    return Err(ImguiContextError::TeardownInProgress { context_id });
                }
                ImguiViewportBridgePhase::Detached => {
                    let result = self
                        .context
                        .as_mut()
                        .expect("Context owner must retain its suspended Context")
                        .try_with_active(|context| {
                            context
                                .preflight_attachment_registration::<
                                    viewport::ImguiViewportBridgeAttachmentMarker,
                                >(dear_imgui_rs::ContextAttachmentRole::Platform)
                                .map_err(|_| "ContextAttachment")?;
                            viewport::preflight_owned_platform_callbacks(context)
                                .map_err(|_| "PlatformIO")
                        });
                    if let Err(field) = result {
                        return Err(ImguiContextError::BackendOwnershipConflict {
                            context_id,
                            field,
                        });
                    }
                }
            }
        }

        let ownership = &self.backend_ownership;
        let result = self
            .context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                preflight_backend_context_claims(
                    context,
                    ownership,
                    backend.render_integration_installed,
                )
            });
        result.map_err(|field| ImguiContextError::BackendOwnershipConflict { context_id, field })
    }

    #[cfg(feature = "render")]
    pub(crate) fn preflight_renderer_admission(
        &mut self,
        backend: &BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        if !backend.render_integration_installed || self.renderer_consumer.is_some() {
            return Ok(());
        }
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| context.preflight_renderer_consumer())
            .map_err(|source| ImguiContextError::RendererAdmission { context_id, source })
    }

    #[cfg(not(feature = "render"))]
    pub(crate) fn preflight_renderer_admission(
        &mut self,
        _backend: &BackendAttachment,
    ) -> Result<(), ImguiContextError> {
        Ok(())
    }

    #[cfg(feature = "render")]
    pub(crate) fn commit_renderer_admission(&mut self, backend: &BackendAttachment) {
        if !backend.render_integration_installed || self.renderer_consumer.is_some() {
            return;
        }
        let context_id = self
            .context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id();
        let consumer = self
            .context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                let consumer = context.create_renderer_consumer().unwrap_or_else(|error| {
                    panic!("renderer admission changed after its global preflight: {error}")
                });
                let reset = context
                    .prepare_renderer_texture_reset(&consumer)
                    .unwrap_or_else(|error| {
                        panic!("a newly admitted renderer consumer must be idle: {error}")
                    });
                let _ = reset.commit();
                Ok::<_, std::convert::Infallible>(consumer)
            })
            .unwrap_or_else(|never| match never {});
        self.renderer_consumer = Some(consumer);
        let releases = backend
            .renderer_releases
            .as_ref()
            .expect("installed Bevy rendering must provide a Context release registry");
        self.renderer_release = Some(releases.admit(context_id));
    }

    #[cfg(not(feature = "render"))]
    pub(crate) fn commit_renderer_admission(&mut self, _backend: &BackendAttachment) {}

    pub(crate) fn commit_backend_attachment(
        &mut self,
        backend: &BackendAttachment,
        config: &ImguiContextConfig,
    ) -> Result<(), ImguiContextError> {
        let ownership = &mut self.backend_ownership;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_keepalive = self.viewport_bridge.attached_keepalive();
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                sync_backend_context_config(context, ownership, backend, config);
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                if let Some(keepalive) = viewport_keepalive {
                    viewport::record_owned_platform_name(context, keepalive);
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
        Ok(())
    }

    pub(crate) fn attach_backend(
        &mut self,
        backend: &BackendAttachment,
        config: &ImguiContextConfig,
    ) -> Result<(), ImguiContextError> {
        self.preflight_backend_attachment(backend, config)?;
        self.preflight_renderer_admission(backend)?;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if config.multi_viewport() {
            let registration = backend
                .viewport_bridge_registration
                .as_ref()
                .ok_or_else(|| ImguiContextError::BackendOwnershipConflict {
                    context_id: self.context_id(),
                    field: "ViewportBridge",
                })?;
            self.attach_context_viewport_bridge(registration)?;
        }
        self.commit_renderer_admission(backend);
        self.commit_backend_attachment(backend, config)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn context_id(&self) -> dear_imgui_rs::ContextId {
        self.context
            .as_ref()
            .expect("Context owner must retain its suspended Context")
            .id()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn attach_context_viewport_bridge(
        &mut self,
        registration: &viewport::ImguiViewportBridgeRegistration,
    ) -> Result<(), ImguiContextError> {
        let context_id = self.context_id();
        match self.viewport_bridge.phase {
            ImguiViewportBridgePhase::Attached => {
                let installed = &mut self
                    .viewport_bridge
                    .owner
                    .as_mut()
                    .expect("an attached bridge must retain its owner")
                    .registration;
                if installed.is_none() {
                    *installed = Some(registration.clone());
                }
                return Ok(());
            }
            ImguiViewportBridgePhase::EcsReleasePending
            | ImguiViewportBridgePhase::ViewportDrained => {
                return Err(ImguiContextError::TeardownInProgress { context_id });
            }
            ImguiViewportBridgePhase::Detached => {}
        }

        let keepalive = Rc::new(viewport::ImguiViewportBridgeShared::default());
        let attachment = self
            .try_with_active_context(|context| {
                let attachment = context
                    .register_attachment::<viewport::ImguiViewportBridgeAttachmentMarker>(
                        dear_imgui_rs::ContextAttachmentRole::Platform,
                        viewport::viewport_bridge_teardown_attachment(Rc::clone(&keepalive)),
                    )
                    .map_err(|_| "ContextAttachment")?;
                // SAFETY: the keepalive is retained by both the Context attachment and the owner
                // lifecycle before callback pointers can be observed by Dear ImGui.
                unsafe { viewport::install_owned_platform_callbacks(context, &keepalive) }
                    .map_err(|_| "PlatformIO")?;
                Ok::<_, &'static str>(attachment)
            })
            .map_err(|field| ImguiContextError::BackendOwnershipConflict { context_id, field })?;

        registration.register_context(context_id, Rc::clone(&keepalive));
        self.attach_viewport_bridge_with_registration(
            keepalive,
            attachment,
            Some(registration.clone()),
        );
        Ok(())
    }

    pub(crate) fn into_unattached_context(
        mut self,
    ) -> Result<dear_imgui_rs::SuspendedContext, Self> {
        if self.is_unattached() {
            Ok(self
                .context
                .take()
                .expect("Context owner must retain its suspended Context"))
        } else {
            Err(self)
        }
    }

    pub(crate) fn into_suspended(mut self) -> dear_imgui_rs::SuspendedContext {
        self.context
            .take()
            .expect("detached Context owner must retain its suspended Context")
    }

    #[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn attach_viewport_bridge(
        &mut self,
        keepalive: viewport::ImguiViewportBridgeKeepalive,
        attachment: dear_imgui_rs::ContextAttachmentLease,
    ) {
        self.attach_viewport_bridge_with_registration(keepalive, attachment, None);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn attach_viewport_bridge_with_registration(
        &mut self,
        keepalive: viewport::ImguiViewportBridgeKeepalive,
        attachment: dear_imgui_rs::ContextAttachmentLease,
        registration: Option<viewport::ImguiViewportBridgeRegistration>,
    ) {
        assert!(
            self.viewport_bridge.is_detached(),
            "dear-imgui-bevy viewport bridge was attached more than once"
        );
        self.backend_ownership.viewport_contract = true;
        self.backend_ownership.flags_added |= dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
            | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
        let context_id = self.context_id();
        self.viewport_bridge.owner = Some(ImguiViewportBridgeOwner {
            keepalive,
            attachment,
            registration,
            context_id,
            capabilities_still_owned: false,
        });
        self.viewport_bridge.phase = ImguiViewportBridgePhase::Attached;
    }

    #[cfg(feature = "render")]
    pub(crate) fn try_recover_renderer(
        &mut self,
    ) -> Result<(), ImguiActiveRendererContextError<dear_imgui_rs::render::RendererConsumerError>>
    {
        let renderer_consumer = self
            .renderer_consumer
            .as_ref()
            .expect("renderer recovery requires an admitted consumer");
        let renderer_release = self
            .renderer_release
            .as_ref()
            .expect("renderer recovery requires a release lease");
        let renderer_ownership = &mut self.backend_ownership;
        self.context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                validate_active_renderer_ownership(context, renderer_ownership)
                    .map_err(ImguiActiveRendererContextError::RendererOwnership)?;
                let reset = context
                    .prepare_renderer_texture_reset(renderer_consumer)
                    .map_err(ImguiActiveRendererContextError::Operation)?;
                renderer_release.release_renderer_resources();
                let _invalidated = reset.commit();
                renderer_release.finish_device_recovery();
                Ok(())
            })
    }

    pub(crate) fn try_detach_backend(&mut self) -> Result<(), ImguiContextIntoInnerErrorReason> {
        if self.context.is_none() {
            return Ok(());
        }
        #[cfg(feature = "render")]
        if let Some(snapshot_mailbox) = self.snapshot_mailbox.as_ref() {
            let context_id = self
                .context
                .as_ref()
                .expect("Context owner must retain its suspended Context")
                .id();
            snapshot_mailbox.clear(context_id);
        }
        #[cfg(feature = "render")]
        {
            let ownership = &mut self.backend_ownership;
            self.context
                .as_mut()
                .expect("Context owner must retain its suspended Context")
                .try_with_active(|context| {
                    preflight_renderer_teardown_ownership(context, ownership)
                        .map_err(ImguiContextIntoInnerErrorReason::RendererOwnership)
                })?;
        }
        #[cfg(feature = "render")]
        // Request release before ECS despawn establishes a fail-closed extraction barrier.
        let renderer_release_acknowledged = self
            .renderer_release
            .as_ref()
            .is_none_or(|release| release.request_release());
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            let viewport_bridge = &mut self.viewport_bridge;
            self.context
                .as_mut()
                .expect("Context owner must retain its suspended Context")
                .try_with_active(|context| advance_viewport_drain(context, viewport_bridge))?;
        }
        #[cfg(feature = "render")]
        if !renderer_release_acknowledged {
            return Err(ImguiContextIntoInnerErrorReason::RenderWorldReleasePending);
        }

        let ownership = &mut self.backend_ownership;
        #[cfg(feature = "render")]
        let consumer = &mut self.renderer_consumer;
        #[cfg(feature = "render")]
        let renderer_release = self.renderer_release.as_ref();
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewport_bridge = &mut self.viewport_bridge;
        let result = self
            .context
            .as_mut()
            .expect("Context owner must retain its suspended Context")
            .try_with_active(|context| {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                {
                    let viewport_capabilities_still_owned =
                        finish_viewport_detach(context, viewport_bridge);
                    clear_viewport_backend_contract(
                        context,
                        ownership,
                        viewport_capabilities_still_owned,
                    );
                }
                #[cfg(feature = "render")]
                if let Some(renderer_consumer) = consumer.as_ref() {
                    let reset = context
                        .prepare_renderer_texture_reset(renderer_consumer)
                        .map_err(ImguiContextIntoInnerErrorReason::Renderer)?;
                    renderer_release
                        .expect("an admitted Bevy renderer consumer must retain its release lease")
                        .release_renderer_resources();
                    let _invalidated = reset.commit();
                }
                #[cfg(feature = "render")]
                {
                    drop(consumer.take());
                    let _ = context
                        .poll_snapshot_completions()
                        .map_err(ImguiContextIntoInnerErrorReason::Renderer)?;
                }
                clear_backend_data(context, ownership);
                Ok(())
            });
        #[cfg(feature = "render")]
        if result.is_ok()
            && let Some(renderer_release) = self.renderer_release.take()
        {
            renderer_release.retire();
        }
        result
    }
}

impl ContextRetirement {
    fn new(owner: ContextOwner, sink: ImguiContextRetirementSink) -> Self {
        Self {
            owner: Some(ManuallyDrop::new(owner)),
            sink,
        }
    }

    fn advance(&mut self) -> Result<(), ImguiContextIntoInnerErrorReason> {
        self.owner
            .as_deref_mut()
            .expect("a pending Context retirement must retain its owner")
            .try_detach_backend()
    }

    fn finish(mut self) {
        let owner = self
            .owner
            .take()
            .expect("a completed Context retirement must retain its owner");
        let mut owner = ManuallyDrop::into_inner(owner);
        let context = owner
            .context
            .take()
            .expect("a completed Context retirement must retain its Context");
        drop(owner);
        drop(context);
    }
}

impl Drop for ContextRetirement {
    fn drop(&mut self) {
        let Some(owner) = self.owner.take() else {
            return;
        };
        // A failed enqueue intentionally leaks the complete owner. Releasing only part of it
        // would invalidate renderer or PlatformIO pointers still owned by another Bevy world.
        let _leaked = self.sink.try_enqueue(owner);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn validate_viewport_bridge(
    context: &mut dear_imgui_rs::Context,
    keepalive: &viewport::ImguiViewportBridgeKeepalive,
) -> Result<(), viewport::ImguiViewportBridgeError> {
    if let Some(error) = viewport::platform_callback_error(keepalive) {
        return Err(error);
    }
    viewport::platform_callback_ownership(context, keepalive)
        .map_err(viewport::ImguiViewportBridgeError::CallbackOwnership)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn complete_platform_frame_if_needed(
    context: &mut dear_imgui_rs::Context,
    keepalive: &viewport::ImguiViewportBridgeKeepalive,
) -> Result<(), viewport::ImguiViewportBridgeError> {
    let _ = context.end_frame();
    // SAFETY: the owner keeps this Context active and current for the whole completion check.
    let platform_frame_pending = unsafe {
        let raw = &*context.as_raw();
        raw.FrameCount > 0
            && raw.FrameCountEnded == raw.FrameCount
            && raw.FrameCountPlatformEnded < raw.FrameCount
    };
    if !platform_frame_pending {
        return viewport::platform_callback_error(keepalive).map_or(Ok(()), Err);
    }
    validate_viewport_bridge(context, keepalive)?;
    context.update_platform_windows();
    viewport::platform_callback_error(keepalive).map_or(Ok(()), Err)
}

impl Drop for ContextOwner {
    fn drop(&mut self) {
        if self.context.is_none() {
            return;
        }
        if self.retirement_sink.is_none() && self.is_unattached() {
            return;
        }
        let sink = self.retirement_sink.clone().unwrap_or_default();
        let owner = self.take_for_retirement();
        drop(ContextRetirement::new(owner, sink));
    }
}

fn preflight_backend_context_claims(
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

fn sync_backend_context_config(
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

    let imgui_name = backend.config.name.replace('\0', "?");
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
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    if let Some(_requested) = config.multi_viewport().then_some(()) {
        let _ = _requested;
    }
}

#[cfg(feature = "render")]
fn preflight_renderer_teardown_ownership(
    context: &dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
) -> Result<(), ImguiRendererOwnershipError> {
    let Some(expected) = ownership.renderer_contract else {
        ownership.renderer_fault = None;
        return Ok(());
    };
    let actual = ImguiRendererRuntimeContract::capture(context);
    let Some(error) = expected.first_drift(actual) else {
        ownership.renderer_fault = None;
        return Ok(());
    };
    if expected.retains_any_identity(actual) {
        return Err(ownership.renderer_fault.unwrap_or(error));
    }
    ownership.renderer_fault = None;
    Ok(())
}

#[cfg(feature = "render")]
fn validate_active_renderer_ownership(
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

fn clear_backend_data(context: &mut dear_imgui_rs::Context, ownership: &mut ImguiBackendOwnership) {
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

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn clear_viewport_backend_contract(
    context: &mut dear_imgui_rs::Context,
    ownership: &mut ImguiBackendOwnership,
    capabilities_still_owned: bool,
) {
    let viewport_flags = dear_imgui_rs::BackendFlags::PLATFORM_HAS_VIEWPORTS
        | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS
        | dear_imgui_rs::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
    let added_viewport_flags = ownership.flags_added & viewport_flags;
    ownership.flags_added.remove(viewport_flags);

    if ownership.viewport_contract && capabilities_still_owned {
        let mut config_flags = context.io().config_flags();
        config_flags.remove(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE);
        context.io_mut().set_config_flags(config_flags);
    }
    ownership.viewport_contract = false;

    if capabilities_still_owned {
        let current_flags = context.io().backend_flags();
        context
            .io_mut()
            .set_backend_flags(current_flags & !added_viewport_flags);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn advance_viewport_drain(
    context: &mut dear_imgui_rs::Context,
    lifecycle: &mut ImguiViewportBridgeLifecycle,
) -> Result<(), ImguiContextIntoInnerErrorReason> {
    match lifecycle.phase {
        ImguiViewportBridgePhase::Detached | ImguiViewportBridgePhase::ViewportDrained => Ok(()),
        ImguiViewportBridgePhase::Attached => {
            let owner = lifecycle
                .owner
                .as_ref()
                .expect("an attached bridge must retain its owner");
            let capabilities_still_owned =
                viewport::platform_capabilities_still_owned(context, &owner.keepalive);
            let ownership_error =
                viewport::begin_owned_bridge_release(context, &owner.keepalive).err();
            let ecs_release_pending = viewport::viewport_ecs_release_pending(&owner.keepalive);
            lifecycle
                .owner
                .as_mut()
                .expect("an attached bridge must retain its owner")
                .capabilities_still_owned = capabilities_still_owned;
            lifecycle.phase = if ecs_release_pending {
                ImguiViewportBridgePhase::EcsReleasePending
            } else {
                ImguiViewportBridgePhase::ViewportDrained
            };
            if let Some(error) = ownership_error
                && capabilities_still_owned
            {
                return Err(ImguiContextIntoInnerErrorReason::ViewportCallbackOwnership(
                    error,
                ));
            }
            if ecs_release_pending {
                return Err(ImguiContextIntoInnerErrorReason::ViewportWorldReleasePending);
            }
            Ok(())
        }
        ImguiViewportBridgePhase::EcsReleasePending => {
            let owner = lifecycle
                .owner
                .as_ref()
                .expect("a draining bridge must retain its owner");
            if viewport::viewport_ecs_release_pending(&owner.keepalive) {
                return Err(ImguiContextIntoInnerErrorReason::ViewportWorldReleasePending);
            }
            lifecycle.phase = ImguiViewportBridgePhase::ViewportDrained;
            Ok(())
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn finish_viewport_detach(
    context: &mut dear_imgui_rs::Context,
    lifecycle: &mut ImguiViewportBridgeLifecycle,
) -> bool {
    if lifecycle.phase == ImguiViewportBridgePhase::Detached {
        return false;
    }
    assert_eq!(
        lifecycle.phase,
        ImguiViewportBridgePhase::ViewportDrained,
        "viewport detach cannot finish before the ECS viewport world drains"
    );
    let owner = lifecycle
        .owner
        .as_mut()
        .expect("a drained bridge must retain its owner");
    viewport::finish_owned_bridge_release(context, &owner.keepalive);
    let _ = owner.attachment.detach();
    viewport::finish_viewport_ecs_release(&owner.keepalive);
    if let Some(registration) = owner.registration.as_ref() {
        registration.unregister_context(owner.context_id);
    }
    let capabilities_still_owned = owner.capabilities_still_owned;
    drop(lifecycle.owner.take());
    lifecycle.phase = ImguiViewportBridgePhase::Detached;
    capabilities_still_owned
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

#[cfg(all(test, feature = "render"))]
mod renderer_contract_tests {
    use super::*;

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

#[cfg(test)]
mod retirement_tests {
    use std::{
        cell::Cell,
        rc::Rc,
        sync::{Mutex, OnceLock},
    };

    use super::*;

    struct RetirementProbeMarker;

    struct RetirementProbe {
        destroyed: Rc<Cell<bool>>,
    }

    impl dear_imgui_rs::ContextAttachment for RetirementProbe {
        fn context_destroyed(&self, _context: dear_imgui_rs::ContextDestroyed) {
            self.destroyed.set(true);
        }
    }

    fn context_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn vanished_retirement_sink_leaks_the_complete_context_without_destroying_it() {
        let _guard = context_guard();
        let destroyed = Rc::new(Cell::new(false));
        let mut context = dear_imgui_rs::Context::create();
        context
            .register_attachment::<RetirementProbeMarker>(
                dear_imgui_rs::ContextAttachmentRole::Extension,
                Rc::new(RetirementProbe {
                    destroyed: Rc::clone(&destroyed),
                }),
            )
            .unwrap()
            .defer_to_context();

        let retirements = ImguiContextRetirements::default();
        let mut owner = ContextOwner::new(context.suspend());
        owner.set_retirement_sink(retirements.sink());
        drop(retirements);
        drop(owner);

        assert!(
            !destroyed.get(),
            "a vanished sink must leak ownership instead of partially destroying the Context"
        );
    }
}

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
mod tests {
    use std::rc::Rc;
    use std::sync::{Mutex, OnceLock};

    use super::*;

    fn context_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn viewport_owner() -> ContextOwner {
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_config_input_trickle_event_queue(false);
        let _ = context.font_atlas().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);

        let keepalive = Rc::new(viewport::ImguiViewportBridgeShared::default());
        let attachment = context
            .register_attachment::<viewport::ImguiViewportBridgeAttachmentMarker>(
                dear_imgui_rs::ContextAttachmentRole::Platform,
                viewport::viewport_bridge_teardown_attachment(Rc::clone(&keepalive)),
            )
            .unwrap();
        // SAFETY: the owner retains both the callback allocation and its Context attachment.
        unsafe { viewport::install_owned_platform_callbacks(&mut context, &keepalive) }.unwrap();

        let mut owner = ContextOwner::new(context.suspend());
        owner.attach_viewport_bridge(keepalive, attachment);
        owner
    }

    fn render_test_frame(context: &mut dear_imgui_rs::Context) {
        context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
            [64.0, 64.0],
            1.0 / 60.0,
        ));
        let _ = context.frame();
        let _ = context.render();
    }

    fn assert_platform_frame_completed(owner: &mut ContextOwner) {
        owner
            .try_with_active_context(|context| {
                // SAFETY: the Context is current and remains active for this inspection.
                let raw = unsafe { &*context.as_raw() };
                assert_eq!(raw.FrameCountPlatformEnded, raw.FrameCount);
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
    }

    #[test]
    fn pending_retirement_keeps_the_complete_viewport_owner_alive() {
        let _guard = context_guard();
        let retirements = ImguiContextRetirements::default();
        let mut owner = viewport_owner();
        owner.set_retirement_sink(retirements.sink());
        let keepalive = Rc::clone(
            &owner
                .viewport_bridge
                .owner
                .as_ref()
                .expect("the viewport fixture must retain its bridge owner")
                .keepalive,
        );
        let strong_count = Rc::strong_count(&keepalive);

        drop(owner);

        assert_eq!(retirements.sink().pending_len(), 1);
        assert_eq!(
            Rc::strong_count(&keepalive),
            strong_count,
            "queueing retirement must transfer rather than release the viewport payload"
        );
    }

    #[test]
    fn renderer_error_after_render_still_completes_the_platform_frame() {
        let _guard = context_guard();
        let mut owner = viewport_owner();

        let result = owner.try_with_active_renderer_context(true, |context, _consumer| {
            render_test_frame(context);
            Err::<(), _>("snapshot capture failed")
        });

        assert_eq!(result, Err("snapshot capture failed"));
        assert_platform_frame_completed(&mut owner);
    }

    #[test]
    fn renderer_panic_after_render_preserves_payload_and_completes_the_platform_frame() {
        let _guard = context_guard();
        let mut owner = viewport_owner();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ()> =
                owner.try_with_active_renderer_context(true, |context, _consumer| {
                    render_test_frame(context);
                    std::panic::panic_any(0xC0FFEE_u32);
                });
        }))
        .expect_err("the renderer panic must propagate");

        assert_eq!(panic.downcast_ref::<u32>(), Some(&0xC0FFEE));
        assert_platform_frame_completed(&mut owner);
    }

    #[test]
    fn ui_panic_ends_the_open_frame_before_platform_completion() {
        let _guard = context_guard();
        let mut owner = viewport_owner();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ()> =
                owner.try_with_active_renderer_context(true, |context, _consumer| {
                    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
                        [64.0, 64.0],
                        1.0 / 60.0,
                    ));
                    let _ = context.frame();
                    std::panic::panic_any("original UI panic");
                });
        }))
        .expect_err("the UI panic must propagate");

        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"original UI panic")
        );
        assert_platform_frame_completed(&mut owner);
    }
}
