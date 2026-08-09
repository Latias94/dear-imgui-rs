//! Private SDL3 + Glow multi-viewport runtime contract for CI.

// The shared lifecycle also exposes the scenario used only by the interactive example.
#[allow(dead_code)]
#[path = "../support/sdl3_glow_multi_viewport_runtime.rs"]
mod sdl3_glow_multi_viewport_runtime;

use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use dear_imgui_glow::{
    GlBuffer, GlProgram, GlSampler, GlTexture, GlVersion, GlVertexArray, GlowRenderState,
    GlowRenderStateAccessError, GlowRenderer, GlowSamplerStrategy,
    multi_viewport::GlowPreparedViewportFrame,
};
use dear_imgui_rs::{Context, FrameToken, Id, RawDrawCallback, TextureId, Ui};
use dear_imgui_test_engine::{
    FrameDriveOutcome, MainRenderOutcome, RunFlags, RunSpeed, ScriptCount, TestEngine,
    TestFrameDriver, TestGroup, VerboseLevel,
};
use glow::HasContext;
use sdl3::video::SwapInterval;
use sdl3_glow_multi_viewport_runtime::{
    ExternalTextureView, GlowApp, MainDrawDataTransform, MainFrameReport, SdlGlowFrameDriver,
    SdlGlowFrameError, SecondaryViewportFrameEvidence, ViewportScenario,
};
use sdl3_main::{AppResult, AppResultWithState, app_impl};

static RAW_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);
static RAW_CALLBACK_MUTATOR_TYPED_STATE: AtomicBool = AtomicBool::new(false);
static RAW_CALLBACK_PROBE_TYPED_STATE: AtomicBool = AtomicBool::new(false);
static RAW_CALLBACK_NESTED_BORROW_REJECTED: AtomicBool = AtomicBool::new(false);
static RAW_CALLBACK_RESET_STATE_OBSERVED: AtomicBool = AtomicBool::new(false);
static RAW_CALLBACK_SAMPLER_STRATEGY: AtomicUsize = AtomicUsize::new(0);

fn sampler_strategy_code(strategy: GlowSamplerStrategy) -> usize {
    match strategy {
        GlowSamplerStrategy::SamplerObjects => 1,
        GlowSamplerStrategy::TextureParameters => 2,
        _ => 0,
    }
}

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

fn reset_raw_callback_probe() {
    RAW_CALLBACK_COUNT.store(0, Ordering::SeqCst);
    RAW_CALLBACK_MUTATOR_TYPED_STATE.store(false, Ordering::SeqCst);
    RAW_CALLBACK_PROBE_TYPED_STATE.store(false, Ordering::SeqCst);
    RAW_CALLBACK_NESTED_BORROW_REJECTED.store(false, Ordering::SeqCst);
    RAW_CALLBACK_RESET_STATE_OBSERVED.store(false, Ordering::SeqCst);
    RAW_CALLBACK_SAMPLER_STRATEGY.store(0, Ordering::SeqCst);
}

fn raw_callback_probe_passed(expected_strategy: GlowSamplerStrategy) -> bool {
    RAW_CALLBACK_COUNT.load(Ordering::SeqCst) == 2
        && RAW_CALLBACK_MUTATOR_TYPED_STATE.load(Ordering::SeqCst)
        && RAW_CALLBACK_PROBE_TYPED_STATE.load(Ordering::SeqCst)
        && RAW_CALLBACK_NESTED_BORROW_REJECTED.load(Ordering::SeqCst)
        && RAW_CALLBACK_SAMPLER_STRATEGY.load(Ordering::SeqCst)
            == sampler_strategy_code(expected_strategy)
}

fn reset_render_state_probe_passed() -> bool {
    RAW_CALLBACK_RESET_STATE_OBSERVED.load(Ordering::SeqCst)
}

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

#[derive(Clone, Copy)]
struct SamplerProbeScreenPoints {
    nearest: [f32; 2],
    linear: [f32; 2],
    reset_linear: [f32; 2],
}

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
        transform: MainDrawDataTransform,
    ) -> Result<SamplerReadbackTargets, Box<dyn Error>> {
        let display_pos = transform.display_pos();
        let scale = transform.framebuffer_scale();
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

#[derive(Clone, Copy)]
struct SamplerReadbackTargets {
    nearest: [i32; 2],
    linear: [i32; 2],
    reset_linear: [i32; 2],
}

struct PixelReadbackStateGuard<'gl> {
    gl: &'gl glow::Context,
    pack_buffer: Option<GlBuffer>,
    pack_alignment: i32,
}

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

