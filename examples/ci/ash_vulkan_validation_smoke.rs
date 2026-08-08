//! Private Ash/Winit Vulkan validation contract.
//!
//! This binary owns CI-only environment parsing, validation probes, scenario UI, and JSON
//! evidence. Native Winit, Vulkan, swapchain, renderer, and teardown order remain in the shared
//! Ash lifecycle used by the interactive example.

// The shared lifecycle also exposes interactive fields consumed only by the teaching example.
#[allow(dead_code)]
#[path = "../support/ash_multi_viewport.rs"]
mod ash_multi_viewport;

use ash::vk::{self, Handle as _};
use ash_multi_viewport::{
    AshCompletionRequest, AshFrameOutcome, AshFrameUi, AshSecondarySubmissions,
    AshViewportScenario, ExampleResult, TeardownEvidence, ValidationConfig, ValidationState,
    VulkanAdapterInfo,
};
use dear_imgui_ash::AshRenderState;
use dear_imgui_rs::{
    Condition, Context, Id, ManagedTextureId, OwnedTextureData, TextureDataError, TextureFormat,
    sys,
};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const SMOKE_FRAME_BUDGET: u32 = 600;

static RAW_CALLBACK_OBSERVED: AtomicBool = AtomicBool::new(false);
static CALLBACK_CONTRACT_FAILED: AtomicBool = AtomicBool::new(false);
static CALLBACK_ONLY_OBSERVED: AtomicBool = AtomicBool::new(false);
static NEAREST_SAMPLER_SET: AtomicU64 = AtomicU64::new(0);
static LINEAR_SAMPLER_SET: AtomicU64 = AtomicU64::new(0);
static RESET_AFTER_DRAW_OBSERVED: AtomicBool = AtomicBool::new(false);

fn required_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| value == "1")
}

fn smoke_texture_pixels(revision: u8) -> Vec<u8> {
    const SIDE: usize = 8;
    let mut pixels = Vec::with_capacity(SIDE * SIDE * 4);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let bright = ((x + y + usize::from(revision)) & 1) == 0;
            let (red, green, blue) = if bright {
                (240, 48u8.saturating_add(revision), 32)
            } else {
                (24, 72, 220u8.saturating_sub(revision))
            };
            pixels.extend_from_slice(&[red, green, blue, 255]);
        }
    }
    pixels
}

fn smoke_texture_data(revision: u8) -> Result<OwnedTextureData, TextureDataError> {
    OwnedTextureData::from_pixels(TextureFormat::RGBA32, 8, 8, &smoke_texture_pixels(revision))
}

