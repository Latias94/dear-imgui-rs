use crate::sys;

use super::DrawListMut;

#[derive(Clone, Copy)]
enum StandardSampler {
    Linear,
    Nearest,
    FromTexture,
}

impl StandardSampler {
    const fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Nearest => "nearest",
            Self::FromTexture => "from_texture",
        }
    }
}

impl DrawListMut<'_> {
    fn set_standard_sampler(&self, sampler: StandardSampler) {
        self.ui().run_with_bound_context(|| {
            let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
            assert!(
                !platform_io.is_null(),
                "DrawListMut::set_sampler_{}() requires ImGuiPlatformIO",
                sampler.name()
            );
            let callback = unsafe {
                match sampler {
                    StandardSampler::Linear => (*platform_io).DrawCallback_SetSamplerLinear,
                    StandardSampler::Nearest => (*platform_io).DrawCallback_SetSamplerNearest,
                    StandardSampler::FromTexture => (*platform_io).DrawCallback_SetSamplerFromTex,
                }
            }
            .unwrap_or_else(|| {
                panic!(
                    "DrawListMut::set_sampler_{}() requires renderer support for the standard sampler callback",
                    sampler.name()
                )
            });

            unsafe {
                sys::ImDrawList_AddCallback(
                    self.draw_list,
                    Some(callback),
                    std::ptr::null_mut(),
                    0,
                )
            }
        });
    }

    /// Insert the standard renderer command that selects linear texture sampling.
    ///
    /// The command affects subsequent draw commands until another sampler command, renderer-state
    /// reset, or raw callback changes the binding. The active renderer must advertise the standard
    /// callback through `ImGuiPlatformIO`; otherwise this method panics before modifying the draw
    /// list.
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub fn set_sampler_linear(&self) {
        self.set_standard_sampler(StandardSampler::Linear);
    }

    /// Insert the standard renderer command that selects nearest texture sampling.
    ///
    /// The command affects subsequent draw commands until another sampler command, renderer-state
    /// reset, or raw callback changes the binding. The active renderer must advertise the standard
    /// callback through `ImGuiPlatformIO`; otherwise this method panics before modifying the draw
    /// list.
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub fn set_sampler_nearest(&self) {
        self.set_standard_sampler(StandardSampler::Nearest);
    }

    /// Insert the standard renderer command that restores each texture's sampler.
    ///
    /// Subsequent draws use the sampler associated with their texture until a sampler
    /// override, renderer-state reset, or raw callback changes the binding. Panics before
    /// modifying the draw list if the active renderer does not advertise this callback.
    #[doc(alias = "DrawCallback_SetSamplerFromTex")]
    pub fn set_sampler_from_texture(&self) {
        self.set_standard_sampler(StandardSampler::FromTexture);
    }
}
