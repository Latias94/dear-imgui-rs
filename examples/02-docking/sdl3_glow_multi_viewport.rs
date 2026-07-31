//! SDL3 + Glow multi-viewport example.
//!
//! This example drives Dear ImGui using:
//! - SDL3 for the main window, input, and secondary platform windows;
//! - the Rust Glow renderer backend (`dear-imgui-glow`);
//! - the high-level `dear-imgui-rs` API.
//!
//! It does not use the official OpenGL3 renderer from `dear-imgui-sdl3`.
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport \
//!     --features sdl3-glow-multi-viewport
//! ```
//!
//! Automated Linux secondary-window lifecycle smoke:
//! ```text
//! python3 tools/ci/run_contract.py sdl3-glow-multi-viewport-smoke
//! ```

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
#[cfg(feature = "test-engine")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
#[cfg(feature = "test-engine")]
use dear_imgui_glow::{
    GlBuffer, GlProgram, GlSampler, GlVersion, GlVertexArray, GlowRenderState,
    GlowRenderStateAccessError, GlowSamplerStrategy,
};
use dear_imgui_glow::{
    GlTexture, GlowRenderer, SimpleTextureMap, TextureMap, create_texture_from_rgba,
    multi_viewport::GlowViewportRuntime,
};
#[cfg(feature = "test-engine")]
use dear_imgui_rs::Id;
use dear_imgui_rs::{
    Condition, ConfigFlags, Context, RawDrawCallback, TextureId,
    render::{ReconciledFrame, RenderedFrame},
};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, Sdl3PlatformBackend};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestFrameDriver, TestGroup, VerboseLevel,
};
use glow::HasContext;
use sdl3::video::{GLProfile, SwapInterval, WindowPos};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};
use std::fmt;
#[cfg(feature = "test-engine")]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(feature = "test-engine")]
static RAW_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "test-engine")]
static RAW_CALLBACK_MUTATOR_TYPED_STATE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "test-engine")]
static RAW_CALLBACK_PROBE_TYPED_STATE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "test-engine")]
static RAW_CALLBACK_NESTED_BORROW_REJECTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "test-engine")]
static RAW_CALLBACK_RESET_STATE_OBSERVED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "test-engine")]
static RAW_CALLBACK_SAMPLER_STRATEGY: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "test-engine")]
fn sampler_strategy_code(strategy: GlowSamplerStrategy) -> usize {
    match strategy {
        GlowSamplerStrategy::SamplerObjects => 1,
        GlowSamplerStrategy::TextureParameters => 2,
        _ => 0,
    }
}

#[cfg(feature = "test-engine")]
unsafe extern "C" fn glow_render_state_mutator(
    _draw_list: *const dear_imgui_rs::sys::ImDrawList,
    _draw_command: *const dear_imgui_rs::sys::ImDrawCmd,
) {
    RAW_CALLBACK_COUNT.fetch_add(1, Ordering::SeqCst);
    let observed = unsafe {
        GlowRenderState::with_current(|state| {
            RAW_CALLBACK_SAMPLER_STRATEGY.store(
                sampler_strategy_code(state.sampler_strategy()),
                Ordering::SeqCst,
            );
            let gl = state.gl();
            gl.bind_vertex_array(None);
            gl.disable(glow::FRAMEBUFFER_SRGB);
            true
        })
    }
    .unwrap_or(false);
    RAW_CALLBACK_MUTATOR_TYPED_STATE.store(observed, Ordering::SeqCst);
}

#[cfg(feature = "test-engine")]
unsafe extern "C" fn glow_render_state_probe(
    _draw_list: *const dear_imgui_rs::sys::ImDrawList,
    _draw_command: *const dear_imgui_rs::sys::ImDrawCmd,
) {
    RAW_CALLBACK_COUNT.fetch_add(1, Ordering::SeqCst);
    let observed = unsafe {
        GlowRenderState::with_current(|state| {
            let strategy_code = sampler_strategy_code(state.sampler_strategy());
            let strategy_matches = RAW_CALLBACK_SAMPLER_STRATEGY.load(Ordering::SeqCst)
                == strategy_code
                && strategy_code != 0;
            let nested_rejected = matches!(
                GlowRenderState::with_current(|_| ()),
                Err(GlowRenderStateAccessError::AlreadyBorrowed)
            );
            RAW_CALLBACK_NESTED_BORROW_REJECTED.store(nested_rejected, Ordering::SeqCst);
            let gl = state.gl();
            let reset_state_observed = gl
                .get_parameter_vertex_array(glow::VERTEX_ARRAY_BINDING)
                .is_some()
                && gl.is_enabled(glow::FRAMEBUFFER_SRGB);
            RAW_CALLBACK_RESET_STATE_OBSERVED.store(reset_state_observed, Ordering::SeqCst);
            strategy_matches
        })
    }
    .unwrap_or(false);
    RAW_CALLBACK_PROBE_TYPED_STATE.store(observed, Ordering::SeqCst);
}

#[cfg(feature = "test-engine")]
fn reset_raw_callback_probe() {
    RAW_CALLBACK_COUNT.store(0, Ordering::SeqCst);
    RAW_CALLBACK_MUTATOR_TYPED_STATE.store(false, Ordering::SeqCst);
    RAW_CALLBACK_PROBE_TYPED_STATE.store(false, Ordering::SeqCst);
    RAW_CALLBACK_NESTED_BORROW_REJECTED.store(false, Ordering::SeqCst);
    RAW_CALLBACK_RESET_STATE_OBSERVED.store(false, Ordering::SeqCst);
    RAW_CALLBACK_SAMPLER_STRATEGY.store(0, Ordering::SeqCst);
}

#[cfg(feature = "test-engine")]
fn raw_callback_probe_passed(expected_strategy: GlowSamplerStrategy) -> bool {
    RAW_CALLBACK_COUNT.load(Ordering::SeqCst) == 2
        && RAW_CALLBACK_MUTATOR_TYPED_STATE.load(Ordering::SeqCst)
        && RAW_CALLBACK_PROBE_TYPED_STATE.load(Ordering::SeqCst)
        && RAW_CALLBACK_NESTED_BORROW_REJECTED.load(Ordering::SeqCst)
        && RAW_CALLBACK_SAMPLER_STRATEGY.load(Ordering::SeqCst)
            == sampler_strategy_code(expected_strategy)
}

#[cfg(feature = "test-engine")]
fn reset_render_state_probe_passed() -> bool {
    RAW_CALLBACK_RESET_STATE_OBSERVED.load(Ordering::SeqCst)
}

struct TextureUnitZeroGuard<'a> {
    gl: &'a glow::Context,
    active_texture: u32,
    texture: Option<GlTexture>,
}

