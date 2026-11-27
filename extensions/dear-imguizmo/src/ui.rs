use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use dear_imgui_rs::{Id, Ui, math::Vec3Like};
use dear_imgui_sys as imgui_sys;
use dear_imguizmo_sys as sys;

use crate::frame::{DrawList, GizmoFrame};
use crate::mat::Mat4Like;
use crate::style::Style;
use crate::types::{AxisMask, DrawListTarget, GuizmoId, Mode, MoveType, Operation};

fn hash_bytes(bytes: &[u8]) -> u32 {
    // ImHashStr treats a zero byte count as a NUL-terminated string.
    if bytes.is_empty() {
        return unsafe { sys::ImGuizmo_GetID_Str(c"".as_ptr()) };
    }
    let range = bytes.as_ptr_range();
    unsafe { sys::ImGuizmo_GetID_StrStr(range.start.cast(), range.end.cast()) }
}

pub struct GizmoUi<'ui> {
    ui: &'ui Ui,
    frame: Rc<GizmoFrame>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GridColors {
    pub major: u32,
    pub minor: u32,
    pub center: u32,
}

fn validate_grid(grid_size: f32, major_step: f32, subdivision: u32) {
    assert!(
        grid_size.is_finite() && grid_size >= 0.0,
        "grid size must be finite and non-negative"
    );
    assert!(
        major_step.is_finite() && major_step > 0.0,
        "grid step must be finite and positive"
    );
    assert!(
        (1..=i32::MAX as u32).contains(&subdivision),
        "grid subdivision must be a positive signed index"
    );
    let minor_step = major_step / subdivision as f32;
    let line_count = (grid_size / minor_step).ceil();
    assert!(
        minor_step > 0.0 && line_count < i32::MAX as f32,
        "grid line count exceeds the native index range"
    );
}

