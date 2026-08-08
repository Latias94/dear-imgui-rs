use super::flags::{DockNodeFlags, validate_dock_node_flags};
use super::validation::{
    assert_docking_available, assert_dockspace_has_no_active_content,
    assert_dockspace_host_name_supported, assert_dockspace_size,
    assert_existing_dockspace_node_is_root, assert_nonzero_id, claim_dockspace_submission,
    current_window_skips_items, main_viewport_dockspace_id,
};
use super::window_class::WindowClass;
use crate::ui::Ui;
use crate::{Id, sys};
use std::ptr;

/// Docking-related functionality
impl Ui {
    /// Configure and submit a dockspace through the canonical builder.
    pub fn dockspace(&self) -> crate::DockspaceBuilder<'_, 'static> {
        crate::DockspaceBuilder::new(self)
    }

    /// Submit Dear ImGui's low-level dockspace-over-viewport operation for the main viewport.
    ///
    /// This creates Dear ImGui's hidden main-viewport host window, applies the viewport work
    /// rectangle and platform ownership, and submits a dockspace within that host.
    /// Submit it before every window that can be hosted by this dockspace. A
    /// `KEEP_ALIVE_ONLY` submission may be made later because it does not create a visible host.
    /// Without an earlier submission, Dear ImGui may already undock a window when that window is
    /// begun, before this method can diagnose the ordering error.
    ///
    /// # Parameters
    ///
    /// * `dockspace_id` - The ID for the dockspace (use 0 to auto-generate)
    /// * `flags` - Dock node flags
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame, when the effective dockspace
    /// ID names a child of another dock tree, when the dockspace was already submitted without
    /// `KEEP_ALIVE_ONLY` during this frame, or when a hosted window was submitted before a visible
    /// dockspace submission while that window is still attached. `KEEP_ALIVE_ONLY` remains valid
    /// after hosted windows.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dock_space_over_main_viewport_raw(
    ///     0.into(),
    ///     DockNodeFlags::PASSTHRU_CENTRAL_NODE
    /// );
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dock_space_over_main_viewport_raw(&self, dockspace_id: Id, flags: DockNodeFlags) -> Id {
        const CALLER: &str = "Ui::dock_space_over_main_viewport_raw()";
        validate_dock_node_flags(CALLER, flags);
        self.run_with_bound_context(|| {
            assert_docking_available(CALLER);
            let requested = (dockspace_id.raw() != 0).then_some(dockspace_id);
            let effective_id = main_viewport_dockspace_id(CALLER, requested);
            assert_existing_dockspace_node_is_root(CALLER, effective_id);
            let claim = claim_dockspace_submission(self, CALLER, effective_id, flags, false)
                .unwrap_or_else(|_| {
                    panic!("{CALLER} cannot submit dockspace {effective_id:?} twice in one frame")
                });
            if !flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) {
                assert_dockspace_has_no_active_content(CALLER, effective_id);
            }
            let submitted = unsafe {
                Id::from(sys::igDockSpaceOverViewport(
                    effective_id.into(),
                    sys::igGetMainViewport(),
                    flags.bits(),
                    ptr::null(),
                ))
            };
            if let Some(claim) = claim {
                claim.commit();
            }
            assert_eq!(
                submitted, effective_id,
                "{CALLER} native submission returned an unexpected dockspace ID"
            );
            submitted
        })
    }

    /// Submit Dear ImGui's low-level dockspace operation in the current window.
    ///
    /// Submit it before every window that can be hosted by this dockspace. A
    /// `KEEP_ALIVE_ONLY` submission may be made later because it does not create a visible host.
    /// Without an earlier submission, Dear ImGui may already undock a window when that window is
    /// begun, before this method can diagnose the ordering error.
    ///
    /// # Parameters
    ///
    /// * `id` - The non-zero ID for the dockspace. Use [`Ui::get_id`] to create one.
    /// * `size` - The size of the dockspace in pixels
    /// * `flags` - Dock node flags
    /// * `window_class` - Optional window class for docking configuration
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame, when `id` names a child of
    /// another dock tree, when `id` was already submitted without `KEEP_ALIVE_ONLY` during this
    /// frame, or when a hosted window was submitted before a visible dockspace submission while
    /// that window is still attached. `KEEP_ALIVE_ONLY` remains valid after hosted windows.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// let dockspace_id = ui.dock_space_raw(
    ///     dockspace_id,
    ///     [800.0, 600.0],
    ///     DockNodeFlags::NO_DOCKING_SPLIT,
    ///     Some(&WindowClass::new(Id::from(1u32)))
    /// );
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space_raw(
        &self,
        id: Id,
        size: [f32; 2],
        flags: DockNodeFlags,
        window_class: Option<&WindowClass>,
    ) -> Id {
        const CALLER: &str = "Ui::dock_space_raw()";
        validate_dock_node_flags(CALLER, flags);
        assert_nonzero_id(CALLER, "id", id);
        assert_dockspace_size(CALLER, "size", size);
        let size_vec = sys::ImVec2 {
            x: size[0],
            y: size[1],
        };
        let imgui_window_class = window_class.map(|class| class.to_imgui(CALLER));
        let window_class_ptr = imgui_window_class
            .as_ref()
            .map_or(ptr::null(), |wc| wc as *const _);
        self.run_with_bound_context(|| {
            assert_dockspace_host_name_supported(CALLER);
            assert_existing_dockspace_node_is_root(CALLER, id);
            let host_skipped = current_window_skips_items(CALLER);
            let claim =
                claim_dockspace_submission(self, CALLER, id, flags, true).unwrap_or_else(|_| {
                    panic!("{CALLER} cannot submit dockspace {id:?} twice in one frame")
                });
            if !flags.contains(DockNodeFlags::KEEP_ALIVE_ONLY) && !host_skipped {
                assert_dockspace_has_no_active_content(CALLER, id);
            }
            let submitted = unsafe {
                Id::from(sys::igDockSpace(
                    id.into(),
                    size_vec,
                    flags.bits(),
                    window_class_ptr,
                ))
            };
            if let Some(claim) = claim {
                claim.commit();
            }
            assert_eq!(
                submitted, id,
                "{CALLER} native submission returned an unexpected dockspace ID"
            );
            submitted
        })
    }

    /// Sets the dock ID for the next window with condition
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    ///
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame.
    ///
    /// # Parameters
    ///
    /// * `dock_id` - The ID of the dock node to dock the next window to
    /// * `cond` - Condition for when to apply the docking
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace().build()?;
    /// ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// # Ok::<(), DockspaceError>(())
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id_with_cond(&self, dock_id: Id, cond: crate::Condition) {
        const CALLER: &str = "Ui::set_next_window_dock_id_with_cond()";
        self.run_with_bound_context(|| {
            assert_docking_available(CALLER);
            unsafe {
                sys::igSetNextWindowDockID(dock_id.into(), cond as i32);
            }
        });
    }

    /// Sets the dock ID for the next window
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    /// Uses `Condition::Always` by default.
    ///
    /// # Panics
    ///
    /// Panics when docking was not enabled before the first frame.
    ///
    /// # Parameters
    ///
    /// * `dock_id` - The ID of the dock node to dock the next window to
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace().build()?;
    /// ui.set_next_window_dock_id(dockspace_id);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// # Ok::<(), DockspaceError>(())
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id(&self, dock_id: Id) {
        self.set_next_window_dock_id_with_cond(dock_id, crate::Condition::Always)
    }

    /// Sets the window class for the next window
    ///
    /// This function must be called before creating a window to apply the window class configuration.
    ///
    /// # Parameters
    ///
    /// * `window_class` - The window class configuration
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let window_class = WindowClass::new(Id::from(1u32)).docking_always_tab_bar(true);
    /// ui.set_next_window_class(&window_class);
    /// ui.window("Classed Window").build(|| {
    ///     ui.text("This window has a custom class!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowClass")]
    pub fn set_next_window_class(&self, window_class: &WindowClass) {
        let imgui_wc = window_class.to_imgui("Ui::set_next_window_class()");
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextWindowClass(&imgui_wc as *const _);
        });
    }

    /// Gets the dock ID of the current window
    ///
    /// # Returns
    ///
    /// The dock ID of the current window, or 0 if the window is not docked
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.window("My Window").build(|| {
    ///     let dock_id = ui.get_window_dock_id();
    ///     if dock_id != 0.into() {
    ///         ui.text(format!("This window is docked with ID: {}", dock_id.raw()));
    ///     } else {
    ///         ui.text("This window is not docked");
    ///     }
    /// });
    /// ```
    #[doc(alias = "GetWindowDockID")]
    pub fn get_window_dock_id(&self) -> Id {
        self.run_with_bound_context(|| unsafe { Id::from(sys::igGetWindowDockID()) })
    }

    /// Checks if the current window is docked
    ///
    /// # Returns
    ///
    /// `true` if the current window is docked, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.window("My Window").build(|| {
    ///     if ui.is_window_docked() {
    ///         ui.text("This window is docked!");
    ///     } else {
    ///         ui.text("This window is floating");
    ///     }
    /// });
    /// ```
    #[doc(alias = "IsWindowDocked")]
    pub fn is_window_docked(&self) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsWindowDocked() })
    }
}