#[derive(Clone, Copy, Debug)]
struct SamplerPixelReadback {
    nearest: [u8; 4],
    linear: [u8; 4],
    reset_linear: [u8; 4],
}

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

#[derive(Clone, Copy, Debug, Default)]
struct GlowContractEvidence {
    external_texture_filters_preserved: bool,
    sampler_pixels_prove_isolation: bool,
    raw_callback_typed_state_observed: bool,
    reset_render_state_recovered: bool,
    render_state_cleared_after_callback: bool,
    application_gl_state_restored: bool,
}

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

#[derive(Clone)]
struct OpenGlRendererInfo {
    vendor: String,
    renderer: String,
    version: String,
}

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

struct CompletedViewportSmoke {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    sampler_strategy: GlowSamplerStrategy,
    glow_draw_issued_viewports: Vec<Id>,
    saw_merged_viewport: bool,
    main_present_bracketed_by_test_engine: bool,
    contract: GlowContractEvidence,
}

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
            glow_draw_issued_viewports: evidence.glow_rendered_viewports.clone(),
            saw_merged_viewport: self.saw_merged_viewport,
            main_present_bracketed_by_test_engine: self.main_present_bracketed_by_test_engine,
            contract: self.contract,
        })
    }
}

impl CompletedViewportSmoke {
    fn write_after_teardown(self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.result_path else {
            return Ok(());
        };
        let json = format!(
            "{{\"schema_version\":6,\"renderer\":{{\"backend\":\"OpenGL\",\"vendor\":\"{}\",\"name\":\"{}\",\"version\":\"{}\"}},\"sampler_strategy\":\"{}\",\"secondary_draw_issued_before_main_present_viewport_ids\":{},\"merge_observed\":{},\"main_present_bracketed_by_test_engine\":{},\"external_texture_filters_preserved\":{},\"sampler_pixels_prove_isolation\":{},\"raw_callback_typed_state_observed\":{},\"reset_render_state_recovered\":{},\"render_state_cleared_after_callback\":{},\"application_gl_state_restored\":{}}}",
            json_escape(&self.renderer.vendor),
            json_escape(&self.renderer.renderer),
            json_escape(&self.renderer.version),
            sampler_strategy_name(self.sampler_strategy),
            viewport_ids_json(&self.glow_draw_issued_viewports),
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

fn sampler_strategy_name(strategy: GlowSamplerStrategy) -> &'static str {
    match strategy {
        GlowSamplerStrategy::SamplerObjects => "sampler_objects",
        GlowSamplerStrategy::TextureParameters => "texture_parameters",
        _ => "unknown",
    }
}

fn viewport_ids_json(ids: &[Id]) -> String {
    let ids = ids
        .iter()
        .map(|id| id.raw().to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{ids}]")
}

fn query_opengl_renderer(gl: &glow::Context) -> OpenGlRendererInfo {
    unsafe {
        OpenGlRendererInfo {
            vendor: gl.get_parameter_string(glow::VENDOR),
            renderer: gl.get_parameter_string(glow::RENDERER),
            version: gl.get_parameter_string(glow::VERSION),
        }
    }
}

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

const EXPECTED_EXTERNAL_TEXTURE_FILTERS: [i32; 2] =
    [glow::LINEAR_MIPMAP_NEAREST as i32, glow::NEAREST as i32];

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

fn external_texture_filters(gl: &glow::Context, texture: GlTexture) -> [i32; 2] {
    let _binding = TextureUnitZeroGuard::bind(gl, texture);
    unsafe {
        [
            gl.get_tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER),
            gl.get_tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER),
        ]
    }
}

#[derive(Clone, Copy)]
struct GlowCallbacks {
    reset_render_state: RawDrawCallback,
    sampler_linear: RawDrawCallback,
    sampler_nearest: RawDrawCallback,
}

#[derive(Clone, Copy)]
struct GlowFrameContractProbe {
    screen_points: SamplerProbeScreenPoints,
    external_texture: GlTexture,
    gl_version: GlVersion,
    supports_sampler_objects: bool,
}

#[derive(Debug)]
struct GlowFrameContractResult {
    readback: SamplerPixelReadback,
    application_gl_state_restored: bool,
}

struct SmokeFrameDriver<'driver, 'runtime> {
    inner: &'driver mut SdlGlowFrameDriver<'runtime>,
    probe: GlowFrameContractProbe,
    contract_result: Option<GlowFrameContractResult>,
}