impl<'ui> GizmoUi<'ui> {
    #[inline]
    fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.ui.with_bound_context(|| {
            assert!(
                self.frame.is_open(),
                "ImGuizmo needs its original ImGui frame"
            );
            f()
        })
    }

    fn with_draw_list<R>(&self, f: impl FnOnce(*mut imgui_sys::ImDrawList) -> R) -> R {
        self.with_bound_context(|| f(self.frame.draw_list()))
    }

    pub fn set_rect(&self, x: f32, y: f32, width: f32, height: f32) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_SetRect(x, y, width, height) });
    }
    pub fn set_orthographic(&self, is_ortho: bool) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_SetOrthographic(is_ortho) });
    }
    pub fn allow_axis_flip(&self, enable: bool) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_AllowAxisFlip(enable) });
    }
    pub fn set_gizmo_size_clip_space(&self, value: f32) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_SetGizmoSizeClipSpace(value) });
    }
    pub fn set_axis_limit(&self, value: f32) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_SetAxisLimit(value) });
    }
    pub fn set_plane_limit(&self, value: f32) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_SetPlaneLimit(value) });
    }
    pub fn set_axis_mask(&self, mask: AxisMask) {
        let x = mask.contains(AxisMask::X);
        let y = mask.contains(AxisMask::Y);
        let z = mask.contains(AxisMask::Z);
        self.with_bound_context(|| unsafe { sys::ImGuizmo_SetAxisMask(x, y, z) });
    }

    pub fn draw_grid<T: Mat4Like>(&self, view: &T, projection: &T, model: &T, grid_size: f32) {
        assert!(
            (0.0..16_777_216.0).contains(&grid_size),
            "grid size must allow native unit steps to make progress"
        );
        let view = view.to_cols_array();
        let projection = projection.to_cols_array();
        let model = model.to_cols_array();
        self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_DrawGrid(
                view.as_ptr(),
                projection.as_ptr(),
                model.as_ptr(),
                grid_size,
            )
        });
    }

    pub fn draw_grid_custom<T: Mat4Like>(
        &self,
        view: &T,
        projection: &T,
        model: &T,
        grid_size: f32,
        major_step: f32,
        subdivision: u32,
    ) {
        validate_grid(grid_size, major_step, subdivision);
        let view = view.to_cols_array();
        let projection = projection.to_cols_array();
        let model = model.to_cols_array();
        self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_DrawGridCustom(
                view.as_ptr(),
                projection.as_ptr(),
                model.as_ptr(),
                grid_size,
                major_step,
                subdivision,
            )
        });
    }

    pub fn draw_grid_custom_color<T: Mat4Like>(
        &self,
        view: &T,
        projection: &T,
        model: &T,
        grid_size: f32,
        major_step: f32,
        subdivision: u32,
        colors: GridColors,
    ) {
        validate_grid(grid_size, major_step, subdivision);
        let view = view.to_cols_array();
        let projection = projection.to_cols_array();
        let model = model.to_cols_array();
        self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_DrawGridCustomColor(
                view.as_ptr(),
                projection.as_ptr(),
                model.as_ptr(),
                grid_size,
                major_step,
                subdivision,
                colors.major,
                colors.minor,
                colors.center,
            )
        });
    }

    /// Draw axes using a contiguous slice of column-major model matrices.
    pub fn draw_axes<T: Mat4Like>(&self, view: &T, projection: &T, matrices: &[[f32; 16]]) {
        if matrices.is_empty() {
            return;
        }
        assert!(
            matrices.len() <= i32::MAX as usize / 16,
            "axis matrix count exceeds the native index range"
        );
        let view = view.to_cols_array();
        let projection = projection.to_cols_array();
        self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_DrawAxes(
                view.as_ptr(),
                projection.as_ptr(),
                matrices.as_ptr().cast(),
                matrices.len() as i32,
            )
        });
    }

    /// Draw cubes using a contiguous slice of column-major model matrices.
    pub fn draw_cubes<T: Mat4Like>(&self, view: &T, projection: &T, matrices: &[[f32; 16]]) {
        if matrices.is_empty() {
            return;
        }
        assert!(
            matrices.len() <= i32::MAX as usize / 16,
            "cube matrix count exceeds the native index range"
        );
        let view = view.to_cols_array();
        let projection = projection.to_cols_array();
        self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_DrawCubes(
                view.as_ptr(),
                projection.as_ptr(),
                matrices.as_ptr().cast(),
                matrices.len() as i32,
            )
        });
    }

    /// Select the current window. Draw with this window current or select another target.
    pub fn set_drawlist_window(&self) {
        self.with_bound_context(|| {
            self.frame.select(DrawList::Window(unsafe {
                imgui_sys::igGetCurrentWindowRead()
            }))
        });
    }
    pub fn set_drawlist_background(&self) {
        self.with_bound_context(|| unsafe {
            self.frame
                .select(DrawList::Frame(imgui_sys::igGetBackgroundDrawList(
                    std::ptr::null_mut(),
                )))
        });
    }
    pub fn set_drawlist_foreground(&self) {
        self.with_bound_context(|| unsafe {
            self.frame.select(DrawList::Frame(
                imgui_sys::igGetForegroundDrawList_ViewportPtr(std::ptr::null_mut()),
            ))
        });
    }
    pub fn set_drawlist(&self, target: DrawListTarget) {
        match target {
            DrawListTarget::Window => self.set_drawlist_window(),
            DrawListTarget::Background => self.set_drawlist_background(),
            DrawListTarget::Foreground => self.set_drawlist_foreground(),
        }
    }

    /// Set an alternative ImGui window to consider hovered for ImGuizmo interactions.
    /// Its ID is looked up again in subsequent frames of the same context.
    ///
    /// This corresponds to `ImGuizmo::SetAlternativeWindow(ImGuiWindow*)`.
    ///
    /// Notes:
    /// - `window_name` must match the exact ImGui window name (including any `###id` suffix).
    /// - Returns `true` if the window was found; otherwise clears the alternative window and returns `false`.
    pub fn set_alternative_window_by_name(&self, window_name: &str) -> bool {
        assert!(!window_name.contains('\0'), "window_name contained NUL");
        self.with_bound_context(|| {
            dear_imgui_rs::with_scratch_txt(window_name, |ptr| {
                let window = unsafe { imgui_sys::igFindWindowByName(ptr) };
                let id = std::ptr::NonNull::new(window).map(|w| unsafe { (*w.as_ptr()).ID });
                self.frame.set_alternative(window.cast(), id);
                !window.is_null()
            })
        })
    }

    /// Clear the alternative hovered window for ImGuizmo.
    pub fn clear_alternative_window(&self) {
        self.with_bound_context(|| self.frame.set_alternative(std::ptr::null_mut(), None));
    }

    /// Set a raw `ImGuiWindow*` as ImGuizmo's alternative hovered window.
    ///
    /// # Safety
    ///
    /// A non-null window must belong to this context and remain valid through the frame or until cleared.
    pub unsafe fn set_alternative_window_raw(&self, window: *mut sys::ImGuiWindow) {
        self.with_bound_context(|| self.frame.set_alternative(window, None));
    }

    /// Select a raw ImGui draw list for this frame. A null pointer selects the current window.
    ///
    /// # Safety
    ///
    /// A non-null list and its shared data must stay writable in this context for the frame.
    /// A non-null owner name must remain a valid C string. The caller must uphold any narrower
    /// window, clip, or channel scope on every draw that targets the list.
    pub unsafe fn set_drawlist_raw(&self, drawlist: *mut imgui_sys::ImDrawList) {
        if drawlist.is_null() {
            self.set_drawlist_window();
        } else {
            self.with_bound_context(|| self.frame.select(DrawList::Frame(drawlist)));
        }
    }

    /// Manipulate using the selected draw list. Each frame starts on ImGuizmo's helper window.
    pub fn manipulate<T: Mat4Like>(
        &self,
        view: &T,
        projection: &T,
        operation: Operation,
        mode: Mode,
        model_matrix: &mut T,
        mut delta_matrix: Option<&mut T>,
        snap: Option<&[f32; 3]>,
        local_bounds: Option<&[f32; 6]>,
        bounds_snap: Option<&[f32; 3]>,
    ) -> bool {
        let mut model_arr = model_matrix.to_cols_array();
        let mut delta_arr = match &delta_matrix {
            Some(dm) => dm.to_cols_array(),
            None => T::identity().to_cols_array(),
        };
        let view = view.to_cols_array();
        let projection = projection.to_cols_array();
        let used = self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_Manipulate(
                view.as_ptr(),
                projection.as_ptr(),
                operation.into(),
                mode.into(),
                model_arr.as_mut_ptr(),
                delta_arr.as_mut_ptr(),
                snap.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                local_bounds.map(|b| b.as_ptr()).unwrap_or(std::ptr::null()),
                bounds_snap.map(|b| b.as_ptr()).unwrap_or(std::ptr::null()),
            )
        });
        model_matrix.set_from_cols_array(model_arr);
        if let Some(dm) = &mut delta_matrix {
            dm.set_from_cols_array(delta_arr);
        }
        used
    }

    pub fn view_manipulate<T: Mat4Like>(
        &self,
        view: &mut T,
        length: f32,
        position: impl Into<[f32; 2]>,
        size: impl Into<[f32; 2]>,
        background_color: u32,
    ) -> bool {
        let mut arr = view.to_cols_array();
        let position = position.into();
        let size = size.into();
        let used = self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_ViewManipulate_Float(
                arr.as_mut_ptr(),
                length,
                sys::ImVec2_c {
                    x: position[0],
                    y: position[1],
                },
                sys::ImVec2_c {
                    x: size[0],
                    y: size[1],
                },
                background_color,
            );
            sys::ImGuizmo_IsUsingViewManipulate()
        });
        view.set_from_cols_array(arr);
        used
    }

    /// Extended view manipulator that also takes projection and edits a target matrix.
    pub fn view_manipulate_with_camera<T: Mat4Like>(
        &self,
        view: &mut T,
        projection: &T,
        operation: Operation,
        mode: Mode,
        matrix: &mut T,
        length: f32,
        position: impl Into<[f32; 2]>,
        size: impl Into<[f32; 2]>,
        background_color: u32,
    ) -> bool {
        let mut view_arr = view.to_cols_array();
        let mut matrix_arr = matrix.to_cols_array();
        let projection = projection.to_cols_array();
        let position = position.into();
        let size = size.into();
        let used = self.with_draw_list(|_| unsafe {
            sys::ImGuizmo_ViewManipulate_FloatPtr(
                view_arr.as_mut_ptr(),
                projection.as_ptr(),
                operation.into(),
                mode.into(),
                matrix_arr.as_mut_ptr(),
                length,
                sys::ImVec2_c {
                    x: position[0],
                    y: position[1],
                },
                sys::ImVec2_c {
                    x: size[0],
                    y: size[1],
                },
                background_color,
            );
            sys::ImGuizmo_IsUsingViewManipulate()
        });
        view.set_from_cols_array(view_arr);
        matrix.set_from_cols_array(matrix_arr);
        used
    }

    /// Convenience: set rect from pos/size vectors
    pub fn set_rect_pos_size(&self, pos: impl Into<[f32; 2]>, size: impl Into<[f32; 2]>) {
        let pos = pos.into();
        let size = size.into();
        self.set_rect(pos[0], pos[1], size[0], size[1]);
    }

    pub fn enable(&self, enable: bool) {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_Enable(enable) });
    }
    pub fn is_over(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_IsOver_Nil() })
    }
    pub fn is_using(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_IsUsing() })
    }
    pub fn is_using_any(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_IsUsingAny() })
    }
    pub fn is_using_view_manipulate(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_IsUsingViewManipulate() })
    }
    pub fn is_view_manipulate_hovered(&self) -> bool {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_IsViewManipulateHovered() })
    }
    pub fn is_over_operation(&self, operation: Operation) -> bool {
        self.with_bound_context(|| unsafe { sys::ImGuizmo_IsOver_OPERATION(operation.into()) })
    }

    pub fn active_handle_type(&self) -> MoveType {
        self.with_bound_context(|| {
            MoveType::from_raw(unsafe { sys::ImGuizmo_GetActiveHandleType() })
        })
    }

    pub fn hovered_handle_type(&self) -> MoveType {
        self.with_bound_context(|| {
            MoveType::from_raw(unsafe { sys::ImGuizmo_GetHoveredHandleType() })
        })
    }

    pub fn active_move_type(&self) -> MoveType {
        self.with_bound_context(|| MoveType::from_raw(unsafe { sys::ImGuizmo_GetActiveMoveType() }))
    }

    pub fn hovered_move_type(&self) -> MoveType {
        self.with_bound_context(|| {
            MoveType::from_raw(unsafe { sys::ImGuizmo_GetHoveredMoveType() })
        })
    }

    /// Test if the mouse is within `pixel_radius` of a world position once
    /// ImGuizmo has a valid view/projection context for the current frame.
    ///
    /// Notes:
    /// - This uses ImGuizmo internal `ViewProjection` from the last compute
    ///   (e.g. after calling `manipulate`).
    /// - `position` is a world-space 3D point.
    pub fn is_over_at<V: Vec3Like>(&self, position: V, pixel_radius: f32) -> bool {
        let mut p = position.to_array();
        self.with_bound_context(|| unsafe {
            sys::ImGuizmo_IsOver_FloatPtr(p.as_mut_ptr(), pixel_radius)
        })
    }

    /// Push a frame-local ImGuizmo ID. Dropping an outer token early preserves inner IDs.
    pub fn push_id<'a, I>(&self, id: I) -> IdToken<'ui>
    where
        I: Into<GuizmoId<'a>>,
    {
        let id: GuizmoId<'a> = id.into();
        let slot = self.with_bound_context(|| self.frame.push_id(self.hash_id(id)));
        IdToken {
            frame: Rc::downgrade(&self.frame),
            slot,
            _ui: PhantomData,
        }
    }
    /// Convenience for string ID push without needing to keep the guard name verbose.
    pub fn push_id_str(&self, id: &str) -> IdToken<'ui> {
        self.push_id(GuizmoId::Str(id))
    }
    /// Push an ID using a non-NUL-terminated byte slice (ImGuizmo `str_begin/str_end` form).
    pub fn push_id_bytes<'a>(&self, bytes: &'a [u8]) -> IdToken<'ui> {
        self.push_id(GuizmoId::Bytes(bytes))
    }
    /// Obtain a hashed ID value following ImGuizmo's ID scheme.
    pub fn get_id_str(&self, id: &str) -> Id {
        self.with_bound_context(|| Id::from(self.hash_id(GuizmoId::Str(id))))
    }
    /// Obtain a hashed ID value using a non-NUL-terminated byte slice (ImGuizmo `str_begin/str_end` form).
    pub fn get_id_bytes(&self, bytes: &[u8]) -> Id {
        self.with_bound_context(|| Id::from(hash_bytes(bytes)))
    }

    /// Obtain a hashed ID from a pointer following ImGuizmo's ID scheme.
    pub fn get_id_ptr<T>(&self, ptr: *const T) -> Id {
        self.with_bound_context(|| Id::from(self.hash_id(GuizmoId::Ptr(ptr.cast()))))
    }

    fn hash_id(&self, id: GuizmoId<'_>) -> u32 {
        match id {
            // The C API exposes only the push overload for integers. ImGuizmo's GetID(int)
            // hashes the integer representation with the current ID as its seed.
            GuizmoId::Int(i) => unsafe {
                imgui_sys::igImHashData(
                    std::ptr::from_ref(&i).cast(),
                    size_of::<i32>(),
                    self.frame.current_id(),
                )
            },
            GuizmoId::Str(s) => {
                assert!(!s.contains('\0'), "string contained NUL");
                hash_bytes(s.as_bytes())
            }
            GuizmoId::Bytes(bytes) => hash_bytes(bytes),
            GuizmoId::Ptr(p) => unsafe { sys::ImGuizmo_GetID_Ptr(p) },
        }
    }

    /// Access ImGuizmo global style through a safe wrapper bound to this UI lifetime.
    pub fn style(&self) -> Style<'ui> {
        let ptr = self.with_bound_context(|| unsafe { sys::ImGuizmo_GetStyle() });
        Style {
            ptr,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Start a builder-style manipulation call
    pub fn manipulate_config<T: Mat4Like>(
        &'ui self,
        view: &'ui T,
        projection: &'ui T,
        model: &'ui mut T,
    ) -> crate::op::Manipulate<'ui, T> {
        crate::op::Manipulate::new(self, view, projection, model)
    }
}

