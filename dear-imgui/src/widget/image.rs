//! Image widgets
//!
//! Draw images from a legacy `TextureId` or from modern `TextureData` handled
//! via `DrawData::textures()`. See crate-level docs for texture management.
//!
//! Quick example (image button):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let tex_id = texture::TextureId::new(42);
//! if ui.image_button("btn", tex_id, [32.0, 32.0]) {
//!     // clicked
//! }
//! ```
//!
use crate::sys;
use crate::texture::TextureRef;
use crate::ui::Ui;
use crate::{StyleColor, StyleVar};
use std::borrow::Cow;

fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

fn assert_finite_vec4(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

fn is_default_tint_color(color: [f32; 4]) -> bool {
    color == [1.0, 1.0, 1.0, 1.0]
}

fn is_transparent_color(color: [f32; 4]) -> bool {
    color == [0.0, 0.0, 0.0, 0.0]
}

fn im_vec4(value: [f32; 4]) -> sys::ImVec4 {
    sys::ImVec4 {
        x: value[0],
        y: value[1],
        z: value[2],
        w: value[3],
    }
}

/// # Image Widgets
///
/// Examples
/// - Using a plain texture id:
/// ```no_run
/// # use dear_imgui_rs::*;
/// # fn demo(ui: &Ui) {
/// let tex_id = texture::TextureId::new(0xDEAD_BEEF);
/// ui.image(tex_id, [128.0, 128.0]);
/// # }
/// ```
/// - Using an ImGui-managed texture:
/// ```no_run
/// # use dear_imgui_rs::*;
/// # fn demo(ui: &Ui) {
/// let mut tex = texture::TextureData::new();
/// tex.create(texture::TextureFormat::RGBA32, 64, 64);
/// ui.image(&mut *tex, [64.0, 64.0]);
/// # }
/// ```
impl Ui {
    /// Creates an image widget
    #[doc(alias = "Image")]
    pub fn image(&self, texture: impl Into<TextureRef>, size: [f32; 2]) {
        self.image_config(texture, size).build()
    }

    /// Creates an image button widget
    #[doc(alias = "ImageButton")]
    pub fn image_button(
        &self,
        str_id: impl AsRef<str>,
        texture: impl Into<TextureRef>,
        size: [f32; 2],
    ) -> bool {
        self.image_button_config(str_id.as_ref(), texture, size)
            .build()
    }

    /// Creates an image builder
    pub fn image_config(&self, texture: impl Into<TextureRef>, size: [f32; 2]) -> Image<'_> {
        Image::new(self, texture, size)
    }

    /// Creates an image button builder
    pub fn image_button_config<'ui>(
        &'ui self,
        str_id: impl Into<Cow<'ui, str>>,
        texture: impl Into<TextureRef>,
        size: [f32; 2],
    ) -> ImageButton<'ui> {
        ImageButton::new(self, str_id, texture, size)
    }
}

/// Builder for an image widget
#[derive(Debug)]
#[must_use]
pub struct Image<'ui> {
    _ui: &'ui Ui,
    texture: TextureRef,
    size: [f32; 2],
    uv0: [f32; 2],
    uv1: [f32; 2],
    tint_color: [f32; 4],
    border_color: [f32; 4],
}

