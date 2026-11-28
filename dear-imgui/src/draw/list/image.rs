use crate::draw::{
    DrawFlags, ImColor32, assert_corner_flags, assert_non_negative_f32, finite_vec2,
};
use crate::sys;

use super::DrawListMut;

/// Tracks a texture pushed to a draw-list texture stack.
///
/// The texture is popped when the token is dropped or when [`Self::pop`] is
/// called explicitly. Tokens for the same draw list must finish in reverse creation order. A token
/// from a window draw list must also finish in the exact window `Begin` scope that created the
/// draw-list view. Prefer [`DrawListMut::with_texture`] for ordinary scoped use.
#[must_use]
pub struct DrawListTextureToken<'draw_list, 'tex> {
    scope: crate::scope::NativeScopeToken<'draw_list>,
    _texture: std::marker::PhantomData<&'tex ()>,
}

impl<'draw_list, 'tex> DrawListTextureToken<'draw_list, 'tex> {
    fn new(
        ui: &'draw_list crate::Ui,
        draw_list: *mut sys::ImDrawList,
        window_scoped: bool,
    ) -> Self {
        Self {
            scope: ui.begin_native_scope(
                crate::scope::NativeScopePop::DrawListPopTexture {
                    draw_list,
                    window_scoped,
                },
                "DrawListTextureToken",
            ),
            _texture: std::marker::PhantomData,
        }
    }

    /// Pop the texture immediately instead of waiting for drop.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later texture token for the same draw list is active or a
    /// window-scoped draw list is no longer in its originating window `Begin` scope.
    #[doc(alias = "PopTexture")]
    pub fn pop(self) {}

    /// Pop the texture immediately instead of waiting for drop.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::pop`].
    #[doc(alias = "PopTexture")]
    pub fn end(self) {}
}

impl Drop for DrawListTextureToken<'_, '_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

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

