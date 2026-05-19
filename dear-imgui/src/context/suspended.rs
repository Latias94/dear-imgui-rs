use std::ptr;
use std::rc::Rc;

use crate::clipboard::ClipboardContext;
use crate::fonts::SharedFontAtlas;
use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, clear_current_context, no_current_context};

impl Context {
    /// Suspends this context so another context can be the active context
    pub fn suspend(self) -> SuspendedContext {
        let _guard = CTX_MUTEX.lock();
        assert!(
            self.is_current_context(),
            "context to be suspended is not the active context"
        );
        clear_current_context();
        SuspendedContext(self)
    }
}

/// A suspended Dear ImGui context
///
/// A suspended context retains its state, but is not usable without activating it first.
#[derive(Debug)]
pub struct SuspendedContext(pub(super) Context);

impl SuspendedContext {
    /// Tries to create a new suspended Dear ImGui context
    pub fn try_create() -> crate::error::ImGuiResult<Self> {
        Self::try_create_internal(None)
    }

    /// Tries to create a new suspended Dear ImGui context with a shared font atlas
    pub fn try_create_with_shared_font_atlas(
        shared_font_atlas: SharedFontAtlas,
    ) -> crate::error::ImGuiResult<Self> {
        Self::try_create_internal(Some(shared_font_atlas))
    }

    /// Creates a new suspended Dear ImGui context (panics on error)
    pub fn create() -> Self {
        Self::try_create().expect("Failed to create Dear ImGui context")
    }

    /// Creates a new suspended Dear ImGui context with a shared font atlas (panics on error)
    pub fn create_with_shared_font_atlas(shared_font_atlas: SharedFontAtlas) -> Self {
        Self::try_create_with_shared_font_atlas(shared_font_atlas)
            .expect("Failed to create Dear ImGui context")
    }

    // removed legacy create_or_panic variants (use create()/try_create())

    fn try_create_internal(
        mut shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Self> {
        let _guard = CTX_MUTEX.lock();

        let shared_font_atlas_ptr = match &mut shared_font_atlas {
            Some(atlas) => atlas.as_ptr_mut(),
            None => ptr::null_mut(),
        };

        let raw = unsafe { sys::igCreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            return Err(crate::error::ImGuiError::ContextCreation {
                reason: "ImGui_CreateContext returned null".to_string(),
            });
        }

        let ctx = Context {
            raw,
            alive: Rc::new(()),
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(ClipboardContext::dummy()),
            ui: crate::ui::Ui::new(),
        };

        // If the context was activated during creation, deactivate it
        if ctx.is_current_context() {
            clear_current_context();
        }

        Ok(SuspendedContext(ctx))
    }

    /// Attempts to activate this suspended context
    ///
    /// If there is no active context, this suspended context is activated and `Ok` is returned.
    /// If there is already an active context, nothing happens and `Err` is returned.
    pub fn activate(self) -> Result<Context, SuspendedContext> {
        let _guard = CTX_MUTEX.lock();
        if no_current_context() {
            unsafe {
                sys::igSetCurrentContext(self.0.raw);
            }
            Ok(self.0)
        } else {
            Err(self)
        }
    }
}
