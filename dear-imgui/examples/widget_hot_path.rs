//! Headless widget timing and allocation counts after warming both allocators.
//!
//! Run with `cargo run --release -p dear-imgui-rs --example widget_hot_path`.
//! Use the same example, toolchain, features and workload in both checkouts.
//! Pass `-- --native-only` to time the same submissions without widget wrappers.
//! Pass `-- --foreign-current` to start each frame with a second context current.

use std::alloc::{GlobalAlloc, Layout, System};
use std::ffi::{CString, c_void};
use std::hint::black_box;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use dear_imgui_rs::{Condition, Context, FramePrepareOptions, Ui, sys};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
static MEASURE: AtomicBool = AtomicBool::new(false);
static RUST_ALLOCS: AtomicUsize = AtomicUsize::new(0);
static NATIVE_ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(&RUST_ALLOCS);
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(&RUST_ALLOCS);
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }
}

fn count(counter: &AtomicUsize) {
    if MEASURE.load(Relaxed) {
        counter.fetch_add(1, Relaxed);
    }
}

struct NativeAllocator {
    alloc: sys::ImGuiMemAllocFunc,
    free: sys::ImGuiMemFreeFunc,
    data: *mut c_void,
}

unsafe extern "C" fn native_alloc(size: usize, data: *mut c_void) -> *mut c_void {
    count(&NATIVE_ALLOCS);
    let original = unsafe { &*data.cast::<NativeAllocator>() };
    match original.alloc {
        Some(alloc) => unsafe { alloc(size, original.data) },
        None => ptr::null_mut(),
    }
}

unsafe extern "C" fn native_free(ptr: *mut c_void, data: *mut c_void) {
    let original = unsafe { &*data.cast::<NativeAllocator>() };
    if let Some(free) = original.free {
        unsafe { free(ptr, original.data) };
    }
}

struct NativeMeter(*mut NativeAllocator);

impl NativeMeter {
    fn install() -> Self {
        let mut original = Box::new(NativeAllocator {
            alloc: None,
            free: None,
            data: ptr::null_mut(),
        });
        unsafe {
            sys::igGetAllocatorFunctions(
                &mut original.alloc,
                &mut original.free,
                &mut original.data,
            );
            assert!(original.alloc.is_some() && original.free.is_some());
        }
        let original = Box::into_raw(original);
        unsafe {
            sys::igSetAllocatorFunctions(Some(native_alloc), Some(native_free), original.cast());
        }
        Self(original)
    }
}

impl Drop for NativeMeter {
    fn drop(&mut self) {
        unsafe {
            let original = &*self.0;
            sys::igSetAllocatorFunctions(original.alloc, original.free, original.data);
            drop(Box::from_raw(self.0));
        }
    }
}

fn draw_widgets(ui: &Ui, windows: &[String], labels: &[String]) {
    ui.window("Widget benchmark")
        .size([800.0, 560.0], Condition::Always)
        .build(|| {
            for label in labels {
                ui.text("Value");
                black_box(ui.button(label));
            }
        });
    for title in windows {
        ui.window(title)
            .size([180.0, 96.0], Condition::Always)
            .build(|| ui.text("View"));
    }
}

fn draw_native(ui: &Ui, windows: &[CString], labels: &[CString]) {
    let value = c"Value".as_ptr();
    let view = c"View".as_ptr();
    ui.with_bound_context(|| unsafe {
        sys::igSetNextWindowSize([800.0, 560.0].into(), Condition::Always as _);
        if sys::igBegin(c"Widget benchmark".as_ptr(), ptr::null_mut(), 0) {
            for label in labels {
                sys::igTextUnformatted(value, value.add(5));
                black_box(sys::igButton(label.as_ptr(), [0.0, 0.0].into()));
            }
        }
        sys::igEnd();
        for title in windows {
            sys::igSetNextWindowSize([180.0, 96.0].into(), Condition::Always as _);
            if sys::igBegin(title.as_ptr(), ptr::null_mut(), 0) {
                sys::igTextUnformatted(view, view.add(4));
            }
            sys::igEnd();
        }
    });
}

fn main() {
    const WARMUP: usize = 16;
    const SAMPLES: usize = 41;

    let _native = NativeMeter::install();
    let mut context = Context::create();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    context.prepare_frame(FramePrepareOptions::new([1280.0, 720.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let _previous = std::env::args()
        .any(|arg| arg == "--foreign-current")
        .then(Context::create);
    let windows: Vec<_> = (0..500).map(|index| format!("View {index}")).collect();
    let labels: Vec<_> = (0..10_000).map(|index| format!("Button {index}")).collect();
    let native_only = std::env::args().any(|arg| arg == "--native-only");
    let c_strings = |values: &[String]| {
        values
            .iter()
            .map(|value| CString::new(value.as_str()).unwrap())
            .collect::<Vec<_>>()
    };
    let native_windows = c_strings(&windows);
    let native_labels = c_strings(&labels);
    let draw = |ui: &Ui| {
        if native_only {
            draw_native(ui, &native_windows, &native_labels);
        } else {
            draw_widgets(ui, &windows, &labels);
        }
    };

    for _ in 0..WARMUP {
        draw(context.frame());
        drop(context.render_legacy());
    }

    let mut samples = [Duration::ZERO; SAMPLES];
    for sample in &mut samples {
        let ui = context.frame();
        let start = Instant::now();
        MEASURE.store(true, Relaxed);
        draw(ui);
        MEASURE.store(false, Relaxed);
        *sample = start.elapsed();
        drop(context.render_legacy());
    }
    samples.sort_unstable();
    println!("median: {} µs", samples[SAMPLES / 2].as_micros());
    println!(
        "allocations over {SAMPLES} frames: {} Rust, {} Dear ImGui",
        RUST_ALLOCS.load(Relaxed),
        NATIVE_ALLOCS.load(Relaxed)
    );
}
