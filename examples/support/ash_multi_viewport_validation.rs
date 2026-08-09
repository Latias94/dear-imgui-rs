//! CI-only Vulkan validation policy layered over the shared Ash/Winit lifecycle.

use super::{
    ExampleResult, RuntimeFrameDirective, RuntimeFrameUi, RuntimeInstancePolicy, RuntimeScenario,
    RuntimeValidation, VulkanAdapterInfo, run_runtime,
};
use ash::vk;
use dear_imgui_rs::{Context, Id, Ui, sys};
use std::{
    ffi::{CStr, c_void},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
use tracing::info;

pub(super) const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

pub type DrawCallback = unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd);

#[derive(Clone, Copy)]
pub(super) struct RuntimeDrawCallbacks {
    pub(super) sampler_linear: DrawCallback,
    pub(super) sampler_nearest: DrawCallback,
    pub(super) reset_render_state: DrawCallback,
}

pub(super) fn load_renderer_callbacks(context: &Context) -> ExampleResult<RuntimeDrawCallbacks> {
    let platform_io = context.platform_io();
    Ok(RuntimeDrawCallbacks {
        sampler_linear: platform_io
            .draw_callback_set_sampler_linear_raw()
            .ok_or("Ash did not publish its linear sampler callback")?,
        sampler_nearest: platform_io
            .draw_callback_set_sampler_nearest_raw()
            .ok_or("Ash did not publish its nearest sampler callback")?,
        reset_render_state: platform_io
            .draw_callback_reset_render_state_raw()
            .ok_or("Ash did not publish its reset-render-state callback")?,
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ValidationConfig {
    pub validation_enabled: bool,
    pub require_software_vulkan: bool,
}

#[derive(Debug, Default)]
pub struct ValidationState {
    warnings: AtomicU32,
    errors: AtomicU32,
    messages: Mutex<Vec<String>>,
}

impl ValidationState {
    fn record(&self, severity: vk::DebugUtilsMessageSeverityFlagsEXT, message: String) {
        if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            self.errors.fetch_add(1, Ordering::Relaxed);
        } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
            self.warnings.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut messages) = self.messages.lock()
            && messages.len() < 32
        {
            messages.push(message);
        }
    }

    pub fn warning_count(&self) -> u32 {
        self.warnings.load(Ordering::Acquire)
    }

    pub fn error_count(&self) -> u32 {
        self.errors.load(Ordering::Acquire)
    }

    pub fn diagnostics(&self) -> String {
        self.messages
            .lock()
            .map(|messages| messages.join(" | "))
            .unwrap_or_else(|_| "validation diagnostics lock was poisoned".to_owned())
    }
}

unsafe extern "system" fn validation_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    user_data: *mut c_void,
) -> vk::Bool32 {
    if callback_data.is_null() || user_data.is_null() {
        return vk::FALSE;
    }
    let state = unsafe { &*user_data.cast::<ValidationState>() };
    let message = unsafe { CStr::from_ptr((*callback_data).p_message) }
        .to_string_lossy()
        .into_owned();
    state.record(severity, message);
    vk::FALSE
}

pub(super) fn validation_messenger_info(
    state: &Arc<ValidationState>,
) -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
    vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(validation_callback))
        .user_data(Arc::as_ptr(state).cast_mut().cast())
}

pub(super) fn validate_software_adapter(adapter: &VulkanAdapterInfo) -> ExampleResult {
    if adapter.device_type != "Cpu" {
        return Err(format!(
            "Ash validation smoke requires a CPU Vulkan adapter, selected '{}' ({})",
            adapter.name, adapter.device_type
        )
        .into());
    }
    let identity = format!(
        "{} {} {}",
        adapter.name, adapter.driver, adapter.driver_info
    )
    .to_lowercase();
    if !identity.contains("lavapipe") && !identity.contains("llvmpipe") {
        return Err(format!(
            "Ash validation smoke requires Lavapipe/llvmpipe, selected '{}'",
            adapter.name
        )
        .into());
    }
    Ok(())
}

