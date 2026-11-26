use std::ptr;
use std::rc::Rc;

use crate::clipboard::ClipboardContext;
use crate::ffi::{RawBox, RawCString};
use crate::fonts::SharedFontAtlas;
use crate::io::Io;
use crate::sys;

use super::attachment::{
    AttachmentRegistry, ContextAttachment, ContextAttachmentError, ContextAttachmentHandle,
    ContextAttachmentLease, ContextAttachmentPhase, ContextAttachmentRole,
    ContextPlatformAttachmentRelease, ContextPlatformAttachmentReleaseError, run_post_destroy,
    run_pre_destroy_phase,
};
use super::binding::{
    CTX_MUTEX, ContextAliveToken, ContextBinding, ContextId, ContextState, ContextThreadLease,
    RawBoundContextGuard, bound_context_scope_active, set_current_context, with_bound_context,
};
use super::snapshot_hub::SnapshotHub;
use super::texture_registry::{ManagedTextureRegistry, SharedTextureRegistry};
use super::{ContextOperationError, ContextScopeError, FrameLifecycleState};

/// An imgui context.
///
/// Multiple contexts can coexist on the same OS thread. Each owns its frames and input.
/// Safe operations bind the owning context as needed. Use [`Self::binding`] to scope raw
/// integrations that read Dear ImGui's global current context.
///
/// ```
/// let first = dear_imgui_rs::Context::create();
/// let second = dear_imgui_rs::Context::create();
/// assert_ne!(first.id(), second.id());
/// ```
#[doc(
    alias = "CreateContext",
    alias = "DestroyContext",
    alias = "GetCurrentContext",
    alias = "SetCurrentContext"
)]
#[derive(Debug)]
pub struct Context {
    pub(super) raw: *mut sys::ImGuiContext,
    pub(super) state: Rc<ContextState>,
    pub(super) attachments: AttachmentRegistry,
    pub(super) snapshot_hub: SnapshotHub,
    pub(crate) texture_registry: SharedTextureRegistry,
    pub(in crate::context) shared_font_atlas: Option<SharedFontAtlas>,
    pub(in crate::context) ini_filename: Option<RawCString>,
    pub(in crate::context) log_filename: Option<RawCString>,
    pub(in crate::context) platform_name: Option<RawCString>,
    pub(in crate::context) renderer_name: Option<RawCString>,
    pub(in crate::context) clipboard_ctx: RawBox<ClipboardContext>,
    pub(in crate::context) ui: crate::ui::Ui,
    // Keep process-global GImGui ownership until every Context-owned Rust field is gone.
    pub(super) _thread_lease: ContextThreadLease,
}

impl Context {
    /// Tries to create a new Dear ImGui context on the owner thread.
    ///
    /// Existing contexts on this thread remain usable.
    pub fn try_create() -> crate::error::ImGuiResult<Context> {
        Self::try_create_internal(None)
    }

    /// Tries to create a new Dear ImGui context with a shared font atlas.
    ///
    /// Multiple contexts may share the atlas while using legacy renderer-managed texture handling.
    /// Once a managed renderer claims the atlas, registering another context returns
    /// [`ImGuiError::SharedFontAtlasManaged`](crate::ImGuiError::SharedFontAtlasManaged).
    /// If its prior managed Context was dropped without a committed renderer reset, this returns
    /// [`ImGuiError::SharedFontAtlasRendererReleasePending`](crate::ImGuiError::SharedFontAtlasRendererReleasePending).
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
    ///
    /// This panics if a managed renderer has already claimed the atlas. Use
    /// [`Context::try_create_with_shared_font_atlas`] to handle ownership and pending-release
    /// errors.
    pub fn create_with_shared_font_atlas(shared_font_atlas: SharedFontAtlas) -> Context {
        Self::try_create_with_shared_font_atlas(shared_font_atlas)
            .expect("Failed to create Dear ImGui context")
    }

    /// Returns the raw `ImGuiContext*` for FFI integrations.
    pub fn as_raw(&self) -> *mut sys::ImGuiContext {
        self.raw
    }

