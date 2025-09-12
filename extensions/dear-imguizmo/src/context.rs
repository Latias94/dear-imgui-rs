//! Context management for ImGuizmo
//!
//! This module provides the main context and UI structures for ImGuizmo,
//! following the same pattern as dear-implot for safe Rust integration.

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
    /// Current gizmo size in clip space
    pub(crate) gizmo_size_clip_space: f32,
    /// Whether axis flipping is allowed
    pub(crate) allow_axis_flip: bool,
    /// Axis visibility limits
    pub(crate) axis_limit: f32,
    /// Plane visibility limits
    pub(crate) plane_limit: f32,
    /// Axis mask (true = hidden, false = shown)
    pub(crate) axis_mask: [bool; 3],
    /// Current style
    pub(crate) style: Style,
    /// Current draw list
    pub(crate) draw_list: Option<*mut dear_imgui::sys::ImDrawList>,
    /// Whether we're using bounds manipulation
    pub(crate) using_bounds: bool,
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
            gizmo_size_clip_space: 1.0,
            allow_axis_flip: true,
            axis_limit: 0.05,
            plane_limit: 0.2,
            axis_mask: [false; 3],
            style: Style::default(),
            draw_list: None,
            using_bounds: false,
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
        self.context.state.borrow().using != crate::gizmo::ManipulationType::None
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
        println!(
            "DEBUG: manipulate() called with operation: {:?}, mode: {:?}",
            operation, mode
        );
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
        _snap: Option<&[f32; 3]>,
        _local_bounds: Option<&[f32; 6]>,
        _bounds_snap: Option<&[f32; 3]>,
    ) -> GuizmoResult<bool> {
        println!("DEBUG: manipulate_with_options() called");

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

        // Set delta matrix to identity if provided
        if let Some(delta) = delta_matrix {
            *delta = Mat4::IDENTITY;
        }

        // Check if object is behind camera (for perspective projection)
        let mvp = *projection * *view * *matrix;
        let cam_space_pos = mvp.transform_point3(Vec3::ZERO);
        println!(
            "DEBUG: Camera space check - orthographic: {}, cam_space_pos.z: {}, using: {:?}",
            state.orthographic, cam_space_pos.z, state.using
        );
        if !state.orthographic
            && cam_space_pos.z < 0.001
            && state.using == crate::gizmo::ManipulationType::None
        {
            println!("DEBUG: Object is behind camera, returning early");
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

        // Draw translation gizmo
        println!(
            "DEBUG: Checking if operation contains TRANSLATE: operation={:?}, TRANSLATE={:?}",
            operation,
            Operation::TRANSLATE
        );
        println!(
            "DEBUG: operation.contains(Operation::TRANSLATE) = {}",
            operation.contains(Operation::TRANSLATE)
        );
        if operation.contains(Operation::TRANSLATE) {
            println!("DEBUG: Drawing translation gizmo");
            crate::draw::draw_translation_gizmo(
                draw_list,
                state.mvp,
                state.model_matrix,
                state.viewport,
                state.screen_factor,
                operation,
                state.using,
            )?;
        } else {
            println!("DEBUG: NOT drawing translation gizmo - condition failed");
        }

        // Draw rotation gizmo
        if operation.contains(Operation::ROTATE) {
            println!("DEBUG: Drawing rotation gizmo");
            crate::draw::draw_rotation_gizmo(draw_list, self.context, operation, state.using)?;
        }

        // Draw scale gizmo
        if operation.contains(Operation::SCALE) {
            println!("DEBUG: Drawing scale gizmo");
            crate::draw::draw_scale_gizmo(draw_list, self.context, operation, state.using)?;
        }

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

        Ok(modified)
    }

    /// Draw a grid in 3D space
    pub fn draw_grid(&self, _view: &Mat4, _projection: &Mat4, _matrix: &Mat4, grid_size: f32) {
        crate::guizmo_trace!("Drawing grid with size {}", grid_size);
        // TODO: Implement grid drawing
    }

    /// Draw debug cubes
    pub fn draw_cubes(&self, _view: &Mat4, _projection: &Mat4, matrices: &[Mat4]) {
        crate::guizmo_trace!("Drawing {} debug cubes", matrices.len());
        // TODO: Implement cube drawing
    }

    /// View manipulation cube
    pub fn view_manipulate(
        &self,
        _view: &mut Mat4,
        _length: f32,
        position: [f32; 2],
        size: [f32; 2],
        _background_color: u32,
    ) -> bool {
        crate::guizmo_trace!(
            "View manipulate at ({}, {}) with size ({}, {})",
            position[0],
            position[1],
            size[0],
            size[1]
        );
        // TODO: Implement view manipulation
        false
    }

    /// Check if mouse is over a specific operation
    pub fn is_over_operation(&self, _operation: Operation) -> bool {
        // TODO: Implement operation-specific hover detection
        false
    }

    /// Check if a 3D position is under the mouse cursor
    pub fn is_over_position(&self, _position: &Vec3, _pixel_radius: f32) -> bool {
        // TODO: Implement position-based hover detection
        false
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

        // Compute combined matrices
        state.view_projection = *view * *projection;
        state.mvp = *matrix * state.view_projection;
        state.mvp_local = state.model_local * state.view_projection;

        // Compute camera vectors from view matrix inverse
        let view_inverse = view.inverse();
        state.camera_dir = -view_inverse.z_axis.truncate(); // Camera looks down -Z
        state.camera_eye = view_inverse.w_axis.truncate();
        state.camera_right = view_inverse.x_axis.truncate();
        state.camera_up = view_inverse.y_axis.truncate();

        // Check if projection is orthographic
        // In orthographic projection, the w component of transformed points doesn't change with depth
        let near_vec = Vec4::new(0.0, 0.0, 1.0, 1.0);
        let far_vec = Vec4::new(0.0, 0.0, 2.0, 1.0);
        let near_pos = *projection * near_vec;
        let far_pos = *projection * far_vec;
        state.orthographic = (near_pos.z / near_pos.w - far_pos.z / far_pos.w).abs() < 0.001;

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