unsafe extern "C" fn smoke_raw_callback(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let valid = unsafe {
        AshRenderState::with_current(|state| {
            let valid = state.command_buffer() != vk::CommandBuffer::null()
                && state.pipeline() != vk::Pipeline::null()
                && state.pipeline_layout() != vk::PipelineLayout::null()
                && state.device().handle() != vk::Device::null();
            if valid {
                state.device().cmd_set_viewport(
                    state.command_buffer(),
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
            }
            valid
        })
    }
    .unwrap_or(false);
    RAW_CALLBACK_OBSERVED.fetch_or(valid, Ordering::AcqRel);
    if !valid {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_callback_only_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let valid = unsafe {
        AshRenderState::with_current(|state| {
            state.command_buffer() != vk::CommandBuffer::null()
                && state.sampler_descriptor_set() != vk::DescriptorSet::null()
        })
    }
    .unwrap_or(false);
    CALLBACK_ONLY_OBSERVED.fetch_or(valid, Ordering::AcqRel);
    RAW_CALLBACK_OBSERVED.fetch_or(valid, Ordering::AcqRel);
    if !valid {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_nearest_sampler_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let observed =
        unsafe { AshRenderState::with_current(|state| state.sampler_descriptor_set().as_raw()) }
            .unwrap_or(0);
    if observed == 0 {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    } else {
        NEAREST_SAMPLER_SET.store(observed, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_linear_sampler_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let observed =
        unsafe { AshRenderState::with_current(|state| state.sampler_descriptor_set().as_raw()) }
            .unwrap_or(0);
    if observed == 0 {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    } else {
        LINEAR_SAMPLER_SET.store(observed, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_reset_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let expected_linear = LINEAR_SAMPLER_SET.load(Ordering::Acquire);
    let (state_valid, draw_recovered) = unsafe {
        AshRenderState::with_current(|state| {
            let state_valid = expected_linear != 0
                && state.sampler_descriptor_set().as_raw() == expected_linear
                && state.reset_count() > 0;
            (
                state_valid,
                state_valid && state.draw_commands_since_reset() > 0,
            )
        })
    }
    .unwrap_or((false, false));
    RESET_AFTER_DRAW_OBSERVED.fetch_or(draw_recovered, Ordering::AcqRel);
    if !state_valid {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokePhase {
    CallbackOnly,
    Spawn,
    Resize,
    Merge,
    Complete,
}

struct ValidationSmoke {
    validation_config: ValidationConfig,
    result_path: Option<PathBuf>,
    adapter: Option<VulkanAdapterInfo>,
    validation: Option<Arc<ValidationState>>,
    frame_count: u32,
    phase: SmokePhase,
    secondary_id: Option<Id>,
    initial_secondary_size: Option<[f32; 2]>,
    secondary_created: bool,
    secondary_resized: bool,
    merge_observed: bool,
    render_submitted_ids: Vec<u32>,
    present_submitted_ids: Vec<u32>,
    callback_only_frame_executed: bool,
    raw_callback_typed_state_observed: bool,
    nearest_sampler_descriptor_set_observed: bool,
    linear_sampler_descriptor_set_observed: bool,
    sampler_descriptor_sets_distinct: bool,
    reset_render_state_recovered: bool,
    render_state_cleared_after_callback: bool,
    managed_texture: Option<ManagedTextureId>,
    managed_texture_updated: bool,
    managed_texture_removed: bool,
    texture_retirement_null_fence_rejected: bool,
    texture_retirement_fence_completion_count: usize,
    texture_retirement_queue_drained: bool,
    main_present_completed: bool,
}

#[derive(Clone)]
struct CompletedValidationSmoke {
    result_path: Option<PathBuf>,
    adapter: VulkanAdapterInfo,
    validation: Arc<ValidationState>,
    secondary_created: bool,
    secondary_resized: bool,
    merge_observed: bool,
    render_submitted_ids: Vec<u32>,
    present_submitted_ids: Vec<u32>,
    callback_only_frame_executed: bool,
    raw_callback_typed_state_observed: bool,
    nearest_sampler_descriptor_set_observed: bool,
    linear_sampler_descriptor_set_observed: bool,
    sampler_descriptor_sets_distinct: bool,
    reset_render_state_recovered: bool,
    render_state_cleared_after_callback: bool,
    managed_texture_updated: bool,
    managed_texture_removed: bool,
    texture_retirement_null_fence_rejected: bool,
    texture_retirement_fence_completion_count: usize,
    texture_retirement_queue_drained: bool,
    main_present_completed: bool,
}

impl ValidationSmoke {
    fn from_environment() -> Self {
        Self {
            validation_config: ValidationConfig {
                validation_enabled: required_flag("DEAR_IMGUI_REQUIRE_VULKAN_VALIDATION"),
                require_software_vulkan: required_flag("DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN"),
            },
            result_path: env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            adapter: None,
            validation: None,
            frame_count: 0,
            phase: SmokePhase::CallbackOnly,
            secondary_id: None,
            initial_secondary_size: None,
            secondary_created: false,
            secondary_resized: false,
            merge_observed: false,
            render_submitted_ids: Vec::new(),
            present_submitted_ids: Vec::new(),
            callback_only_frame_executed: false,
            raw_callback_typed_state_observed: false,
            nearest_sampler_descriptor_set_observed: false,
            linear_sampler_descriptor_set_observed: false,
            sampler_descriptor_sets_distinct: false,
            reset_render_state_recovered: false,
            render_state_cleared_after_callback: false,
            managed_texture: None,
            managed_texture_updated: false,
            managed_texture_removed: false,
            texture_retirement_null_fence_rejected: false,
            texture_retirement_fence_completion_count: 0,
            texture_retirement_queue_drained: false,
            main_present_completed: false,
        }
    }
}

impl ValidationSmoke {
    fn prepare_managed_texture(&mut self, context: &mut Context) -> ExampleResult {
        match self.phase {
            SmokePhase::Spawn if !self.managed_texture_updated => {
                let texture = self
                    .managed_texture
                    .ok_or("Ash smoke managed texture disappeared before its update")?;
                let pixels = smoke_texture_pixels(37);
                context
                    .try_with_texture_mut(texture, |mut texture| texture.replace_pixels(&pixels))?;
                self.managed_texture_updated = true;
            }
            SmokePhase::Merge if !self.managed_texture_removed => {
                let texture = self
                    .managed_texture
                    .ok_or("Ash smoke managed texture disappeared before its removal")?;
                context.remove_texture(texture)?;
                self.managed_texture = None;
                self.managed_texture_removed = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn advance_frame_budget(&mut self) -> ExampleResult {
        self.frame_count = self
            .frame_count
            .checked_add(1)
            .ok_or("Ash validation smoke frame counter overflowed")?;
        if self.frame_count > SMOKE_FRAME_BUDGET {
            return Err(format!(
                "Ash validation smoke exceeded {SMOKE_FRAME_BUDGET} frames in phase {:?}; \
                 callback_only={}, raw_callback={}, nearest_sampler={}, linear_sampler={}, \
                 distinct_samplers={}, reset_after_draw={}, render_state_cleared={}, \
                 callback_contract_failed={}",
                self.phase,
                self.callback_only_frame_executed,
                self.raw_callback_typed_state_observed,
                self.nearest_sampler_descriptor_set_observed,
                self.linear_sampler_descriptor_set_observed,
                self.sampler_descriptor_sets_distinct,
                self.reset_render_state_recovered,
                self.render_state_cleared_after_callback,
                CALLBACK_CONTRACT_FAILED.load(Ordering::Acquire),
            )
            .into());
        }
        Ok(())
    }

    fn draw_validation_ui(&mut self, frame: AshFrameUi<'_>) -> bool {
        if self.phase == SmokePhase::CallbackOnly {
            let draw_list = frame.ui.get_background_draw_list();
            unsafe {
                draw_list.add_callback(frame.sampler_nearest_callback, std::ptr::null_mut(), 0);
                draw_list.add_callback(smoke_nearest_sampler_probe, std::ptr::null_mut(), 0);
                draw_list.add_callback(smoke_callback_only_probe, std::ptr::null_mut(), 0);
                draw_list.add_callback(frame.sampler_linear_callback, std::ptr::null_mut(), 0);
                draw_list.add_callback(smoke_linear_sampler_probe, std::ptr::null_mut(), 0);
            }
            return true;
        }

        let ui = frame.ui;
        let main_viewport_id = ui.main_viewport().id();
        let (position, size) = match self.phase {
            SmokePhase::CallbackOnly => unreachable!("handled above"),
            SmokePhase::Spawn => ([1500.0, 120.0], [360.0, 240.0]),
            SmokePhase::Resize => ([1500.0, 120.0], [620.0, 420.0]),
            SmokePhase::Merge | SmokePhase::Complete => {
                ui.set_next_window_viewport(main_viewport_id);
                ([720.0, 120.0], [420.0, 280.0])
            }
        };
        let mut observed_viewport_id = main_viewport_id;
        let mut observed_viewport_size = [0.0, 0.0];
        let managed_texture = self.managed_texture;
        ui.window("Ash Vulkan validation smoke")
            .position(position, Condition::Always)
            .size(size, Condition::Always)
            .build(|| {
                let viewport = ui.window_viewport();
                observed_viewport_id = viewport.id();
                observed_viewport_size = viewport.size();
                ui.text("Ash dynamic rendering validation surface");
                {
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(
                            frame.sampler_nearest_callback,
                            std::ptr::null_mut(),
                            0,
                        );
                        draw_list.add_callback(
                            smoke_nearest_sampler_probe,
                            std::ptr::null_mut(),
                            0,
                        );
                    }
                }
                if let Some(texture) = managed_texture {
                    ui.image(texture, [64.0, 64.0]);
                } else {
                    ui.text("Font atlas sampler probe");
                }
                {
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(
                            frame.sampler_linear_callback,
                            std::ptr::null_mut(),
                            0,
                        );
                        draw_list.add_callback(smoke_linear_sampler_probe, std::ptr::null_mut(), 0);
                        draw_list.add_callback(smoke_raw_callback, std::ptr::null_mut(), 0);
                        draw_list.add_callback(
                            frame.reset_render_state_callback,
                            std::ptr::null_mut(),
                            0,
                        );
                    }
                }
                ui.text("Draw after reset-render-state callback");
                {
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(smoke_reset_probe, std::ptr::null_mut(), 0);
                    }
                }
            });
        self.observe_window(
            observed_viewport_id,
            observed_viewport_size,
            main_viewport_id,
            frame.viewport_count,
        );
        false
    }

    fn observe_window(
        &mut self,
        viewport_id: Id,
        viewport_size: [f32; 2],
        main_viewport_id: Id,
        viewport_count: usize,
    ) {
        match self.phase {
            SmokePhase::Spawn if viewport_id != main_viewport_id && viewport_count > 1 => {
                self.secondary_created = true;
                self.secondary_id = Some(viewport_id);
                self.initial_secondary_size = Some(viewport_size);
            }
            SmokePhase::Resize if Some(viewport_id) == self.secondary_id => {
                if self.initial_secondary_size.is_some_and(|initial| {
                    (initial[0] - viewport_size[0]).abs() > 64.0
                        || (initial[1] - viewport_size[1]).abs() > 64.0
                }) {
                    self.secondary_resized = true;
                }
            }
            SmokePhase::Merge if viewport_id == main_viewport_id && viewport_count == 1 => {
                self.merge_observed = true;
            }
            _ => {}
        }
    }

    fn observe_submissions(&mut self, rendered: &[Id], presented: &[Id]) {
        self.render_submitted_ids
            .extend(rendered.iter().map(|id| id.raw()));
        self.render_submitted_ids.sort_unstable();
        self.render_submitted_ids.dedup();
        self.present_submitted_ids
            .extend(presented.iter().map(|id| id.raw()));
        self.present_submitted_ids.sort_unstable();
        self.present_submitted_ids.dedup();
        let secondary_presented = self.secondary_id.is_some_and(|secondary| {
            rendered.contains(&secondary) && presented.contains(&secondary)
        });
        match self.phase {
            SmokePhase::Spawn if self.secondary_created && secondary_presented => {
                self.phase = SmokePhase::Resize;
            }
            SmokePhase::Resize if self.secondary_resized && secondary_presented => {
                self.phase = SmokePhase::Merge;
            }
            _ => {}
        }
    }

    fn update_callback_evidence(
        &mut self,
        callback_only_zero_geometry: bool,
        render_state_cleared: bool,
    ) {
        let callback_failed = CALLBACK_CONTRACT_FAILED.load(Ordering::Acquire);
        self.raw_callback_typed_state_observed =
            RAW_CALLBACK_OBSERVED.load(Ordering::Acquire) && !callback_failed;
        self.callback_only_frame_executed |= callback_only_zero_geometry
            && CALLBACK_ONLY_OBSERVED.load(Ordering::Acquire)
            && !callback_failed;
        let nearest = NEAREST_SAMPLER_SET.load(Ordering::Acquire);
        let linear = LINEAR_SAMPLER_SET.load(Ordering::Acquire);
        self.nearest_sampler_descriptor_set_observed |= nearest != 0 && !callback_failed;
        self.linear_sampler_descriptor_set_observed |= linear != 0 && !callback_failed;
        self.sampler_descriptor_sets_distinct |= nearest != 0 && linear != 0 && nearest != linear;
        self.reset_render_state_recovered |=
            RESET_AFTER_DRAW_OBSERVED.load(Ordering::Acquire) && !callback_failed;
        self.render_state_cleared_after_callback |= render_state_cleared;

        if self.phase == SmokePhase::CallbackOnly
            && self.callback_only_frame_executed
            && self.raw_callback_typed_state_observed
            && self.nearest_sampler_descriptor_set_observed
            && self.linear_sampler_descriptor_set_observed
            && self.sampler_descriptor_sets_distinct
            && self.render_state_cleared_after_callback
        {
            self.phase = SmokePhase::Spawn;
        }
    }

    fn record_texture_retirement(&mut self, outcome: AshFrameOutcome) {
        self.texture_retirement_null_fence_rejected |= outcome.null_fence_rejected;
        self.texture_retirement_fence_completion_count = self
            .texture_retirement_fence_completion_count
            .saturating_add(outcome.fence_completion_count);
        self.texture_retirement_queue_drained = outcome.texture_retirement_queue_drained;
    }

    fn mark_main_presented(&mut self) {
        self.main_present_completed = true;
        if self.phase == SmokePhase::Merge
            && self.merge_observed
            && self.secondary_created
            && self.secondary_resized
            && self.callback_only_frame_executed
            && self.raw_callback_typed_state_observed
            && self.nearest_sampler_descriptor_set_observed
            && self.linear_sampler_descriptor_set_observed
            && self.sampler_descriptor_sets_distinct
            && self.reset_render_state_recovered
            && self.render_state_cleared_after_callback
            && self.managed_texture_updated
            && self.managed_texture_removed
            && self.texture_retirement_null_fence_rejected
            && self.texture_retirement_fence_completion_count >= 2
            && self.texture_retirement_queue_drained
        {
            self.phase = SmokePhase::Complete;
        }
    }

    fn completed_result(&self) -> Option<CompletedValidationSmoke> {
        (self.phase == SmokePhase::Complete).then(|| CompletedValidationSmoke {
            result_path: self.result_path.clone(),
            adapter: self
                .adapter
                .clone()
                .expect("completed Ash smoke must retain its adapter"),
            validation: Arc::clone(
                self.validation
                    .as_ref()
                    .expect("completed Ash smoke must retain validation state"),
            ),
            secondary_created: self.secondary_created,
            secondary_resized: self.secondary_resized,
            merge_observed: self.merge_observed,
            render_submitted_ids: self.render_submitted_ids.clone(),
            present_submitted_ids: self.present_submitted_ids.clone(),
            callback_only_frame_executed: self.callback_only_frame_executed,
            raw_callback_typed_state_observed: self.raw_callback_typed_state_observed,
            nearest_sampler_descriptor_set_observed: self.nearest_sampler_descriptor_set_observed,
            linear_sampler_descriptor_set_observed: self.linear_sampler_descriptor_set_observed,
            sampler_descriptor_sets_distinct: self.sampler_descriptor_sets_distinct,
            reset_render_state_recovered: self.reset_render_state_recovered,
            render_state_cleared_after_callback: self.render_state_cleared_after_callback,
            managed_texture_updated: self.managed_texture_updated,
            managed_texture_removed: self.managed_texture_removed,
            texture_retirement_null_fence_rejected: self.texture_retirement_null_fence_rejected,
            texture_retirement_fence_completion_count: self
                .texture_retirement_fence_completion_count,
            texture_retirement_queue_drained: self.texture_retirement_queue_drained,
            main_present_completed: self.main_present_completed,
        })
    }
}

impl AshViewportScenario for ValidationSmoke {
    type Evidence = CompletedValidationSmoke;

    fn validation_config(&self) -> ValidationConfig {
        self.validation_config
    }

    fn requires_dynamic_rendering(&self) -> bool {
        true
    }

    fn requires_validation(&self) -> bool {
        true
    }

    fn initialize(
        &mut self,
        context: &mut Context,
        adapter: &VulkanAdapterInfo,
        validation: Arc<ValidationState>,
    ) -> ExampleResult {
        println!(
            "Ash Vulkan adapter: name='{}', type={}, driver='{}', info='{}'",
            adapter.name, adapter.device_type, adapter.driver, adapter.driver_info,
        );
        RAW_CALLBACK_OBSERVED.store(false, Ordering::Release);
        CALLBACK_CONTRACT_FAILED.store(false, Ordering::Release);
        CALLBACK_ONLY_OBSERVED.store(false, Ordering::Release);
        NEAREST_SAMPLER_SET.store(0, Ordering::Release);
        LINEAR_SAMPLER_SET.store(0, Ordering::Release);
        RESET_AFTER_DRAW_OBSERVED.store(false, Ordering::Release);
        self.adapter = Some(adapter.clone());
        self.validation = Some(validation);
        self.managed_texture = Some(context.register_texture(smoke_texture_data(0)?));
        Ok(())
    }

    fn prepare_frame(&mut self, context: &mut Context) -> ExampleResult {
        self.prepare_managed_texture(context)
    }

    fn begin_frame(&mut self) -> ExampleResult {
        self.advance_frame_budget()
    }

    fn draw_ui(&mut self, frame: AshFrameUi<'_>) -> ExampleResult<bool> {
        Ok(self.draw_validation_ui(frame))
    }

    fn observe_secondary_submissions(&mut self, report: AshSecondarySubmissions<'_>) {
        self.observe_submissions(report.rendered, report.presented);
    }

    fn completion_request(&self) -> AshCompletionRequest {
        let reject_null_fence =
            self.managed_texture_updated && !self.texture_retirement_null_fence_rejected;
        AshCompletionRequest {
            reject_null_fence,
            complete_with_submitted_fence: !reject_null_fence,
        }
    }

    fn observe_frame_outcome(&mut self, outcome: AshFrameOutcome) {
        if let (Some(callback_only_zero_geometry), Some(render_state_cleared)) = (
            outcome.callback_only_zero_geometry,
            outcome.render_state_cleared,
        ) {
            self.update_callback_evidence(callback_only_zero_geometry, render_state_cleared);
        }
        self.record_texture_retirement(outcome);
        if outcome.main_presented {
            self.mark_main_presented();
        }
    }

    fn is_complete(&self) -> bool {
        self.phase == SmokePhase::Complete
    }

    fn completed_evidence(&self) -> Option<Self::Evidence> {
        self.completed_result()
    }

    fn finalize(evidence: Self::Evidence, teardown: TeardownEvidence) -> ExampleResult {
        evidence.write_after_teardown(teardown)?;
        if evidence.validation.warning_count() != 0 || evidence.validation.error_count() != 0 {
            return Err(format!(
                "Vulkan validation reported {} warning(s) and {} error(s): {}",
                evidence.validation.warning_count(),
                evidence.validation.error_count(),
                evidence.validation.diagnostics(),
            )
            .into());
        }
        Ok(())
    }
}

impl CompletedValidationSmoke {
    fn write_after_teardown(&self, teardown: TeardownEvidence) -> ExampleResult {
        let Some(path) = self.result_path.as_ref() else {
            return Ok(());
        };
        let payload = serde_json::json!({
            "schema_version": 2,
            "adapter": {
                "name": self.adapter.name,
                "backend": "Vulkan",
                "device_type": self.adapter.device_type,
                "driver": self.adapter.driver,
                "driver_info": self.adapter.driver_info,
                "vendor": self.adapter.vendor,
                "device": self.adapter.device,
            },
            "dynamic_rendering_enabled": cfg!(feature = "ash-dynamic-rendering"),
            "validation_layer_enabled": true,
            "secondary_viewport_created": self.secondary_created,
            "secondary_viewport_resized": self.secondary_resized,
            "merge_observed": self.merge_observed,
            "secondary_render_submitted_viewport_ids": self.render_submitted_ids,
            "secondary_present_submitted_viewport_ids": self.present_submitted_ids,
            "callback_only_frame_executed": self.callback_only_frame_executed,
            "raw_callback_typed_state_observed": self.raw_callback_typed_state_observed,
            "nearest_sampler_descriptor_set_observed":
                self.nearest_sampler_descriptor_set_observed,
            "linear_sampler_descriptor_set_observed":
                self.linear_sampler_descriptor_set_observed,
            "sampler_descriptor_sets_distinct": self.sampler_descriptor_sets_distinct,
            "reset_render_state_recovered": self.reset_render_state_recovered,
            "render_state_cleared_after_callback": self.render_state_cleared_after_callback,
            "managed_texture_updated": self.managed_texture_updated,
            "managed_texture_removed": self.managed_texture_removed,
            "texture_retirement_null_fence_rejected":
                self.texture_retirement_null_fence_rejected,
            "texture_retirement_fence_completion_count":
                self.texture_retirement_fence_completion_count,
            "texture_retirement_queue_drained": self.texture_retirement_queue_drained,
            "main_present_completed": self.main_present_completed,
            "renderer_shutdown_complete": teardown.renderer_shutdown_complete,
            "viewport_runtime_shutdown_complete": teardown.viewport_runtime_shutdown_complete,
            "platform_shutdown_complete": teardown.platform_shutdown_complete,
            "gpu_idle_before_teardown": teardown.gpu_idle_before_teardown,
            "vulkan_resources_dropped": true,
            "validation_warning_count": self.validation.warning_count(),
            "validation_error_count": self.validation.error_count(),
        });
        write_json_atomic(path, &serde_json::to_string(&payload)?)
    }
}

fn write_json_atomic(path: &Path, contents: &str) -> ExampleResult {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("DEAR_IMGUI_VIEWPORT_SMOKE_JSON must name a file")?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn main() -> ExampleResult {
    ash_multi_viewport::run(ValidationSmoke::from_environment())
}