    /// Returns the process-unique identity of this Context.
    pub fn id(&self) -> ContextId {
        self.state.id()
    }

    /// Returns a persistent capability for calling against this Context while it is alive.
    pub fn binding(&self) -> ContextBinding {
        ContextBinding::new(&self.state)
    }

    /// Runs an operation with this context current, then restores the prior binding.
    ///
    /// The operation must start and finish without an open frame. If it returns an error or
    /// panics, a frame it left open is ended before the outcome propagates.
    ///
    /// # Panics
    ///
    /// Propagates the closure's panic. Replacing the Context owner inside the closure is not
    /// permitted.
    pub fn try_with_current<T, E>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, ContextOperationError<E>> {
        if self.frame_lifecycle_state_unlocked() == FrameLifecycleState::InFrame {
            return Err(ContextScopeError::FrameAlreadyOpen.into());
        }
        let id = self.id();
        self.binding()
            .try_with_bound_context(|| {
                let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    operation(self)
                })) {
                    Ok(result) => result,
                    Err(payload) => {
                        if self.id() == id {
                            let cleanup =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    self.end_frame_for_teardown_unlocked();
                                }));
                            if let Err(cleanup_panic) = cleanup {
                                std::mem::forget(cleanup_panic);
                            }
                        }
                        std::panic::resume_unwind(payload);
                    }
                };
                assert_eq!(self.id(), id, "the Context operation replaced its owner");
                let ended_frame = self.end_frame_for_teardown_unlocked();
                match result {
                    Ok(_) if ended_frame => Err(ContextScopeError::FrameLeftOpen.into()),
                    Ok(value) => Ok(value),
                    Err(error) => Err(ContextOperationError::Closure(error)),
                }
            })
            .map_err(ContextScopeError::ContextUnavailable)?
    }

    /// Returns a token that can be used to check whether this context is still alive.
    ///
    /// Useful for extension crates that store raw pointers and need to avoid calling into FFI
    /// after the owning `Context` has been dropped.
    pub fn alive_token(&self) -> ContextAliveToken {
        ContextAliveToken::from_binding(self.binding())
    }

    /// Registers a typed lifecycle attachment owned by this Context.
    ///
    /// The marker type identifies the attachment independently of its erased implementation.
    /// Platform and renderer roles are exclusive, and a renderer requires an active platform.
    pub fn register_attachment<Marker: 'static>(
        &mut self,
        role: ContextAttachmentRole,
        attachment: Rc<dyn ContextAttachment>,
    ) -> Result<ContextAttachmentLease, ContextAttachmentError> {
        self.attachments
            .register::<Marker>(self.state.lifecycle(), role, attachment)
    }

    /// Validate a typed lifecycle attachment registration without mutating the registry.
    ///
    /// Backends use this to preflight a multi-Context transaction before registering any
    /// attachment. A successful result remains valid until the attachment registry or Context
    /// lifecycle changes.
    pub fn preflight_attachment_registration<Marker: 'static>(
        &self,
        role: ContextAttachmentRole,
    ) -> Result<(), ContextAttachmentError> {
        self.attachments
            .preflight_register::<Marker>(self.state.lifecycle(), role)
    }

    /// Prepares an explicit release of this Context's exact platform attachment generation.
    ///
    /// Preparation fails while a renderer attachment is active. No frame, callback, or native
    /// state is changed on failure. The returned permit exclusively borrows this Context; perform
    /// any fallible platform cleanup through [`ContextPlatformAttachmentRelease::context_mut`]
    /// and call [`ContextPlatformAttachmentRelease::commit`] only after native cleanup succeeds.
    pub fn prepare_platform_attachment_release(
        &mut self,
        handle: &ContextAttachmentHandle,
    ) -> Result<ContextPlatformAttachmentRelease<'_>, ContextPlatformAttachmentReleaseError> {
        let control = self.attachments.prepare_platform_release(handle)?;
        Ok(ContextPlatformAttachmentRelease::new(self, control))
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

    fn try_create_internal(
        shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Context> {
        let thread_lease = ContextThreadLease::acquire()?;
        let _guard = CTX_MUTEX.lock();

        let shared_font_atlas_ptr = match &shared_font_atlas {
            Some(atlas) => atlas.as_ptr(),
            None => ptr::null_mut(),
        };
        crate::fonts::validate_font_atlas_context_registration(shared_font_atlas_ptr)?;

        let id =
            ContextId::allocate().ok_or_else(|| crate::error::ImGuiError::ContextCreation {
                reason: "process Context identity space is exhausted".to_string(),
            })?;

        // Create the actual ImGui context
        let raw = unsafe { sys::igCreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            return Err(crate::error::ImGuiError::ContextCreation {
                reason: "ImGui_CreateContext returned null".to_string(),
            });
        }

        if !bound_context_scope_active() {
            let current = unsafe { sys::igGetCurrentContext() };
            if current.is_null() || !unsafe { (*current).WithinFrameScope } {
                set_current_context(raw);
            }
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
        let ui = crate::ui::Ui::new(
            raw,
            ContextBinding::new(&state),
            texture_registry.clone(),
            state.ime.clone(),
        );

        Ok(Context {
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
            clipboard_ctx: RawBox::new(ClipboardContext::dummy()),
            ui,
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
}

impl Drop for Context {
    fn drop(&mut self) {
        let _lock = CTX_MUTEX.lock();
        if self.raw.is_null() {
            self.state.mark_native_destroyed();
            return;
        }

        self.state.begin_drop();
        let attachment_controls = self.attachments.begin_teardown();
        let context_id = self.state.id();
        let raw = self.raw;
        let _bound = RawBoundContextGuard::bind(raw);

        // End the native frame while backend callbacks and attachment state are still live.
        // EndFrame may update viewport bookkeeping, so quiescing backends first would make an
        // otherwise recoverable dropped FrameToken depend on torn-down callback state.
        self.end_frame_for_teardown_unlocked();

        for phase in [
            ContextAttachmentPhase::Quiesce,
            ContextAttachmentPhase::RendererResources,
            ContextAttachmentPhase::PlatformWindows,
        ] {
            if !run_pre_destroy_phase(&attachment_controls, self, phase) {
                // Continuing would destroy resources whose preceding teardown phase failed. We
                // cannot unwind safely either: native Context fields still borrow Rust storage.
                std::process::abort();
            }
            if phase == ContextAttachmentPhase::RendererResources {
                // Renderer attachments must still see their active consumer and any detached
                // epoch while proving that a texture reset is safe. After that phase no native
                // renderer resource may observe a completion, so close the hub before platform
                // windows and native Context state begin disappearing.
                self.snapshot_hub.close();
            }
        }

        unsafe {
            let io = sys::igGetIO_ContextPtr(raw);
            let font_atlas = if io.is_null() {
                std::ptr::null_mut()
            } else {
                (*io).Fonts
            };
            let owned_font_atlas = if self.shared_font_atlas.is_none() {
                font_atlas
            } else {
                std::ptr::null_mut()
            };
            self.texture_registry.borrow_mut().prepare_teardown();
            with_bound_context(raw, || {
                crate::platform_io::clear_aggregate_callbacks_for_current_context();
            });
            #[cfg(feature = "stack-layout")]
            sys::ImGuiStack_DestroyContextState(raw);
            crate::fonts::unregister_font_atlas_context(font_atlas, raw, context_id);
            if let Some(shared_font_atlas) = &self.shared_font_atlas {
                with_bound_context(raw, || {
                    shared_font_atlas.unregister_from_current_context();
                });
            }
            sys::igDestroyContext(raw);
            self.texture_registry
                .borrow_mut()
                .release_after_native_destroy();
            crate::fonts::forget_font_atlas_generation(owned_font_atlas);
        }

        self.raw = ptr::null_mut();
        self.state.mark_native_destroyed();
        if !run_post_destroy(attachment_controls, context_id) {
            std::process::abort();
        }
    }
}
