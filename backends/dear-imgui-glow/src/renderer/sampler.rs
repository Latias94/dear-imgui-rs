use glow::{Context, HasContext};

use crate::{GlSampler, error::InitError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SamplerFilter {
    Linear,
    Nearest,
}

impl SamplerFilter {
    fn gl_filter(self) -> i32 {
        match self {
            Self::Linear => glow::LINEAR as i32,
            Self::Nearest => glow::NEAREST as i32,
        }
    }
}

#[derive(Debug)]
pub(super) struct SamplerObjects {
    linear: GlSampler,
    nearest: GlSampler,
}

impl SamplerObjects {
    pub(super) fn create(gl: &Context) -> Result<Self, InitError> {
        let linear = unsafe { gl.create_sampler() }.map_err(InitError::CreateSampler)?;
        let mut pending = PendingSampler {
            gl,
            sampler: Some(linear),
        };
        configure_sampler(gl, linear, SamplerFilter::Linear);

        let nearest = unsafe { gl.create_sampler() }.map_err(InitError::CreateSampler)?;
        let mut pending_nearest = PendingSampler {
            gl,
            sampler: Some(nearest),
        };
        configure_sampler(gl, nearest, SamplerFilter::Nearest);
        let linear = pending
            .sampler
            .take()
            .expect("pending linear sampler must remain owned");
        let nearest = pending_nearest
            .sampler
            .take()
            .expect("pending nearest sampler must remain owned");
        Ok(Self { linear, nearest })
    }

    pub(super) fn bind(&self, gl: &Context, filter: SamplerFilter) {
        let sampler = match filter {
            SamplerFilter::Linear => self.linear,
            SamplerFilter::Nearest => self.nearest,
        };
        unsafe { gl.bind_sampler(0, Some(sampler)) };
    }

    pub(super) fn destroy(self, gl: &Context) {
        unsafe {
            gl.delete_sampler(self.nearest);
            gl.delete_sampler(self.linear);
        }
    }
}

fn configure_sampler(gl: &Context, sampler: GlSampler, filter: SamplerFilter) {
    unsafe {
        gl.sampler_parameter_i32(sampler, glow::TEXTURE_MIN_FILTER, filter.gl_filter());
        gl.sampler_parameter_i32(sampler, glow::TEXTURE_MAG_FILTER, filter.gl_filter());
        gl.sampler_parameter_i32(sampler, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.sampler_parameter_i32(sampler, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    }
}

struct PendingSampler<'gl> {
    gl: &'gl Context,
    sampler: Option<GlSampler>,
}

impl Drop for PendingSampler<'_> {
    fn drop(&mut self) {
        if let Some(sampler) = self.sampler.take() {
            unsafe { self.gl.delete_sampler(sampler) };
        }
    }
}

pub(super) struct TextureFilterGuard<'gl> {
    gl: &'gl Context,
    min_filter: i32,
    mag_filter: i32,
}

impl<'gl> TextureFilterGuard<'gl> {
    pub(super) fn override_bound_texture(gl: &'gl Context, filter: SamplerFilter) -> Self {
        let min_filter =
            unsafe { gl.get_tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER) };
        let mag_filter =
            unsafe { gl.get_tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER) };
        let filter = filter.gl_filter();
        unsafe {
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
        }
        Self {
            gl,
            min_filter,
            mag_filter,
        }
    }
}

