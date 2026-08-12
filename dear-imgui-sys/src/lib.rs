//! Low-level FFI bindings for Dear ImGui (via cimgui C API) with docking support
//!
//! This crate provides raw, unsafe bindings to Dear ImGui using the cimgui C API,
//! specifically targeting the docking branch (multi-viewport capable).
//!
//! ## Features
//!
//! - **docking**: Always enabled in this crate
//! - **freetype**: Enable FreeType font rasterizer support
//! - **wasm**: Enable WebAssembly compatibility
//! - **stack-layout**: Enable the native patched core artifact used by blueprint-style layouts
//! - **prebuilt**: Allow downloading a compatible native prebuilt artifact
//! - **build-from-source**: Force compilation of the native artifact from vendored sources
//! - **test-engine**: Enable native test engine hooks and force a source build
//! - **backend-shim-\***: Expose selected repository-owned backend shim modules
//!   for low-level integrations
//!
//! ## WebAssembly Support
//!
//! When the `wasm` feature is enabled, this crate provides full WASM compatibility:
//! - Disables platform-specific functions (file I/O, shell functions, etc.)
//! - Configures Dear ImGui for WASM environment
//! - Compatible with wasm-bindgen and web targets
//!
//! ## Safety
//!
//! This crate provides raw FFI bindings and is inherently unsafe. Users should
//! prefer the high-level `dear-imgui-rs` crate for safe Rust bindings.
//!
//! ## Usage
//!
//! This crate is typically not used directly. Instead, use the `dear-imgui-rs` crate
//! which provides safe, idiomatic Rust bindings built on top of these FFI bindings.
//!
//! ## Backend Shim Modules
//!
//! For downstream backend crates, engine integrations, and platform-specific
//! application glue, `dear-imgui-sys` can expose selected official backend
//! pieces through `backend_shim::*`.
//!
//! Important boundary:
//!
//! - these modules expose the repository-owned C shim ABI
//! - they do not expose upstream `imgui_impl_*` C++ symbol names as a stable
//!   Rust-facing contract
//! - enabling `backend-shim-*` features does not imply that `dear-imgui-rs`
//!   already owns a safe wrapper for those backends
//!
//! Typical feature gates:
//!
//! - `backend-shim-opengl3`
//! - `backend-shim-android`
//! - `backend-shim-win32`
//! - `backend-shim-dx11`
//!
//! SDLRenderer3 and SDLGPU3 renderer shims are owned by `dear-imgui-sdl3`.
//!
//! ## Android Direction
//!
//! The current Android story is intentionally low-level but supported.
//!
//! ```toml
//! [dependencies]
//! dear-imgui-rs = "0.10"
//! dear-imgui-sys = { version = "0.10", features = ["backend-shim-android", "backend-shim-opengl3"] }
//! ```
//!
//! Recommended split of responsibilities:
//!
//! - `dear-imgui-rs` owns the safe core `Context`, `Io`, frame lifecycle, and
//!   render snapshots
//! - `dear-imgui-sys::backend_shim::{android, opengl3}` exposes the low-level
//!   official backend pieces
//! - the Android application still owns lifecycle glue, input translation
//!   strategy, EGL / GLES context creation, and packaging
//!
//! The repository's concrete reference for this path is
//! `examples-android/dear-imgui-android-smoke/`, which now carries a minimal
//! NativeActivity + EGL / GLES3 render loop proving that downstream users can
//! build Android support on top of `dear-imgui-rs` + `dear-imgui-sys` even
//! before a dedicated first-party Android convenience crate exists.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(clippy::all)]
// Bindgen may derive Eq/Hash for structs containing function pointers.
// New Clippy lint warns these comparisons are unpredictable; suppress for raw FFI types.
#![allow(unpredictable_function_pointer_comparisons)]

#[cfg(all(feature = "stack-layout", dear_imgui_rs_wasm_import_target))]
compile_error!("feature `stack-layout` is native-only and cannot be combined with WASM");

#[cfg(all(feature = "test-engine", dear_imgui_rs_wasm_import_target))]
compile_error!("feature `test-engine` is native source-only and cannot be combined with WASM");

#[cfg(all(feature = "prebuilt", dear_imgui_rs_wasm_import_target))]
compile_error!("feature `prebuilt` is native-only and cannot be combined with WASM");

// Bindings are generated into OUT_DIR and included via a submodule so that
// possible inner attributes in the generated file are accepted at module root.
mod ffi;
pub use ffi::*;

