//! Per-frame UI entry point
//!
//! The `Ui` type exposes most user-facing Dear ImGui APIs for a single frame:
//! creating windows, drawing widgets, accessing draw lists, showing built-in
//! tools and more. Obtain it from [`Context::frame`].
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! let mut ctx = Context::create();
//! let ui = ctx.frame();
//! ui.text("Hello, world!");
//! ```
//!

use crate::Id;
use crate::context::ContextBinding;
use crate::context::SharedTextureRegistry;
use crate::draw::DrawListMut;
use crate::input::MouseCursor;
use crate::internal::RawWrapper;
use crate::scope::NativeScopeTracker;
use crate::string::{ScratchBuffers, UiBuffer};
use crate::sys;
use crate::validation::{assert_finite_f32, assert_finite_vec2};
use std::cell::{Cell, RefCell};

pub use crate::scope::NativeWindowScope;

/// Represents the Dear ImGui user interface for one frame
#[derive(Debug)]
pub struct Ui {
    native_current: *const *mut sys::ImGuiContext,
    /// Dear ImGui context that owns this per-frame UI entry point.
    pub(crate) ctx: *mut sys::ImGuiContext,
    pub(crate) ctx_binding: ContextBinding,
    pub(crate) texture_registry: SharedTextureRegistry,
    pub(crate) ime: std::rc::Rc<RefCell<crate::context::ime::ImeState>>,
    pub(crate) native_scopes: RefCell<NativeScopeTracker>,
    pub(crate) scope_recovery_pending: Cell<bool>,
    /// Reused in stack order so nested label conversions and callbacks have independent storage.
    scratch_buffers: ScratchBuffers,
}

impl Ui {
    /// Creates a new Ui instance
    ///
    /// This should only be called by Context::create()
    pub(crate) fn new(
        ctx: *mut sys::ImGuiContext,
        ctx_binding: crate::ContextBinding,
        texture_registry: crate::context::SharedTextureRegistry,
        ime: std::rc::Rc<std::cell::RefCell<crate::context::ime::ImeState>>,
    ) -> Self {
        Ui {
            ctx,
            native_current: unsafe { sys::current_context_slot() },
            ctx_binding,
            texture_registry,
            ime,
            native_scopes: std::cell::RefCell::new(crate::scope::NativeScopeTracker::default()),
            scope_recovery_pending: std::cell::Cell::new(false),
            scratch_buffers: ScratchBuffers::default(),
        }
    }

    pub(crate) fn context_raw(&self) -> *mut sys::ImGuiContext {
        self.ctx
    }

    /// Returns a persistent capability for the Context that owns this `Ui`.
    pub fn binding(&self) -> crate::ContextBinding {
        self.ctx_binding.clone()
    }

    /// Returns the process-unique identity of the Context that owns this `Ui`.
    pub fn context_id(&self) -> crate::ContextId {
        self.ctx_binding.id()
    }