impl TestFrameDriver for SmokeFrameDriver<'_, '_> {
    type PreparedFrame<'frame> = GlowPreparedViewportFrame<'frame>;
    type PrepareError = SdlGlowFrameError;
    type RenderError = SdlGlowFrameError;
    type PresentError = SdlGlowFrameError;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        self.inner.prepare_frame(frame)
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId {
        frame.context_id()
    }

    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        _frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError> {
        self.inner.prepare_main_target()?;
        let before = ApplicationGlStateSnapshot::prepare(
            self.inner.gl(),
            self.probe.external_texture,
            self.probe.gl_version,
            self.probe.supports_sampler_objects,
        );
        self.inner.render_main_frame(frame)?;
        let report = self.inner.main_report().ok_or_else(|| {
            SdlGlowFrameError::message("Glow render-main did not publish its frame report")
        })?;
        let readback_targets = self
            .probe
            .screen_points
            .map_to_framebuffer(report.draw_data_transform())
            .map_err(SdlGlowFrameError::boxed)?;
        let after_renderer = ApplicationGlStateSnapshot::capture(
            self.inner.gl(),
            self.probe.gl_version,
            self.probe.supports_sampler_objects,
        );
        let readback = readback_targets
            .read(self.inner.gl(), self.inner.window().size_in_pixels())
            .map_err(SdlGlowFrameError::boxed)?;
        let after_readback = ApplicationGlStateSnapshot::capture(
            self.inner.gl(),
            self.probe.gl_version,
            self.probe.supports_sampler_objects,
        );
        self.contract_result = Some(GlowFrameContractResult {
            readback,
            application_gl_state_restored: before == after_renderer
                && after_renderer == after_readback,
        });
        Ok(MainRenderOutcome::ReadyToPresent)
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        self.inner.present_frame()
    }
}

struct ViewportSmokeScenario {
    result_path: Option<PathBuf>,
    require_software_opengl: bool,
    external_texture: Option<ExternalTextureView>,
    callbacks: Option<GlowCallbacks>,
    test_engine: Option<TestEngine>,
    smoke: Option<ViewportSmokeState>,
    sampler_probe: Option<SamplerProbeScreenPoints>,
    contract_result: Option<GlowFrameContractResult>,
    main_present_bracketed_by_test_engine: bool,
}

impl ViewportSmokeScenario {
    fn from_environment() -> Result<Self, Box<dyn Error>> {
        if !std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1") {
            return Err(
                "private SDL3/Glow viewport runtime requires DEAR_IMGUI_VIEWPORT_SMOKE=1".into(),
            );
        }
        Ok(Self {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            require_software_opengl: std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_OPENGL")
                .is_ok_and(|value| value == "1"),
            external_texture: None,
            callbacks: None,
            test_engine: None,
            smoke: None,
            sampler_probe: None,
            contract_result: None,
            main_present_bracketed_by_test_engine: false,
        })
    }

    fn smoke(&self) -> Result<&ViewportSmokeState, Box<dyn Error>> {
        self.smoke
            .as_ref()
            .ok_or_else(|| "SDL3/Glow viewport smoke was not initialized".into())
    }

    fn smoke_mut(&mut self) -> Result<&mut ViewportSmokeState, Box<dyn Error>> {
        self.smoke
            .as_mut()
            .ok_or_else(|| "SDL3/Glow viewport smoke was not initialized".into())
    }
}

impl ViewportScenario for ViewportSmokeScenario {
    type Output = CompletedViewportSmoke;

    fn swap_interval(&self) -> SwapInterval {
        SwapInterval::Immediate
    }

    fn configure_context(&mut self, context: &mut Context) -> Result<(), Box<dyn Error>> {
        context.set_ini_filename(None::<String>)?;
        Ok(())
    }

