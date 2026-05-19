use crate::sys;

use super::super::callback::Callback;
use super::super::color::ImColor32;
use super::super::util::{count_to_i32, finite_vec2};
use super::DrawListMut;

impl<'ui> DrawListMut<'ui> {
    /// Insert a raw draw callback.
    ///
    /// # Safety
    ///
    /// - `callback` must be an `extern "C"` function compatible with `ImDrawCallback` and must not unwind
    ///   across the FFI boundary.
    /// - `userdata` must remain valid until the draw list is executed by the renderer.
    /// - If you allocate memory and store its pointer in `userdata`, you are responsible for reclaiming it
    ///   from within the callback or otherwise ensuring no leaks occur. Note that callbacks are only invoked
    ///   if the draw list is actually rendered.
    #[doc(alias = "AddCallback")]
    pub unsafe fn add_callback(
        &self,
        callback: sys::ImDrawCallback,
        userdata: *mut std::os::raw::c_void,
        userdata_size: usize,
    ) {
        unsafe { sys::ImDrawList_AddCallback(self.draw_list, callback, userdata, userdata_size) }
    }

    /// Insert a new draw command (forces a new draw call boundary).
    #[doc(alias = "AddDrawCmd")]
    pub fn add_draw_cmd(&self) {
        unsafe { sys::ImDrawList_AddDrawCmd(self.draw_list) }
    }

    /// Clone the current draw list output into an owned, independent copy.
    ///
    /// The returned draw list is heap-allocated by Dear ImGui and will be destroyed on drop.
    ///
    /// # Panics
    ///
    /// Panics if the draw list contains user callbacks. Dear ImGui's clone operation copies
    /// callback userdata as an opaque pointer, which cannot be duplicated safely by this safe API.
    #[doc(alias = "CloneOutput")]
    pub fn clone_output(&self) -> crate::render::OwnedDrawList {
        unsafe {
            crate::render::draw_data::assert_draw_list_cloneable(
                self.draw_list.cast_const(),
                "DrawListMut::clone_output",
            );
            crate::render::OwnedDrawList::from_raw(sys::ImDrawList_CloneOutput(self.draw_list))
        }
    }
}

impl<'ui> DrawListMut<'ui> {
    /// Safe variant: add a Rust callback (executed when the draw list is rendered).
    /// Note: if the draw list is never rendered, the callback will not run and its resources won't be reclaimed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_callback_safe<F: FnOnce() + 'static>(&'ui self, callback: F) -> Callback<'ui, F> {
        Callback::new(self, callback)
    }

    /// Safe variant: add a Rust callback (executed when the draw list is rendered).
    ///
    /// On wasm32 targets using the import-style Dear ImGui provider, C code cannot
    /// safely invoke Rust function pointers across module boundaries. For now this
    /// API is disabled on wasm to avoid undefined behaviour; use other mechanisms
    /// (e.g. higher-level rendering hooks) instead.
    #[cfg(target_arch = "wasm32")]
    pub fn add_callback_safe<F: FnOnce() + 'static>(&'ui self, _callback: F) -> Callback<'ui, F> {
        panic!(
            "DrawListMut::add_callback_safe is not supported on wasm32 targets; \
             C->Rust callbacks are not available in the import-style web build."
        );
    }
}

impl<'ui> DrawListMut<'ui> {
    /// Unsafe low-level geometry API: reserve index and vertex space.
    ///
    /// # Safety
    /// Caller must write exactly the reserved amount using `prim_write_*` and ensure valid topology.
    pub unsafe fn prim_reserve(&self, idx_count: usize, vtx_count: usize) {
        let idx_count = count_to_i32("DrawListMut::prim_reserve()", "idx_count", idx_count);
        let vtx_count = count_to_i32("DrawListMut::prim_reserve()", "vtx_count", vtx_count);
        unsafe { sys::ImDrawList_PrimReserve(self.draw_list, idx_count, vtx_count) }
    }

    /// Unsafe low-level geometry API: unreserve previously reserved space.
    ///
    /// # Safety
    /// Must match a prior call to `prim_reserve` which hasn't been fully written.
    pub unsafe fn prim_unreserve(&self, idx_count: usize, vtx_count: usize) {
        let idx_count = count_to_i32("DrawListMut::prim_unreserve()", "idx_count", idx_count);
        let vtx_count = count_to_i32("DrawListMut::prim_unreserve()", "vtx_count", vtx_count);
        unsafe { sys::ImDrawList_PrimUnreserve(self.draw_list, idx_count, vtx_count) }
    }