impl<'ui> DrawListMut<'ui> {
    /// Add an image quad (axis-aligned). Tint via `col`.
    #[doc(alias = "AddImage")]
    pub fn add_image<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
        let p_min = finite_vec2("DrawListMut::add_image()", "p_min", p_min);
        let p_max = finite_vec2("DrawListMut::add_image()", "p_max", p_max);
        let uv_min = finite_vec2("DrawListMut::add_image()", "uv_min", uv_min);
        let uv_max = finite_vec2("DrawListMut::add_image()", "uv_max", uv_max);
        let col = col.into().to_bits();
        let texture = texture.into();
        self.with_raw_bound("DrawListMut::add_image()", |draw_list| {
            let tex_ref = self
                .ui()
                .resolve_texture_ref(texture)
                .unwrap_or_else(|error| {
                    panic!("DrawListMut::add_image() rejected texture: {error}")
                });
            unsafe {
                sys::ImDrawList_AddImage(draw_list, tex_ref, p_min, p_max, uv_min, uv_max, col)
            }
        })
    }

    /// Add an image with 4 arbitrary corners.
    #[doc(alias = "AddImageQuad")]
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors ImDrawList::AddImageQuad"
    )]
    pub fn add_image_quad<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        uv1: impl Into<sys::ImVec2>,
        uv2: impl Into<sys::ImVec2>,
        uv3: impl Into<sys::ImVec2>,
        uv4: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_quad(
        //     tex,
        //     [10.0,10.0], [110.0,20.0], [120.0,120.0], [5.0,100.0],
        //     [0.0,0.0], [1.0,0.0], [1.0,1.0], [0.0,1.0],
        //     Color::WHITE,
        // );
        let p1 = finite_vec2("DrawListMut::add_image_quad()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_image_quad()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_image_quad()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::add_image_quad()", "p4", p4);
        let uv1 = finite_vec2("DrawListMut::add_image_quad()", "uv1", uv1);
        let uv2 = finite_vec2("DrawListMut::add_image_quad()", "uv2", uv2);
        let uv3 = finite_vec2("DrawListMut::add_image_quad()", "uv3", uv3);
        let uv4 = finite_vec2("DrawListMut::add_image_quad()", "uv4", uv4);
        let col = col.into().to_bits();
        let texture = texture.into();
        self.with_raw_bound("DrawListMut::add_image_quad()", |draw_list| {
            let tex_ref = self
                .ui()
                .resolve_texture_ref(texture)
                .unwrap_or_else(|error| {
                    panic!("DrawListMut::add_image_quad() rejected texture: {error}")
                });
            unsafe {
                sys::ImDrawList_AddImageQuad(
                    draw_list, tex_ref, p1, p2, p3, p4, uv1, uv2, uv3, uv4, col,
                )
            }
        })
    }

    /// Add an axis-aligned rounded image.
    #[doc(alias = "AddImageRounded")]
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors ImDrawList::AddImageRounded"
    )]
    pub fn add_image_rounded<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        rounding: f32,
        flags: DrawFlags,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_rounded(
        //     tex,
        //     [10.0,10.0], [110.0,110.0],
        //     [0.0,0.0], [1.0,1.0],
        //     Color::WHITE,
        //     8.0,
        //     DrawFlags::ROUND_ALL,
        // );
        let p_min = finite_vec2("DrawListMut::add_image_rounded()", "p_min", p_min);
        let p_max = finite_vec2("DrawListMut::add_image_rounded()", "p_max", p_max);
        let uv_min = finite_vec2("DrawListMut::add_image_rounded()", "uv_min", uv_min);
        let uv_max = finite_vec2("DrawListMut::add_image_rounded()", "uv_max", uv_max);
        assert_non_negative_f32("DrawListMut::add_image_rounded()", "rounding", rounding);
        assert_corner_flags("DrawListMut::add_image_rounded()", flags);
        let col = col.into().to_bits();
        let texture = texture.into();
        self.with_raw_bound("DrawListMut::add_image_rounded()", |draw_list| {
            let tex_ref = self
                .ui()
                .resolve_texture_ref(texture)
                .unwrap_or_else(|error| {
                    panic!("DrawListMut::add_image_rounded() rejected texture: {error}")
                });
            unsafe {
                sys::ImDrawList_AddImageRounded(
                    draw_list,
                    tex_ref,
                    p_min,
                    p_max,
                    uv_min,
                    uv_max,
                    col,
                    rounding,
                    flags.bits() as sys::ImDrawFlags,
                )
            }
        })
    }

    /// Push a texture on the drawlist texture stack (ImGui 1.92+).
    ///
    /// While pushed, image and primitives will use this texture unless otherwise specified.
    /// The returned token pops the texture when dropped.
    ///
    /// Example:
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # fn demo(ui: &Ui) {
    /// ui.with_window_draw_list(|dl| {
    ///     let tex = texture::TextureId::new(1);
    ///     let _texture = dl.push_texture(tex);
    ///     dl.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
    /// });
    /// # }
    /// ```
    #[doc(alias = "PushTexture")]
    pub fn push_texture<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
    ) -> DrawListTextureToken<'_, 'tex> {
        let texture = texture.into();
        self.with_raw_bound("DrawListMut::push_texture()", |draw_list| {
            let tex_ref = self
                .ui()
                .resolve_texture_ref(texture)
                .unwrap_or_else(|error| {
                    panic!("DrawListMut::push_texture() rejected texture: {error}")
                });
            unsafe { sys::ImDrawList_PushTexture(draw_list, tex_ref) };
            DrawListTextureToken::new(self.ui(), draw_list, self.is_window_scoped())
        })
    }

    /// Push a texture, run `f`, then pop the texture.
    ///
    /// The texture is popped during unwinding if `f` panics.
    #[doc(alias = "PushTexture", alias = "PopTexture")]
    pub fn with_texture<'tex, R>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        f: impl FnOnce() -> R,
    ) -> R {
        let texture = self.push_texture(texture);
        let result = f();
        drop(texture);
        result
    }

    fn set_standard_sampler(&self, sampler: StandardSampler) {
        self.with_raw_bound("DrawListMut::set_standard_sampler()", |draw_list| {
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
                    draw_list,
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
