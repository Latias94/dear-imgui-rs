//! Exercise standard sampler commands through the production draw-list interpreter.

use std::{cell::RefCell, num::NonZeroU32};

use dear_imgui_rs::{Context, FramePrepareOptions, TextureId, render::RendererRenderStateGuard};

use super::{FramebufferExtent, GlowRenderer, VertexArrayGuard};
use crate::{
    GlVersion,
    renderer::{
        callbacks::{
            draw_callback_reset_render_state, draw_callback_set_sampler_from_texture,
            draw_callback_set_sampler_linear, draw_callback_set_sampler_nearest,
        },
        sampler::SamplerObjects,
    },
    shaders::Shaders,
    state::{GlowRenderStateStorage, GlowSamplerStrategy},
    texture::TextureRegistry,
};

const ORIGINAL_MIN_FILTER: i32 = glow::LINEAR_MIPMAP_LINEAR as i32;
const ORIGINAL_MAG_FILTER: i32 = glow::LINEAR as i32;

#[derive(Debug, PartialEq)]
struct DrawSample {
    sampler: u32,
    min_filter: i32,
    mag_filter: i32,
    parameter_writes: usize,
}

struct FakeGl {
    next_sampler: u32,
    sampler: u32,
    sampler_bindings: Vec<u32>,
    min_filter: i32,
    mag_filter: i32,
    parameter_writes: usize,
    draws: Vec<DrawSample>,
}

impl Default for FakeGl {
    fn default() -> Self {
        Self {
            next_sampler: 40,
            sampler: 0,
            sampler_bindings: Vec::new(),
            min_filter: ORIGINAL_MIN_FILTER,
            mag_filter: ORIGINAL_MAG_FILTER,
            parameter_writes: 0,
            draws: Vec::new(),
        }
    }
}

thread_local! {
    static GL_STATE: RefCell<FakeGl> = RefCell::new(FakeGl::default());
}

unsafe extern "system" fn get_string(name: u32) -> *const u8 {
    if name == glow::VERSION {
        c"3.3".as_ptr().cast()
    } else {
        c"".as_ptr().cast()
    }
}

unsafe extern "system" fn get_integer(_name: u32, value: *mut i32) {
    unsafe { *value = 0 };
}

unsafe extern "system" fn gen_samplers(count: i32, samplers: *mut u32) {
    GL_STATE.with_borrow_mut(|state| {
        for index in 0..count as usize {
            unsafe { *samplers.add(index) = state.next_sampler };
            state.next_sampler += 1;
        }
    });
}

unsafe extern "system" fn bind_sampler(_unit: u32, sampler: u32) {
    GL_STATE.with_borrow_mut(|state| {
        state.sampler = sampler;
        state.sampler_bindings.push(sampler);
    });
}

unsafe extern "system" fn sampler_parameter(_sampler: u32, _name: u32, _value: i32) {}
unsafe extern "system" fn delete_objects(_count: i32, _objects: *const u32) {}
unsafe extern "system" fn gen_vertex_arrays(_count: i32, arrays: *mut u32) {
    unsafe { *arrays = 44 };
}
unsafe extern "system" fn bind_vertex_array(_array: u32) {}
unsafe extern "system" fn one_u32(_value: u32) {}
unsafe extern "system" fn two_u32(_first: u32, _second: u32) {}
unsafe extern "system" fn blend_func(_src_rgb: u32, _dst_rgb: u32, _src_a: u32, _dst_a: u32) {}
unsafe extern "system" fn four_i32(_x: i32, _y: i32, _w: i32, _h: i32) {}
unsafe extern "system" fn vertex_attrib(
    _index: u32,
    _size: i32,
    _kind: u32,
    _normalized: u8,
    _stride: i32,
    _pointer: *const std::ffi::c_void,
) {
}
unsafe extern "system" fn buffer_data(
    _target: u32,
    _size: isize,
    _data: *const std::ffi::c_void,
    _usage: u32,
) {
}

unsafe extern "system" fn get_texture_parameter(_target: u32, name: u32, value: *mut i32) {
    GL_STATE.with_borrow(|state| unsafe {
        *value = match name {
            glow::TEXTURE_MIN_FILTER => state.min_filter,
            glow::TEXTURE_MAG_FILTER => state.mag_filter,
            _ => unreachable!(),
        };
    });
}