    #[inline]
    pub(crate) fn run_with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.assert_native_scopes_usable();
        self.with_live_context(f)
    }

    /// Enter the context owned by this live frame borrow, including during stack cleanup.
    #[inline]
    pub(crate) fn with_live_context<R>(&self, f: impl FnOnce() -> R) -> R {
        let current = unsafe {
            if sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
                // The native shim returns the address of the live GImGui slot.
                self.native_current.read()
            } else {
                sys::igGetCurrentContext()
            }
        };
        if current == self.ctx {
            f()
        } else {
            self.with_other_context_current(f)
        }
    }

    #[cold]
    #[inline(never)]
    fn with_other_context_current<R>(&self, f: impl FnOnce() -> R) -> R {
        let _bound = crate::context::binding::RawBoundContextGuard::bind(self.ctx);
        f()
    }

    pub(crate) fn resolve_texture_ref(
        &self,
        texture: crate::texture::TextureRef<'_>,
    ) -> Result<sys::ImTextureRef, crate::TextureError> {
        match texture.source() {
            crate::texture::TextureSource::Legacy(id) => Ok(sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: sys::ImTextureID::from(id),
            }),
            crate::texture::TextureSource::Managed(id) => {
                self.texture_registry.borrow().resolve(id)
            }
            crate::texture::TextureSource::FontAtlas { atlas, texture } => {
                let io = unsafe { sys::igGetIO_ContextPtr(self.ctx) };
                if io.is_null() || !std::ptr::eq(unsafe { (*io).Fonts }, atlas) {
                    return Err(crate::TextureError::ForeignFontAtlas);
                }
                Ok(texture)
            }
        }
    }

    /// Resolve a logical texture for submission to this frame's native draw lists.
    ///
    /// # Safety
    ///
    /// Use the reference on the owning context's thread. An extension may queue it until the
    /// end of its drawing scope to emit commands into this same ImGui frame. The context keeps
    /// managed and atlas textures alive for the resulting draw data. Limit the extension's own
    /// copy to this submission and consume the resulting frame through its renderer API.
    /// For a legacy texture ID, the caller supplies the renderer binding and its lifetime.
    #[doc(hidden)]
    pub unsafe fn resolve_texture_ref_raw(
        &self,
        texture: crate::texture::TextureRef<'_>,
    ) -> Result<sys::ImTextureRef, crate::TextureError> {
        self.run_with_bound_context(|| self.resolve_texture_ref(texture))
    }

    /// Runs a closure while this `Ui`'s owning ImGui context is current.
    ///
    /// The previously current context is restored before this method returns,
    /// including when the closure panics. This is primarily intended for
    /// extension crates that need to call raw Dear ImGui-adjacent FFI while
    /// still honoring the `Ui` that created the safe wrapper.
    pub fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.assert_native_scopes_usable();
        let _bound = crate::context::binding::RawBoundContextGuard::bind(self.ctx);
        f()
    }

    /// Returns an immutable reference to the inputs/outputs object
    #[doc(alias = "GetIO")]
    pub fn io(&self) -> &crate::io::Io {
        self.run_with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            if io.is_null() {
                panic!("Ui::io() requires an active ImGui context");
            }
            &*(io as *const crate::io::Io)
        })
    }

    /// Keeps the latest scratch allocation alive throughout `f`. Reentrant calls use another
    /// buffer. On return or unwind this buffer becomes the next one available for reuse.
    pub(crate) fn with_scratch_buffer<R>(&self, f: impl FnOnce(&mut UiBuffer) -> R) -> R {
        self.scratch_buffers.with(f)
    }

    #[inline]
    pub(crate) fn with_scratch_c_string<R>(
        &self,
        text: &str,
        f: impl FnOnce(*const std::ffi::c_char) -> R,
    ) -> R {
        self.scratch_buffers.with_c_string(text, f)
    }

    /// Keeps the staged C string alive while its native consumer runs.
    pub(crate) fn with_scratch_txt<R>(
        &self,
        text: impl AsRef<str>,
        f: impl FnOnce(*const std::ffi::c_char) -> R,
    ) -> R {
        let text = text.as_ref();
        self.run_with_bound_context(|| self.with_scratch_c_string(text, f))
    }

    pub(crate) fn with_scratch_txt_two<R>(
        &self,
        first: impl AsRef<str>,
        second: impl AsRef<str>,
        f: impl FnOnce(*const std::ffi::c_char, *const std::ffi::c_char) -> R,
    ) -> R {
        self.with_scratch_txt_with_opt(first, Some(second), f)
    }

    pub(crate) fn with_scratch_txt_with_opt<R>(
        &self,
        first: impl AsRef<str>,
        second: Option<impl AsRef<str>>,
        f: impl FnOnce(*const std::ffi::c_char, *const std::ffi::c_char) -> R,
    ) -> R {
        let first = first.as_ref();
        let second = second.as_ref().map(AsRef::as_ref);
        self.run_with_bound_context(|| {
            self.with_scratch_buffer(|buf| {
                let (first, second) = buf.scratch_txt_with_opt(first, second);
                f(first, second)
            })
        })
    }

    pub(crate) fn with_scratch_txt_opt<R>(
        &self,
        text: Option<&str>,
        f: impl FnOnce(*const std::ffi::c_char) -> R,
    ) -> R {
        match text {
            Some(text) => self.with_scratch_txt(text, f),
            None => self.run_with_bound_context(|| f(std::ptr::null())),
        }
    }

    /// Returns an ID from a string label in the current ID scope.
    ///
    /// This mirrors `ImGui::GetID(label)`. Useful for building stable IDs
    /// for widgets or dockspaces inside the current window/scope.
    #[doc(alias = "GetID")]
    pub fn get_id(&self, label: &str) -> Id {
        let start = if label.is_empty() {
            c"".as_ptr()
        } else {
            label.as_ptr().cast()
        };
        self.run_with_bound_context(|| unsafe {
            Id::from(sys::igGetID_StrStr(start, start.add(label.len())))
        })
    }

    /// Set next item to be open by default.
    ///
    /// This is useful for tree nodes, collapsing headers, etc.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open(&self, is_open: bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemOpen(is_open, 0); // 0 = ImGuiCond_Always
        });
    }

    /// Set next item to be open by default with condition.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open_with_cond(&self, is_open: bool, cond: crate::Condition) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemOpen(is_open, cond as sys::ImGuiCond)
        });
    }

    /// Set next item width.
    ///
    /// Set to 0.0 for default width, >0.0 for explicit width, <0.0 for relative width.
    #[doc(alias = "SetNextItemWidth")]
    pub fn set_next_item_width(&self, item_width: f32) {
        assert_finite_f32("Ui::set_next_item_width()", "item_width", item_width);
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemWidth(item_width);
        });
    }

    /// Display a text label with a boolean value (for quick debug UIs).
    #[doc(alias = "Value")]
    pub fn value_bool(&self, prefix: impl AsRef<str>, v: bool) {
        self.with_scratch_txt(prefix, |prefix| unsafe { sys::igValue_Bool(prefix, v) })
    }

    /// Creates a window builder
    pub fn window<'ui>(
        &'ui self,
        name: impl Into<crate::WindowLabel<'ui>>,
    ) -> crate::window::Window<'ui> {
        crate::window::Window::new(self, name)
    }

    /// Focus a window by name, or clear focus from all windows.
    ///
    /// Passing `None` is equivalent to `ImGui::SetWindowFocus(NULL)` in the C++ API.
    /// This can be used to "unfocus" the entire UI (e.g. on Escape, to behave like
    /// clicking outside of the UI).
    #[doc(alias = "SetWindowFocus")]
    pub fn set_window_focus(&self, name: Option<&str>) {
        self.with_scratch_txt_opt(name, |name| unsafe { sys::igSetWindowFocus_Str(name) });
    }

    /// Sets the position of the current window.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos(&self, pos: [f32; 2]) {
        self.set_window_pos_with_cond(pos, crate::Condition::Always);
    }

    /// Sets the position of the current window with a condition.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos_with_cond(&self, pos: [f32; 2], cond: crate::Condition) {
        assert_finite_vec2("Ui::set_window_pos_with_cond()", "position", pos);
        let pos_vec = sys::ImVec2_c {
            x: pos[0],
            y: pos[1],
        };
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowPos_Vec2(pos_vec, cond as sys::ImGuiCond)
        });
    }

    /// Sets the position of a named window.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos_by_name(&self, name: impl AsRef<str>, pos: [f32; 2]) {
        self.set_window_pos_by_name_with_cond(name, pos, crate::Condition::Always);
    }

    /// Sets the position of a named window with a condition.
    #[doc(alias = "SetWindowPos")]
    pub fn set_window_pos_by_name_with_cond(
        &self,
        name: impl AsRef<str>,
        pos: [f32; 2],
        cond: crate::Condition,
    ) {
        assert_finite_vec2("Ui::set_window_pos_by_name_with_cond()", "position", pos);
        let pos_vec = sys::ImVec2_c {
            x: pos[0],
            y: pos[1],
        };
        self.with_scratch_txt(name, |name| unsafe {
            sys::igSetWindowPos_Str(name, pos_vec, cond as sys::ImGuiCond)
        });
    }

    /// Sets the size of the current window.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size(&self, size: [f32; 2]) {
        self.set_window_size_with_cond(size, crate::Condition::Always);
    }

    /// Sets the size of the current window with a condition.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size_with_cond(&self, size: [f32; 2], cond: crate::Condition) {
        assert_finite_vec2("Ui::set_window_size_with_cond()", "size", size);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowSize_Vec2(size_vec, cond as sys::ImGuiCond)
        });
    }

    /// Sets the size of a named window.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size_by_name(&self, name: impl AsRef<str>, size: [f32; 2]) {
        self.set_window_size_by_name_with_cond(name, size, crate::Condition::Always);
    }

    /// Sets the size of a named window with a condition.
    #[doc(alias = "SetWindowSize")]
    pub fn set_window_size_by_name_with_cond(
        &self,
        name: impl AsRef<str>,
        size: [f32; 2],
        cond: crate::Condition,
    ) {
        assert_finite_vec2("Ui::set_window_size_by_name_with_cond()", "size", size);
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };
        self.with_scratch_txt(name, |name| unsafe {
            sys::igSetWindowSize_Str(name, size_vec, cond as sys::ImGuiCond);
        });
    }

    /// Collapses or expands the current window.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed(&self, collapsed: bool) {
        self.set_window_collapsed_with_cond(collapsed, crate::Condition::Always);
    }

    /// Collapses or expands the current window with a condition.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed_with_cond(&self, collapsed: bool, cond: crate::Condition) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetWindowCollapsed_Bool(collapsed, cond as sys::ImGuiCond)
        });
    }

    /// Collapses or expands a named window.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed_by_name(&self, name: impl AsRef<str>, collapsed: bool) {
        self.set_window_collapsed_by_name_with_cond(name, collapsed, crate::Condition::Always);
    }

    /// Collapses or expands a named window with a condition.
    #[doc(alias = "SetWindowCollapsed")]
    pub fn set_window_collapsed_by_name_with_cond(
        &self,
        name: impl AsRef<str>,
        collapsed: bool,
        cond: crate::Condition,
    ) {
        self.with_scratch_txt(name, |name| unsafe {
            sys::igSetWindowCollapsed_Str(name, collapsed, cond as sys::ImGuiCond);
        });
    }

    /// Returns DPI scale currently associated to the current window's viewport.
    #[doc(alias = "GetWindowDpiScale")]
    pub fn window_dpi_scale(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetWindowDpiScale() })
    }

    /// Get current window width (shortcut for `GetWindowSize().x`).
    #[doc(alias = "GetWindowWidth")]
    pub fn window_width(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetWindowWidth() })
    }

    /// Get current window height (shortcut for `GetWindowSize().y`).
    #[doc(alias = "GetWindowHeight")]
    pub fn window_height(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetWindowHeight() })
    }

    /// Get current window position in screen space.
    #[doc(alias = "GetWindowPos")]
    pub fn window_pos(&self) -> [f32; 2] {
        let v = self.run_with_bound_context(|| unsafe { sys::igGetWindowPos() });
        [v.x, v.y]
    }

    /// Get current window size.
    #[doc(alias = "GetWindowSize")]
    pub fn window_size(&self) -> [f32; 2] {
        let v = self.run_with_bound_context(|| unsafe { sys::igGetWindowSize() });
        [v.x, v.y]
    }

    /// Returns a reference to the main Dear ImGui viewport (safe wrapper)
    ///
    /// Same viewport used by [`Ui::dockspace`](crate::Ui::dockspace)'s default host.
    ///
    /// The returned reference is owned by the currently active ImGui context and
    /// must not be used after the context is destroyed.
    #[doc(alias = "GetMainViewport")]
    pub fn main_viewport(&self) -> &crate::platform_io::Viewport {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igGetMainViewport();
            if ptr.is_null() {
                panic!("Ui::main_viewport() requires an active ImGui context");
            }
            crate::platform_io::Viewport::from_raw(ptr as *const sys::ImGuiViewport)
        })
    }

    /// Set the viewport for the next window.
    ///
    /// This is a convenience wrapper over `ImGui::SetNextWindowViewport`.
    /// Useful when hosting a fullscreen DockSpace window inside the main viewport.
    #[doc(alias = "SetNextWindowViewport")]
    pub fn set_next_window_viewport(&self, viewport_id: Id) {
        self.run_with_bound_context(|| unsafe { sys::igSetNextWindowViewport(viewport_id.into()) });
    }

    /// Returns the viewport of the current window.
    ///
    /// Dear ImGui keeps an implicit fallback window current for the duration of an open frame, so
    /// this may be called before, after, or outside an explicit user window's `Begin`/`End` scope.
    /// Inside an explicit window it returns that window's viewport; after the window ends it
    /// returns the fallback window's viewport again.
    ///
    /// # Panics
    ///
    /// Panics if raw/native API use has cleared the current viewport while this [`Ui`] still
    /// represents an active frame.
    #[doc(alias = "GetWindowViewport")]
    pub fn window_viewport(&self) -> &crate::platform_io::Viewport {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igGetWindowViewport();
            if ptr.is_null() {
                panic!("Ui::window_viewport() requires a current window");
            }
            crate::platform_io::Viewport::from_raw(ptr as *const sys::ImGuiViewport)
        })
    }

    /// Find a viewport by ID.
    #[doc(alias = "FindViewportByID")]
    pub fn find_viewport_by_id(&self, viewport_id: Id) -> Option<&crate::platform_io::Viewport> {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igFindViewportByID(viewport_id.raw());
            if ptr.is_null() {
                None
            } else {
                Some(crate::platform_io::Viewport::from_raw(
                    ptr as *const sys::ImGuiViewport,
                ))
            }
        })
    }

    /// Find a viewport by its platform handle.
    ///
    /// The platform handle type depends on the backend (e.g. `HWND` on Windows).
    #[doc(alias = "FindViewportByPlatformHandle")]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn find_viewport_by_platform_handle(
        &self,
        platform_handle: *mut std::ffi::c_void,
    ) -> Option<&crate::platform_io::Viewport> {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igFindViewportByPlatformHandle(platform_handle);
            if ptr.is_null() {
                None
            } else {
                Some(crate::platform_io::Viewport::from_raw(
                    ptr as *const sys::ImGuiViewport,
                ))
            }
        })
    }

    /// Draw with the current window's draw list while its Context is bound.
    ///
    /// The borrow and any builders or stack tokens derived from it cannot escape the closure.
    ///
    /// ```compile_fail,E0521
    /// use dear_imgui_rs::{DrawListMut, Ui};
    ///
    /// fn escape(ui: &Ui) {
    ///     let mut saved: Option<&DrawListMut<'_>> = None;
    ///     ui.with_window_draw_list(|draw_list| saved = Some(draw_list));
    ///     drop(saved);
    /// }
    /// ```
    #[doc(alias = "GetWindowDrawList")]
    pub fn with_window_draw_list<R>(&self, draw: impl FnOnce(&DrawListMut<'_>) -> R) -> R {
        self.with_bound_context(|| draw(&DrawListMut::window(self)))
    }

    /// Access to the background draw list
    #[doc(alias = "GetBackgroundDrawList")]
    pub fn get_background_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::background(self)
    }

    /// Access to the foreground draw list
    #[doc(alias = "GetForegroundDrawList")]
    pub fn get_foreground_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::foreground(self)
    }

    /// Returns the currently desired mouse cursor type
    ///
    /// Returns `None` if no cursor should be displayed
    #[doc(alias = "GetMouseCursor")]
    pub fn mouse_cursor(&self) -> Option<MouseCursor> {
        unsafe {
            match sys::igGetMouseCursor() {
                sys::ImGuiMouseCursor_Arrow => Some(MouseCursor::Arrow),
                sys::ImGuiMouseCursor_TextInput => Some(MouseCursor::TextInput),
                sys::ImGuiMouseCursor_ResizeAll => Some(MouseCursor::ResizeAll),
                sys::ImGuiMouseCursor_ResizeNS => Some(MouseCursor::ResizeNS),
                sys::ImGuiMouseCursor_ResizeEW => Some(MouseCursor::ResizeEW),
                sys::ImGuiMouseCursor_ResizeNESW => Some(MouseCursor::ResizeNESW),
                sys::ImGuiMouseCursor_ResizeNWSE => Some(MouseCursor::ResizeNWSE),
                sys::ImGuiMouseCursor_Hand => Some(MouseCursor::Hand),
                sys::ImGuiMouseCursor_NotAllowed => Some(MouseCursor::NotAllowed),
                _ => None,
            }
        }
    }

    /// Sets the desired mouse cursor type
    ///
    /// Passing `None` hides the mouse cursor
    #[doc(alias = "SetMouseCursor")]
    pub fn set_mouse_cursor(&self, cursor_type: Option<MouseCursor>) {
        self.run_with_bound_context(|| unsafe {
            let val: sys::ImGuiMouseCursor = cursor_type
                .map(|x| x as sys::ImGuiMouseCursor)
                .unwrap_or(sys::ImGuiMouseCursor_None);
            sys::igSetMouseCursor(val);
        });
    }

    /// Controls whether Dear ImGui renders its own mouse cursor for this Context.
    ///
    /// This is the frame-scoped alternative to mutably borrowing [`crate::Io`] while a `Ui` is
    /// live. Platform backends should hide the OS cursor while this value is enabled.
    pub fn set_mouse_draw_cursor(&self, draw: bool) {
        self.run_with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            assert!(
                !io.is_null(),
                "Ui::set_mouse_draw_cursor() requires an active ImGui context"
            );
            (*io).MouseDrawCursor = draw;
        });
    }

    // ============================================================================
    // Focus and Navigation
    // ============================================================================

    /// Focuses keyboard on the next widget.
    ///
    /// This is the equivalent to [set_keyboard_focus_here_with_offset](Self::set_keyboard_focus_here_with_offset)
    /// with `offset` set to 0.
    #[doc(alias = "SetKeyboardFocusHere")]
    pub fn set_keyboard_focus_here(&self) {
        self.set_keyboard_focus_here_with_offset(0);
    }

    /// Focuses keyboard on a widget relative to current position.
    ///
    /// Use positive offset to focus on next widgets, negative offset to focus on previous widgets.
    #[doc(alias = "SetKeyboardFocusHere")]
    pub fn set_keyboard_focus_here_with_offset(&self, offset: i32) {
        unsafe {
            sys::igSetKeyboardFocusHere(offset);
        }
    }

    /// Shows or hides the navigation cursor (a small marker indicating nav focus).
    #[doc(alias = "SetNavCursorVisible")]
    pub fn set_nav_cursor_visible(&self, visible: bool) {
        self.run_with_bound_context(|| unsafe { sys::igSetNavCursorVisible(visible) });
    }

    /// Renders a style editor block (not a window) for the given `Style` structure.
    ///
    /// The safe editor retains all upstream style controls except its destructive Fonts tab.
    /// Font selection and font-scale controls remain available because they do not mutate the
    /// atlas topology.
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_style_editor(&self, style: &mut crate::style::Style) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_style_editor_without_font_atlas(style.raw_mut());
        });
    }

    /// Renders a style editor block (not a window) for the currently active style.
    ///
    /// The safe editor retains all upstream style controls except its destructive Fonts tab.
    #[doc(alias = "ShowStyleEditor")]
    pub fn show_default_style_editor(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_style_editor_without_font_atlas(std::ptr::null_mut());
        });
    }

    /// Renders the exact upstream style editor for the given `Style` structure.
    ///
    /// Prefer [`show_style_editor`](Self::show_style_editor) unless the application deliberately
    /// owns the full font-atlas mutation contract.
    ///
    /// # Safety
    ///
    /// The upstream Fonts tab exposes destructive font-atlas operations that bypass Rust's atlas
    /// generation tracking. The caller must uphold the native font-atlas contract.
    pub unsafe fn show_upstream_style_editor(&self, style: &mut crate::style::Style) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowStyleEditor(style.raw_mut());
        });
    }

    /// Renders the exact upstream style editor for the active style.
    ///
    /// # Safety
    ///
    /// The upstream Fonts tab exposes destructive font-atlas operations that bypass Rust's atlas
    /// generation tracking. The caller must uphold the native font-atlas contract.
    pub unsafe fn show_upstream_default_style_editor(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowStyleEditor(std::ptr::null_mut());
        });
    }

    // ============================================================================
    // Style Access
    // ============================================================================

    /// Borrows the live [`crate::Style`] of this UI's Context.
    ///
    /// Accessors reflect subsequent style changes, including stack pushes and pops.
    /// Use [`Self::clone_style`] for an independent snapshot.
    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> &crate::Style {
        self.run_with_bound_context(|| unsafe { &*sys::igGetStyle().cast::<crate::Style>() })
    }

    /// Returns an independent copy of the current style.
    #[doc(alias = "GetStyle")]
    pub fn clone_style(&self) -> crate::Style {
        self.style().clone()
    }

    /// Apply the built-in Dark style to the current style.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark(&self) {
        self.run_with_bound_context(|| unsafe { sys::igStyleColorsDark(std::ptr::null_mut()) });
    }

    /// Apply the built-in Light style to the current style.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light(&self) {
        self.run_with_bound_context(|| unsafe { sys::igStyleColorsLight(std::ptr::null_mut()) });
    }

    /// Apply the built-in Classic style to the current style.
    #[doc(alias = "StyleColorsClassic")]
    pub fn style_colors_classic(&self) {
        self.run_with_bound_context(|| unsafe { sys::igStyleColorsClassic(std::ptr::null_mut()) });
    }

    /// Write the Dark style values into the provided [`crate::Style`] object.
    #[doc(alias = "StyleColorsDark")]
    pub fn style_colors_dark_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsDark(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Write the Light style values into the provided [`crate::Style`] object.
    #[doc(alias = "StyleColorsLight")]
    pub fn style_colors_light_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsLight(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Write the Classic style values into the provided [`crate::Style`] object.
    #[doc(alias = "StyleColorsClassic")]
    pub fn style_colors_classic_into(&self, dst: &mut crate::Style) {
        self.run_with_bound_context(|| unsafe {
            sys::igStyleColorsClassic(dst.raw_mut() as *mut sys::ImGuiStyle)
        });
    }

    /// Renders a style selector combo box.
    ///
    /// Returns true when a different style was selected.
    #[doc(alias = "ShowStyleSelector")]
    pub fn show_style_selector(&self, label: impl AsRef<str>) -> bool {
        self.with_scratch_txt(label, |label| unsafe { sys::igShowStyleSelector(label) })
    }

    /// Renders a font selector combo box.
    #[doc(alias = "ShowFontSelector")]
    pub fn show_font_selector(&self, label: impl AsRef<str>) {
        self.with_scratch_txt(label, |label| unsafe {
            sys::igShowFontSelector(label);
        });
    }

    /// Renders Dear ImGui's demo window without its destructive font-atlas debug controls.
    ///
    /// This preserves the ordinary demo, Metrics/Debugger, and Style Editor controls. Only the
    /// panels backed by upstream `ShowFontAtlas()` are omitted, so the safe API does not bypass
    /// Rust's font-atlas lifetime and generation tracking.
    ///
    /// Use [`show_upstream_demo_window`](Self::show_upstream_demo_window) to opt into the exact
    /// upstream window, including its font-atlas controls.
    #[doc(alias = "ShowDemoWindow")]
    pub fn show_demo_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_demo_window_without_font_atlas(opened);
        });
    }

    /// Renders the exact upstream Dear ImGui demo window, including font-atlas debug controls.
    ///
    /// Prefer [`show_demo_window`](Self::show_demo_window) unless the application deliberately
    /// owns the full font-atlas mutation contract.
    ///
    /// # Safety
    ///
    /// With `BackendFlags::RENDERER_HAS_TEXTURES`, the upstream Fonts panel can delete an
    /// `ImFont` and continue reading it in the same native call. Other destructive controls also
    /// bypass Rust's atlas-generation tracking. The caller must prevent those controls from being
    /// activated or otherwise uphold the native font-atlas contract.
    pub unsafe fn show_upstream_demo_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowDemoWindow(opened);
        });
    }

    /// Renders an about window.
    ///
    /// Displays the Dear ImGui version/credits, and build/system information.
    #[doc(alias = "ShowAboutWindow")]
    pub fn show_about_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowAboutWindow(opened);
        });
    }

    /// Renders a metrics/debug window without its destructive font-atlas tree.
    ///
    /// Displays Dear ImGui internals: draw commands (with individual draw calls and vertices),
    /// window list, basic internal state, etc.
    #[doc(alias = "ShowMetricsWindow")]
    pub fn show_metrics_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_metrics_window_without_font_atlas(opened);
        });
    }

    /// Renders the exact upstream metrics/debug window, including its font-atlas tree.
    ///
    /// # Safety
    ///
    /// The upstream Fonts tree can mutate or destroy font-atlas data while Rust font handles and
    /// renderer state are live. The caller must uphold the native font-atlas contract.
    pub unsafe fn show_upstream_metrics_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowMetricsWindow(opened);
        });
    }

    /// Renders upstream's internal Font Atlas debug panel for this context.
    ///
    /// This is the isolated font-specific part omitted from the safe demo, metrics, and style
    /// editor APIs.
    ///
    /// # Safety
    ///
    /// The panel exposes destructive atlas operations and may continue using native font pointers
    /// after a control mutates the atlas. The caller must uphold the native font-atlas contract.
    #[doc(alias = "ShowFontAtlas")]
    pub unsafe fn show_font_atlas_debug_panel(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::dear_imgui_rs_show_font_atlas_debug_panel();
        });
    }

    /// Renders a basic help/info block (not a window)
    #[doc(alias = "ShowUserGuide")]
    pub fn show_user_guide(&self) {
        self.run_with_bound_context(|| unsafe {
            crate::sys::igShowUserGuide();
        });
    }

    // ============================================================================
    // Additional Demo, Debug, Information (non-duplicate methods)
    // ============================================================================

    /// Renders a debug log window.
    ///
    /// Displays a simplified log of important dear imgui events.
    #[doc(alias = "ShowDebugLogWindow")]
    pub fn show_debug_log_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowDebugLogWindow(opened);
        });
    }

    /// Renders an ID stack tool window.
    ///
    /// Hover items with mouse to query information about the source of their unique ID.
    #[doc(alias = "ShowIDStackToolWindow")]
    pub fn show_id_stack_tool_window(&self, opened: &mut bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igShowIDStackToolWindow(opened);
        });
    }

    /// Renders a table that breaks `text` down into UTF-8 bytes and codepoints.
    ///
    /// This is intended for diagnosing text encoding and missing-glyph issues.
    ///
    /// # Panics
    ///
    /// Panics if `text` contains an interior NUL byte, which the upstream
    /// NUL-terminated API cannot represent.
    #[doc(alias = "DebugTextEncoding")]
    pub fn debug_text_encoding(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        assert!(
            !text.contains('\0'),
            "Ui::debug_text_encoding() text must not contain interior NUL bytes"
        );
        self.with_scratch_txt(text, |text| unsafe { sys::igDebugTextEncoding(text) });
    }

    /// Temporarily flashes a style color in Dear ImGui's debug tools.
    #[doc(alias = "DebugFlashStyleColor")]
    pub fn debug_flash_style_color(&self, color: crate::StyleColor) {
        self.run_with_bound_context(|| unsafe { sys::igDebugFlashStyleColor(color as i32) });
    }

    /// Starts Dear ImGui's interactive item picker debug tool.
    #[doc(alias = "DebugStartItemPicker")]
    pub fn debug_start_item_picker(&self) {
        self.run_with_bound_context(|| unsafe { sys::igDebugStartItemPicker() });
    }

    /// Returns the Dear ImGui version string
    #[doc(alias = "GetVersion")]
    pub fn get_version(&self) -> &str {
        self.run_with_bound_context(|| unsafe {
            let version_ptr = sys::igGetVersion();
            if version_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(version_ptr);
            c_str.to_str().unwrap_or("Unknown")
        })
    }
}

