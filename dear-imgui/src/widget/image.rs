use crate::types::Vec2;
use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Image widgets
///
/// This module contains all image-related UI components.

/// # Widgets: Image
impl<'frame> Ui<'frame> {
    /// Display an image
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// // texture_id should be a valid texture ID from your renderer
    /// let texture_id = 1u64; // Example texture ID
    /// ui.image(texture_id, Vec2::new(100.0, 100.0));
    /// # });
    /// ```
    pub fn image(&mut self, texture_id: u64, size: Vec2) {
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        let uv0_vec = sys::ImVec2 { x: 0.0, y: 0.0 };
        let uv1_vec = sys::ImVec2 { x: 1.0, y: 1.0 };
        let tex_ref = sys::ImTextureRef {
            _TexData: std::ptr::null_mut(),
            _TexID: texture_id,
        };
        unsafe {
            sys::ImGui_Image(
                tex_ref,
                &size_vec as *const _,
                &uv0_vec as *const _,
                &uv1_vec as *const _,
            );
        }
    }

    /// Display an image with custom UV coordinates and colors
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let texture_id = 1u64;
    /// ui.image_ex(
    ///     texture_id,
    ///     Vec2::new(100.0, 100.0),
    ///     Vec2::new(0.0, 0.0), // uv0
    ///     Vec2::new(1.0, 1.0), // uv1
    ///     Color::WHITE,        // tint
    ///     Color::RED,          // border
    /// );
    /// # });
    /// ```
    pub fn image_ex(
        &mut self,
        texture_id: u64,
        size: Vec2,
        uv0: Vec2,
        uv1: Vec2,
        tint_col: crate::types::Color,
        border_col: crate::types::Color,
    ) {
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        let uv0_vec = sys::ImVec2 { x: uv0.x, y: uv0.y };
        let uv1_vec = sys::ImVec2 { x: uv1.x, y: uv1.y };
        let tint_vec = sys::ImVec4 {
            x: tint_col.r(),
            y: tint_col.g(),
            z: tint_col.b(),
            w: tint_col.a(),
        };
        let border_vec = sys::ImVec4 {
            x: border_col.r(),
            y: border_col.g(),
            z: border_col.b(),
            w: border_col.a(),
        };
        let tex_ref = sys::ImTextureRef {
            _TexData: std::ptr::null_mut(),
            _TexID: texture_id,
        };
        unsafe {
            sys::ImGui_Image1(
                tex_ref,
                &size_vec as *const _,
                &uv0_vec as *const _,
                &uv1_vec as *const _,
                &tint_vec as *const _,
                &border_vec as *const _,
            );
        }
    }

    /// Display an image button
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let texture_id = 1u64;
    /// if ui.image_button("img_btn", texture_id, Vec2::new(50.0, 50.0)) {
    ///     println!("Image button clicked!");
    /// }
    /// # });
    /// ```
    pub fn image_button(&mut self, str_id: impl AsRef<str>, texture_id: u64, size: Vec2) -> bool {
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        let uv0_vec = sys::ImVec2 { x: 0.0, y: 0.0 };
        let uv1_vec = sys::ImVec2 { x: 1.0, y: 1.0 };
        let bg_vec = sys::ImVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }; // transparent
        let tint_vec = sys::ImVec4 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
            w: 1.0,
        }; // white
        let tex_ref = sys::ImTextureRef {
            _TexData: std::ptr::null_mut(),
            _TexID: texture_id,
        };
        unsafe {
            sys::ImGui_ImageButton(
                self.scratch_txt(str_id),
                tex_ref,
                &size_vec as *const _,
                &uv0_vec as *const _,
                &uv1_vec as *const _,
                &bg_vec as *const _,
                &tint_vec as *const _,
            )
        }
    }

    /// Display an image button with custom colors
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let texture_id = 1u64;
    /// if ui.image_button_ex(
    ///     "img_btn",
    ///     texture_id,
    ///     Vec2::new(50.0, 50.0),
    ///     Vec2::new(0.0, 0.0), // uv0
    ///     Vec2::new(1.0, 1.0), // uv1
    ///     Color::TRANSPARENT,  // background
    ///     Color::WHITE,        // tint
    /// ) {
    ///     println!("Image button clicked!");
    /// }
    /// # });
    /// ```
    pub fn image_button_ex(
        &mut self,
        str_id: impl AsRef<str>,
        texture_id: u64,
        size: Vec2,
        uv0: Vec2,
        uv1: Vec2,
        bg_col: crate::types::Color,
        tint_col: crate::types::Color,
    ) -> bool {
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        let uv0_vec = sys::ImVec2 { x: uv0.x, y: uv0.y };
        let uv1_vec = sys::ImVec2 { x: uv1.x, y: uv1.y };
        let bg_vec = sys::ImVec4 {
            x: bg_col.r(),
            y: bg_col.g(),
            z: bg_col.b(),
            w: bg_col.a(),
        };
        let tint_vec = sys::ImVec4 {
            x: tint_col.r(),
            y: tint_col.g(),
            z: tint_col.b(),
            w: tint_col.a(),
        };
        let tex_ref = sys::ImTextureRef {
            _TexData: std::ptr::null_mut(),
            _TexID: texture_id,
        };
        unsafe {
            sys::ImGui_ImageButton(
                self.scratch_txt(str_id),
                tex_ref,
                &size_vec as *const _,
                &uv0_vec as *const _,
                &uv1_vec as *const _,
                &bg_vec as *const _,
                &tint_vec as *const _,
            )
        }
    }
}