unsafe extern "system" fn texture_parameter(_target: u32, name: u32, value: i32) {
    GL_STATE.with_borrow_mut(|state| {
        match name {
            glow::TEXTURE_MIN_FILTER => state.min_filter = value,
            glow::TEXTURE_MAG_FILTER => state.mag_filter = value,
            _ => unreachable!(),
        }
        state.parameter_writes += 1;
    });
}

unsafe extern "system" fn draw_elements(
    _mode: u32,
    _count: i32,
    _kind: u32,
    _indices: *const std::ffi::c_void,
) {
    GL_STATE.with_borrow_mut(|state| {
        state.draws.push(DrawSample {
            sampler: state.sampler,
            min_filter: state.min_filter,
            mag_filter: state.mag_filter,
            parameter_writes: state.parameter_writes,
        });
    });
}

unsafe extern "system" fn draw_elements_base_vertex(
    mode: u32,
    count: i32,
    kind: u32,
    indices: *const std::ffi::c_void,
    _base_vertex: i32,
) {
    unsafe { draw_elements(mode, count, kind, indices) };
}

fn fake_gl(has_sampler_objects: bool) -> glow::Context {
    // SAFETY: Each installed entry point has the OpenGL ABI and runs on this test thread.
    unsafe {
        glow::Context::from_loader_function(|name| {
            match name {
                "glGetString" => get_string as *const (),
                "glGetIntegerv" => get_integer as *const (),
                "glGenSamplers" => gen_samplers as *const (),
                "glBindSampler" if has_sampler_objects => bind_sampler as *const (),
                "glSamplerParameteri" => sampler_parameter as *const (),
                "glDeleteSamplers" | "glDeleteVertexArrays" => delete_objects as *const (),
                "glGenVertexArrays" => gen_vertex_arrays as *const (),
                "glBindVertexArray" => bind_vertex_array as *const (),
                "glActiveTexture"
                | "glEnable"
                | "glDisable"
                | "glBlendEquation"
                | "glUseProgram"
                | "glEnableVertexAttribArray" => one_u32 as *const (),
                "glPolygonMode" | "glBindBuffer" | "glBindTexture" => two_u32 as *const (),
                "glBlendFuncSeparate" => blend_func as *const (),
                "glViewport" | "glScissor" => four_i32 as *const (),
                "glVertexAttribPointer" => vertex_attrib as *const (),
                "glBufferData" => buffer_data as *const (),
                "glGetTexParameteriv" => get_texture_parameter as *const (),
                "glTexParameteri" => texture_parameter as *const (),
                "glDrawElements" => draw_elements as *const (),
                "glDrawElementsBaseVertex" => draw_elements_base_vertex as *const (),
                _ => std::ptr::null(),
            }
            .cast()
        })
    }
}

fn renderer(gl: &glow::Context, has_sampler_objects: bool) -> GlowRenderer {
    GlowRenderer {
        shaders: Shaders {
            program: None,
            attrib_location_tex: None,
            attrib_location_proj_mtx: None,
            attrib_location_color_gamma: None,
            attrib_location_vtx_pos: 0,
            attrib_location_vtx_uv: 1,
            attrib_location_vtx_color: 2,
        },
        vbo_handle: None,
        ebo_handle: None,
        samplers: has_sampler_objects.then(|| SamplerObjects::create(gl).unwrap()),
        gl_version: GlVersion {
            major: 3,
            minor: if has_sampler_objects { 3 } else { 0 },
            is_es: false,
        },
        has_clip_origin_support: false,
        clip_origin: Default::default(),
        has_separate_polygon_modes: false,
        has_sampler_object_support: has_sampler_objects,
        gl_context: None,
        context_binding: None,
        backend_user_data: Box::default(),
        renderer_name_ptr: std::ptr::null(),
        renderer_texture_max: [0, 0],
        renderer_state_fault: None,
        synthetic_test_renderer: true,
        texture_registry: TextureRegistry::default(),
        managed_textures: Default::default(),
        destroyed_managed_textures: Default::default(),
        renderer_consumer: None,
        framebuffer_srgb: false,
        color_gamma_override: None,
        viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
    }
}