#[cfg_attr(
    dear_imgui_rs_wasm_import_target,
    link(wasm_import_module = "imgui-sys-v1")
)]
unsafe extern "C" {
    pub fn dear_imgui_rs_show_demo_window_without_font_atlas(p_open: *mut bool);
    pub fn dear_imgui_rs_show_metrics_window_without_font_atlas(p_open: *mut bool);
    pub fn dear_imgui_rs_show_style_editor_without_font_atlas(ref_: *mut ImGuiStyle);
    pub fn dear_imgui_rs_show_font_atlas_debug_panel();
    pub fn dear_imgui_rs_dock_builder_keep_root_alive(root_id: ImGuiID) -> ::std::os::raw::c_int;
    pub fn dear_imgui_rs_dock_builder_root_has_active_content_window(
        root_id: ImGuiID,
    ) -> ::std::os::raw::c_int;
    pub fn dear_imgui_rs_dock_builder_copy_node(
        source_root_id: ImGuiID,
        destination_root_id: ImGuiID,
        remap_data: *mut ImGuiID,
        remap_capacity: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
}

/// Optional backend shim entry points for downstream integrations.
///
/// These modules expose the repository-owned C shim ABI for selected official
/// Dear ImGui backends. They do not expose the upstream C++ symbols directly,
/// and they do not imply that `dear-imgui-sys` or `dear-imgui-rs` owns full
/// safe integration for those backends.
pub mod backend_shim;

// This project always builds Dear ImGui with `IMGUI_DISABLE_OBSOLETE_FUNCTIONS` so the native
// layouts match cimgui's generated struct definitions, and with `IMGUI_USE_WCHAR32` so `ImWchar`
// must be 32-bit.
const _: [(); 4] = [(); std::mem::size_of::<ImWchar>()];

// Ensure common ImGui typedefs are available even if bindgen doesn't emit them explicitly

// cimgui exposes typed vectors (e.g., ImVector_ImVec2) instead of a generic ImVector<T>.
// The sys crate intentionally avoids adding higher-level helpers here.

// Core cimgui calls use the C ABI. PlatformIO aggregate callbacks retain C++ ABI-sensitive
// signatures, so repository-owned shims translate them to pointer/out-parameter callbacks.

/// Whether this build linked the repository-owned PlatformIO aggregate ABI hook shim.
pub const HAS_PLATFORM_IO_AGGREGATE_HOOKS: bool = cfg!(dear_imgui_rs_platform_io_hooks);

#[cfg(feature = "stack-layout")]
mod stack_layout {
    use super::*;

    unsafe extern "C" {
        fn dear_imgui_stack_begin_horizontal_str(
            str_id: *const std::os::raw::c_char,
            size: ImVec2,
            align: f32,
        );
        fn dear_imgui_stack_begin_horizontal_ptr(
            ptr_id: *const std::ffi::c_void,
            size: ImVec2,
            align: f32,
        );
        fn dear_imgui_stack_begin_horizontal_int(id: std::os::raw::c_int, size: ImVec2, align: f32);
        fn dear_imgui_stack_begin_horizontal_id(id: ImGuiID, size: ImVec2, align: f32);
        fn dear_imgui_stack_end_horizontal();
        fn dear_imgui_stack_begin_vertical_str(
            str_id: *const std::os::raw::c_char,
            size: ImVec2,
            align: f32,
        );
        fn dear_imgui_stack_begin_vertical_ptr(
            ptr_id: *const std::ffi::c_void,
            size: ImVec2,
            align: f32,
        );
        fn dear_imgui_stack_begin_vertical_int(id: std::os::raw::c_int, size: ImVec2, align: f32);
        fn dear_imgui_stack_begin_vertical_id(id: ImGuiID, size: ImVec2, align: f32);
        fn dear_imgui_stack_end_vertical();
        fn dear_imgui_stack_spring(weight: f32, spacing: f32);
        fn dear_imgui_stack_suspend_layout();
        fn dear_imgui_stack_resume_layout();
        fn dear_imgui_stack_destroy_context_state(context: *mut ImGuiContext);
        fn dear_imgui_stack_state_count() -> usize;
    }

    /// Start a stack-layout horizontal group using a string ID.
    ///
    /// This is a repository-owned compatibility shim for the stack layout extension
    /// used by `imgui-node-editor` examples; it is not an official Dear ImGui API.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window. `str_id` must point
    /// to a valid NUL-terminated string for the duration of the call.
    #[inline]
    pub unsafe fn ImGuiStack_BeginHorizontal_Str(
        str_id: *const std::os::raw::c_char,
        size: ImVec2,
        align: f32,
    ) {
        unsafe { dear_imgui_stack_begin_horizontal_str(str_id, size, align) }
    }

    /// Start a stack-layout horizontal group using a pointer ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window. `ptr_id` is used as
    /// an ID value only and is not dereferenced.
    #[inline]
    pub unsafe fn ImGuiStack_BeginHorizontal_Ptr(
        ptr_id: *const std::ffi::c_void,
        size: ImVec2,
        align: f32,
    ) {
        unsafe { dear_imgui_stack_begin_horizontal_ptr(ptr_id, size, align) }
    }

    /// Start a stack-layout horizontal group using an integer ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window.
    #[inline]
    pub unsafe fn ImGuiStack_BeginHorizontal_Int(
        id: std::os::raw::c_int,
        size: ImVec2,
        align: f32,
    ) {
        unsafe { dear_imgui_stack_begin_horizontal_int(id, size, align) }
    }

    /// Start a stack-layout horizontal group using a precomputed ImGui ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window.
    #[inline]
    pub unsafe fn ImGuiStack_BeginHorizontal_Id(id: ImGuiID, size: ImVec2, align: f32) {
        unsafe { dear_imgui_stack_begin_horizontal_id(id, size, align) }
    }

    /// End the current stack-layout horizontal group.
    ///
    /// # Safety
    ///
    /// Must match a previous `ImGuiStack_BeginHorizontal_*` call.
    #[inline]
    pub unsafe fn ImGuiStack_EndHorizontal() {
        unsafe { dear_imgui_stack_end_horizontal() }
    }

    /// Start a stack-layout vertical group using a string ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window. `str_id` must point
    /// to a valid NUL-terminated string for the duration of the call.
    #[inline]
    pub unsafe fn ImGuiStack_BeginVertical_Str(
        str_id: *const std::os::raw::c_char,
        size: ImVec2,
        align: f32,
    ) {
        unsafe { dear_imgui_stack_begin_vertical_str(str_id, size, align) }
    }

    /// Start a stack-layout vertical group using a pointer ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window. `ptr_id` is used as
    /// an ID value only and is not dereferenced.
    #[inline]
    pub unsafe fn ImGuiStack_BeginVertical_Ptr(
        ptr_id: *const std::ffi::c_void,
        size: ImVec2,
        align: f32,
    ) {
        unsafe { dear_imgui_stack_begin_vertical_ptr(ptr_id, size, align) }
    }

    /// Start a stack-layout vertical group using an integer ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window.
    #[inline]
    pub unsafe fn ImGuiStack_BeginVertical_Int(id: std::os::raw::c_int, size: ImVec2, align: f32) {
        unsafe { dear_imgui_stack_begin_vertical_int(id, size, align) }
    }

    /// Start a stack-layout vertical group using a precomputed ImGui ID.
    ///
    /// # Safety
    ///
    /// Requires an active Dear ImGui context and current window.
    #[inline]
    pub unsafe fn ImGuiStack_BeginVertical_Id(id: ImGuiID, size: ImVec2, align: f32) {
        unsafe { dear_imgui_stack_begin_vertical_id(id, size, align) }
    }

    /// End the current stack-layout vertical group.
    ///
    /// # Safety
    ///
    /// Must match a previous `ImGuiStack_BeginVertical_*` call.
    #[inline]
    pub unsafe fn ImGuiStack_EndVertical() {
        unsafe { dear_imgui_stack_end_vertical() }
    }

    /// Insert a spring separator into the current stack layout.
    ///
    /// # Safety
    ///
    /// Requires an active stack layout.
    #[inline]
    pub unsafe fn ImGuiStack_Spring(weight: f32, spacing: f32) {
        unsafe { dear_imgui_stack_spring(weight, spacing) }
    }

    /// Temporarily suspend the current stack layout.
    ///
    /// # Safety
    ///
    /// Requires an active stack layout and must be matched by resume.
    #[inline]
    pub unsafe fn ImGuiStack_SuspendLayout() {
        unsafe { dear_imgui_stack_suspend_layout() }
    }

    /// Resume a suspended stack layout.
    ///
    /// # Safety
    ///
    /// Must match a previous suspend call.
    #[inline]
    pub unsafe fn ImGuiStack_ResumeLayout() {
        unsafe { dear_imgui_stack_resume_layout() }
    }

    /// Remove all stack-layout state owned by an ImGui context.
    ///
    /// # Safety
    ///
    /// `context` must be null or point to a live ImGui context. Call this before destroying that
    /// context and while no stack-layout operation is active concurrently.
    #[inline]
    pub unsafe fn ImGuiStack_DestroyContextState(context: *mut ImGuiContext) {
        unsafe { dear_imgui_stack_destroy_context_state(context) }
    }

    /// Return the number of ImGui contexts with retained stack-layout state.
    ///
    /// # Safety
    ///
    /// No stack-layout operation may mutate the native state concurrently.
    #[doc(hidden)]
    #[inline]
    pub unsafe fn ImGuiStack_StateCount() -> usize {
        unsafe { dear_imgui_stack_state_count() }
    }
}

#[cfg(feature = "stack-layout")]
pub use stack_layout::*;

#[cfg(dear_imgui_rs_platform_io_hooks)]
unsafe extern "C" {
    fn dear_imgui_rs_platform_io_set_platform_set_window_pos(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, pos: *const ImVec2)>,
    );

    fn dear_imgui_rs_platform_io_set_platform_get_window_pos(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2)>,
    );

    fn dear_imgui_rs_platform_io_set_platform_set_window_size(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2)>,
    );

    fn dear_imgui_rs_platform_io_set_platform_get_window_size(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_size: *mut ImVec2)>,
    );

    fn dear_imgui_rs_platform_io_set_platform_get_window_framebuffer_scale(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_scale: *mut ImVec2)>,
    );

    fn dear_imgui_rs_platform_io_set_platform_get_window_work_area_insets(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<
            unsafe extern "C" fn(vp: *mut ImGuiViewport, out_insets: *mut ImVec4),
        >,
    );

    fn dear_imgui_rs_platform_io_clear_platform_set_window_pos_if_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, pos: *const ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_clear_platform_set_window_size_if_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_clear_platform_get_window_pos_if_out_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_clear_platform_get_window_size_if_out_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_size: *mut ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_clear_platform_get_window_framebuffer_scale_if_out_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_scale: *mut ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_get_platform_set_window_pos_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        out_callback: *mut Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, pos: *const ImVec2)>,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_get_platform_set_window_size_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        out_callback: *mut Option<
            unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
        >,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_get_platform_get_window_pos_out_param(
        platform_io: *mut ImGuiPlatformIO,
        out_callback: *mut Option<
            unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2),
        >,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_get_platform_get_window_size_out_param(
        platform_io: *mut ImGuiPlatformIO,
        out_callback: *mut Option<
            unsafe extern "C" fn(vp: *mut ImGuiViewport, out_size: *mut ImVec2),
        >,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_get_platform_get_window_framebuffer_scale_out_param(
        platform_io: *mut ImGuiPlatformIO,
        out_callback: *mut Option<
            unsafe extern "C" fn(vp: *mut ImGuiViewport, out_scale: *mut ImVec2),
        >,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_clear_platform_get_window_work_area_insets_if_out_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_insets: *mut ImVec4),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_set_renderer_set_window_size(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2)>,
    );

    fn dear_imgui_rs_platform_io_renderer_set_window_size_matches_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_get_renderer_set_window_size_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        out_callback: *mut Option<
            unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
        >,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_clear_renderer_set_window_size_if_pointer_param(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_probe_aggregate_callbacks(
        platform_io: *mut ImGuiPlatformIO,
        out_result: *mut DearImguiRsPlatformIoAggregateProbeResult,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_invoke_platform_set_window_pos(
        callbacks: *const ImGuiPlatformIO,
        viewport: *mut ImGuiViewport,
        pos: *const ImVec2,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_invoke_platform_set_window_size(
        callbacks: *const ImGuiPlatformIO,
        viewport: *mut ImGuiViewport,
        size: *const ImVec2,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_invoke_platform_get_window_pos(
        callbacks: *const ImGuiPlatformIO,
        viewport: *mut ImGuiViewport,
        out_pos: *mut ImVec2,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_invoke_platform_get_window_size(
        callbacks: *const ImGuiPlatformIO,
        viewport: *mut ImGuiViewport,
        out_size: *mut ImVec2,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_invoke_platform_get_window_framebuffer_scale(
        callbacks: *const ImGuiPlatformIO,
        viewport: *mut ImGuiViewport,
        out_scale: *mut ImVec2,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_invoke_renderer_set_window_size(
        callbacks: *const ImGuiPlatformIO,
        viewport: *mut ImGuiViewport,
        size: *const ImVec2,
    ) -> std::os::raw::c_int;

    fn dear_imgui_rs_platform_io_aggregate_callback_storage_count() -> std::os::raw::c_int;

    fn dear_imgui_rs_find_live_viewport_by_address(
        context: *mut ImGuiContext,
        address: usize,
    ) -> *mut ImGuiViewport;
}

/// Resolve a retained viewport address through the current Context's internal live registry.
///
/// Dear ImGui may change a viewport's numeric ID in place while docking transfers ownership.
/// This repository-owned helper therefore compares the address against `ImGuiContext::Viewports`
/// without dereferencing the retained address.
///
/// # Safety
///
/// `context` must be the current live Context. The returned pointer remains valid only while that
/// Context stays current and no Dear ImGui operation removes the viewport.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiContext_FindLiveViewportByAddress(
    context: *mut ImGuiContext,
    address: usize,
) -> *mut ImGuiViewport {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    {
        unsafe { dear_imgui_rs_find_live_viewport_by_address(context, address) }
    }
    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = (context, address);
        std::ptr::null_mut()
    }
}

/// Results returned by [`ImGuiPlatformIO_ProbeAggregateCallbacks`].
///
/// This test-support ABI is intentionally defined by the repository-owned C++ shim. It lets Rust
/// tests exercise Dear ImGui's real C++ callback slots without calling Rust trampolines directly.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DearImguiRsPlatformIoAggregateProbeResult {
    pub PlatformGetWindowPos: ImVec2,
    pub PlatformGetWindowSize: ImVec2,
    pub PlatformGetWindowFramebufferScale: ImVec2,
    pub PlatformGetWindowWorkAreaInsets: ImVec4,
}

/// C++ measurements for selected public Dear ImGui aggregate layouts.
///
/// This is an internal CI-only ABI probe enabled by the `abi-probe` feature.
#[cfg(feature = "abi-probe")]
#[doc(hidden)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DearImguiRsPublicAggregateLayoutProbe {
    pub ImDrawDataSize: usize,
    pub ImDrawDataAlign: usize,
    pub ImDrawDataFrameCountOffset: usize,
    pub ImDrawDataCmdListsOffset: usize,
    pub ImDrawDataTexturesOffset: usize,
    pub ImTextureDataSize: usize,
    pub ImTextureDataAlign: usize,
    pub ImTextureDataUniqueIDOffset: usize,
    pub ImTextureDataQueueUserDataOffset: usize,
    pub ImTextureDataTexIDOffset: usize,
    pub ImTextureDataUpdatesOffset: usize,
    pub ImTextureDataWantDestroyNextFrameOffset: usize,
    pub ImGuiPlatformIOSize: usize,
    pub ImGuiPlatformIOAlign: usize,
    pub ImGuiPlatformIOSessionDateOffset: usize,
    pub ImGuiPlatformIORenderStateOffset: usize,
    pub ImGuiPlatformIOPlatformCreateWindowOffset: usize,
    pub ImGuiPlatformIORendererCreateWindowOffset: usize,
    pub ImGuiPlatformIOMonitorsOffset: usize,
    pub ImGuiPlatformIOTexturesOffset: usize,
    pub ImGuiPlatformIOViewportsOffset: usize,
}

#[cfg(feature = "abi-probe")]
unsafe extern "C" {
    fn dear_imgui_rs_probe_public_aggregate_layouts(
        out_probe: *mut DearImguiRsPublicAggregateLayoutProbe,
    ) -> std::os::raw::c_int;
}

/// Ask the native C++ translation unit to measure selected aggregate layouts.
///
/// This is test support, not a stable downstream API.
#[cfg(feature = "abi-probe")]
#[doc(hidden)]
#[inline]
pub fn ImGui_ProbePublicAggregateLayouts(
    out_probe: &mut DearImguiRsPublicAggregateLayoutProbe,
) -> bool {
    unsafe { dear_imgui_rs_probe_public_aggregate_layouts(out_probe) != 0 }
}

/// Install a C-compatible pointer-parameter callback for
/// `ImGuiPlatformIO::Platform_SetWindowPos`.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's platform callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, pos: *const ImVec2)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_platform_set_window_pos(platform_io, user_callback)
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_SetWindowPos callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Platform_SetWindowPos = None;
        }
    }
}

