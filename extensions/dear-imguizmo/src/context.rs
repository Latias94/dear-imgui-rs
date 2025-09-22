//! Context management for ImGuizmo
//!
//! This module provides the main context and UI structures for ImGuizmo,
//! following the same pattern as dear-implot for safe Rust integration.

use crate::types::ColorExt;
use crate::types::{Mat4, Mode, Operation, Rect, Vec2, Vec3, Vec4};
use crate::{GuizmoError, GuizmoResult, Style};
use dear_imgui::internal::RawWrapper;
use dear_imgui::{DrawList, DrawListMut, Ui};
use std::cell::RefCell;
use std::rc::Rc;

/// Internal state for gizmo operations
#[derive(Debug)]
pub(crate) struct GuizmoState {
    /// Current viewport rectangle
    pub(crate) viewport: Rect,
    /// Current manipulation type being used
    pub(crate) using: crate::gizmo::ManipulationType,
    /// Whether the mouse is over any gizmo element
    pub(crate) is_over: bool,
    /// Whether view manipulate is being used
    pub(crate) is_using_view_manipulate: bool,
    /// Whether view manipulate is hovered
    pub(crate) is_view_manipulate_hovered: bool,
    /// Whether gizmo is enabled
    pub(crate) enabled: bool,
    /// Whether orthographic projection is used
    pub(crate) orthographic: bool,
    /// If true, orthographic state has been explicitly forced by user
    pub(crate) orthographic_forced: bool,
    /// Current gizmo size in clip space
    pub(crate) gizmo_size_clip_space: f32,
    /// Whether axis flipping is allowed
    pub(crate) allow_axis_flip: bool,
    /// Axis visibility limits
    pub(crate) axis_limit: f32,
    /// Plane visibility limits
    pub(crate) plane_limit: f32,
    /// Whether each axis is below projection visibility threshold
    pub(crate) below_axis_limit: [bool; 3],
    /// Whether each plane is below projection visibility threshold
    pub(crate) below_plane_limit: [bool; 3],
    /// Axis mask (true = hidden, false = shown)
    pub(crate) axis_mask: [bool; 3],
    /// Current style
    pub(crate) style: Style,
    /// Current draw list
    pub(crate) draw_list: Option<*mut dear_imgui::sys::ImDrawList>,
    /// Whether we're using bounds manipulation
    pub(crate) using_bounds: bool,
    /// Selected bounds corner index (-1 if none)
    pub(crate) selected_bounds_corner: i32,
    /// Selected bounds face index (-1 if none), order: [minX,maxX,minY,maxY,minZ,maxZ]
    pub(crate) selected_bounds_face: i32,
    /// Selected bounds edge index (-1 if none), order: X-parallel[0..3], Y-parallel[4..7], Z-parallel[8..11]
    pub(crate) selected_bounds_edge: i32,
    /// Hovered bounds corner index (-1 if none)
    pub(crate) hover_bounds_corner: i32,
    /// Hovered bounds face index (-1 if none)
    pub(crate) hover_bounds_face: i32,
    /// Hovered bounds edge index (-1 if none)
    pub(crate) hover_bounds_edge: i32,
    /// Optional local bounds [minx,maxx,miny,maxy,minz,maxz]
    pub(crate) local_bounds: Option<[f32; 6]>,
    /// Optional bounds snap [sx,sy,sz]
    pub(crate) bounds_snap: Option<[f32; 3]>,
    /// Bounds manipulation: best axis index (0=X,1=Y,2=Z)
    pub(crate) bounds_best_axis: i32,
    /// Bounds manipulation: active axes for this handle (e.g., corner has 2, edge has 1; -1 for none)
    pub(crate) bounds_axes: [i32; 2],
    /// Bounds manipulation: local pivot used for scaling around
    pub(crate) bounds_local_pivot: Vec3,
    /// Current view matrix
    pub(crate) view_matrix: Mat4,
    /// Current projection matrix
    pub(crate) projection_matrix: Mat4,
    /// Current model matrix
    pub(crate) model_matrix: Mat4,
    /// Current operation being performed
    pub(crate) operation: Operation,
    /// Current transformation mode
    pub(crate) mode: Mode,
    /// Current manipulation type
    pub(crate) current_manipulation_type: crate::gizmo::ManipulationType,
    /// Whether mouse is over gizmo hotspot
    pub(crate) over_gizmo_hotspot: bool,
    /// Current editing ID
    pub(crate) editing_id: u32,
    /// Screen factor for gizmo sizing
    pub(crate) screen_factor: f32,
    /// Camera direction
    pub(crate) camera_dir: Vec3,
    /// Camera eye position
    pub(crate) camera_eye: Vec3,
    /// Camera right vector
    pub(crate) camera_right: Vec3,
    /// Camera up vector
    pub(crate) camera_up: Vec3,
    /// Model matrix for local transformations
    pub(crate) model_local: Mat4,
    /// Model matrix inverse
    pub(crate) model_inverse: Mat4,
    /// Model source matrix (original)
    pub(crate) model_source: Mat4,
    /// Model source matrix inverse
    pub(crate) model_source_inverse: Mat4,
    /// View-projection matrix
    pub(crate) view_projection: Mat4,
    /// Model-view-projection matrix
    pub(crate) mvp: Mat4,
    /// Local model-view-projection matrix
    pub(crate) mvp_local: Mat4,
    /// Translation plane for manipulation
    pub(crate) translation_plane: [f32; 4], // plane equation: ax + by + cz + d = 0
    /// Translation plane origin
    pub(crate) translation_plane_origin: Vec3,
    /// Matrix origin (starting position)
    pub(crate) matrix_origin: Vec3,
    /// Relative origin for manipulation
    pub(crate) relative_origin: Vec3,
    /// Last translation delta
    pub(crate) translation_last_delta: Vec3,
    /// Ray origin for picking
    pub(crate) ray_origin: Vec3,
    /// Ray direction for picking
    pub(crate) ray_vector: Vec3,
    /// Rotation start angle for rotation operations
    pub(crate) rotation_start_angle: f32,
    /// Scale start mouse position for scale operations
    pub(crate) scale_start_mouse_pos: Vec2,
    /// Mouse position when manipulation started
    pub(crate) mouse_down_pos: Vec2,
}