    /// Unsafe low-level geometry API: append a rectangle primitive with a single color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_rect(
        &self,
        a: impl Into<sys::ImVec2>,
        b: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let a = finite_vec2("DrawListMut::prim_rect()", "a", a);
        let b = finite_vec2("DrawListMut::prim_rect()", "b", b);
        unsafe { sys::ImDrawList_PrimRect(self.draw_list, a, b, col.into().into()) }
    }

    /// Unsafe low-level geometry API: append a rectangle primitive with UVs and color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_rect_uv(
        &self,
        a: impl Into<sys::ImVec2>,
        b: impl Into<sys::ImVec2>,
        uv_a: impl Into<sys::ImVec2>,
        uv_b: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let a = finite_vec2("DrawListMut::prim_rect_uv()", "a", a);
        let b = finite_vec2("DrawListMut::prim_rect_uv()", "b", b);
        let uv_a = finite_vec2("DrawListMut::prim_rect_uv()", "uv_a", uv_a);
        let uv_b = finite_vec2("DrawListMut::prim_rect_uv()", "uv_b", uv_b);

        unsafe { sys::ImDrawList_PrimRectUV(self.draw_list, a, b, uv_a, uv_b, col.into().into()) }
    }

    /// Unsafe low-level geometry API: append a quad primitive with UVs and color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_quad_uv(
        &self,
        a: impl Into<sys::ImVec2>,
        b: impl Into<sys::ImVec2>,
        c: impl Into<sys::ImVec2>,
        d: impl Into<sys::ImVec2>,
        uv_a: impl Into<sys::ImVec2>,
        uv_b: impl Into<sys::ImVec2>,
        uv_c: impl Into<sys::ImVec2>,
        uv_d: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let a = finite_vec2("DrawListMut::prim_quad_uv()", "a", a);
        let b = finite_vec2("DrawListMut::prim_quad_uv()", "b", b);
        let c = finite_vec2("DrawListMut::prim_quad_uv()", "c", c);
        let d = finite_vec2("DrawListMut::prim_quad_uv()", "d", d);
        let uv_a = finite_vec2("DrawListMut::prim_quad_uv()", "uv_a", uv_a);
        let uv_b = finite_vec2("DrawListMut::prim_quad_uv()", "uv_b", uv_b);
        let uv_c = finite_vec2("DrawListMut::prim_quad_uv()", "uv_c", uv_c);
        let uv_d = finite_vec2("DrawListMut::prim_quad_uv()", "uv_d", uv_d);

        unsafe {
            sys::ImDrawList_PrimQuadUV(
                self.draw_list,
                a,
                b,
                c,
                d,
                uv_a,
                uv_b,
                uv_c,
                uv_d,
                col.into().into(),
            )
        }
    }

    /// Unsafe low-level geometry API: write a vertex.
    ///
    /// # Safety
    /// Only use to fill space reserved by `prim_reserve`.
    pub unsafe fn prim_write_vtx(
        &self,
        pos: impl Into<sys::ImVec2>,
        uv: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let pos = finite_vec2("DrawListMut::prim_write_vtx()", "pos", pos);
        let uv = finite_vec2("DrawListMut::prim_write_vtx()", "uv", uv);
        unsafe { sys::ImDrawList_PrimWriteVtx(self.draw_list, pos, uv, col.into().into()) }
    }

    /// Unsafe low-level geometry API: write an index.
    ///
    /// # Safety
    /// Only use to fill space reserved by `prim_reserve`.
    pub unsafe fn prim_write_idx(&self, idx: sys::ImDrawIdx) {
        unsafe { sys::ImDrawList_PrimWriteIdx(self.draw_list, idx) }
    }

    /// Unsafe low-level geometry API: convenience to append one vertex (pos+uv+col).
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_vtx(
        &self,
        pos: impl Into<sys::ImVec2>,
        uv: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let pos = finite_vec2("DrawListMut::prim_vtx()", "pos", pos);
        let uv = finite_vec2("DrawListMut::prim_vtx()", "uv", uv);
        unsafe { sys::ImDrawList_PrimVtx(self.draw_list, pos, uv, col.into().into()) }
    }
}
