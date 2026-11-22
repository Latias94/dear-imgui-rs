use core::alloc::{GlobalAlloc, Layout};

#[link(wasm_import_module = "imgui-sys-v1")]
unsafe extern "C" {
    fn dear_imgui_wasm_memalign(alignment: usize, size: usize) -> *mut u8;
    fn dear_imgui_wasm_free(ptr: *mut u8);
}

/// Uses the repository's Emscripten provider heap for Rust allocations.
///
/// Select this as `#[global_allocator]` in the final browser application.
/// Initialize the provider first and pass its fixed memory to Rust. Link Rust's
/// initial memory at the provider's `GLOBAL_BASE` so the stack, statics and
/// `std::alloc::System` arena fit below the provider (see `docs/WASM.md`).
pub struct WasmProviderAllocator;

// SAFETY: emmalloc's memalign accepts power-of-two alignments, returns disjoint
// storage or null on failure, and pairs with free from the same provider instance.
unsafe impl GlobalAlloc for WasmProviderAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { dear_imgui_wasm_memalign(layout.align(), layout.size()) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { dear_imgui_wasm_free(ptr) }
    }
}
