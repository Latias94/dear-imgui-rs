use crate::Ui;
use crate::scope::{NativeScopePop, NativeScopeToken};

// ============================================================================
// Disabled scope (RAII)
// ============================================================================

/// Tracks a disabled scope begun with [`Ui::begin_disabled`] and ended on drop.
///
/// Disabled scopes share Dear ImGui's item-flag stack with [`Ui::push_item_flag`]. A scope that
/// transitions into the disabled state also shares `Style.Alpha` restoration order with
/// [`crate::StyleVar::Alpha`] tokens. Tokens on either affected stack must finish in reverse
/// creation order and in their originating window `Begin` scope. Prefer [`Ui::with_disabled`] or
/// [`Ui::with_disabled_if`] when a closure expresses the intended lifetime.
#[must_use]
#[doc(alias = "EndDisabled")]
pub struct DisabledToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> DisabledToken<'ui> {
    fn new(ui: &'ui Ui, restores_alpha: bool) -> Self {
        Self {
            scope: ui.begin_native_scope(
                NativeScopePop::EndDisabled { restores_alpha },
                "DisabledToken",
            ),
        }
    }

    /// Ends the disabled scope explicitly.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later item-flag or disabled token is active, an Alpha style token
    /// depends on this scope's saved state, or this token is outside its originating window
    /// `Begin` scope.
    pub fn end(self) {
        // Drop will call EndDisabled
    }
}

impl<'ui> Drop for DisabledToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

impl Ui {
    /// Begin a disabled scope for subsequent items.
    ///
    /// All following widgets will be disabled (grayed out and non-interactive)
    /// until the returned token is dropped.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled(&self) -> DisabledToken<'_> {
        self.begin_disabled_with_cond(true)
    }

    /// Begin a conditionally disabled scope for subsequent items.
    ///
    /// If `disabled` is false, this still needs to be paired with the returned
    /// token being dropped to correctly balance the internal stack.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled_with_cond(&self, disabled: bool) -> DisabledToken<'_> {
        let restores_alpha = self.run_with_bound_context(|| unsafe {
            let context = self.context_raw();
            let was_disabled =
                (*context).CurrentItemFlags & sys::ImGuiItemFlags_Disabled as i32 != 0;
            sys::igBeginDisabled(disabled);
            disabled && !was_disabled
        });
        DisabledToken::new(self, restores_alpha)
    }

    /// Runs `f` while subsequent items are disabled.
    ///
    /// The disabled scope is ended before a successful closure result is returned and during
    /// unwinding if `f` panics.
    #[doc(alias = "BeginDisabled", alias = "EndDisabled")]
    pub fn with_disabled<R>(&self, f: impl FnOnce() -> R) -> R {
        self.with_disabled_if(true, f)
    }

    /// Runs `f` inside a conditionally disabled scope.
    ///
    /// Dear ImGui requires a balanced `BeginDisabled`/`EndDisabled` pair even when `disabled` is
    /// false. This helper preserves that balance before returning or during unwinding.
    #[doc(alias = "BeginDisabled", alias = "EndDisabled")]
    pub fn with_disabled_if<R>(&self, disabled: bool, f: impl FnOnce() -> R) -> R {
        let token = self.begin_disabled_with_cond(disabled);
        let result = f();
        drop(token);
        result
    }
}