impl<'a> TextureUnitZeroGuard<'a> {
    fn bind(gl: &'a glow::Context, texture: GlTexture) -> Self {
        unsafe {
            let active_texture =
                u32::try_from(gl.get_parameter_i32(glow::ACTIVE_TEXTURE)).unwrap_or(glow::TEXTURE0);
            gl.active_texture(glow::TEXTURE0);
            let previous = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            Self {
                gl,
                active_texture,
                texture: previous,
            }
        }
    }
}

impl Drop for TextureUnitZeroGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, self.texture);
            self.gl.active_texture(self.active_texture);
        }
    }
}

struct ExternalTexture {
    id: TextureId,
    handle: GlTexture,
    #[cfg(feature = "test-engine")]
    expected_filters: [i32; 2],
}

impl ExternalTexture {
    fn create(gl: &glow::Context) -> Result<Self, Box<dyn Error>> {
        let pixels = [
            255, 255, 255, 255, 255, 0, 255, 255, 0, 255, 255, 255, 0, 0, 0, 255,
        ];
        let handle = create_texture_from_rgba(gl, 2, 2, &pixels)?;
        let expected_filters = [glow::LINEAR_MIPMAP_NEAREST as i32, glow::NEAREST as i32];
        {
            let _binding = TextureUnitZeroGuard::bind(gl, handle);
            unsafe {
                gl.generate_mipmap(glow::TEXTURE_2D);
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    expected_filters[0],
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    expected_filters[1],
                );
            }
        }
        Ok(Self {
            id: TextureId::new(u64::MAX - 1),
            handle,
            #[cfg(feature = "test-engine")]
            expected_filters,
        })
    }

    #[cfg(feature = "test-engine")]
    fn filters(&self, gl: &glow::Context) -> [i32; 2] {
        let _binding = TextureUnitZeroGuard::bind(gl, self.handle);
        unsafe {
            [
                gl.get_tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER),
                gl.get_tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER),
            ]
        }
    }

    #[cfg(feature = "test-engine")]
    fn filters_are_preserved(&self, gl: &glow::Context) -> bool {
        self.filters(gl) == self.expected_filters
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone, Debug, PartialEq)]
struct ApplicationGlStateSnapshot {
    blend_enabled: bool,
    blend_func: [i32; 4],
    blend_equation: [i32; 2],
    viewport: [i32; 4],
    scissor_enabled: bool,
    scissor_box: [i32; 4],
    clear_color: [f32; 4],
    color_write_mask: [bool; 4],
    array_buffer: Option<GlBuffer>,
    pixel_pack_buffer: Option<GlBuffer>,
    pack_alignment: i32,
    vertex_array: Option<GlVertexArray>,
    active_texture: i32,
    texture_unit_zero: Option<GlTexture>,
    sampler_unit_zero: Option<GlSampler>,
    program: Option<GlProgram>,
    cull_enabled: bool,
    depth_enabled: bool,
    stencil_enabled: bool,
    polygon_mode: Option<[i32; 2]>,
    primitive_restart_enabled: Option<bool>,
    framebuffer_srgb_enabled: bool,
}

#[cfg(feature = "test-engine")]
impl ApplicationGlStateSnapshot {
    fn prepare(
        gl: &glow::Context,
        external_texture: GlTexture,
        gl_version: GlVersion,
        supports_sampler_objects: bool,
    ) -> Self {
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(external_texture));
            if supports_sampler_objects {
                gl.bind_sampler(0, None);
            }
            gl.active_texture(glow::TEXTURE0 + 3);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            gl.pixel_store_i32(glow::PACK_ALIGNMENT, 8);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::BLEND);
            gl.blend_func_separate(
                glow::ONE,
                glow::ZERO,
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            gl.blend_equation_separate(glow::FUNC_SUBTRACT, glow::FUNC_REVERSE_SUBTRACT);
            gl.viewport(3, 5, 211, 197);
            gl.disable(glow::SCISSOR_TEST);
            gl.scissor(7, 11, 173, 149);
            gl.clear_color(0.03, 0.07, 0.11, 0.13);
            gl.color_mask(true, true, true, true);
            gl.enable(glow::CULL_FACE);
            gl.enable(glow::DEPTH_TEST);
            gl.enable(glow::STENCIL_TEST);
            if gl_version.supports_polygon_mode() {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
            }
            if gl_version.supports_primitive_restart() {
                gl.enable(glow::PRIMITIVE_RESTART);
            }
            gl.disable(glow::FRAMEBUFFER_SRGB);
        }
        Self::capture(gl, gl_version, supports_sampler_objects)
    }

    fn capture(gl: &glow::Context, gl_version: GlVersion, supports_sampler_objects: bool) -> Self {
        unsafe {
            let mut viewport = [0; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut viewport);
            let mut scissor_box = [0; 4];
            gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut scissor_box);
            let mut clear_color = [0.0; 4];
            gl.get_parameter_f32_slice(glow::COLOR_CLEAR_VALUE, &mut clear_color);
            let active_texture = gl.get_parameter_i32(glow::ACTIVE_TEXTURE);
            gl.active_texture(glow::TEXTURE0);
            let texture_unit_zero = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
            let sampler_unit_zero = supports_sampler_objects
                .then(|| gl.get_parameter_sampler(glow::SAMPLER_BINDING))
                .flatten();
            gl.active_texture(u32::try_from(active_texture).unwrap_or(glow::TEXTURE0));
            let polygon_mode = gl_version.supports_polygon_mode().then(|| {
                let mut modes = [0; 2];
                gl.get_parameter_i32_slice(glow::POLYGON_MODE, &mut modes);
                modes
            });
            Self {
                blend_enabled: gl.is_enabled(glow::BLEND),
                blend_func: [
                    gl.get_parameter_i32(glow::BLEND_SRC_RGB),
                    gl.get_parameter_i32(glow::BLEND_DST_RGB),
                    gl.get_parameter_i32(glow::BLEND_SRC_ALPHA),
                    gl.get_parameter_i32(glow::BLEND_DST_ALPHA),
                ],
                blend_equation: [
                    gl.get_parameter_i32(glow::BLEND_EQUATION_RGB),
                    gl.get_parameter_i32(glow::BLEND_EQUATION_ALPHA),
                ],
                viewport,
                scissor_enabled: gl.is_enabled(glow::SCISSOR_TEST),
                scissor_box,
                clear_color,
                color_write_mask: gl.get_parameter_bool_array(glow::COLOR_WRITEMASK),
                array_buffer: gl.get_parameter_buffer(glow::ARRAY_BUFFER_BINDING),
                pixel_pack_buffer: gl.get_parameter_buffer(glow::PIXEL_PACK_BUFFER_BINDING),
                pack_alignment: gl.get_parameter_i32(glow::PACK_ALIGNMENT),
                vertex_array: gl.get_parameter_vertex_array(glow::VERTEX_ARRAY_BINDING),
                active_texture,
                texture_unit_zero,
                sampler_unit_zero,
                program: gl.get_parameter_program(glow::CURRENT_PROGRAM),
                cull_enabled: gl.is_enabled(glow::CULL_FACE),
                depth_enabled: gl.is_enabled(glow::DEPTH_TEST),
                stencil_enabled: gl.is_enabled(glow::STENCIL_TEST),
                polygon_mode,
                primitive_restart_enabled: gl_version
                    .supports_primitive_restart()
                    .then(|| gl.is_enabled(glow::PRIMITIVE_RESTART)),
                framebuffer_srgb_enabled: gl.is_enabled(glow::FRAMEBUFFER_SRGB),
            }
        }
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone, Copy)]
struct SamplerProbeScreenPoints {
    nearest: [f32; 2],
    linear: [f32; 2],
    reset_linear: [f32; 2],
}

