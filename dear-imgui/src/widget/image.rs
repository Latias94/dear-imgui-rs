//! Image widgets
//!
//! Draw images from a legacy `TextureId` or a Context-owned `ManagedTextureId`.
//! Managed pixels reach renderer backends as owned `TextureRequest` values on a
//! `PendingFrame` or `FrameSnapshot`; safe renderers never borrow `TextureData`.
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
use crate::validation::{assert_finite_vec2, assert_finite_vec4};
use crate::{Color, StyleColor, StyleVar};
use std::borrow::Cow;

fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

fn is_default_tint_color(color: [f32; 4]) -> bool {
    color == [1.0, 1.0, 1.0, 1.0]
}

fn is_transparent_color(color: [f32; 4]) -> bool {
    color == [0.0, 0.0, 0.0, 0.0]
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
/// # fn demo(context: &mut Context) -> Result<(), texture::TextureDataError> {
/// let tex = texture::OwnedTextureData::from_pixels(
///     texture::TextureFormat::RGBA32,
///     64,
///     64,
///     &vec![255; 64 * 64 * 4],
/// )?;
/// let tex = context.register_texture(tex);
/// let ui = context.frame();
/// ui.image(tex, [64.0, 64.0]);
/// # Ok(())
/// # }
/// ```
impl Ui {
    /// Creates an image widget
    #[doc(alias = "Image")]
    pub fn image<'tex>(&self, texture: impl Into<TextureRef<'tex>>, size: impl Into<[f32; 2]>) {
        self.image_config(texture, size).build()
    }

    /// Draws an image with a background and tint.
    #[doc(alias = "ImageWithBg")]
    pub fn image_with_bg<'tex>(
        &self,
        texture: impl Into<TextureRef<'tex>>,
        size: impl Into<[f32; 2]>,
        bg_color: impl Into<Color>,
        tint_color: impl Into<Color>,
    ) {
        self.image_config(texture, size)
            .build_with_bg(bg_color, tint_color)
    }

    /// Creates an image button widget
    #[doc(alias = "ImageButton")]
    pub fn image_button<'tex>(
        &self,
        str_id: impl AsRef<str>,
        texture: impl Into<TextureRef<'tex>>,
        size: impl Into<[f32; 2]>,
    ) -> bool {
        self.image_button_config(str_id.as_ref(), texture, size)
            .build()
    }

    /// Creates an image builder
    pub fn image_config<'ui, 'tex>(
        &'ui self,
        texture: impl Into<TextureRef<'tex>>,
        size: impl Into<[f32; 2]>,
    ) -> Image<'ui, 'tex> {
        Image::new(self, texture, size)
    }

    /// Creates an image button builder
    pub fn image_button_config<'ui, 'tex>(
        &'ui self,
        str_id: impl Into<Cow<'ui, str>>,
        texture: impl Into<TextureRef<'tex>>,
        size: impl Into<[f32; 2]>,
    ) -> ImageButton<'ui, 'tex> {
        ImageButton::new(self, str_id, texture, size)
    }
}

/// Builder for an image widget
#[derive(Debug)]
#[must_use]
pub struct Image<'ui, 'tex> {
    ui: &'ui Ui,
    texture: TextureRef<'tex>,
    size: [f32; 2],
    uv0: [f32; 2],
    uv1: [f32; 2],
    tint_color: [f32; 4],
    border_color: [f32; 4],
}