/// Install a C-compatible out-parameter callback for `ImGuiPlatformIO::Platform_GetWindowPos`.
///
/// This avoids exposing Rust callbacks through the small-aggregate `ImVec2` return ABI used by
/// Dear ImGui's C++ callback slot. The shim keeps its own per-`ImGuiPlatformIO` storage and does
/// not occupy `ImGuiIO::BackendLanguageUserData`.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's platform callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_platform_get_window_pos(platform_io, user_callback)
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_GetWindowPos callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Platform_GetWindowPos = None;
        }
    }
}

/// Install a C-compatible pointer-parameter callback for
/// `ImGuiPlatformIO::Platform_SetWindowSize`.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's platform callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_platform_set_window_size(platform_io, user_callback)
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_SetWindowSize callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Platform_SetWindowSize = None;
        }
    }
}

/// Install a C-compatible out-parameter callback for `ImGuiPlatformIO::Platform_GetWindowSize`.
///
/// See [`ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam`] for the ABI rationale.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's platform callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_size: *mut ImVec2)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_platform_get_window_size(platform_io, user_callback)
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_GetWindowSize callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Platform_GetWindowSize = None;
        }
    }
}

/// Install a C-compatible out-parameter callback for
/// `ImGuiPlatformIO::Platform_GetWindowFramebufferScale`.
///
/// See [`ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam`] for the ABI rationale.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's platform callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_scale: *mut ImVec2)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_platform_get_window_framebuffer_scale(
            platform_io,
            user_callback,
        )
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install \
                 Platform_GetWindowFramebufferScale callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Platform_GetWindowFramebufferScale = None;
        }
    }
}