#[cfg(feature = "test-engine")]
impl SamplerProbeScreenPoints {
    fn submit(
        ui: &dear_imgui_rs::Ui,
        texture: TextureId,
        nearest_callback: RawDrawCallback,
        linear_callback: RawDrawCallback,
        reset_callback: RawDrawCallback,
    ) -> Self {
        const SIZE: f32 = 32.0;
        const GAP: f32 = 12.0;
        let [viewport_x, viewport_y] = ui.main_viewport().pos();
        let y = viewport_y + 32.0;
        let nearest_min = [viewport_x + 32.0, y];
        let linear_min = [nearest_min[0] + SIZE + GAP, y];
        let reset_linear_min = [linear_min[0] + SIZE + GAP, y];
        let draw_list = ui.get_foreground_draw_list();
        let add_constant_uv_image = |min: [f32; 2]| {
            draw_list.add_image(
                texture,
                min,
                [min[0] + SIZE, min[1] + SIZE],
                [0.5, 0.5],
                [0.5, 0.5],
                [1.0, 1.0, 1.0, 1.0],
            );
        };
        unsafe {
            draw_list.add_callback(nearest_callback, std::ptr::null_mut(), 0);
        }
        add_constant_uv_image(nearest_min);
        unsafe {
            draw_list.add_callback(linear_callback, std::ptr::null_mut(), 0);
        }
        add_constant_uv_image(linear_min);
        unsafe {
            draw_list.add_callback(glow_render_state_mutator, std::ptr::null_mut(), 0);
            draw_list.add_callback(reset_callback, std::ptr::null_mut(), 0);
            draw_list.add_callback(glow_render_state_probe, std::ptr::null_mut(), 0);
            draw_list.add_callback(linear_callback, std::ptr::null_mut(), 0);
        }
        add_constant_uv_image(reset_linear_min);

        let center = |min: [f32; 2]| [min[0] + SIZE / 2.0, min[1] + SIZE / 2.0];
        Self {
            nearest: center(nearest_min),
            linear: center(linear_min),
            reset_linear: center(reset_linear_min),
        }
    }

    fn map_to_framebuffer(
        self,
        draw_data: &dear_imgui_rs::render::DrawData,
    ) -> Result<SamplerReadbackTargets, Box<dyn Error>> {
        let display_pos = draw_data.display_pos();
        let scale = draw_data.framebuffer_scale();
        let map = |point: [f32; 2]| -> Result<[i32; 2], Box<dyn Error>> {
            let mapped = [
                (point[0] - display_pos[0]) * scale[0],
                (point[1] - display_pos[1]) * scale[1],
            ];
            if mapped
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
                || mapped.iter().any(|value| *value > i32::MAX as f32)
            {
                return Err(format!("invalid sampler readback coordinate: {mapped:?}").into());
            }
            Ok([mapped[0].floor() as i32, mapped[1].floor() as i32])
        };
        Ok(SamplerReadbackTargets {
            nearest: map(self.nearest)?,
            linear: map(self.linear)?,
            reset_linear: map(self.reset_linear)?,
        })
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone, Copy)]
struct SamplerReadbackTargets {
    nearest: [i32; 2],
    linear: [i32; 2],
    reset_linear: [i32; 2],
}

#[cfg(feature = "test-engine")]
struct PixelReadbackStateGuard<'gl> {
    gl: &'gl glow::Context,
    pack_buffer: Option<GlBuffer>,
    pack_alignment: i32,
}

#[cfg(feature = "test-engine")]
impl<'gl> PixelReadbackStateGuard<'gl> {
    fn enter(gl: &'gl glow::Context) -> Self {
        let pack_buffer = unsafe { gl.get_parameter_buffer(glow::PIXEL_PACK_BUFFER_BINDING) };
        let pack_alignment = unsafe { gl.get_parameter_i32(glow::PACK_ALIGNMENT) };
        unsafe {
            gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        }
        Self {
            gl,
            pack_buffer,
            pack_alignment,
        }
    }
}

#[cfg(feature = "test-engine")]
impl Drop for PixelReadbackStateGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.gl
                .pixel_store_i32(glow::PACK_ALIGNMENT, self.pack_alignment);
            self.gl
                .bind_buffer(glow::PIXEL_PACK_BUFFER, self.pack_buffer);
        }
    }
}

#[cfg(feature = "test-engine")]
impl SamplerReadbackTargets {
    fn read(
        self,
        gl: &glow::Context,
        framebuffer_size: (u32, u32),
    ) -> Result<SamplerPixelReadback, Box<dyn Error>> {
        let width = i32::try_from(framebuffer_size.0)?;
        let height = i32::try_from(framebuffer_size.1)?;
        let _state = PixelReadbackStateGuard::enter(gl);
        let read = |point: [i32; 2]| -> Result<[u8; 4], Box<dyn Error>> {
            if point[0] < 0 || point[0] >= width || point[1] < 0 || point[1] >= height {
                return Err(format!(
                    "sampler readback point {point:?} is outside {width}x{height}"
                )
                .into());
            }
            let mut pixel = [0; 4];
            unsafe {
                gl.read_pixels(
                    point[0],
                    height - point[1] - 1,
                    1,
                    1,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut pixel)),
                );
            }
            Ok(pixel)
        };
        Ok(SamplerPixelReadback {
            nearest: read(self.nearest)?,
            linear: read(self.linear)?,
            reset_linear: read(self.reset_linear)?,
        })
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone, Copy, Debug)]
struct SamplerPixelReadback {
    nearest: [u8; 4],
    linear: [u8; 4],
    reset_linear: [u8; 4],
}

