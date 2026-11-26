use super::validation::assert_finite_vec2;
use crate::sys;
use crate::{Id, Ui};
use std::ffi::c_void;

create_token!(
    /// Tracks a stack-layout horizontal group.
    ///
    /// This wraps the repository-owned stack layout compatibility shim used by
    /// the upstream imgui-node-editor blueprints example. It is not part of the
    /// official Dear ImGui API.
    pub struct HorizontalStackLayoutToken<'ui>;

    pop crate::scope::NativeScopePop::EndHorizontal;

    /// Ends the stack-layout horizontal group.
    drop { unsafe { sys::ImGuiStack_EndHorizontal() } }
);

create_token!(
    /// Tracks a stack-layout vertical group.
    ///
    /// This wraps the repository-owned stack layout compatibility shim used by
    /// the upstream imgui-node-editor blueprints example. It is not part of the
    /// official Dear ImGui API.
    pub struct VerticalStackLayoutToken<'ui>;

    pop crate::scope::NativeScopePop::EndVertical;

    /// Ends the stack-layout vertical group.
    drop { unsafe { sys::ImGuiStack_EndVertical() } }
);

create_token!(
    /// Tracks a suspended stack layout and resumes it on drop.
    pub struct StackLayoutSuspensionToken<'ui>;

    pop crate::scope::NativeScopePop::ResumeLayout;

    /// Resumes a suspended stack layout.
    drop { unsafe { sys::ImGuiStack_ResumeLayout() } }
);

/// Identifier accepted by the stack layout compatibility helpers.
///
/// The pointer form mirrors the upstream `BeginHorizontal(id.AsPointer())`
/// usage from `imgui-node-editor`; the pointer is used only as an ID and is not
/// dereferenced.
#[derive(Clone, Copy, Debug)]
pub enum StackLayoutId<'a> {
    Str(&'a str),
    Ptr(*const c_void),
    Int(i32),
    Raw(Id),
}

impl<'a> StackLayoutId<'a> {
    /// Construct an ID from a pointer value.
    #[inline]
    pub const fn ptr(ptr: *const c_void) -> Self {
        Self::Ptr(ptr)
    }

    /// Construct an ID from a pointer-sized integer, matching upstream
    /// `NodeId::AsPointer()` / `PinId::AsPointer()` usage.
    #[inline]
    pub const fn pointer_value(value: usize) -> Self {
        Self::Ptr(value as *const c_void)
    }
}

impl<'a> From<&'a str> for StackLayoutId<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}

impl From<i32> for StackLayoutId<'_> {
    #[inline]
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<Id> for StackLayoutId<'_> {
    #[inline]
    fn from(value: Id) -> Self {
        Self::Raw(value)
    }
}

impl Ui {
    fn begin_stack_layout(
        &self,
        id: StackLayoutId<'_>,
        size: [f32; 2],
        align: f32,
        caller: &'static str,
        begin: unsafe fn(sys::ImGuiID, sys::ImVec2, f32),
    ) {
        assert_finite_vec2(caller, "size", size);
        assert!(align.is_finite(), "{caller} align must be finite");
        self.run_with_bound_context(|| unsafe {
            let window = (*self.context_raw()).CurrentWindow;
            assert!(!window.is_null(), "{caller} requires a current window");
            let id = match id {
                StackLayoutId::Str(value) => self.with_scratch_buffer(|scratch| {
                    let label = scratch.scratch_txt(value);
                    sys::ImGuiWindow_GetID_Str(window, label, std::ptr::null())
                }),
                StackLayoutId::Ptr(value) => sys::ImGuiWindow_GetID_Ptr(window, value),
                StackLayoutId::Int(value) => {
                    sys::ImGuiWindow_GetID_Ptr(window, value as isize as *const c_void)
                }
                StackLayoutId::Raw(value) => value.raw(),
            };
            self.claim_stack_layout_id(window, id);
            begin(id, sys::ImVec2::from(size), align);
        });
    }