impl<'ui, 'tex> Image<'ui, 'tex> {
    /// Creates a new image builder
    pub fn new(
        ui: &'ui Ui,
        texture: impl Into<TextureRef<'tex>>,
        size: impl Into<[f32; 2]>,
    ) -> Self {
        Self {
            ui,
            texture: texture.into(),
            size: size.into(),
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            tint_color: [1.0, 1.0, 1.0, 1.0],
            border_color: [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Sets the UV coordinates for the top-left corner (default: [0.0, 0.0])
    pub fn uv0(mut self, uv0: impl Into<[f32; 2]>) -> Self {
        self.uv0 = uv0.into();
        self
    }

    /// Sets the UV coordinates for the bottom-right corner (default: [1.0, 1.0])
    pub fn uv1(mut self, uv1: impl Into<[f32; 2]>) -> Self {
        self.uv1 = uv1.into();
        self
    }

    /// Sets the tint color (default: white, no tint)
    ///
    /// Dear ImGui 1.91.9 moved image tinting from `Image()` to `ImageWithBg()`.
    /// If this is set, [`build`](Self::build) will call the tinted path while
    /// keeping a transparent background.
    pub fn tint_color(mut self, tint_color: impl Into<Color>) -> Self {
        self.tint_color = tint_color.into().to_array();
        self
    }

    /// Sets the border color (default: transparent, no border)
    ///
    /// Dear ImGui 1.91.9 moved image border thickness to `Style::ImageBorderSize`
    /// and border color to `StyleColor::Border`; this builder applies matching
    /// temporary style overrides around [`build`](Self::build).
    pub fn border_color(mut self, border_color: impl Into<Color>) -> Self {
        self.border_color = border_color.into().to_array();
        self
    }

    /// Builds the image widget
    pub fn build(self) {
        let tint = self.tint_color;
        self.submit("Image::build()", None, tint);
    }

    /// Builds the image widget with background color and tint (v1.92+)
    pub fn build_with_bg(self, bg_color: impl Into<Color>, tint_color: impl Into<Color>) {
        self.submit(
            "Image::build_with_bg()",
            Some(bg_color.into().to_array()),
            tint_color.into().to_array(),
        );
    }

    fn submit(self, caller: &str, background: Option<[f32; 4]>, tint: [f32; 4]) {
        assert_non_negative_finite_vec2(caller, "size", self.size);
        assert_finite_vec2(caller, "uv0", self.uv0);
        assert_finite_vec2(caller, "uv1", self.uv1);
        assert_finite_vec4(caller, "tint_color", tint);
        assert_finite_vec4(caller, "border_color", self.border_color);
        if let Some(background) = background {
            assert_finite_vec4(caller, "bg_color", background);
        }

        let ui = self.ui;
        ui.run_with_bound_context(|| {
            let texture = ui
                .resolve_texture_ref(self.texture)
                .unwrap_or_else(|error| panic!("{caller} rejected texture: {error}"));
            let _border_size = (self.border_color[3] > 0.0).then(|| {
                let current_size = ui.style().image_border_size();
                ui.push_style_var(StyleVar::ImageBorderSize(current_size.max(1.0)))
            });
            let _border_color = (self.border_color[3] > 0.0)
                .then(|| ui.push_style_color(StyleColor::Border, self.border_color));
            let size = self.size.into();
            let uv0 = self.uv0.into();
            let uv1 = self.uv1.into();
            unsafe {
                if background.is_none()
                    && is_default_tint_color(tint)
                    && is_transparent_color(self.border_color)
                {
                    sys::igImage(texture, size, uv0, uv1);
                } else {
                    sys::igImageWithBg(
                        texture,
                        size,
                        uv0,
                        uv1,
                        background.unwrap_or_default().into(),
                        tint.into(),
                    );
                }
            }
        });
    }
}

/// Builder for an image button widget
#[derive(Debug)]
#[must_use]
pub struct ImageButton<'ui, 'tex> {
    ui: &'ui Ui,
    str_id: Cow<'ui, str>,
    texture: TextureRef<'tex>,
    size: [f32; 2],
    uv0: [f32; 2],
    uv1: [f32; 2],
    bg_color: [f32; 4],
    tint_color: [f32; 4],
}

impl<'ui, 'tex> ImageButton<'ui, 'tex> {
    /// Creates a new image button builder
    pub fn new(
        ui: &'ui Ui,
        str_id: impl Into<Cow<'ui, str>>,
        texture: impl Into<TextureRef<'tex>>,
        size: impl Into<[f32; 2]>,
    ) -> Self {
        Self {
            ui,
            str_id: str_id.into(),
            texture: texture.into(),
            size: size.into(),
            uv0: [0.0, 0.0],
            uv1: [1.0, 1.0],
            bg_color: [0.0, 0.0, 0.0, 0.0],
            tint_color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Sets the UV coordinates for the top-left corner (default: [0.0, 0.0])
    pub fn uv0(mut self, uv0: impl Into<[f32; 2]>) -> Self {
        self.uv0 = uv0.into();
        self
    }

    /// Sets the UV coordinates for the bottom-right corner (default: [1.0, 1.0])
    pub fn uv1(mut self, uv1: impl Into<[f32; 2]>) -> Self {
        self.uv1 = uv1.into();
        self
    }

    /// Sets the background color (default: transparent)
    pub fn bg_color(mut self, bg_color: impl Into<Color>) -> Self {
        self.bg_color = bg_color.into().to_array();
        self
    }

    /// Sets the tint color (default: white, no tint)
    pub fn tint_color(mut self, tint_color: impl Into<Color>) -> Self {
        self.tint_color = tint_color.into().to_array();
        self
    }

    /// Builds the image button widget
    pub fn build(self) -> bool {
        assert_non_negative_finite_vec2("ImageButton::build()", "size", self.size);
        assert_finite_vec2("ImageButton::build()", "uv0", self.uv0);
        assert_finite_vec2("ImageButton::build()", "uv1", self.uv1);
        assert_finite_vec4("ImageButton::build()", "bg_color", self.bg_color);
        assert_finite_vec4("ImageButton::build()", "tint_color", self.tint_color);

        let size_vec: sys::ImVec2 = self.size.into();
        let uv0_vec: sys::ImVec2 = self.uv0.into();
        let uv1_vec: sys::ImVec2 = self.uv1.into();

        self.ui
            .with_scratch_txt(self.str_id.as_ref(), |str_id_ptr| {
                let texture = self
                    .ui
                    .resolve_texture_ref(self.texture)
                    .unwrap_or_else(|error| {
                        panic!("ImageButton::build() rejected texture: {error}")
                    });
                unsafe {
                    sys::igImageButton(
                        str_id_ptr,
                        texture,
                        size_vec,
                        uv0_vec,
                        uv1_vec,
                        self.bg_color.into(),
                        self.tint_color.into(),
                    )
                }
            })
    }
}