impl Default for GuizmoState {
    fn default() -> Self {
        Self {
            viewport: Rect::default(),
            using: crate::gizmo::ManipulationType::None,
            is_over: false,
            is_using_view_manipulate: false,
            is_view_manipulate_hovered: false,
            enabled: true,
            orthographic: false,
            orthographic_forced: false,
            gizmo_size_clip_space: 0.1,
            allow_axis_flip: true,
            axis_limit: 0.0025,
            plane_limit: 0.02,
            axis_mask: [false; 3],
            below_axis_limit: [false; 3],
            below_plane_limit: [false; 3],
            style: Style::default(),
            draw_list: None,
            using_bounds: false,
            selected_bounds_corner: -1,
            selected_bounds_face: -1,
            selected_bounds_edge: -1,
            hover_bounds_corner: -1,
            hover_bounds_face: -1,
            hover_bounds_edge: -1,
            local_bounds: None,
            bounds_snap: None,
            bounds_best_axis: -1,
            bounds_axes: [-1, -1],
            bounds_local_pivot: Vec3::ZERO,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            model_matrix: Mat4::IDENTITY,
            operation: Operation::empty(),
            mode: Mode::Local,
            current_manipulation_type: crate::gizmo::ManipulationType::None,
            over_gizmo_hotspot: false,
            editing_id: 0,
            screen_factor: 1.0,
            camera_dir: Vec3::NEG_Z,
            camera_eye: Vec3::ZERO,
            camera_right: Vec3::X,
            camera_up: Vec3::Y,
            model_local: Mat4::IDENTITY,
            model_inverse: Mat4::IDENTITY,
            model_source: Mat4::IDENTITY,
            model_source_inverse: Mat4::IDENTITY,
            view_projection: Mat4::IDENTITY,
            mvp: Mat4::IDENTITY,
            mvp_local: Mat4::IDENTITY,
            translation_plane: [0.0, 0.0, 1.0, 0.0], // XY plane by default
            translation_plane_origin: Vec3::ZERO,
            matrix_origin: Vec3::ZERO,
            relative_origin: Vec3::ZERO,
            translation_last_delta: Vec3::ZERO,
            ray_origin: Vec3::ZERO,
            ray_vector: Vec3::NEG_Z,
            rotation_start_angle: 0.0,
            scale_start_mouse_pos: Vec2::ZERO,
            mouse_down_pos: Vec2::ZERO,
        }
    }
}

/// ImGuizmo context that manages the gizmo state
///
/// This context is separate from the Dear ImGui context but works alongside it.
/// You need both contexts to create gizmos.
pub struct GuizmoContext {
    pub(crate) state: Rc<RefCell<GuizmoState>>,
}

