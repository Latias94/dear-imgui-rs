use crate::sys;
use crate::ui::Ui;

/// Texture ID for images
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ImageTextureId(pub usize);

impl ImageTextureId {
    /// Creates a new texture ID
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    /// Returns the raw ID
    pub fn id(self) -> usize {
        self.0
    }
}

impl From<usize> for ImageTextureId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<*mut std::os::raw::c_void> for ImageTextureId {
    fn from(ptr: *mut std::os::raw::c_void) -> Self {
        Self(ptr as usize)
    }
}

/// # Image Widgets
impl Ui {
    /// Creates an image widget
    #[doc(alias = "Image")]
    pub fn image(&self, texture_id: ImageTextureId, size: [f32; 2]) {
        self.image_config(texture_id, size).build()
    }

    /// Creates an image button widget
    #[doc(alias = "ImageButton")]
    pub fn image_button(
        &self,
        str_id: impl AsRef<str>,
        texture_id: ImageTextureId,
        size: [f32; 2],
    ) -> bool {
        self.image_button_config(str_id, texture_id, size).build()
    }

    /// Creates an image builder
    pub fn image_config(&self, texture_id: ImageTextureId, size: [f32; 2]) -> Image<'_> {
        Image::new(self, texture_id, size)
    }

    /// Creates an image button builder
    pub fn image_button_config(
        &self,
        str_id: impl AsRef<str>,
        texture_id: ImageTextureId,
        size: [f32; 2],
    ) -> ImageButton<'_> {
        ImageButton::new(self, str_id, texture_id, size)
    }
}

/// Builder for an image widget
#[derive(Debug)]
#[must_use]
pub struct Image<'ui> {
    ui: &'ui Ui,
    texture_id: ImageTextureId,
    size: [f32; 2],
    uv0: [f32; 2],
    uv1: [f32; 2],
    tint_color: [f32; 4],
    border_color: [f32; 4],
}

impl<'ui> Image<'ui> {
    /// Creates a new image builder
    pub fn new(ui: &'ui Ui, texture_id: ImageTextureId, size: [f32; 2]) -> Self {
        Self {
            ui,
            texture_id,
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
    pub fn tint_color(mut self, tint_color: [f32; 4]) -> Self {
        self.tint_color = tint_color;
        self
    }

    /// Sets the border color (default: transparent, no border)
    pub fn border_color(mut self, border_color: [f32; 4]) -> Self {
        self.border_color = border_color;
        self
    }

    /// Builds the image widget
    pub fn build(self) {
        let size_vec: sys::ImVec2 = self.size.into();
        let uv0_vec: sys::ImVec2 = self.uv0.into();
        let uv1_vec: sys::ImVec2 = self.uv1.into();
        let tint_vec: sys::ImVec4 = sys::ImVec4 {
            x: self.tint_color[0],
            y: self.tint_color[1],
            z: self.tint_color[2],
            w: self.tint_color[3],
        };
        let border_vec: sys::ImVec4 = sys::ImVec4 {
            x: self.border_color[0],
            y: self.border_color[1],
            z: self.border_color[2],
            w: self.border_color[3],
        };

        unsafe {
            sys::ImGui_Image(
                sys::ImTextureRef {
                    _TexData: std::ptr::null_mut(),
                    _TexID: self.texture_id.id() as sys::ImTextureID,
                },
                &size_vec,
                &uv0_vec,
                &uv1_vec,
            );
        }
    }
}

/// Builder for an image button widget
#[derive(Debug)]
#[must_use]
pub struct ImageButton<'ui> {
    ui: &'ui Ui,
    str_id: String,
    texture_id: ImageTextureId,
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
        str_id: impl AsRef<str>,
        texture_id: ImageTextureId,
        size: [f32; 2],
    ) -> Self {
        Self {
            ui,
            str_id: str_id.as_ref().to_string(),
            texture_id,
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
        let str_id_ptr = self.ui.scratch_txt(&self.str_id);
        let size_vec: sys::ImVec2 = self.size.into();
        let uv0_vec: sys::ImVec2 = self.uv0.into();
        let uv1_vec: sys::ImVec2 = self.uv1.into();
        let bg_vec: sys::ImVec4 = sys::ImVec4 {
            x: self.bg_color[0],
            y: self.bg_color[1],
            z: self.bg_color[2],
            w: self.bg_color[3],
        };
        let tint_vec: sys::ImVec4 = sys::ImVec4 {
            x: self.tint_color[0],
            y: self.tint_color[1],
            z: self.tint_color[2],
            w: self.tint_color[3],
        };

        unsafe {
            sys::ImGui_ImageButton(
                str_id_ptr,
                sys::ImTextureRef {
                    _TexData: std::ptr::null_mut(),
                    _TexID: self.texture_id.id() as sys::ImTextureID,
                },
                &size_vec,
                &uv0_vec,
                &uv1_vec,
                &bg_vec,
                &tint_vec,
            )
        }
    }
}