/// Renderer and platform values exposed to the private validation scenario.
pub struct AshFrameUi<'a> {
    pub ui: &'a Ui,
    pub viewport_count: usize,
    pub sampler_linear_callback: DrawCallback,
    pub sampler_nearest_callback: DrawCallback,
    pub reset_render_state_callback: DrawCallback,
}

/// Same-scope secondary viewport submission evidence produced by the Ash route.
pub struct AshSecondarySubmissions<'a> {
    pub rendered: &'a [Id],
    pub presented: &'a [Id],
}

/// Renderer completion behavior requested by a validation probe.
#[derive(Clone, Copy, Debug, Default)]
pub struct AshCompletionRequest {
    pub reject_null_fence: bool,
    pub complete_with_submitted_fence: bool,
}

/// Observable result after the prepared renderer transaction is completed.
#[derive(Clone, Copy, Debug)]
pub struct AshFrameOutcome {
    pub main_presented: bool,
    pub callback_only_zero_geometry: Option<bool>,
    pub render_state_cleared: Option<bool>,
    pub null_fence_rejected: bool,
    pub fence_completion_count: usize,
    pub texture_retirement_queue_drained: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TeardownEvidence {
    pub renderer_shutdown_complete: bool,
    pub viewport_runtime_shutdown_complete: bool,
    pub platform_shutdown_complete: bool,
    pub gpu_idle_before_teardown: bool,
}

pub(super) struct RuntimeSecondarySubmissions<'a> {
    pub(super) rendered: &'a [Id],
    pub(super) presented: &'a [Id],
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RuntimeCompletionRequest {
    pub(super) reject_null_fence: bool,
    pub(super) complete_with_submitted_fence: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeFrameOutcome {
    pub(super) main_presented: bool,
    pub(super) callback_only_zero_geometry: Option<bool>,
    pub(super) render_state_cleared: Option<bool>,
    pub(super) null_fence_rejected: bool,
    pub(super) fence_completion_count: usize,
    pub(super) texture_retirement_queue_drained: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeTeardownEvidence {
    pub(super) renderer_shutdown_complete: bool,
    pub(super) viewport_runtime_shutdown_complete: bool,
    pub(super) platform_shutdown_complete: bool,
    pub(super) gpu_idle_before_teardown: bool,
}

/// CI-only policy for validation, fault injection, and evidence collection.
pub trait AshValidationScenario: 'static {
    type Evidence;

    fn validation_config(&self) -> ValidationConfig {
        ValidationConfig::default()
    }

    fn requires_dynamic_rendering(&self) -> bool {
        false
    }

    fn requires_validation(&self) -> bool {
        false
    }

    fn initialize(
        &mut self,
        _context: &mut Context,
        _adapter: &VulkanAdapterInfo,
        _validation: Arc<ValidationState>,
    ) -> ExampleResult {
        Ok(())
    }

    fn prepare_frame(&mut self, _context: &mut Context) -> ExampleResult {
        Ok(())
    }

    fn begin_frame(&mut self) -> ExampleResult {
        Ok(())
    }

    /// Compose probe UI and return whether this frame intentionally contains callbacks only.
    fn draw_ui(&mut self, frame: AshFrameUi<'_>) -> ExampleResult<bool>;

    fn observe_secondary_submissions(&mut self, _report: AshSecondarySubmissions<'_>) {}

    fn completion_request(&self) -> AshCompletionRequest {
        AshCompletionRequest::default()
    }

    fn observe_frame_outcome(&mut self, _outcome: AshFrameOutcome) {}

    fn is_complete(&self) -> bool {
        false
    }

    fn completed_evidence(&self) -> Option<Self::Evidence> {
        None
    }

    fn finalize(_evidence: Self::Evidence, _teardown: TeardownEvidence) -> ExampleResult {
        Ok(())
    }
}

struct ValidationScenarioAdapter<S>(S);

impl<S: AshValidationScenario> RuntimeScenario for ValidationScenarioAdapter<S> {
    type Evidence = S::Evidence;

    fn instance_policy(&self) -> RuntimeInstancePolicy {
        let config = self.0.validation_config();
        RuntimeInstancePolicy {
            validation_enabled: config.validation_enabled,
            require_software_vulkan: config.require_software_vulkan,
        }
    }

    fn requires_dynamic_rendering(&self) -> bool {
        self.0.requires_dynamic_rendering()
    }

    fn requires_validation(&self) -> bool {
        self.0.requires_validation()
    }

    fn requires_renderer_callbacks(&self) -> bool {
        true
    }

    fn initialize(
        &mut self,
        context: &mut Context,
        adapter: &VulkanAdapterInfo,
        validation: &RuntimeValidation,
    ) -> ExampleResult {
        self.0
            .initialize(context, adapter, Arc::clone(&validation.state))
    }

    fn prepare_frame(&mut self, context: &mut Context) -> ExampleResult {
        self.0.prepare_frame(context)
    }

    fn begin_frame(&mut self) -> ExampleResult {
        self.0.begin_frame()
    }

    fn draw_ui(&mut self, frame: RuntimeFrameUi<'_>) -> ExampleResult<RuntimeFrameDirective> {
        let callbacks = frame
            ._callbacks
            .validation
            .ok_or("Ash validation callbacks were not initialized")?;
        let callback_only = self.0.draw_ui(AshFrameUi {
            ui: frame.ui,
            viewport_count: frame.viewport_count,
            sampler_linear_callback: callbacks.sampler_linear,
            sampler_nearest_callback: callbacks.sampler_nearest,
            reset_render_state_callback: callbacks.reset_render_state,
        })?;
        Ok(RuntimeFrameDirective { callback_only })
    }

    fn observe_secondary_submissions(&mut self, report: RuntimeSecondarySubmissions<'_>) {
        self.0
            .observe_secondary_submissions(AshSecondarySubmissions {
                rendered: report.rendered,
                presented: report.presented,
            });
    }

    fn completion_request(&self) -> RuntimeCompletionRequest {
        let request = self.0.completion_request();
        RuntimeCompletionRequest {
            reject_null_fence: request.reject_null_fence,
            complete_with_submitted_fence: request.complete_with_submitted_fence,
        }
    }

    fn observe_frame_outcome(&mut self, outcome: RuntimeFrameOutcome) {
        self.0.observe_frame_outcome(AshFrameOutcome {
            main_presented: outcome.main_presented,
            callback_only_zero_geometry: outcome.callback_only_zero_geometry,
            render_state_cleared: outcome.render_state_cleared,
            null_fence_rejected: outcome.null_fence_rejected,
            fence_completion_count: outcome.fence_completion_count,
            texture_retirement_queue_drained: outcome.texture_retirement_queue_drained,
        });
    }

    fn is_complete(&self) -> bool {
        self.0.is_complete()
    }

    fn completed_evidence(&self) -> Option<Self::Evidence> {
        self.0.completed_evidence()
    }

    fn finalize(evidence: Self::Evidence, teardown: RuntimeTeardownEvidence) -> ExampleResult {
        S::finalize(
            evidence,
            TeardownEvidence {
                renderer_shutdown_complete: teardown.renderer_shutdown_complete,
                viewport_runtime_shutdown_complete: teardown.viewport_runtime_shutdown_complete,
                platform_shutdown_complete: teardown.platform_shutdown_complete,
                gpu_idle_before_teardown: teardown.gpu_idle_before_teardown,
            },
        )
    }
}

pub fn run_validation<S: AshValidationScenario>(scenario: S) -> ExampleResult {
    dear_imgui_examples::init_tracing_with_filter(
        "dear_imgui=debug,ash_vulkan_validation_smoke=info",
    );
    info!("Starting private Ash Vulkan validation contract");
    run_runtime(ValidationScenarioAdapter(scenario))
}
