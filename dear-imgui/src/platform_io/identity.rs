use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{ContextId, Id};

static NEXT_NATIVE_WINDOW_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Identity of one native window registration owned by a platform backend.
///
/// This identity is independent of both the operating system's reusable window identifier and
/// Dear ImGui's mutable viewport routing ID. Backends allocate it when publishing a native window,
/// preserve it across docking changes, and retire it when releasing that registration. Reattaching
/// even the same host window after retirement creates a new instance.
///
/// The value contains no native handle and can outlive the Context and window. It is metadata,
/// not a capability to access a live window; lookups must validate the current platform owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeWindowInstanceId {
    context_id: ContextId,
    serial: NonZeroU64,
}

impl NativeWindowInstanceId {
    /// Allocates a process-unique registration identity for a platform backend.
    ///
    /// Allocation never wraps, including across backend shutdown and reattachment. An identity
    /// does not establish native ownership by itself. Custom platform backends must publish it
    /// transactionally with their own native window registration.
    ///
    /// ```
    /// use dear_imgui_rs::{Context, NativeWindowInstanceId};
    /// let context = Context::create();
    /// let first = NativeWindowInstanceId::allocate(context.id())?;
    /// let reopened = NativeWindowInstanceId::allocate(context.id())?;
    /// assert_ne!(first, reopened);
    /// assert_eq!(first.context_id(), context.id());
    /// # Ok::<(), dear_imgui_rs::NativeWindowIdentityExhausted>(())
    /// ```
    pub fn allocate(context_id: ContextId) -> Result<Self, NativeWindowIdentityExhausted> {
        let serial = allocate_serial(&NEXT_NATIVE_WINDOW_INSTANCE)?;
        Ok(Self { context_id, serial })
    }

    /// Returns the Context that owned this registration.
    pub fn context_id(self) -> ContextId {
        self.context_id
    }

    /// Returns the process-unique serial, without granting native handle access.
    pub fn get(self) -> NonZeroU64 {
        self.serial
    }
}

/// Native window registration identity space was exhausted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("native window registration identity space is exhausted")]
pub struct NativeWindowIdentityExhausted;

fn allocate_serial(counter: &AtomicU64) -> Result<NonZeroU64, NativeWindowIdentityExhausted> {
    let value = counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| NativeWindowIdentityExhausted)?;
    NonZeroU64::new(value).ok_or(NativeWindowIdentityExhausted)
}

/// Owned association between a native allocation and its viewport at observation time.
///
/// Copy this metadata into frame reports instead of resolving an old viewport ID against a newer
/// registry. The routing ID may change while `instance_id` remains the same. This snapshot retains
/// no window, Context, platform pointer, or operating-system handle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NativeWindowTarget {
    /// Stable identity of the native registration.
    pub instance_id: NativeWindowInstanceId,
    /// Dear ImGui routing ID when the association was observed.
    pub viewport_id: Id,
    /// Whether this was the Context's main viewport when observed.
    pub is_main: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_exhaustion_is_sticky_and_never_reuses_an_identity() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate_serial(&counter).unwrap().get(), u64::MAX - 1);
        assert_eq!(
            allocate_serial(&counter),
            Err(NativeWindowIdentityExhausted)
        );
        assert_eq!(
            allocate_serial(&counter),
            Err(NativeWindowIdentityExhausted)
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn registrations_remain_distinct_after_an_owner_restarts() {
        let context = ContextId::allocate().unwrap();
        let original = NativeWindowInstanceId::allocate(context).unwrap();
        let replacement = NativeWindowInstanceId::allocate(context).unwrap();
        assert_ne!(original, replacement);
        assert_eq!(original.context_id(), replacement.context_id());
        assert_ne!(original.get(), replacement.get());
    }

    #[test]
    fn targets_are_detached_and_routing_changes_preserve_the_instance() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NativeWindowTarget>();
        let context = ContextId::allocate().unwrap();
        let before = NativeWindowTarget {
            instance_id: NativeWindowInstanceId::allocate(context).unwrap(),
            viewport_id: Id::from(7),
            is_main: false,
        };
        let after = NativeWindowTarget {
            viewport_id: Id::from(9),
            ..before
        };
        assert_eq!(before.instance_id, after.instance_id);
        assert_ne!(before.viewport_id, after.viewport_id);
    }
}