/// Install a C-compatible out-parameter callback for
/// `ImGuiPlatformIO::Platform_GetWindowWorkAreaInsets`.
///
/// See [`ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam`] for the ABI rationale.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's platform callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_insets: *mut ImVec4)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_platform_get_window_work_area_insets(
            platform_io,
            user_callback,
        )
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_GetWindowWorkAreaInsets \
                 callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Platform_GetWindowWorkAreaInsets = None;
        }
    }
}

/// Clear `Platform_SetWindowPos` only when it is still owned by the given pointer callback.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearPlatformSetWindowPosIfPointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, pos: *const ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_platform_set_window_pos_if_pointer_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Clear `Platform_SetWindowSize` only when it is still owned by the given pointer callback.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearPlatformSetWindowSizeIfPointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_platform_set_window_size_if_pointer_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Clear `Platform_GetWindowPos` only when it is still owned by the given out-parameter callback.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearPlatformGetWindowPosIfOutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_platform_get_window_pos_if_out_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Clear `Platform_GetWindowSize` only when it is still owned by the given out-parameter callback.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearPlatformGetWindowSizeIfOutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_size: *mut ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_platform_get_window_size_if_out_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Clear `Platform_GetWindowFramebufferScale` only when it is still owned by the given callback.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearPlatformGetWindowFramebufferScaleIfOutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_scale: *mut ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_platform_get_window_framebuffer_scale_if_out_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

