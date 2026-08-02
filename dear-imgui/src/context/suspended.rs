use std::panic::{self, AssertUnwindSafe};
use std::ptr;

use crate::clipboard::ClipboardContext;
use crate::fonts::SharedFontAtlas;
use crate::sys;

use super::Context;
use super::attachment::AttachmentRegistry;
use super::binding::{
    CTX_MUTEX, ContextBinding, ContextId, ContextState, ContextThreadLease,
    bound_context_scope_active, clear_current_context, no_current_context, set_current_context,
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
    /// Returns the process-unique identity of this Context.
    pub fn id(&self) -> ContextId {
        self.0.id()
    }

    /// Runs a closure while this suspended Context is active.
    ///
    /// Any previously current Context is restored before this method returns. An open frame left
    /// behind when the closure returns `Err` or panics is ended before propagating that outcome.
    ///
    /// # Panics
    ///
    /// Resumes any panic raised by the closure with its original payload. This method also panics
    /// after ending the frame if the closure returns `Ok` while a Dear ImGui frame is still open.
    pub fn try_with_active<T, E>(
        &mut self,
        f: impl FnOnce(&mut Context) -> Result<T, E>,
    ) -> Result<T, E> {
        let expected_id = self.0.id();
        let expected_raw = self.0.raw;
        let binding = self.0.binding();
        binding
            .try_with_bound_context_guarded(|bound| {
                let result = panic::catch_unwind(AssertUnwindSafe(|| f(&mut self.0)));

                if self.0.id() != expected_id || self.0.raw != expected_raw {
                    if !bound.previous_context().is_null()
                        && self.0.raw == bound.previous_context()
                        && bound.previous_context() != expected_raw
                    {
                        bound.restore_bound_target_or_clear();
                    }
                    if let Err(payload) = result {
                        panic::resume_unwind(payload);
                    }
                    panic!(
                        "SuspendedContext::try_with_active(): closure moved or replaced the Context owner"
                    );
                }

                match result {
                    Ok(Ok(value)) => {
                        if self.0.end_frame_for_teardown_unlocked() {
                            panic!(
                                "SuspendedContext::try_with_active(): closure returned Ok while a Dear ImGui frame was still open"
                            );
                        }
                        Ok(value)
                    }
                    Ok(Err(error)) => {
                        self.0.end_frame_for_teardown_unlocked();
                        Err(error)
                    }
                    Err(payload) => {
                        // Cleanup must not replace the closure's panic payload.
                        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
                            self.0.end_frame_for_teardown_unlocked();
                        }));
                        panic::resume_unwind(payload)
                    }
                }
            })
            .unwrap_or_else(|error| panic!("SuspendedContext::try_with_active(): {error}"))
    }

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
        if bound_context_scope_active() {
            return Err(crate::error::ImGuiError::ContextBindingScopeActive);
        }
        let thread_lease = ContextThreadLease::acquire()?;
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
            _thread_lease: thread_lease,
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
