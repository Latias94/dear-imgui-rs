use std::ffi::CString;
use std::ptr;
use std::rc::{Rc, Weak};

use crate::clipboard::ClipboardContext;
use crate::fonts::SharedFontAtlas;
use crate::io::Io;
use crate::sys;

use super::binding::{CTX_MUTEX, clear_current_context, no_current_context, with_bound_context};
use super::texture_registry::unregister_user_textures_for_context;

/// An imgui context.
///
/// A context needs to be created to access most library functions. Due to current Dear ImGui
/// design choices, at most one active Context can exist at any time. This limitation will likely
/// be removed in a future Dear ImGui version.
///
/// If you need more than one context, you can use suspended contexts. As long as only one context
/// is active at a time, it's possible to have multiple independent contexts.
///
/// # Examples
///
/// Creating a new active context:
/// ```
/// let ctx = dear_imgui_rs::Context::create();
/// // ctx is dropped naturally when it goes out of scope, which deactivates and destroys the
/// // context
/// ```
///
/// Never try to create an active context when another one is active:
///
/// ```should_panic
/// let ctx1 = dear_imgui_rs::Context::create();
///
/// let ctx2 = dear_imgui_rs::Context::create(); // PANIC
/// ```
#[derive(Debug)]
pub struct Context {
    pub(super) raw: *mut sys::ImGuiContext,
    pub(super) alive: Rc<()>,
    pub(in crate::context) shared_font_atlas: Option<SharedFontAtlas>,
    pub(in crate::context) ini_filename: Option<CString>,
    pub(in crate::context) log_filename: Option<CString>,
    pub(in crate::context) platform_name: Option<CString>,
    pub(in crate::context) renderer_name: Option<CString>,
    // Boxed so the raw PlatformIO user-data pointer remains stable.
    // Interior mutability and reentrancy guarding live inside ClipboardContext.
    pub(in crate::context) clipboard_ctx: Box<ClipboardContext>,
    pub(in crate::context) ui: crate::ui::Ui,
}

/// A weak token that indicates whether a `Context` is still alive.
#[derive(Clone, Debug)]
pub struct ContextAliveToken(Weak<()>);

impl ContextAliveToken {
    /// Returns true if the originating `Context` has not been dropped.
    pub fn is_alive(&self) -> bool {
        self.0.upgrade().is_some()
    }
}

impl Context {
    /// Tries to create a new active Dear ImGui context.
    ///
    /// Returns an error if another context is already active or creation fails.
    pub fn try_create() -> crate::error::ImGuiResult<Context> {
        Self::try_create_internal(None)
    }

    /// Tries to create a new active Dear ImGui context with a shared font atlas.
    pub fn try_create_with_shared_font_atlas(
        shared_font_atlas: SharedFontAtlas,
    ) -> crate::error::ImGuiResult<Context> {
        Self::try_create_internal(Some(shared_font_atlas))
    }

    /// Creates a new active Dear ImGui context (panics on error).
    ///
    /// This aligns with imgui-rs behavior. For fallible creation use `try_create()`.
    pub fn create() -> Context {
        Self::try_create().expect("Failed to create Dear ImGui context")
    }

    /// Creates a new active Dear ImGui context with a shared font atlas (panics on error).
    pub fn create_with_shared_font_atlas(shared_font_atlas: SharedFontAtlas) -> Context {
        Self::try_create_with_shared_font_atlas(shared_font_atlas)
            .expect("Failed to create Dear ImGui context")
    }

    /// Returns the raw `ImGuiContext*` for FFI integrations.
    pub fn as_raw(&self) -> *mut sys::ImGuiContext {
        self.raw
    }

    /// Returns a token that can be used to check whether this context is still alive.
    ///
    /// Useful for extension crates that store raw pointers and need to avoid calling into FFI
    /// after the owning `Context` has been dropped.
    pub fn alive_token(&self) -> ContextAliveToken {
        ContextAliveToken(Rc::downgrade(&self.alive))
    }

    // removed legacy create_or_panic variants (use create()/try_create())

    pub(super) fn io_ptr(&self, caller: &str) -> *mut sys::ImGuiIO {
        let io = unsafe { sys::igGetIO_ContextPtr(self.raw) };
        if io.is_null() {
            panic!("{caller} requires a valid ImGui context");
        }
        io
    }

    pub(super) fn platform_io_ptr(&self, caller: &str) -> *mut sys::ImGuiPlatformIO {
        let pio = unsafe { sys::igGetPlatformIO_ContextPtr(self.raw) };
        if pio.is_null() {
            panic!("{caller} requires a valid ImGui context");
        }
        pio
    }

    pub(super) fn assert_current_context(&self, caller: &str) {
        assert!(
            self.is_current_context(),
            "{caller} requires this context to be current"
        );
    }

    fn try_create_internal(
        mut shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Context> {
        let _guard = CTX_MUTEX.lock();

        if !no_current_context() {
            return Err(crate::error::ImGuiError::ContextAlreadyActive);
        }

        let shared_font_atlas_ptr = match &mut shared_font_atlas {
            Some(atlas) => atlas.as_ptr_mut(),
            None => ptr::null_mut(),
        };

        // Create the actual ImGui context
        let raw = unsafe { sys::igCreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            return Err(crate::error::ImGuiError::ContextCreation {
                reason: "ImGui_CreateContext returned null".to_string(),
            });
        }

        // Set it as the current context
        unsafe {
            sys::igSetCurrentContext(raw);
        }

        Ok(Context {
            raw,
            alive: Rc::new(()),
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(ClipboardContext::dummy()),
            ui: crate::ui::Ui::new(),
        })
    }

    /// Returns a mutable reference to this context's IO object.
    pub fn io_mut(&mut self) -> &mut Io {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io_ptr = self.io_ptr("Context::io_mut()");
            &mut *(io_ptr as *mut Io)
        }
    }

    /// Get shared access to this context's IO object.
    pub fn io(&self) -> &crate::io::Io {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io_ptr = self.io_ptr("Context::io()");
            &*(io_ptr as *const crate::io::Io)
        }
    }

    /// Get access to the Style structure
    pub fn style(&self) -> &crate::style::Style {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let style_ptr = sys::igGetStyle();
                if style_ptr.is_null() {
                    panic!("Context::style() requires a valid ImGui context");
                }
                &*(style_ptr as *const crate::style::Style)
            })
        }
    }

    /// Get mutable access to the Style structure
    pub fn style_mut(&mut self) -> &mut crate::style::Style {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let style_ptr = sys::igGetStyle();
                if style_ptr.is_null() {
                    panic!("Context::style_mut() requires a valid ImGui context");
                }
                &mut *(style_ptr as *mut crate::style::Style)
            })
        }
    }

    pub(super) fn is_current_context(&self) -> bool {
        let ctx = unsafe { sys::igGetCurrentContext() };
        self.raw == ctx
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            if !self.raw.is_null() {
                unregister_user_textures_for_context(self.raw);
                if self.shared_font_atlas.is_none() {
                    let io = sys::igGetIO_ContextPtr(self.raw);
                    if !io.is_null() {
                        crate::fonts::forget_font_atlas_generation((*io).Fonts);
                    }
                }
                crate::platform_io::clear_typed_callbacks_for_context(self.raw);
                with_bound_context(self.raw, || {
                    crate::platform_io::clear_out_param_callbacks_for_current_context();
                });
                if sys::igGetCurrentContext() == self.raw {
                    clear_current_context();
                }
                sys::igDestroyContext(self.raw);
            }
        }
    }
}