    /// Starts a stack-layout horizontal group.
    ///
    /// This is a compatibility helper for examples and utilities that follow
    /// `imgui-node-editor`'s blueprint builder. It is backed by a local shim,
    /// because Dear ImGui itself does not ship `BeginHorizontal`.
    /// Panics if the resolved ID has already been used in the current window and frame.
    #[doc(alias = "BeginHorizontal")]
    pub fn begin_horizontal_stack_layout<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> HorizontalStackLayoutToken<'ui> {
        self.begin_stack_layout(
            id.into(),
            size.into(),
            align,
            "Ui::begin_horizontal_stack_layout()",
            sys::ImGuiStack_BeginHorizontal_Id,
        );
        HorizontalStackLayoutToken::new(self)
    }

    /// Starts a stack-layout horizontal group.
    ///
    /// Alias for [`Self::begin_horizontal_stack_layout`] with the upstream
    /// naming used by imgui-node-editor examples.
    #[doc(alias = "BeginHorizontal")]
    pub fn begin_horizontal<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> HorizontalStackLayoutToken<'ui> {
        self.begin_horizontal_stack_layout(id, size, align)
    }

    /// Runs a closure inside a stack-layout horizontal group.
    #[doc(alias = "BeginHorizontal", alias = "EndHorizontal")]
    pub fn horizontal_stack_layout<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        let token = self.begin_horizontal_stack_layout(id, size, align);
        let result = f();
        token.end();
        result
    }

    /// Runs a closure inside a stack-layout horizontal group.
    ///
    /// Alias for [`Self::horizontal_stack_layout`].
    #[doc(alias = "BeginHorizontal", alias = "EndHorizontal")]
    pub fn horizontal<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        self.horizontal_stack_layout(id, size, align, f)
    }

    /// Starts a stack-layout vertical group.
    ///
    /// This is a compatibility helper for examples and utilities that follow
    /// `imgui-node-editor`'s blueprint builder. It is backed by a local shim,
    /// because Dear ImGui itself does not ship `BeginVertical`.
    /// Panics if the resolved ID has already been used in the current window and frame.
    #[doc(alias = "BeginVertical")]
    pub fn begin_vertical_stack_layout<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> VerticalStackLayoutToken<'ui> {
        self.begin_stack_layout(
            id.into(),
            size.into(),
            align,
            "Ui::begin_vertical_stack_layout()",
            sys::ImGuiStack_BeginVertical_Id,
        );
        VerticalStackLayoutToken::new(self)
    }

    /// Starts a stack-layout vertical group.
    ///
    /// Alias for [`Self::begin_vertical_stack_layout`] with the upstream naming
    /// used by imgui-node-editor examples.
    #[doc(alias = "BeginVertical")]
    pub fn begin_vertical<'ui, 'id>(
        &'ui self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
    ) -> VerticalStackLayoutToken<'ui> {
        self.begin_vertical_stack_layout(id, size, align)
    }

    /// Runs a closure inside a stack-layout vertical group.
    #[doc(alias = "BeginVertical", alias = "EndVertical")]
    pub fn vertical_stack_layout<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        let token = self.begin_vertical_stack_layout(id, size, align);
        let result = f();
        token.end();
        result
    }

    /// Runs a closure inside a stack-layout vertical group.
    ///
    /// Alias for [`Self::vertical_stack_layout`].
    #[doc(alias = "BeginVertical", alias = "EndVertical")]
    pub fn vertical<'id, R>(
        &self,
        id: impl Into<StackLayoutId<'id>>,
        size: impl Into<[f32; 2]>,
        align: f32,
        f: impl FnOnce() -> R,
    ) -> R {
        self.vertical_stack_layout(id, size, align, f)
    }

    /// Inserts a spring into the current stack layout.
    ///
    /// `weight <= 0.0` reserves only spacing. `spacing < 0.0` uses the current
    /// style item spacing along the layout axis, matching the upstream stack
    /// layout extension semantics.
    /// Panics if the current window has no active, unsuspended stack layout.
    #[doc(alias = "Spring")]
    pub fn stack_layout_spring(&self, weight: f32, spacing: f32) {
        assert!(
            weight.is_finite(),
            "Ui::stack_layout_spring() weight must be finite"
        );
        assert!(
            spacing.is_finite(),
            "Ui::stack_layout_spring() spacing must be finite"
        );
        self.run_with_bound_context(|| {
            self.assert_stack_layout_active();
            unsafe { sys::ImGuiStack_Spring(weight, spacing) };
        });
    }

    /// Inserts a spring into the current stack layout.
    ///
    /// Alias for [`Self::stack_layout_spring`] with the upstream naming used by
    /// imgui-node-editor examples.
    #[doc(alias = "Spring")]
    pub fn spring(&self, weight: f32, spacing: f32) {
        self.stack_layout_spring(weight, spacing)
    }

    /// Suspends the current stack layout until the returned token is dropped.
    #[doc(alias = "SuspendLayout")]
    pub fn suspend_stack_layout(&self) -> StackLayoutSuspensionToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::ImGuiStack_SuspendLayout() });
        StackLayoutSuspensionToken::new(self)
    }
}
