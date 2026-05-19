use crate::sys;

use super::Context;
use super::binding::CTX_MUTEX;

impl Context {
    /// Creates a new frame and returns a Ui object for building the interface.
    ///
    /// Note: you must update `io.DisplaySize` (and usually `io.DeltaTime`) before calling this,
    /// unless you are using a platform backend that does it for you (e.g. `dear-imgui-winit`).
    pub fn frame(&mut self) -> &mut crate::ui::Ui {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::frame()");

        unsafe {
            // Dear ImGui initializes DisplaySize to (-1, -1). Calling NewFrame() without a
            // platform backend (or without setting DisplaySize manually) will trip an internal
            // assertion and abort the process. Fail fast with a Rust panic to make the setup
            // requirement obvious.
            let io = sys::igGetIO_Nil();
            if !io.is_null() && ((*io).DisplaySize.x < 0.0 || (*io).DisplaySize.y < 0.0) {
                panic!(
                    "Context::frame() called with invalid io.DisplaySize ({}, {}). \
Set io.DisplaySize (and typically io.DeltaTime) before starting a frame. \
If you are using a windowing/event-loop library, prefer a platform backend such as \
dear-imgui-winit::WinitPlatform::prepare_frame().",
                    (*io).DisplaySize.x,
                    (*io).DisplaySize.y
                );
            }
            sys::igNewFrame();
        }
        &mut self.ui
    }

    /// Create a new frame with a callback
    pub fn frame_with<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&crate::ui::Ui) -> R,
    {
        let ui = self.frame();
        f(ui)
    }

    /// Renders the frame and returns a mutable reference to the resulting draw data
    ///
    /// This finalizes the Dear ImGui frame and prepares all draw data for rendering.
    /// The returned draw data contains all the information needed to render the frame.
    ///
    /// Renderer backends receive mutable draw data because ImGui 1.92 texture requests require
    /// backends to write `TexID`/`Status` feedback into `ImTextureData`.
    pub fn render(&mut self) -> &mut crate::render::DrawData {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::render()");

        unsafe {
            sys::igRender();
            let dd = sys::igGetDrawData();
            if dd.is_null() {
                panic!("Context::render() returned null draw data");
            }
            &mut *(dd as *mut crate::render::DrawData)
        }
    }

    /// Gets the draw data for the current frame
    ///
    /// This returns the draw data without calling render. Only valid after
    /// `render()` has been called and before the next `new_frame()`.
    pub fn draw_data(&self) -> Option<&crate::render::DrawData> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::draw_data()");

        unsafe {
            let draw_data = sys::igGetDrawData();
            if draw_data.is_null() {
                None
            } else {
                let data = &*(draw_data as *const crate::render::DrawData);
                if data.valid() { Some(data) } else { None }
            }
        }
    }

    /// Gets mutable draw data for the current frame.
    ///
    /// This returns the draw data without calling render. Only valid after `render()` has been
    /// called and before the next `new_frame()`. Use this when a renderer needs to process texture
    /// updates after draw data has already been produced.
    pub fn draw_data_mut(&mut self) -> Option<&mut crate::render::DrawData> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::draw_data_mut()");

        unsafe {
            let draw_data = sys::igGetDrawData();
            if draw_data.is_null() {
                None
            } else {
                let data = &mut *(draw_data as *mut crate::render::DrawData);
                if data.valid() { Some(data) } else { None }
            }
        }
    }
}
