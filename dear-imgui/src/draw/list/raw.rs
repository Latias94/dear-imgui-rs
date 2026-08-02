use crate::sys;

use super::super::color::ImColor32;
use super::super::util::{count_to_i32, finite_vec2};
use super::DrawListMut;

/// Non-null native callback accepted by [`DrawListMut::add_callback`].
pub type RawDrawCallback =
    unsafe extern "C" fn(parent_list: *const sys::ImDrawList, command: *const sys::ImDrawCmd);

impl<'ui> DrawListMut<'ui> {
    /// Insert a raw draw callback.
    ///
    /// # Safety
    ///
    /// - `callback` must not unwind across the FFI boundary.
    /// - When `userdata_size` is zero, Dear ImGui stores `userdata` verbatim. Any non-null pointer
    ///   must remain valid until a renderer executes the draw command. A renderer may reject or
    ///   discard callback-bearing data, so this mode must not rely on callback execution to reclaim
    ///   uniquely owned Rust memory.
    /// - When `userdata_size` is non-zero, `userdata` must point to that many readable bytes for
    ///   this call. Dear ImGui copies the bytes synchronously and later supplies a pointer into its
    ///   internal byte buffer. The callback must not free that pointer or assume it is aligned for
    ///   the original Rust type; copy the bytes out or use unaligned reads.
    /// - The callback and userdata interpretation must remain ABI-compatible with every renderer
    ///   that may consume the draw list.
    /// - Arbitrary Rust function pointers are not supported by the import-style WASM provider;
    ///   only provider-native callbacks with a proven shared callback table may be used there.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::Context;
    /// # let mut context = Context::create();
    /// # let ui = context.frame();
    /// unsafe extern "C" fn callback(
    ///     _list: *const dear_imgui_rs::sys::ImDrawList,
    ///     _command: *const dear_imgui_rs::sys::ImDrawCmd,
    /// ) {}
    /// let draw_list = ui.get_window_draw_list();
    /// draw_list.add_callback(callback, std::ptr::null_mut(), 0);
    /// ```
    ///
    /// Rust-owned closure callbacks are intentionally unavailable: a draw command may be dropped
    /// without ever reaching a renderer, so the command stream cannot guarantee closure cleanup.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::Context;
    /// # let mut context = Context::create();
    /// # let ui = context.frame();
    /// let draw_list = ui.get_window_draw_list();
    /// draw_list.add_callback_safe(|| {}).build();
    /// ```
    #[doc(alias = "AddCallback")]
    pub unsafe fn add_callback(
        &self,
        callback: RawDrawCallback,
        userdata: *mut std::os::raw::c_void,
        userdata_size: usize,
    ) {
        assert!(
            userdata_size < (1usize << 31),
            "DrawListMut::add_callback() userdata_size must be smaller than 2^31 bytes"
        );
        assert!(
            userdata_size == 0 || !userdata.is_null(),
            "DrawListMut::add_callback() copied userdata must not be null"
        );
        unsafe {
            sys::ImDrawList_AddCallback(self.draw_list, Some(callback), userdata, userdata_size)
        }
    }

    /// Insert a new draw command (forces a new draw call boundary).
    #[doc(alias = "AddDrawCmd")]
    pub fn add_draw_cmd(&self) {
        unsafe { sys::ImDrawList_AddDrawCmd(self.draw_list) }
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