macro_rules! define_platform_callback_getter {
    ($name:ident, $native:ident, $callback:ty) => {
        /// Return the exact callback stored behind a shared C++ aggregate hook.
        ///
        /// The query is observationally read-only and returns `None` when the raw slot no longer
        /// points at this repository's hook.
        #[doc(hidden)]
        #[inline]
        pub unsafe fn $name(platform_io: *mut ImGuiPlatformIO) -> Option<$callback> {
            #[cfg(dear_imgui_rs_platform_io_hooks)]
            unsafe {
                let mut callback = None;
                if $native(platform_io, &mut callback) != 0 {
                    return callback;
                }
                None
            }

            #[cfg(not(dear_imgui_rs_platform_io_hooks))]
            {
                let _ = platform_io;
                None
            }
        }
    };
}

define_platform_callback_getter!(
    ImGuiPlatformIO_PlatformSetWindowPosPointerParam,
    dear_imgui_rs_platform_io_get_platform_set_window_pos_pointer_param,
    unsafe extern "C" fn(*mut ImGuiViewport, *const ImVec2)
);
define_platform_callback_getter!(
    ImGuiPlatformIO_PlatformSetWindowSizePointerParam,
    dear_imgui_rs_platform_io_get_platform_set_window_size_pointer_param,
    unsafe extern "C" fn(*mut ImGuiViewport, *const ImVec2)
);
define_platform_callback_getter!(
    ImGuiPlatformIO_PlatformGetWindowPosOutParam,
    dear_imgui_rs_platform_io_get_platform_get_window_pos_out_param,
    unsafe extern "C" fn(*mut ImGuiViewport, *mut ImVec2)
);
define_platform_callback_getter!(
    ImGuiPlatformIO_PlatformGetWindowSizeOutParam,
    dear_imgui_rs_platform_io_get_platform_get_window_size_out_param,
    unsafe extern "C" fn(*mut ImGuiViewport, *mut ImVec2)
);
define_platform_callback_getter!(
    ImGuiPlatformIO_PlatformGetWindowFramebufferScaleOutParam,
    dear_imgui_rs_platform_io_get_platform_get_window_framebuffer_scale_out_param,
    unsafe extern "C" fn(*mut ImGuiViewport, *mut ImVec2)
);

/// Clear `Platform_GetWindowWorkAreaInsets` only when it is still owned by the given callback.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearPlatformGetWindowWorkAreaInsetsIfOutParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, out_insets: *mut ImVec4),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_platform_get_window_work_area_insets_if_out_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Install a C-compatible pointer-parameter callback for
/// `ImGuiPlatformIO::Renderer_SetWindowSize`.
///
/// # Safety
///
/// `platform_io` must be null or point to a live `ImGuiPlatformIO`. `user_callback`, when present,
/// must obey Dear ImGui's renderer callback contract and must not unwind.
#[inline]
pub unsafe fn ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2)>,
) {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        dear_imgui_rs_platform_io_set_renderer_set_window_size(platform_io, user_callback)
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO aggregate ABI hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Renderer_SetWindowSize callbacks"
            );
        } else if let Some(platform_io) = unsafe { platform_io.as_mut() } {
            platform_io.Renderer_SetWindowSize = None;
        }
    }
}

/// Whether `Renderer_SetWindowSize` is currently owned by the given pointer callback.
///
/// This is an internal ownership primitive for renderer backends. It recognizes only callbacks
/// installed through [`ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam`].
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_RendererSetWindowSizeMatchesPointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_renderer_set_window_size_matches_pointer_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Return the pointer-parameter callback currently stored for `Renderer_SetWindowSize`.
///
/// This is an internal ownership primitive for backends that must preserve a foreign aggregate
/// callback replacement even though all pointer-parameter callbacks share one C++ hook address.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_RendererSetWindowSizePointerParam(
    platform_io: *mut ImGuiPlatformIO,
) -> Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2)> {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        let mut callback = None;
        if dear_imgui_rs_platform_io_get_renderer_set_window_size_pointer_param(
            platform_io,
            &mut callback,
        ) != 0
        {
            return callback;
        }
        None
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        None
    }
}

/// Clear `Renderer_SetWindowSize` only when it is still owned by the given pointer callback.
///
/// This is an internal ownership primitive for renderer backends.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_ClearRendererSetWindowSizeIfPointerParam(
    platform_io: *mut ImGuiPlatformIO,
    user_callback: unsafe extern "C" fn(vp: *mut ImGuiViewport, size: *const ImVec2),
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_clear_renderer_set_window_size_if_pointer_param(
            platform_io,
            user_callback,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = user_callback;
        false
    }
}

/// Invoke all aggregate PlatformIO callback slots through C++.
///
/// This is a test-support ABI for validating that callbacks installed by the safe layer cross the
/// C++/Rust seam through pointers or out parameters rather than small aggregates by value.
///
/// # Safety
///
/// `platform_io` and `out_result` must point to valid live values. The current ImGui context must
/// own `platform_io`.
#[inline]
pub unsafe fn ImGuiPlatformIO_ProbeAggregateCallbacks(
    platform_io: *mut ImGuiPlatformIO,
    out_result: *mut DearImguiRsPlatformIoAggregateProbeResult,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_probe_aggregate_callbacks(platform_io, out_result) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = platform_io;
        let _ = out_result;
        false
    }
}