#[cfg(feature = "test-engine")]
impl SamplerPixelReadback {
    fn proves_sampler_isolation(self) -> bool {
        let texel_is_extreme = self.nearest[..3]
            .iter()
            .all(|channel| *channel <= 16 || *channel >= 239);
        let linear_has_interpolation = self.linear[..3]
            .iter()
            .filter(|channel| **channel > 32 && **channel < 239)
            .count()
            >= 2;
        let reset_matches_linear = self
            .linear
            .iter()
            .zip(self.reset_linear)
            .all(|(expected, actual)| expected.abs_diff(actual) <= 3);
        texel_is_extreme && linear_has_interpolation && reset_matches_linear
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone, Copy, Debug, Default)]
struct GlowContractEvidence {
    external_texture_filters_preserved: bool,
    sampler_pixels_prove_isolation: bool,
    raw_callback_typed_state_observed: bool,
    reset_render_state_recovered: bool,
    render_state_cleared_after_callback: bool,
    application_gl_state_restored: bool,
}

#[cfg(feature = "test-engine")]
impl GlowContractEvidence {
    fn is_complete(self) -> bool {
        self.external_texture_filters_preserved
            && self.sampler_pixels_prove_isolation
            && self.raw_callback_typed_state_observed
            && self.reset_render_state_recovered
            && self.render_state_cleared_after_callback
            && self.application_gl_state_restored
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone)]
struct OpenGlRendererInfo {
    vendor: String,
    renderer: String,
    version: String,
}

#[cfg(feature = "test-engine")]
struct ViewportSmokeState {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    sampler_strategy: GlowSamplerStrategy,
    saw_secondary_viewport: bool,
    completed_frame_evidence: Option<SecondaryViewportFrameEvidence>,
    saw_merged_viewport: bool,
    main_present_bracketed_by_test_engine: bool,
    contract: GlowContractEvidence,
    complete: bool,
}

#[cfg(feature = "test-engine")]
struct CompletedViewportSmoke {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    sampler_strategy: GlowSamplerStrategy,
    context_ready_viewports: Vec<Id>,
    glow_draw_issued_viewports: Vec<Id>,
    swap_succeeded_viewports: Vec<Id>,
    saw_merged_viewport: bool,
    main_present_bracketed_by_test_engine: bool,
    contract: GlowContractEvidence,
}

#[cfg(feature = "test-engine")]
impl ViewportSmokeState {
    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        if !self.complete {
            return None;
        }
        let evidence = self.completed_frame_evidence.as_ref()?;
        Some(CompletedViewportSmoke {
            result_path: self.result_path.clone(),
            renderer: self.renderer.clone(),
            sampler_strategy: self.sampler_strategy,
            context_ready_viewports: evidence.context_activated_viewports.clone(),
            glow_draw_issued_viewports: evidence.glow_rendered_viewports.clone(),
            swap_succeeded_viewports: evidence.swapped_viewports.clone(),
            saw_merged_viewport: self.saw_merged_viewport,
            main_present_bracketed_by_test_engine: self.main_present_bracketed_by_test_engine,
            contract: self.contract,
        })
    }
}

#[cfg(feature = "test-engine")]
impl CompletedViewportSmoke {
    fn write_after_teardown(self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.result_path else {
            return Ok(());
        };
        let json = format!(
            "{{\"schema_version\":5,\"renderer\":{{\"backend\":\"OpenGL\",\"vendor\":\"{}\",\"name\":\"{}\",\"version\":\"{}\"}},\"sampler_strategy\":\"{}\",\"secondary_context_ready_before_main_present_viewport_ids\":{},\"secondary_draw_issued_before_main_present_viewport_ids\":{},\"secondary_swap_succeeded_before_main_present_viewport_ids\":{},\"merge_observed\":{},\"main_present_bracketed_by_test_engine\":{},\"external_texture_filters_preserved\":{},\"sampler_pixels_prove_isolation\":{},\"raw_callback_typed_state_observed\":{},\"reset_render_state_recovered\":{},\"render_state_cleared_after_callback\":{},\"application_gl_state_restored\":{}}}",
            json_escape(&self.renderer.vendor),
            json_escape(&self.renderer.renderer),
            json_escape(&self.renderer.version),
            sampler_strategy_name(self.sampler_strategy),
            viewport_ids_json(&self.context_ready_viewports),
            viewport_ids_json(&self.glow_draw_issued_viewports),
            viewport_ids_json(&self.swap_succeeded_viewports),
            self.saw_merged_viewport,
            self.main_present_bracketed_by_test_engine,
            self.contract.external_texture_filters_preserved,
            self.contract.sampler_pixels_prove_isolation,
            self.contract.raw_callback_typed_state_observed,
            self.contract.reset_render_state_recovered,
            self.contract.render_state_cleared_after_callback,
            self.contract.application_gl_state_restored,
        );
        write_json_atomic(&path, &json)
    }
}

#[cfg(feature = "test-engine")]
fn sampler_strategy_name(strategy: GlowSamplerStrategy) -> &'static str {
    match strategy {
        GlowSamplerStrategy::SamplerObjects => "sampler_objects",
        GlowSamplerStrategy::TextureParameters => "texture_parameters",
        _ => "unknown",
    }
}

#[cfg(feature = "test-engine")]
fn viewport_ids_json(ids: &[Id]) -> String {
    let ids = ids
        .iter()
        .map(|id| id.raw().to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{ids}]")
}

#[cfg(feature = "test-engine")]
fn query_opengl_renderer(gl: &glow::Context) -> OpenGlRendererInfo {
    unsafe {
        OpenGlRendererInfo {
            vendor: gl.get_parameter_string(glow::VENDOR),
            renderer: gl.get_parameter_string(glow::RENDERER),
            version: gl.get_parameter_string(glow::VERSION),
        }
    }
}

#[cfg(feature = "test-engine")]
fn validate_software_opengl_renderer(info: &OpenGlRendererInfo) -> Result<(), String> {
    let identity = format!("{} {} {}", info.vendor, info.renderer, info.version).to_lowercase();
    if !identity.contains("llvmpipe") && !identity.contains("lavapipe") {
        return Err(format!(
            "viewport smoke requires Mesa llvmpipe, selected '{}' ({}, {})",
            info.renderer, info.vendor, info.version
        ));
    }
    Ok(())
}

#[cfg(feature = "test-engine")]
fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(feature = "test-engine")]
fn write_json_atomic(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
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
        Ok::<_, Box<dyn Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(feature = "test-engine")]
type RunResult = Option<CompletedViewportSmoke>;
#[cfg(not(feature = "test-engine"))]
type RunResult = ();

struct GlowApp {
    main: MainThreadData<RefCell<Option<MainData>>>,
    events: Sdl3CallbackEventHandoff,
}

