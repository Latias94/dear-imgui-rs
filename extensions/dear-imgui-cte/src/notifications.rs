use crate::{
    CteError, CteResult,
    context::CteContextBinding,
    error::c_string,
    sys,
    validation::{duration_millis_i32, validate_finite_vec2},
};
use dear_imgui_rs::{Context, ContextId, Ui};
use std::{marker::PhantomData, ptr::NonNull, rc::Rc, time::Duration};

/// Visual category for a notification.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum NotificationType {
    Success,
    Warning,
    Error,
    Info,
}

impl NotificationType {
    const fn into_raw(self) -> sys::Type {
        match self {
            Self::Success => sys::success,
            Self::Warning => sys::warning,
            Self::Error => sys::error,
            Self::Info => sys::info,
        }
    }
}

/// An owned, context-bound notification queue.
///
/// The queue is intentionally neither [`Send`] nor [`Sync`]. Rendering advances
/// notification timing and removes expired entries.
pub struct Notifications {
    raw: NonNull<sys::Notifications>,
    binding: CteContextBinding,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl Notifications {
    /// Creates a notification queue bound to `context`.
    pub fn try_create(context: &Context) -> CteResult<Self> {
        let binding = CteContextBinding::new(context);
        let raw = binding.try_with_bound_context("Notifications::try_create", || unsafe {
            sys::Notifications_Notifications()
        })?;
        let raw = NonNull::new(raw).ok_or(CteError::CreationFailed {
            object: "Notifications",
        })?;
        Ok(Self {
            raw,
            binding,
            _not_send_sync: PhantomData,
        })
    }

    /// Creates a notification queue and panics if native allocation fails.
    pub fn create(context: &Context) -> Self {
        Self::try_create(context).expect("failed to create cimCTE Notifications")
    }

    /// Returns the stable identity of the owning Dear ImGui context.
    pub fn context_id(&self) -> ContextId {
        self.binding.id()
    }

    /// Returns the raw notification-queue pointer.
    ///
    /// # Safety
    ///
    /// The pointer may only be used while the owning Dear ImGui context is current. The
    /// caller must preserve every cimCTE precondition, ownership and pointer-lifetime rule,
    /// and invariant relied on by this safe wrapper. In particular, the caller must not
    /// destroy the queue or pass invalid pointers through the raw API.
    pub unsafe fn as_raw(&self) -> *mut sys::Notifications {
        self.raw.as_ptr()
    }

    /// Adds a notification whose steady-state display lasts for `dismiss_after`.
    pub fn add(
        &mut self,
        kind: NotificationType,
        message: &str,
        dismiss_after: Duration,
    ) -> CteResult<()> {
        const OPERATION: &str = "Notifications::add";
        let dismiss_ms = duration_millis_i32(OPERATION, "dismiss_after", dismiss_after)?;
        let message = c_string(OPERATION, message)?;
        self.with_context(OPERATION, |raw| unsafe {
            sys::Notifications_Add(raw, kind.into_raw(), message.as_ptr(), dismiss_ms)
        });
        Ok(())
    }

    fn with_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::Notifications) -> R,
    ) -> R {
        self.try_with_context(operation, f)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn try_with_context<R>(
        &self,
        operation: &'static str,
        f: impl FnOnce(*mut sys::Notifications) -> R,
    ) -> CteResult<R> {
        let raw = self.raw.as_ptr();
        self.binding.try_with_bound_context(operation, || f(raw))
    }
}

impl Drop for Notifications {
    fn drop(&mut self) {
        let raw = self.raw;
        let _ = self
            .binding
            .try_with_bound_context("Notifications::drop", || unsafe {
                sys::Notifications_destroy(raw.as_ptr())
            });
        // If context teardown already started, touching CTE state is no longer proven safe.
        // The native handle is intentionally leaked rather than calling into a dead context.
    }
}

/// Builder for one notification-stack render submission.
#[must_use = "call build() to render the notification stack"]
pub struct NotificationsRenderer<'ui, 'notifications> {
    ui: &'ui Ui,
    notifications: &'notifications mut Notifications,
    position: Option<[f32; 2]>,
}

impl NotificationsRenderer<'_, '_> {
    pub(crate) fn new<'ui, 'notifications>(
        ui: &'ui Ui,
        notifications: &'notifications mut Notifications,
    ) -> NotificationsRenderer<'ui, 'notifications> {
        NotificationsRenderer {
            ui,
            notifications,
            position: None,
        }
    }

    /// Sets the bottom-right stack position in viewport coordinates.
    pub fn position(mut self, position: [f32; 2]) -> Self {
        self.position = Some(position);
        self
    }

    /// Renders the stack, defaulting to the main viewport's work-area bottom-right corner.
    pub fn build(self) -> CteResult<()> {
        const OPERATION: &str = "NotificationsRenderer::build";
        self.notifications.binding.require_ui(OPERATION, self.ui)?;
        let position = self.position.unwrap_or_else(|| {
            const MARGIN: f32 = 20.0;
            let viewport = self.ui.main_viewport();
            let origin = viewport.work_pos();
            let size = viewport.work_size();
            [origin[0] + size[0] - MARGIN, origin[1] + size[1] - MARGIN]
        });
        validate_finite_vec2(OPERATION, "position", position)?;
        self.notifications
            .try_with_context(OPERATION, |raw| unsafe {
                sys::Notifications_Render(raw, position.into())
            })
    }
}