    fn initialize(
        &mut self,
        context: &mut Context,
        renderer: &mut GlowRenderer,
        gl: &glow::Context,
        window: &sdl3::video::Window,
        external_texture: ExternalTextureView,
    ) -> Result<(), Box<dyn Error>> {
        let renderer_info = query_opengl_renderer(gl);
        println!(
            "OpenGL renderer: vendor='{}', renderer='{}', version='{}'",
            renderer_info.vendor, renderer_info.renderer, renderer_info.version
        );
        if self.require_software_opengl {
            validate_software_opengl_renderer(&renderer_info)?;
        }
        let sampler_strategy = if renderer.supports_sampler_objects() {
            GlowSamplerStrategy::SamplerObjects
        } else {
            GlowSamplerStrategy::TextureParameters
        };
        renderer.set_framebuffer_srgb_enabled(true)?;
        let callbacks = GlowCallbacks {
            reset_render_state: context
                .platform_io()
                .draw_callback_reset_render_state_raw()
                .ok_or("Glow did not publish its reset-render-state callback")?,
            sampler_linear: context
                .platform_io()
                .draw_callback_set_sampler_linear_raw()
                .ok_or("Glow did not publish its linear sampler callback")?,
            sampler_nearest: context
                .platform_io()
                .draw_callback_set_sampler_nearest_raw()
                .ok_or("Glow did not publish its nearest sampler callback")?,
        };

        let (main_x, main_y) = window.position();
        let (main_width, _) = window.size();
        let external_pos = [
            main_x as f32 + main_width as f32 + 100.0,
            main_y as f32 + 100.0,
        ];
        let merged_pos = [main_x as f32 + 100.0, main_y as f32 + 100.0];
        let mut engine = TestEngine::create()?;
        engine.start(context)?;
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

        self.external_texture = Some(external_texture);
        self.callbacks = Some(callbacks);
        self.test_engine = Some(engine);
        self.smoke = Some(ViewportSmokeState {
            result_path: self.result_path.clone(),
            renderer: renderer_info,
            sampler_strategy,
            saw_secondary_viewport: false,
            completed_frame_evidence: None,
            saw_merged_viewport: false,
            main_present_bracketed_by_test_engine: false,
            contract: GlowContractEvidence::default(),
            complete: false,
        });
        Ok(())
    }

    fn before_ui(&mut self, viewport_count: i32) {
        if let Some(smoke) = self.smoke.as_mut() {
            if viewport_count > 1 {
                smoke.saw_secondary_viewport = true;
            } else if smoke.saw_secondary_viewport {
                smoke.saw_merged_viewport = true;
            }
        }
        reset_raw_callback_probe();
    }

    fn extend_main_window(&mut self, ui: &Ui, viewport_count: &mut i32) {
        ui.input_int_config("Viewport Count")
            .flags(dear_imgui_rs::InputScalarFlags::READ_ONLY)
            .build(viewport_count);
    }

    fn after_ui(&mut self, ui: &Ui, external_texture: TextureId) -> Result<(), Box<dyn Error>> {
        let callbacks = self
            .callbacks
            .ok_or("SDL3/Glow callbacks were not initialized")?;
        self.sampler_probe = Some(SamplerProbeScreenPoints::submit(
            ui,
            external_texture,
            callbacks.sampler_nearest,
            callbacks.sampler_linear,
            callbacks.reset_render_state,
        ));
        Ok(())
    }