#[cfg(test)]
mod style_tests {
    #[test]
    fn safe_style_editors_keep_font_atlas_controls_explicit() {
        let _: fn(&crate::Ui, &mut crate::Style) = crate::Ui::show_style_editor;
        let _: fn(&crate::Ui) = crate::Ui::show_default_style_editor;
        let _: unsafe fn(&crate::Ui, &mut crate::Style) = crate::Ui::show_upstream_style_editor;
        let _: unsafe fn(&crate::Ui) = crate::Ui::show_upstream_default_style_editor;
    }
}

#[cfg(test)]
mod debug_tools_tests {
    #[test]
    fn safe_debug_windows_keep_font_atlas_controls_explicit() {
        let _: fn(&crate::Ui, &mut bool) = crate::Ui::show_demo_window;
        let _: fn(&crate::Ui, &mut bool) = crate::Ui::show_metrics_window;
        let _: unsafe fn(&crate::Ui, &mut bool) = crate::Ui::show_upstream_demo_window;
        let _: unsafe fn(&crate::Ui, &mut bool) = crate::Ui::show_upstream_metrics_window;
        let _: unsafe fn(&crate::Ui) = crate::Ui::show_font_atlas_debug_panel;
    }

    #[test]
    fn public_debug_helpers_are_safe_to_call_in_a_frame() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        let ui = ctx.frame();

        let mut demo_open = true;
        ui.show_demo_window(&mut demo_open);
        let mut metrics_open = true;
        ui.show_metrics_window(&mut metrics_open);

        ui.window("debug_helpers").build(|| {
            ui.show_default_style_editor();
            let cursor_y = ui.cursor_pos_y();
            ui.debug_text_encoding("A UTF-8 string: 界");
            assert!(ui.cursor_pos_y() > cursor_y);

            ui.debug_flash_style_color(crate::StyleColor::Text);
            ui.debug_start_item_picker();
        });

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.debug_text_encoding("A\0B");
            }))
            .is_err()
        );
    }
}
