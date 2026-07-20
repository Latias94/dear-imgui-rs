use std::ffi::CString;
use std::ptr;
use std::rc::Rc;

use crate::clipboard::ClipboardContext;
use crate::fonts::SharedFontAtlas;
use crate::io::Io;
use crate::sys;

use super::attachment::{
    AttachmentRegistry, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentPhase, ContextAttachmentRole, run_post_destroy, run_pre_destroy_phase,
};
use super::binding::{
    CTX_MUTEX, ContextAliveToken, ContextBinding, ContextId, ContextState, RawBoundContextGuard,
    no_current_context, set_current_context, with_bound_context,
};
use super::snapshot_hub::SnapshotHub;
use super::texture_registry::{ManagedTextureRegistry, SharedTextureRegistry};

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
    pub(in crate::context) ini_filename: Option<CString>,
    pub(in crate::context) log_filename: Option<CString>,
    pub(in crate::context) platform_name: Option<CString>,
    pub(in crate::context) renderer_name: Option<CString>,
    // Boxed so the raw PlatformIO user-data pointer remains stable.
    // Interior mutability and reentrancy guarding live inside ClipboardContext.
    pub(in crate::context) clipboard_ctx: Box<ClipboardContext>,
    pub(in crate::context) ui: crate::ui::Ui,
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

    /// Returns the process-unique identity of this Context.
    pub fn id(&self) -> ContextId {
        self.state.id()
    }

    /// Returns a persistent capability for calling against this Context while it is alive.
    pub fn binding(&self) -> ContextBinding {
        ContextBinding::new(&self.state)
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
        shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Context> {
        let _guard = CTX_MUTEX.lock();

        if !no_current_context() {
            return Err(crate::error::ImGuiError::ContextAlreadyActive);
        }

        let shared_font_atlas_ptr = match &shared_font_atlas {
            Some(atlas) => atlas.as_ptr(),
            None => ptr::null_mut(),
        };

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

        // Set it as the current context
        set_current_context(raw);

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

        Ok(Context {
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
        let _lock = CTX_MUTEX.lock();
        if self.raw.is_null() {
            self.state.mark_native_destroyed();
            return;
        }

        self.state.begin_drop();
        self.snapshot_hub.close();
        let attachment_controls = self.attachments.begin_teardown();
        let context_id = self.state.id();
        let raw = self.raw;
        let _bound = RawBoundContextGuard::bind(raw);

        // End the native frame while backend callbacks and attachment state are still live.
        // EndFrame may update viewport bookkeeping, so quiescing backends first would make an
        // otherwise recoverable dropped FrameToken depend on torn-down callback state.
        unsafe {
            let _ = crate::list_clipper::forget_context_clippers(raw);
            if (*raw).WithinFrameScope {
                sys::igEndFrame();
            }
        }

        run_pre_destroy_phase(
            &attachment_controls,
            &self.state,
            ContextAttachmentPhase::Quiesce,
        );
        run_pre_destroy_phase(
            &attachment_controls,
            &self.state,
            ContextAttachmentPhase::RendererResources,
        );
        run_pre_destroy_phase(
            &attachment_controls,
            &self.state,
            ContextAttachmentPhase::PlatformWindows,
        );

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
            self.texture_registry.borrow_mut().teardown();
            with_bound_context(raw, || {
                crate::platform_io::clear_aggregate_callbacks_for_current_context();
            });
            #[cfg(feature = "stack-layout")]
            sys::ImGuiStack_DestroyContextState(raw);
            crate::fonts::unregister_font_atlas_context(font_atlas, raw);
            if let Some(shared_font_atlas) = &self.shared_font_atlas {
                with_bound_context(raw, || {
                    shared_font_atlas.unregister_from_current_context();
                });
            }
            sys::igDestroyContext(raw);
            // Native context destruction may invoke typed destroy callbacks, so their registry
            // entries must outlive `igDestroyContext` itself.
            crate::platform_io::clear_typed_callbacks_for_context(raw);
            crate::fonts::forget_font_atlas_generation(owned_font_atlas);
        }

        self.raw = ptr::null_mut();
        self.state.mark_native_destroyed();
        run_post_destroy(attachment_controls, context_id);
    }
}