/// Invoke a saved native `Platform_SetWindowPos` callback through C++.
///
/// This prevents Rust from directly calling a C++ callback slot whose signature contains an
/// aggregate by value.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the duration of the call. When all pointers are
/// non-null, `callbacks` must be readable, `viewport` must identify a live viewport accepted by
/// the saved callback, and `pos` must be readable. The populated callback slot must have been
/// copied from a C++ ABI-compatible Dear ImGui backend callback table and must remain callable.
/// The callback must not throw or unwind across the C ABI boundary.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_InvokePlatformSetWindowPos(
    callbacks: *const ImGuiPlatformIO,
    viewport: *mut ImGuiViewport,
    pos: *const ImVec2,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_invoke_platform_set_window_pos(callbacks, viewport, pos)
            != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = callbacks;
        let _ = viewport;
        let _ = pos;
        false
    }
}

/// Invoke a saved native `Platform_SetWindowSize` callback through C++.
///
/// This prevents Rust from directly calling a C++ callback slot whose signature contains an
/// aggregate by value.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the duration of the call. When all pointers are
/// non-null, `callbacks` must be readable, `viewport` must identify a live viewport accepted by
/// the saved callback, and `size` must be readable. The populated callback slot must have been
/// copied from a C++ ABI-compatible Dear ImGui backend callback table and must remain callable.
/// The callback must not throw or unwind across the C ABI boundary.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_InvokePlatformSetWindowSize(
    callbacks: *const ImGuiPlatformIO,
    viewport: *mut ImGuiViewport,
    size: *const ImVec2,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_invoke_platform_set_window_size(
            callbacks, viewport, size,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = callbacks;
        let _ = viewport;
        let _ = size;
        false
    }
}

/// Invoke a saved native `Platform_GetWindowPos` callback through C++.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the duration of the call. `out_pos` must be null or
/// valid for writes. A non-null output is initialized to zero before the other arguments are
/// validated and is overwritten with the callback result on success. When the callback is invoked,
/// `callbacks` must be readable, `viewport` must identify a live viewport accepted by the callback,
/// and the populated slot must have been copied from a C++ ABI-compatible Dear ImGui backend
/// callback table and remain callable. The callback must not throw or unwind across the C ABI
/// boundary.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_InvokePlatformGetWindowPos(
    callbacks: *const ImGuiPlatformIO,
    viewport: *mut ImGuiViewport,
    out_pos: *mut ImVec2,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_invoke_platform_get_window_pos(
            callbacks, viewport, out_pos,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    unsafe {
        if !out_pos.is_null() {
            out_pos.write(ImVec2 { x: 0.0, y: 0.0 });
        }
        let _ = callbacks;
        let _ = viewport;
        false
    }
}

/// Invoke a saved native `Platform_GetWindowSize` callback through C++.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the duration of the call. `out_size` must be null or
/// valid for writes. A non-null output is initialized to zero before the other arguments are
/// validated and is overwritten with the callback result on success. When the callback is invoked,
/// `callbacks` must be readable, `viewport` must identify a live viewport accepted by the callback,
/// and the populated slot must have been copied from a C++ ABI-compatible Dear ImGui backend
/// callback table and remain callable. The callback must not throw or unwind across the C ABI
/// boundary.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_InvokePlatformGetWindowSize(
    callbacks: *const ImGuiPlatformIO,
    viewport: *mut ImGuiViewport,
    out_size: *mut ImVec2,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_invoke_platform_get_window_size(
            callbacks, viewport, out_size,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    unsafe {
        if !out_size.is_null() {
            out_size.write(ImVec2 { x: 0.0, y: 0.0 });
        }
        let _ = callbacks;
        let _ = viewport;
        false
    }
}

/// Invoke a saved native `Platform_GetWindowFramebufferScale` callback through C++.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the duration of the call. `out_scale` must be null
/// or valid for writes. A non-null output is initialized to zero before the other arguments are
/// validated and is overwritten with the callback result on success. When the callback is invoked,
/// `callbacks` must be readable, `viewport` must identify a live viewport accepted by the callback,
/// and the populated slot must have been copied from a C++ ABI-compatible Dear ImGui backend
/// callback table and remain callable. The callback must not throw or unwind across the C ABI
/// boundary.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(
    callbacks: *const ImGuiPlatformIO,
    viewport: *mut ImGuiViewport,
    out_scale: *mut ImVec2,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_invoke_platform_get_window_framebuffer_scale(
            callbacks, viewport, out_scale,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    unsafe {
        if !out_scale.is_null() {
            out_scale.write(ImVec2 { x: 0.0, y: 0.0 });
        }
        let _ = callbacks;
        let _ = viewport;
        false
    }
}

/// Invoke a saved native `Renderer_SetWindowSize` callback through C++.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the duration of the call. When all pointers are
/// non-null, `callbacks` must be readable, `viewport` must identify a live viewport accepted by
/// the saved callback, and `size` must be readable. The populated callback slot must have been
/// copied from a C++ ABI-compatible Dear ImGui backend callback table and must remain callable.
/// The callback must not throw or unwind across the C ABI boundary.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_InvokeRendererSetWindowSize(
    callbacks: *const ImGuiPlatformIO,
    viewport: *mut ImGuiViewport,
    size: *const ImVec2,
) -> bool {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_invoke_renderer_set_window_size(
            callbacks, viewport, size,
        ) != 0;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        let _ = callbacks;
        let _ = viewport;
        let _ = size;
        false
    }
}

/// Return the number of live aggregate callback storage entries.
///
/// This is an internal test-support API. It is not a stable public ABI.
///
/// # Safety
///
/// The C++ hook storage follows Dear ImGui's single-threaded context model. Serialize this read
/// with every aggregate callback install, clear, and context destruction.
#[doc(hidden)]
#[inline]
pub unsafe fn ImGuiPlatformIO_AggregateCallbackStorageCount() -> usize {
    #[cfg(dear_imgui_rs_platform_io_hooks)]
    unsafe {
        return dear_imgui_rs_platform_io_aggregate_callback_storage_count() as usize;
    }

    #[cfg(not(dear_imgui_rs_platform_io_hooks))]
    {
        0
    }
}