    fn drive_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        frame_index: u64,
        driver: &mut SdlGlowFrameDriver<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let external_texture = self
            .external_texture
            .ok_or("SDL3/Glow external texture was not initialized")?;
        let sampler_strategy = self.smoke()?.sampler_strategy;
        let probe = GlowFrameContractProbe {
            screen_points: self
                .sampler_probe
                .take()
                .ok_or("SDL3/Glow sampler probe was not submitted")?,
            external_texture: external_texture.handle(),
            gl_version: GlVersion::read(driver.gl()),
            supports_sampler_objects: matches!(
                sampler_strategy,
                GlowSamplerStrategy::SamplerObjects
            ),
        };
        let mut smoke_driver = SmokeFrameDriver {
            inner: driver,
            probe,
            contract_result: None,
        };
        let outcome = self
            .test_engine
            .as_mut()
            .ok_or("SDL3/Glow Test Engine was not initialized")?
            .drive_frame(frame, frame_index, &mut smoke_driver)
            .map_err(|error| Box::new(error) as Box<dyn Error>)?;
        self.contract_result = smoke_driver.contract_result.take();
        self.main_present_bracketed_by_test_engine =
            matches!(outcome, FrameDriveOutcome::Presented);
        Ok(())
    }

    fn after_frame(
        &mut self,
        context: &Context,
        gl: &glow::Context,
        presented: bool,
        main_report: Option<MainFrameReport>,
    ) -> Result<(), Box<dyn Error>> {
        let main_report = main_report.ok_or("Glow frame did not produce a main report")?;
        let secondary_viewport_evidence = main_report.secondary_viewports.unwrap_or_default();
        let contract_result = self
            .contract_result
            .take()
            .ok_or("Glow contract probe did not produce framebuffer evidence")?;
        let external_texture = self
            .external_texture
            .ok_or("SDL3/Glow external texture was not initialized")?;
        let actual_filters = external_texture_filters(gl, external_texture.handle());
        let external_texture_filters_preserved =
            actual_filters == EXPECTED_EXTERNAL_TEXTURE_FILTERS;
        let render_state_cleared_after_callback = context.binding().with_bound_context(|| {
            matches!(
                unsafe { GlowRenderState::with_current(|_| ()) },
                Err(GlowRenderStateAccessError::Inactive)
            )
        });
        let sampler_strategy = self.smoke()?.sampler_strategy;
        let contract = GlowContractEvidence {
            external_texture_filters_preserved,
            sampler_pixels_prove_isolation: contract_result.readback.proves_sampler_isolation(),
            raw_callback_typed_state_observed: raw_callback_probe_passed(sampler_strategy),
            reset_render_state_recovered: reset_render_state_probe_passed(),
            render_state_cleared_after_callback,
            application_gl_state_restored: contract_result.application_gl_state_restored,
        };
        if !contract.external_texture_filters_preserved {
            return Err(format!(
                "Glow changed application-owned texture filters: expected {:?}, got {:?}",
                EXPECTED_EXTERNAL_TEXTURE_FILTERS, actual_filters
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

        let main_present_bracketed = self.main_present_bracketed_by_test_engine && presented;
        {
            let smoke = self.smoke_mut()?;
            smoke.contract = contract;
            if smoke.completed_frame_evidence.is_none()
                && !secondary_viewport_evidence
                    .glow_rendered_viewports
                    .is_empty()
            {
                smoke.completed_frame_evidence = Some(secondary_viewport_evidence);
                smoke.main_present_bracketed_by_test_engine = main_present_bracketed;
            }
        }

        let smoke_pending = !self.smoke()?.complete;
        if smoke_pending
            && let Some(summary) = self
                .test_engine
                .as_mut()
                .ok_or("SDL3/Glow Test Engine was not initialized")?
                .take_terminal_summary()?
        {
            if summary.count_tested != 1 || summary.count_success != 1 {
                return Err(format!(
                    "viewport smoke failed: tested={}, success={}",
                    summary.count_tested, summary.count_success
                )
                .into());
            }
            let smoke = self.smoke_mut()?;
            if !smoke.saw_secondary_viewport
                || smoke.completed_frame_evidence.is_none()
                || !smoke.saw_merged_viewport
                || !smoke.main_present_bracketed_by_test_engine
                || !smoke.contract.is_complete()
            {
                let evidence = smoke.completed_frame_evidence.as_ref();
                return Err(format!(
                    "viewport smoke did not observe the complete owning-route, sampler, and callback contract: secondary={}, glow_drawn={:?}, merged={}, main_present_bracketed={}, contract={:?}",
                    smoke.saw_secondary_viewport,
                    evidence.map(|evidence| &evidence.glow_rendered_viewports),
                    smoke.saw_merged_viewport,
                    smoke.main_present_bracketed_by_test_engine,
                    smoke.contract,
                )
                .into());
            }
            println!("SDL3/Glow multi-viewport Test Engine smoke passed");
            smoke.complete = true;
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.smoke.as_ref().is_some_and(|smoke| smoke.complete)
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(engine) = self.test_engine.as_mut() {
            engine.shutdown()?;
        }
        Ok(())
    }

    fn take_output(&mut self) -> Option<Self::Output> {
        self.smoke
            .as_ref()
            .and_then(ViewportSmokeState::completed_result)
    }

    fn finish_after_teardown(output: Self::Output) -> Result<(), Box<dyn Error>> {
        output.write_after_teardown()
    }
}

struct SmokeApp(GlowApp<ViewportSmokeScenario>);

#[app_impl]
impl SmokeApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        let scenario = match ViewportSmokeScenario::from_environment() {
            Ok(scenario) => scenario,
            Err(error) => {
                eprintln!("invalid SDL3/Glow viewport smoke configuration: {error}");
                return AppResultWithState::Failure(None);
            }
        };
        match GlowApp::new(scenario) {
            Ok(app) => AppResultWithState::Continue(Box::new(Self(app))),
            Err(error) => {
                eprintln!("failed to initialize SDL3/Glow viewport smoke: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        self.0.iterate()
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        // SAFETY: SDL supplies a valid event whose transient payload remains live for this call.
        unsafe { self.0.queue_event(raw) };
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state {
            app.0.shutdown();
        }
    }
}
