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
//! - `backend-shim-sdlrenderer3`
//! - `backend-shim-android`
//! - `backend-shim-win32`
//! - `backend-shim-dx11`
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

// Bindings are generated into OUT_DIR and included via a submodule so that
// possible inner attributes in the generated file are accepted at module root.
mod ffi;
pub use ffi::*;

/// Optional backend shim entry points for downstream integrations.
///
/// These modules expose the repository-owned C shim ABI for selected official
/// Dear ImGui backends. They do not expose the upstream C++ symbols directly,
/// and they do not imply that `dear-imgui-sys` or `dear-imgui-rs` owns full
/// safe integration for those backends.
pub mod backend_shim;

// This project always builds Dear ImGui with `IMGUI_USE_WCHAR32`, so `ImWchar` must be 32-bit.
const _: [(); 4] = [(); std::mem::size_of::<ImWchar>()];

// Ensure common ImGui typedefs are available even if bindgen doesn't emit them explicitly

// cimgui exposes typed vectors (e.g., ImVector_ImVec2) instead of a generic ImVector<T>.
// The sys crate intentionally avoids adding higher-level helpers here.

// cimgui C API avoids C++ ABI pitfalls; no MSVC-specific conversions are required.

/// Whether this build linked the repository-owned PlatformIO out-parameter hook shim.
pub const HAS_PLATFORM_IO_OUT_PARAM_HOOKS: bool = cfg!(dear_imgui_rs_platform_io_hooks);

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
pub unsafe fn ImGuiStack_BeginHorizontal_Int(id: std::os::raw::c_int, size: ImVec2, align: f32) {
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

#[cfg(dear_imgui_rs_platform_io_hooks)]
unsafe extern "C" {
    fn dear_imgui_rs_platform_io_set_platform_get_window_pos(
        platform_io: *mut ImGuiPlatformIO,
        user_callback: Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2)>,
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
        let _ = platform_io;
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO out-parameter hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_GetWindowPos callbacks"
            );
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
        let _ = platform_io;
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO out-parameter hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_GetWindowSize callbacks"
            );
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
        let _ = platform_io;
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO out-parameter hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install \
                 Platform_GetWindowFramebufferScale callbacks"
            );
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
        let _ = platform_io;
        if user_callback.is_some() {
            panic!(
                "dear-imgui-sys was built without PlatformIO out-parameter hooks; \
                 rebuild without IMGUI_SYS_SKIP_CC to install Platform_GetWindowWorkAreaInsets \
                 callbacks"
            );
        }
    }
}

// Re-export commonly used types for convenience
pub use ImColor as Color;
pub use ImVec2 as Vector2;
pub use ImVec4 as Vector4;

/// Version information for the Dear ImGui library
pub const IMGUI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Docking features are always available in this crate
pub const HAS_DOCKING: bool = true;

/// Check if FreeType support is available
#[cfg(feature = "freetype")]
pub const HAS_FREETYPE: bool = true;

#[cfg(not(feature = "freetype"))]
pub const HAS_FREETYPE: bool = false;

/// Check if WASM support is available
#[cfg(feature = "wasm")]
pub const HAS_WASM: bool = true;

#[cfg(not(feature = "wasm"))]
pub const HAS_WASM: bool = false;

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
