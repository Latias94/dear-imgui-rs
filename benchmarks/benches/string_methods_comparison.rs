// Comprehensive benchmark comparing different string handling methods:
// 1. scratch_txt (UiBuffer) - Traditional imgui-rs approach
// 2. ImString - Pre-allocated with null terminator
// 3. CString - Current string_view implementation
// 4. Zero-copy (ImStrv) - Ideal string_view implementation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::ffi::CString;

// ============================================================================
// Helper Structures
// ============================================================================

/// UiBuffer - Traditional scratch buffer approach
struct UiBuffer {
    buffer: Vec<u8>,
    max_len: usize,
}

impl UiBuffer {
    fn new(max_len: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_len,
        }
    }

    fn refresh_buffer(&mut self) {
        if self.buffer.len() > self.max_len {
            self.buffer.clear();
        }
    }

    fn push(&mut self, txt: &str) -> usize {
        let len = self.buffer.len();
        self.buffer.extend(txt.as_bytes());
        self.buffer.push(b'\0');
        len
    }

    fn scratch_txt(&mut self, txt: &str) -> *const i8 {
        self.refresh_buffer();
        let start = self.push(txt);
        unsafe { self.buffer.as_ptr().add(start) as *const i8 }
    }
}

/// ImString - Pre-allocated string with null terminator
struct ImString(Vec<u8>);

impl ImString {
    fn new(value: &str) -> Self {
        let mut v = value.as_bytes().to_vec();
        v.push(b'\0');
        ImString(v)
    }

    fn as_ptr(&self) -> *const i8 {
        self.0.as_ptr() as *const i8
    }
}

/// ImStrv - Zero-copy string view
#[repr(C)]
struct ImStrv {
    begin: *const i8,
    end: *const i8,
}

impl ImStrv {
    fn from_str(s: &str) -> Self {
        Self {
            begin: s.as_ptr() as *const i8,
            end: unsafe { s.as_ptr().add(s.len()) as *const i8 },
        }
    }
}

// ============================================================================
// Benchmark 1: Single String Creation (Different Lengths)
// ============================================================================

fn bench_single_string_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_string_creation");
    
    let test_strings = vec![
        ("short_10", "Button123"),
        ("medium_50", "This is a medium length string for testing purposes"),
        ("long_200", "This is a very long string that is used to test the performance of string creation with longer text. It contains multiple sentences and should be representative of longer labels or text that might appear in a UI. Let's make it even longer to reach 200 characters!"),
    ];
    
    for (name, text) in test_strings.iter() {
        // Method 1: scratch_txt
        group.bench_with_input(BenchmarkId::new("scratch_txt", name), text, |b, text| {
            let mut buffer = UiBuffer::new(1024);
            b.iter(|| {
                let _ptr = buffer.scratch_txt(black_box(text));
            });
        });
        
        // Method 2: ImString
        group.bench_with_input(BenchmarkId::new("imstring", name), text, |b, text| {
            b.iter(|| {
                let _imstr = ImString::new(black_box(text));
            });
        });
        
        // Method 3: CString (current string_view)
        group.bench_with_input(BenchmarkId::new("cstring", name), text, |b, text| {
            b.iter(|| {
                let _cstr = CString::new(black_box(*text)).unwrap();
            });
        });
        
        // Method 4: Zero-copy (ideal string_view)
        group.bench_with_input(BenchmarkId::new("zero_copy", name), text, |b, text| {
            b.iter(|| {
                let _strv = ImStrv::from_str(black_box(text));
            });
        });
    }
    
    group.finish();
}

// ============================================================================
// Benchmark 2: Repeated Operations (Simulating Real UI)
// ============================================================================