/// Restores the nearest still-active parent ID. IDs expire at the next ImGuizmo frame.
#[must_use = "keep the token alive while using this ImGuizmo ID"]
pub struct IdToken<'ui> {
    frame: Weak<GizmoFrame>,
    slot: usize,
    _ui: PhantomData<&'ui Ui>,
}
impl Drop for IdToken<'_> {
    fn drop(&mut self) {
        if let Some(frame) = self.frame.upgrade() {
            frame.release_id(self.slot);
        }
    }
}

/// Extension methods on dear-imgui's Ui to access ImGuizmo in a unified way
pub trait GuizmoExt {
    /// Share ImGuizmo's state for this ImGui frame.
    ///
    /// Switching contexts cancels interaction from the previous context. Panics if that context
    /// still has an open ImGuizmo frame.
    fn guizmo(&self) -> GizmoUi<'_>;
}
impl GuizmoExt for Ui {
    fn guizmo(&self) -> GizmoUi<'_> {
        GizmoUi {
            ui: self,
            frame: GizmoFrame::begin(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GuizmoExt, Mat4Like, sys};
    use dear_imgui_rs::Context;
    use dear_imgui_sys as imgui_sys;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn prepare_imgui(imgui: &mut Context) {
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
    }

    #[test]
    fn nested_ids_survive_an_early_outer_drop_and_expire_with_the_frame() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);