struct MainData {
    sdl3_backend: Sdl3PlatformBackend,
    imgui: Context,
    renderer: GlowViewportRuntime,
    gl: Rc<glow::Context>,
    gl_context: sdl3::video::GLContext,
    window: sdl3::video::Window,
    external_texture: ExternalTexture,
    #[cfg(feature = "test-engine")]
    reset_render_state_callback: RawDrawCallback,
    sampler_linear_callback: RawDrawCallback,
    sampler_nearest_callback: RawDrawCallback,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
    last_frame: Instant,
    #[cfg(feature = "test-engine")]
    test_engine: Option<TestEngine>,
    #[cfg(feature = "test-engine")]
    viewport_smoke: Option<ViewportSmokeState>,
    #[cfg(feature = "test-engine")]
    test_engine_frame_index: u64,
}

#[derive(Debug)]
struct SdlGlowFrameError {
    source: Box<dyn Error>,
}

impl SdlGlowFrameError {
    fn new(source: impl Error + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }

    fn message(message: impl Into<String>) -> Self {
        Self::new(std::io::Error::other(message.into()))
    }
}

impl fmt::Display for SdlGlowFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl Error for SdlGlowFrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(feature = "test-engine")]
#[derive(Debug, Default)]
struct SecondaryViewportFrameEvidence {
    context_activated_viewports: Vec<Id>,
    glow_rendered_viewports: Vec<Id>,
    swapped_viewports: Vec<Id>,
    completed_viewports: Vec<Id>,
}

#[cfg(feature = "test-engine")]
impl SecondaryViewportFrameEvidence {
    fn from_reports(
        glow: &dear_imgui_glow::multi_viewport::GlowViewportFrameReport,
        sdl3: &dear_imgui_sdl3::Sdl3OpenGlViewportFrameReport,
    ) -> Self {
        let context_activated_viewports = sdl3.context_activated_viewports().to_vec();
        let glow_rendered_viewports = glow.rendered_viewports().to_vec();
        let swapped_viewports = sdl3.swapped_viewports().to_vec();
        let completed_viewports = glow_rendered_viewports
            .iter()
            .copied()
            .filter(|id| context_activated_viewports.contains(id) && swapped_viewports.contains(id))
            .collect();
        Self {
            context_activated_viewports,
            glow_rendered_viewports,
            swapped_viewports,
            completed_viewports,
        }
    }
}

#[cfg(feature = "test-engine")]
#[derive(Clone, Copy)]
struct GlowFrameContractProbe {
    readback_targets: SamplerReadbackTargets,
    external_texture: GlTexture,
    gl_version: GlVersion,
    supports_sampler_objects: bool,
}

#[cfg(feature = "test-engine")]
#[derive(Debug)]
struct GlowFrameContractResult {
    readback: SamplerPixelReadback,
    application_gl_state_restored: bool,
}

struct SdlGlowFrameDriver<'a> {
    sdl3_backend: &'a Sdl3PlatformBackend,
    renderer: &'a GlowViewportRuntime,
    gl: &'a glow::Context,
    window: &'a sdl3::video::Window,
    gl_context: &'a sdl3::video::GLContext,
    rendered: bool,
    #[cfg(feature = "test-engine")]
    secondary_viewport_evidence: Option<SecondaryViewportFrameEvidence>,
    #[cfg(feature = "test-engine")]
    contract_probe: Option<GlowFrameContractProbe>,
    #[cfg(feature = "test-engine")]
    contract_result: Option<GlowFrameContractResult>,
    presented: bool,
}

impl SdlGlowFrameDriver<'_> {
    fn render_frame<'frame>(
        &mut self,
        frame: RenderedFrame<'frame>,
    ) -> Result<ReconciledFrame<'frame>, SdlGlowFrameError> {
        if self.rendered {
            return Err(SdlGlowFrameError::message(
                "main OpenGL frame was rendered more than once",
            ));
        }
        unsafe {
            let (width, height) = self.window.size_in_pixels();
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.clear_color(0.1, 0.12, 0.15, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        #[cfg(feature = "test-engine")]
        let application_state_before = self.contract_probe.map(|probe| {
            ApplicationGlStateSnapshot::prepare(
                self.gl,
                probe.external_texture,
                probe.gl_version,
                probe.supports_sampler_objects,
            )
        });

        self.renderer.new_frame().map_err(SdlGlowFrameError::new)?;
        #[cfg(feature = "test-engine")]
        let sdl3_trace = self
            .sdl3_backend
            .begin_opengl_viewport_frame_trace()
            .map_err(SdlGlowFrameError::new)?;
        #[cfg(feature = "test-engine")]
        let glow_trace = self
            .renderer
            .begin_frame_trace()
            .map_err(SdlGlowFrameError::new)?;
        let render_result = self
            .renderer
            .render_with_platform_windows_reconciled(frame)
            .map_err(SdlGlowFrameError::new);
        let restore_result = self
            .window
            .gl_make_current(self.gl_context)
            .map_err(|error| {
                SdlGlowFrameError::message(format!(
                    "failed to restore the main OpenGL context: {error}"
                ))
            });
        #[cfg(feature = "test-engine")]
        let glow_report = glow_trace.finish();
        #[cfg(feature = "test-engine")]
        let sdl3_report = sdl3_trace.finish();
        let glow_fault = self.renderer.poll_fault().map_err(SdlGlowFrameError::new);
        let sdl3_fault = self
            .sdl3_backend
            .poll_fault()
            .map_err(SdlGlowFrameError::new);

        restore_result?;
        glow_fault?;
        sdl3_fault?;
        let reconciled = render_result?;
        #[cfg(feature = "test-engine")]
        if let (Some(probe), Some(before)) = (self.contract_probe, application_state_before) {
            let after_renderer = ApplicationGlStateSnapshot::capture(
                self.gl,
                probe.gl_version,
                probe.supports_sampler_objects,
            );
            let readback = probe
                .readback_targets
                .read(self.gl, self.window.size_in_pixels())
                .map_err(|error| SdlGlowFrameError { source: error })?;
            let after_readback = ApplicationGlStateSnapshot::capture(
                self.gl,
                probe.gl_version,
                probe.supports_sampler_objects,
            );
            self.contract_result = Some(GlowFrameContractResult {
                readback,
                application_gl_state_restored: before == after_renderer
                    && after_renderer == after_readback,
            });
        }
        #[cfg(feature = "test-engine")]
        {
            self.secondary_viewport_evidence = Some(SecondaryViewportFrameEvidence::from_reports(
                &glow_report,
                &sdl3_report,
            ));
        }
        self.rendered = true;
        Ok(reconciled)
    }

    fn present_frame(&mut self) -> Result<(), SdlGlowFrameError> {
        if !self.rendered {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented before frame and platform rendering completed",
            ));
        }
        #[cfg(feature = "test-engine")]
        if self.secondary_viewport_evidence.is_none() {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented before viewport evidence was collected",
            ));
        }
        if self.presented {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented more than once",
            ));
        }
        self.window.gl_swap_window();
        self.presented = true;
        Ok(())
    }
}