// Re-export commonly used types for convenience
pub use ImColor as Color;
pub use ImVec2 as Vector2;
pub use ImVec4 as Vector4;

/// Version of this Rust binding release.
///
/// Use [`igGetVersion`] when the linked Dear ImGui runtime version is required.
pub const BINDING_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Docking features are always available in this crate
pub const HAS_DOCKING: bool = true;

/// Check if FreeType support is available
#[cfg(feature = "freetype")]
pub const HAS_FREETYPE: bool = true;

#[cfg(not(feature = "freetype"))]
pub const HAS_FREETYPE: bool = false;

/// Check if WASM support is available
#[cfg(all(dear_imgui_rs_wasm_import_target, feature = "wasm"))]
pub const HAS_WASM: bool = true;

#[cfg(not(all(dear_imgui_rs_wasm_import_target, feature = "wasm")))]
pub const HAS_WASM: bool = false;

#[cfg(test)]
mod target_contract_tests {
    #[test]
    fn wasm_availability_requires_the_build_selected_import_target() {
        assert_eq!(
            super::HAS_WASM,
            cfg!(all(dear_imgui_rs_wasm_import_target, feature = "wasm"))
        );
    }
}

#[cfg(all(test, not(dear_imgui_rs_platform_io_hooks)))]
mod aggregate_setter_fallback_tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::ptr;

    use super::*;

    unsafe extern "C" fn set_vec2_pointer(_viewport: *mut ImGuiViewport, _value: *const ImVec2) {}

    unsafe extern "C" fn get_vec2_out(_viewport: *mut ImGuiViewport, _value: *mut ImVec2) {}

    unsafe extern "C" fn get_vec4_out(_viewport: *mut ImGuiViewport, _value: *mut ImVec4) {}

    unsafe extern "C" fn raw_set_vec2(_viewport: *mut ImGuiViewport, _value: ImVec2) {}

    unsafe extern "C" fn raw_get_vec2(_viewport: *mut ImGuiViewport) -> ImVec2 {
        ImVec2::zero()
    }

    unsafe extern "C" fn raw_get_vec4(_viewport: *mut ImGuiViewport) -> ImVec4 {
        ImVec4::zero()
    }

    fn assert_panics(action: impl FnOnce()) {
        assert!(catch_unwind(AssertUnwindSafe(action)).is_err());
    }

    #[test]
    fn aggregate_setters_without_cpp_hooks_reject_some_callbacks() {
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(
                ptr::null_mut(),
                Some(set_vec2_pointer),
            );
        });
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(ptr::null_mut(), Some(get_vec2_out));
        });
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(
                ptr::null_mut(),
                Some(set_vec2_pointer),
            );
        });
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                ptr::null_mut(),
                Some(get_vec2_out),
            );
        });
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                ptr::null_mut(),
                Some(get_vec2_out),
            );
        });
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(
                ptr::null_mut(),
                Some(get_vec4_out),
            );
        });
        assert_panics(|| unsafe {
            ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
                ptr::null_mut(),
                Some(set_vec2_pointer),
            );
        });
    }

    #[test]
    fn aggregate_setters_without_cpp_hooks_clear_matching_raw_slots() {
        let mut platform_io = ImGuiPlatformIO {
            Platform_SetWindowPos: Some(raw_set_vec2),
            Platform_GetWindowPos: Some(raw_get_vec2),
            Platform_SetWindowSize: Some(raw_set_vec2),
            Platform_GetWindowSize: Some(raw_get_vec2),
            Platform_GetWindowFramebufferScale: Some(raw_get_vec2),
            Platform_GetWindowWorkAreaInsets: Some(raw_get_vec4),
            Renderer_SetWindowSize: Some(raw_set_vec2),
            ..ImGuiPlatformIO::default()
        };

        unsafe {
            ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(&mut platform_io, None);
        }
        assert!(platform_io.Platform_SetWindowPos.is_none());
        assert!(platform_io.Platform_GetWindowPos.is_some());

        unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(&mut platform_io, None);
        }
        assert!(platform_io.Platform_GetWindowPos.is_none());
        assert!(platform_io.Platform_SetWindowSize.is_some());

        unsafe {
            ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(&mut platform_io, None);
        }
        assert!(platform_io.Platform_SetWindowSize.is_none());
        assert!(platform_io.Platform_GetWindowSize.is_some());

        unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(&mut platform_io, None);
        }
        assert!(platform_io.Platform_GetWindowSize.is_none());
        assert!(platform_io.Platform_GetWindowFramebufferScale.is_some());

        unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(&mut platform_io, None);
        }
        assert!(platform_io.Platform_GetWindowFramebufferScale.is_none());
        assert!(platform_io.Platform_GetWindowWorkAreaInsets.is_some());

        unsafe {
            ImGuiPlatformIO_Set_Platform_GetWindowWorkAreaInsets_OutParam(&mut platform_io, None);
        }
        assert!(platform_io.Platform_GetWindowWorkAreaInsets.is_none());
        assert!(platform_io.Renderer_SetWindowSize.is_some());

        unsafe {
            ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(&mut platform_io, None);
        }
        assert!(platform_io.Renderer_SetWindowSize.is_none());
    }
}

#[cfg(all(test, dear_imgui_rs_platform_io_hooks))]
mod aggregate_callback_identity_tests {
    use super::*;

    type SetCallback = unsafe extern "C" fn(*mut ImGuiViewport, *const ImVec2);
    type GetCallback = unsafe extern "C" fn(*mut ImGuiViewport, *mut ImVec2);

    unsafe extern "C" fn set_a(_viewport: *mut ImGuiViewport, _value: *const ImVec2) {}

    unsafe extern "C" fn set_b(_viewport: *mut ImGuiViewport, _value: *const ImVec2) {}

    unsafe extern "C" fn get_a(_viewport: *mut ImGuiViewport, _value: *mut ImVec2) {}

    unsafe extern "C" fn get_b(_viewport: *mut ImGuiViewport, _value: *mut ImVec2) {}

