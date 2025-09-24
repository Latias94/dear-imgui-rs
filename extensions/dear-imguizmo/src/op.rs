use dear_imguizmo_sys as sys;

use crate::mat::Mat4Like;
use crate::types::{AxisMask, Bounds, DrawListTarget, Mode, Operation, Vec3Like};
use crate::ui::GizmoUi;

/// Builder for ImGuizmo::Manipulate in a dear-imgui-style API
pub struct Manipulate<'ui, T: Mat4Like> {
    giz: &'ui GizmoUi<'ui>,
    view: &'ui T,
    projection: &'ui T,
    model: &'ui mut T,
    // options
    operation: Operation,
    mode: Mode,
    delta_out: Option<&'ui mut T>,
    snap: Option<[f32; 3]>,
    local_bounds: Option<[f32; 6]>,
    bounds_snap: Option<[f32; 3]>,
}

impl<'ui, T: Mat4Like> Manipulate<'ui, T> {
    pub(crate) fn new(
        giz: &'ui GizmoUi<'ui>,
        view: &'ui T,
        projection: &'ui T,
        model: &'ui mut T,
    ) -> Self {
        Self {
            giz,
            view,
            projection,
            model,
            operation: Operation::TRANSLATE,
            mode: Mode::Local,
            delta_out: None,
            snap: None,
            local_bounds: None,
            bounds_snap: None,
        }
    }

    pub fn operation(mut self, op: Operation) -> Self {
        self.operation = op;
        self
    }
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
    pub fn delta_out(mut self, out: &'ui mut T) -> Self {
        self.delta_out = Some(out);
        self
    }
    pub fn snap<V: Vec3Like>(mut self, snap: V) -> Self {
        self.snap = Some(snap.to_array());
        self
    }
    pub fn bounds<V: Vec3Like>(mut self, min: V, max: V) -> Self {
        let min = min.to_array();
        let max = max.to_array();
        self.local_bounds = Some([min[0], min[1], min[2], max[0], max[1], max[2]]);
        self
    }
    pub fn bounds_snap<V: Vec3Like>(mut self, snap: V) -> Self {
        self.bounds_snap = Some(snap.to_array());
        self
    }

    /// Typed bounds variant
    pub fn bounds_typed(mut self, b: Bounds) -> Self {
        self.local_bounds = Some([b.min[0], b.min[1], b.min[2], b.max[0], b.max[1], b.max[2]]);
        self
    }

    /// Convenience: translate snapping
    pub fn translate_snap<V: Vec3Like>(mut self, snap: V) -> Self {
        self.snap = Some(snap.to_array());
        self
    }
    /// Convenience: rotate snapping in degrees (uses x component)
    pub fn rotate_snap_deg(mut self, degrees: f32) -> Self {
        self.snap = Some([degrees, 0.0, 0.0]);
        self
    }
    /// Convenience: scale snapping
    pub fn scale_snap<V: Vec3Like>(mut self, snap: V) -> Self {
        self.snap = Some(snap.to_array());
        self
    }

    /// Configure draw destination
    pub fn drawlist(self, target: DrawListTarget) -> Self {
        match target {
            DrawListTarget::Window => self.giz.set_drawlist_window(),
            DrawListTarget::Background => self.giz.set_drawlist_background(),
            DrawListTarget::Foreground => self.giz.set_drawlist_foreground(),
        }
        self
    }
    /// Set gizmo rect (x,y,width,height)
    pub fn rect(self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.giz.set_rect(x, y, w, h);
        self
    }
    /// Set orthographic flag
    pub fn orthographic(self, is_ortho: bool) -> Self {
        self.giz.set_orthographic(is_ortho);
        self
    }
    /// Set clip-space gizmo size
    pub fn gizmo_size_clip_space(self, value: f32) -> Self {
        self.giz.set_gizmo_size_clip_space(value);
        self
    }
    /// Configure axis mask
    pub fn axis_mask(self, mask: AxisMask) -> Self {
        self.giz.set_axis_mask(mask);
        self
    }
    /// Allow axis flip
    pub fn allow_axis_flip(self, value: bool) -> Self {
        self.giz.allow_axis_flip(value);
        self
    }

    /// Executes the manipulation and returns whether it was used this frame.
    pub fn build(self) -> bool {
        let mut model_arr = self.model.to_cols_array();
        let mut delta_arr = match &self.delta_out {
            Some(dm) => dm.to_cols_array(),
            None => T::identity().to_cols_array(),
        };
        let used = unsafe {
            sys::ImGuizmo_Manipulate(
                self.view.to_cols_array().as_ptr(),
                self.projection.to_cols_array().as_ptr(),
                self.operation.into(),
                self.mode.into(),
                model_arr.as_mut_ptr(),
                delta_arr.as_mut_ptr(),
                self.snap
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(std::ptr::null()),
                self.local_bounds
                    .as_ref()
                    .map(|b| b.as_ptr())
                    .unwrap_or(std::ptr::null()),
                self.bounds_snap
                    .as_ref()
                    .map(|b| b.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        };
        self.model.set_from_cols_array(model_arr);
        if let Some(dm) = self.delta_out {
            dm.set_from_cols_array(delta_arr);
        }
        used
    }
}
