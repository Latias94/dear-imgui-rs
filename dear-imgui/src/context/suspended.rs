use std::ptr;

use crate::clipboard::ClipboardContext;
use crate::fonts::SharedFontAtlas;
use crate::sys;

use super::Context;
use super::attachment::AttachmentRegistry;
use super::binding::{
    CTX_MUTEX, ContextBinding, ContextId, ContextState, clear_current_context, no_current_context,
    set_current_context,
};
use super::frame::FrameLifecycleState;
use super::snapshot_hub::SnapshotHub;
use super::texture_registry::ManagedTextureRegistry;

impl Context {
    /// Suspends this context so another context can be the active context
    pub fn suspend(self) -> SuspendedContext {
        let _guard = CTX_MUTEX.lock();
        assert!(
            self.is_current_context(),
            "context to be suspended is not the active context"
        );
        assert_ne!(
            self.frame_lifecycle_state_unlocked(),
            FrameLifecycleState::InFrame,
            "cannot suspend a context while a Dear ImGui frame is open"
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

    /// Tries to create a new suspended Dear ImGui context with a shared font atlas.
    ///
    /// Multiple contexts may share the atlas while using legacy renderer-managed texture handling.
    /// Once a managed renderer claims the atlas, registering another context returns
    /// [`ImGuiError::SharedFontAtlasManaged`](crate::ImGuiError::SharedFontAtlasManaged).
    /// If its prior managed Context was dropped without a committed renderer reset, this returns
    /// [`ImGuiError::SharedFontAtlasRendererReleasePending`](crate::ImGuiError::SharedFontAtlasRendererReleasePending).
    pub fn try_create_with_shared_font_atlas(
        shared_font_atlas: SharedFontAtlas,
    ) -> crate::error::ImGuiResult<Self> {
        Self::try_create_internal(Some(shared_font_atlas))
    }

    /// Creates a new suspended Dear ImGui context (panics on error)
    pub fn create() -> Self {
        Self::try_create().expect("Failed to create Dear ImGui context")
    }

    /// Creates a new suspended Dear ImGui context with a shared font atlas (panics on error).
    ///
    /// This panics if a managed renderer has already claimed the atlas. Use
    /// [`SuspendedContext::try_create_with_shared_font_atlas`] to handle ownership and
    /// pending-release errors.
    pub fn create_with_shared_font_atlas(shared_font_atlas: SharedFontAtlas) -> Self {
        Self::try_create_with_shared_font_atlas(shared_font_atlas)
            .expect("Failed to create Dear ImGui context")
    }

    // removed legacy create_or_panic variants (use create()/try_create())

    fn try_create_internal(
        shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Self> {
        let _guard = CTX_MUTEX.lock();
        let previous_context = unsafe { sys::igGetCurrentContext() };

        let shared_font_atlas_ptr = match &shared_font_atlas {
            Some(atlas) => atlas.as_ptr(),
            None => ptr::null_mut(),
        };
        crate::fonts::validate_font_atlas_context_registration(shared_font_atlas_ptr)?;

        let id =
            ContextId::allocate().ok_or_else(|| crate::error::ImGuiError::ContextCreation {
                reason: "process Context identity space is exhausted".to_string(),
            })?;

        let raw = unsafe { sys::igCreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            set_current_context(previous_context);
            return Err(crate::error::ImGuiError::ContextCreation {
                reason: "ImGui_CreateContext returned null".to_string(),
            });
        }

        unsafe {
            let io = sys::igGetIO_ContextPtr(raw);
            assert!(
                !io.is_null(),
                "new ImGui context returned a null IO pointer"
            );
            crate::fonts::register_font_atlas_context((*io).Fonts, raw);
        }

        let state = ContextState::new(id, raw);
        let texture_registry = ManagedTextureRegistry::new(id);
        let ui = crate::ui::Ui::new(raw, ContextBinding::new(&state), texture_registry.clone());

        let ctx = Context {
            raw,
            state,
            attachments: AttachmentRegistry::default(),
            snapshot_hub: SnapshotHub::new(id),
            texture_registry,
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(ClipboardContext::dummy()),
            ui,
        };

        if previous_context.is_null() {
            clear_current_context();
        } else {
            set_current_context(previous_context);
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
            set_current_context(self.0.raw);
            Ok(self.0)
        } else {
            Err(self)
        }
    }
}