    fn assert_set_identity(
        platform_io: &mut ImGuiPlatformIO,
        install: unsafe fn(*mut ImGuiPlatformIO, Option<SetCallback>),
        raw_slot_address: fn(&ImGuiPlatformIO) -> usize,
        query: unsafe fn(*mut ImGuiPlatformIO) -> Option<SetCallback>,
    ) {
        unsafe { install(platform_io, Some(set_a)) };
        let hook = raw_slot_address(platform_io);
        unsafe { install(platform_io, Some(set_b)) };
        assert_eq!(raw_slot_address(platform_io), hook);
        assert!(std::ptr::fn_addr_eq(
            unsafe { query(platform_io) }.unwrap(),
            set_b as SetCallback
        ));
        assert!(
            raw_slot_address(platform_io) == hook,
            "query must not mutate the raw slot"
        );
        assert!(
            std::ptr::fn_addr_eq(unsafe { query(platform_io) }.unwrap(), set_b as SetCallback),
            "query must not mutate hook storage"
        );
        unsafe { install(platform_io, None) };
    }

    fn assert_get_identity(
        platform_io: &mut ImGuiPlatformIO,
        install: unsafe fn(*mut ImGuiPlatformIO, Option<GetCallback>),
        raw_slot_address: fn(&ImGuiPlatformIO) -> usize,
        query: unsafe fn(*mut ImGuiPlatformIO) -> Option<GetCallback>,
    ) {
        unsafe { install(platform_io, Some(get_a)) };
        let hook = raw_slot_address(platform_io);
        unsafe { install(platform_io, Some(get_b)) };
        assert_eq!(raw_slot_address(platform_io), hook);
        assert!(std::ptr::fn_addr_eq(
            unsafe { query(platform_io) }.unwrap(),
            get_b as GetCallback
        ));
        assert!(
            raw_slot_address(platform_io) == hook,
            "query must not mutate the raw slot"
        );
        assert!(
            std::ptr::fn_addr_eq(unsafe { query(platform_io) }.unwrap(), get_b as GetCallback),
            "query must not mutate hook storage"
        );
        unsafe { install(platform_io, None) };
    }

    #[test]
    fn platform_vec2_hooks_expose_exact_callback_identity_without_mutation() {
        let mut platform_io = ImGuiPlatformIO::default();

        assert_set_identity(
            &mut platform_io,
            ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam,
            |io| io.Platform_SetWindowPos.unwrap() as usize,
            ImGuiPlatformIO_PlatformSetWindowPosPointerParam,
        );
        assert_set_identity(
            &mut platform_io,
            ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam,
            |io| io.Platform_SetWindowSize.unwrap() as usize,
            ImGuiPlatformIO_PlatformSetWindowSizePointerParam,
        );
        assert_get_identity(
            &mut platform_io,
            ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam,
            |io| io.Platform_GetWindowPos.unwrap() as usize,
            ImGuiPlatformIO_PlatformGetWindowPosOutParam,
        );
        assert_get_identity(
            &mut platform_io,
            ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam,
            |io| io.Platform_GetWindowSize.unwrap() as usize,
            ImGuiPlatformIO_PlatformGetWindowSizeOutParam,
        );
        assert_get_identity(
            &mut platform_io,
            ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam,
            |io| io.Platform_GetWindowFramebufferScale.unwrap() as usize,
            ImGuiPlatformIO_PlatformGetWindowFramebufferScaleOutParam,
        );
    }
}

// (No wasm-specific shims are required when using shared memory import style.)

impl ImVec2 {
    #[inline]
    pub const fn new(x: f32, y: f32) -> ImVec2 {
        ImVec2 { x, y }
    }

    #[inline]
    pub const fn zero() -> ImVec2 {
        ImVec2 { x: 0.0, y: 0.0 }
    }
}

impl From<[f32; 2]> for ImVec2 {
    #[inline]
    fn from(array: [f32; 2]) -> ImVec2 {
        ImVec2::new(array[0], array[1])
    }
}

impl From<(f32, f32)> for ImVec2 {
    #[inline]
    fn from((x, y): (f32, f32)) -> ImVec2 {
        ImVec2::new(x, y)
    }
}

impl From<ImVec2> for [f32; 2] {
    #[inline]
    fn from(v: ImVec2) -> [f32; 2] {
        [v.x, v.y]
    }
}

impl From<ImVec2> for (f32, f32) {
    #[inline]
    fn from(v: ImVec2) -> (f32, f32) {
        (v.x, v.y)
    }
}

impl From<mint::Vector2<f32>> for ImVec2 {
    #[inline]
    fn from(v: mint::Vector2<f32>) -> ImVec2 {
        ImVec2::new(v.x, v.y)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for ImVec2 {
    #[inline]
    fn from(v: glam::Vec2) -> ImVec2 {
        ImVec2::new(v.x, v.y)
    }
}

impl ImVec4 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> ImVec4 {
        ImVec4 { x, y, z, w }
    }

    #[inline]
    pub const fn zero() -> ImVec4 {
        ImVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }
}

impl From<[f32; 4]> for ImVec4 {
    #[inline]
    fn from(array: [f32; 4]) -> ImVec4 {
        ImVec4::new(array[0], array[1], array[2], array[3])
    }
}

impl From<(f32, f32, f32, f32)> for ImVec4 {
    #[inline]
    fn from((x, y, z, w): (f32, f32, f32, f32)) -> ImVec4 {
        ImVec4::new(x, y, z, w)
    }
}

impl From<ImVec4> for [f32; 4] {
    #[inline]
    fn from(v: ImVec4) -> [f32; 4] {
        [v.x, v.y, v.z, v.w]
    }
}

impl From<ImVec4> for (f32, f32, f32, f32) {
    #[inline]
    fn from(v: ImVec4) -> (f32, f32, f32, f32) {
        (v.x, v.y, v.z, v.w)
    }
}

impl From<mint::Vector4<f32>> for ImVec4 {
    #[inline]
    fn from(v: mint::Vector4<f32>) -> ImVec4 {
        ImVec4::new(v.x, v.y, v.z, v.w)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec4> for ImVec4 {
    #[inline]
    fn from(v: glam::Vec4) -> ImVec4 {
        ImVec4::new(v.x, v.y, v.z, v.w)
    }
}