impl GuizmoContext {
    /// Create a new ImGuizmo context
    ///
    /// This should be called after creating the Dear ImGui context.
    pub fn new() -> Self {
        crate::guizmo_info!("Creating new ImGuizmo context");

        Self {
            state: Rc::new(RefCell::new(GuizmoState::default())),
        }
    }

    /// Get a GuizmoUi for creating gizmos
    ///
    /// This borrows both the ImGuizmo context and the Dear ImGui Ui,
    /// ensuring that gizmos can only be created when both are available.
    pub fn get_ui<'ui>(&'ui self, ui: &'ui Ui) -> GuizmoUi<'ui> {
        GuizmoUi { context: self, ui }
    }

    /// Begin a new frame
    ///
    /// This should be called at the beginning of each frame, similar to ImGui::NewFrame().
    pub fn begin_frame(&self) {
        let mut state = self.state.borrow_mut();
        state.using = crate::gizmo::ManipulationType::None;
        state.is_over = false;
        state.is_using_view_manipulate = false;
        state.is_view_manipulate_hovered = false;

        crate::guizmo_trace!("ImGuizmo frame started");
    }

    /// Set the current draw list
    ///
    /// This allows drawing gizmos to a specific ImGui draw list.
    pub fn set_draw_list(&self, draw_list: Option<&DrawList>) {
        let mut state = self.state.borrow_mut();
        state.draw_list = draw_list.map(|dl| unsafe { dl.raw() as *const _ as *mut _ });
    }

    /// Get the current style
    pub fn get_style(&self) -> Style {
        self.state.borrow().style.clone()
    }

    /// Set the current style
    pub fn set_style(&self, style: Style) -> GuizmoResult<()> {
        style.validate()?;
        self.state.borrow_mut().style = style;
        Ok(())
    }

    /// Get current unique ID for this context
    pub fn get_current_id(&self) -> u32 {
        // Simple hash of the context pointer
        self as *const _ as usize as u32
    }

    /// Check if we can activate manipulation (mouse clicked and not over UI)
    pub fn can_activate() -> bool {
        unsafe {
            dear_imgui::sys::ImGui_IsMouseClicked(0, false)
                && !dear_imgui::sys::ImGui_IsAnyItemHovered()
                && !dear_imgui::sys::ImGui_IsAnyItemActive()
        }
    }
}

impl Default for GuizmoContext {
    fn default() -> Self {
        Self::new()
    }
}

/// A temporary reference for building gizmos
///
/// This struct ensures that gizmos can only be created when both ImGui and ImGuizmo
/// contexts are available and properly set up.
pub struct GuizmoUi<'ui> {
    context: &'ui GuizmoContext,
    ui: &'ui Ui,
}

impl<'ui> GuizmoUi<'ui> {
    /// Set the viewport rectangle for gizmo rendering
    pub fn set_rect(&self, x: f32, y: f32, width: f32, height: f32) -> GuizmoResult<()> {
        crate::utils::validate_viewport(x, y, width, height)?;

        let mut state = self.context.state.borrow_mut();
        state.viewport = Rect::new(x, y, width, height);

        crate::guizmo_debug!("Viewport set to ({}, {}, {}, {})", x, y, width, height);
        Ok(())
    }

    /// Set whether to use orthographic projection
    pub fn set_orthographic(&self, orthographic: bool) {
        let mut state = self.context.state.borrow_mut();
        state.orthographic = orthographic;
        state.orthographic_forced = true;

        crate::guizmo_debug!("Orthographic projection: {}", orthographic);
    }

    /// Enable or disable the gizmo
    pub fn enable(&self, enabled: bool) {
        let mut state = self.context.state.borrow_mut();
        state.enabled = enabled;

        crate::guizmo_debug!("Gizmo enabled: {}", enabled);
    }

    /// Check if the gizmo is currently being used
    pub fn is_using(&self) -> bool {
        let s = self.context.state.borrow();
        s.using != crate::gizmo::ManipulationType::None || s.using_bounds
    }

    /// Check if the mouse is over any gizmo element
    pub fn is_over(&self) -> bool {
        self.context.state.borrow().is_over
    }

    /// Check if any gizmo is in use
    pub fn is_using_any(&self) -> bool {
        let state = self.context.state.borrow();
        (state.using != crate::gizmo::ManipulationType::None) || state.is_using_view_manipulate
    }