#[cfg(feature = "test-engine")]
impl TestFrameDriver for SdlGlowFrameDriver<'_> {
    type RenderError = SdlGlowFrameError;
    type PresentError = SdlGlowFrameError;

    fn render<'frame>(
        &mut self,
        frame: RenderedFrame<'frame>,
        _frame_index: u64,
    ) -> Result<ReconciledFrame<'frame>, Self::RenderError> {
        self.render_frame(frame)
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        self.present_frame()
    }
}

impl GlowApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        imgui_sdl3_backend::enable_native_ime_ui();
        configure_main_callback_rate();
        Ok(Self {
            main: MainThreadData::assert_new(RefCell::new(Some(MainData::new()?))),
            events: Sdl3CallbackEventHandoff::default(),
        })
    }

    fn process_events(&self) -> Result<AppResult, Box<dyn Error>> {
        let mut events = self.events.drain();
        let mut main = self.main.assert_get().borrow_mut();
        let main = main
            .as_mut()
            .expect("SDL3 Glow state must be active while callbacks run");
        while let Some(event) = events.pop() {
            event.with_imgui_event(|raw| -> Result<(), Box<dyn Error>> {
                if let Some(raw) = raw {
                    let _ = main.sdl3_backend.process_event(&mut main.imgui, raw)?;
                }
                Ok(())
            })?;
            if requests_exit(&event, main.window.id()) {
                return Ok(AppResult::Success);
            }
        }
        Ok(AppResult::Continue)
    }

    fn render(&self) -> Result<bool, Box<dyn Error>> {
        self.main
            .assert_get()
            .borrow_mut()
            .as_mut()
            .expect("SDL3 Glow state must be active while callbacks run")
            .render()
    }

    fn shutdown(&self) {
        let main = self.main.assert_get().borrow_mut().take();
        let Some(main) = main else {
            return;
        };
        match main.shutdown() {
            #[cfg(feature = "test-engine")]
            Ok(Some(result)) => {
                // `MainData::shutdown` consumes and drops every SDL, GL, renderer, backend, and
                // Context owner before returning the smoke summary.
                if let Err(error) = result.write_after_teardown() {
                    eprintln!("failed to write SDL3/Glow viewport smoke result: {error}");
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("SDL3/Glow shutdown failed: {error}"),
        }
    }
}

impl MainData {
    fn new() -> Result<Self, Box<dyn Error>> {
        let sdl = sdl3::init()?;
        let video = sdl.video()?;
        #[cfg(feature = "test-engine")]
        let run_viewport_smoke =
            std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");

        let gl_attr = video.gl_attr();
        gl_attr.set_context_version(3, 2);
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_depth_size(0);

        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);

        let mut window = video
            .window(
                "Dear ImGui + SDL3 + Glow (multi-viewport)",
                (800.0 * main_scale) as u32,
                (600.0 * main_scale) as u32,
            )
            .opengl()
            .resizable()
            .hidden()
            .high_pixel_density()
            .build()
            .map_err(|error| format!("failed to create SDL3 window: {error}"))?;

        let gl_context = window
            .gl_create_context()
            .map_err(|error| format!("SDL_GL_CreateContext failed: {error}"))?;
        window
            .gl_make_current(&gl_context)
            .map_err(|error| format!("SDL_GL_MakeCurrent failed: {error}"))?;
        #[cfg(feature = "test-engine")]
        let main_swap_interval = if run_viewport_smoke {
            SwapInterval::Immediate
        } else {
            SwapInterval::VSync
        };
        #[cfg(not(feature = "test-engine"))]
        let main_swap_interval = SwapInterval::VSync;
        let _ = video.gl_set_swap_interval(main_swap_interval);
        window.set_position(WindowPos::Centered, WindowPos::Centered);
        window.show();

        let gl = Rc::new(unsafe { create_glow_context(&video) });

        #[cfg(feature = "test-engine")]
        let renderer_info = query_opengl_renderer(&gl);
        #[cfg(feature = "test-engine")]
        if run_viewport_smoke {
            println!(
                "OpenGL renderer: vendor='{}', renderer='{}', version='{}'",
                renderer_info.vendor, renderer_info.renderer, renderer_info.version
            );
            if std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_OPENGL").is_ok_and(|value| value == "1") {
                validate_software_opengl_renderer(&renderer_info)?;
            }
        }

        let mut imgui = Context::create();
        #[cfg(feature = "test-engine")]
        if run_viewport_smoke {
            imgui.set_ini_filename(None::<String>)?;
        }
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(flags);
        }

        // SAFETY: `window` and `gl_context` outlive renderer/platform shutdown and Context teardown.
        let sdl3_backend = unsafe {
            Sdl3PlatformBackend::init_platform_for_opengl(&mut imgui, &window, &gl_context)?
        };

        let window_scale = window.display_scale();
        imgui.style_mut().set_font_scale_dpi(window_scale);

        let external_texture = ExternalTexture::create(&gl)?;
        let mut texture_map = SimpleTextureMap::default();
        texture_map.set(external_texture.id, external_texture.handle);
        let renderer =
            GlowRenderer::with_shared_context(Rc::clone(&gl), &mut imgui, Box::new(texture_map))?;
        #[cfg(feature = "test-engine")]
        let sampler_strategy = if renderer.supports_sampler_objects() {
            GlowSamplerStrategy::SamplerObjects
        } else {
            GlowSamplerStrategy::TextureParameters
        };
        #[cfg(feature = "test-engine")]
        let renderer = {
            let mut renderer = renderer;
            if run_viewport_smoke {
                renderer.set_framebuffer_srgb_enabled(true)?;
            }
            renderer
        };
        #[cfg(feature = "test-engine")]
        let reset_render_state_callback = imgui
            .platform_io()
            .draw_callback_reset_render_state_raw()
            .ok_or("Glow did not publish its reset-render-state callback")?;
        let sampler_linear_callback = imgui
            .platform_io()
            .draw_callback_set_sampler_linear_raw()
            .ok_or("Glow did not publish its linear sampler callback")?;
        let sampler_nearest_callback = imgui
            .platform_io()
            .draw_callback_set_sampler_nearest_raw()
            .ok_or("Glow did not publish its nearest sampler callback")?;
        // SAFETY: SDL3's OpenGL viewport backend creates every secondary context in the main
        // context's share group and makes the matching context current for renderer callbacks. The
        // frame driver explicitly restores `gl_context` after the platform-window pump.
        let renderer = unsafe { GlowViewportRuntime::attach(&mut imgui, renderer)? };

        #[cfg(feature = "test-engine")]
        let test_engine = if run_viewport_smoke {
            let (main_x, main_y) = window.position();
            let (main_width, _) = window.size();
            let external_pos = [
                main_x as f32 + main_width as f32 + 100.0,
                main_y as f32 + 100.0,
            ];
            let merged_pos = [main_x as f32 + 100.0, main_y as f32 + 100.0];

            let mut engine = TestEngine::create()?;
            engine.start(&mut imgui)?;
            engine.set_run_speed(RunSpeed::Fast)?;
            engine.set_verbose_level(VerboseLevel::Info)?;
            engine.add_script_test("sdl3-glow", "multi_viewport_surface_smoke", move |test| {
                test.wait_for_item("Main/Viewport Count", ScriptCount::new(240)?)?;
                test.window_move("Main", external_pos[0], external_pos[1])?;
                test.yield_frames(ScriptCount::new(30)?)?;
                test.assert_item_read_int_eq("Main/Viewport Count", 2)?;
                test.window_move("Main", merged_pos[0], merged_pos[1])?;
                test.yield_frames(ScriptCount::new(30)?)?;
                test.assert_item_read_int_eq("Main/Viewport Count", 1)
            })?;
            engine.queue_tests(
                TestGroup::Tests,
                Some("multi_viewport_surface_smoke"),
                RunFlags::RUN_FROM_COMMAND_LINE,
            )?;
            Some(engine)
        } else {
            None
        };

        #[cfg(feature = "test-engine")]
        let viewport_smoke = run_viewport_smoke.then(|| ViewportSmokeState {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            renderer: renderer_info,
            sampler_strategy,
            saw_secondary_viewport: false,
            completed_frame_evidence: None,
            saw_merged_viewport: false,
            main_present_bracketed_by_test_engine: false,
            contract: GlowContractEvidence::default(),
            complete: false,
        });

        Ok(Self {
            sdl3_backend,
            imgui,
            renderer,
            gl,
            gl_context,
            window,
            external_texture,
            #[cfg(feature = "test-engine")]
            reset_render_state_callback,
            sampler_linear_callback,
            sampler_nearest_callback,
            _video: video,
            _sdl: sdl,
            last_frame: Instant::now(),
            #[cfg(feature = "test-engine")]
            test_engine,
            #[cfg(feature = "test-engine")]
            viewport_smoke,
            #[cfg(feature = "test-engine")]
            test_engine_frame_index: 0,
        })
    }

    fn render(&mut self) -> Result<bool, Box<dyn Error>> {
        let now = Instant::now();
        self.imgui
            .io_mut()
            .set_delta_time((now - self.last_frame).as_secs_f32());
        self.last_frame = now;

        self.sdl3_backend.new_frame(&mut self.imgui)?;
        #[cfg(feature = "test-engine")]
        let mut viewport_count =
            i32::try_from(self.imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            if viewport_count > 1 {
                smoke.saw_secondary_viewport = true;
            } else if smoke.saw_secondary_viewport {
                smoke.saw_merged_viewport = true;
            }
        }

        #[cfg(feature = "test-engine")]
        reset_raw_callback_probe();
        let ui = self.imgui.frame();
        ui.dockspace_over_main_viewport();
        let external_texture_id = self.external_texture.id;
        #[cfg(feature = "test-engine")]
        let reset_render_state_callback = self.reset_render_state_callback;
        let sampler_linear_callback = self.sampler_linear_callback;
        let sampler_nearest_callback = self.sampler_nearest_callback;
        #[cfg(feature = "test-engine")]
        let run_contract_probe = self.test_engine.is_some();
        #[cfg(not(feature = "test-engine"))]
        let run_contract_probe = false;
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + Glow + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn an OS window.");
                #[cfg(feature = "test-engine")]
                if self.test_engine.is_some() {
                    ui.input_int_config("Viewport Count")
                        .flags(dear_imgui_rs::InputScalarFlags::READ_ONLY)
                        .build(&mut viewport_count);
                }
                if !run_contract_probe {
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(sampler_nearest_callback, std::ptr::null_mut(), 0);
                    }
                    ui.image(external_texture_id, [64.0, 64.0]);
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(sampler_linear_callback, std::ptr::null_mut(), 0);
                    }
                }
            });

        #[cfg(feature = "test-engine")]
        let sampler_probe = run_contract_probe.then(|| {
            SamplerProbeScreenPoints::submit(
                ui,
                external_texture_id,
                sampler_nearest_callback,
                sampler_linear_callback,
                reset_render_state_callback,
            )
        });

        let frame = self.imgui.render();
        #[cfg(feature = "test-engine")]
        let contract_probe = if let Some(sampler_probe) = sampler_probe {
            let sampler_strategy = self
                .viewport_smoke
                .as_ref()
                .ok_or("contract probe requires viewport smoke state")?
                .sampler_strategy;
            Some(GlowFrameContractProbe {
                readback_targets: sampler_probe.map_to_framebuffer(frame.draw_data())?,
                external_texture: self.external_texture.handle,
                gl_version: GlVersion::read(&self.gl),
                supports_sampler_objects: matches!(
                    sampler_strategy,
                    GlowSamplerStrategy::SamplerObjects
                ),
            })
        } else {
            None
        };
        #[cfg(feature = "test-engine")]
        let frame_index = {
            self.test_engine_frame_index = self
                .test_engine_frame_index
                .checked_add(1)
                .ok_or("Test Engine frame index exhausted")?;
            self.test_engine_frame_index
        };
        #[cfg(feature = "test-engine")]
        let used_test_engine = self.test_engine.is_some();

        let mut driver = SdlGlowFrameDriver {
            sdl3_backend: &self.sdl3_backend,
            renderer: &self.renderer,
            gl: self.gl.as_ref(),
            window: &self.window,
            gl_context: &self.gl_context,
            rendered: false,
            #[cfg(feature = "test-engine")]
            secondary_viewport_evidence: None,
            #[cfg(feature = "test-engine")]
            contract_probe,
            #[cfg(feature = "test-engine")]
            contract_result: None,
            presented: false,
        };
        #[cfg(feature = "test-engine")]
        let presentation_result: Result<(), Box<dyn Error>> =
            if let Some(engine) = self.test_engine.as_mut() {
                engine
                    .drive_frame(frame, frame_index, &mut driver)
                    .map_err(|error| Box::new(error) as Box<dyn Error>)
            } else {
                let reconciled = driver.render_frame(frame)?;
                drop(reconciled);
                driver
                    .present_frame()
                    .map_err(|error| Box::new(error) as Box<dyn Error>)
            };
        #[cfg(not(feature = "test-engine"))]
        let presentation_result: Result<(), Box<dyn Error>> = {
            let reconciled = driver.render_frame(frame)?;
            drop(reconciled);
            driver
                .present_frame()
                .map_err(|error| Box::new(error) as Box<dyn Error>)
        };

        #[cfg(feature = "test-engine")]
        let secondary_viewport_evidence = driver
            .secondary_viewport_evidence
            .take()
            .unwrap_or_default();
        #[cfg(feature = "test-engine")]
        let contract_result = driver.contract_result.take();
        #[cfg(feature = "test-engine")]
        let was_presented = driver.presented;
        drop(driver);
        presentation_result?;

        #[cfg(feature = "test-engine")]
        let external_texture_filters_preserved =
            self.external_texture.filters_are_preserved(&self.gl);
        #[cfg(feature = "test-engine")]
        let render_state_cleared_after_callback = self.imgui.binding().with_bound_context(|| {
            matches!(
                unsafe { GlowRenderState::with_current(|_| ()) },
                Err(GlowRenderStateAccessError::Inactive)
            )
        });

        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            let contract_result = contract_result
                .as_ref()
                .ok_or("Glow contract probe did not produce framebuffer evidence")?;
            let contract = GlowContractEvidence {
                external_texture_filters_preserved,
                sampler_pixels_prove_isolation: contract_result.readback.proves_sampler_isolation(),
                raw_callback_typed_state_observed: raw_callback_probe_passed(
                    smoke.sampler_strategy,
                ),
                reset_render_state_recovered: reset_render_state_probe_passed(),
                render_state_cleared_after_callback,
                application_gl_state_restored: contract_result.application_gl_state_restored,
            };
            if !contract.external_texture_filters_preserved {
                return Err(format!(
                    "Glow changed application-owned texture filters: expected {:?}, got {:?}",
                    self.external_texture.expected_filters,
                    self.external_texture.filters(&self.gl)
                )
                .into());
            }
            if !contract.sampler_pixels_prove_isolation {
                return Err(format!(
                    "Glow sampler readback did not distinguish nearest, linear, and reset-linear draws: {:?}",
                    contract_result.readback
                )
                .into());
            }
            if !contract.raw_callback_typed_state_observed {
                return Err("Glow raw callbacks did not observe scoped typed render state".into());
            }
            if !contract.reset_render_state_recovered {
                return Err("Glow reset did not recover its VAO and framebuffer sRGB state".into());
            }
            if !contract.render_state_cleared_after_callback {
                return Err("Glow Renderer_RenderState remained published after rendering".into());
            }
            if !contract.application_gl_state_restored {
                return Err("Glow did not restore the application OpenGL state snapshot".into());
            }
            smoke.contract = contract;
        }

        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut()
            && smoke.completed_frame_evidence.is_none()
            && !secondary_viewport_evidence.completed_viewports.is_empty()
        {
            smoke.completed_frame_evidence = Some(secondary_viewport_evidence);
            smoke.main_present_bracketed_by_test_engine = used_test_engine && was_presented;
        }

        #[cfg(feature = "test-engine")]
        if let Some(engine) = self.test_engine.as_mut() {
            let smoke_pending = self
                .viewport_smoke
                .as_ref()
                .is_some_and(|smoke| !smoke.complete);
            if smoke_pending && let Some(summary) = engine.take_terminal_summary()? {
                if summary.count_tested != 1 || summary.count_success != 1 {
                    return Err(format!(
                        "viewport smoke failed: tested={}, success={}",
                        summary.count_tested, summary.count_success
                    )
                    .into());
                }
                let smoke = self
                    .viewport_smoke
                    .as_mut()
                    .expect("a pending viewport smoke state must exist");
                if !smoke.saw_secondary_viewport
                    || smoke.completed_frame_evidence.is_none()
                    || !smoke.saw_merged_viewport
                    || !smoke.main_present_bracketed_by_test_engine
                    || !smoke.contract.is_complete()
                {
                    let evidence = smoke.completed_frame_evidence.as_ref();
                    return Err(format!(
                        "viewport smoke did not observe the complete viewport, sampler, and callback contract: secondary={}, context_ready={:?}, glow_drawn={:?}, swapped={:?}, completed={:?}, merged={}, main_present_bracketed={}, contract={:?}",
                        smoke.saw_secondary_viewport,
                        evidence.map(|evidence| &evidence.context_activated_viewports),
                        evidence.map(|evidence| &evidence.glow_rendered_viewports),
                        evidence.map(|evidence| &evidence.swapped_viewports),
                        evidence.map(|evidence| &evidence.completed_viewports),
                        smoke.saw_merged_viewport,
                        smoke.main_present_bracketed_by_test_engine,
                        smoke.contract,
                    )
                    .into());
                }
                println!("SDL3/Glow multi-viewport Test Engine smoke passed");
                smoke.complete = true;
            }
        }
        #[cfg(feature = "test-engine")]
        return Ok(self
            .viewport_smoke
            .as_ref()
            .is_some_and(|smoke| smoke.complete));

        #[cfg(not(feature = "test-engine"))]
        Ok(false)
    }

    fn shutdown(mut self) -> Result<RunResult, Box<dyn Error>> {
        #[cfg(feature = "test-engine")]
        let completed_result = self
            .viewport_smoke
            .as_ref()
            .and_then(ViewportSmokeState::completed_result);
        #[cfg(feature = "test-engine")]
        if let Some(engine) = self.test_engine.as_mut() {
            engine.shutdown()?;
        }
        self.window.gl_make_current(&self.gl_context)?;
        self.renderer.shutdown(&mut self.imgui)?;
        self.sdl3_backend.shutdown(&mut self.imgui)?;
        self.window.gl_make_current(&self.gl_context)?;
        unsafe { self.gl.delete_texture(self.external_texture.handle) };

        #[cfg(feature = "test-engine")]
        return Ok(completed_result);
        #[cfg(not(feature = "test-engine"))]
        Ok(())
    }
}

#[app_impl]
impl GlowApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 Glow example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        match self.process_events() {
            Ok(AppResult::Continue) => {}
            Ok(result) => return result,
            Err(error) => {
                eprintln!("SDL3 Glow event processing failed: {error}");
                return AppResult::Failure;
            }
        }
        match self.render() {
            Ok(true) => AppResult::Success,
            Ok(false) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 Glow frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        self.events.push(raw);
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state {
            app.shutdown();
        }
    }
}

/// Create a Glow context from an SDL3 `VideoSubsystem`.
///
/// # Safety
///
/// Call this only after there is a current OpenGL context for the thread.
unsafe fn create_glow_context(video: &sdl3::VideoSubsystem) -> glow::Context {
    use std::ffi::c_void;

    unsafe {
        glow::Context::from_loader_function(|name| {
            video
                .gl_get_proc_address(name)
                .map(|function| function as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    }
}