        let root = {
            let ui = imgui.frame();
            let giz = ui.guizmo();
            let root = giz.get_id_str("leaf");
            let outer = giz.push_id(7);
            let inner = ui.guizmo().push_id_bytes(b"inner\0id");
            let nested = giz.get_id_str("leaf");
            assert_ne!(nested, root);
            drop(outer);
            assert_eq!(giz.get_id_str("leaf"), nested);
            drop(inner);
            assert_eq!(giz.get_id_str("leaf"), root);
            std::mem::forget(giz.push_id("frame-local"));
            root
        };
        assert!(imgui.end_frame());
        assert_eq!(imgui.frame().guizmo().get_id_str("leaf"), root);
        assert!(imgui.end_frame());
    }

    #[test]
    fn id_token_drops_on_owner_context_and_restores_previous_current_context() {
        let _guard = test_guard();
        let mut imgui_a = Context::create();
        prepare_imgui(&mut imgui_a);
        let raw_a = imgui_a.as_raw();

        let ui = imgui_a.frame();
        let giz = ui.guizmo();
        let id = giz.push_id(7);

        unsafe { imgui_sys::igSetCurrentContext(std::ptr::null_mut()) };
        drop(id);
        assert_eq!(
            unsafe { imgui_sys::igGetCurrentContext() },
            std::ptr::null_mut()
        );

        unsafe {
            imgui_sys::igSetCurrentContext(raw_a);
        }
        drop(imgui_a.render_legacy());
    }

    #[test]
    fn bound_call_restores_previous_context_after_panic() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);
        let raw = imgui.as_raw();

        let ui = imgui.frame();
        let giz = ui.guizmo();
        unsafe { imgui_sys::igSetCurrentContext(std::ptr::null_mut()) };
        let other = Context::create();
        let other_raw = other.as_raw();

        let result = catch_unwind(AssertUnwindSafe(|| {
            giz.with_bound_context(|| {
                assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, raw);
                panic!("forced panic while ImGuizmo context is bound");
            });
        }));
        assert!(result.is_err());
        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, other_raw);

        drop(other);
        unsafe { imgui_sys::igSetCurrentContext(raw) };
        drop(imgui.render_legacy());
    }

    #[test]
    fn another_context_can_begin_after_the_owner_ends_its_frame() {
        let _guard = test_guard();
        let mut a = Context::create();
        prepare_imgui(&mut a);
        let binding = a.binding();
        let giz = a.frame().guizmo();
        let root = giz.get_id_str("leaf");
        let id = giz.push_id("A");
        let selected = giz.get_id_str("leaf");

        unsafe { imgui_sys::igSetCurrentContext(std::ptr::null_mut()) };
        let mut b = Context::create();
        prepare_imgui(&mut b);
        let b_raw = b.as_raw();
        let ui_b = b.frame();
        assert!(catch_unwind(AssertUnwindSafe(|| ui_b.guizmo())).is_err());
        assert_eq!(giz.get_id_str("leaf"), selected);
        drop(id);
        drop(giz);
        binding.with_bound_context(|| assert!(a.end_frame()));
        assert_eq!(unsafe { imgui_sys::igGetCurrentContext() }, b_raw);
        assert_eq!(ui_b.guizmo().get_id_str("leaf"), root);
        assert!(b.end_frame());
    }

    #[test]
    fn helper_window_preserves_pending_window_data_and_shared_draw_target() {
        let _guard = test_guard();
        let mut ctx = Context::create();
        prepare_imgui(&mut ctx);
        let ui = ctx.frame();
        let size = [240.0, 160.0];
        unsafe { imgui_sys::igSetNextWindowSize(size.into(), imgui_sys::ImGuiCond_Always as _) };
        let giz = ui.guizmo();
        ui.window("Shared target").build(|| {
            assert_eq!(ui.window_size(), size);
            giz.set_drawlist_window();
            let current = unsafe { imgui_sys::igGetWindowDrawList() };
            let helper = unsafe { imgui_sys::igFindWindowByName(c"gizmo".as_ptr()) };
            let begins = unsafe { (*helper).BeginCount };
            let other = ui.guizmo();
            assert_eq!(unsafe { (*helper).BeginCount }, begins);
            assert_eq!(other.with_draw_list(|list| list), current);
        });
        assert!(catch_unwind(AssertUnwindSafe(|| giz.with_draw_list(|_| ()))).is_err());
        giz.set_drawlist_foreground();
        let fg = unsafe { imgui_sys::igGetForegroundDrawList_ViewportPtr(std::ptr::null_mut()) };
        assert_eq!(giz.with_draw_list(|list| list), fg);
        assert!(ctx.end_frame());
    }

    #[test]
    fn culled_manipulation_preserves_the_callers_clip_rectangle() {
        let _guard = test_guard();
        let mut ctx = Context::create();
        prepare_imgui(&mut ctx);
        let ui = ctx.frame();
        let giz = ui.guizmo();
        giz.set_rect(0.0, 0.0, 800.0, 600.0);
        giz.set_orthographic(false);
        giz.set_drawlist_window();
        let raw = unsafe { imgui_sys::igGetWindowDrawList() };
        ui.with_window_draw_list(|draw| {
            draw.with_clip_rect([0.0, 0.0], [120.0, 120.0], || {
                let clips = unsafe { (*raw)._ClipRectStack.Size };
                let identity = <[f32; 16] as Mat4Like>::identity();
                let mut model = identity;
                model[14] = -5.0;
                assert!(
                    !giz.manipulate_config(&identity, &identity, &mut model)
                        .build()
                );
                assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, clips);
            })
        });
        assert!(ctx.end_frame());
    }

    #[test]
    fn get_id_helpers_return_typed_imgui_ids() {
        let _guard = test_guard();
        let mut imgui = Context::create();
        prepare_imgui(&mut imgui);

        let ptr_value = 1234_u32;
        {
            let ui = imgui.frame();
            let giz = ui.guizmo();

            let str_id = giz.get_id_str("typed-id");
            let expected_str = dear_imgui_rs::with_scratch_txt("typed-id", |ptr| unsafe {
                sys::ImGuizmo_GetID_Str(ptr)
            });
            assert_eq!(str_id.raw(), expected_str);

            let bytes = b"typed-id";
            let bytes_id = giz.get_id_bytes(bytes);
            let range = bytes.as_ptr_range();
            let expected_bytes = unsafe {
                sys::ImGuizmo_GetID_StrStr(
                    range.start.cast::<std::os::raw::c_char>(),
                    range.end.cast::<std::os::raw::c_char>(),
                )
            };
            assert_eq!(bytes_id.raw(), expected_bytes);

            // Both allocations have a readable terminator. The empty range must ignore its tail.
            let empty = &b"unused\0"[..0];
            assert_eq!(giz.get_id_bytes(empty), giz.get_id_str(""));
            let leaf = giz.get_id_str("leaf");
            let _empty = giz.push_id_bytes(empty);
            assert_eq!(giz.get_id_str("leaf"), leaf);

            let ptr_id = giz.get_id_ptr(&ptr_value);
            let expected_ptr =
                unsafe { sys::ImGuizmo_GetID_Ptr((&ptr_value as *const u32).cast()) };
            assert_eq!(ptr_id.raw(), expected_ptr);
        }
        drop(imgui.render_legacy());
    }
}