fn draw_image(draw_list: &dear_imgui_rs::DrawListMut<'_>, texture: TextureId) {
    draw_list.add_image(
        texture,
        [1.0, 1.0],
        [8.0, 8.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn render_sampler_sequence(has_sampler_objects: bool) {
    GL_STATE.set(FakeGl::default());
    let gl = fake_gl(has_sampler_objects);
    let mut renderer = renderer(&gl, has_sampler_objects);
    let texture = renderer
        .texture_registry
        .register_external(glow::NativeTexture(NonZeroU32::new(9).unwrap()))
        .unwrap()
        .texture_id();
    let mut context = Context::create();
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        platform_io.set_draw_callback_set_sampler_from_texture_raw(Some(
            draw_callback_set_sampler_from_texture,
        ));
    }
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    context.prepare_frame(FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0));
    let ui = context.frame();
    let draw_list = ui.get_background_draw_list();
    draw_list.set_sampler_nearest();
    draw_image(&draw_list, texture);
    draw_list.set_sampler_from_texture();
    draw_image(&draw_list, texture);
    draw_list.set_sampler_linear();
    draw_image(&draw_list, texture);
    draw_list.set_sampler_from_texture();
    draw_image(&draw_list, texture);
    unsafe {
        draw_list.add_callback(draw_callback_reset_render_state, std::ptr::null_mut(), 0);
    }
    draw_image(&draw_list, texture);
    drop(draw_list);
    let frame = context.render_legacy();
    let draw_data = frame.draw_data();
    let extent = FramebufferExtent::from_draw_data(draw_data)
        .unwrap()
        .unwrap();
    let vertex_array = VertexArrayGuard::create_and_bind(&gl).unwrap();
    let strategy = if has_sampler_objects {
        GlowSamplerStrategy::SamplerObjects
    } else {
        GlowSamplerStrategy::TextureParameters
    };
    let mut callback_state = GlowRenderStateStorage::new(&gl, strategy);
    // SAFETY: The current Context and callback storage remain alive for this render scope.
    let render_state = unsafe {
        RendererRenderStateGuard::install(
            dear_imgui_rs::sys::igGetPlatformIO_Nil(),
            &mut callback_state,
        )
    }
    .unwrap();
    renderer
        .set_up_render_state(&gl, draw_data, extent, &vertex_array, None)
        .unwrap();
    renderer
        .render_draw_lists(
            &gl,
            &renderer.texture_registry,
            draw_data,
            extent,
            &vertex_array,
            None,
            &render_state,
        )
        .unwrap();
    render_state.finish().unwrap();
    if let Some(samplers) = renderer.samplers.take() {
        samplers.destroy(&gl);
    }
}

#[test]
fn texture_owned_sampler_unbinds_the_previous_sampler_during_rendering() {
    render_sampler_sequence(true);
    GL_STATE.with_borrow(|state| {
        assert_eq!(state.sampler_bindings, [40, 41, 0, 40, 0, 40]);
        assert_eq!(
            state
                .draws
                .iter()
                .map(|draw| draw.sampler)
                .collect::<Vec<_>>(),
            [41, 0, 40, 0, 40]
        );
        for draw in &state.draws {
            assert_eq!(draw.min_filter, ORIGINAL_MIN_FILTER);
            assert_eq!(draw.mag_filter, ORIGINAL_MAG_FILTER);
            assert_eq!(draw.parameter_writes, 0);
        }
    });
}

#[test]
fn texture_owned_sampler_preserves_mipmapped_texture_parameters_during_fallback_rendering() {
    render_sampler_sequence(false);
    GL_STATE.with_borrow(|state| {
        let nearest = glow::NEAREST as i32;
        let linear = glow::LINEAR as i32;
        let expected = [
            (nearest, nearest, 2),
            (ORIGINAL_MIN_FILTER, ORIGINAL_MAG_FILTER, 4),
            (linear, linear, 6),
            (ORIGINAL_MIN_FILTER, ORIGINAL_MAG_FILTER, 8),
            (ORIGINAL_MIN_FILTER, ORIGINAL_MAG_FILTER, 8),
        ];
        assert_eq!(state.draws.len(), expected.len());
        for (draw, (min_filter, mag_filter, parameter_writes)) in state.draws.iter().zip(expected) {
            assert_eq!(draw.sampler, 0);
            assert_eq!(draw.min_filter, min_filter);
            assert_eq!(draw.mag_filter, mag_filter);
            assert_eq!(draw.parameter_writes, parameter_writes);
        }
        assert_eq!(state.min_filter, ORIGINAL_MIN_FILTER);
        assert_eq!(state.mag_filter, ORIGINAL_MAG_FILTER);
        assert_eq!(state.parameter_writes, 8);
        assert!(state.sampler_bindings.is_empty());
    });
}