    /// Check if view manipulate is being used
    pub fn is_using_view_manipulate(&self) -> bool {
        self.context.state.borrow().is_using_view_manipulate
    }

    /// Check if view manipulate is hovered
    pub fn is_view_manipulate_hovered(&self) -> bool {
        self.context.state.borrow().is_view_manipulate_hovered
    }

    /// Set the gizmo size in clip space
    pub fn set_gizmo_size_clip_space(&self, size: f32) {
        let mut state = self.context.state.borrow_mut();
        state.gizmo_size_clip_space = size.max(0.1);

        crate::guizmo_debug!("Gizmo size set to {}", size);
    }

    /// Allow or disallow axis flipping for better visibility
    pub fn allow_axis_flip(&self, allow: bool) {
        let mut state = self.context.state.borrow_mut();
        state.allow_axis_flip = allow;
    }

    /// Set the limit where axes are hidden
    pub fn set_axis_limit(&self, limit: f32) {
        let mut state = self.context.state.borrow_mut();
        state.axis_limit = limit.clamp(0.0, 1.0);
    }

    /// Set axis mask to permanently hide/show axes
    pub fn set_axis_mask(&self, x: bool, y: bool, z: bool) {
        let mut state = self.context.state.borrow_mut();
        state.axis_mask = [x, y, z];

        crate::guizmo_debug!("Axis mask set to [{}, {}, {}]", x, y, z);
    }

    /// Set the limit where planes are hidden
    pub fn set_plane_limit(&self, limit: f32) {
        let mut state = self.context.state.borrow_mut();
        state.plane_limit = limit.clamp(0.0, 1.0);
    }

    /// Get the current style
    pub fn get_style(&self) -> Style {
        self.context.get_style()
    }

    /// Set the current style
    pub fn set_style(&self, style: &Style) -> GuizmoResult<()> {
        self.context.set_style(style.clone())
    }

    /// Main manipulation function
    ///
    /// This is the core function for 3D object manipulation.
    pub fn manipulate(
        &self,
        draw_list: &DrawListMut,
        view: &Mat4,
        projection: &Mat4,
        operation: Operation,
        mode: Mode,
        matrix: &mut Mat4,
    ) -> GuizmoResult<bool> {
        self.manipulate_with_options(
            draw_list, view, projection, operation, mode, matrix, None, None, None, None,
        )
    }

    /// Manipulation with snapping support
    pub fn manipulate_with_snap(
        &self,
        draw_list: &DrawListMut,
        view: &Mat4,
        projection: &Mat4,
        operation: Operation,
        mode: Mode,
        matrix: &mut Mat4,
        snap: Option<&[f32; 3]>,
    ) -> GuizmoResult<bool> {
        self.manipulate_with_options(
            draw_list, view, projection, operation, mode, matrix, None, snap, None, None,
        )
    }