impl Drop for TextureFilterGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, self.min_filter);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, self.mag_filter);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{SamplerFilter, SamplerObjects, TextureFilterGuard};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct SamplerParameterCall {
        sampler: u32,
        parameter: u32,
        value: i32,
    }

    impl SamplerParameterCall {
        const EMPTY: Self = Self {
            sampler: 0,
            parameter: 0,
            value: 0,
        };
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FakeSamplerGl {
        next_sampler: u32,
        fail_creation: u32,
        created: u32,
        deleted: u32,
        sampler_parameter_calls: [SamplerParameterCall; 8],
        sampler_parameter_call_count: usize,
        min_filter: i32,
        mag_filter: i32,
    }

    impl FakeSamplerGl {
        const fn initial() -> Self {
            Self {
                next_sampler: 40,
                fail_creation: 0,
                created: 0,
                deleted: 0,
                sampler_parameter_calls: [SamplerParameterCall::EMPTY; 8],
                sampler_parameter_call_count: 0,
                min_filter: glow::LINEAR_MIPMAP_LINEAR as i32,
                mag_filter: glow::LINEAR as i32,
            }
        }
    }

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static FAKE_GL: Mutex<FakeSamplerGl> = Mutex::new(FakeSamplerGl::initial());

    unsafe extern "system" fn get_string(name: u32) -> *const u8 {
        if name == glow::VERSION {
            c"4.6".as_ptr().cast()
        } else {
            c"".as_ptr().cast()
        }
    }

    unsafe extern "system" fn get_string_i(_name: u32, _index: u32) -> *const u8 {
        c"".as_ptr().cast()
    }

    unsafe extern "system" fn get_integer(_name: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe { *value = 0 };
        }
    }

    unsafe extern "system" fn gen_samplers(count: i32, samplers: *mut u32) {
        let mut state = FAKE_GL.lock().unwrap();
        for index in 0..count.max(0) as usize {
            state.created += 1;
            let sampler = if state.created == state.fail_creation {
                0
            } else {
                let sampler = state.next_sampler;
                state.next_sampler += 1;
                sampler
            };
            unsafe { *samplers.add(index) = sampler };
        }
    }

    unsafe extern "system" fn delete_samplers(count: i32, _samplers: *const u32) {
        FAKE_GL.lock().unwrap().deleted += count.max(0) as u32;
    }

    unsafe extern "system" fn sampler_parameter_i(sampler: u32, parameter: u32, value: i32) {
        let mut state = FAKE_GL.lock().unwrap();
        let index = state.sampler_parameter_call_count;
        state.sampler_parameter_calls[index] = SamplerParameterCall {
            sampler,
            parameter,
            value,
        };
        state.sampler_parameter_call_count += 1;
    }

    unsafe extern "system" fn get_tex_parameter_i(_target: u32, parameter: u32, value: *mut i32) {
        let state = FAKE_GL.lock().unwrap();
        let current = match parameter {
            glow::TEXTURE_MIN_FILTER => state.min_filter,
            glow::TEXTURE_MAG_FILTER => state.mag_filter,
            _ => 0,
        };
        unsafe { *value = current };
    }

    unsafe extern "system" fn tex_parameter_i(_target: u32, parameter: u32, value: i32) {
        let mut state = FAKE_GL.lock().unwrap();
        match parameter {
            glow::TEXTURE_MIN_FILTER => state.min_filter = value,
            glow::TEXTURE_MAG_FILTER => state.mag_filter = value,
            _ => {}
        }
    }

    fn fake_gl() -> glow::Context {
        unsafe {
            glow::Context::from_loader_function(|name| {
                match name {
                    "glGetString" => get_string as *const (),
                    "glGetStringi" => get_string_i as *const (),
                    "glGetIntegerv" => get_integer as *const (),
                    "glGenSamplers" => gen_samplers as *const (),
                    "glDeleteSamplers" => delete_samplers as *const (),
                    "glSamplerParameteri" => sampler_parameter_i as *const (),
                    "glGetTexParameteriv" => get_tex_parameter_i as *const (),
                    "glTexParameteri" => tex_parameter_i as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }

    fn reset(fail_creation: u32) {
        let mut state = FakeSamplerGl::initial();
        state.fail_creation = fail_creation;
        *FAKE_GL.lock().unwrap() = state;
    }

    #[test]
    fn sampler_creation_rolls_back_the_first_object_when_the_second_fails() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(2);
        let result = SamplerObjects::create(&fake_gl());
        assert!(result.is_err());
        let state = *FAKE_GL.lock().unwrap();
        assert_eq!(state.created, 2);
        assert_eq!(state.deleted, 1);
    }

    #[test]
    fn sampler_objects_destroy_each_owned_object_once() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(0);
        let gl = fake_gl();
        SamplerObjects::create(&gl).unwrap().destroy(&gl);
        let state = *FAKE_GL.lock().unwrap();
        assert_eq!(state.created, 2);
        assert_eq!(state.deleted, 2);
        assert_eq!(state.sampler_parameter_call_count, 8);
        assert_eq!(
            state.sampler_parameter_calls,
            [
                SamplerParameterCall {
                    sampler: 40,
                    parameter: glow::TEXTURE_MIN_FILTER,
                    value: glow::LINEAR as i32,
                },
                SamplerParameterCall {
                    sampler: 40,
                    parameter: glow::TEXTURE_MAG_FILTER,
                    value: glow::LINEAR as i32,
                },
                SamplerParameterCall {
                    sampler: 40,
                    parameter: glow::TEXTURE_WRAP_S,
                    value: glow::CLAMP_TO_EDGE as i32,
                },
                SamplerParameterCall {
                    sampler: 40,
                    parameter: glow::TEXTURE_WRAP_T,
                    value: glow::CLAMP_TO_EDGE as i32,
                },
                SamplerParameterCall {
                    sampler: 41,
                    parameter: glow::TEXTURE_MIN_FILTER,
                    value: glow::NEAREST as i32,
                },
                SamplerParameterCall {
                    sampler: 41,
                    parameter: glow::TEXTURE_MAG_FILTER,
                    value: glow::NEAREST as i32,
                },
                SamplerParameterCall {
                    sampler: 41,
                    parameter: glow::TEXTURE_WRAP_S,
                    value: glow::CLAMP_TO_EDGE as i32,
                },
                SamplerParameterCall {
                    sampler: 41,
                    parameter: glow::TEXTURE_WRAP_T,
                    value: glow::CLAMP_TO_EDGE as i32,
                },
            ]
        );
    }

    #[test]
    fn fallback_filter_override_restores_exact_mipmapped_parameters() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(0);
        let gl = fake_gl();
        {
            let _override = TextureFilterGuard::override_bound_texture(&gl, SamplerFilter::Nearest);
            let state = *FAKE_GL.lock().unwrap();
            assert_eq!(state.min_filter, glow::NEAREST as i32);
            assert_eq!(state.mag_filter, glow::NEAREST as i32);
        }
        let state = *FAKE_GL.lock().unwrap();
        assert_eq!(state.min_filter, glow::LINEAR_MIPMAP_LINEAR as i32);
        assert_eq!(state.mag_filter, glow::LINEAR as i32);
    }

    #[test]
    fn fallback_filter_override_restores_during_unwind() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset(0);
        let gl = fake_gl();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _override = TextureFilterGuard::override_bound_texture(&gl, SamplerFilter::Nearest);
            panic!("injected draw failure");
        }));
        assert!(panic.is_err());
        let state = *FAKE_GL.lock().unwrap();
        assert_eq!(state.min_filter, glow::LINEAR_MIPMAP_LINEAR as i32);
        assert_eq!(state.mag_filter, glow::LINEAR as i32);
    }
}
