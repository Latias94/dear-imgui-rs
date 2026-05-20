use super::flags::{DockNodeFlags, validate_dock_node_flags};
use super::validation::{assert_finite_vec2, assert_nonzero_id};
use super::window_class::WindowClass;
use crate::ui::Ui;
use crate::{Id, sys};
use std::ptr;

/// Docking-related functionality
impl Ui {
    /// Creates a dockspace over the main viewport
    ///
    /// This is a convenience function that creates a dockspace covering the entire main viewport.
    /// It's equivalent to calling `dock_space` with the main viewport's ID and size.
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
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport_with_flags(
    ///     0.into(),
    ///     DockNodeFlags::PASSTHRU_CENTRAL_NODE
    /// );
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dockspace_over_main_viewport_with_flags(
        &self,
        dockspace_id: Id,
        flags: DockNodeFlags,
    ) -> Id {
        validate_dock_node_flags("Ui::dockspace_over_main_viewport_with_flags()", flags);
        unsafe {
            Id::from(sys::igDockSpaceOverViewport(
                dockspace_id.into(),
                sys::igGetMainViewport(),
                flags.bits(),
                ptr::null(),
            ))
        }
    }

    /// Creates a dockspace over the main viewport with default settings
    ///
    /// This is a convenience function that creates a dockspace covering the entire main viewport
    /// with passthrough central node enabled.
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dockspace_over_main_viewport(&self) -> Id {
        self.dockspace_over_main_viewport_with_flags(
            Id::from(0u32),
            DockNodeFlags::PASSTHRU_CENTRAL_NODE,
        )
    }

    /// Creates a dockspace with the specified ID, size, and flags
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
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// let dockspace_id = ui.dock_space_with_class(
    ///     dockspace_id,
    ///     [800.0, 600.0],
    ///     DockNodeFlags::NO_DOCKING_SPLIT,
    ///     Some(&WindowClass::new(Id::from(1u32)))
    /// );
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space_with_class(
        &self,
        id: Id,
        size: [f32; 2],
        flags: DockNodeFlags,
        window_class: Option<&WindowClass>,
    ) -> Id {
        validate_dock_node_flags("Ui::dock_space_with_class()", flags);
        assert_nonzero_id("Ui::dock_space_with_class()", "id", id);
        assert_finite_vec2("Ui::dock_space_with_class()", "size", size);
        unsafe {
            let size_vec = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            let imgui_window_class =
                window_class.map(|class| class.to_imgui("Ui::dock_space_with_class()"));
            let window_class_ptr = imgui_window_class
                .as_ref()
                .map_or(ptr::null(), |wc| wc as *const _);
            Id::from(sys::igDockSpace(
                id.into(),
                size_vec,
                flags.bits(),
                window_class_ptr,
            ))
        }
    }

    /// Creates a dockspace with the specified ID and size
    ///
    /// # Parameters
    ///
    /// * `id` - The non-zero ID for the dockspace
    /// * `size` - The size of the dockspace in pixels
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// let dockspace_id = ui.dock_space(dockspace_id, [800.0, 600.0]);
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space(&self, id: Id, size: [f32; 2]) -> Id {
        self.dock_space_with_class(id, size, DockNodeFlags::NONE, None)
    }

    /// Sets the dock ID for the next window with condition
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
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
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id_with_cond(&self, dock_id: Id, cond: crate::Condition) {
        unsafe {
            sys::igSetNextWindowDockID(dock_id.into(), cond as i32);
        }
    }

    /// Sets the dock ID for the next window
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    /// Uses `Condition::Always` by default.
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
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ui.set_next_window_dock_id(dockspace_id);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
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
        unsafe {
            let imgui_wc = window_class.to_imgui("Ui::set_next_window_class()");
            sys::igSetNextWindowClass(&imgui_wc as *const _);
        }
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
        unsafe { Id::from(sys::igGetWindowDockID()) }
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
        unsafe { sys::igIsWindowDocked() }
    }
}