    /// Full manipulation function with all options
    pub fn manipulate_with_options(
        &self,
        draw_list: &DrawListMut,
        view: &Mat4,
        projection: &Mat4,
        operation: Operation,
        mode: Mode,
        matrix: &mut Mat4,
        delta_matrix: Option<&mut Mat4>,
        snap: Option<&[f32; 3]>,
        _local_bounds: Option<&[f32; 6]>,
        _bounds_snap: Option<&[f32; 3]>,
    ) -> GuizmoResult<bool> {
        // Validate inputs
        if !crate::math::is_matrix_finite(view) {
            return Err(crate::GuizmoError::invalid_matrix(
                "View matrix contains invalid values",
            ));
        }
        if !crate::math::is_matrix_finite(projection) {
            return Err(crate::GuizmoError::invalid_matrix(
                "Projection matrix contains invalid values",
            ));
        }
        if !crate::math::is_matrix_finite(matrix) {
            return Err(crate::GuizmoError::invalid_matrix(
                "Transformation matrix contains invalid values",
            ));
        }

        let mut state = self.context.state.borrow_mut();

        if !state.enabled {
            return Ok(false);
        }

        // Validate viewport
        if state.viewport.width <= 0.0 || state.viewport.height <= 0.0 {
            return Err(GuizmoError::invalid_viewport(
                "Viewport not set or has invalid dimensions",
            ));
        }

        // Store bounds options
        state.local_bounds = _local_bounds.copied();
        state.bounds_snap = _bounds_snap.copied();

        // Set delta matrix to identity if provided
        if let Some(delta) = delta_matrix {
            *delta = Mat4::IDENTITY;
        }

        // Check if object is behind camera (for perspective projection)
        let mvp = *projection * *view * *matrix;
        let cam_space_pos = mvp.transform_point3(Vec3::ZERO);
        if !state.orthographic
            && cam_space_pos.z < 0.001
            && state.using == crate::gizmo::ManipulationType::None
        {
            return Ok(false);
        }

        // Update context state
        state.view_matrix = *view;
        state.projection_matrix = *projection;
        state.model_matrix = *matrix;
        state.operation = operation;
        state.mode = mode;

        crate::guizmo_trace!(
            "Manipulate called with operation {:?}, mode {:?}",
            operation,
            mode
        );

        // Compute context matrices and state
        self.compute_context(
            view,
            projection,
            matrix,
            if operation.contains(Operation::SCALE) {
                Mode::Local
            } else {
                mode
            },
            &mut state,
        )?;

        // Draw the gizmo based on the current operation
        let ui = self.ui;
        let mut modified = false;

        // Compute gizmo size for consistent on-screen scale
        let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
        let gizmo_size = crate::draw::calculate_gizmo_size(
            &state.view_matrix,
            gizmo_center,
            state.gizmo_size_clip_space,
        );

        // Determine highlight manipulation type (hover takes precedence when not using)
        let mouse = ui.io().mouse_pos();
        let mouse_vec2 = Vec2::new(mouse[0], mouse[1]);
        let hover_hit =
            crate::interaction::is_over_gizmo(ui, &state, mouse_vec2).unwrap_or_default();
        let highlight = if state.using != crate::gizmo::ManipulationType::None {
            state.using
        } else if hover_hit.hit {
            hover_hit.manipulation_type
        } else {
            crate::gizmo::ManipulationType::None
        };

        // Release the mutable borrow before calling draw functions
        drop(state);

        // Draw translation gizmo
        if operation.contains(Operation::TRANSLATE) {
            crate::draw::draw_translation_gizmo(draw_list, self.context, operation, highlight)?;
        }

        // Draw rotation gizmo
        if operation.contains(Operation::ROTATE) {
            crate::draw::draw_rotation_gizmo(draw_list, self.context, operation, highlight)?;
        }

        // Draw scale gizmo
        if operation.contains(Operation::SCALE) {
            crate::draw::draw_scale_gizmo(draw_list, self.context, operation, highlight)?;
        }

        // Re-borrow for mouse interaction handling
        let mut state = self.context.state.borrow_mut();

        // Handle mouse interaction
        if ui.is_mouse_clicked(dear_imgui::MouseButton::Left) {
            let mouse_pos = ui.io().mouse_pos();
            let mouse_vec2 = glam::Vec2::new(mouse_pos[0], mouse_pos[1]);

            // Test for gizmo interaction
            if let Ok(hit_result) = crate::interaction::is_over_gizmo(ui, &state, mouse_vec2) {
                if hit_result.hit {
                    state.using = hit_result.manipulation_type;
                    state.mouse_down_pos = mouse_vec2;
                    crate::guizmo_trace!("Started manipulation: {:?}", state.using);
                }
            }
        }

        // Update bounds hover indices (corner > edge > face)
        if state.local_bounds.is_some() {
            let mouse_pos = ui.io().mouse_pos();
            let mp = Vec2::new(mouse_pos[0], mouse_pos[1]);
            state.hover_bounds_corner = match crate::interaction::is_over_bounds_corners(&state, mp)
            {
                Ok(Some(i)) => i as i32,
                _ => -1,
            };
            if state.hover_bounds_corner < 0 {
                state.hover_bounds_edge = match crate::interaction::is_over_bounds_edges(&state, mp)
                {
                    Ok(Some(i)) => i as i32,
                    _ => -1,
                };
            } else {
                state.hover_bounds_edge = -1;
            }
            if state.hover_bounds_corner < 0 && state.hover_bounds_edge < 0 {
                state.hover_bounds_face = match crate::interaction::is_over_bounds_faces(&state, mp)
                {
                    Ok(Some(i)) => i as i32,
                    _ => -1,
                };
            } else {
                state.hover_bounds_face = -1;
            }
        }

        // Handle mouse drag
        if ui.is_mouse_dragging(dear_imgui::MouseButton::Left)
            && state.using != crate::gizmo::ManipulationType::None
        {
            let mouse_pos = ui.io().mouse_pos();
            let mouse_vec2 = glam::Vec2::new(mouse_pos[0], mouse_pos[1]);

            let is_left_down = ui.is_mouse_down(dear_imgui::MouseButton::Left);
            let is_left_clicked = ui.is_mouse_clicked(dear_imgui::MouseButton::Left);
            let is_left_released = ui.is_mouse_released(dear_imgui::MouseButton::Left);

            if let Ok(interaction_result) = crate::interaction::handle_mouse_interaction(
                ui,
                &mut state,
                mouse_vec2,
                is_left_down,
                is_left_clicked,
                is_left_released,
                snap,
            ) {
                if interaction_result {
                    modified = true;
                    *matrix = state.model_matrix;
                    crate::guizmo_trace!("Matrix modified during manipulation");
                }
            }
        }

        // Handle mouse release
        if ui.is_mouse_released(dear_imgui::MouseButton::Left)
            && state.using != crate::gizmo::ManipulationType::None
        {
            crate::guizmo_trace!("Ended manipulation: {:?}", state.using);
            state.using = crate::gizmo::ManipulationType::None;
        }

        // Draw bounds if provided
        if let Some(local_bounds) = state.local_bounds {
            let mvp = state.mvp;
            let _ = crate::draw::draw_local_bounds(draw_list, &mvp, &state.viewport, &local_bounds);
            let corners =
                crate::draw::draw_bounds_handles(draw_list, &mvp, &state.viewport, &local_bounds)
                    .unwrap_or([[0.0; 2]; 8]);
            let faces = crate::draw::draw_bounds_face_handles(
                draw_list,
                &mvp,
                &state.viewport,
                &local_bounds,
            )
            .unwrap_or([[0.0; 2]; 6]);
            let edges = crate::draw::draw_bounds_edge_handles(
                draw_list,
                &mvp,
                &state.viewport,
                &local_bounds,
            )
            .unwrap_or([[0.0; 2]; 12]);

            // Highlight hovered/selected handles
            let sel_col =
                state.style.colors[crate::types::ColorElement::Selection as usize].as_u32();
            let outline = 0xFF000000u32;
            // Corner
            if state.hover_bounds_corner >= 0 || state.selected_bounds_corner >= 0 {
                let idx = if state.selected_bounds_corner >= 0 {
                    state.selected_bounds_corner
                } else {
                    state.hover_bounds_corner
                } as usize;
                let p = corners[idx];
                draw_list.add_circle(p, 6.0, sel_col).filled(true).build();
                draw_list.add_circle(p, 6.0, outline).build();
            }
            // Edge
            if state.hover_bounds_edge >= 0 || state.selected_bounds_edge >= 0 {
                let idx = if state.selected_bounds_edge >= 0 {
                    state.selected_bounds_edge
                } else {
                    state.hover_bounds_edge
                } as usize;
                let p = edges[idx];
                // Diamond highlight
                let s = 6.0;
                let a = [p[0], p[1] - s];
                let b = [p[0] + s, p[1]];
                let c2 = [p[0], p[1] + s];
                let d = [p[0] - s, p[1]];
                draw_list
                    .add_triangle(a, b, c2, sel_col)
                    .filled(true)
                    .build();
                draw_list
                    .add_triangle(a, c2, d, sel_col)
                    .filled(true)
                    .build();
                draw_list.add_line(a, b, outline).build();
                draw_list.add_line(b, c2, outline).build();
                draw_list.add_line(c2, d, outline).build();
                draw_list.add_line(d, a, outline).build();
            }
            // Face
            if state.hover_bounds_face >= 0 || state.selected_bounds_face >= 0 {
                let idx = if state.selected_bounds_face >= 0 {
                    state.selected_bounds_face
                } else {
                    state.hover_bounds_face
                } as usize;
                let p = faces[idx];
                let s = 7.0;
                draw_list
                    .add_rect([p[0] - s, p[1] - s], [p[0] + s, p[1] + s], sel_col)
                    .filled(true)
                    .build();
                draw_list
                    .add_rect([p[0] - s, p[1] - s], [p[0] + s, p[1] + s], outline)
                    .build();
            }
        }

        Ok(modified)
    }

