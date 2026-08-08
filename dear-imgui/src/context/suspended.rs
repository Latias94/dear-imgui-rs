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
use super::{
    ContextActivationError, ContextActivationReason, ContextScopeError, ContextSuspensionError,
    ContextSuspensionReason, ScopedActivationError,
};

impl Context {
    /// Suspends this Context so another Context can become active.
    ///
    /// Rejection retains this Context in [`ContextSuspensionError`], allowing the caller to end an
    /// open frame, leave a binding scope, or otherwise repair the conflict and retry.
    pub fn suspend(self) -> Result<SuspendedContext, ContextSuspensionError> {
        let _guard = CTX_MUTEX.lock();
        if bound_context_scope_active() {
            return Err(ContextSuspensionError::new(
                self,
                ContextSuspensionReason::BindingScopeActive,
            ));
        }
        if !self.is_current_context() {
            return Err(ContextSuspensionError::new(
                self,
                ContextSuspensionReason::NotCurrent,
            ));
        }
        if self.frame_lifecycle_state_unlocked() == FrameLifecycleState::InFrame {
            return Err(ContextSuspensionError::new(
                self,
                ContextSuspensionReason::FrameOpen,
            ));
        }
        clear_current_context();
        Ok(SuspendedContext(self))
    }

    /// Suspends this Context or panics with the rejection reason.
    ///
    /// # Panics
    ///
    /// Panics if a Context binding scope is active, this Context is not current, or a frame is
    /// still open. Use [`Context::suspend`] when any of those states is recoverable.
    pub fn suspend_or_panic(self) -> SuspendedContext {
        self.suspend()
            .unwrap_or_else(|error| panic!("Context::suspend_or_panic(): {error}"))
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
    /// No other Context or Context binding scope may be active. This makes the closure's
    /// `&mut Context` the only safe live Context owner in the process, so it cannot be exchanged
    /// with another owner while native `GImGui` points at it. An open frame left behind when the
    /// closure returns `Err` or panics is ended before propagating that outcome.
    ///
    /// Admission conflicts and a successful closure that leaves a frame open are returned as
    /// [`ScopedActivationError::Scope`] containing a [`ContextScopeError`]. A closure error is
    /// wrapped in [`ScopedActivationError::Closure`]. The borrowed suspended owner remains
    /// available for every returned error.
    ///
    /// # Panics
    ///
    /// Resumes a panic raised by the closure with its original payload. It also panics if the
    /// closure replaces the complete Context owner while native `GImGui` still points at the
    /// original Context; ordinary safe code should not attempt that owner exchange.
    pub fn try_with_active<T, E>(
        &mut self,
        f: impl FnOnce(&mut Context) -> Result<T, E>,
    ) -> Result<T, ScopedActivationError<E>> {
        let _guard = CTX_MUTEX.lock();
        if bound_context_scope_active() {
            return Err(
                ContextScopeError::Activation(ContextActivationReason::BindingScopeActive).into(),
            );
        }
        if !no_current_context() {
            return Err(ContextScopeError::Activation(
                ContextActivationReason::ContextAlreadyActive,
            )
            .into());
        }
        let expected_id = self.0.id();
        let expected_raw = self.0.raw;
        let binding = self.0.binding();
        binding
            .try_with_bound_context_guarded(|bound| {
                let result = panic::catch_unwind(AssertUnwindSafe(|| f(&mut self.0)));

                debug_assert!(bound.previous_context().is_null());
                if self.0.id() != expected_id || self.0.raw != expected_raw {
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
                            return Err(ContextScopeError::FrameLeftOpen.into());
                        }
                        Ok(value)
                    }
                    Ok(Err(error)) => {
                        self.0.end_frame_for_teardown_unlocked();
                        Err(ScopedActivationError::Closure(error))
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
            .map_err(|error| {
                ScopedActivationError::Scope(ContextScopeError::ContextUnavailable(error))
            })?
    }

    /// Runs a closure while this suspended Context is active, panicking on scope errors.
    ///
    /// # Panics
    ///
    /// Panics if another Context or binding scope is active, if the closure leaves a frame open,
    /// if the Context cannot be bound, or if the closure itself panics.
    pub fn with_active_or_panic<T>(&mut self, f: impl FnOnce(&mut Context) -> T) -> T {
        self.try_with_active(|context| Ok::<_, std::convert::Infallible>(f(context)))
            .unwrap_or_else(|error| match error {
                ScopedActivationError::Closure(never) => match never {},
                ScopedActivationError::Scope(error) => {
                    panic!("SuspendedContext::with_active_or_panic(): {error}")
                }
            })
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

    /// Attempts to activate this suspended Context.
    ///
    /// If activation is rejected, [`ContextActivationError`] retains this suspended owner and
    /// reports whether another Context or a binding scope blocked activation.
    pub fn activate(self) -> Result<Context, ContextActivationError> {
        let _guard = CTX_MUTEX.lock();
        if bound_context_scope_active() {
            return Err(ContextActivationError::new(
                self,
                ContextActivationReason::BindingScopeActive,
            ));
        }
        if !no_current_context() {
            return Err(ContextActivationError::new(
                self,
                ContextActivationReason::ContextAlreadyActive,
            ));
        }
        set_current_context(self.0.raw);
        Ok(self.0)
    }

    /// Activates this suspended Context or panics with the rejection reason.
    ///
    /// # Panics
    ///
    /// Panics if another Context or Context binding scope is active. Use
    /// [`SuspendedContext::activate`] when activation conflicts are recoverable.
    pub fn activate_or_panic(self) -> Context {
        self.activate()
            .unwrap_or_else(|error| panic!("SuspendedContext::activate_or_panic(): {error}"))
    }
}