fn bench_repeated_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_operations");
    
    // Test case 1: 100 buttons per frame
    let button_labels: Vec<String> = (0..100).map(|i| format!("Button {}", i)).collect();
    
    group.bench_function("100_buttons/scratch_txt", |b| {
        let mut buffer = UiBuffer::new(1024);
        b.iter(|| {
            for label in &button_labels {
                let _ptr = buffer.scratch_txt(black_box(label));
            }
        });
    });
    
    group.bench_function("100_buttons/imstring", |b| {
        b.iter(|| {
            for label in &button_labels {
                let _imstr = ImString::new(black_box(label));
            }
        });
    });
    
    group.bench_function("100_buttons/cstring", |b| {
        b.iter(|| {
            for label in &button_labels {
                let _cstr = CString::new(black_box(label.as_str())).unwrap();
            }
        });
    });
    
    group.bench_function("100_buttons/zero_copy", |b| {
        b.iter(|| {
            for label in &button_labels {
                let _strv = ImStrv::from_str(black_box(label));
            }
        });
    });
    
    // Test case 2: 10 text inputs per frame
    let input_labels: Vec<String> = (0..10)
        .map(|i| format!("Input field {} with some default text", i))
        .collect();
    
    group.bench_function("10_inputs/scratch_txt", |b| {
        let mut buffer = UiBuffer::new(1024);
        b.iter(|| {
            for label in &input_labels {
                let _ptr = buffer.scratch_txt(black_box(label));
            }
        });
    });
    
    group.bench_function("10_inputs/imstring", |b| {
        b.iter(|| {
            for label in &input_labels {
                let _imstr = ImString::new(black_box(label));
            }
        });
    });
    
    group.bench_function("10_inputs/cstring", |b| {
        b.iter(|| {
            for label in &input_labels {
                let _cstr = CString::new(black_box(label.as_str())).unwrap();
            }
        });
    });
    
    group.bench_function("10_inputs/zero_copy", |b| {
        b.iter(|| {
            for label in &input_labels {
                let _strv = ImStrv::from_str(black_box(label));
            }
        });
    });
    
    group.finish();
}

// ============================================================================
// Benchmark 3: Memory Overhead
// ============================================================================

fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_overhead");
    
    let text = "Button with some text";
    
    // Measure allocation overhead
    group.bench_function("scratch_txt_with_buffer_reset", |b| {
        b.iter(|| {
            let mut buffer = UiBuffer::new(1024);
            for _ in 0..100 {
                let _ptr = buffer.scratch_txt(black_box(text));
            }
            // Buffer will accumulate, then reset
        });
    });
    
    group.bench_function("imstring_repeated_allocation", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let _imstr = ImString::new(black_box(text));
            }
        });
    });
    
    group.bench_function("cstring_repeated_allocation", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let _cstr = CString::new(black_box(text)).unwrap();
            }
        });
    });
    
    group.bench_function("zero_copy_no_allocation", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let _strv = ImStrv::from_str(black_box(text));
            }
        });
    });
    
    group.finish();
}

// ============================================================================
// Benchmark 4: Complex UI Scenario
// ============================================================================

fn bench_complex_ui_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_ui_frame");
    
    // Simulate a complex UI frame with mixed widget types
    let window_titles: Vec<String> = (0..5).map(|i| format!("Window {}", i)).collect();
    let button_labels: Vec<String> = (0..50).map(|i| format!("Button {}", i)).collect();
    let input_labels: Vec<String> = (0..10).map(|i| format!("Input {}", i)).collect();
    let text_labels: Vec<String> = (0..20).map(|i| format!("Label {}: Some text here", i)).collect();
    
    group.bench_function("scratch_txt", |b| {
        let mut buffer = UiBuffer::new(2048);
        b.iter(|| {
            for title in &window_titles {
                let _ptr = buffer.scratch_txt(black_box(title));
            }
            for label in &button_labels {
                let _ptr = buffer.scratch_txt(black_box(label));
            }
            for label in &input_labels {
                let _ptr = buffer.scratch_txt(black_box(label));
            }
            for label in &text_labels {
                let _ptr = buffer.scratch_txt(black_box(label));
            }
        });
    });
    
    group.bench_function("cstring", |b| {
        b.iter(|| {
            for title in &window_titles {
                let _cstr = CString::new(black_box(title.as_str())).unwrap();
            }
            for label in &button_labels {
                let _cstr = CString::new(black_box(label.as_str())).unwrap();
            }
            for label in &input_labels {
                let _cstr = CString::new(black_box(label.as_str())).unwrap();
            }
            for label in &text_labels {
                let _cstr = CString::new(black_box(label.as_str())).unwrap();
            }
        });
    });
    
    group.bench_function("zero_copy", |b| {
        b.iter(|| {
            for title in &window_titles {
                let _strv = ImStrv::from_str(black_box(title));
            }
            for label in &button_labels {
                let _strv = ImStrv::from_str(black_box(label));
            }
            for label in &input_labels {
                let _strv = ImStrv::from_str(black_box(label));
            }
            for label in &text_labels {
                let _strv = ImStrv::from_str(black_box(label));
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_string_creation,
    bench_repeated_operations,
    bench_memory_overhead,
    bench_complex_ui_frame
);
criterion_main!(benches);