    /// Draw a grid in 3D space
    pub fn draw_grid(&self, view: &Mat4, projection: &Mat4, matrix: &Mat4, grid_size: f32) {
        // Extract viewport data first to avoid RefCell borrow conflicts
        let viewport = self.context.state.borrow().viewport;
        let draw_list = self.ui.get_window_draw_list();
        let _ = crate::draw::draw_grid(
            &draw_list,
            view,
            projection,
            matrix,
            &viewport,
            grid_size,
        );
    }

    /// Draw debug cubes
    pub fn draw_cubes(&self, view: &Mat4, projection: &Mat4, matrices: &[Mat4]) {
        // Extract viewport data first to avoid RefCell borrow conflicts
        let viewport = self.context.state.borrow().viewport;
        let draw_list = self.ui.get_window_draw_list();
        let _ = crate::draw::draw_cubes(
            &draw_list,
            view,
            projection,
            matrices,
            &viewport,
        );
    }

    /// View manipulation cube
    pub fn view_manipulate(
        &self,
        view: &mut Mat4,
        length: f32,
        position: [f32; 2],
        size: [f32; 2],
        background_color: u32,
    ) -> bool {
        match crate::view::view_manipulate(
            self.ui,
            self.context,
            view,
            length,
            position,
            size,
            background_color,
        ) {
            Ok(result) => result.modified,
            Err(_) => false,
        }
    }