impl<'ui> Image<'ui> {
    /// Creates a new image builder
    pub fn new(ui: &'ui Ui, texture: impl Into<TextureRef>, size: [f32; 2]) -> Self {
        Self {
            _ui: ui,
            texture: texture.into(),
            size,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint_color: [1.0, 1.0, 1.0, 1.0],
            border_color: [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Sets the UV coordinates for the top-left corner (default: [0.0, 0.0])
    pub fn uv0(mut self, uv0: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self
    }

    /// Sets the UV coordinates for the bottom-right corner (default: [1.0, 1.0])
    pub fn uv1(mut self, uv1: [f32; 2]) -> Self {
        self.uv1 = uv1;
        self
    }

    /// Sets the tint color (default: white, no tint)
    ///
    /// Dear ImGui 1.91.9 moved image tinting from `Image()` to `ImageWithBg()`.
    /// If this is set, [`build`](Self::build) will call the tinted path while
    /// keeping a transparent background.
    pub fn tint_color(mut self, tint_color: [f32; 4]) -> Self {
        self.tint_color = tint_color;
        self
    }

    /// Sets the border color (default: transparent, no border)
    ///
    /// Dear ImGui 1.91.9 moved image border thickness to `Style::ImageBorderSize`
    /// and border color to `StyleColor::Border`; this builder applies matching
    /// temporary style overrides around [`build`](Self::build).
    pub fn border_color(mut self, border_color: [f32; 4]) -> Self {
        self.border_color = border_color;
        self
    }

    /// Builds the image widget
    pub fn build(self) {
        assert_non_negative_finite_vec2("Image::build()", "size", self.size);
        assert_finite_vec2("Image::build()", "uv0", self.uv0);
        assert_finite_vec2("Image::build()", "uv1", self.uv1);
        assert_finite_vec4("Image::build()", "tint_color", self.tint_color);
        assert_finite_vec4("Image::build()", "border_color", self.border_color);

        let size_vec: sys::ImVec2 = self.size.into();
        let uv0_vec: sys::ImVec2 = self.uv0.into();
        let uv1_vec: sys::ImVec2 = self.uv1.into();

        let _border_size_token = (self.border_color[3] > 0.0).then(|| {
            let current_size = unsafe { self._ui.style().image_border_size() };
            self._ui
                .push_style_var(StyleVar::ImageBorderSize(current_size.max(1.0)))
        });
        let _border_color_token = (self.border_color[3] > 0.0).then(|| {
            self._ui
                .push_style_color(StyleColor::Border, self.border_color)
        });

        if is_default_tint_color(self.tint_color) && is_transparent_color(self.border_color) {
            unsafe { sys::igImage(self.texture.raw(), size_vec, uv0_vec, uv1_vec) }
        } else {
            unsafe {
                sys::igImageWithBg(
                    self.texture.raw(),
                    size_vec,
                    uv0_vec,
                    uv1_vec,
                    im_vec4([0.0, 0.0, 0.0, 0.0]),
                    im_vec4(self.tint_color),
                )
            }
        }
    }

    /// Builds the image widget with background color and tint (v1.92+)
    pub fn build_with_bg(self, bg_color: [f32; 4], tint_color: [f32; 4]) {
        assert_non_negative_finite_vec2("Image::build_with_bg()", "size", self.size);
        assert_finite_vec2("Image::build_with_bg()", "uv0", self.uv0);
        assert_finite_vec2("Image::build_with_bg()", "uv1", self.uv1);
        assert_finite_vec4("Image::build_with_bg()", "bg_color", bg_color);
        assert_finite_vec4("Image::build_with_bg()", "tint_color", tint_color);
        assert_finite_vec4("Image::build_with_bg()", "border_color", self.border_color);

        let size_vec: sys::ImVec2 = self.size.into();
        let uv0_vec: sys::ImVec2 = self.uv0.into();
        let uv1_vec: sys::ImVec2 = self.uv1.into();

        let _border_size_token = (self.border_color[3] > 0.0).then(|| {
            let current_size = unsafe { self._ui.style().image_border_size() };
            self._ui
                .push_style_var(StyleVar::ImageBorderSize(current_size.max(1.0)))
        });
        let _border_color_token = (self.border_color[3] > 0.0).then(|| {
            self._ui
                .push_style_color(StyleColor::Border, self.border_color)
        });

        unsafe {
            sys::igImageWithBg(
                self.texture.raw(),
                size_vec,
                uv0_vec,
                uv1_vec,
                im_vec4(bg_color),
                im_vec4(tint_color),
            )
        }
    }
}

/// Builder for an image button widget
#[derive(Debug)]
#[must_use]
pub struct ImageButton<'ui> {
    ui: &'ui Ui,
    str_id: Cow<'ui, str>,
    texture: TextureRef,
    size: [f32; 2],
    uv0: [f32; 2],
    uv1: [f32; 2],
    bg_color: [f32; 4],
    tint_color: [f32; 4],
}

impl<'ui> ImageButton<'ui> {
    /// Creates a new image button builder
    pub fn new(
        ui: &'ui Ui,
        str_id: impl Into<Cow<'ui, str>>,
        texture: impl Into<TextureRef>,
        size: [f32; 2],
    ) -> Self {
        Self {
            ui,
            str_id: str_id.into(),
            texture: texture.into(),
            size,
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            bg_color: [0.0, 0.0, 0.0, 0.0],
            tint_color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Sets the UV coordinates for the top-left corner (default: [0.0, 0.0])
    pub fn uv0(mut self, uv0: [f32; 2]) -> Self {
        self.uv0 = uv0;
        self
    }

    /// Sets the UV coordinates for the bottom-right corner (default: [1.0, 1.0])
    pub fn uv1(mut self, uv1: [f32; 2]) -> Self {
        self.uv1 = uv1;
        self
    }

    /// Sets the background color (default: transparent)
    pub fn bg_color(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = bg_color;
        self
    }

    /// Sets the tint color (default: white, no tint)
    pub fn tint_color(mut self, tint_color: [f32; 4]) -> Self {
        self.tint_color = tint_color;
        self
    }

    /// Builds the image button widget
    pub fn build(self) -> bool {
        assert_non_negative_finite_vec2("ImageButton::build()", "size", self.size);
        assert_finite_vec2("ImageButton::build()", "uv0", self.uv0);
        assert_finite_vec2("ImageButton::build()", "uv1", self.uv1);
        assert_finite_vec4("ImageButton::build()", "bg_color", self.bg_color);
        assert_finite_vec4("ImageButton::build()", "tint_color", self.tint_color);

        let str_id_ptr = self.ui.scratch_txt(self.str_id.as_ref());
        let size_vec: sys::ImVec2 = self.size.into();
        let uv0_vec: sys::ImVec2 = self.uv0.into();
        let uv1_vec: sys::ImVec2 = self.uv1.into();

        unsafe {
            sys::igImageButton(
                str_id_ptr,
                self.texture.raw(),
                size_vec,
                uv0_vec,
                uv1_vec,
                im_vec4(self.bg_color),
                im_vec4(self.tint_color),
            )
        }
    }
}