    /// Check if mouse is over a specific operation
    pub fn is_over_operation(&self, operation: Operation) -> bool {
        let ui = self.ui;
        let sref = self.context.state.borrow();
        let mouse = ui.io().mouse_pos();
        let mouse_pos = Vec2::new(mouse[0], mouse[1]);
        match crate::interaction::is_over_gizmo(ui, &sref, mouse_pos) {
            Ok(hit) if hit.hit => {
                use crate::gizmo::ManipulationType as MT;
                match hit.manipulation_type {
                    MT::MoveX | MT::MoveY | MT::MoveZ | MT::MoveXY | MT::MoveYZ | MT::MoveZX => {
                        operation.intersects(Operation::TRANSLATE)
                    }
                    MT::RotateX | MT::RotateY | MT::RotateZ => {
                        operation.intersects(Operation::ROTATE)
                    }
                    MT::ScaleX | MT::ScaleY | MT::ScaleZ => operation.intersects(Operation::SCALE),
                    MT::ScaleXYZ => {
                        operation.intersects(Operation::SCALE_UNIFORM)
                            || operation.intersects(Operation::SCALE)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Check if a 3D position is under the mouse cursor
    pub fn is_over_position(&self, position: &Vec3, pixel_radius: f32) -> bool {
        let sref = self.context.state.borrow();
        if sref.viewport.width <= 0.0 || sref.viewport.height <= 0.0 {
            return false;
        }
        let mvp = sref.view_projection * sref.model_matrix;
        if let Ok(p) = crate::draw::project_to_screen(&mvp, *position, &sref.viewport.as_viewport())
        {
            let mouse = self.ui.io().mouse_pos();
            let m = Vec2::new(mouse[0], mouse[1]);
            (m - p).length() <= pixel_radius.max(1.0)
        } else {
            false
        }
    }

    /// Compute context matrices and state for manipulation
    fn compute_context(
        &self,
        view: &Mat4,
        projection: &Mat4,
        matrix: &Mat4,
        mode: Mode,
        state: &mut std::cell::RefMut<GuizmoState>,
    ) -> GuizmoResult<()> {
        // Store matrices
        state.view_matrix = *view;
        state.projection_matrix = *projection;
        state.model_matrix = *matrix;
        state.model_source = *matrix;
        state.mode = mode;

        // Compute matrix inverses
        state.model_inverse = matrix.inverse();
        state.model_source_inverse = matrix.inverse();

        // Normalize the model matrix for local transformations
        state.model_local = crate::math::orthonormalize_matrix(matrix);

        // Compute combined matrices with glam (column vector) convention
        // MVP = Projection * View * Model
        state.view_projection = *projection * *view;
        state.mvp = state.view_projection * *matrix;
        state.mvp_local = state.view_projection * state.model_local;

        // Compute camera vectors from view matrix inverse
        let view_inverse = view.inverse();
        state.camera_dir = -view_inverse.z_axis.truncate(); // Camera looks down -Z
        state.camera_eye = view_inverse.w_axis.truncate();
        state.camera_right = view_inverse.x_axis.truncate();
        state.camera_up = view_inverse.y_axis.truncate();

        // Check if projection is orthographic (unless user forced a mode)
        // In orthographic projection, the w component of transformed points doesn't change with depth
        if !state.orthographic_forced {
            let near_vec = Vec4::new(0.0, 0.0, -1.0, 1.0);
            let far_vec = Vec4::new(0.0, 0.0, 1.0, 1.0);
            let near_pos = *projection * near_vec;
            let far_pos = *projection * far_vec;
            state.orthographic = (near_pos.z / near_pos.w - far_pos.z / far_pos.w).abs() < 0.001;
        }

        // Compute screen factor for gizmo sizing
        // This ensures the gizmo appears the same size regardless of distance
        let point_right = view_inverse.x_axis.truncate();
        let right_length = crate::math::get_segment_length_clip_space(
            Vec3::ZERO,
            point_right,
            &state.view_projection,
        );
        state.screen_factor = if right_length > 0.0 {
            state.gizmo_size_clip_space / right_length
        } else {
            1.0
        };

        // Compute ray for mouse picking
        self.compute_camera_ray(state)?;

        // Compute axis/plane visibility flags based on projection
        {
            // Use local, orthonormalized axes to avoid scale skew
            let x_dir = state.model_local.transform_vector3(Vec3::X);
            let y_dir = state.model_local.transform_vector3(Vec3::Y);
            let z_dir = state.model_local.transform_vector3(Vec3::Z);

            let len_x = crate::math::get_segment_length_clip_space(
                Vec3::ZERO,
                x_dir,
                &state.view_projection,
            );
            let len_y = crate::math::get_segment_length_clip_space(
                Vec3::ZERO,
                y_dir,
                &state.view_projection,
            );
            let len_z = crate::math::get_segment_length_clip_space(
                Vec3::ZERO,
                z_dir,
                &state.view_projection,
            );

            state.below_axis_limit[0] = len_x < state.axis_limit;
            state.below_axis_limit[1] = len_y < state.axis_limit;
            state.below_axis_limit[2] = len_z < state.axis_limit;

            // Planes: approximate by projected lengths product
            let area_x = len_y * len_z; // plane normal X (YZ plane)
            let area_y = len_x * len_z; // plane normal Y (XZ plane)
            let area_z = len_x * len_y; // plane normal Z (XY plane)

            state.below_plane_limit[0] = area_x < state.plane_limit;
            state.below_plane_limit[1] = area_y < state.plane_limit;
            state.below_plane_limit[2] = area_z < state.plane_limit;
        }

        crate::guizmo_trace!(
            "Context computed: orthographic={}, screen_factor={}",
            state.orthographic,
            state.screen_factor
        );

        Ok(())
    }

    /// Compute camera ray for mouse picking
    fn compute_camera_ray(&self, state: &mut std::cell::RefMut<GuizmoState>) -> GuizmoResult<()> {
        let io = unsafe { dear_imgui::sys::ImGui_GetIO() };
        let mouse_pos = unsafe { (*io).MousePos };

        // Convert mouse position to normalized device coordinates
        let viewport = state.viewport;
        let ndc_x = (mouse_pos.x - viewport.x) / viewport.width * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.y - viewport.y) / viewport.height * 2.0;

        // Compute ray in world space
        let view_proj_inverse = state.view_projection.inverse();

        if state.orthographic {
            // Orthographic projection: ray direction is constant
            state.ray_vector = state.camera_dir;

            // Ray origin is on the near plane
            let near_point = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
            let world_near = view_proj_inverse * near_point;
            state.ray_origin = world_near.truncate() / world_near.w;
        } else {
            // Perspective projection: ray origin is camera position
            state.ray_origin = state.camera_eye;

            // Compute ray direction
            let far_point = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
            let world_far = view_proj_inverse * far_point;
            let world_far_pos = world_far.truncate() / world_far.w;

            state.ray_vector = (world_far_pos - state.ray_origin).normalize();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let context = GuizmoContext::new();
        let style = context.get_style();
        assert_eq!(style.translation_line_thickness, 3.0);
    }

    #[test]
    fn test_viewport_setting() {
        let _context = GuizmoContext::new();
        // We can't easily test GuizmoUi without a real ImGui context
        // This would require integration tests
    }

    #[test]
    fn test_style_operations() {
        let context = GuizmoContext::new();
        let mut style = Style::new();
        style.translation_line_thickness = 5.0;

        assert!(context.set_style(style.clone()).is_ok());
        let retrieved_style = context.get_style();
        assert_eq!(retrieved_style.translation_line_thickness, 5.0);
    }
}
